use super::core::{open_read_only_connection, open_write_connection};
use rusqlite::params;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingRecord {
    pub key: String,
    pub value: String,
}

pub fn read_setting_record(key: &str) -> Option<SettingRecord> {
    let conn = open_read_only_connection()?;
    conn.query_row(
        "SELECT key, value FROM settings WHERE key = ?1",
        params![key],
        |row| {
            Ok(SettingRecord {
                key: row.get(0)?,
                value: row.get(1)?,
            })
        },
    )
    .ok()
}

pub fn read_setting_value(key: &str) -> Option<String> {
    read_setting_record(key).map(|record| record.value)
}

pub fn write_setting_value(key: &str, value: &str) {
    if let Err(err) = write_settings_batch(&[(key.to_string(), value.to_string())]) {
        tracing::error!("写入 settings 失败: key={}, err={}", key, err);
    }
}

pub fn write_settings_batch(entries: &[(String, String)]) -> Result<(), String> {
    if entries.is_empty() {
        return Ok(());
    }

    let mut conn = open_write_connection()?;
    let tx = conn
        .transaction()
        .map_err(|e| format!("开始 settings 事务失败: {}", e))?;

    {
        let mut stmt = tx
            .prepare("INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)")
            .map_err(|e| format!("准备 settings 写入语句失败: {}", e))?;

        for (key, value) in entries {
            stmt.execute(params![key, value])
                .map_err(|e| format!("写入 settings 失败: key={}, err={}", key, e))?;
        }
    }

    tx.commit()
        .map_err(|e| format!("提交 settings 事务失败: {}", e))
}

pub fn list_setting_records() -> Vec<SettingRecord> {
    let Some(conn) = open_read_only_connection() else {
        return Vec::new();
    };
    let mut stmt = match conn.prepare("SELECT key, value FROM settings") {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    let rows = match stmt.query_map([], |row| {
        Ok(SettingRecord {
            key: row.get(0)?,
            value: row.get(1)?,
        })
    }) {
        Ok(rows) => rows,
        Err(_) => return Vec::new(),
    };
    rows.filter_map(|row| row.ok()).collect()
}

pub fn read_migration_meta(key: &str) -> Option<String> {
    let conn = open_read_only_connection()?;
    conn.query_row(
        "SELECT value FROM migration_meta WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .ok()
}

pub fn write_migration_meta(key: &str, value: &str) {
    let Ok(conn) = open_write_connection() else {
        return;
    };
    conn.execute(
        "INSERT OR REPLACE INTO migration_meta (key, value) VALUES (?1, ?2)",
        params![key, value],
    )
    .unwrap_or_else(|e| {
        tracing::error!("写入 migration_meta 失败: key={}, err={}", key, e);
        0
    });
}
