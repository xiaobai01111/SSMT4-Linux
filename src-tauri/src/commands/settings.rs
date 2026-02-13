use crate::configs::app_config::{self, AppConfig};
use crate::configs::database as db;
use std::sync::Mutex;
use tauri::State;

#[tauri::command]
pub fn load_settings(config: State<'_, Mutex<AppConfig>>) -> Result<AppConfig, String> {
    // 优先从 SQLite 读取
    let pairs = db::get_all_settings();
    let loaded = if pairs.is_empty() {
        // 尝试从旧 settings.json 迁移
        migrate_json_to_db()?
    } else {
        settings_from_kv(&pairs)
    };

    // 同步全局 dataDir
    apply_data_dir(&loaded);

    let mut state = config.lock().map_err(|e| e.to_string())?;
    *state = loaded.clone();
    Ok(loaded)
}

#[tauri::command]
pub fn save_settings(
    config: State<'_, Mutex<AppConfig>>,
    settings: AppConfig,
) -> Result<(), String> {
    // 写入 SQLite
    settings_to_kv(&settings);

    // 同步全局 dataDir
    apply_data_dir(&settings);

    let mut state = config.lock().map_err(|e| e.to_string())?;
    *state = settings;
    Ok(())
}

/// 从旧 settings.json 迁移到 SQLite（首次升级时执行一次）
fn migrate_json_to_db() -> Result<AppConfig, String> {
    let config_dir = app_config::get_app_config_dir();
    let json_path = config_dir.join("settings.json");

    let cfg = if json_path.exists() {
        let content = std::fs::read_to_string(&json_path)
            .map_err(|e| format!("读取 settings.json 失败: {}", e))?;
        let loaded: AppConfig = serde_json::from_str(&content)
            .map_err(|e| format!("解析 settings.json 失败: {}", e))?;
        tracing::info!("从 settings.json 迁移到 SQLite");
        loaded
    } else {
        AppConfig::default()
    };

    settings_to_kv(&cfg);
    Ok(cfg)
}

/// AppConfig → SQLite KV 写入
fn settings_to_kv(cfg: &AppConfig) {
    db::set_setting("background_type", &cfg.background_type);
    db::set_setting("cache_dir", &cfg.cache_dir);
    db::set_setting("window_width", &cfg.window_width.to_string());
    db::set_setting("window_height", &cfg.window_height.to_string());
    db::set_setting("window_x", &cfg.window_x.map(|v| v.to_string()).unwrap_or_default());
    db::set_setting("window_y", &cfg.window_y.map(|v| v.to_string()).unwrap_or_default());
    db::set_setting("language", &cfg.language);
    db::set_setting("theme", &cfg.theme);
    db::set_setting("custom_search_paths", &serde_json::to_string(&cfg.custom_search_paths).unwrap_or_default());
    db::set_setting("data_dir", &cfg.data_dir);
    db::set_setting("initialized", if cfg.initialized { "true" } else { "false" });
}

/// SQLite KV → AppConfig 读取
fn settings_from_kv(pairs: &[(String, String)]) -> AppConfig {
    let get = |key: &str| -> String {
        pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone()).unwrap_or_default()
    };

    let defaults = AppConfig::default();
    AppConfig {
        background_type: { let v = get("background_type"); if v.is_empty() { defaults.background_type } else { v } },
        cache_dir: { let v = get("cache_dir"); if v.is_empty() { defaults.cache_dir } else { v } },
        window_width: get("window_width").parse().unwrap_or(defaults.window_width),
        window_height: get("window_height").parse().unwrap_or(defaults.window_height),
        window_x: get("window_x").parse().ok(),
        window_y: get("window_y").parse().ok(),
        language: { let v = get("language"); if v.is_empty() { defaults.language } else { v } },
        theme: { let v = get("theme"); if v.is_empty() { defaults.theme } else { v } },
        custom_search_paths: serde_json::from_str(&get("custom_search_paths")).unwrap_or_default(),
        data_dir: get("data_dir"),
        initialized: get("initialized") == "true",
    }
}

/// 根据 AppConfig.data_dir 设置或清除全局自定义数据目录
/// 通过符号链接将 ~/.local/share/ssmt4 指向自定义目录
fn apply_data_dir(cfg: &AppConfig) {
    if cfg.data_dir.is_empty() {
        app_config::clear_custom_data_dir();
        crate::utils::file_manager::remove_data_dir_symlink();
    } else {
        let dir = std::path::PathBuf::from(&cfg.data_dir);
        app_config::set_custom_data_dir(dir.clone());

        // 创建符号链接：~/.local/share/ssmt4 -> 自定义目录
        if let Err(e) = crate::utils::file_manager::setup_data_dir_symlink(&dir) {
            tracing::error!("设置数据目录符号链接失败: {}", e);
        }

        // 创建 Games 子目录
        let games_dir = crate::utils::file_manager::get_global_games_dir();
        crate::utils::file_manager::ensure_dir(&games_dir).ok();
    }
    tracing::info!("数据目录: {}", app_config::get_app_data_dir().display());
}
