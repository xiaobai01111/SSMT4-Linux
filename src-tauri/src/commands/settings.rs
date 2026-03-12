mod data_dir;
mod legacy;
mod versioning;

use crate::configs::app_config::{self, AppConfig};
use crate::configs::database as db;
use crate::services::runtime_config;
use std::sync::Mutex;
use tauri::AppHandle;
use tauri::State;

use self::data_dir::sync_runtime_data_dir_and_filesystem;
use self::legacy::{migrate_json_to_db, migrate_legacy_settings_to_db};
pub use self::versioning::VersionCheckInfo;

fn load_or_migrate_app_config() -> Result<AppConfig, String> {
    let pairs = db::list_setting_records();
    let mut loaded = match db::load_app_config() {
        Ok(Some(cfg)) => cfg,
        Ok(None) => {
            if pairs.is_empty() {
                migrate_json_to_db(normalize_settings)?
            } else {
                migrate_legacy_settings_to_db(&pairs, normalize_settings)?
            }
        }
        Err(err) => {
            tracing::warn!("读取 AppConfig 持久化记录失败，回退到旧设置迁移: {}", err);
            if pairs.is_empty() {
                migrate_json_to_db(normalize_settings)?
            } else {
                migrate_legacy_settings_to_db(&pairs, normalize_settings)?
            }
        }
    };
    let normalized_changed = normalize_settings(&mut loaded);
    if normalized_changed {
        // 启动时自动回写归一化结果，避免旧值反复触发 asset 404。
        db::save_app_config(&loaded)?;
    }

    Ok(loaded)
}

pub(crate) fn bootstrap_runtime_app_config() -> Result<AppConfig, String> {
    let loaded = load_or_migrate_app_config()?;
    app_config::apply_runtime_data_dir_override(&loaded.data_dir);
    Ok(loaded)
}

#[tauri::command]
pub fn load_settings(config: State<'_, Mutex<AppConfig>>) -> Result<AppConfig, String> {
    runtime_config::snapshot(config.inner())
}

#[tauri::command]
pub fn save_settings(
    config: State<'_, Mutex<AppConfig>>,
    mut settings: AppConfig,
) -> Result<(), String> {
    let previous_data_dir = {
        let state = config.lock().map_err(|e| e.to_string())?;
        state.data_dir.clone()
    };

    let _ = normalize_settings(&mut settings);

    db::save_app_config(&settings)?;

    // 仅在 dataDir 发生变化时同步，避免普通设置保存触发昂贵的符号链接/目录操作。
    if previous_data_dir != settings.data_dir {
        sync_runtime_data_dir_and_filesystem(&settings);
    }

    runtime_config::replace(config.inner(), settings)?;
    Ok(())
}

#[tauri::command]
pub async fn get_version_check_info(app: AppHandle) -> Result<VersionCheckInfo, String> {
    versioning::get_version_check_info(app).await
}

#[tauri::command]
pub async fn get_resource_version_info() -> Result<VersionCheckInfo, String> {
    versioning::get_resource_version_info().await
}

#[tauri::command]
pub fn pull_resource_updates() -> Result<String, String> {
    versioning::pull_resource_updates()
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

fn normalize_settings(cfg: &mut AppConfig) -> bool {
    let mut changed = false;

    let normalized_bg_type = normalize_bg_type(&cfg.bg_type);
    if cfg.bg_type != normalized_bg_type {
        cfg.bg_type = normalized_bg_type;
        changed = true;
    }

    let normalized_locale = normalize_locale(&cfg.locale);
    if cfg.locale != normalized_locale {
        cfg.locale = normalized_locale;
        changed = true;
    }

    let normalized_source_policy =
        normalize_snowbreak_source_policy(&cfg.snowbreak_source_policy, "official_first");
    if cfg.snowbreak_source_policy != normalized_source_policy {
        cfg.snowbreak_source_policy = normalized_source_policy;
        changed = true;
    }

    if !cfg.current_config_name.trim().is_empty() && cfg.current_config_name != "Default" {
        let canonical =
            crate::configs::game_identity::to_canonical_or_keep(&cfg.current_config_name);
        if cfg.current_config_name != canonical {
            cfg.current_config_name = canonical;
            changed = true;
        }
    }

    let mut content_opacity = cfg.content_opacity;
    if content_opacity.is_nan() {
        content_opacity = 0.0;
    }
    content_opacity = content_opacity.clamp(0.0, 1.0);
    if cfg.content_opacity != content_opacity {
        cfg.content_opacity = content_opacity;
        changed = true;
    }

    let mut content_blur = cfg.content_blur;
    if content_blur.is_nan() || content_blur < 0.0 {
        content_blur = 0.0;
    }
    if cfg.content_blur != content_blur {
        cfg.content_blur = content_blur;
        changed = true;
    }

    // 迁移旧版默认背景路径（历史包名为 SSMT4-Linux-Dev），避免启动后持续 asset 404。
    if is_legacy_default_background_path(&cfg.bg_image) {
        cfg.bg_image.clear();
        changed = true;
    }

    if !cfg.data_dir.trim().is_empty() {
        let expanded = app_config::expand_user_path_string(&cfg.data_dir);
        if cfg.data_dir != expanded {
            cfg.data_dir = expanded;
            changed = true;
        }
    }
    if cfg.onboarding_version > 999 {
        cfg.onboarding_version = 999;
        changed = true;
    }

    changed
}

fn normalize_snowbreak_source_policy(value: &str, default: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "official_first" | "community_first" => normalized,
        "" => default.to_string(),
        _ => default.to_string(),
    }
}

fn is_legacy_default_background_path(value: &str) -> bool {
    let normalized = normalize_background_path_for_match(value);
    if normalized.is_empty() {
        return false;
    }

    // 历史残留路径特征：包名后带后缀（如 -dev）且直接指向 Background.png
    if normalized.contains("ssmt4-linux-") && normalized.contains("background.png") {
        return true;
    }

    // 生产包默认背景（deb/rpm/pacman/AppImage 挂载目录）不应持久化到用户设置。
    if !normalized.contains("background.png") {
        return false;
    }
    normalized.contains("/usr/lib/ssmt4/resources/background.png")
        || normalized.contains("/usr/lib/ssmt4-linux/resources/background.png")
        || normalized.contains("/usr/lib/ssmt4-linux/background.png")
}

fn normalize_background_path_for_match(value: &str) -> String {
    let mut normalized = value.trim().to_ascii_lowercase().replace('\\', "/");
    if let Some((head, _)) = normalized.split_once('?') {
        normalized = head.to_string();
    }
    if let Some((head, _)) = normalized.split_once('#') {
        normalized = head.to_string();
    }

    normalized = normalized.replace("%2f", "/");
    normalized = normalized.replace("%5c", "/");
    normalized
}

#[cfg(test)]
mod tests {
    use super::{
        is_legacy_default_background_path, normalize_locale, normalize_settings,
        normalize_snowbreak_source_policy,
    };
    use crate::configs::app_config::AppConfig;

    #[test]
    fn legacy_background_path_plain() {
        assert!(is_legacy_default_background_path(
            "/usr/lib/SSMT4-Linux-Dev/Background.png"
        ));
    }

    #[test]
    fn legacy_background_path_encoded() {
        assert!(is_legacy_default_background_path(
            "asset://localhost/%2Fusr%2Flib%2FSSMT4-Linux-Dev%2FBackground.png"
        ));
    }

    #[test]
    fn legacy_background_path_windows_style() {
        assert!(is_legacy_default_background_path(
            r"C:\Program Files\SSMT4-Linux-Dev\Background.png"
        ));
    }

    #[test]
    fn non_legacy_background_path() {
        assert!(!is_legacy_default_background_path(
            "/home/user/Pictures/Background.png"
        ));
    }

    #[test]
    fn appimage_mount_background_path() {
        assert!(is_legacy_default_background_path(
            "/tmp/.mount_SSMT4-pagEcf/usr/lib/SSMT4-Linux/resources/Background.png"
        ));
    }

    #[test]
    fn packaged_background_asset_url_path() {
        assert!(is_legacy_default_background_path(
            "asset://localhost/%2Ftmp%2F.mount_SSMT4-pagEcf%2Fusr%2Flib%2FSSMT4-Linux%2Fresources%2FBackground.png"
        ));
    }

    #[test]
    fn normalize_locale_maps_zh_variants_and_falls_back_to_en() {
        assert_eq!(normalize_locale("zh_CN"), "zhs");
        assert_eq!(normalize_locale("zh-HK"), "zht");
        assert_eq!(normalize_locale("en_US"), "en");
    }

    #[test]
    fn normalize_snowbreak_source_policy_accepts_known_values_and_defaults_unknown() {
        assert_eq!(
            normalize_snowbreak_source_policy("community-first", "official_first"),
            "community_first"
        );
        assert_eq!(
            normalize_snowbreak_source_policy("unknown", "official_first"),
            "official_first"
        );
        assert_eq!(
            normalize_snowbreak_source_policy("", "community_first"),
            "community_first"
        );
    }

    #[test]
    fn normalize_settings_clamps_and_cleans_legacy_values() {
        let mut cfg = AppConfig {
            bg_type: "video".to_string(),
            locale: "zh-HK".to_string(),
            snowbreak_source_policy: "community-first".to_string(),
            content_opacity: f64::NAN,
            content_blur: -5.0,
            bg_image: "/usr/lib/SSMT4-Linux/resources/Background.png".to_string(),
            data_dir: "$HOME/ssmt4-data".to_string(),
            onboarding_version: 1200,
            ..AppConfig::default()
        };

        let changed = normalize_settings(&mut cfg);

        assert!(changed);
        assert_eq!(cfg.bg_type, "Video");
        assert_eq!(cfg.locale, "zht");
        assert_eq!(cfg.snowbreak_source_policy, "community_first");
        assert_eq!(cfg.content_opacity, 0.0);
        assert_eq!(cfg.content_blur, 0.0);
        assert_eq!(cfg.bg_image, "");
        assert_eq!(cfg.onboarding_version, 999);
        if let Ok(home) = std::env::var("HOME") {
            assert_eq!(cfg.data_dir, format!("{}/ssmt4-data", home));
        }
    }
}

// ============================================================
// 3DMigoto / XXMI 资源包管理
// ============================================================

#[tauri::command]
pub fn get_xxmi_package_sources() -> Vec<crate::utils::migoto_packages::XxmiPackageSource> {
    crate::utils::migoto_packages::known_package_sources()
}

#[tauri::command]
pub fn scan_local_xxmi_packages() -> Result<crate::utils::migoto_packages::XxmiLocalStatus, String>
{
    Ok(crate::utils::migoto_packages::scan_local_xxmi_packages())
}

#[tauri::command]
pub async fn fetch_xxmi_remote_versions(
    source_id: &str,
    settings: State<'_, Mutex<AppConfig>>,
) -> Result<Vec<crate::utils::migoto_packages::XxmiRemoteVersion>, String> {
    let github_token = runtime_config::state_view(&settings, |cfg| cfg.github_token.clone())?;
    crate::utils::migoto_packages::fetch_xxmi_remote_versions(source_id, 20, Some(&github_token))
        .await
}

#[tauri::command]
pub async fn download_xxmi_package(
    source_id: &str,
    version: &str,
    download_url: &str,
) -> Result<String, String> {
    crate::utils::migoto_packages::download_xxmi_package(source_id, version, download_url).await
}

#[tauri::command]
pub fn deploy_xxmi_package(
    source_id: &str,
    version: &str,
    target_dir: &str,
) -> Result<String, String> {
    crate::utils::migoto_packages::deploy_xxmi_package(source_id, version, target_dir)
}

#[tauri::command]
pub fn delete_local_xxmi_package(source_id: &str, version: &str) -> Result<String, String> {
    crate::utils::migoto_packages::delete_local_xxmi_package(source_id, version)
}
