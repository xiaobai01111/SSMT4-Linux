use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

const LEGACY_BOOTSTRAP_SETTINGS_FILE: &str = "bootstrap-settings.json";

/// 运行期 dataDir 镜像。
/// 持久化真相源始终是 SQLite 中的 AppConfig.data_dir；
/// 这里仅缓存已解析后的当前进程值，供底层路径函数复用。
static CUSTOM_DATA_DIR: once_cell::sync::Lazy<RwLock<Option<PathBuf>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(None));

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct BootstrapSettings {
    data_dir: String,
}

/// 启动时设置自定义数据根目录
pub fn set_custom_data_dir(dir: PathBuf) {
    tracing::info!("Custom data dir set to: {}", dir.display());
    *CUSTOM_DATA_DIR.write().unwrap() = Some(dir);
}

/// 清除自定义数据根目录（恢复默认）
pub fn clear_custom_data_dir() {
    *CUSTOM_DATA_DIR.write().unwrap() = None;
}

/// 根据持久化配置值同步进程内 dataDir 覆盖，不触发符号链接等副作用。
pub fn apply_runtime_data_dir_override(raw: &str) {
    if raw.trim().is_empty() {
        clear_custom_data_dir();
    } else {
        set_custom_data_dir(expand_user_path(raw));
    }
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

pub fn get_legacy_bootstrap_settings_path() -> PathBuf {
    get_app_config_dir().join(LEGACY_BOOTSTRAP_SETTINGS_FILE)
}

fn read_legacy_bootstrap_data_dir_from_path(path: &Path) -> Option<String> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return None,
    };

    match serde_json::from_str::<BootstrapSettings>(&content) {
        Ok(settings) => Some(settings.data_dir.trim().to_string()),
        Err(err) => {
            tracing::warn!("解析旧引导设置失败 {}: {}", path.display(), err);
            None
        }
    }
}

pub fn read_legacy_bootstrap_data_dir() -> Option<String> {
    read_legacy_bootstrap_data_dir_from_path(&get_legacy_bootstrap_settings_path())
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
    #[serde(alias = "migoto_enabled")]
    pub migoto_enabled: bool,
    #[serde(alias = "language")]
    pub locale: String,
    #[serde(alias = "data_dir")]
    pub data_dir: String,
    pub initialized: bool,
    #[serde(alias = "tos_risk_acknowledged")]
    pub tos_risk_acknowledged: bool,
    #[serde(alias = "onboarding_completed")]
    pub onboarding_completed: bool,
    #[serde(alias = "onboarding_version")]
    pub onboarding_version: u32,
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
            migoto_enabled: false,
            locale: "zhs".to_string(),
            data_dir: String::new(),
            initialized: false,
            tos_risk_acknowledged: false,
            onboarding_completed: false,
            onboarding_version: 0,
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

pub fn serialize_app_config(cfg: &AppConfig) -> Result<String, String> {
    serde_json::to_string(cfg).map_err(|e| format!("序列化 AppConfig 失败: {}", e))
}

pub fn deserialize_app_config(raw: &str) -> Result<AppConfig, String> {
    serde_json::from_str(raw).map_err(|e| format!("解析 AppConfig 失败: {}", e))
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

#[cfg(test)]
mod tests {
    use super::{
        apply_runtime_data_dir_override, clear_custom_data_dir, deserialize_app_config,
        expand_user_path, get_app_data_dir, read_legacy_bootstrap_data_dir_from_path,
        serialize_app_config, set_custom_data_dir, AppConfig,
    };
    use once_cell::sync::Lazy;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_GUARD: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    fn unique_temp_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        std::env::temp_dir()
            .join("ssmt4-tests")
            .join(format!("{}-{}", label, nonce))
    }

    #[test]
    fn legacy_bootstrap_data_dir_reads_trimmed_value() {
        let path = unique_temp_path("bootstrap-data-dir").join("bootstrap-settings.json");
        let parent = path.parent().expect("parent");
        std::fs::create_dir_all(parent).expect("create parent");
        std::fs::write(&path, r#"{"dataDir":"~/Games"}"#).expect("write bootstrap config");

        assert_eq!(
            read_legacy_bootstrap_data_dir_from_path(&path),
            Some("~/Games".to_string())
        );

        let _ = std::fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[test]
    fn legacy_bootstrap_data_dir_empty_value_is_retained() {
        let path = unique_temp_path("bootstrap-empty-data-dir").join("bootstrap-settings.json");
        let parent = path.parent().expect("parent");
        std::fs::create_dir_all(parent).expect("create parent");
        std::fs::write(&path, r#"{"dataDir":""}"#).expect("write bootstrap config");

        assert_eq!(
            read_legacy_bootstrap_data_dir_from_path(&path),
            Some(String::new())
        );

        let _ = std::fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[test]
    fn app_config_roundtrip_preserves_serialized_fields() {
        let mut cfg = AppConfig::default();
        cfg.bg_type = "Video".to_string();
        cfg.current_config_name = "ZenlessZoneZero".to_string();
        cfg.custom_search_paths = vec!["/games/custom".to_string()];
        cfg.window_x = Some(120.0);
        cfg.window_y = Some(240.0);

        let json = serialize_app_config(&cfg).expect("serialize app config");
        let restored = deserialize_app_config(&json).expect("deserialize app config");

        assert_eq!(restored.bg_type, "Video");
        assert_eq!(restored.current_config_name, "ZenlessZoneZero");
        assert_eq!(restored.custom_search_paths, vec!["/games/custom"]);
        assert_eq!(restored.window_x, Some(120.0));
        assert_eq!(restored.window_y, Some(240.0));
    }

    #[test]
    fn app_config_deserialization_honors_aliases_and_defaults() {
        let restored = deserialize_app_config(
            r#"{
                "background_type": "video",
                "language": "zh-TW",
                "current_config_name": "Snowbreak",
                "migoto_enabled": true
            }"#,
        )
        .expect("deserialize aliased app config");

        assert_eq!(restored.bg_type, "video");
        assert_eq!(restored.locale, "zh-TW");
        assert_eq!(restored.current_config_name, "Snowbreak");
        assert!(restored.migoto_enabled);
        assert_eq!(restored.theme, AppConfig::default().theme);
    }

    #[test]
    fn expand_user_path_expands_home_shorthands_and_keeps_plain_paths() {
        let home = std::env::var("HOME").expect("HOME should be set in test environment");

        assert_eq!(expand_user_path(""), PathBuf::new());
        assert_eq!(expand_user_path("~"), PathBuf::from(&home));
        assert_eq!(
            expand_user_path("~/Games/SSMT4"),
            PathBuf::from(&home).join("Games/SSMT4")
        );
        assert_eq!(
            expand_user_path("$HOME/Games/SSMT4"),
            PathBuf::from(&home).join("Games/SSMT4")
        );
        assert_eq!(expand_user_path("/opt/SSMT4"), PathBuf::from("/opt/SSMT4"));
    }

    #[test]
    fn custom_data_dir_overrides_and_clear_restores_default_resolution() {
        let _guard = TEST_GUARD.lock().unwrap();
        clear_custom_data_dir();
        let default_dir = get_app_data_dir();

        let custom_dir = unique_temp_path("custom-data-dir");
        set_custom_data_dir(custom_dir.clone());
        assert_eq!(get_app_data_dir(), custom_dir);

        clear_custom_data_dir();
        assert_eq!(get_app_data_dir(), default_dir);
    }

    #[test]
    fn apply_runtime_data_dir_override_expands_or_clears_custom_dir() {
        let _guard = TEST_GUARD.lock().unwrap();
        clear_custom_data_dir();
        let default_dir = get_app_data_dir();

        apply_runtime_data_dir_override("$HOME/ssmt4-runtime");
        if let Ok(home) = std::env::var("HOME") {
            assert_eq!(
                get_app_data_dir(),
                PathBuf::from(home).join("ssmt4-runtime")
            );
        }

        apply_runtime_data_dir_override("");
        assert_eq!(get_app_data_dir(), default_dir);
    }
}
