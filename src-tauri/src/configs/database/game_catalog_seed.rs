use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeedCatalog {
    #[serde(default = "default_game_catalog_seed_schema")]
    schema_version: u32,
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

pub(super) const GAME_CATALOG_SEED_SCHEMA_META_KEY: &str = "game_catalog_seed_schema_version";

fn default_game_catalog_seed_schema() -> u32 {
    1
}

pub(super) fn ensure_game_catalog_seed(conn: &Connection) {
    let seed_json = match crate::utils::data_parameters::read_catalog_json("game_catalog.seed.json")
    {
        Ok(content) => content,
        Err(err) => {
            tracing::error!("读取游戏目录种子失败: {}", err);
            return;
        }
    };

    let seed = match serde_json::from_str::<SeedCatalog>(&seed_json) {
        Ok(seed) => seed,
        Err(err) => {
            tracing::error!("解析游戏目录种子失败: {}", err);
            return;
        }
    };

    if !should_apply_game_catalog_seed(conn, seed.schema_version) {
        tracing::info!(
            "游戏目录种子已是最新 schema={}, 跳过导入",
            seed.schema_version
        );
        return;
    }

    apply_game_catalog_seed(conn, seed);
}

fn apply_game_catalog_seed(conn: &Connection, seed: SeedCatalog) {
    persist_seed_schema_version(conn, seed.schema_version);
    seed_identities(conn, seed.identities);
    seed_presets(conn, seed.presets);
    remove_deprecated_presets(conn);
}

fn should_apply_game_catalog_seed(conn: &Connection, seed_schema_version: u32) -> bool {
    if !has_catalog_seed_data(conn) {
        return true;
    }

    match read_persisted_seed_schema_version(conn) {
        None => true,
        Some(stored_version) if stored_version < seed_schema_version => true,
        Some(stored_version) if stored_version > seed_schema_version => {
            tracing::warn!(
                "检测到更高版本的游戏目录 seed schema 已导入: stored={}, current={}, 跳过旧版本导入",
                stored_version,
                seed_schema_version
            );
            false
        }
        Some(_) => false,
    }
}

fn read_persisted_seed_schema_version(conn: &Connection) -> Option<u32> {
    conn.query_row(
        "SELECT value FROM migration_meta WHERE key = ?1 LIMIT 1",
        params![GAME_CATALOG_SEED_SCHEMA_META_KEY],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .ok()
    .flatten()
    .and_then(|value| value.trim().parse::<u32>().ok())
}

fn has_catalog_seed_data(conn: &Connection) -> bool {
    has_rows(conn, "game_identities") && has_rows(conn, "game_presets")
}

fn has_rows(conn: &Connection, table_name: &str) -> bool {
    let sql = format!("SELECT EXISTS(SELECT 1 FROM {} LIMIT 1)", table_name);
    conn.query_row(&sql, [], |row| row.get::<_, i64>(0))
        .map(|value| value != 0)
        .unwrap_or(false)
}

fn persist_seed_schema_version(conn: &Connection, schema_version: u32) {
    if let Err(err) = conn.execute(
        "INSERT OR REPLACE INTO migration_meta (key, value) VALUES (?1, ?2)",
        params![
            GAME_CATALOG_SEED_SCHEMA_META_KEY,
            schema_version.to_string()
        ],
    ) {
        tracing::warn!(
            "记录 game_catalog seed schema 失败: version={}, err={}",
            schema_version,
            err
        );
    }
}

fn seed_identities(conn: &Connection, identities: Vec<SeedIdentity>) {
    for identity in identities {
        if identity.canonical_key.trim().is_empty() {
            continue;
        }

        let canonical_key = identity.canonical_key.trim().to_string();
        if let Err(err) = conn.execute(
            "INSERT OR IGNORE INTO game_identities (canonical_key, display_name_en) VALUES (?1, ?2)",
            params![canonical_key, identity.display_name_en.trim()],
        ) {
            tracing::warn!(
                "写入 game_identities 失败: key={}, err={}",
                identity.canonical_key,
                err
            );
        }

        if let Err(err) = conn.execute(
            "DELETE FROM game_key_aliases WHERE lower(canonical_key) = lower(?1)",
            params![identity.canonical_key.trim()],
        ) {
            tracing::warn!(
                "清理 game_key_aliases 失败: canonical={}, err={}",
                identity.canonical_key,
                err
            );
        }

        for alias in identity.legacy_aliases {
            let alias = alias.trim();
            if alias.is_empty() {
                continue;
            }
            if let Err(err) = conn.execute(
                "INSERT OR IGNORE INTO game_key_aliases (alias_key, canonical_key) VALUES (?1, ?2)",
                params![alias, identity.canonical_key.trim()],
            ) {
                tracing::warn!(
                    "写入 game_key_aliases 失败: alias={}, canonical={}, err={}",
                    alias,
                    identity.canonical_key,
                    err
                );
            }
        }
    }
}

fn seed_presets(conn: &Connection, presets: Vec<serde_json::Value>) {
    for preset in presets {
        let Some(id) = preset
            .get("id")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
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
                Ok(value) => value,
                Err(_) => serde_json::json!({}),
            };
            let changed = merge_missing_json_fields(&mut existing_value, &preset);
            if changed {
                if let Ok(merged_json) = serde_json::to_string_pretty(&existing_value) {
                    if let Err(err) = conn.execute(
                        "UPDATE game_presets SET preset_json = ?2, updated_at = datetime('now') WHERE id = ?1",
                        params![id, merged_json],
                    ) {
                        tracing::warn!("更新 game_presets 失败: id={}, err={}", id, err);
                    }
                }
            }
            continue;
        }

        if let Ok(json) = serde_json::to_string_pretty(&preset) {
            if let Err(err) = conn.execute(
                "INSERT OR IGNORE INTO game_presets (id, preset_json, updated_at) VALUES (?1, ?2, datetime('now'))",
                params![id, json],
            ) {
                tracing::warn!("写入 game_presets 失败: id={}, err={}", id, err);
            }
        }
    }
}

fn remove_deprecated_presets(conn: &Connection) {
    let _ = conn.execute(
        "DELETE FROM game_presets WHERE lower(id) = lower('HonkaiImpact3rd')",
        [],
    );
    let _ = conn.execute(
        "DELETE FROM game_identities WHERE lower(canonical_key) = lower('HonkaiImpact3rd')",
        [],
    );
    let _ = conn.execute(
        "DELETE FROM game_key_aliases WHERE lower(canonical_key) = lower('HonkaiImpact3rd')",
        [],
    );
    let _ = conn.execute(
        "DELETE FROM game_configs WHERE lower(game_name) = lower('HonkaiImpact3rd')",
        [],
    );
    let _ = conn.execute(
        "DELETE FROM game_configs_v2 WHERE lower(game_name) = lower('HonkaiImpact3rd')",
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

#[cfg(test)]
mod tests {
    use super::{
        default_game_catalog_seed_schema, has_catalog_seed_data, merge_missing_json_fields,
        read_persisted_seed_schema_version, should_apply_game_catalog_seed, SeedCatalog,
        GAME_CATALOG_SEED_SCHEMA_META_KEY,
    };
    use rusqlite::Connection;
    use serde_json::json;

    fn prepare_seed_tables(conn: &Connection) {
        conn.execute(
            "CREATE TABLE migration_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
            [],
        )
        .expect("migration_meta table");
        conn.execute(
            "CREATE TABLE game_identities (canonical_key TEXT PRIMARY KEY, display_name_en TEXT NOT NULL)",
            [],
        )
        .expect("game_identities table");
        conn.execute(
            "CREATE TABLE game_presets (id TEXT PRIMARY KEY, preset_json TEXT NOT NULL, updated_at TEXT NOT NULL DEFAULT '')",
            [],
        )
        .expect("game_presets table");
    }

    #[test]
    fn merge_missing_json_fields_backfills_without_overwriting_existing_values() {
        let mut existing = json!({
            "id": "WutheringWaves",
            "telemetryServers": ["manual.example.com"],
            "channelProtection": {
                "protectedValue": 999
            }
        });
        let seed = json!({
            "id": "WutheringWaves",
            "telemetryServers": ["seed.example.com"],
            "channelProtection": {
                "configRelativePath": "Client/Config.json",
                "protectedValue": 205
            },
            "downloadMode": "fullGame"
        });

        let changed = merge_missing_json_fields(&mut existing, &seed);

        assert!(changed);
        assert_eq!(
            existing.get("telemetryServers"),
            Some(&json!(["manual.example.com"]))
        );
        assert_eq!(
            existing.pointer("/channelProtection/protectedValue"),
            Some(&json!(999))
        );
        assert_eq!(
            existing.pointer("/channelProtection/configRelativePath"),
            Some(&json!("Client/Config.json"))
        );
        assert_eq!(existing.get("downloadMode"), Some(&json!("fullGame")));
    }

    #[test]
    fn seed_catalog_defaults_schema_version_when_missing() {
        let parsed: SeedCatalog = serde_json::from_value(json!({
            "identities": [],
            "presets": []
        }))
        .expect("seed catalog");

        assert_eq!(parsed.schema_version, default_game_catalog_seed_schema());
    }

    #[test]
    fn game_catalog_schema_meta_key_can_be_persisted() {
        let conn = Connection::open_in_memory().expect("sqlite");
        conn.execute(
            "CREATE TABLE migration_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
            [],
        )
        .expect("table");

        conn.execute(
            "INSERT OR REPLACE INTO migration_meta (key, value) VALUES (?1, ?2)",
            [GAME_CATALOG_SEED_SCHEMA_META_KEY, "1"],
        )
        .expect("write");

        let stored: String = conn
            .query_row(
                "SELECT value FROM migration_meta WHERE key = ?1",
                [GAME_CATALOG_SEED_SCHEMA_META_KEY],
                |row| row.get(0),
            )
            .expect("read");

        assert_eq!(stored, "1");
    }

    #[test]
    fn seed_import_runs_when_catalog_tables_are_empty() {
        let conn = Connection::open_in_memory().expect("sqlite");
        prepare_seed_tables(&conn);

        assert!(!has_catalog_seed_data(&conn));
        assert!(should_apply_game_catalog_seed(&conn, 1));
    }

    #[test]
    fn seed_import_skips_when_schema_current_and_catalog_present() {
        let conn = Connection::open_in_memory().expect("sqlite");
        prepare_seed_tables(&conn);
        conn.execute(
            "INSERT INTO game_identities (canonical_key, display_name_en) VALUES (?1, ?2)",
            ["WutheringWaves", "Wuthering Waves"],
        )
        .expect("insert identity");
        conn.execute(
            "INSERT INTO game_presets (id, preset_json, updated_at) VALUES (?1, ?2, datetime('now'))",
            ["WutheringWaves", "{\"id\":\"WutheringWaves\"}"],
        )
        .expect("insert preset");
        conn.execute(
            "INSERT INTO migration_meta (key, value) VALUES (?1, ?2)",
            [GAME_CATALOG_SEED_SCHEMA_META_KEY, "1"],
        )
        .expect("insert meta");

        assert!(has_catalog_seed_data(&conn));
        assert_eq!(read_persisted_seed_schema_version(&conn), Some(1));
        assert!(!should_apply_game_catalog_seed(&conn, 1));
    }

    #[test]
    fn seed_import_runs_when_schema_upgrades() {
        let conn = Connection::open_in_memory().expect("sqlite");
        prepare_seed_tables(&conn);
        conn.execute(
            "INSERT INTO game_identities (canonical_key, display_name_en) VALUES (?1, ?2)",
            ["WutheringWaves", "Wuthering Waves"],
        )
        .expect("insert identity");
        conn.execute(
            "INSERT INTO game_presets (id, preset_json, updated_at) VALUES (?1, ?2, datetime('now'))",
            ["WutheringWaves", "{\"id\":\"WutheringWaves\"}"],
        )
        .expect("insert preset");
        conn.execute(
            "INSERT INTO migration_meta (key, value) VALUES (?1, ?2)",
            [GAME_CATALOG_SEED_SCHEMA_META_KEY, "1"],
        )
        .expect("insert meta");

        assert!(should_apply_game_catalog_seed(&conn, 2));
    }

    #[test]
    fn older_seed_schema_does_not_override_newer_import_marker() {
        let conn = Connection::open_in_memory().expect("sqlite");
        prepare_seed_tables(&conn);
        conn.execute(
            "INSERT INTO game_identities (canonical_key, display_name_en) VALUES (?1, ?2)",
            ["WutheringWaves", "Wuthering Waves"],
        )
        .expect("insert identity");
        conn.execute(
            "INSERT INTO game_presets (id, preset_json, updated_at) VALUES (?1, ?2, datetime('now'))",
            ["WutheringWaves", "{\"id\":\"WutheringWaves\"}"],
        )
        .expect("insert preset");
        conn.execute(
            "INSERT INTO migration_meta (key, value) VALUES (?1, ?2)",
            [GAME_CATALOG_SEED_SCHEMA_META_KEY, "3"],
        )
        .expect("insert meta");

        assert!(!should_apply_game_catalog_seed(&conn, 2));
    }
}
