use crate::downloader::cdn::{LauncherInfo, ResourceIndex};
use crate::downloader::fetcher;
use crate::downloader::progress::{DownloadProgress, SpeedTracker};
use crate::utils::hash_verify;
use reqwest::Client;
use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tracing::{error, info, warn};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VerifyResult {
    pub total_files: usize,
    pub verified_ok: usize,
    pub redownloaded: usize,
    pub failed: Vec<String>,
}

/// Verify game files — mirrors LutheringLaves.py `verify_gamefile`
pub async fn verify_game_files(
    app: AppHandle,
    launcher_info: &LauncherInfo,
    resource_index: &ResourceIndex,
    game_folder: &Path,
    cancel_token: Arc<Mutex<bool>>,
) -> Result<VerifyResult, String> {
    let client = Client::new();
    let total_files = resource_index.resource.len();
    let total_size: u64 = resource_index.resource.iter().map(|r| r.size).sum();

    info!("Starting verification of {} files", total_files);

    // Remove invalid pak files not in the index
    remove_invalid_paks(game_folder, resource_index).await;

    let mut verified_ok: usize = 0;
    let mut redownloaded: usize = 0;
    let mut failed: Vec<String> = Vec::new();
    let mut finished_size: u64 = 0;
    let mut speed_tracker = SpeedTracker::new();

    for (i, file) in resource_index.resource.iter().enumerate() {
        if *cancel_token.lock().await {
            return Err("Verification cancelled".to_string());
        }

        let file_path = game_folder.join(&file.dest);
        let current_md5 = hash_verify::md5_file(&file_path).await.unwrap_or_default();

        if current_md5 == file.md5 {
            info!("{} MD5 match", file.dest);
            verified_ok += 1;
            finished_size += file.size;
            speed_tracker.record(file.size);
        } else {
            warn!(
                "{} MD5 mismatch (expected: {}, got: {})",
                file.dest, file.md5, current_md5
            );

            // Re-download
            let download_url = build_resource_url(
                &launcher_info.cdn_url,
                &launcher_info.resources_base_path,
                &file.dest,
            );

            if let Err(e) =
                fetcher::download_with_resume(&client, &download_url, &file_path, true, None).await
            {
                error!("Failed to re-download {}: {}", file.dest, e);
                failed.push(file.dest.clone());
                finished_size += file.size;
                continue;
            }

            // Verify again after re-download
            let new_md5 = hash_verify::md5_file(&file_path).await.unwrap_or_default();
            if new_md5 == file.md5 {
                info!("{} MD5 OK after re-download", file.dest);
                redownloaded += 1;
            } else {
                error!("{} still MD5 mismatch after re-download", file.dest);
                failed.push(file.dest.clone());
            }
            finished_size += file.size;
            speed_tracker.record(file.size);
        }

        let remaining = total_size.saturating_sub(finished_size);
        let progress = DownloadProgress {
            phase: "verify".to_string(),
            total_size,
            finished_size,
            total_count: total_files,
            finished_count: i + 1,
            current_file: file.dest.clone(),
            speed_bps: speed_tracker.speed_bps(),
            eta_seconds: speed_tracker.eta_seconds(remaining),
        };
        app.emit("game-verify-progress", &progress).ok();
    }

    info!(
        "Verification complete: ok={}, redownloaded={}, failed={}",
        verified_ok,
        redownloaded,
        failed.len()
    );

    Ok(VerifyResult {
        total_files,
        verified_ok,
        redownloaded,
        failed,
    })
}

async fn remove_invalid_paks(game_folder: &Path, resource_index: &ResourceIndex) {
    let paks_dir = game_folder.join("Client").join("Content").join("Paks");
    if !paks_dir.exists() {
        return;
    }

    // Collect valid pak names from index
    let valid_paks: std::collections::HashSet<String> = resource_index
        .resource
        .iter()
        .filter(|r| r.dest.starts_with("Client/Content/Paks/"))
        .filter_map(|r| r.dest.split('/').last().map(|s| s.to_string()))
        .collect();

    let entries = match tokio::fs::read_dir(&paks_dir).await {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut entries = entries;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().to_string();
        if !valid_paks.contains(&name) {
            warn!("Removing invalid pak: {}", name);
            tokio::fs::remove_file(entry.path()).await.ok();
        }
    }
}

fn build_resource_url(cdn_url: &str, resources_base_path: &str, dest: &str) -> String {
    let base = cdn_url.trim_end_matches('/');
    let mid = resources_base_path.trim_matches('/');
    let file = dest.trim_start_matches('/');
    format!("{}/{}/{}", base, mid, file)
}
