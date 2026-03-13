use super::app_config_store::{load_app_config, save_app_config};
use super::core::open_write_connection;
use rusqlite::{params, Connection};

#[derive(Debug, Clone)]
pub struct IdentityRecord {
    pub canonical_key: String,
    pub display_name_en: String,
    pub legacy_aliases: Vec<String>,
}

pub fn get_game_config(game_name: &str) -> Option<String> {
    let conn = open_write_connection().ok()?;
    for key in lookup_candidates_with_conn(&conn, game_name) {
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
    let Ok(conn) = open_write_connection() else {
        return;
    };
    let canonical = canonical_key_with_conn(&conn, game_name);
    conn.execute(
        "INSERT OR REPLACE INTO game_configs (game_name, config) VALUES (?1, ?2)",
        params![&canonical, config_json],
    )
    .unwrap_or_else(|err| {
        tracing::error!("写入 game_configs 失败: game={}, err={}", canonical, err);
        0
    });
}

pub fn delete_game_config(game_name: &str) {
    let Ok(conn) = open_write_connection() else {
        return;
    };
    for key in lookup_candidates_with_conn(&conn, game_name) {
        conn.execute(
            "DELETE FROM game_configs WHERE game_name = ?1",
            params![key],
        )
        .unwrap_or_else(|err| {
            tracing::error!("删除 game_configs 失败: game={}, err={}", game_name, err);
            0
        });
    }
}

pub fn list_game_names() -> Vec<String> {
    use std::collections::BTreeSet;

    let Ok(conn) = open_write_connection() else {
        return Vec::new();
    };
    let mut stmt = match conn.prepare("SELECT game_name FROM game_configs") {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    let rows = match stmt.query_map([], |row| row.get::<_, String>(0)) {
        Ok(rows) => rows,
        Err(_) => return Vec::new(),
    };

    let mut names = BTreeSet::new();
    for name in rows.filter_map(|row| row.ok()) {
        names.insert(canonical_key_with_conn(&conn, &name));
    }
    names.into_iter().collect()
}

pub fn list_game_names_raw() -> Vec<String> {
    let Ok(conn) = open_write_connection() else {
        return Vec::new();
    };
    let mut stmt = match conn.prepare("SELECT game_name FROM game_configs") {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    let rows = match stmt.query_map([], |row| row.get(0)) {
        Ok(rows) => rows,
        Err(_) => return Vec::new(),
    };
    rows.filter_map(|row| row.ok()).collect()
}

pub fn get_game_config_v2(game_name: &str) -> Option<String> {
    let conn = open_write_connection().ok()?;
    for key in lookup_candidates_with_conn(&conn, game_name) {
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
    let Ok(conn) = open_write_connection() else {
        return;
    };
    let canonical = canonical_key_with_conn(&conn, game_name);
    conn.execute(
        "INSERT OR REPLACE INTO game_configs_v2 (game_name, schema_version, config_json, updated_at) VALUES (?1, ?2, ?3, datetime('now'))",
        params![&canonical, schema_version, config_json],
    )
    .unwrap_or_else(|err| {
        tracing::error!(
            "写入 game_configs_v2 失败: game={}, version={}, err={}",
            canonical,
            schema_version,
            err
        );
        0
    });
}

pub fn delete_game_config_v2(game_name: &str) {
    let Ok(conn) = open_write_connection() else {
        return;
    };
    for key in lookup_candidates_with_conn(&conn, game_name) {
        conn.execute(
            "DELETE FROM game_configs_v2 WHERE game_name = ?1",
            params![key],
        )
        .unwrap_or_else(|err| {
            tracing::error!("删除 game_configs_v2 失败: game={}, err={}", game_name, err);
            0
        });
    }
}

pub fn get_game_config_exact(game_name: &str) -> Option<String> {
    let conn = open_write_connection().ok()?;
    conn.query_row(
        "SELECT config FROM game_configs WHERE game_name = ?1",
        params![game_name],
        |row| row.get(0),
    )
    .ok()
}

pub fn set_game_config_exact(game_name: &str, config_json: &str) {
    let Ok(conn) = open_write_connection() else {
        return;
    };
    conn.execute(
        "INSERT OR REPLACE INTO game_configs (game_name, config) VALUES (?1, ?2)",
        params![game_name, config_json],
    )
    .unwrap_or_else(|err| {
        tracing::error!("写入 game_configs 失败: game={}, err={}", game_name, err);
        0
    });
}

pub fn list_game_names_v2_raw() -> Vec<String> {
    let Ok(conn) = open_write_connection() else {
        return Vec::new();
    };
    let mut stmt = match conn.prepare("SELECT game_name FROM game_configs_v2") {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    let rows = match stmt.query_map([], |row| row.get(0)) {
        Ok(rows) => rows,
        Err(_) => return Vec::new(),
    };
    rows.filter_map(|row| row.ok()).collect()
}

pub fn get_game_config_v2_exact(game_name: &str) -> Option<String> {
    let conn = open_write_connection().ok()?;
    conn.query_row(
        "SELECT config_json FROM game_configs_v2 WHERE game_name = ?1",
        params![game_name],
        |row| row.get(0),
    )
    .ok()
}

pub fn set_game_config_v2_exact(game_name: &str, schema_version: u32, config_json: &str) {
    let Ok(conn) = open_write_connection() else {
        return;
    };
    conn.execute(
        "INSERT OR REPLACE INTO game_configs_v2 (game_name, schema_version, config_json, updated_at) VALUES (?1, ?2, ?3, datetime('now'))",
        params![game_name, schema_version, config_json],
    )
    .unwrap_or_else(|err| {
        tracing::error!(
            "写入 game_configs_v2 失败: game={}, version={}, err={}",
            game_name,
            schema_version,
            err
        );
        0
    });
}

#[allow(dead_code)]
pub fn has_game_config_exact(game_name: &str) -> bool {
    let Ok(conn) = open_write_connection() else {
        return false;
    };
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

pub fn set_game_key_alias(alias_key: &str, canonical_key: &str) {
    let Ok(conn) = open_write_connection() else {
        return;
    };
    conn.execute(
        "INSERT OR REPLACE INTO game_key_aliases (alias_key, canonical_key) VALUES (?1, ?2)",
        params![alias_key, canonical_key],
    )
    .unwrap_or_else(|err| {
        tracing::error!(
            "写入 game_key_aliases 失败: alias={}, canonical={}, err={}",
            alias_key,
            canonical_key,
            err
        );
        0
    });
}

pub fn resolve_game_key_or_alias(input: &str) -> Option<String> {
    let conn = open_write_connection().ok()?;
    resolve_canonical_key_with_conn(&conn, input)
}

pub fn list_aliases_for_canonical(canonical: &str) -> Vec<String> {
    let Ok(conn) = open_write_connection() else {
        return Vec::new();
    };
    let canonical = canonical_key_with_conn(&conn, canonical);
    let mut stmt = match conn
        .prepare("SELECT alias_key FROM game_key_aliases WHERE lower(canonical_key) = lower(?1)")
    {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    let rows = match stmt.query_map(params![canonical], |row| row.get::<_, String>(0)) {
        Ok(rows) => rows,
        Err(_) => return Vec::new(),
    };
    rows.filter_map(|row| row.ok()).collect()
}

pub fn display_name_en_for_key(input: &str) -> Option<String> {
    let conn = open_write_connection().ok()?;
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
    let Ok(conn) = open_write_connection() else {
        return Vec::new();
    };
    let mut stmt = match conn.prepare("SELECT canonical_key, display_name_en FROM game_identities")
    {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    let rows = match stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) {
        Ok(rows) => rows,
        Err(_) => return Vec::new(),
    };

    let mut list = Vec::new();
    for (canonical_key, display_name_en) in rows.filter_map(|row| row.ok()) {
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
                .map(|rows| rows.filter_map(|row| row.ok()).collect::<Vec<String>>())
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
    let Ok(conn) = open_write_connection() else {
        return Vec::new();
    };
    let mut stmt = match conn.prepare("SELECT id, preset_json FROM game_presets") {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    let rows = match stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) {
        Ok(rows) => rows,
        Err(_) => return Vec::new(),
    };
    rows.filter_map(|row| row.ok()).collect()
}

pub fn rename_game_keys(renames: &[(String, String)]) -> Result<(), String> {
    let mut conn = open_write_connection()?;
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

    tx.commit()
        .map_err(|e| format!("提交数据库事务失败: {}", e))?;

    if let Ok(Some(mut cfg)) = load_app_config() {
        let current_name = cfg.current_config_name.clone();
        for (from, to) in renames {
            if current_name.eq_ignore_ascii_case(from) {
                cfg.current_config_name = to.clone();
                save_app_config(&cfg)?;
                break;
            }
        }
    }

    Ok(())
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

fn lookup_candidates_with_conn(conn: &Connection, input: &str) -> Vec<String> {
    let raw = input.trim().to_string();
    if raw.is_empty() {
        return Vec::new();
    }

    let canonical = canonical_key_with_conn(conn, &raw);
    let mut result = Vec::new();
    result.push(canonical.clone());
    if !raw.eq_ignore_ascii_case(&canonical) {
        result.push(raw);
    }

    if let Ok(mut stmt) = conn
        .prepare("SELECT alias_key FROM game_key_aliases WHERE lower(canonical_key) = lower(?1)")
    {
        if let Ok(rows) = stmt.query_map(params![canonical], |row| row.get::<_, String>(0)) {
            for alias in rows.filter_map(|row| row.ok()) {
                if !result.iter().any(|item| item.eq_ignore_ascii_case(&alias)) {
                    result.push(alias);
                }
            }
        }
    }

    result
}
