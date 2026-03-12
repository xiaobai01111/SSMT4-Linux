mod backend;
mod cancel;
mod installer;
mod launcher;
mod source;

use self::backend::{
    BaseTaskContext, ContentTaskContext, GameDownloadBackend, PatchUpdateRequest,
    RepairTaskContext, VerifyTaskContext,
};
use self::cancel::{cleanup_cancel_token, get_cancel_token, request_cancel};
use self::installer::InstallerDownloadRequest;
use crate::configs::app_config::AppConfig;
use crate::downloader::cdn::{self, LauncherInfo};
use crate::downloader::progress::LauncherState;
use crate::downloader::verifier;
use crate::process_monitor;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};
use tauri::{AppHandle, State};
use tokio::sync::Mutex as AsyncMutex;
use tracing::info;

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

async fn run_with_cancel_token<T, F, Fut>(task_id: &str, run: F) -> Result<T, String>
where
    F: FnOnce(Arc<AsyncMutex<bool>>) -> Fut,
    Fut: std::future::Future<Output = Result<T, String>>,
{
    let cancel_token = get_cancel_token(task_id);
    let result = run(cancel_token).await;
    cleanup_cancel_token(task_id);
    result
}

fn derive_download_region_scope(launcher_api: &str, biz_prefix: Option<&str>) -> String {
    process_monitor::derive_region_scope(Some(launcher_api), biz_prefix, None)
}

fn acquire_download_write_guard(
    game_path: &Path,
    launcher_api: &str,
    biz_prefix: Option<&str>,
    operation: &str,
) -> Result<process_monitor::GameWriteGuard, String> {
    let region_scope = derive_download_region_scope(launcher_api, biz_prefix);
    process_monitor::acquire_game_write_guard(game_path, &region_scope, operation)
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
    let backend = GameDownloadBackend::resolve(&launcher_api, biz_prefix.as_deref())?;
    let snowbreak_policy = backend::get_snowbreak_policy(settings.inner());
    let game_path = PathBuf::from(game_folder);
    backend::get_game_state(&backend, snowbreak_policy, &launcher_api, &game_path).await
}

#[tauri::command]
pub async fn get_launcher_installer_state(
    launcher_api: String,
    game_folder: String,
    game_preset: String,
) -> Result<LauncherInstallerState, String> {
    let canonical_preset = crate::configs::game_identity::to_canonical_or_keep(&game_preset);
    let game_path = PathBuf::from(game_folder);
    installer::get_launcher_installer_state(&launcher_api, &game_path, &canonical_preset).await
}

#[tauri::command]
pub async fn download_launcher_installer(
    app: AppHandle,
    launcher_api: String,
    game_folder: String,
    game_preset: String,
    task_id: String,
) -> Result<LauncherInstallerDownloadResult, String> {
    let task_id_for_cleanup = task_id.clone();
    run_with_cancel_token(&task_id_for_cleanup, move |cancel_token| async move {
        let game_path = PathBuf::from(&game_folder);
        let _write_guard = acquire_download_write_guard(
            &game_path,
            &launcher_api,
            None,
            "download_launcher_installer",
        )?;
        let canonical_preset = crate::configs::game_identity::to_canonical_or_keep(&game_preset);

        installer::download_launcher_installer(InstallerDownloadRequest {
            app,
            task_id: &task_id,
            operation: crate::events::GameDownloadOperation::DownloadLauncherInstaller,
            launcher_api,
            game_path,
            game_preset: canonical_preset,
            cancel_token,
        })
        .await
    })
    .await
}

#[tauri::command]
pub async fn update_launcher_installer(
    app: AppHandle,
    launcher_api: String,
    game_folder: String,
    game_preset: String,
    task_id: String,
) -> Result<LauncherInstallerDownloadResult, String> {
    let task_id_for_cleanup = task_id.clone();
    run_with_cancel_token(&task_id_for_cleanup, move |cancel_token| async move {
        let game_path = PathBuf::from(&game_folder);
        let _write_guard = acquire_download_write_guard(
            &game_path,
            &launcher_api,
            None,
            "update_launcher_installer",
        )?;
        let canonical_preset = crate::configs::game_identity::to_canonical_or_keep(&game_preset);

        installer::download_launcher_installer(InstallerDownloadRequest {
            app,
            task_id: &task_id,
            operation: crate::events::GameDownloadOperation::UpdateLauncherInstaller,
            launcher_api,
            game_path,
            game_preset: canonical_preset,
            cancel_token,
        })
        .await
    })
    .await
}

#[tauri::command]
pub async fn download_game(
    app: AppHandle,
    settings: State<'_, StdMutex<AppConfig>>,
    launcher_api: String,
    game_folder: String,
    task_id: String,
    languages: Option<Vec<String>>,
    biz_prefix: Option<String>,
) -> Result<(), String> {
    let task_id_for_cleanup = task_id.clone();
    run_with_cancel_token(&task_id_for_cleanup, move |cancel_token| async move {
        let backend = GameDownloadBackend::resolve(&launcher_api, biz_prefix.as_deref())?;
        let snowbreak_policy = backend::get_snowbreak_policy(settings.inner());
        let game_path = PathBuf::from(&game_folder);
        let _write_guard = acquire_download_write_guard(
            &game_path,
            &launcher_api,
            biz_prefix.as_deref(),
            "download_game",
        )?;
        std::fs::create_dir_all(&game_path)
            .map_err(|e| format!("Failed to create game folder: {}", e))?;
        let languages = languages.unwrap_or_default();

        backend::download_game(ContentTaskContext {
            backend: &backend,
            snowbreak_policy,
            base: BaseTaskContext {
                app,
                task_id: &task_id,
                launcher_api: &launcher_api,
                game_path: &game_path,
                game_folder: &game_folder,
                cancel_token,
            },
            languages: &languages,
        })
        .await
    })
    .await
}

#[tauri::command]
pub async fn update_game(
    app: AppHandle,
    settings: State<'_, StdMutex<AppConfig>>,
    launcher_api: String,
    game_folder: String,
    task_id: String,
    languages: Option<Vec<String>>,
    biz_prefix: Option<String>,
) -> Result<(), String> {
    let task_id_for_cleanup = task_id.clone();
    run_with_cancel_token(&task_id_for_cleanup, move |cancel_token| async move {
        let backend = GameDownloadBackend::resolve(&launcher_api, biz_prefix.as_deref())?;
        let snowbreak_policy = backend::get_snowbreak_policy(settings.inner());
        let game_path = PathBuf::from(&game_folder);
        let _write_guard = acquire_download_write_guard(
            &game_path,
            &launcher_api,
            biz_prefix.as_deref(),
            "update_game",
        )?;
        let languages = languages.unwrap_or_default();

        backend::update_game(ContentTaskContext {
            backend: &backend,
            snowbreak_policy,
            base: BaseTaskContext {
                app,
                task_id: &task_id,
                launcher_api: &launcher_api,
                game_path: &game_path,
                game_folder: &game_folder,
                cancel_token,
            },
            languages: &languages,
        })
        .await
    })
    .await
}

#[tauri::command]
pub async fn update_game_patch(
    app: AppHandle,
    launcher_api: String,
    game_folder: String,
    task_id: String,
) -> Result<(), String> {
    let task_id_for_cleanup = task_id.clone();
    run_with_cancel_token(&task_id_for_cleanup, move |cancel_token| async move {
        let game_path = PathBuf::from(&game_folder);
        let _write_guard =
            acquire_download_write_guard(&game_path, &launcher_api, None, "update_game_patch")?;

        backend::update_game_patch(PatchUpdateRequest {
            app,
            task_id: &task_id,
            launcher_api: &launcher_api,
            game_path: &game_path,
            game_folder: &game_folder,
            cancel_token,
        })
        .await
    })
    .await
}

#[tauri::command]
pub async fn verify_game_files(
    app: AppHandle,
    settings: State<'_, StdMutex<AppConfig>>,
    launcher_api: String,
    game_folder: String,
    task_id: String,
    biz_prefix: Option<String>,
) -> Result<verifier::VerifyResult, String> {
    let task_id_for_cleanup = task_id.clone();
    run_with_cancel_token(&task_id_for_cleanup, move |cancel_token| async move {
        let backend = GameDownloadBackend::resolve(&launcher_api, biz_prefix.as_deref())?;
        let snowbreak_policy = backend::get_snowbreak_policy(settings.inner());
        let game_path = PathBuf::from(&game_folder);
        let _write_guard = acquire_download_write_guard(
            &game_path,
            &launcher_api,
            biz_prefix.as_deref(),
            "verify_game_files",
        )?;

        backend::verify_game_files(VerifyTaskContext {
            backend: &backend,
            snowbreak_policy,
            base: BaseTaskContext {
                app,
                task_id: &task_id,
                launcher_api: &launcher_api,
                game_path: &game_path,
                game_folder: &game_folder,
                cancel_token,
            },
        })
        .await
    })
    .await
}

#[tauri::command]
pub async fn repair_game_files(
    app: AppHandle,
    settings: State<'_, StdMutex<AppConfig>>,
    launcher_api: String,
    game_folder: String,
    task_id: String,
    files: Vec<String>,
    biz_prefix: Option<String>,
) -> Result<verifier::RepairResult, String> {
    let task_id_for_cleanup = task_id.clone();
    run_with_cancel_token(&task_id_for_cleanup, move |cancel_token| async move {
        let backend = GameDownloadBackend::resolve(&launcher_api, biz_prefix.as_deref())?;
        let snowbreak_policy = backend::get_snowbreak_policy(settings.inner());
        let game_path = PathBuf::from(&game_folder);
        let _write_guard = acquire_download_write_guard(
            &game_path,
            &launcher_api,
            biz_prefix.as_deref(),
            "repair_game_files",
        )?;

        backend::repair_game_files(RepairTaskContext {
            backend: &backend,
            snowbreak_policy,
            base: BaseTaskContext {
                app,
                task_id: &task_id,
                launcher_api: &launcher_api,
                game_path: &game_path,
                game_folder: &game_folder,
                cancel_token,
            },
            files: &files,
        })
        .await
    })
    .await
}

#[tauri::command]
pub async fn cancel_download(task_id: Option<String>) -> Result<(), String> {
    for cancelled_task_id in request_cancel(task_id.as_deref()).await {
        info!("Download cancellation requested for: {}", cancelled_task_id);
    }
    Ok(())
}

#[tauri::command]
pub fn get_local_version(game_folder: String) -> Result<Option<String>, String> {
    Ok(source::get_local_version_internal(&PathBuf::from(
        game_folder,
    )))
}

#[tauri::command]
pub fn get_game_launcher_api(game_preset: String) -> Result<serde_json::Value, String> {
    launcher::get_game_launcher_api(game_preset)
}

#[tauri::command]
pub fn get_default_game_folder(game_name: String) -> Result<String, String> {
    launcher::get_default_game_folder(game_name)
}

#[tauri::command]
pub fn resolve_downloaded_game_executable(
    game_name: String,
    game_folder: String,
    launcher_api: Option<String>,
) -> Result<Option<String>, String> {
    launcher::resolve_downloaded_game_executable(game_name, game_folder, launcher_api)
}

#[cfg(test)]
mod tests {
    use super::{derive_download_region_scope, request_cancel, run_with_cancel_token};
    use once_cell::sync::Lazy;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_GUARD: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    fn unique_task_id(label: &str) -> String {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        format!("game-downloader-{label}-{nonce}")
    }

    #[tokio::test]
    async fn run_with_cancel_token_cleans_up_token_after_success() {
        let _guard = TEST_GUARD.lock().unwrap();
        let task_id = unique_task_id("success");

        let value = run_with_cancel_token(&task_id, |token| async move {
            assert!(!(*token.lock().await));
            Ok::<_, String>(42_u32)
        })
        .await
        .expect("task should succeed");
        assert_eq!(value, 42);

        let cancelled = request_cancel(Some(&task_id)).await;
        assert!(cancelled.is_empty());
    }

    #[tokio::test]
    async fn run_with_cancel_token_cleans_up_token_after_error() {
        let _guard = TEST_GUARD.lock().unwrap();
        let task_id = unique_task_id("error");

        let result = run_with_cancel_token(&task_id, |token| async move {
            *token.lock().await = true;
            Err::<(), _>("boom".to_string())
        })
        .await;
        assert!(result.is_err());

        let cancelled = request_cancel(Some(&task_id)).await;
        assert!(cancelled.is_empty());
    }

    #[test]
    fn derive_download_region_scope_prefers_biz_prefix_when_present() {
        let scope = derive_download_region_scope(
            "https://launcher.example.com/get_latest_launcher?channel=6",
            Some("hkrpg_global"),
        );
        assert_eq!(scope, "hkrpg_global");
    }
}
