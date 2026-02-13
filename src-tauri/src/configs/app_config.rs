use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

pub fn get_app_config_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("ssmt4")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config").join("ssmt4")
    } else {
        PathBuf::from("/tmp/ssmt4/config")
    }
}

pub fn get_app_data_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(xdg).join("ssmt4")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".local").join("share").join("ssmt4")
    } else {
        PathBuf::from("/tmp/ssmt4/data")
    }
}

pub fn get_app_cache_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(xdg).join("ssmt4")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".cache").join("ssmt4")
    } else {
        PathBuf::from("/tmp/ssmt4/cache")
    }
}
