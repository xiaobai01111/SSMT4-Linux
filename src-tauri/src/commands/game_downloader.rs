use crate::configs::app_config::AppConfig;
use crate::downloader::cdn::{self, LauncherInfo};
use crate::downloader::hoyoverse;
use crate::downloader::progress::LauncherState;
use crate::downloader::snowbreak;
use crate::downloader::{
    full_download, hoyoverse_download, incremental, snowbreak_download, verifier,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};
use tauri::{AppHandle, State};
use tokio::sync::Mutex as AsyncMutex;
use tracing::info;

/// 按任务（game_folder）管理取消令牌，避免并行任务互相干扰
static CANCEL_TOKENS: once_cell::sync::Lazy<StdMutex<HashMap<String, Arc<AsyncMutex<bool>>>>> =
    once_cell::sync::Lazy::new(|| StdMutex::new(HashMap::new()));

/// 获取指定任务的取消令牌（每次创建全新令牌，避免旧状态污染）
///
/// 总是用新 Arc 替换旧条目，新任务必定从 false 起步。
/// 旧任务仍持有自己的 Arc 副本（可能已被置 true），不受影响。
fn get_cancel_token(task_id: &str) -> Arc<AsyncMutex<bool>> {
    let mut tokens = CANCEL_TOKENS.lock().unwrap();
    let token = Arc::new(AsyncMutex::new(false));
    tokens.insert(task_id.to_string(), token.clone());
    token
}

/// 清理已完成任务的令牌
fn cleanup_cancel_token(task_id: &str) {
    if let Ok(mut tokens) = CANCEL_TOKENS.lock() {
        tokens.remove(task_id);
    }
}

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
    settings: State<'_, StdMutex<AppConfig>>,
    launcher_api: String,
    game_folder: String,
    biz_prefix: Option<String>,
) -> Result<GameState, String> {
    let game_path = PathBuf::from(&game_folder);
    let snowbreak_policy = get_snowbreak_policy(&settings);

    // Snowbreak 分支
    if snowbreak::is_snowbreak_api(&launcher_api) {
        return get_game_state_snowbreak(&game_path, snowbreak_policy).await;
    }

    // HoYoverse 分支
    if hoyoverse::is_hoyoverse_api(&launcher_api) {
        let biz = require_hoyoverse_biz_prefix(biz_prefix.as_deref())?;
        return get_game_state_hoyoverse(&launcher_api, &game_path, biz).await;
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
    settings: State<'_, StdMutex<AppConfig>>,
    launcher_api: String,
    game_folder: String,
    languages: Option<Vec<String>>,
    biz_prefix: Option<String>,
) -> Result<(), String> {
    let cancel_token = get_cancel_token(&game_folder);
    let snowbreak_policy = get_snowbreak_policy(&settings);

    let game_path = PathBuf::from(&game_folder);
    std::fs::create_dir_all(&game_path)
        .map_err(|e| format!("Failed to create game folder: {}", e))?;

    let langs = languages.unwrap_or_default();

    let result = async {
        // Snowbreak 分支
        if snowbreak::is_snowbreak_api(&launcher_api) {
            snowbreak_download::download_or_update_game(
                app,
                &game_path,
                snowbreak_policy,
                cancel_token.clone(),
            )
            .await?;
            info!("Snowbreak download completed for {}", game_folder);
            return Ok(());
        }

        // HoYoverse 分支
        if hoyoverse::is_hoyoverse_api(&launcher_api) {
            let biz = require_hoyoverse_biz_prefix(biz_prefix.as_deref())?;
            let game_pkg = hoyoverse::fetch_game_packages(&launcher_api, biz).await?;
            hoyoverse_download::download_game(
                app,
                &game_pkg,
                &game_path,
                &langs,
                cancel_token.clone(),
            )
            .await?;
            write_local_version(&game_path, &game_pkg.main.major.version)?;
            info!("HoYoverse full download completed for {}", game_folder);
            return Ok(());
        }

        // Kuro Games 分支
        let launcher_info = cdn::fetch_launcher_info(&launcher_api).await?;
        let resource_index =
            cdn::fetch_resource_index(&launcher_info.cdn_url, &launcher_info.index_file_url)
                .await?;

        full_download::download_game(
            app,
            &launcher_info,
            &resource_index,
            &game_path,
            cancel_token.clone(),
        )
        .await?;

        write_local_version(&game_path, &launcher_info.version)?;
        info!("Full download completed for {}", game_folder);
        Ok(())
    }
    .await;

    cleanup_cancel_token(&game_folder);
    result
}

#[tauri::command]
pub async fn update_game(
    app: AppHandle,
    settings: State<'_, StdMutex<AppConfig>>,
    launcher_api: String,
    game_folder: String,
    languages: Option<Vec<String>>,
    biz_prefix: Option<String>,
) -> Result<(), String> {
    let cancel_token = get_cancel_token(&game_folder);
    let snowbreak_policy = get_snowbreak_policy(&settings);

    let game_path = PathBuf::from(&game_folder);
    let langs = languages.unwrap_or_default();

    let result = async {
        // Snowbreak 分支（下载和更新是同一个流程）
        if snowbreak::is_snowbreak_api(&launcher_api) {
            snowbreak_download::download_or_update_game(
                app,
                &game_path,
                snowbreak_policy,
                cancel_token.clone(),
            )
            .await?;
            info!("Snowbreak update completed for {}", game_folder);
            return Ok(());
        }

        // HoYoverse 分支
        if hoyoverse::is_hoyoverse_api(&launcher_api) {
            let biz = require_hoyoverse_biz_prefix(biz_prefix.as_deref())?;
            let game_pkg = hoyoverse::fetch_game_packages(&launcher_api, biz).await?;
            let local_version = get_local_version_internal(&game_path)
                .ok_or("未找到本地版本，请使用全量下载".to_string())?;
            hoyoverse_download::update_game(
                app,
                &game_pkg,
                &local_version,
                &game_path,
                &langs,
                cancel_token.clone(),
            )
            .await?;
            write_local_version(&game_path, &game_pkg.main.major.version)?;
            info!("HoYoverse update completed for {}", game_folder);
            return Ok(());
        }

        // Kuro Games 分支
        let launcher_info = cdn::fetch_launcher_info(&launcher_api).await?;
        let resource_index =
            cdn::fetch_resource_index(&launcher_info.cdn_url, &launcher_info.index_file_url)
                .await?;

        incremental::update_game_full(
            app,
            &launcher_info,
            &resource_index,
            &game_path,
            cancel_token.clone(),
        )
        .await?;

        write_local_version(&game_path, &launcher_info.version)?;
        info!("Full comparison update completed for {}", game_folder);
        Ok(())
    }
    .await;

    cleanup_cancel_token(&game_folder);
    result
}

#[tauri::command]
pub async fn update_game_patch(
    app: AppHandle,
    launcher_api: String,
    game_folder: String,
) -> Result<(), String> {
    let cancel_token = get_cancel_token(&game_folder);

    let game_path = PathBuf::from(&game_folder);
    let local_version = get_local_version_internal(&game_path)
        .ok_or("No local version found, cannot do incremental update")?;

    let launcher_info = cdn::fetch_launcher_info(&launcher_api).await?;

    let result = incremental::update_game_patch(
        app,
        &launcher_info,
        &local_version,
        &game_path,
        cancel_token.clone(),
    )
    .await;

    cleanup_cancel_token(&game_folder);

    result?;
    write_local_version(&game_path, &launcher_info.version)?;
    info!("Incremental patch update completed for {}", game_folder);
    Ok(())
}

#[tauri::command]
pub async fn verify_game_files(
    app: AppHandle,
    settings: State<'_, StdMutex<AppConfig>>,
    launcher_api: String,
    game_folder: String,
    biz_prefix: Option<String>,
) -> Result<verifier::VerifyResult, String> {
    let cancel_token = get_cancel_token(&game_folder);
    let snowbreak_policy = get_snowbreak_policy(&settings);

    let game_path = PathBuf::from(&game_folder);

    let result = async {
        // Snowbreak 分支
        if snowbreak::is_snowbreak_api(&launcher_api) {
            return snowbreak_download::verify_game(
                app,
                &game_path,
                snowbreak_policy,
                cancel_token.clone(),
            )
            .await;
        }

        // HoYoverse 分支
        if hoyoverse::is_hoyoverse_api(&launcher_api) {
            let biz = require_hoyoverse_biz_prefix(biz_prefix.as_deref())?;
            let game_pkg = hoyoverse::fetch_game_packages(&launcher_api, biz).await?;
            return hoyoverse_download::verify_game(
                app,
                &game_pkg,
                &game_path,
                cancel_token.clone(),
            )
            .await;
        }

        // Kuro Games 分支
        let launcher_info = cdn::fetch_launcher_info(&launcher_api).await?;
        let resource_index =
            cdn::fetch_resource_index(&launcher_info.cdn_url, &launcher_info.index_file_url)
                .await?;

        let result = verifier::verify_game_files(
            app,
            &launcher_info,
            &resource_index,
            &game_path,
            cancel_token.clone(),
        )
        .await?;

        if result.failed.is_empty() {
            write_local_version(&game_path, &launcher_info.version)?;
        } else {
            tracing::warn!(
                "Verification finished with {} failed files; local version will not be updated",
                result.failed.len()
            );
        }

        Ok(result)
    }
    .await;

    cleanup_cancel_token(&game_folder);
    result
}

#[tauri::command]
pub async fn cancel_download(game_folder: Option<String>) -> Result<(), String> {
    // 先从 StdMutex 中 clone 出需要的 token（不跨 await 持有 guard）
    let targets: Vec<(String, Arc<AsyncMutex<bool>>)> = {
        let tokens = CANCEL_TOKENS.lock().unwrap();
        if let Some(folder) = &game_folder {
            tokens
                .get(folder)
                .map(|t| vec![(folder.clone(), t.clone())])
                .unwrap_or_default()
        } else {
            tokens.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        }
    };
    // 异步设置取消标志
    for (id, token) in targets {
        *token.lock().await = true;
        info!("Download cancellation requested for: {}", id);
    }
    Ok(())
}

#[tauri::command]
pub fn get_local_version(game_folder: String) -> Result<Option<String>, String> {
    Ok(get_local_version_internal(&PathBuf::from(game_folder)))
}

fn get_local_version_internal(game_folder: &Path) -> Option<String> {
    // 兼容不同目录结构：先看当前目录，再向上探测几级父目录。
    for probe in version_probe_dirs(game_folder, 3) {
        if let Some(version) = read_local_version_from_dir(&probe) {
            return Some(version);
        }
    }
    None
}

fn version_probe_dirs(game_folder: &Path, max_parent_depth: usize) -> Vec<PathBuf> {
    let mut probes = Vec::new();
    let mut current = Some(game_folder.to_path_buf());

    for _ in 0..=max_parent_depth {
        let Some(path) = current else { break };
        if !probes.iter().any(|p: &PathBuf| p == &path) {
            probes.push(path.clone());
        }
        current = path.parent().map(|p| p.to_path_buf());
    }

    probes
}

fn read_local_version_from_dir(game_folder: &Path) -> Option<String> {
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
    data.get("version")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn read_non_empty_string(v: Option<&serde_json::Value>) -> Option<String> {
    v.and_then(|x| x.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}

fn build_launcher_api_from_config(
    game_preset: &str,
    config: &serde_json::Value,
) -> Option<serde_json::Value> {
    let root = config.as_object();
    let other = config.get("other").and_then(|v| v.as_object());

    let launcher_api = read_non_empty_string(
        other
            .and_then(|m| m.get("launcherApi"))
            .or_else(|| root.and_then(|m| m.get("launcherApi"))),
    );

    let launcher_download_api = read_non_empty_string(
        other
            .and_then(|m| m.get("launcherDownloadApi"))
            .or_else(|| root.and_then(|m| m.get("launcherDownloadApi"))),
    );

    let mut servers = Vec::new();
    if let Some(server_list) = other
        .and_then(|m| m.get("downloadServers"))
        .or_else(|| root.and_then(|m| m.get("downloadServers")))
        .and_then(|v| v.as_array())
    {
        for (idx, item) in server_list.iter().enumerate() {
            let Some(obj) = item.as_object() else {
                continue;
            };
            let api = read_non_empty_string(obj.get("launcherApi"));
            let Some(api) = api else {
                continue;
            };
            let id = read_non_empty_string(obj.get("id")).unwrap_or_else(|| {
                if idx == 0 {
                    "custom".to_string()
                } else {
                    format!("custom-{}", idx + 1)
                }
            });
            let label = read_non_empty_string(obj.get("label")).unwrap_or_else(|| "自定义".to_string());
            let biz_prefix = read_non_empty_string(obj.get("bizPrefix")).unwrap_or_default();
            servers.push(serde_json::json!({
                "id": id,
                "label": label,
                "launcherApi": api,
                "bizPrefix": biz_prefix,
            }));
        }
    }

    if servers.is_empty() {
        if let Some(api) = launcher_api.as_ref() {
            servers.push(serde_json::json!({
                "id": "custom",
                "label": "自定义",
                "launcherApi": api,
                "bizPrefix": "",
            }));
        }
    }

    if servers.is_empty() {
        return None;
    }

    let default_folder = read_non_empty_string(
        other
            .and_then(|m| m.get("defaultFolder"))
            .or_else(|| root.and_then(|m| m.get("defaultFolder"))),
    )
    .unwrap_or_else(|| game_preset.to_string());

    let mut result = serde_json::json!({
        "supported": true,
        "defaultFolder": default_folder,
        "servers": servers,
        "audioLanguages": other
            .and_then(|m| m.get("audioLanguages"))
            .or_else(|| root.and_then(|m| m.get("audioLanguages")))
            .cloned()
            .unwrap_or_else(|| serde_json::json!([])),
    });
    if let Some(api) = launcher_api {
        result["launcherApi"] = serde_json::Value::String(api);
    }
    if let Some(api) = launcher_download_api {
        result["launcherDownloadApi"] = serde_json::Value::String(api);
    }

    Some(result)
}

fn read_launcher_api_override_from_game_config(game_preset: &str) -> Option<serde_json::Value> {
    let config_json = crate::configs::database::get_game_config(game_preset)?;
    let config: serde_json::Value = serde_json::from_str(&config_json).ok()?;
    build_launcher_api_from_config(game_preset, &config)
}

/// 根据游戏预设返回对应的 launcher API URL（预设默认值 + 用户配置覆盖）
#[tauri::command]
pub fn get_game_launcher_api(game_preset: String) -> Result<serde_json::Value, String> {
    use crate::configs::game_presets;
    let game_preset = crate::configs::game_identity::to_canonical_or_keep(&game_preset);
    let override_obj = read_launcher_api_override_from_game_config(&game_preset);

    let Some(preset) = game_presets::get_preset(&game_preset) else {
        return Ok(override_obj.unwrap_or_else(|| serde_json::json!({ "supported": false })));
    };

    let mut obj = serde_json::json!({
        "supported": preset.supported,
        "defaultFolder": preset.default_folder,
        "servers": preset.download_servers,
        "audioLanguages": preset.audio_languages,
    });

    if let Some(ref api) = preset.launcher_api {
        obj["launcherApi"] = serde_json::Value::String(api.clone());
    }
    if let Some(ref api) = preset.launcher_download_api {
        obj["launcherDownloadApi"] = serde_json::Value::String(api.clone());
    }

    if let Some(override_value) = override_obj {
        if let Some(override_map) = override_value.as_object() {
            for key in [
                "supported",
                "defaultFolder",
                "servers",
                "audioLanguages",
                "launcherApi",
                "launcherDownloadApi",
            ] {
                if let Some(value) = override_map.get(key) {
                    obj[key] = value.clone();
                }
            }
        }
    }

    Ok(obj)
}

/// 返回游戏默认安装目录（自动跟随软件数据目录 dataDir）
#[tauri::command]
pub fn get_default_game_folder(game_name: String) -> Result<String, String> {
    let game_name = crate::configs::game_identity::to_canonical_or_keep(&game_name);
    let game_dir = crate::utils::file_manager::get_global_games_dir().join(&game_name);
    Ok(game_dir.to_string_lossy().to_string())
}

/// Snowbreak 游戏状态检测
async fn get_game_state_snowbreak(
    game_path: &Path,
    source_policy: snowbreak::SourcePolicy,
) -> Result<GameState, String> {
    let local_version = snowbreak::read_local_version(game_path);

    let (remote_manifest, _cdn) = match snowbreak::fetch_manifest_with_policy(source_policy).await {
        Ok(m) => m,
        Err(e) => {
            tracing::error!("Snowbreak API 失败: {}", e);
            return Ok(GameState {
                state: LauncherState::NetworkError,
                local_version,
                remote_version: None,
                supports_incremental: false,
            });
        }
    };

    let remote_version = remote_manifest.version.clone();

    let state = if local_version.is_none() {
        LauncherState::NeedInstall
    } else if local_version.as_deref() != Some(&remote_version) {
        LauncherState::NeedUpdate
    } else {
        LauncherState::StartGame
    };

    Ok(GameState {
        state,
        local_version,
        remote_version: Some(remote_version),
        supports_incremental: false,
    })
}

fn get_snowbreak_policy(settings: &State<'_, StdMutex<AppConfig>>) -> snowbreak::SourcePolicy {
    settings
        .lock()
        .ok()
        .map(|cfg| snowbreak::SourcePolicy::from_str(&cfg.snowbreak_source_policy))
        .unwrap_or(snowbreak::SourcePolicy::OfficialFirst)
}

fn require_hoyoverse_biz_prefix<'a>(biz_prefix: Option<&'a str>) -> Result<&'a str, String> {
    let Some(raw) = biz_prefix else {
        return Err("HoYoverse 下载缺少 biz_prefix，请在游戏预设服务器配置中提供".to_string());
    };
    let biz = raw.trim();
    if biz.is_empty() {
        return Err("HoYoverse 下载缺少 biz_prefix，请在游戏预设服务器配置中提供".to_string());
    }
    Ok(biz)
}

/// HoYoverse 游戏状态检测
async fn get_game_state_hoyoverse(
    launcher_api: &str,
    game_path: &Path,
    biz: &str,
) -> Result<GameState, String> {
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

fn write_local_version(game_folder: &Path, version: &str) -> Result<(), String> {
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
