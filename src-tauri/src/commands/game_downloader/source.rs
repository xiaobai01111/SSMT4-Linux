use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::warn;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadSourceMeta {
    pub(crate) source_api: String,
    #[serde(default)]
    pub(crate) biz_prefix: Option<String>,
    pub(crate) updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DownloadSourceMatch {
    Match,
    Mismatch,
    Unknown,
}

pub(crate) fn get_local_version_internal(game_folder: &Path) -> Option<String> {
    for probe in version_probe_dirs(game_folder, 3) {
        if let Some(version) = read_local_version_from_dir(&probe) {
            return Some(version);
        }
    }
    None
}

pub(crate) fn write_local_version(game_folder: &Path, version: &str) -> Result<(), String> {
    let config = serde_json::json!({
        "version": version,
        "reUseVersion": "",
        "state": "",
        "isPreDownload": false,
        "appId": "10003"
    });
    let config_path = game_folder.join("launcherDownloadConfig.json");
    let content =
        serde_json::to_string_pretty(&config).map_err(|e| format!("Failed to serialize: {}", e))?;
    std::fs::write(&config_path, content)
        .map_err(|e| format!("Failed to write version config: {}", e))
}

pub(crate) fn write_download_source_meta(
    game_folder: &Path,
    launcher_api: &str,
    biz_prefix: Option<&str>,
) -> Result<(), String> {
    let source_api = launcher_api.trim();
    if source_api.is_empty() {
        return Ok(());
    }
    let meta = DownloadSourceMeta {
        source_api: source_api.to_string(),
        biz_prefix: normalize_biz_prefix(biz_prefix),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };
    let content = serde_json::to_string_pretty(&meta)
        .map_err(|e| format!("Failed to serialize download source meta: {}", e))?;
    std::fs::write(download_source_meta_path(game_folder), content)
        .map_err(|e| format!("Failed to write download source meta: {}", e))
}

pub(crate) fn get_local_version_for_source(
    game_folder: &Path,
    launcher_api: &str,
    biz_prefix: Option<&str>,
) -> Option<String> {
    let local_version = get_local_version_internal(game_folder)?;
    let meta = read_download_source_meta(game_folder)?;
    if can_reuse_local_version(match_download_source(&meta, launcher_api, biz_prefix)) {
        Some(local_version)
    } else {
        None
    }
}

pub(crate) fn persist_local_source_state(
    game_path: &Path,
    version: &str,
    launcher_api: &str,
    biz_prefix: Option<&str>,
) -> Result<(), String> {
    write_local_version(game_path, version)?;
    write_download_source_meta(game_path, launcher_api, biz_prefix)?;
    Ok(())
}

pub(crate) fn finalize_file_health_result(
    game_path: &Path,
    launcher_api: &str,
    biz_prefix: Option<&str>,
    version: &str,
    failed: &[String],
    operation: &str,
) -> Result<(), String> {
    if failed.is_empty() {
        persist_local_source_state(game_path, version, launcher_api, biz_prefix)?;
    } else {
        warn!(
            "{} finished with {} failed files; local version will not be updated",
            operation,
            failed.len()
        );
    }
    Ok(())
}

pub(crate) fn normalize_biz_prefix(biz_prefix: Option<&str>) -> Option<String> {
    biz_prefix
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn can_reuse_local_version(source_match: DownloadSourceMatch) -> bool {
    matches!(source_match, DownloadSourceMatch::Match)
}

pub(crate) fn require_hoyoverse_biz_prefix(biz_prefix: Option<&str>) -> Result<&str, String> {
    let Some(raw) = biz_prefix else {
        return Err("HoYoverse 下载缺少 biz_prefix，请在游戏预设服务器配置中提供".to_string());
    };
    let biz = raw.trim();
    if biz.is_empty() {
        return Err("HoYoverse 下载缺少 biz_prefix，请在游戏预设服务器配置中提供".to_string());
    }
    Ok(biz)
}

fn version_probe_dirs(game_folder: &Path, max_parent_depth: usize) -> Vec<PathBuf> {
    let mut probes = Vec::new();
    let mut current = Some(game_folder.to_path_buf());

    for _ in 0..=max_parent_depth {
        let Some(path) = current else { break };
        if !probes.iter().any(|existing: &PathBuf| existing == &path) {
            probes.push(path.clone());
        }
        current = path.parent().map(|parent| parent.to_path_buf());
    }

    probes
}

fn read_local_version_from_dir(game_folder: &Path) -> Option<String> {
    let version_file = game_folder.join(".version");
    if version_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&version_file) {
            let ver = content.trim().to_string();
            if !ver.is_empty() {
                return Some(ver);
            }
        }
    }

    let config_path = game_folder.join("launcherDownloadConfig.json");
    if !config_path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&config_path).ok()?;
    let data: serde_json::Value = serde_json::from_str(&content).ok()?;
    data.get("version")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn download_source_meta_path(game_folder: &Path) -> PathBuf {
    game_folder.join(".download_source_meta.json")
}

pub(crate) fn read_download_source_meta(game_folder: &Path) -> Option<DownloadSourceMeta> {
    let path = download_source_meta_path(game_folder);
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<DownloadSourceMeta>(&content).ok()
}

fn match_download_source(
    meta: &DownloadSourceMeta,
    launcher_api: &str,
    biz_prefix: Option<&str>,
) -> DownloadSourceMatch {
    if meta.source_api.trim() != launcher_api.trim() {
        return DownloadSourceMatch::Mismatch;
    }

    let current_biz = normalize_biz_prefix(biz_prefix);
    let saved_biz = normalize_biz_prefix(meta.biz_prefix.as_deref());

    match (saved_biz, current_biz) {
        (Some(saved), Some(current)) => {
            if saved == current {
                DownloadSourceMatch::Match
            } else {
                DownloadSourceMatch::Mismatch
            }
        }
        (None, None) => DownloadSourceMatch::Match,
        _ => DownloadSourceMatch::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn unique_temp_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "ssmt4-source-test-{label}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ))
    }

    #[test]
    fn full_game_local_version_is_scoped_by_source_meta() {
        let dir = unique_temp_dir("full");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        write_local_version(&dir, "3.1.0").expect("write local version");
        write_download_source_meta(&dir, "https://example.com/cn", None).expect("write source");

        assert_eq!(
            get_local_version_for_source(&dir, "https://example.com/cn", None).as_deref(),
            Some("3.1.0")
        );
        assert_eq!(
            get_local_version_for_source(&dir, "https://example.com/global", None),
            None
        );

        let _ = std::fs::remove_file(dir.join("launcherDownloadConfig.json"));
        let _ = std::fs::remove_file(download_source_meta_path(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn full_game_local_version_without_source_meta_is_not_reused() {
        let dir = unique_temp_dir("missing-meta");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        write_local_version(&dir, "3.1.0").expect("write local version");

        assert_eq!(
            get_local_version_for_source(&dir, "https://example.com/cn", None),
            None
        );
        assert!(
            read_download_source_meta(&dir).is_none(),
            "state probing should not silently backfill source metadata"
        );

        let _ = std::fs::remove_file(dir.join("launcherDownloadConfig.json"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn download_source_match_treats_missing_biz_as_unknown_not_same() {
        let meta = DownloadSourceMeta {
            source_api: "https://example.com/api".to_string(),
            biz_prefix: None,
            updated_at: "2026-03-09T00:00:00Z".to_string(),
        };

        assert_eq!(
            match_download_source(&meta, "https://example.com/api", Some("hkrpg_global")),
            DownloadSourceMatch::Unknown
        );
        assert_eq!(
            match_download_source(&meta, "https://example.com/api", None),
            DownloadSourceMatch::Match
        );
        assert_eq!(
            match_download_source(&meta, "https://example.com/other", None),
            DownloadSourceMatch::Mismatch
        );
    }

    #[test]
    fn write_download_source_meta_preserves_normalized_biz_prefix() {
        let dir = unique_temp_dir("meta-write");
        std::fs::create_dir_all(&dir).expect("create temp dir");

        write_download_source_meta(&dir, "https://example.com/api", Some("  hkrpg_global  "))
            .expect("write source meta");

        let loaded = read_download_source_meta(&dir).expect("read source meta");
        assert_eq!(loaded.source_api, "https://example.com/api");
        assert_eq!(loaded.biz_prefix.as_deref(), Some("hkrpg_global"));

        let _ = std::fs::remove_file(download_source_meta_path(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn can_reuse_local_version_only_accepts_exact_match() {
        assert!(can_reuse_local_version(DownloadSourceMatch::Match));
        assert!(!can_reuse_local_version(DownloadSourceMatch::Mismatch));
        assert!(!can_reuse_local_version(DownloadSourceMatch::Unknown));
    }

    #[test]
    fn read_local_version_from_download_config_trims_whitespace() {
        let dir = unique_temp_dir("config-trim");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("launcherDownloadConfig.json"),
            r#"{"version":"  4.5.6  "}"#,
        )
        .expect("write config");

        let version = read_local_version_from_dir(&dir);
        assert_eq!(version.as_deref(), Some("4.5.6"));

        let _ = std::fs::remove_file(dir.join("launcherDownloadConfig.json"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn require_hoyoverse_biz_prefix_rejects_missing_and_blank_values() {
        assert!(require_hoyoverse_biz_prefix(None).is_err());
        assert!(require_hoyoverse_biz_prefix(Some("   ")).is_err());
        assert_eq!(
            require_hoyoverse_biz_prefix(Some(" hkrpg_global ")).unwrap(),
            "hkrpg_global"
        );
    }

    #[test]
    fn version_probe_dirs_walks_up_to_parent_depth_without_duplicates() {
        let root = PathBuf::from("/tmp/ssmt4/source/root");
        let probes = version_probe_dirs(&root.join("a").join("b"), 3);

        assert_eq!(
            probes,
            vec![
                root.join("a").join("b"),
                root.join("a"),
                root.clone(),
                PathBuf::from("/tmp/ssmt4/source"),
            ]
        );
    }

    #[test]
    fn get_local_version_internal_falls_back_to_parent_probe_dirs() {
        let root = unique_temp_dir("parent-fallback");
        let nested = root.join("Game").join("Client").join("Binaries");
        std::fs::create_dir_all(&nested).expect("create nested dir");
        std::fs::write(root.join("Game").join(".version"), "3.2.1").expect("write parent version");

        let version = get_local_version_internal(&nested);

        assert_eq!(version.as_deref(), Some("3.2.1"));

        let _ = std::fs::remove_dir_all(root);
    }
}
