use super::core::open_write_connection;
use rusqlite::{params, Connection};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProtonCatalogSeed {
    #[serde(default = "default_proton_seed_schema")]
    schema_version: u32,
    #[serde(default)]
    families: Vec<ProtonFamilySeed>,
    #[serde(default)]
    sources: Vec<ProtonSourceSeed>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProtonFamiliesModule {
    #[serde(default = "default_proton_seed_schema")]
    schema_version: u32,
    #[serde(default)]
    families: Vec<ProtonFamilySeed>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProtonFamilySourcesModule {
    #[serde(default)]
    family_key: String,
    #[serde(default)]
    sources: Vec<ProtonSourceSeed>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProtonFamilySeed {
    family_key: String,
    display_name: String,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    sort_order: i64,
    #[serde(default)]
    detect_patterns: Vec<String>,
    #[serde(default)]
    builtin: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProtonSourceSeed {
    #[serde(default)]
    family_key: String,
    #[serde(default = "default_provider")]
    provider: String,
    #[serde(default)]
    repo: String,
    #[serde(default)]
    endpoint: String,
    #[serde(default)]
    url_template: String,
    #[serde(default = "default_asset_index")]
    asset_index: i64,
    #[serde(default = "default_asset_pattern")]
    asset_pattern: String,
    #[serde(default = "default_tag_pattern")]
    tag_pattern: String,
    #[serde(default = "default_max_count")]
    max_count: i64,
    #[serde(default)]
    include_prerelease: bool,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    note: String,
}

#[derive(Debug, Clone)]
pub struct ProtonFamilyRecord {
    pub family_key: String,
    pub display_name: String,
    pub enabled: bool,
    pub sort_order: i64,
    pub detect_patterns_json: String,
    pub builtin: bool,
}

#[derive(Debug, Clone)]
pub struct ProtonSourceRecord {
    pub id: Option<i64>,
    pub family_key: String,
    pub provider: String,
    pub repo: String,
    pub endpoint: String,
    pub url_template: String,
    pub asset_index: i64,
    pub asset_pattern: String,
    pub tag_pattern: String,
    pub max_count: i64,
    pub include_prerelease: bool,
    pub enabled: bool,
    pub note: String,
}

pub(super) fn ensure_proton_catalog_schema(conn: &Connection) {
    let columns = conn
        .prepare("PRAGMA table_info(proton_sources)")
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map([], |row| row.get::<_, String>(1))
                .ok()
                .map(|rows| rows.filter_map(|row| row.ok()).collect::<Vec<String>>())
        })
        .unwrap_or_default();

    if !columns.iter().any(|column| column == "endpoint") {
        let _ = conn.execute(
            "ALTER TABLE proton_sources ADD COLUMN endpoint TEXT NOT NULL DEFAULT ''",
            [],
        );
    }
    if !columns.iter().any(|column| column == "url_template") {
        let _ = conn.execute(
            "ALTER TABLE proton_sources ADD COLUMN url_template TEXT NOT NULL DEFAULT ''",
            [],
        );
    }
    if !columns.iter().any(|column| column == "asset_index") {
        let _ = conn.execute(
            "ALTER TABLE proton_sources ADD COLUMN asset_index INTEGER NOT NULL DEFAULT -1",
            [],
        );
    }
}

pub(super) fn ensure_proton_catalog_seed(conn: &Connection) {
    let seed = match load_proton_catalog_seed() {
        Ok(seed) => seed,
        Err(err) => {
            tracing::error!("读取 Proton 目录种子失败: {}", err);
            return;
        }
    };

    let _ = conn.execute(
        "INSERT OR REPLACE INTO proton_catalog_meta (key, value) VALUES ('schema_version', ?1)",
        params![seed.schema_version.to_string()],
    );

    for family in seed.families {
        let family_key = family.family_key.trim();
        if family_key.is_empty() {
            continue;
        }
        if let Ok(patterns_json) = serde_json::to_string(&family.detect_patterns) {
            let _ = conn.execute(
                "INSERT OR IGNORE INTO proton_families
                 (family_key, display_name, enabled, sort_order, detect_patterns_json, builtin, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
                params![
                    family_key,
                    family.display_name.trim(),
                    family.enabled as i64,
                    family.sort_order,
                    patterns_json,
                    family.builtin as i64
                ],
            );
        }
    }

    for source in seed.sources {
        let family_key = source.family_key.trim();
        if family_key.is_empty() {
            continue;
        }
        let _ = conn.execute(
            "INSERT OR IGNORE INTO proton_sources
             (family_key, provider, repo, endpoint, url_template, asset_index, asset_pattern, tag_pattern, max_count, include_prerelease, enabled, note, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, datetime('now'))",
            params![
                family_key,
                source.provider.trim(),
                source.repo.trim(),
                source.endpoint.trim(),
                source.url_template.trim(),
                source.asset_index,
                source.asset_pattern.trim(),
                source.tag_pattern.trim(),
                source.max_count,
                source.include_prerelease as i64,
                source.enabled as i64,
                source.note.trim(),
            ],
        );
    }

    let _ = conn.execute(
        "UPDATE proton_sources
         SET enabled = 0,
             note = 'Deprecated upstream (404), configure a new DW source manually',
             updated_at = datetime('now')
         WHERE family_key = 'dw-proton'
           AND provider = 'github_releases'
           AND repo = 'AUNaseef/proton-ge-custom'",
        [],
    );

    let _ = conn.execute(
        "UPDATE proton_sources
         SET asset_index = 1,
             updated_at = datetime('now')
         WHERE family_key = 'ge-proton'
           AND provider = 'github_releases'
           AND repo = 'GloriousEggroll/proton-ge-custom'
           AND asset_index = -1",
        [],
    );
    let _ = conn.execute(
        "UPDATE proton_sources
         SET asset_index = 1,
             updated_at = datetime('now')
         WHERE family_key = 'proton-cachyos'
           AND provider = 'github_releases'
           AND repo = 'CachyOS/proton-cachyos'
           AND asset_index = -1",
        [],
    );
    let _ = conn.execute(
        "UPDATE proton_sources
         SET asset_index = 1,
             updated_at = datetime('now')
         WHERE family_key = 'dw-proton'
           AND provider = 'forgejo_releases'
           AND endpoint LIKE '%/dawn-winery/dwproton/releases%'
           AND asset_index = -1",
        [],
    );
    let _ = conn.execute(
        "UPDATE proton_sources
         SET asset_index = 1,
             updated_at = datetime('now')
         WHERE family_key = 'proton-sarek'
           AND provider = 'github_releases'
           AND repo = 'pythonlover02/Proton-Sarek'
           AND asset_index = -1",
        [],
    );
    let _ = conn.execute(
        "UPDATE proton_sources
         SET asset_pattern = '.*',
             updated_at = datetime('now')
         WHERE family_key = 'proton-sarek'
           AND provider = 'github_releases'
           AND repo = 'pythonlover02/Proton-Sarek'",
        [],
    );
    let _ = conn.execute(
        "UPDATE proton_sources
         SET tag_pattern = '.*',
             updated_at = datetime('now')
         WHERE family_key = 'proton-sarek'
           AND provider = 'github_releases'
           AND repo = 'pythonlover02/Proton-Sarek'
           AND tag_pattern = '^(?!.*Sarek9-13).*$'",
        [],
    );
    let _ = conn.execute(
        "UPDATE proton_sources
         SET asset_index = 0,
             updated_at = datetime('now')
         WHERE family_key = 'proton-sarek-async'
           AND provider = 'github_releases'
           AND repo = 'pythonlover02/Proton-Sarek'
           AND asset_index = -1",
        [],
    );
    let _ = conn.execute(
        "UPDATE proton_sources
         SET asset_pattern = '.*',
             updated_at = datetime('now')
         WHERE family_key = 'proton-sarek-async'
           AND provider = 'github_releases'
           AND repo = 'pythonlover02/Proton-Sarek'",
        [],
    );
    let _ = conn.execute(
        "UPDATE proton_sources
         SET tag_pattern = '.*',
             updated_at = datetime('now')
         WHERE family_key = 'proton-sarek-async'
           AND provider = 'github_releases'
           AND repo = 'pythonlover02/Proton-Sarek'
           AND tag_pattern = '^(?!.*Sarek9-13).*$'",
        [],
    );
    let _ = conn.execute(
        "UPDATE proton_sources
         SET asset_index = 1,
             updated_at = datetime('now')
         WHERE family_key = 'proton-em'
           AND provider = 'github_releases'
           AND repo = 'Etaash-mathamsetty/Proton'
           AND asset_index = -1",
        [],
    );
    let _ = conn.execute(
        "UPDATE proton_sources
         SET asset_index = 0,
             updated_at = datetime('now')
         WHERE family_key = 'proton-tkg'
          AND provider = 'github_actions'
          AND endpoint LIKE '%/wine-tkg-git/actions/workflows/29873769/runs%'
          AND asset_index = -1",
        [],
    );

    let _ = conn.execute(
        "DELETE FROM proton_sources
         WHERE family_key IN ('proton-tkg', 'custom')",
        [],
    );
    let _ = conn.execute(
        "DELETE FROM proton_families
         WHERE family_key IN ('proton-tkg', 'custom')",
        [],
    );
}

pub fn list_proton_family_rows() -> Vec<ProtonFamilyRecord> {
    let Ok(conn) = open_write_connection() else {
        return Vec::new();
    };
    let mut stmt = match conn.prepare(
        "SELECT family_key, display_name, enabled, sort_order, detect_patterns_json, builtin
         FROM proton_families
         ORDER BY sort_order ASC, family_key ASC",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    let rows = match stmt.query_map([], |row| {
        Ok(ProtonFamilyRecord {
            family_key: row.get::<_, String>(0)?,
            display_name: row.get::<_, String>(1)?,
            enabled: row.get::<_, i64>(2)? != 0,
            sort_order: row.get::<_, i64>(3)?,
            detect_patterns_json: row.get::<_, String>(4)?,
            builtin: row.get::<_, i64>(5)? != 0,
        })
    }) {
        Ok(rows) => rows,
        Err(_) => return Vec::new(),
    };
    rows.filter_map(|row| row.ok()).collect()
}

pub fn list_proton_source_rows() -> Vec<ProtonSourceRecord> {
    let Ok(conn) = open_write_connection() else {
        return Vec::new();
    };
    let mut stmt = match conn.prepare(
        "SELECT id, family_key, provider, repo, endpoint, url_template, asset_index, asset_pattern, tag_pattern, max_count, include_prerelease, enabled, note
         FROM proton_sources
         ORDER BY family_key ASC, id ASC",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    let rows = match stmt.query_map([], |row| {
        Ok(ProtonSourceRecord {
            id: Some(row.get::<_, i64>(0)?),
            family_key: row.get::<_, String>(1)?,
            provider: row.get::<_, String>(2)?,
            repo: row.get::<_, String>(3)?,
            endpoint: row.get::<_, String>(4)?,
            url_template: row.get::<_, String>(5)?,
            asset_index: row.get::<_, i64>(6)?,
            asset_pattern: row.get::<_, String>(7)?,
            tag_pattern: row.get::<_, String>(8)?,
            max_count: row.get::<_, i64>(9)?,
            include_prerelease: row.get::<_, i64>(10)? != 0,
            enabled: row.get::<_, i64>(11)? != 0,
            note: row.get::<_, String>(12)?,
        })
    }) {
        Ok(rows) => rows,
        Err(_) => return Vec::new(),
    };
    rows.filter_map(|row| row.ok()).collect()
}

pub fn replace_proton_catalog_rows(
    families: &[ProtonFamilyRecord],
    sources: &[ProtonSourceRecord],
) -> Result<(), String> {
    let mut conn = open_write_connection()?;
    let tx = conn
        .transaction()
        .map_err(|e| format!("开始 Proton 目录事务失败: {}", e))?;

    tx.execute("DELETE FROM proton_sources", [])
        .map_err(|e| format!("清理 proton_sources 失败: {}", e))?;
    tx.execute("DELETE FROM proton_families", [])
        .map_err(|e| format!("清理 proton_families 失败: {}", e))?;

    for family in families {
        tx.execute(
            "INSERT INTO proton_families
             (family_key, display_name, enabled, sort_order, detect_patterns_json, builtin, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
            params![
                family.family_key,
                family.display_name,
                family.enabled as i64,
                family.sort_order,
                family.detect_patterns_json,
                family.builtin as i64
            ],
        )
        .map_err(|e| format!("写入 proton_families 失败 ({}): {}", family.family_key, e))?;
    }

    for source in sources {
        tx.execute(
            "INSERT INTO proton_sources
             (family_key, provider, repo, endpoint, url_template, asset_index, asset_pattern, tag_pattern, max_count, include_prerelease, enabled, note, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, datetime('now'))",
            params![
                source.family_key,
                source.provider,
                source.repo,
                source.endpoint,
                source.url_template,
                source.asset_index,
                source.asset_pattern,
                source.tag_pattern,
                source.max_count,
                source.include_prerelease as i64,
                source.enabled as i64,
                source.note
            ],
        )
        .map_err(|e| format!("写入 proton_sources 失败 ({}): {}", source.repo, e))?;
    }

    tx.commit()
        .map_err(|e| format!("提交 Proton 目录事务失败: {}", e))
}

fn default_true() -> bool {
    true
}

fn default_provider() -> String {
    "github_releases".to_string()
}

fn default_asset_pattern() -> String {
    "(?i)\\.tar\\.(gz|xz)$".to_string()
}

fn default_asset_index() -> i64 {
    -1
}

fn default_tag_pattern() -> String {
    ".*".to_string()
}

fn default_max_count() -> i64 {
    15
}

fn default_proton_seed_schema() -> u32 {
    2
}

fn load_proton_catalog_seed() -> Result<ProtonCatalogSeed, String> {
    if crate::utils::data_parameters::resolve_data_path("proton/families.json").is_some() {
        let modular_result = (|| -> Result<ProtonCatalogSeed, String> {
            let raw = crate::utils::data_parameters::read_data_json("proton/families.json")?;
            let module = serde_json::from_str::<ProtonFamiliesModule>(&raw)
                .map_err(|e| format!("解析 proton/families.json 失败: {}", e))?;

            if module.families.is_empty() {
                return Err("proton/families.json 中 families 为空".to_string());
            }

            let mut sources = Vec::new();
            for family in &module.families {
                let family_key = family.family_key.trim();
                if family_key.is_empty() {
                    continue;
                }
                let rel = format!("proton/{}/sources.json", family_key);
                let Some(source_path) = crate::utils::data_parameters::resolve_data_path(&rel)
                else {
                    continue;
                };

                let source_raw = std::fs::read_to_string(&source_path)
                    .map_err(|e| format!("读取 {} 失败: {}", source_path.display(), e))?;
                let source_module = serde_json::from_str::<ProtonFamilySourcesModule>(&source_raw)
                    .map_err(|e| format!("解析 {} 失败: {}", source_path.display(), e))?;

                for mut source in source_module.sources {
                    if source.family_key.trim().is_empty() {
                        if !source_module.family_key.trim().is_empty() {
                            source.family_key = source_module.family_key.trim().to_string();
                        } else {
                            source.family_key = family_key.to_string();
                        }
                    }
                    sources.push(source);
                }
            }

            Ok(ProtonCatalogSeed {
                schema_version: module.schema_version,
                families: module.families,
                sources,
            })
        })();

        match modular_result {
            Ok(seed) => {
                tracing::info!(
                    "已加载模块化 Proton 配置: families={}, sources={}",
                    seed.families.len(),
                    seed.sources.len()
                );
                return Ok(seed);
            }
            Err(err) => {
                tracing::warn!("读取模块化 Proton 配置失败，回退 catalog: {}", err);
            }
        }
    }

    let seed_json = crate::utils::data_parameters::read_catalog_json("proton_catalog.seed.json")?;
    serde_json::from_str::<ProtonCatalogSeed>(&seed_json)
        .map_err(|e| format!("解析 proton_catalog.seed.json 失败: {}", e))
}
