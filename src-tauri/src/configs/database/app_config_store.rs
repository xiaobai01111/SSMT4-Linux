use super::core::{open_read_only_connection, open_write_connection};
use crate::configs::app_config::{deserialize_app_config, serialize_app_config, AppConfig};
use rusqlite::{params, OptionalExtension};

pub const APP_CONFIG_STORAGE_KEY: &str = "app_config_json";

const CURRENT_CONFIG_NAME_KEY: &str = "current_config_name";
const DATA_DIR_KEY: &str = "data_dir";
const MIGOTO_ENABLED_KEY: &str = "migoto_enabled";
const TOS_RISK_ACKNOWLEDGED_KEY: &str = "tos_risk_acknowledged";

fn legacy_shadow_setting_keys() -> &'static [&'static str] {
    &[
        CURRENT_CONFIG_NAME_KEY,
        DATA_DIR_KEY,
        MIGOTO_ENABLED_KEY,
        TOS_RISK_ACKNOWLEDGED_KEY,
    ]
}

pub fn load_app_config() -> Result<Option<AppConfig>, String> {
    let Some(conn) = open_read_only_connection() else {
        return Ok(None);
    };

    let raw = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![APP_CONFIG_STORAGE_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| format!("读取 AppConfig 持久化记录失败: {}", e))?;

    raw.map(|raw| deserialize_app_config(&raw)).transpose()
}

pub fn save_app_config(cfg: &AppConfig) -> Result<(), String> {
    let json = serialize_app_config(cfg)?;
    let mut conn = open_write_connection()?;
    let tx = conn
        .transaction()
        .map_err(|e| format!("开始 AppConfig 持久化事务失败: {}", e))?;

    tx.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
        params![APP_CONFIG_STORAGE_KEY, json],
    )
    .map_err(|e| format!("写入 AppConfig JSON 失败: {}", e))?;

    {
        let mut stmt = tx
            .prepare("DELETE FROM settings WHERE key = ?1")
            .map_err(|e| format!("准备清理 AppConfig 影子字段失败: {}", e))?;
        for key in legacy_shadow_setting_keys() {
            stmt.execute(params![key])
                .map_err(|e| format!("清理 AppConfig 影子字段失败: key={}, err={}", key, e))?;
        }
    }

    tx.commit()
        .map_err(|e| format!("提交 AppConfig 持久化事务失败: {}", e))
}

#[cfg(test)]
mod tests {
    use super::{legacy_shadow_setting_keys, DATA_DIR_KEY};

    #[test]
    fn legacy_shadow_keys_include_bootstrap_data_dir() {
        assert!(legacy_shadow_setting_keys().contains(&DATA_DIR_KEY));
        assert_eq!(legacy_shadow_setting_keys().len(), 4);
    }
}
