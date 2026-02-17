use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::RwLock;

/// 用户自定义的数据根目录（全局，启动时从 settings.json 读取后设置）
static CUSTOM_DATA_DIR: once_cell::sync::Lazy<RwLock<Option<PathBuf>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(None));

/// 启动时设置自定义数据根目录
pub fn set_custom_data_dir(dir: PathBuf) {
    tracing::info!("Custom data dir set to: {}", dir.display());
    *CUSTOM_DATA_DIR.write().unwrap() = Some(dir);
}

/// 清除自定义数据根目录（恢复默认）
pub fn clear_custom_data_dir() {
    *CUSTOM_DATA_DIR.write().unwrap() = None;
}

/// Expand user-entered paths like `~/...` and `$HOME/...`.
pub fn expand_user_path(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return PathBuf::new();
    }

    if trimmed == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home);
        }
    }

    if let Some(rest) = trimmed.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    if let Some(rest) = trimmed.strip_prefix("~\\") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }

    if let Some(rest) = trimmed.strip_prefix("$HOME/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }

    PathBuf::from(trimmed)
}

pub fn expand_user_path_string(raw: &str) -> String {
    expand_user_path(raw).to_string_lossy().to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(alias = "background_type", alias = "backgroundType", alias = "bg_type")]
    pub bg_type: String,
    pub bg_image: String,
    pub bg_video: String,
    pub content_opacity: f64,
    pub content_blur: f64,
    #[serde(alias = "cache_dir")]
    pub cache_dir: String,
    #[serde(alias = "current_config_name")]
    pub current_config_name: String,
    #[serde(alias = "github_token")]
    pub github_token: String,
    #[serde(alias = "show_websites")]
    pub show_websites: bool,
    #[serde(alias = "show_documents")]
    pub show_documents: bool,
    #[serde(alias = "language")]
    pub locale: String,
    #[serde(alias = "data_dir")]
    pub data_dir: String,
    pub initialized: bool,
    #[serde(alias = "tos_risk_acknowledged")]
    pub tos_risk_acknowledged: bool,
    #[serde(alias = "snowbreak_source_policy")]
    pub snowbreak_source_policy: String,
    #[serde(alias = "window_width")]
    pub window_width: f64,
    #[serde(alias = "window_height")]
    pub window_height: f64,
    #[serde(alias = "window_x")]
    pub window_x: Option<f64>,
    #[serde(alias = "window_y")]
    pub window_y: Option<f64>,
    pub theme: String,
    #[serde(alias = "custom_search_paths")]
    pub custom_search_paths: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let cache_dir = dirs_cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp/ssmt4"))
            .to_string_lossy()
            .to_string();
        Self {
            bg_type: "Image".to_string(),
            bg_image: String::new(),
            bg_video: String::new(),
            content_opacity: 0.0,
            content_blur: 0.0,
            cache_dir,
            current_config_name: "Default".to_string(),
            github_token: String::new(),
            show_websites: false,
            show_documents: false,
            locale: "zhs".to_string(),
            data_dir: String::new(),
            initialized: false,
            tos_risk_acknowledged: false,
            snowbreak_source_policy: "official_first".to_string(),
            window_width: 1280.0,
            window_height: 720.0,
            window_x: None,
            window_y: None,
            theme: "dark".to_string(),
            custom_search_paths: Vec::new(),
        }
    }
}

fn dirs_cache_dir() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        Some(PathBuf::from(xdg).join("ssmt4"))
    } else if let Ok(home) = std::env::var("HOME") {
        Some(PathBuf::from(home).join(".cache").join("ssmt4"))
    } else {
        None
    }
}

/// 配置目录：始终为 ~/.config/ssmt4（存放 settings.json、数据库等引导文件）
pub fn get_app_config_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("ssmt4")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config").join("ssmt4")
    } else {
        PathBuf::from("/tmp/ssmt4/config")
    }
}

/// 数据目录：dataDir 设置时直接使用该路径，否则 ~/.local/share/ssmt4
pub fn get_app_data_dir() -> PathBuf {
    if let Ok(guard) = CUSTOM_DATA_DIR.read() {
        if let Some(ref dir) = *guard {
            return dir.clone();
        }
    }
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(xdg).join("ssmt4")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("ssmt4")
    } else {
        PathBuf::from("/tmp/ssmt4/data")
    }
}

/// 缓存目录：始终为 ~/.cache/ssmt4
pub fn get_app_cache_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(xdg).join("ssmt4")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".cache").join("ssmt4")
    } else {
        PathBuf::from("/tmp/ssmt4/cache")
    }
}
