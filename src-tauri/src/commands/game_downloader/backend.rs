use super::source::{
    finalize_file_health_result, get_local_version_for_source, get_local_version_internal,
    persist_local_source_state, require_hoyoverse_biz_prefix,
};
use super::GameState;
use crate::configs::app_config::AppConfig;
use crate::downloader::cdn::{self, LauncherInfo};
use crate::downloader::progress::LauncherState;
use crate::downloader::{
    full_download, hoyoverse_download, incremental, snowbreak_download, verifier,
};
use crate::downloader::{hoyoverse, snowbreak};
use crate::services::runtime_config;
use std::path::Path;
use std::sync::{Arc, Mutex as StdMutex};
use tauri::AppHandle;
use tokio::sync::Mutex as AsyncMutex;
use tracing::{info, warn};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GameDownloadBackend {
    Snowbreak,
    Hoyoverse { biz_prefix: String },
    Kuro { source_biz_prefix: Option<String> },
}

impl GameDownloadBackend {
    pub(crate) fn resolve(launcher_api: &str, biz_prefix: Option<&str>) -> Result<Self, String> {
        if snowbreak::is_snowbreak_api(launcher_api) {
            return Ok(Self::Snowbreak);
        }

        if hoyoverse::is_hoyoverse_api(launcher_api) {
            return Ok(Self::Hoyoverse {
                biz_prefix: require_hoyoverse_biz_prefix(biz_prefix)?.to_string(),
            });
        }

        Ok(Self::Kuro {
            source_biz_prefix: super::source::normalize_biz_prefix(biz_prefix),
        })
    }
}

struct KuroDownloadContext {
    launcher_info: LauncherInfo,
    resource_index: cdn::ResourceIndex,
}

pub(crate) struct BaseTaskContext<'a> {
    pub(crate) app: AppHandle,
    pub(crate) task_id: &'a str,
    pub(crate) launcher_api: &'a str,
    pub(crate) game_path: &'a Path,
    pub(crate) game_folder: &'a str,
    pub(crate) cancel_token: Arc<AsyncMutex<bool>>,
}

pub(crate) struct ContentTaskContext<'a> {
    pub(crate) backend: &'a GameDownloadBackend,
    pub(crate) snowbreak_policy: snowbreak::SourcePolicy,
    pub(crate) base: BaseTaskContext<'a>,
    pub(crate) languages: &'a [String],
}

pub(crate) struct VerifyTaskContext<'a> {
    pub(crate) backend: &'a GameDownloadBackend,
    pub(crate) snowbreak_policy: snowbreak::SourcePolicy,
    pub(crate) base: BaseTaskContext<'a>,
}

pub(crate) struct RepairTaskContext<'a> {
    pub(crate) backend: &'a GameDownloadBackend,
    pub(crate) snowbreak_policy: snowbreak::SourcePolicy,
    pub(crate) base: BaseTaskContext<'a>,
    pub(crate) files: &'a [String],
}

pub(crate) struct PatchUpdateRequest<'a> {
    pub(crate) app: AppHandle,
    pub(crate) task_id: &'a str,
    pub(crate) launcher_api: &'a str,
    pub(crate) game_path: &'a Path,
    pub(crate) game_folder: &'a str,
    pub(crate) cancel_token: Arc<AsyncMutex<bool>>,
}

pub(crate) fn get_snowbreak_policy(settings: &StdMutex<AppConfig>) -> snowbreak::SourcePolicy {
    runtime_config::view(settings, |cfg| {
        snowbreak::SourcePolicy::from_str(&cfg.snowbreak_source_policy)
    })
    .unwrap_or(snowbreak::SourcePolicy::OfficialFirst)
}

fn resolve_launcher_state(local_version: Option<&str>, remote_version: &str) -> LauncherState {
    match local_version {
        None => LauncherState::NeedInstall,
        Some(local) if local != remote_version => LauncherState::NeedUpdate,
        Some(_) => LauncherState::StartGame,
    }
}

fn build_network_error_state(local_version: Option<String>) -> GameState {
    GameState {
        state: LauncherState::NetworkError,
        local_version,
        remote_version: None,
        supports_incremental: false,
    }
}

fn build_resolved_game_state(
    local_version: Option<String>,
    remote_version: String,
    supports_incremental: bool,
) -> GameState {
    let state = resolve_launcher_state(local_version.as_deref(), &remote_version);
    GameState {
        state,
        local_version,
        remote_version: Some(remote_version),
        supports_incremental,
    }
}

fn supports_incremental_for_local<F>(local_version: Option<&str>, mut has_patch: F) -> bool
where
    F: FnMut(&str) -> bool,
{
    match local_version {
        Some(version) => has_patch(version),
        None => false,
    }
}

fn kuro_has_incremental_patch(launcher_info: &LauncherInfo, local_version: Option<&str>) -> bool {
    supports_incremental_for_local(local_version, |version| {
        launcher_info
            .patch_configs
            .iter()
            .any(|patch| patch.version == version)
    })
}

pub(crate) async fn get_game_state(
    backend: &GameDownloadBackend,
    snowbreak_policy: snowbreak::SourcePolicy,
    launcher_api: &str,
    game_path: &Path,
) -> Result<GameState, String> {
    match backend {
        GameDownloadBackend::Snowbreak => {
            get_snowbreak_game_state(game_path, snowbreak_policy).await
        }
        GameDownloadBackend::Hoyoverse { biz_prefix } => {
            get_hoyoverse_game_state(launcher_api, game_path, biz_prefix).await
        }
        GameDownloadBackend::Kuro { source_biz_prefix } => {
            get_kuro_game_state(launcher_api, game_path, source_biz_prefix.as_deref()).await
        }
    }
}

pub(crate) async fn download_game(context: ContentTaskContext<'_>) -> Result<(), String> {
    let ContentTaskContext {
        backend,
        snowbreak_policy,
        base,
        languages,
    } = context;

    match backend {
        GameDownloadBackend::Snowbreak => {
            snowbreak_download::download_or_update_game(
                base.app,
                base.task_id,
                crate::events::GameDownloadOperation::DownloadGame,
                base.game_path,
                snowbreak_policy,
                base.cancel_token,
            )
            .await?;
            info!("Snowbreak download completed for {}", base.game_folder);
            Ok(())
        }
        GameDownloadBackend::Hoyoverse { biz_prefix } => {
            let game_pkg = hoyoverse::fetch_game_packages(base.launcher_api, biz_prefix).await?;
            hoyoverse_download::download_game(
                base.app,
                base.task_id,
                &game_pkg,
                base.game_path,
                languages,
                base.cancel_token,
            )
            .await?;
            persist_local_source_state(
                base.game_path,
                &game_pkg.main.major.version,
                base.launcher_api,
                Some(biz_prefix.as_str()),
            )?;
            info!("HoYoverse full download completed for {}", base.game_folder);
            Ok(())
        }
        GameDownloadBackend::Kuro { source_biz_prefix } => {
            let download_context = fetch_kuro_download_context(base.launcher_api).await?;
            full_download::download_game(
                base.app,
                base.task_id,
                &download_context.launcher_info,
                &download_context.resource_index,
                base.game_path,
                base.cancel_token,
            )
            .await?;
            persist_local_source_state(
                base.game_path,
                &download_context.launcher_info.version,
                base.launcher_api,
                source_biz_prefix.as_deref(),
            )?;
            info!("Full download completed for {}", base.game_folder);
            Ok(())
        }
    }
}

pub(crate) async fn update_game(context: ContentTaskContext<'_>) -> Result<(), String> {
    let ContentTaskContext {
        backend,
        snowbreak_policy,
        base,
        languages,
    } = context;

    match backend {
        GameDownloadBackend::Snowbreak => {
            snowbreak_download::download_or_update_game(
                base.app,
                base.task_id,
                crate::events::GameDownloadOperation::UpdateGame,
                base.game_path,
                snowbreak_policy,
                base.cancel_token,
            )
            .await?;
            info!("Snowbreak update completed for {}", base.game_folder);
            Ok(())
        }
        GameDownloadBackend::Hoyoverse { biz_prefix } => {
            let game_pkg = hoyoverse::fetch_game_packages(base.launcher_api, biz_prefix).await?;
            let local_version = get_local_version_for_source(
                base.game_path,
                base.launcher_api,
                Some(biz_prefix.as_str()),
            )
            .or_else(|| get_local_version_internal(base.game_path))
            .ok_or("未找到本地版本，请使用全量下载".to_string())?;
            hoyoverse_download::update_game(
                base.app,
                base.task_id,
                &game_pkg,
                &local_version,
                base.game_path,
                languages,
                base.cancel_token,
            )
            .await?;
            persist_local_source_state(
                base.game_path,
                &game_pkg.main.major.version,
                base.launcher_api,
                Some(biz_prefix.as_str()),
            )?;
            info!("HoYoverse update completed for {}", base.game_folder);
            Ok(())
        }
        GameDownloadBackend::Kuro { source_biz_prefix } => {
            let download_context = fetch_kuro_download_context(base.launcher_api).await?;
            let local_version = get_local_version_for_source(
                base.game_path,
                base.launcher_api,
                source_biz_prefix.as_deref(),
            )
            .or_else(|| get_local_version_internal(base.game_path));

            if kuro_has_incremental_patch(&download_context.launcher_info, local_version.as_deref())
            {
                let version = local_version
                    .as_deref()
                    .expect("kuro incremental patch requires a local version");
                info!(
                    "Attempting incremental patch update for {} from local version {}",
                    base.game_folder, version
                );
                match incremental::update_game_patch(
                    base.app.clone(),
                    base.task_id,
                    &download_context.launcher_info,
                    version,
                    base.game_path,
                    base.cancel_token.clone(),
                )
                .await
                {
                    Ok(()) => {
                        persist_local_source_state(
                            base.game_path,
                            &download_context.launcher_info.version,
                            base.launcher_api,
                            source_biz_prefix.as_deref(),
                        )?;
                        info!(
                            "Incremental patch update completed for {}",
                            base.game_folder
                        );
                        return Ok(());
                    }
                    Err(err) => {
                        if *base.cancel_token.lock().await {
                            return Err(err);
                        }
                        warn!(
                            "Incremental patch update failed for {}: {}. Falling back to full comparison update.",
                            base.game_folder, err
                        );
                    }
                }
            } else {
                info!(
                    "No incremental patch available for {}, using full comparison update",
                    base.game_folder
                );
            }

            incremental::update_game_full(
                base.app,
                base.task_id,
                &download_context.launcher_info,
                &download_context.resource_index,
                base.game_path,
                base.cancel_token,
            )
            .await?;
            persist_local_source_state(
                base.game_path,
                &download_context.launcher_info.version,
                base.launcher_api,
                source_biz_prefix.as_deref(),
            )?;
            info!("Full comparison update completed for {}", base.game_folder);
            Ok(())
        }
    }
}

pub(crate) async fn update_game_patch(request: PatchUpdateRequest<'_>) -> Result<(), String> {
    let PatchUpdateRequest {
        app,
        task_id,
        launcher_api,
        game_path,
        game_folder,
        cancel_token,
    } = request;

    let local_version = get_local_version_internal(game_path)
        .ok_or("No local version found, cannot do incremental update")?;
    let launcher_info = cdn::fetch_launcher_info(launcher_api).await?;

    incremental::update_game_patch(
        app,
        task_id,
        &launcher_info,
        &local_version,
        game_path,
        cancel_token,
    )
    .await?;

    persist_local_source_state(game_path, &launcher_info.version, launcher_api, None)?;
    info!("Incremental patch update completed for {}", game_folder);
    Ok(())
}

pub(crate) async fn verify_game_files(
    context: VerifyTaskContext<'_>,
) -> Result<verifier::VerifyResult, String> {
    let VerifyTaskContext {
        backend,
        snowbreak_policy,
        base,
    } = context;

    match backend {
        GameDownloadBackend::Snowbreak => {
            snowbreak_download::verify_game(
                base.app,
                base.task_id,
                base.game_path,
                snowbreak_policy,
                base.cancel_token,
            )
            .await
        }
        GameDownloadBackend::Hoyoverse { biz_prefix } => {
            let game_pkg = hoyoverse::fetch_game_packages(base.launcher_api, biz_prefix).await?;
            let result = hoyoverse_download::verify_game(
                base.app,
                base.task_id,
                &game_pkg,
                base.game_path,
                base.cancel_token,
            )
            .await?;
            finalize_file_health_result(
                base.game_path,
                base.launcher_api,
                Some(biz_prefix.as_str()),
                &game_pkg.main.major.version,
                &result.failed,
                "Verification",
            )?;
            Ok(result)
        }
        GameDownloadBackend::Kuro { source_biz_prefix } => {
            let download_context = fetch_kuro_download_context(base.launcher_api).await?;
            let result = verifier::verify_game_files(
                base.app,
                base.task_id,
                &download_context.resource_index,
                base.game_path,
                base.cancel_token,
            )
            .await?;
            finalize_file_health_result(
                base.game_path,
                base.launcher_api,
                source_biz_prefix.as_deref(),
                &download_context.launcher_info.version,
                &result.failed,
                "Verification",
            )?;
            Ok(result)
        }
    }
}

pub(crate) async fn repair_game_files(
    context: RepairTaskContext<'_>,
) -> Result<verifier::RepairResult, String> {
    let RepairTaskContext {
        backend,
        snowbreak_policy,
        base,
        files,
    } = context;

    match backend {
        GameDownloadBackend::Snowbreak => {
            let _ = snowbreak_policy;
            Err("Snowbreak 暂不支持按异常列表单文件修复，请使用完整下载/更新".to_string())
        }
        GameDownloadBackend::Hoyoverse { .. } => {
            Err("HoYoverse 目前不支持按异常列表单文件修复，请使用重新下载".to_string())
        }
        GameDownloadBackend::Kuro { source_biz_prefix } => {
            let download_context = fetch_kuro_download_context(base.launcher_api).await?;
            let result = verifier::repair_game_files(
                base.app,
                base.task_id,
                &download_context.launcher_info,
                &download_context.resource_index,
                base.game_path,
                files,
                base.cancel_token,
            )
            .await?;
            finalize_file_health_result(
                base.game_path,
                base.launcher_api,
                source_biz_prefix.as_deref(),
                &download_context.launcher_info.version,
                &result.failed,
                "Repair",
            )?;
            Ok(result)
        }
    }
}

async fn get_snowbreak_game_state(
    game_path: &Path,
    source_policy: snowbreak::SourcePolicy,
) -> Result<GameState, String> {
    let local_version = snowbreak::read_local_version(game_path);

    let (remote_manifest, _cdn) = match snowbreak::fetch_manifest_with_policy(source_policy).await {
        Ok(manifest) => manifest,
        Err(err) => {
            tracing::error!("Snowbreak API 失败: {}", err);
            return Ok(build_network_error_state(local_version));
        }
    };

    Ok(build_resolved_game_state(
        local_version,
        remote_manifest.version,
        false,
    ))
}

async fn get_hoyoverse_game_state(
    launcher_api: &str,
    game_path: &Path,
    biz: &str,
) -> Result<GameState, String> {
    let game_pkg = match hoyoverse::fetch_game_packages(launcher_api, biz).await {
        Ok(pkg) => pkg,
        Err(err) => {
            tracing::error!("HoYoverse API 失败: {}", err);
            return Ok(build_network_error_state(get_local_version_for_source(
                game_path,
                launcher_api,
                Some(biz),
            )));
        }
    };

    let local_version = get_local_version_for_source(game_path, launcher_api, Some(biz));
    let remote_version = game_pkg.main.major.version.clone();
    let supports_incremental =
        supports_incremental_for_local(local_version.as_deref(), |version| {
            game_pkg
                .main
                .patches
                .iter()
                .any(|patch| patch.version == version)
        });

    Ok(build_resolved_game_state(
        local_version,
        remote_version,
        supports_incremental,
    ))
}

async fn get_kuro_game_state(
    launcher_api: &str,
    game_path: &Path,
    source_biz_prefix: Option<&str>,
) -> Result<GameState, String> {
    let launcher_info = match cdn::fetch_launcher_info(launcher_api).await {
        Ok(info) => info,
        Err(err) => {
            tracing::error!("fetch_launcher_info failed: {}", err);
            return Ok(build_network_error_state(get_local_version_for_source(
                game_path,
                launcher_api,
                source_biz_prefix,
            )));
        }
    };

    let local_version = get_local_version_for_source(game_path, launcher_api, source_biz_prefix);
    let remote_version = launcher_info.version.clone();
    let supports_incremental = kuro_has_incremental_patch(&launcher_info, local_version.as_deref());

    Ok(build_resolved_game_state(
        local_version,
        remote_version,
        supports_incremental,
    ))
}

async fn fetch_kuro_download_context(launcher_api: &str) -> Result<KuroDownloadContext, String> {
    let launcher_info = cdn::fetch_launcher_info(launcher_api).await?;
    let resource_index =
        cdn::fetch_resource_index(&launcher_info.cdn_url, &launcher_info.index_file_url).await?;
    Ok(KuroDownloadContext {
        launcher_info,
        resource_index,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::downloader::cdn::PatchConfig;
    use serde_json::json;

    #[test]
    fn resolve_backend_detects_snowbreak_without_biz_prefix() {
        let backend =
            GameDownloadBackend::resolve("https://snowbreak.amazingseasuncdn.com/game", None)
                .unwrap();

        assert_eq!(backend, GameDownloadBackend::Snowbreak);
    }

    #[test]
    fn resolve_backend_requires_biz_prefix_for_hoyoverse() {
        let result = GameDownloadBackend::resolve(
            "https://sg-hyp-api.hoyoverse.com/hyp/hyp-connect/api",
            None,
        );

        assert!(result.is_err());
        assert!(result
            .err()
            .unwrap()
            .contains("HoYoverse 下载缺少 biz_prefix"));
    }

    #[test]
    fn resolve_backend_normalizes_hoyoverse_and_kuro_biz_prefix() {
        let hoyo = GameDownloadBackend::resolve(
            "https://sg-hyp-api.hoyoverse.com/hyp/hyp-connect/api",
            Some("  hkrpg_global  "),
        )
        .unwrap();
        assert_eq!(
            hoyo,
            GameDownloadBackend::Hoyoverse {
                biz_prefix: "hkrpg_global".to_string()
            }
        );

        let kuro = GameDownloadBackend::resolve(
            "https://prod-alicdn-gamestarter.kurogame.com/launcher",
            Some("  cn  "),
        )
        .unwrap();
        assert_eq!(
            kuro,
            GameDownloadBackend::Kuro {
                source_biz_prefix: Some("cn".to_string())
            }
        );
    }

    #[test]
    fn resolve_backend_drops_empty_kuro_biz_prefix() {
        let backend = GameDownloadBackend::resolve(
            "https://prod-alicdn-gamestarter.kurogame.com/launcher",
            Some("   "),
        )
        .unwrap();

        assert_eq!(
            backend,
            GameDownloadBackend::Kuro {
                source_biz_prefix: None
            }
        );
    }

    #[test]
    fn resolve_launcher_state_matches_expected_transitions() {
        assert_eq!(
            resolve_launcher_state(None, "1.0.0"),
            LauncherState::NeedInstall
        );
        assert_eq!(
            resolve_launcher_state(Some("0.9.0"), "1.0.0"),
            LauncherState::NeedUpdate
        );
        assert_eq!(
            resolve_launcher_state(Some("1.0.0"), "1.0.0"),
            LauncherState::StartGame
        );
    }

    #[test]
    fn build_network_error_state_clears_remote_and_incremental_support() {
        let state = build_network_error_state(Some("1.2.3".to_string()));
        assert_eq!(state.state, LauncherState::NetworkError);
        assert_eq!(state.local_version.as_deref(), Some("1.2.3"));
        assert_eq!(state.remote_version, None);
        assert!(!state.supports_incremental);
    }

    #[test]
    fn build_resolved_game_state_uses_shared_state_resolver() {
        let install = build_resolved_game_state(None, "2.0.0".to_string(), false);
        assert_eq!(install.state, LauncherState::NeedInstall);

        let update =
            build_resolved_game_state(Some("1.0.0".to_string()), "2.0.0".to_string(), true);
        assert_eq!(update.state, LauncherState::NeedUpdate);
        assert!(update.supports_incremental);

        let ready = build_resolved_game_state(Some("2.0.0".to_string()), "2.0.0".to_string(), true);
        assert_eq!(ready.state, LauncherState::StartGame);
    }

    #[test]
    fn supports_incremental_for_local_only_checks_when_local_version_exists() {
        let mut called = false;
        let supports = supports_incremental_for_local(None, |_| {
            called = true;
            true
        });
        assert!(!supports);
        assert!(!called);

        let supports = supports_incremental_for_local(Some("1.0.0"), |version| version == "1.0.0");
        assert!(supports);
    }

    #[test]
    fn kuro_has_incremental_patch_requires_matching_version() {
        let launcher = LauncherInfo {
            version: "1.1.0".to_string(),
            resources_base_path: "".to_string(),
            cdn_url: "https://example.com".to_string(),
            index_file_url: "index.json".to_string(),
            patch_configs: vec![
                PatchConfig {
                    version: "1.0.0".to_string(),
                    base_url: "patch-a".to_string(),
                    index_file: "a.json".to_string(),
                    ext: json!({}),
                },
                PatchConfig {
                    version: "1.0.1".to_string(),
                    base_url: "patch-b".to_string(),
                    index_file: "b.json".to_string(),
                    ext: json!({"kind": "patch"}),
                },
            ],
            raw: json!({}),
        };

        assert!(kuro_has_incremental_patch(&launcher, Some("1.0.0")));
        assert!(kuro_has_incremental_patch(&launcher, Some("1.0.1")));
        assert!(!kuro_has_incremental_patch(&launcher, Some("0.9.9")));
        assert!(!kuro_has_incremental_patch(&launcher, None));
    }
}
