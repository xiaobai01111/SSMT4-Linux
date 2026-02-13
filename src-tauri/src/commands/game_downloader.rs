use crate::downloader::cdn::{self, LauncherInfo};
use crate::downloader::hoyoverse;
use crate::downloader::progress::LauncherState;
use crate::downloader::{full_download, hoyoverse_download, incremental, verifier};
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

    // HoYoverse 分支
    if hoyoverse::is_hoyoverse_api(&launcher_api) {
        return get_game_state_hoyoverse(&launcher_api, &game_path).await;
    }

    // Kuro Games 分支
    let launcher_info = match cdn::fetch_launcher_info(&launcher_api).await {
        Ok(info) => info,
        Err(e) => {
            tracing::error!("fetch_launcher_info failed: {}", e);
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

    let state = if local_version.is_none() {
        LauncherState::NeedInstall
    } else if local_version.as_deref() != Some(&launcher_info.version) {
        LauncherState::NeedUpdate
    } else {
        LauncherState::StartGame
    };

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
    *CANCEL_TOKEN.lock().await = false;

    let game_path = PathBuf::from(&game_folder);
    std::fs::create_dir_all(&game_path)
        .map_err(|e| format!("Failed to create game folder: {}", e))?;

    // HoYoverse 分支
    if hoyoverse::is_hoyoverse_api(&launcher_api) {
        let biz = detect_biz_prefix(&launcher_api);
        let game_pkg = hoyoverse::fetch_game_packages(&launcher_api, biz).await?;
        hoyoverse_download::download_game(app, &game_pkg, &game_path, CANCEL_TOKEN.clone()).await?;
        info!("HoYoverse full download completed for {}", game_folder);
        return Ok(());
    }

    // Kuro Games 分支
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

    // HoYoverse 分支
    if hoyoverse::is_hoyoverse_api(&launcher_api) {
        let biz = detect_biz_prefix(&launcher_api);
        let game_pkg = hoyoverse::fetch_game_packages(&launcher_api, biz).await?;
        let local_version = get_local_version_internal(&game_path)
            .ok_or("未找到本地版本，请使用全量下载".to_string())?;
        hoyoverse_download::update_game(
            app, &game_pkg, &local_version, &game_path, CANCEL_TOKEN.clone()
        ).await?;
        info!("HoYoverse update completed for {}", game_folder);
        return Ok(());
    }

    // Kuro Games 分支
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

    // HoYoverse 分支
    if hoyoverse::is_hoyoverse_api(&launcher_api) {
        let biz = detect_biz_prefix(&launcher_api);
        let game_pkg = hoyoverse::fetch_game_packages(&launcher_api, biz).await?;
        return hoyoverse_download::verify_game(
            app, &game_pkg, &game_path, CANCEL_TOKEN.clone()
        ).await;
    }

    // Kuro Games 分支
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
    // 优先读取 .version (HoYoverse)
    let version_file = game_folder.join(".version");
    if version_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&version_file) {
            let ver = content.trim().to_string();
            if !ver.is_empty() {
                return Some(ver);
            }
        }
    }
    // 回退到 launcherDownloadConfig.json (Kuro Games)
    let config_path = game_folder.join("launcherDownloadConfig.json");
    if !config_path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&config_path).ok()?;
    let data: serde_json::Value = serde_json::from_str(&content).ok()?;
    data.get("version").and_then(|v| v.as_str()).map(|s| s.to_string())
}

/// 根据游戏预设返回对应的 launcher API URL
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
        // 崩坏：星穹铁道（国服）
        "SRMI" => Ok(serde_json::json!({
            "launcherApi": "https://hyp-api.mihoyo.com/hyp/hyp-connect/api/getGamePackages?launcher_id=jGHBHlcOq1",
            "defaultFolder": "StarRail",
            "supported": true
        })),
        _ => Ok(serde_json::json!({
            "supported": false
        })),
    }
}

/// 返回游戏默认安装目录（自动跟随软件数据目录 dataDir）
#[tauri::command]
pub fn get_default_game_folder(game_name: String) -> Result<String, String> {
    let data_dir = crate::configs::app_config::get_app_data_dir();
    let game_dir = data_dir.join("games").join(&game_name);
    Ok(game_dir.to_string_lossy().to_string())
}

/// HoYoverse 游戏状态检测
async fn get_game_state_hoyoverse(
    launcher_api: &str,
    game_path: &PathBuf,
) -> Result<GameState, String> {
    let biz = detect_biz_prefix(launcher_api);
    let game_pkg = match hoyoverse::fetch_game_packages(launcher_api, biz).await {
        Ok(pkg) => pkg,
        Err(e) => {
            tracing::error!("HoYoverse API 失败: {}", e);
            return Ok(GameState {
                state: LauncherState::NetworkError,
                local_version: get_local_version_internal(game_path),
                remote_version: None,
                supports_incremental: false,
            });
        }
    };

    let remote_version = game_pkg.main.major.version.clone();
    let local_version = get_local_version_internal(game_path);

    let state = if local_version.is_none() {
        LauncherState::NeedInstall
    } else if local_version.as_deref() != Some(&remote_version) {
        LauncherState::NeedUpdate
    } else {
        LauncherState::StartGame
    };

    // 检查是否有匹配当前版本的增量补丁
    let supports_incremental = if let Some(ref lv) = local_version {
        game_pkg.main.patches.iter().any(|p| p.version == *lv)
    } else {
        false
    };

    Ok(GameState {
        state,
        local_version,
        remote_version: Some(remote_version),
        supports_incremental,
    })
}

/// 根据 API URL 推断 biz_prefix
fn detect_biz_prefix(launcher_api: &str) -> &str {
    // 目前所有 HoYoverse API 共用同一个 launcher_id，需要靠 biz 前缀区分游戏
    // 但实际上 getGamePackages 会返回所有游戏，我们通过 biz 前缀筛选
    // 默认使用 hkrpg_ (星穹铁道)，未来可根据 URL 参数扩展
    if launcher_api.contains("getGamePackages") {
        // 通用 API，默认 Star Rail
        "hkrpg_"
    } else {
        ""
    }
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
