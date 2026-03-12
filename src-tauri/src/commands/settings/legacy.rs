use crate::configs::app_config::{self, AppConfig};
use crate::configs::database::SettingRecord;

fn apply_legacy_bootstrap_data_dir_if_missing(
    cfg: &mut AppConfig,
    legacy_bootstrap_data_dir: Option<String>,
) {
    if cfg.data_dir.trim().is_empty() {
        if let Some(value) = legacy_bootstrap_data_dir {
            cfg.data_dir = value.trim().to_string();
        }
    }
}

pub(super) fn migrate_json_to_db(
    normalize_settings: impl FnOnce(&mut AppConfig) -> bool,
) -> Result<AppConfig, String> {
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

    apply_legacy_bootstrap_data_dir_if_missing(
        &mut cfg,
        app_config::read_legacy_bootstrap_data_dir(),
    );
    let _ = normalize_settings(&mut cfg);
    crate::configs::database::save_app_config(&cfg)?;
    Ok(cfg)
}

pub(super) fn migrate_legacy_settings_to_db(
    pairs: &[SettingRecord],
    normalize_settings: impl FnOnce(&mut AppConfig) -> bool,
) -> Result<AppConfig, String> {
    let mut cfg = legacy_settings_from_kv(pairs);
    apply_legacy_bootstrap_data_dir_if_missing(
        &mut cfg,
        app_config::read_legacy_bootstrap_data_dir(),
    );
    let _ = normalize_settings(&mut cfg);
    crate::configs::database::save_app_config(&cfg)?;
    Ok(cfg)
}

pub(super) fn legacy_settings_from_kv(pairs: &[SettingRecord]) -> AppConfig {
    let get = |key: &str| -> String {
        pairs
            .iter()
            .find(|record| record.key == key)
            .map(|record| record.value.clone())
            .unwrap_or_default()
    };
    let get_any = |keys: &[&str]| -> String {
        keys.iter()
            .find_map(|key| {
                pairs
                    .iter()
                    .find(|record| record.key == *key)
                    .map(|record| record.value.clone())
            })
            .unwrap_or_default()
    };

    let defaults = AppConfig::default();

    let bg_type_raw = get_any(&["bg_type", "bgType", "background_type", "backgroundType"]);
    let locale_raw = get_any(&["locale", "language"]);

    AppConfig {
        bg_type: if bg_type_raw.trim().eq_ignore_ascii_case("video") {
            "Video".to_string()
        } else {
            defaults.bg_type.clone()
        },
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
        content_opacity: parse_f64_or_default(
            &get_any(&["content_opacity", "contentOpacity"]),
            defaults.content_opacity,
        ),
        content_blur: parse_f64_or_default(
            &get_any(&["content_blur", "contentBlur"]),
            defaults.content_blur,
        ),
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
        show_websites: parse_bool_or_default(
            &get_any(&["show_websites", "showWebsites"]),
            defaults.show_websites,
        ),
        show_documents: parse_bool_or_default(
            &get_any(&["show_documents", "showDocuments"]),
            defaults.show_documents,
        ),
        migoto_enabled: parse_bool_or_default(
            &get_any(&["migoto_enabled", "migotoEnabled"]),
            defaults.migoto_enabled,
        ),
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
        tos_risk_acknowledged: parse_bool_or_default(
            &get_any(&["tos_risk_acknowledged", "tosRiskAcknowledged"]),
            defaults.tos_risk_acknowledged,
        ),
        onboarding_completed: parse_bool_or_default(
            &get_any(&["onboarding_completed", "onboardingCompleted"]),
            defaults.onboarding_completed,
        ),
        onboarding_version: parse_u32_or_default(
            &get_any(&["onboarding_version", "onboardingVersion"]),
            defaults.onboarding_version,
        ),
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

fn parse_u32_or_default(value: &str, default: u32) -> u32 {
    value.parse().unwrap_or(default)
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

fn normalize_snowbreak_source_policy(value: &str, default: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "official_first" | "community_first" => normalized,
        "" => default.to_string(),
        _ => default.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_legacy_bootstrap_data_dir_if_missing, legacy_settings_from_kv};
    use crate::configs::app_config::AppConfig;
    use crate::configs::database::SettingRecord;

    #[test]
    fn legacy_settings_kv_restores_aliased_fields() {
        let pairs = vec![
            SettingRecord {
                key: "background_type".to_string(),
                value: "video".to_string(),
            },
            SettingRecord {
                key: "language".to_string(),
                value: "zh-TW".to_string(),
            },
            SettingRecord {
                key: "show_websites".to_string(),
                value: "true".to_string(),
            },
            SettingRecord {
                key: "current_config_name".to_string(),
                value: "Snowbreak".to_string(),
            },
            SettingRecord {
                key: "custom_search_paths".to_string(),
                value: r#"["/games/custom"]"#.to_string(),
            },
        ];

        let restored = legacy_settings_from_kv(&pairs);

        assert_eq!(restored.bg_type, "Video");
        assert_eq!(restored.locale, "zht");
        assert!(restored.show_websites);
        assert_eq!(restored.current_config_name, "Snowbreak");
        assert_eq!(restored.custom_search_paths, vec!["/games/custom"]);
    }

    #[test]
    fn bootstrap_data_dir_only_fills_missing_runtime_data_dir() {
        let mut missing = AppConfig::default();
        apply_legacy_bootstrap_data_dir_if_missing(
            &mut missing,
            Some("~/bootstrap-data".to_string()),
        );
        assert_eq!(missing.data_dir, "~/bootstrap-data");

        let mut explicit = AppConfig {
            data_dir: "~/sqlite-data".to_string(),
            ..AppConfig::default()
        };
        apply_legacy_bootstrap_data_dir_if_missing(
            &mut explicit,
            Some("~/bootstrap-data".to_string()),
        );
        assert_eq!(explicit.data_dir, "~/sqlite-data");
    }
}
