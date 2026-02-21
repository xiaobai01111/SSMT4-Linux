use crate::configs::app_config::{self, AppConfig};
use crate::configs::database as db;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::Manager;
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

    // 仅同步内存中的 dataDir（不创建符号链接，符号链接仅在 save_settings 时创建）
    if loaded.data_dir.is_empty() {
        app_config::clear_custom_data_dir();
    } else {
        let expanded = app_config::expand_user_path(&loaded.data_dir);
        app_config::set_custom_data_dir(expanded);
    }

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
    settings_to_kv(&settings)?;

    // 同步全局 dataDir
    apply_data_dir(&settings);

    let mut state = config.lock().map_err(|e| e.to_string())?;
    *state = settings;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionCheckInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub update_log: String,
}

const DATA_PARAMETERS_REPO_URL: &str = "https://github.com/xiaobai01111/Data-parameters";
const DATA_PARAMETERS_VERSION_URL: &str =
    "https://raw.githubusercontent.com/xiaobai01111/Data-parameters/main/version";

fn read_trimmed_file(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn read_data_parameters_local_version() -> Option<(String, String)> {
    let path = crate::utils::data_parameters::resolve_data_path("version")?;
    let version = read_trimmed_file(&path)?;
    Some((version, path.to_string_lossy().to_string()))
}

fn read_raw_file(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn resolve_version_file_paths(app: &tauri::AppHandle) -> Vec<PathBuf> {
    let mut bases = Vec::<PathBuf>::new();

    if let Some(root) = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.to_path_buf())
    {
        bases.push(root);
    }

    if let Ok(resource_dir) = app.path().resource_dir() {
        bases.push(resource_dir);
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            bases.push(dir.to_path_buf());
        }
    }

    bases
}

async fn fetch_remote_data_parameters_version() -> Result<String, String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(DATA_PARAMETERS_VERSION_URL)
        .header("User-Agent", "SSMT4/0.1")
        .send()
        .await
        .map_err(|e| format!("请求远程资源版本失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("远程资源版本接口返回 HTTP {}", resp.status()));
    }

    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取远程资源版本内容失败: {}", e))?;
    let version = text.trim().to_string();
    if version.is_empty() {
        return Err("远程资源版本内容为空".to_string());
    }
    Ok(version)
}

#[tauri::command]
pub fn get_version_check_info(app: tauri::AppHandle) -> Result<VersionCheckInfo, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let mut latest_version = String::new();
    let mut update_log = String::new();
    for base in resolve_version_file_paths(&app) {
        if latest_version.is_empty() {
            let candidate = base.join("version");
            if let Some(v) = read_trimmed_file(&candidate) {
                latest_version = v;
            }
        }
        if update_log.is_empty() {
            let candidate = base.join("version-log");
            if let Some(v) = read_raw_file(&candidate) {
                update_log = v;
            }
        }
        if !latest_version.is_empty() && !update_log.is_empty() {
            break;
        }
    }

    if latest_version.is_empty() {
        latest_version = current_version.clone();
    }

    let has_update = latest_version != current_version;

    Ok(VersionCheckInfo {
        current_version,
        latest_version,
        has_update,
        update_log,
    })
}

#[tauri::command]
pub async fn get_resource_version_info() -> Result<VersionCheckInfo, String> {
    let mut notes = vec![format!("资源仓库: {}", DATA_PARAMETERS_REPO_URL)];
    let (current_version, local_path_note) = match read_data_parameters_local_version() {
        Some((version, path)) => (version, Some(path)),
        None => ("unknown".to_string(), None),
    };

    if let Some(path) = local_path_note {
        notes.push(format!("本地版本文件: {}", path));
    } else {
        notes.push("本地版本文件: 未找到 Data-parameters/version".to_string());
    }

    let latest_version = match fetch_remote_data_parameters_version().await {
        Ok(v) => v,
        Err(e) => {
            notes.push(format!("远程检查失败: {}", e));
            current_version.clone()
        }
    };

    let has_update = current_version != "unknown"
        && latest_version != "unknown"
        && latest_version != current_version;

    Ok(VersionCheckInfo {
        current_version,
        latest_version,
        has_update,
        update_log: notes.join("\n"),
    })
}

#[tauri::command]
pub fn pull_resource_updates() -> Result<String, String> {
    crate::utils::data_parameters::sync_managed_repo()?;
    let version = read_data_parameters_local_version()
        .map(|(v, _)| v)
        .unwrap_or_else(|| "unknown".to_string());
    Ok(format!("资源更新完成，本地版本: {}", version))
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
    settings_to_kv(&cfg)?;
    Ok(cfg)
}

/// AppConfig → SQLite KV 写入
fn settings_to_kv(cfg: &AppConfig) -> Result<(), String> {
    let mut entries: Vec<(String, String)> = Vec::new();

    entries.push(("bg_type".to_string(), cfg.bg_type.clone()));
    // 向后兼容旧键名
    entries.push(("background_type".to_string(), cfg.bg_type.clone()));

    entries.push(("bg_image".to_string(), cfg.bg_image.clone()));
    entries.push(("bg_video".to_string(), cfg.bg_video.clone()));
    entries.push((
        "content_opacity".to_string(),
        cfg.content_opacity.to_string(),
    ));
    entries.push(("content_blur".to_string(), cfg.content_blur.to_string()));
    entries.push(("cache_dir".to_string(), cfg.cache_dir.clone()));
    entries.push((
        "current_config_name".to_string(),
        cfg.current_config_name.clone(),
    ));
    entries.push(("github_token".to_string(), cfg.github_token.clone()));
    entries.push((
        "show_websites".to_string(),
        if cfg.show_websites {
            "true".to_string()
        } else {
            "false".to_string()
        },
    ));
    entries.push((
        "show_documents".to_string(),
        if cfg.show_documents {
            "true".to_string()
        } else {
            "false".to_string()
        },
    ));
    entries.push(("locale".to_string(), cfg.locale.clone()));
    // 向后兼容旧键名
    entries.push(("language".to_string(), cfg.locale.clone()));

    entries.push(("window_width".to_string(), cfg.window_width.to_string()));
    entries.push(("window_height".to_string(), cfg.window_height.to_string()));
    entries.push((
        "window_x".to_string(),
        cfg.window_x.map(|v| v.to_string()).unwrap_or_default(),
    ));
    entries.push((
        "window_y".to_string(),
        cfg.window_y.map(|v| v.to_string()).unwrap_or_default(),
    ));
    entries.push(("theme".to_string(), cfg.theme.clone()));
    entries.push((
        "custom_search_paths".to_string(),
        serde_json::to_string(&cfg.custom_search_paths).unwrap_or_default(),
    ));
    entries.push(("data_dir".to_string(), cfg.data_dir.clone()));
    entries.push((
        "initialized".to_string(),
        if cfg.initialized {
            "true".to_string()
        } else {
            "false".to_string()
        },
    ));
    entries.push((
        "tos_risk_acknowledged".to_string(),
        if cfg.tos_risk_acknowledged {
            "true".to_string()
        } else {
            "false".to_string()
        },
    ));
    entries.push((
        "snowbreak_source_policy".to_string(),
        cfg.snowbreak_source_policy.clone(),
    ));

    db::set_settings_batch(&entries)
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
        tos_risk_acknowledged: {
            let v = get_any(&["tos_risk_acknowledged", "tosRiskAcknowledged"]);
            parse_bool_or_default(&v, defaults.tos_risk_acknowledged)
        },
        snowbreak_source_policy: normalize_snowbreak_source_policy(
            &get_any(&["snowbreak_source_policy", "snowbreakSourcePolicy"]),
            &defaults.snowbreak_source_policy,
        ),
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
    cfg.snowbreak_source_policy =
        normalize_snowbreak_source_policy(&cfg.snowbreak_source_policy, "official_first");
    if !cfg.current_config_name.trim().is_empty() && cfg.current_config_name != "Default" {
        cfg.current_config_name =
            crate::configs::game_identity::to_canonical_or_keep(&cfg.current_config_name);
    }

    if cfg.content_opacity.is_nan() {
        cfg.content_opacity = 0.0;
    }
    cfg.content_opacity = cfg.content_opacity.clamp(0.0, 1.0);

    if cfg.content_blur.is_nan() || cfg.content_blur < 0.0 {
        cfg.content_blur = 0.0;
    }

    if !cfg.data_dir.trim().is_empty() {
        cfg.data_dir = app_config::expand_user_path_string(&cfg.data_dir);
    }
}

fn normalize_snowbreak_source_policy(value: &str, default: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "official_first" | "community_first" => normalized,
        "" => default.to_string(),
        _ => default.to_string(),
    }
}

/// 根据 AppConfig.data_dir 设置或清除全局自定义数据目录
/// 通过符号链接将 ~/.local/share/ssmt4 指向自定义目录
fn apply_data_dir(cfg: &AppConfig) {
    if cfg.data_dir.is_empty() {
        app_config::clear_custom_data_dir();
        crate::utils::file_manager::remove_data_dir_symlink();
    } else {
        let dir = app_config::expand_user_path(&cfg.data_dir);
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
