use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Mutex;

/// 全局数据库连接（存放在 ~/.config/ssmt4/ssmt4.db）
static DB: once_cell::sync::Lazy<Mutex<Connection>> = once_cell::sync::Lazy::new(|| {
    let db_path = get_db_path();
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let conn = Connection::open(&db_path)
        .unwrap_or_else(|e| panic!("无法打开数据库 {}: {}", db_path.display(), e));
    init_tables(&conn);
    tracing::info!("SQLite 数据库已打开: {}", db_path.display());
    Mutex::new(conn)
});

fn get_db_path() -> PathBuf {
    super::app_config::get_app_config_dir().join("ssmt4.db")
}

fn init_tables(conn: &Connection) {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS game_configs (
            game_name TEXT PRIMARY KEY,
            config    TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS game_configs_v2 (
            game_name      TEXT PRIMARY KEY,
            schema_version INTEGER NOT NULL,
            config_json    TEXT NOT NULL,
            updated_at     TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS game_key_aliases (
            alias_key     TEXT PRIMARY KEY,
            canonical_key TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS game_identities (
            canonical_key TEXT PRIMARY KEY,
            display_name_en TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS game_presets (
            id         TEXT PRIMARY KEY,
            preset_json TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS proton_families (
            family_key TEXT PRIMARY KEY,
            display_name TEXT NOT NULL,
            enabled INTEGER NOT NULL DEFAULT 1,
            sort_order INTEGER NOT NULL DEFAULT 0,
            detect_patterns_json TEXT NOT NULL DEFAULT '[]',
            builtin INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS proton_sources (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            family_key TEXT NOT NULL,
            provider TEXT NOT NULL,
            repo TEXT NOT NULL,
            endpoint TEXT NOT NULL DEFAULT '',
            url_template TEXT NOT NULL DEFAULT '',
            asset_index INTEGER NOT NULL DEFAULT -1,
            asset_pattern TEXT NOT NULL DEFAULT '(?i)\\.tar\\.(gz|xz)$',
            tag_pattern TEXT NOT NULL DEFAULT '.*',
            max_count INTEGER NOT NULL DEFAULT 15,
            include_prerelease INTEGER NOT NULL DEFAULT 0,
            enabled INTEGER NOT NULL DEFAULT 1,
            note TEXT NOT NULL DEFAULT '',
            updated_at TEXT NOT NULL
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_proton_sources_family_provider_repo
            ON proton_sources (family_key, provider, repo);

        CREATE TABLE IF NOT EXISTS proton_catalog_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS migration_meta (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS hash_cache (
            file_path TEXT PRIMARY KEY,
            file_size INTEGER NOT NULL,
            mtime_sec INTEGER NOT NULL,
            md5       TEXT NOT NULL
        );
        ",
    )
    .expect("创建数据库表失败");
    ensure_proton_catalog_schema(conn);
    ensure_game_catalog_seed(conn);
    ensure_proton_catalog_seed(conn);
}

fn ensure_proton_catalog_schema(conn: &Connection) {
    let columns = conn
        .prepare("PRAGMA table_info(proton_sources)")
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map([], |row| row.get::<_, String>(1))
                .ok()
                .map(|rows| rows.filter_map(|r| r.ok()).collect::<Vec<String>>())
        })
        .unwrap_or_default();

    if !columns.iter().any(|c| c == "endpoint") {
        let _ = conn.execute(
            "ALTER TABLE proton_sources ADD COLUMN endpoint TEXT NOT NULL DEFAULT ''",
            [],
        );
    }
    if !columns.iter().any(|c| c == "url_template") {
        let _ = conn.execute(
            "ALTER TABLE proton_sources ADD COLUMN url_template TEXT NOT NULL DEFAULT ''",
            [],
        );
    }
    if !columns.iter().any(|c| c == "asset_index") {
        let _ = conn.execute(
            "ALTER TABLE proton_sources ADD COLUMN asset_index INTEGER NOT NULL DEFAULT -1",
            [],
        );
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeedCatalog {
    identities: Vec<SeedIdentity>,
    presets: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeedIdentity {
    canonical_key: String,
    display_name_en: String,
    #[serde(default)]
    legacy_aliases: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct IdentityRecord {
    pub canonical_key: String,
    pub display_name_en: String,
    pub legacy_aliases: Vec<String>,
}

const GAME_CATALOG_SEED_JSON: &str =
    include_str!("../../resources/bootstrap/game_catalog.seed.json");
const PROTON_CATALOG_SEED_JSON: &str =
    include_str!("../../resources/bootstrap/proton_catalog.seed.json");

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
    1
}

fn ensure_game_catalog_seed(conn: &Connection) {
    let seed = match serde_json::from_str::<SeedCatalog>(GAME_CATALOG_SEED_JSON) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("解析游戏目录种子失败: {}", e);
            return;
        }
    };

    for identity in seed.identities {
        if identity.canonical_key.trim().is_empty() {
            continue;
        }
        if let Err(e) = conn.execute(
            "INSERT OR IGNORE INTO game_identities (canonical_key, display_name_en) VALUES (?1, ?2)",
            params![identity.canonical_key.trim(), identity.display_name_en.trim()],
        ) {
            tracing::warn!(
                "写入 game_identities 失败: key={}, err={}",
                identity.canonical_key,
                e
            );
        }

        for alias in identity.legacy_aliases {
            let alias = alias.trim();
            if alias.is_empty() {
                continue;
            }
            if let Err(e) = conn.execute(
                "INSERT OR IGNORE INTO game_key_aliases (alias_key, canonical_key) VALUES (?1, ?2)",
                params![alias, identity.canonical_key.trim()],
            ) {
                tracing::warn!(
                    "写入 game_key_aliases 失败: alias={}, canonical={}, err={}",
                    alias,
                    identity.canonical_key,
                    e
                );
            }
        }
    }

    for preset in seed.presets {
        let Some(id) = preset
            .get("id")
            .and_then(|v| v.as_str())
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
        else {
            continue;
        };

        let existing_json: Option<String> = conn
            .query_row(
                "SELECT preset_json FROM game_presets WHERE id = ?1 LIMIT 1",
                params![id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .ok()
            .flatten();

        if let Some(existing_json) = existing_json {
            let mut existing_value = match serde_json::from_str::<serde_json::Value>(&existing_json)
            {
                Ok(v) => v,
                Err(_) => serde_json::json!({}),
            };
            let mut changed = merge_missing_json_fields(&mut existing_value, &preset);
            changed |= sync_managed_preset_fields(&mut existing_value, &preset);
            if changed {
                if let Ok(merged_json) = serde_json::to_string_pretty(&existing_value) {
                    if let Err(e) = conn.execute(
                        "UPDATE game_presets SET preset_json = ?2, updated_at = datetime('now') WHERE id = ?1",
                        params![id, merged_json],
                    ) {
                        tracing::warn!("更新 game_presets 失败: id={}, err={}", id, e);
                    }
                }
            }
            continue;
        }

        if let Ok(json) = serde_json::to_string_pretty(&preset) {
            if let Err(e) = conn.execute(
                "INSERT OR IGNORE INTO game_presets (id, preset_json, updated_at) VALUES (?1, ?2, datetime('now'))",
                params![id, json],
            ) {
                tracing::warn!("写入 game_presets 失败: id={}, err={}", id, e);
            }
        }
    }
}

fn ensure_proton_catalog_seed(conn: &Connection) {
    let seed = match serde_json::from_str::<ProtonCatalogSeed>(PROTON_CATALOG_SEED_JSON) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("解析 Proton 目录种子失败: {}", e);
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

    // 兼容修正：旧默认 DW 源已失效（GitHub API 404），避免每次刷新都产生失败日志。
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

    // 兼容修正：补齐关键来源的 asset_index（旧库字段默认 -1，导致部分项目匹配不到资产）
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
}

fn merge_missing_json_fields(target: &mut serde_json::Value, defaults: &serde_json::Value) -> bool {
    match (target, defaults) {
        (serde_json::Value::Object(target_obj), serde_json::Value::Object(default_obj)) => {
            let mut changed = false;
            for (key, default_value) in default_obj {
                match target_obj.get_mut(key) {
                    Some(existing_value) => {
                        changed |= merge_missing_json_fields(existing_value, default_value);
                    }
                    None => {
                        target_obj.insert(key.clone(), default_value.clone());
                        changed = true;
                    }
                }
            }
            changed
        }
        _ => false,
    }
}

fn sync_managed_preset_fields(
    target: &mut serde_json::Value,
    defaults: &serde_json::Value,
) -> bool {
    let serde_json::Value::Object(target_obj) = target else {
        return false;
    };
    let serde_json::Value::Object(default_obj) = defaults else {
        return false;
    };

    let mut changed = false;
    for key in [
        "requireProtectionBeforeLaunch",
        "forceDirectProton",
        "forceDisablePressureVessel",
        "enableNetworkLogByDefault",
        "telemetryServers",
        "telemetryDlls",
        "channelProtection",
    ] {
        let Some(default_value) = default_obj.get(key) else {
            continue;
        };
        let need_update = target_obj.get(key) != Some(default_value);
        if need_update {
            target_obj.insert(key.to_string(), default_value.clone());
            changed = true;
        }
    }
    changed
}

fn resolve_canonical_key_with_conn(conn: &Connection, input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(found) = conn.query_row(
        "SELECT canonical_key FROM game_identities WHERE lower(canonical_key) = lower(?1) LIMIT 1",
        params![trimmed],
        |row| row.get::<_, String>(0),
    ) {
        return Some(found);
    }

    conn.query_row(
        "SELECT canonical_key FROM game_key_aliases WHERE lower(alias_key) = lower(?1) LIMIT 1",
        params![trimmed],
        |row| row.get::<_, String>(0),
    )
    .ok()
}

fn canonical_key_with_conn(conn: &Connection, input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    resolve_canonical_key_with_conn(conn, trimmed).unwrap_or_else(|| trimmed.to_string())
}

fn canonical_key(input: &str) -> String {
    let conn = DB.lock().unwrap();
    canonical_key_with_conn(&conn, input)
}

fn lookup_candidates(input: &str) -> Vec<String> {
    let raw = input.trim().to_string();
    if raw.is_empty() {
        return Vec::new();
    }

    let conn = DB.lock().unwrap();
    let canonical = canonical_key_with_conn(&conn, &raw);
    let mut result = Vec::new();
    result.push(canonical.clone());
    if !raw.eq_ignore_ascii_case(&canonical) {
        result.push(raw);
    }

    if let Ok(mut stmt) = conn
        .prepare("SELECT alias_key FROM game_key_aliases WHERE lower(canonical_key) = lower(?1)")
    {
        if let Ok(rows) = stmt.query_map(params![canonical], |row| row.get::<_, String>(0)) {
            for alias in rows.filter_map(|r| r.ok()) {
                if !result.iter().any(|item| item.eq_ignore_ascii_case(&alias)) {
                    result.push(alias);
                }
            }
        }
    }

    result
}

// ============================
//  通用 KV 操作 — settings 表
// ============================

pub fn get_setting(key: &str) -> Option<String> {
    let conn = DB.lock().unwrap();
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .ok()
}

pub fn set_setting(key: &str, value: &str) {
    let conn = DB.lock().unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
        params![key, value],
    )
    .unwrap_or_else(|e| {
        tracing::error!("写入 settings 失败: key={}, err={}", key, e);
        0
    });
}

pub fn get_all_settings() -> Vec<(String, String)> {
    let conn = DB.lock().unwrap();
    let mut stmt = conn.prepare("SELECT key, value FROM settings").unwrap();
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .unwrap();
    rows.filter_map(|r| r.ok()).collect()
}

// ============================
//  游戏配置操作 — game_configs 表
// ============================

pub fn get_game_config(game_name: &str) -> Option<String> {
    let candidates = lookup_candidates(game_name);
    let conn = DB.lock().unwrap();
    for key in candidates {
        if let Ok(content) = conn.query_row(
            "SELECT config FROM game_configs WHERE game_name = ?1",
            params![key],
            |row| row.get(0),
        ) {
            return Some(content);
        }
    }
    None
}

pub fn set_game_config(game_name: &str, config_json: &str) {
    let canonical = canonical_key(game_name);
    let conn = DB.lock().unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO game_configs (game_name, config) VALUES (?1, ?2)",
        params![&canonical, config_json],
    )
    .unwrap_or_else(|e| {
        tracing::error!("写入 game_configs 失败: game={}, err={}", canonical, e);
        0
    });
}

pub fn delete_game_config(game_name: &str) {
    let keys = lookup_candidates(game_name);
    let conn = DB.lock().unwrap();
    for key in keys {
        conn.execute(
            "DELETE FROM game_configs WHERE game_name = ?1",
            params![key],
        )
        .unwrap_or_else(|e| {
            tracing::error!("删除 game_configs 失败: game={}, err={}", game_name, e);
            0
        });
    }
}

pub fn list_game_names() -> Vec<String> {
    use std::collections::BTreeSet;
    let conn = DB.lock().unwrap();
    let mut stmt = conn.prepare("SELECT game_name FROM game_configs").unwrap();
    let rows = stmt.query_map([], |row| row.get::<_, String>(0)).unwrap();
    let mut names = BTreeSet::new();
    for name in rows.filter_map(|r| r.ok()) {
        names.insert(canonical_key_with_conn(&conn, &name));
    }
    names.into_iter().collect()
}

pub fn list_game_names_raw() -> Vec<String> {
    let conn = DB.lock().unwrap();
    let mut stmt = conn.prepare("SELECT game_name FROM game_configs").unwrap();
    let rows = stmt.query_map([], |row| row.get(0)).unwrap();
    rows.filter_map(|r| r.ok()).collect()
}

pub fn get_game_config_v2(game_name: &str) -> Option<String> {
    let candidates = lookup_candidates(game_name);
    let conn = DB.lock().unwrap();
    for key in candidates {
        if let Ok(content) = conn.query_row(
            "SELECT config_json FROM game_configs_v2 WHERE game_name = ?1",
            params![key],
            |row| row.get(0),
        ) {
            return Some(content);
        }
    }
    None
}

pub fn set_game_config_v2(game_name: &str, schema_version: u32, config_json: &str) {
    let canonical = canonical_key(game_name);
    let conn = DB.lock().unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO game_configs_v2 (game_name, schema_version, config_json, updated_at) VALUES (?1, ?2, ?3, datetime('now'))",
        params![&canonical, schema_version, config_json],
    )
    .unwrap_or_else(|e| {
        tracing::error!(
            "写入 game_configs_v2 失败: game={}, version={}, err={}",
            canonical,
            schema_version,
            e
        );
        0
    });
}

pub fn delete_game_config_v2(game_name: &str) {
    let keys = lookup_candidates(game_name);
    let conn = DB.lock().unwrap();
    for key in keys {
        conn.execute(
            "DELETE FROM game_configs_v2 WHERE game_name = ?1",
            params![key],
        )
        .unwrap_or_else(|e| {
            tracing::error!("删除 game_configs_v2 失败: game={}, err={}", game_name, e);
            0
        });
    }
}

pub fn get_game_config_exact(game_name: &str) -> Option<String> {
    let conn = DB.lock().unwrap();
    conn.query_row(
        "SELECT config FROM game_configs WHERE game_name = ?1",
        params![game_name],
        |row| row.get(0),
    )
    .ok()
}

pub fn set_game_config_exact(game_name: &str, config_json: &str) {
    let conn = DB.lock().unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO game_configs (game_name, config) VALUES (?1, ?2)",
        params![game_name, config_json],
    )
    .unwrap_or_else(|e| {
        tracing::error!("写入 game_configs 失败: game={}, err={}", game_name, e);
        0
    });
}

pub fn list_game_names_v2_raw() -> Vec<String> {
    let conn = DB.lock().unwrap();
    let mut stmt = conn
        .prepare("SELECT game_name FROM game_configs_v2")
        .unwrap();
    let rows = stmt.query_map([], |row| row.get(0)).unwrap();
    rows.filter_map(|r| r.ok()).collect()
}

pub fn get_game_config_v2_exact(game_name: &str) -> Option<String> {
    let conn = DB.lock().unwrap();
    conn.query_row(
        "SELECT config_json FROM game_configs_v2 WHERE game_name = ?1",
        params![game_name],
        |row| row.get(0),
    )
    .ok()
}

pub fn set_game_config_v2_exact(game_name: &str, schema_version: u32, config_json: &str) {
    let conn = DB.lock().unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO game_configs_v2 (game_name, schema_version, config_json, updated_at) VALUES (?1, ?2, ?3, datetime('now'))",
        params![game_name, schema_version, config_json],
    )
    .unwrap_or_else(|e| {
        tracing::error!(
            "写入 game_configs_v2 失败: game={}, version={}, err={}",
            game_name,
            schema_version,
            e
        );
        0
    });
}

#[allow(dead_code)]
pub fn has_game_config_exact(game_name: &str) -> bool {
    let conn = DB.lock().unwrap();
    conn.query_row(
        "SELECT 1 FROM game_configs WHERE game_name = ?1 LIMIT 1",
        params![game_name],
        |_row| Ok(()),
    )
    .is_ok()
        || conn
            .query_row(
                "SELECT 1 FROM game_configs_v2 WHERE game_name = ?1 LIMIT 1",
                params![game_name],
                |_row| Ok(()),
            )
            .is_ok()
}

pub fn get_migration_meta(key: &str) -> Option<String> {
    let conn = DB.lock().unwrap();
    conn.query_row(
        "SELECT value FROM migration_meta WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .ok()
}

pub fn set_migration_meta(key: &str, value: &str) {
    let conn = DB.lock().unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO migration_meta (key, value) VALUES (?1, ?2)",
        params![key, value],
    )
    .unwrap_or_else(|e| {
        tracing::error!("写入 migration_meta 失败: key={}, err={}", key, e);
        0
    });
}

pub fn set_game_key_alias(alias_key: &str, canonical_key: &str) {
    let conn = DB.lock().unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO game_key_aliases (alias_key, canonical_key) VALUES (?1, ?2)",
        params![alias_key, canonical_key],
    )
    .unwrap_or_else(|e| {
        tracing::error!(
            "写入 game_key_aliases 失败: alias={}, canonical={}, err={}",
            alias_key,
            canonical_key,
            e
        );
        0
    });
}

pub fn resolve_game_key_or_alias(input: &str) -> Option<String> {
    let conn = DB.lock().unwrap();
    resolve_canonical_key_with_conn(&conn, input)
}

pub fn list_aliases_for_canonical(canonical: &str) -> Vec<String> {
    let conn = DB.lock().unwrap();
    let canonical = canonical_key_with_conn(&conn, canonical);
    let mut stmt = match conn
        .prepare("SELECT alias_key FROM game_key_aliases WHERE lower(canonical_key) = lower(?1)")
    {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let rows = match stmt.query_map(params![canonical], |row| row.get::<_, String>(0)) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    rows.filter_map(|r| r.ok()).collect()
}

pub fn display_name_en_for_key(input: &str) -> Option<String> {
    let conn = DB.lock().unwrap();
    let canonical = canonical_key_with_conn(&conn, input);
    if canonical.is_empty() {
        return None;
    }
    conn.query_row(
        "SELECT display_name_en FROM game_identities WHERE lower(canonical_key) = lower(?1) LIMIT 1",
        params![canonical],
        |row| row.get::<_, String>(0),
    )
    .ok()
}

pub fn list_identity_records() -> Vec<IdentityRecord> {
    let conn = DB.lock().unwrap();
    let mut stmt = match conn.prepare("SELECT canonical_key, display_name_en FROM game_identities")
    {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let rows = match stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let mut list = Vec::new();
    for (canonical_key, display_name_en) in rows.filter_map(|r| r.ok()) {
        let legacy_aliases = conn
            .prepare(
                "SELECT alias_key FROM game_key_aliases WHERE lower(canonical_key) = lower(?1)",
            )
            .ok()
            .and_then(|mut stmt| {
                stmt.query_map(params![canonical_key.clone()], |row| {
                    row.get::<_, String>(0)
                })
                .ok()
                .map(|rows| rows.filter_map(|r| r.ok()).collect::<Vec<String>>())
            })
            .unwrap_or_default();
        list.push(IdentityRecord {
            canonical_key,
            display_name_en,
            legacy_aliases,
        });
    }
    list
}

pub fn list_game_preset_rows() -> Vec<(String, String)> {
    let conn = DB.lock().unwrap();
    let mut stmt = match conn.prepare("SELECT id, preset_json FROM game_presets") {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let rows = match stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    rows.filter_map(|r| r.ok()).collect()
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

pub fn list_proton_family_rows() -> Vec<ProtonFamilyRecord> {
    let conn = DB.lock().unwrap();
    let mut stmt = match conn.prepare(
        "SELECT family_key, display_name, enabled, sort_order, detect_patterns_json, builtin
         FROM proton_families
         ORDER BY sort_order ASC, family_key ASC",
    ) {
        Ok(v) => v,
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
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    rows.filter_map(|r| r.ok()).collect()
}

pub fn list_proton_source_rows() -> Vec<ProtonSourceRecord> {
    let conn = DB.lock().unwrap();
    let mut stmt = match conn.prepare(
        "SELECT id, family_key, provider, repo, endpoint, url_template, asset_index, asset_pattern, tag_pattern, max_count, include_prerelease, enabled, note
         FROM proton_sources
         ORDER BY family_key ASC, id ASC",
    ) {
        Ok(v) => v,
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
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    rows.filter_map(|r| r.ok()).collect()
}

pub fn replace_proton_catalog_rows(
    families: &[ProtonFamilyRecord],
    sources: &[ProtonSourceRecord],
) -> Result<(), String> {
    let mut conn = DB.lock().unwrap();
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

pub fn rename_game_keys(renames: &[(String, String)]) -> Result<(), String> {
    let mut conn = DB.lock().unwrap();
    let tx = conn
        .transaction()
        .map_err(|e| format!("开始数据库事务失败: {}", e))?;

    for (from, to) in renames {
        tx.execute(
            "UPDATE game_configs SET game_name = ?2 WHERE game_name = ?1",
            params![from, to],
        )
        .map_err(|e| format!("更新 game_configs {} -> {} 失败: {}", from, to, e))?;

        tx.execute(
            "UPDATE game_configs_v2 SET game_name = ?2 WHERE game_name = ?1",
            params![from, to],
        )
        .map_err(|e| format!("更新 game_configs_v2 {} -> {} 失败: {}", from, to, e))?;

        tx.execute(
            "INSERT OR REPLACE INTO game_key_aliases (alias_key, canonical_key) VALUES (?1, ?2)",
            params![from, to],
        )
        .map_err(|e| format!("更新 game_key_aliases {} -> {} 失败: {}", from, to, e))?;
    }

    let current = tx
        .query_row(
            "SELECT value FROM settings WHERE key = 'current_config_name'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| format!("读取 current_config_name 失败: {}", e))?;
    if let Some(current_name) = current {
        let mut updated = current_name.clone();
        for (from, to) in renames {
            if current_name.eq_ignore_ascii_case(from) {
                updated = to.clone();
                break;
            }
        }
        if updated != current_name {
            tx.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('current_config_name', ?1)",
                params![updated],
            )
            .map_err(|e| format!("写入 current_config_name 失败: {}", e))?;
        }
    }

    tx.commit()
        .map_err(|e| format!("提交数据库事务失败: {}", e))
}

// ============================
//  哈希缓存操作 — hash_cache 表
// ============================

/// 查询缓存：如果 size+mtime 匹配，返回缓存的 md5
pub fn get_cached_md5(file_path: &str, file_size: i64, mtime_sec: i64) -> Option<String> {
    let conn = DB.lock().unwrap();
    conn.query_row(
        "SELECT md5 FROM hash_cache WHERE file_path = ?1 AND file_size = ?2 AND mtime_sec = ?3",
        params![file_path, file_size, mtime_sec],
        |row| row.get(0),
    )
    .ok()
}

/// 写入/更新哈希缓存
pub fn set_cached_md5(file_path: &str, file_size: i64, mtime_sec: i64, md5: &str) {
    let conn = DB.lock().unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO hash_cache (file_path, file_size, mtime_sec, md5) VALUES (?1, ?2, ?3, ?4)",
        params![file_path, file_size, mtime_sec, md5],
    )
    .unwrap_or_else(|e| {
        tracing::error!("写入 hash_cache 失败: path={}, err={}", file_path, e);
        0
    });
}
