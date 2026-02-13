use crate::downloader::cdn::{LauncherInfo, ResourceIndex};
use crate::downloader::fetcher;
use crate::downloader::progress::{DownloadProgress, SpeedTracker};
use reqwest::Client;
use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tracing::info;

/// Full game download — mirrors LutheringLaves.py `download_game`
pub async fn download_game(
    app: AppHandle,
    launcher_info: &LauncherInfo,
    resource_index: &ResourceIndex,
    game_folder: &Path,
    cancel_token: Arc<Mutex<bool>>,
) -> Result<(), String> {
    let client = Client::new();
    let total_count = resource_index.resource.len();
    let total_size: u64 = resource_index.resource.iter().map(|r| r.size).sum();

    info!(
        "Starting full download: {} files, {} bytes",
        total_count, total_size
    );

    let mut finished_size: u64 = 0;
    let mut finished_count: usize = 0;
    let mut speed_tracker = SpeedTracker::new();

    for file in &resource_index.resource {
        // Check cancellation
        if *cancel_token.lock().await {
            info!("Download cancelled by user");
            return Err("Download cancelled".to_string());
        }

        let download_url = build_resource_url(
            &launcher_info.cdn_url,
            &launcher_info.resources_base_path,
            &file.dest,
        );
        let file_path = game_folder.join(&file.dest);

        info!(
            "Downloading file {}/{}: {}",
            finished_count + 1,
            total_count,
            file.dest
        );

        fetcher::download_with_resume(
            &client,
            &download_url,
            &file_path,
            false,
            None,
        )
        .await?;

        // Update progress after each file
        finished_size += file.size;
        finished_count += 1;
        speed_tracker.record(file.size);

        let remaining = total_size.saturating_sub(finished_size);
        let progress = DownloadProgress {
            phase: "download".to_string(),
            total_size,
            finished_size,
            total_count,
            finished_count,
            current_file: file.dest.clone(),
            speed_bps: speed_tracker.speed_bps(),
            eta_seconds: speed_tracker.eta_seconds(remaining),
        };

        app.emit("game-download-progress", &progress).ok();
    }

    info!("Full download complete: {} files", finished_count);
    Ok(())
}

fn build_resource_url(cdn_url: &str, resources_base_path: &str, dest: &str) -> String {
    let base = cdn_url.trim_end_matches('/');
    let mid = resources_base_path.trim_matches('/');
    let file = dest.trim_start_matches('/');

    let raw = format!("{}/{}/{}", base, mid, file);
    // URL-encode path segments but keep :/
    urlencoded_safe(&raw)
}

fn urlencoded_safe(url: &str) -> String {
    // Split at :// to preserve scheme
    if let Some(idx) = url.find("://") {
        let scheme = &url[..idx + 3];
        let rest = &url[idx + 3..];
        let encoded: String = rest
            .chars()
            .map(|c| match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '/' | '~' | ':' => {
                    c.to_string()
                }
                ' ' => "%20".to_string(),
                _ => format!("%{:02X}", c as u32),
            })
            .collect();
        format!("{}{}", scheme, encoded)
    } else {
        url.to_string()
    }
}
