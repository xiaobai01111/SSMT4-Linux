use crate::configs::app_config::{self, AppConfig};
use crate::configs::database as db;
use std::sync::Mutex;
use tauri::State;

#[tauri::command]
pub fn load_settings(config: State<'_, Mutex<AppConfig>>) -> Result<AppConfig, String> {
    // 优先从 SQLite 读取
    let pairs = db::get_all_settings();
    let mut loaded = if pairs.is_empty() {
        // 尝试从旧 settings.json 迁移
        migrate_json_to_db()?
    } else {
        settings_from_kv(&pairs)
    };
    normalize_settings(&mut loaded);

    // 同步全局 dataDir
    apply_data_dir(&loaded);

    let mut state = config.lock().map_err(|e| e.to_string())?;
    *state = loaded.clone();
    Ok(loaded)
}

#[tauri::command]
pub fn save_settings(
    config: State<'_, Mutex<AppConfig>>,
    mut settings: AppConfig,
) -> Result<(), String> {
    normalize_settings(&mut settings);

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

    let mut cfg = if json_path.exists() {
        let content = std::fs::read_to_string(&json_path)
            .map_err(|e| format!("读取 settings.json 失败: {}", e))?;
        let loaded: AppConfig = serde_json::from_str(&content)
            .map_err(|e| format!("解析 settings.json 失败: {}", e))?;
        tracing::info!("从 settings.json 迁移到 SQLite");
        loaded
    } else {
        AppConfig::default()
    };

    normalize_settings(&mut cfg);
    settings_to_kv(&cfg);
    Ok(cfg)
}

/// AppConfig → SQLite KV 写入
fn settings_to_kv(cfg: &AppConfig) {
    db::set_setting("bg_type", &cfg.bg_type);
    // 向后兼容旧键名
    db::set_setting("background_type", &cfg.bg_type);

    db::set_setting("bg_image", &cfg.bg_image);
    db::set_setting("bg_video", &cfg.bg_video);
    db::set_setting("content_opacity", &cfg.content_opacity.to_string());
    db::set_setting("content_blur", &cfg.content_blur.to_string());
    db::set_setting("cache_dir", &cfg.cache_dir);
    db::set_setting("current_config_name", &cfg.current_config_name);
    db::set_setting("github_token", &cfg.github_token);
    db::set_setting("show_mods", if cfg.show_mods { "true" } else { "false" });
    db::set_setting(
        "show_websites",
        if cfg.show_websites { "true" } else { "false" },
    );
    db::set_setting(
        "show_documents",
        if cfg.show_documents { "true" } else { "false" },
    );
    db::set_setting("locale", &cfg.locale);
    // 向后兼容旧键名
    db::set_setting("language", &cfg.locale);

    db::set_setting("window_width", &cfg.window_width.to_string());
    db::set_setting("window_height", &cfg.window_height.to_string());
    db::set_setting(
        "window_x",
        &cfg.window_x.map(|v| v.to_string()).unwrap_or_default(),
    );
    db::set_setting(
        "window_y",
        &cfg.window_y.map(|v| v.to_string()).unwrap_or_default(),
    );
    db::set_setting("theme", &cfg.theme);
    db::set_setting(
        "custom_search_paths",
        &serde_json::to_string(&cfg.custom_search_paths).unwrap_or_default(),
    );
    db::set_setting("data_dir", &cfg.data_dir);
    db::set_setting(
        "initialized",
        if cfg.initialized { "true" } else { "false" },
    );
}

/// SQLite KV → AppConfig 读取
fn settings_from_kv(pairs: &[(String, String)]) -> AppConfig {
    let get = |key: &str| -> String {
        pairs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    let get_any = |keys: &[&str]| -> String {
        keys.iter()
            .find_map(|key| pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone()))
            .unwrap_or_default()
    };

    let defaults = AppConfig::default();

    let bg_type_raw = get_any(&["bg_type", "bgType", "background_type", "backgroundType"]);
    let locale_raw = get_any(&["locale", "language"]);

    AppConfig {
        bg_type: normalize_bg_type(if bg_type_raw.is_empty() {
            &defaults.bg_type
        } else {
            &bg_type_raw
        }),
        bg_image: {
            let v = get_any(&["bg_image", "bgImage"]);
            if v.is_empty() {
                defaults.bg_image
            } else {
                v
            }
        },
        bg_video: {
            let v = get_any(&["bg_video", "bgVideo"]);
            if v.is_empty() {
                defaults.bg_video
            } else {
                v
            }
        },
        content_opacity: {
            let v = get_any(&["content_opacity", "contentOpacity"]);
            parse_f64_or_default(&v, defaults.content_opacity)
        },
        content_blur: {
            let v = get_any(&["content_blur", "contentBlur"]);
            parse_f64_or_default(&v, defaults.content_blur)
        },
        cache_dir: {
            let v = get_any(&["cache_dir", "cacheDir"]);
            if v.is_empty() {
                defaults.cache_dir
            } else {
                v
            }
        },
        current_config_name: {
            let v = get_any(&["current_config_name", "currentConfigName"]);
            if v.is_empty() {
                defaults.current_config_name
            } else {
                v
            }
        },
        github_token: {
            let v = get_any(&["github_token", "githubToken"]);
            if v.is_empty() {
                defaults.github_token
            } else {
                v
            }
        },
        show_mods: {
            let v = get_any(&["show_mods", "showMods"]);
            parse_bool_or_default(&v, defaults.show_mods)
        },
        show_websites: {
            let v = get_any(&["show_websites", "showWebsites"]);
            parse_bool_or_default(&v, defaults.show_websites)
        },
        show_documents: {
            let v = get_any(&["show_documents", "showDocuments"]);
            parse_bool_or_default(&v, defaults.show_documents)
        },
        locale: normalize_locale(if locale_raw.is_empty() {
            &defaults.locale
        } else {
            &locale_raw
        }),
        window_width: parse_f64_or_default(
            &get_any(&["window_width", "windowWidth"]),
            defaults.window_width,
        ),
        window_height: parse_f64_or_default(
            &get_any(&["window_height", "windowHeight"]),
            defaults.window_height,
        ),
        window_x: get_any(&["window_x", "windowX"]).parse().ok(),
        window_y: get_any(&["window_y", "windowY"]).parse().ok(),
        theme: {
            let v = get("theme");
            if v.is_empty() {
                defaults.theme
            } else {
                v
            }
        },
        custom_search_paths: {
            let v = get_any(&["custom_search_paths", "customSearchPaths"]);
            if v.is_empty() {
                defaults.custom_search_paths
            } else {
                serde_json::from_str(&v).unwrap_or_default()
            }
        },
        data_dir: {
            let v = get_any(&["data_dir", "dataDir"]);
            if v.is_empty() {
                defaults.data_dir
            } else {
                v
            }
        },
        initialized: parse_bool_or_default(&get("initialized"), defaults.initialized),
    }
}

fn parse_bool_or_default(value: &str, default: bool) -> bool {
    match value.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "1" | "true" | "yes" | "on" => true,
        "0" | "false" | "no" | "off" => false,
        _ => default,
    }
}

fn parse_f64_or_default(value: &str, default: f64) -> f64 {
    value.parse().unwrap_or(default)
}

fn normalize_bg_type(value: &str) -> String {
    if value.trim().eq_ignore_ascii_case("video") {
        "Video".to_string()
    } else {
        "Image".to_string()
    }
}

fn normalize_locale(value: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase().replace('_', "-");

    if normalized == "zht"
        || normalized.starts_with("zh-tw")
        || normalized.starts_with("zh-hk")
        || normalized.starts_with("zh-hant")
    {
        "zht".to_string()
    } else if normalized == "zhs" || normalized.starts_with("zh") {
        "zhs".to_string()
    } else {
        "en".to_string()
    }
}

fn normalize_settings(cfg: &mut AppConfig) {
    cfg.bg_type = normalize_bg_type(&cfg.bg_type);
    cfg.locale = normalize_locale(&cfg.locale);

    if cfg.content_opacity.is_nan() {
        cfg.content_opacity = 0.0;
    }
    cfg.content_opacity = cfg.content_opacity.clamp(0.0, 1.0);

    if cfg.content_blur.is_nan() || cfg.content_blur < 0.0 {
        cfg.content_blur = 0.0;
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
