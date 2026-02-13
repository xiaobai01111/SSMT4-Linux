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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub background_type: String,
    pub cache_dir: String,
    pub window_width: f64,
    pub window_height: f64,
    pub window_x: Option<f64>,
    pub window_y: Option<f64>,
    pub language: String,
    pub theme: String,
    pub custom_search_paths: Vec<String>,
    #[serde(default, rename = "dataDir")]
    pub data_dir: String,
    #[serde(default)]
    pub initialized: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        let cache_dir = dirs_cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp/ssmt4"))
            .to_string_lossy()
            .to_string();
        Self {
            background_type: "image".to_string(),
            cache_dir,
            window_width: 1280.0,
            window_height: 720.0,
            window_x: None,
            window_y: None,
            language: "zh-CN".to_string(),
            theme: "dark".to_string(),
            custom_search_paths: Vec::new(),
            data_dir: String::new(),
            initialized: false,
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
        PathBuf::from(home).join(".local").join("share").join("ssmt4")
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
