use super::{LauncherInstallerDownloadResult, LauncherInstallerState};
use crate::downloader::progress::{DownloadProgress, LauncherState, SpeedTracker};
use crate::events::{
    emit_game_download_progress, GameDownloadOperation, GameDownloadProgressEvent,
};
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::Mutex as AsyncMutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LauncherInstallerMeta {
    version: String,
    installer_path: String,
    source_api: String,
    remote_file_name: Option<String>,
    exe_size: Option<u64>,
    downloaded_at: String,
}

#[derive(Debug, Clone)]
struct LauncherInstallerRemoteInfo {
    version: String,
    installer_url: String,
    exe_size: Option<u64>,
}

pub(crate) struct InstallerDownloadRequest<'a> {
    pub(crate) app: AppHandle,
    pub(crate) task_id: &'a str,
    pub(crate) operation: GameDownloadOperation,
    pub(crate) launcher_api: String,
    pub(crate) game_path: PathBuf,
    pub(crate) game_preset: String,
    pub(crate) cancel_token: Arc<AsyncMutex<bool>>,
}

pub(crate) async fn get_launcher_installer_state(
    launcher_api: &str,
    game_folder: &Path,
    game_preset: &str,
) -> Result<LauncherInstallerState, String> {
    let installer_path = installer_path_for_preset(game_folder, game_preset);
    let local_meta = read_launcher_installer_meta(game_folder);
    let (local_version, local_installer_exists) =
        resolve_local_installer_for_source(local_meta.as_ref(), &installer_path, launcher_api);

    let remote_info = fetch_launcher_installer_remote(launcher_api).await;
    if let Err(err) = &remote_info {
        tracing::error!("fetch launcher installer info failed: {}", err);
    }
    let (remote_version, installer_url) = extract_remote_state_fields(&remote_info);
    let state = determine_launcher_state_from_remote(
        local_version.as_deref(),
        local_installer_exists,
        &remote_info,
    );

    Ok(LauncherInstallerState {
        state,
        local_version,
        remote_version,
        supports_incremental: false,
        installer_path: Some(installer_path.to_string_lossy().to_string()),
        installer_url,
    })
}

pub(crate) async fn download_launcher_installer(
    request: InstallerDownloadRequest<'_>,
) -> Result<LauncherInstallerDownloadResult, String> {
    let InstallerDownloadRequest {
        app,
        task_id,
        operation,
        launcher_api,
        game_path,
        game_preset,
        cancel_token,
    } = request;

    std::fs::create_dir_all(&game_path)
        .map_err(|e| format!("Failed to create game folder: {}", e))?;
    let remote = fetch_launcher_installer_remote(&launcher_api).await?;

    let installer_path = installer_path_for_preset(&game_path, &game_preset);
    let installer_name = installer_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("LauncherInstaller.exe")
        .to_string();
    let temp_path = installer_path.with_file_name(format!("{}.part", installer_name));
    let remote_name = remote_file_name_from_url(&remote.installer_url).unwrap_or(installer_name);

    let client = Client::builder()
        .user_agent("Mozilla/5.0")
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| format!("Failed to build download client: {}", e))?;

    let resp = client
        .get(&remote.installer_url)
        .send()
        .await
        .map_err(|e| format!("Failed to download launcher installer: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!(
            "Failed to download launcher installer: HTTP {}",
            resp.status()
        ));
    }

    let mut output = std::fs::File::create(&temp_path)
        .map_err(|e| format!("Failed to create temp installer file: {}", e))?;
    let total_size = resp.content_length().or(remote.exe_size).unwrap_or(0);
    let mut downloaded: u64 = 0;
    let mut speed = SpeedTracker::new();
    let mut stream = resp.bytes_stream();

    while let Some(chunk_result) = stream.next().await {
        if let Some(cancel_error) = cancellation_error_if_requested(&cancel_token, &temp_path).await
        {
            drop(output);
            return Err(cancel_error);
        }
        let chunk = chunk_result.map_err(|e| format!("Failed to read installer stream: {}", e))?;
        output
            .write_all(&chunk)
            .map_err(|e| format!("Failed to write installer file: {}", e))?;
        downloaded += chunk.len() as u64;
        speed.record(chunk.len() as u64);

        let progress =
            build_installer_progress_snapshot(downloaded, total_size, &remote_name, &mut speed);
        emit_game_download_progress(
            &app,
            &GameDownloadProgressEvent::from_progress(task_id, operation, &progress),
        );
    }

    output
        .flush()
        .map_err(|e| format!("Failed to flush installer file: {}", e))?;
    drop(output);

    if let Some(cancel_error) = cancellation_error_if_requested(&cancel_token, &temp_path).await {
        return Err(cancel_error);
    }

    if installer_path.exists() {
        std::fs::remove_file(&installer_path)
            .map_err(|e| format!("Failed to replace old installer: {}", e))?;
    }
    std::fs::rename(&temp_path, &installer_path)
        .map_err(|e| format!("Failed to finalize installer download: {}", e))?;

    let meta = LauncherInstallerMeta {
        version: remote.version.clone(),
        installer_path: installer_path.to_string_lossy().to_string(),
        source_api: launcher_api,
        remote_file_name: Some(remote_name),
        exe_size: remote.exe_size,
        downloaded_at: chrono::Utc::now().to_rfc3339(),
    };
    write_launcher_installer_meta(&game_path, &meta)?;

    Ok(LauncherInstallerDownloadResult {
        installer_path: installer_path.to_string_lossy().to_string(),
        installer_url: remote.installer_url,
        version: remote.version,
    })
}

fn installer_file_name_for_preset(game_preset: &str) -> String {
    if game_preset.eq_ignore_ascii_case("ArknightsEndfield") {
        return "ArknightsEndfieldLauncherInstaller.exe".to_string();
    }
    format!("{}LauncherInstaller.exe", game_preset)
}

fn installer_path_for_preset(game_folder: &Path, game_preset: &str) -> PathBuf {
    game_folder.join(installer_file_name_for_preset(game_preset))
}

fn installer_meta_path(game_folder: &Path) -> PathBuf {
    game_folder.join(".launcher_installer_meta.json")
}

fn same_launcher_source(saved_source_api: &str, current_launcher_api: &str) -> bool {
    saved_source_api.trim() == current_launcher_api.trim()
}

fn resolve_local_installer_for_source(
    local_meta: Option<&LauncherInstallerMeta>,
    installer_path: &Path,
    launcher_api: &str,
) -> (Option<String>, bool) {
    let Some(meta) = local_meta else {
        return (None, false);
    };
    if !same_launcher_source(&meta.source_api, launcher_api) {
        return (None, false);
    }

    let meta_installer_exists = PathBuf::from(&meta.installer_path).exists();
    let fallback_exists = installer_path.exists();
    let exists = meta_installer_exists || fallback_exists;
    if !exists {
        return (None, false);
    }

    (Some(meta.version.clone()), true)
}

fn read_launcher_installer_meta(game_folder: &Path) -> Option<LauncherInstallerMeta> {
    let path = installer_meta_path(game_folder);
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<LauncherInstallerMeta>(&content).ok()
}

fn write_launcher_installer_meta(
    game_folder: &Path,
    meta: &LauncherInstallerMeta,
) -> Result<(), String> {
    let path = installer_meta_path(game_folder);
    let content = serde_json::to_string_pretty(meta)
        .map_err(|e| format!("Failed to serialize launcher installer meta: {}", e))?;
    std::fs::write(path, content)
        .map_err(|e| format!("Failed to write launcher installer meta: {}", e))
}

fn remote_file_name_from_url(url: &str) -> Option<String> {
    let no_query = url.split('?').next().unwrap_or(url);
    no_query
        .rsplit('/')
        .find(|seg| !seg.trim().is_empty())
        .map(|seg| seg.to_string())
}

fn read_u64_from_json(value: &serde_json::Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|v| u64::try_from(v).ok()))
        .or_else(|| value.as_str().and_then(|s| s.trim().parse::<u64>().ok()))
}

fn read_non_empty_string(v: Option<&serde_json::Value>) -> Option<String> {
    v.and_then(|x| x.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}

fn parse_launcher_installer_remote_from_value(
    value: &serde_json::Value,
) -> Result<LauncherInstallerRemoteInfo, String> {
    let payload = value.get("rsp").unwrap_or(value);
    let version = read_non_empty_string(payload.get("version"))
        .ok_or_else(|| "launcher response missing version".to_string())?;
    let installer_url = read_non_empty_string(payload.get("exe_url"))
        .ok_or_else(|| "launcher response missing exe_url".to_string())?;
    let exe_size = payload.get("exe_size").and_then(read_u64_from_json);

    Ok(LauncherInstallerRemoteInfo {
        version,
        installer_url,
        exe_size,
    })
}

fn determine_launcher_installer_state(
    local_version: Option<&str>,
    remote_version: Option<&str>,
    installer_exists: bool,
    remote_fetch_failed: bool,
) -> LauncherState {
    if remote_fetch_failed {
        return LauncherState::NetworkError;
    }

    let Some(remote) = remote_version else {
        return LauncherState::NeedInstall;
    };
    let Some(local) = local_version else {
        return LauncherState::NeedInstall;
    };

    if !installer_exists {
        return LauncherState::NeedUpdate;
    }
    if local != remote {
        return LauncherState::NeedUpdate;
    }
    LauncherState::StartGame
}

fn extract_remote_state_fields(
    remote_info: &Result<LauncherInstallerRemoteInfo, String>,
) -> (Option<String>, Option<String>) {
    let remote_version = remote_info
        .as_ref()
        .ok()
        .map(|remote| remote.version.clone());
    let installer_url = remote_info
        .as_ref()
        .ok()
        .map(|remote| remote.installer_url.clone());
    (remote_version, installer_url)
}

fn determine_launcher_state_from_remote(
    local_version: Option<&str>,
    local_installer_exists: bool,
    remote_info: &Result<LauncherInstallerRemoteInfo, String>,
) -> LauncherState {
    match remote_info {
        Ok(remote) => determine_launcher_installer_state(
            local_version,
            Some(remote.version.as_str()),
            local_installer_exists,
            false,
        ),
        Err(_) => {
            determine_launcher_installer_state(local_version, None, local_installer_exists, true)
        }
    }
}

fn build_installer_progress_snapshot(
    downloaded: u64,
    total_size: u64,
    remote_name: &str,
    speed: &mut SpeedTracker,
) -> DownloadProgress {
    let has_known_total = total_size > 0;
    let total_for_progress = if has_known_total {
        total_size
    } else {
        downloaded
    };
    let remaining = total_for_progress.saturating_sub(downloaded);
    DownloadProgress {
        phase: "download".to_string(),
        total_size: total_for_progress,
        finished_size: downloaded,
        total_count: 1,
        finished_count: usize::from(has_known_total && downloaded >= total_for_progress),
        current_file: remote_name.to_string(),
        speed_bps: speed.speed_bps(),
        eta_seconds: speed.eta_seconds(remaining),
    }
}

async fn cancellation_error_if_requested(
    cancel_token: &Arc<AsyncMutex<bool>>,
    temp_path: &Path,
) -> Option<String> {
    if *cancel_token.lock().await {
        let _ = std::fs::remove_file(temp_path);
        return Some("download cancelled".to_string());
    }
    None
}

async fn fetch_launcher_installer_remote(
    launcher_api: &str,
) -> Result<LauncherInstallerRemoteInfo, String> {
    let client = Client::builder()
        .user_agent("Mozilla/5.0")
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("Failed to build http client: {}", e))?;

    let resp = client
        .get(launcher_api)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch launcher api: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}: {}", resp.status(), launcher_api));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Failed to read launcher api body: {}", e))?;
    let text = match String::from_utf8(bytes.to_vec()) {
        Ok(v) => v,
        Err(_) => {
            let (decoded, _, _) = encoding_rs::GBK.decode(&bytes);
            decoded.into_owned()
        }
    };
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Invalid launcher api json: {}", e))?;
    parse_launcher_installer_remote_from_value(&value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;
    use std::sync::Arc;
    use std::sync::Mutex;
    use tokio::sync::Mutex as AsyncMutex;

    static TEST_GUARD: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    #[test]
    fn parse_launcher_installer_payload_supports_wrapped_json() {
        let wrapped = serde_json::json!({
            "rsp": {
                "version": "1.1.1",
                "exe_url": "https://example.com/launcher.exe?auth=1",
                "exe_size": "123456"
            }
        });
        let info = parse_launcher_installer_remote_from_value(&wrapped).expect("parse wrapped");
        assert_eq!(info.version, "1.1.1");
        assert_eq!(
            info.installer_url,
            "https://example.com/launcher.exe?auth=1"
        );
        assert_eq!(info.exe_size, Some(123456));
    }

    #[test]
    fn parse_launcher_installer_payload_supports_flat_json() {
        let flat = serde_json::json!({
            "version": "1.1.0",
            "exe_url": "https://example.com/launcher_global.exe",
            "exe_size": 223344
        });
        let info = parse_launcher_installer_remote_from_value(&flat).expect("parse flat");
        assert_eq!(info.version, "1.1.0");
        assert_eq!(
            info.installer_url,
            "https://example.com/launcher_global.exe"
        );
        assert_eq!(info.exe_size, Some(223344));
    }

    #[test]
    fn launcher_installer_state_machine_matches_expected() {
        assert_eq!(
            determine_launcher_installer_state(None, Some("1.0.0"), false, false),
            LauncherState::NeedInstall
        );
        assert_eq!(
            determine_launcher_installer_state(Some("1.0.0"), Some("1.1.0"), true, false),
            LauncherState::NeedUpdate
        );
        assert_eq!(
            determine_launcher_installer_state(Some("1.1.0"), Some("1.1.0"), true, false),
            LauncherState::StartGame
        );
        assert_eq!(
            determine_launcher_installer_state(Some("1.1.0"), None, true, true),
            LauncherState::NetworkError
        );
    }

    #[test]
    fn installer_meta_roundtrip_and_fixed_name() {
        let id = format!(
            "ssmt4-endfield-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        );
        let dir = std::env::temp_dir().join(id);
        std::fs::create_dir_all(&dir).expect("create temp dir");

        let installer = installer_path_for_preset(&dir, "ArknightsEndfield");
        assert!(installer
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n == "ArknightsEndfieldLauncherInstaller.exe"));

        let meta = LauncherInstallerMeta {
            version: "1.1.1".to_string(),
            installer_path: installer.to_string_lossy().to_string(),
            source_api: "https://example.com".to_string(),
            remote_file_name: Some("launcher.exe".to_string()),
            exe_size: Some(123),
            downloaded_at: "2026-02-18T00:00:00Z".to_string(),
        };
        write_launcher_installer_meta(&dir, &meta).expect("write meta");
        let loaded = read_launcher_installer_meta(&dir).expect("read meta");
        assert_eq!(loaded.version, "1.1.1");
        assert_eq!(loaded.exe_size, Some(123));

        let _ = std::fs::remove_file(installer_meta_path(&dir));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn local_installer_is_scoped_by_launcher_source() {
        let dir = std::env::temp_dir().join(format!(
            "ssmt4-installer-source-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");

        let installer_path = dir.join("ArknightsLauncherInstaller.exe");
        std::fs::write(&installer_path, b"dummy").expect("write installer");
        let meta = LauncherInstallerMeta {
            version: "1.1.1".to_string(),
            installer_path: installer_path.to_string_lossy().to_string(),
            source_api: "https://launcher.hypergryph.com/api/launcher/get_latest_launcher?appcode=ab&channel=1&sub_channel=1&ta=arknights".to_string(),
            remote_file_name: Some("launcher.exe".to_string()),
            exe_size: Some(123),
            downloaded_at: "2026-02-19T00:00:00Z".to_string(),
        };

        let (same_server_version, same_server_exists) = resolve_local_installer_for_source(
            Some(&meta),
            &installer_path,
            "https://launcher.hypergryph.com/api/launcher/get_latest_launcher?appcode=ab&channel=1&sub_channel=1&ta=arknights",
        );
        assert_eq!(same_server_version.as_deref(), Some("1.1.1"));
        assert!(same_server_exists);

        let (other_server_version, other_server_exists) = resolve_local_installer_for_source(
            Some(&meta),
            &installer_path,
            "https://launcher.gryphline.com/api/launcher/get_latest_launcher?appcode=ab&channel=6&sub_channel=6&ta=official",
        );
        assert!(other_server_version.is_none());
        assert!(!other_server_exists);

        let _ = std::fs::remove_file(&installer_path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn extract_remote_state_fields_handles_success_and_error() {
        let remote = LauncherInstallerRemoteInfo {
            version: "2.0.0".to_string(),
            installer_url: "https://example.com/launcher.exe".to_string(),
            exe_size: Some(1),
        };
        let (version, url) = extract_remote_state_fields(&Ok(remote));
        assert_eq!(version.as_deref(), Some("2.0.0"));
        assert_eq!(url.as_deref(), Some("https://example.com/launcher.exe"));

        let (version, url) = extract_remote_state_fields(&Err("network".to_string()));
        assert!(version.is_none());
        assert!(url.is_none());
    }

    #[test]
    fn determine_launcher_state_from_remote_reuses_core_state_machine() {
        let remote = LauncherInstallerRemoteInfo {
            version: "3.0.0".to_string(),
            installer_url: "https://example.com/launcher.exe".to_string(),
            exe_size: None,
        };
        assert_eq!(
            determine_launcher_state_from_remote(Some("3.0.0"), true, &Ok(remote.clone())),
            LauncherState::StartGame
        );
        assert_eq!(
            determine_launcher_state_from_remote(Some("2.0.0"), true, &Ok(remote)),
            LauncherState::NeedUpdate
        );
        assert_eq!(
            determine_launcher_state_from_remote(Some("2.0.0"), true, &Err("network".to_string())),
            LauncherState::NetworkError
        );
    }

    #[test]
    fn build_installer_progress_snapshot_handles_unknown_total_size() {
        let mut speed = SpeedTracker::new();
        speed.record(1024);
        let progress = build_installer_progress_snapshot(2048, 0, "launcher.exe", &mut speed);
        assert_eq!(progress.total_size, 2048);
        assert_eq!(progress.finished_size, 2048);
        assert_eq!(progress.total_count, 1);
        assert_eq!(progress.finished_count, 0);
        assert_eq!(progress.current_file, "launcher.exe");
    }

    #[tokio::test]
    async fn cancellation_error_if_requested_removes_temp_file_when_cancelled() {
        let _guard = TEST_GUARD.lock().unwrap();
        let dir = std::env::temp_dir().join(format!(
            "ssmt4-installer-cancel-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let temp_path = dir.join("launcher.part");
        std::fs::write(&temp_path, b"partial").expect("write part file");

        let token = Arc::new(AsyncMutex::new(true));
        let err = cancellation_error_if_requested(&token, &temp_path).await;
        assert_eq!(err.as_deref(), Some("download cancelled"));
        assert!(!temp_path.exists());

        let _ = std::fs::remove_dir_all(dir);
    }
}
