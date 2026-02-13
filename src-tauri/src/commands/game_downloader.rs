use crate::downloader::cdn::{self, LauncherInfo};
use crate::downloader::progress::LauncherState;
use crate::downloader::{full_download, incremental, verifier};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::Mutex;
use tracing::info;

/// Global cancel token for download operations
static CANCEL_TOKEN: once_cell::sync::Lazy<Arc<Mutex<bool>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(false)));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub state: LauncherState,
    pub local_version: Option<String>,
    pub remote_version: Option<String>,
    pub supports_incremental: bool,
}

#[tauri::command]
pub async fn get_launcher_info(launcher_api: String) -> Result<LauncherInfo, String> {
    cdn::fetch_launcher_info(&launcher_api).await
}

#[tauri::command]
pub async fn get_game_state(
    launcher_api: String,
    game_folder: String,
) -> Result<GameState, String> {
    let game_path = PathBuf::from(&game_folder);

    // Fetch remote info
    let launcher_info = match cdn::fetch_launcher_info(&launcher_api).await {
        Ok(info) => info,
        Err(_e) => {
            return Ok(GameState {
                state: LauncherState::NetworkError,
                local_version: get_local_version_internal(&game_path),
                remote_version: None,
                supports_incremental: false,
            });
        }
    };

    let local_version = get_local_version_internal(&game_path);
    let remote_version = Some(launcher_info.version.clone());

    // Determine state
    let state = if local_version.is_none() {
        // Check if any resource files exist
        LauncherState::NeedInstall
    } else if local_version.as_deref() != Some(&launcher_info.version) {
        LauncherState::NeedUpdate
    } else {
        LauncherState::StartGame
    };

    // Check incremental support
    let supports_incremental = if let Some(ref lv) = local_version {
        launcher_info
            .patch_configs
            .iter()
            .any(|p| p.version == *lv && !p.ext.is_empty())
    } else {
        false
    };

    Ok(GameState {
        state,
        local_version,
        remote_version,
        supports_incremental,
    })
}

#[tauri::command]
pub async fn download_game(
    app: AppHandle,
    launcher_api: String,
    game_folder: String,
) -> Result<(), String> {
    // Reset cancel token
    *CANCEL_TOKEN.lock().await = false;

    let game_path = PathBuf::from(&game_folder);
    std::fs::create_dir_all(&game_path)
        .map_err(|e| format!("Failed to create game folder: {}", e))?;

    let launcher_info = cdn::fetch_launcher_info(&launcher_api).await?;
    let resource_index =
        cdn::fetch_resource_index(&launcher_info.cdn_url, &launcher_info.index_file_url).await?;

    full_download::download_game(
        app,
        &launcher_info,
        &resource_index,
        &game_path,
        CANCEL_TOKEN.clone(),
    )
    .await?;

    // Write local version after successful download
    write_local_version(&game_path, &launcher_info.version)?;

    info!("Full download completed for {}", game_folder);
    Ok(())
}

#[tauri::command]
pub async fn update_game(
    app: AppHandle,
    launcher_api: String,
    game_folder: String,
) -> Result<(), String> {
    *CANCEL_TOKEN.lock().await = false;

    let game_path = PathBuf::from(&game_folder);
    let launcher_info = cdn::fetch_launcher_info(&launcher_api).await?;
    let resource_index =
        cdn::fetch_resource_index(&launcher_info.cdn_url, &launcher_info.index_file_url).await?;

    incremental::update_game_full(
        app,
        &launcher_info,
        &resource_index,
        &game_path,
        CANCEL_TOKEN.clone(),
    )
    .await?;

    write_local_version(&game_path, &launcher_info.version)?;
    info!("Full comparison update completed for {}", game_folder);
    Ok(())
}

#[tauri::command]
pub async fn update_game_patch(
    app: AppHandle,
    launcher_api: String,
    game_folder: String,
) -> Result<(), String> {
    *CANCEL_TOKEN.lock().await = false;

    let game_path = PathBuf::from(&game_folder);
    let local_version = get_local_version_internal(&game_path)
        .ok_or("No local version found, cannot do incremental update")?;

    let launcher_info = cdn::fetch_launcher_info(&launcher_api).await?;

    incremental::update_game_patch(
        app,
        &launcher_info,
        &local_version,
        &game_path,
        CANCEL_TOKEN.clone(),
    )
    .await?;

    write_local_version(&game_path, &launcher_info.version)?;
    info!("Incremental patch update completed for {}", game_folder);
    Ok(())
}

#[tauri::command]
pub async fn verify_game_files(
    app: AppHandle,
    launcher_api: String,
    game_folder: String,
) -> Result<verifier::VerifyResult, String> {
    *CANCEL_TOKEN.lock().await = false;

    let game_path = PathBuf::from(&game_folder);
    let launcher_info = cdn::fetch_launcher_info(&launcher_api).await?;
    let resource_index =
        cdn::fetch_resource_index(&launcher_info.cdn_url, &launcher_info.index_file_url).await?;

    let result = verifier::verify_game_files(
        app,
        &launcher_info,
        &resource_index,
        &game_path,
        CANCEL_TOKEN.clone(),
    )
    .await?;

    write_local_version(&game_path, &launcher_info.version)?;
    Ok(result)
}

#[tauri::command]
pub async fn cancel_download() -> Result<(), String> {
    *CANCEL_TOKEN.lock().await = true;
    info!("Download cancellation requested");
    Ok(())
}

#[tauri::command]
pub fn get_local_version(game_folder: String) -> Result<Option<String>, String> {
    Ok(get_local_version_internal(&PathBuf::from(game_folder)))
}

fn get_local_version_internal(game_folder: &PathBuf) -> Option<String> {
    let config_path = game_folder.join("launcherDownloadConfig.json");
    if !config_path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&config_path).ok()?;
    let data: serde_json::Value = serde_json::from_str(&content).ok()?;
    data.get("version").and_then(|v| v.as_str()).map(|s| s.to_string())
}

/// 根据游戏预设返回对应的 launcher API URL（目前仅支持鸣潮）
#[tauri::command]
pub fn get_game_launcher_api(game_preset: String) -> Result<serde_json::Value, String> {
    match game_preset.as_str() {
        // 鸣潮（国服）
        "WWMI" | "WuWa" => Ok(serde_json::json!({
            "launcherApi": "https://prod-cn-alicdn-gamestarter.kurogame.com/launcher/game/G152/10003_Y8xXrXk65DqFHEDgApn3cpK5lfczpFx5/index.json",
            "launcherDownloadApi": "https://prod-cn-alicdn-gamestarter.kurogame.com/launcher/launcher/10003_Y8xXrXk65DqFHEDgApn3cpK5lfczpFx5/G152/index.json",
            "defaultFolder": "Wuthering Waves Game",
            "supported": true
        })),
        _ => Ok(serde_json::json!({
            "supported": false
        })),
    }
}

/// 返回游戏默认安装目录
#[tauri::command]
pub fn get_default_game_folder(game_name: String) -> Result<String, String> {
    let data_dir = crate::configs::app_config::get_app_data_dir();
    let game_dir = data_dir.join("games").join(&game_name);
    Ok(game_dir.to_string_lossy().to_string())
}

fn write_local_version(game_folder: &PathBuf, version: &str) -> Result<(), String> {
    let config = serde_json::json!({
        "version": version,
        "reUseVersion": "",
        "state": "",
        "isPreDownload": false,
        "appId": "10003"
    });
    let config_path = game_folder.join("launcherDownloadConfig.json");
    let content = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize: {}", e))?;
    std::fs::write(&config_path, content)
        .map_err(|e| format!("Failed to write version config: {}", e))
}
