use crate::configs::app_config::AppConfig;
use crate::downloader::cdn::{self, LauncherInfo};
use crate::downloader::hoyoverse;
use crate::downloader::progress::{DownloadProgress, LauncherState, SpeedTracker};
use crate::downloader::snowbreak;
use crate::downloader::{
    full_download, hoyoverse_download, incremental, snowbreak_download, verifier,
};
use crate::process_monitor;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};
use tauri::{AppHandle, Emitter, State};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherInstallerState {
    pub state: LauncherState,
    pub local_version: Option<String>,
    pub remote_version: Option<String>,
    pub supports_incremental: bool,
    pub installer_path: Option<String>,
    pub installer_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherInstallerDownloadResult {
    pub installer_path: String,
    pub installer_url: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DownloadSourceMeta {
    source_api: String,
    #[serde(default)]
    biz_prefix: Option<String>,
    updated_at: String,
}

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
    let source_biz_prefix = biz_prefix
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string);
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
                local_version: get_local_version_for_source(
                    &game_path,
                    &launcher_api,
                    source_biz_prefix.as_deref(),
                    false,
                ),
                remote_version: None,
                supports_incremental: false,
            });
        }
    };

    let local_version = get_local_version_for_source(
        &game_path,
        &launcher_api,
        source_biz_prefix.as_deref(),
        true,
    );
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
pub async fn get_launcher_installer_state(
    launcher_api: String,
    game_folder: String,
    game_preset: String,
) -> Result<LauncherInstallerState, String> {
    let canonical_preset = crate::configs::game_identity::to_canonical_or_keep(&game_preset);
    let game_path = PathBuf::from(&game_folder);
    let installer_path = installer_path_for_preset(&game_path, &canonical_preset);
    let local_meta = read_launcher_installer_meta(&game_path);
    let (local_version, local_installer_exists) =
        resolve_local_installer_for_source(local_meta.as_ref(), &installer_path, &launcher_api);

    let remote_info = fetch_launcher_installer_remote(&launcher_api).await;
    let remote_version = remote_info.as_ref().ok().map(|r| r.version.clone());
    let installer_url = remote_info.as_ref().ok().map(|r| r.installer_url.clone());

    let state = match remote_info {
        Ok(ref remote) => determine_launcher_installer_state(
            local_version.as_deref(),
            Some(remote.version.as_str()),
            local_installer_exists,
            false,
        ),
        Err(ref err) => {
            tracing::error!("fetch launcher installer info failed: {}", err);
            determine_launcher_installer_state(
                local_version.as_deref(),
                None,
                local_installer_exists,
                true,
            )
        }
    };

    Ok(LauncherInstallerState {
        state,
        local_version,
        remote_version,
        supports_incremental: false,
        installer_path: Some(installer_path.to_string_lossy().to_string()),
        installer_url,
    })
}

#[tauri::command]
pub async fn download_launcher_installer(
    app: AppHandle,
    launcher_api: String,
    game_folder: String,
    game_preset: String,
) -> Result<LauncherInstallerDownloadResult, String> {
    let cancel_token = get_cancel_token(&game_folder);
    let game_path = PathBuf::from(&game_folder);
    let region_scope = process_monitor::derive_region_scope(Some(&launcher_api), None, None);
    let _write_guard = process_monitor::acquire_game_write_guard(
        &game_path,
        &region_scope,
        "download_launcher_installer",
    )?;
    let canonical_preset = crate::configs::game_identity::to_canonical_or_keep(&game_preset);
    let result = download_launcher_installer_internal(
        app,
        launcher_api,
        game_path,
        canonical_preset,
        cancel_token.clone(),
    )
    .await;

    cleanup_cancel_token(&game_folder);
    result
}

#[tauri::command]
pub async fn update_launcher_installer(
    app: AppHandle,
    launcher_api: String,
    game_folder: String,
    game_preset: String,
) -> Result<LauncherInstallerDownloadResult, String> {
    download_launcher_installer(app, launcher_api, game_folder, game_preset).await
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
    let region_scope =
        process_monitor::derive_region_scope(Some(&launcher_api), biz_prefix.as_deref(), None);
    let _write_guard =
        process_monitor::acquire_game_write_guard(&game_path, &region_scope, "download_game")?;
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
            write_download_source_meta(&game_path, &launcher_api, Some(biz))?;
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
        write_download_source_meta(&game_path, &launcher_api, None)?;
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
    let region_scope =
        process_monitor::derive_region_scope(Some(&launcher_api), biz_prefix.as_deref(), None);
    let _write_guard =
        process_monitor::acquire_game_write_guard(&game_path, &region_scope, "update_game")?;
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
            write_download_source_meta(&game_path, &launcher_api, Some(biz))?;
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
        write_download_source_meta(&game_path, &launcher_api, None)?;
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
    let region_scope = process_monitor::derive_region_scope(Some(&launcher_api), None, None);
    let _write_guard =
        process_monitor::acquire_game_write_guard(&game_path, &region_scope, "update_game_patch")?;
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
    write_download_source_meta(&game_path, &launcher_api, None)?;
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
    let region_scope =
        process_monitor::derive_region_scope(Some(&launcher_api), biz_prefix.as_deref(), None);
    let _write_guard =
        process_monitor::acquire_game_write_guard(&game_path, &region_scope, "verify_game_files")?;

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
            write_download_source_meta(&game_path, &launcher_api, biz_prefix.as_deref())?;
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

fn download_source_meta_path(game_folder: &Path) -> PathBuf {
    game_folder.join(".download_source_meta.json")
}

fn read_download_source_meta(game_folder: &Path) -> Option<DownloadSourceMeta> {
    let path = download_source_meta_path(game_folder);
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<DownloadSourceMeta>(&content).ok()
}

fn write_download_source_meta(
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
        biz_prefix: biz_prefix
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToString::to_string),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };
    let content = serde_json::to_string_pretty(&meta)
        .map_err(|e| format!("Failed to serialize download source meta: {}", e))?;
    std::fs::write(download_source_meta_path(game_folder), content)
        .map_err(|e| format!("Failed to write download source meta: {}", e))
}

fn is_same_download_source(
    meta: &DownloadSourceMeta,
    launcher_api: &str,
    biz_prefix: Option<&str>,
) -> bool {
    if meta.source_api.trim() != launcher_api.trim() {
        return false;
    }

    let current_biz = biz_prefix
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string);
    let saved_biz = meta
        .biz_prefix
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string);

    match (saved_biz, current_biz) {
        (Some(saved), Some(current)) => saved == current,
        _ => true,
    }
}

fn get_local_version_for_source(
    game_folder: &Path,
    launcher_api: &str,
    biz_prefix: Option<&str>,
    bootstrap_meta_if_missing: bool,
) -> Option<String> {
    let local_version = get_local_version_internal(game_folder)?;
    match read_download_source_meta(game_folder) {
        Some(meta) => {
            if is_same_download_source(&meta, launcher_api, biz_prefix) {
                Some(local_version)
            } else {
                None
            }
        }
        None => {
            if bootstrap_meta_if_missing {
                let _ = write_download_source_meta(game_folder, launcher_api, biz_prefix);
            }
            Some(local_version)
        }
    }
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
    let mut has_explicit_servers = false;
    if let Some(server_list) = other
        .and_then(|m| m.get("downloadServers"))
        .or_else(|| root.and_then(|m| m.get("downloadServers")))
        .and_then(|v| v.as_array())
    {
        has_explicit_servers = true;
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
            let label =
                read_non_empty_string(obj.get("label")).unwrap_or_else(|| "自定义".to_string());
            let biz_prefix = read_non_empty_string(obj.get("bizPrefix")).unwrap_or_default();
            servers.push(serde_json::json!({
                "id": id,
                "label": label,
                "launcherApi": api,
                "bizPrefix": biz_prefix,
            }));
        }
    }

    if has_explicit_servers && servers.is_empty() {
        if let Some(api) = launcher_api.as_ref() {
            servers.push(serde_json::json!({
                "id": "custom",
                "label": "自定义",
                "launcherApi": api,
                "bizPrefix": "",
            }));
        }
    }

    if has_explicit_servers && servers.is_empty() {
        has_explicit_servers = false;
    }

    let has_default_folder_override = other
        .and_then(|m| m.get("defaultFolder"))
        .or_else(|| root.and_then(|m| m.get("defaultFolder")))
        .is_some();
    let has_audio_languages_override = other
        .and_then(|m| m.get("audioLanguages"))
        .or_else(|| root.and_then(|m| m.get("audioLanguages")))
        .is_some();
    let default_folder = read_non_empty_string(
        other
            .and_then(|m| m.get("defaultFolder"))
            .or_else(|| root.and_then(|m| m.get("defaultFolder"))),
    )
    .unwrap_or_else(|| game_preset.to_string());
    let download_mode = read_non_empty_string(
        other
            .and_then(|m| m.get("downloadMode"))
            .or_else(|| root.and_then(|m| m.get("downloadMode"))),
    );
    if !has_explicit_servers
        && launcher_api.is_none()
        && launcher_download_api.is_none()
        && download_mode.is_none()
        && !has_default_folder_override
        && !has_audio_languages_override
    {
        return None;
    }

    let mut result = serde_json::json!({
        "supported": true,
        "defaultFolder": default_folder,
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
    if let Some(mode) = download_mode {
        result["downloadMode"] = serde_json::Value::String(mode);
    }
    if has_explicit_servers {
        result["servers"] = serde_json::Value::Array(servers);
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
        "downloadMode": preset.download_mode,
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
                "downloadMode",
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

#[tauri::command]
pub fn resolve_downloaded_game_executable(
    game_name: String,
    game_folder: String,
    launcher_api: Option<String>,
) -> Result<Option<String>, String> {
    let game_preset = crate::configs::game_identity::to_canonical_or_keep(&game_name);
    let folder = game_folder.trim();
    if folder.is_empty() {
        return Ok(None);
    }
    let root = PathBuf::from(folder);
    if !root.exists() || !root.is_dir() {
        return Ok(None);
    }
    if !supports_auto_exe_detection(&game_preset) {
        return Ok(None);
    }

    if let Some(path) = resolve_known_game_executable(&game_preset, &root, launcher_api.as_deref())
    {
        return Ok(Some(path.to_string_lossy().to_string()));
    }

    Ok(resolve_best_executable_by_scan(&game_preset, &root)
        .map(|p| p.to_string_lossy().to_string()))
}

fn supports_auto_exe_detection(game_preset: &str) -> bool {
    matches!(
        game_preset,
        "HonkaiStarRail"
            | "GenshinImpact"
            | "ZenlessZoneZero"
            | "WutheringWaves"
            | "SnowbreakContainmentZone"
    )
}

fn resolve_known_game_executable(
    game_preset: &str,
    game_root: &Path,
    launcher_api: Option<&str>,
) -> Option<PathBuf> {
    let mut candidates: Vec<String> = Vec::new();
    match game_preset {
        "HonkaiStarRail" => candidates.push("StarRail.exe".to_string()),
        "GenshinImpact" => {
            let is_cn = launcher_api
                .map(|api| api.contains("mihoyo.com"))
                .unwrap_or(false);
            if is_cn {
                candidates.push("YuanShen.exe".to_string());
                candidates.push("GenshinImpact.exe".to_string());
            } else {
                candidates.push("GenshinImpact.exe".to_string());
                candidates.push("YuanShen.exe".to_string());
            }
        }
        "ZenlessZoneZero" => candidates.push("ZenlessZoneZero.exe".to_string()),
        "WutheringWaves" => {
            candidates.push("Wuthering Waves.exe".to_string());
            candidates.push("Client/Binaries/Win64/Client-Win64-Shipping.exe".to_string());
        }
        "SnowbreakContainmentZone" => {
            candidates.push("Snowbreak.exe".to_string());
            candidates.push("X6Game.exe".to_string());
            candidates.push("X6Game/Binaries/Win64/X6Game-Win64-Shipping.exe".to_string());
            candidates.push("Game/Binaries/Win64/Game-Win64-Shipping.exe".to_string());
        }
        _ => {}
    }

    for rel in candidates {
        let path = game_root.join(rel);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

fn resolve_best_executable_by_scan(game_preset: &str, game_root: &Path) -> Option<PathBuf> {
    let candidates = collect_exe_files(game_root, 7, 20_000);
    let mut best: Option<(i32, usize, PathBuf)> = None;

    for path in candidates {
        let score = score_executable_candidate(game_preset, game_root, &path);
        if score < 10 {
            continue;
        }
        let depth = relative_depth(game_root, &path);
        match &best {
            None => best = Some((score, depth, path)),
            Some((best_score, best_depth, _)) => {
                if score > *best_score || (score == *best_score && depth < *best_depth) {
                    best = Some((score, depth, path));
                }
            }
        }
    }

    best.map(|(_, _, path)| path)
}

fn collect_exe_files(root: &Path, max_depth: usize, max_entries: usize) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let mut stack: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];
    let mut visited: usize = 0;

    while let Some((dir, depth)) = stack.pop() {
        if visited >= max_entries {
            break;
        }
        let Ok(read_dir) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in read_dir.flatten() {
            if visited >= max_entries {
                break;
            }
            visited += 1;
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if file_type.is_dir() {
                if depth < max_depth {
                    stack.push((path, depth + 1));
                }
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            if path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("exe"))
                .unwrap_or(false)
            {
                result.push(path);
            }
        }
    }

    result
}

fn relative_depth(root: &Path, path: &Path) -> usize {
    path.strip_prefix(root)
        .ok()
        .map(|p| p.components().count())
        .unwrap_or(usize::MAX)
}

fn score_executable_candidate(game_preset: &str, root: &Path, path: &Path) -> i32 {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let rel = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();

    let blocked_tokens = [
        "launcher",
        "uninstall",
        "unins",
        "updater",
        "repair",
        "crashreport",
        "crashpad",
        "cef",
        "elevate",
        "easyanticheat",
        "eac",
        "vc_redist",
        "dxsetup",
    ];
    if blocked_tokens
        .iter()
        .any(|token| file_name.contains(token) || rel.contains(token))
    {
        return -1000;
    }

    let mut score = 0;

    if file_name.ends_with("-win64-shipping.exe") {
        score += 55;
    }
    if rel.contains("/binaries/win64/") {
        score += 25;
    }
    if rel.contains("/engine/") || rel.contains("/thirdparty/") {
        score -= 25;
    }
    if relative_depth(root, path) <= 2 {
        score += 10;
    }

    let keywords: &[&str] = match game_preset {
        "WutheringWaves" => &["wuthering", "client-win64-shipping"],
        "SnowbreakContainmentZone" => &["snowbreak", "x6game", "shipping"],
        "HonkaiStarRail" => &["starrail"],
        "GenshinImpact" => &["yuanshen", "genshinimpact"],
        "ZenlessZoneZero" => &["zenlesszonezero", "zenless"],
        _ => &[],
    };
    for kw in keywords {
        if rel.contains(kw) || file_name.contains(kw) {
            score += 30;
        }
    }

    if let Ok(meta) = std::fs::metadata(path) {
        let size = meta.len();
        if size < 300 * 1024 {
            score -= 20;
        }
        if size >= 20 * 1024 * 1024 {
            score += 8;
        }
        if size >= 80 * 1024 * 1024 {
            score += 8;
        }
    }

    score
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
                local_version: get_local_version_for_source(
                    game_path,
                    launcher_api,
                    Some(biz),
                    false,
                ),
                remote_version: None,
                supports_incremental: false,
            });
        }
    };

    let remote_version = game_pkg.main.major.version.clone();
    let local_version = get_local_version_for_source(game_path, launcher_api, Some(biz), true);

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

async fn download_launcher_installer_internal(
    app: AppHandle,
    launcher_api: String,
    game_path: PathBuf,
    game_preset: String,
    cancel_token: Arc<AsyncMutex<bool>>,
) -> Result<LauncherInstallerDownloadResult, String> {
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
        if *cancel_token.lock().await {
            drop(output);
            let _ = std::fs::remove_file(&temp_path);
            return Err("download cancelled".to_string());
        }
        let chunk = chunk_result.map_err(|e| format!("Failed to read installer stream: {}", e))?;
        output
            .write_all(&chunk)
            .map_err(|e| format!("Failed to write installer file: {}", e))?;
        downloaded += chunk.len() as u64;
        speed.record(chunk.len() as u64);

        let total_for_progress = if total_size > 0 {
            total_size
        } else {
            downloaded
        };
        let remaining = total_for_progress.saturating_sub(downloaded);
        let progress = DownloadProgress {
            phase: "download".to_string(),
            total_size: total_for_progress,
            finished_size: downloaded,
            total_count: 1,
            finished_count: usize::from(total_for_progress > 0 && downloaded >= total_for_progress),
            current_file: remote_name.clone(),
            speed_bps: speed.speed_bps(),
            eta_seconds: speed.eta_seconds(remaining),
        };
        app.emit("game-download-progress", &progress).ok();
    }

    output
        .flush()
        .map_err(|e| format!("Failed to flush installer file: {}", e))?;
    drop(output);

    if *cancel_token.lock().await {
        let _ = std::fs::remove_file(&temp_path);
        return Err("download cancelled".to_string());
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn full_game_local_version_is_scoped_by_source_meta() {
        let dir = std::env::temp_dir().join(format!(
            "ssmt4-full-source-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        write_local_version(&dir, "3.1.0").expect("write local version");
        write_download_source_meta(&dir, "https://example.com/cn", None).expect("write source");

        assert_eq!(
            get_local_version_for_source(&dir, "https://example.com/cn", None, false).as_deref(),
            Some("3.1.0")
        );
        assert_eq!(
            get_local_version_for_source(&dir, "https://example.com/global", None, false),
            None
        );

        let _ = std::fs::remove_file(dir.join("launcherDownloadConfig.json"));
        let _ = std::fs::remove_file(download_source_meta_path(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn full_game_local_version_bootstraps_source_meta_when_missing() {
        let dir = std::env::temp_dir().join(format!(
            "ssmt4-full-source-bootstrap-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        write_local_version(&dir, "3.1.0").expect("write local version");

        assert_eq!(
            get_local_version_for_source(&dir, "https://example.com/cn", None, true).as_deref(),
            Some("3.1.0")
        );
        let meta = read_download_source_meta(&dir).expect("read source meta");
        assert_eq!(meta.source_api, "https://example.com/cn");
        assert_eq!(
            get_local_version_for_source(&dir, "https://example.com/global", None, false),
            None
        );

        let _ = std::fs::remove_file(dir.join("launcherDownloadConfig.json"));
        let _ = std::fs::remove_file(download_source_meta_path(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn launcher_api_override_with_only_launcher_api_keeps_preset_server_list() {
        let cfg = serde_json::json!({
            "other": {
                "launcherApi": "https://example.com/custom"
            }
        });
        let override_obj = build_launcher_api_from_config("Arknights", &cfg).expect("override");
        assert_eq!(
            override_obj.get("launcherApi").and_then(|v| v.as_str()),
            Some("https://example.com/custom")
        );
        assert!(
            override_obj.get("servers").is_none(),
            "servers should not be overridden when config only has launcherApi"
        );
    }

    #[test]
    fn launcher_api_override_with_explicit_servers_replaces_server_list() {
        let cfg = serde_json::json!({
            "other": {
                "downloadServers": [
                    {
                        "id": "custom",
                        "label": "自定义",
                        "launcherApi": "https://example.com/custom",
                        "bizPrefix": ""
                    }
                ]
            }
        });
        let override_obj = build_launcher_api_from_config("Arknights", &cfg).expect("override");
        let servers = override_obj
            .get("servers")
            .and_then(|v| v.as_array())
            .expect("servers array");
        assert_eq!(servers.len(), 1);
        assert_eq!(
            servers[0].get("launcherApi").and_then(|v| v.as_str()),
            Some("https://example.com/custom")
        );
    }
}
