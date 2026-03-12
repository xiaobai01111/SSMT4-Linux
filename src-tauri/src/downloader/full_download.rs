use crate::downloader::cdn::{LauncherInfo, ResourceFile, ResourceIndex};
use crate::downloader::progress::{
    emit_game_download_snapshot, DownloadProgressStats, SpeedTracker,
};
use crate::events::GameDownloadOperation;
use crate::utils::file_manager::safe_join_remote;
use crate::utils::hash_verify;
use futures_util::StreamExt;
use reqwest::Client;
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::AppHandle;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// 最大并行下载数（小文件多时高并发效果好）
const MAX_CONCURRENT: usize = 64;

struct ParallelDownloadRequest<'a> {
    client: &'a Client,
    url: &'a str,
    dest: &'a Path,
    expected_size: u64,
    expected_sha256: Option<&'a str>,
    expected_md5: Option<&'a str>,
    shared_bytes: Arc<AtomicU64>,
    cancel_token: Arc<Mutex<bool>>,
}

/// Full game download — 高并发并行下载
pub async fn download_game(
    app: AppHandle,
    task_id: &str,
    launcher_info: &LauncherInfo,
    resource_index: &ResourceIndex,
    game_folder: &Path,
    cancel_token: Arc<Mutex<bool>>,
) -> Result<(), String> {
    let total_count = resource_index.resource.len();
    let total_size: u64 = resource_index.resource.iter().map(|r| r.size).sum();

    info!(
        "开始并行下载: {} 个文件, {:.1} GB, 最大并发 {}",
        total_count,
        total_size as f64 / 1073741824.0,
        MAX_CONCURRENT
    );

    // ===== 阶段 1: 预检查已有文件，跳过已下载的 =====
    let mut cached_size: u64 = 0;
    let mut to_download: Vec<&ResourceFile> = Vec::new();

    for file in &resource_index.resource {
        let file_path = safe_join_remote(game_folder, &file.dest)?;
        if file_path.exists() {
            match hash_verify::verify_file_integrity(
                &file_path,
                file.size,
                file.sha256.as_deref(),
                Some(file.md5.as_str()),
            )
            .await
            {
                Ok(hash_verify::VerifiedHashAlgo::Sha256)
                | Ok(hash_verify::VerifiedHashAlgo::Md5) => {
                    cached_size += file.size;
                    continue;
                }
                Err(err) => {
                    warn!("{} 完整性校验失败，需重下: {}", file.dest, err);
                }
            }
        }
        to_download.push(file);
    }

    let cached_count = total_count - to_download.len();
    if cached_count > 0 {
        info!(
            "跳过 {} 个已下载文件 ({:.1} GB), 需下载 {} 个文件",
            cached_count,
            cached_size as f64 / 1073741824.0,
            to_download.len()
        );
    }

    if to_download.is_empty() {
        info!("所有文件已存在，无需下载");
        emit_progress(
            &app,
            task_id,
            DownloadProgressStats {
                finished_size: cached_size,
                total_size,
                finished_count: total_count,
                total_count,
                ..DownloadProgressStats::default()
            },
            "全部完成",
        );
        return Ok(());
    }

    // ===== 阶段 2: 并行下载 =====
    let client = Client::builder()
        .pool_max_idle_per_host(MAX_CONCURRENT + 4)
        .tcp_keepalive(Some(std::time::Duration::from_secs(30)))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let shared_bytes = Arc::new(AtomicU64::new(cached_size));
    let shared_done = Arc::new(AtomicUsize::new(cached_count));

    // 进度上报任务（每 200ms 刷新一次）
    let reporter = {
        let app = app.clone();
        let bytes = shared_bytes.clone();
        let done = shared_done.clone();
        let cancel = cancel_token.clone();
        let task_id = task_id.to_string();
        tokio::spawn(async move {
            let mut tracker = SpeedTracker::new();
            let mut prev = bytes.load(Ordering::Relaxed);
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                if *cancel.lock().await {
                    break;
                }
                let current = bytes.load(Ordering::Relaxed);
                let delta = current.saturating_sub(prev);
                prev = current;
                if delta > 0 {
                    tracker.record(delta);
                }
                let remaining = total_size.saturating_sub(current);
                emit_progress(
                    &app,
                    &task_id,
                    DownloadProgressStats {
                        finished_size: current,
                        total_size,
                        finished_count: done.load(Ordering::Relaxed),
                        total_count,
                        speed_bps: tracker.speed_bps(),
                        eta_seconds: tracker.eta_seconds(remaining),
                    },
                    format!("并行下载中 ({}路)", MAX_CONCURRENT),
                );
            }
        })
    };

    // 信号量控制并发
    let sem = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT));
    let mut handles = Vec::new();

    for file in &to_download {
        let sem = sem.clone();
        let client = client.clone();
        let cancel = cancel_token.clone();
        let bytes = shared_bytes.clone();
        let done = shared_done.clone();

        let url = build_resource_url(
            &launcher_info.cdn_url,
            &launcher_info.resources_base_path,
            &file.dest,
        );
        let dest = safe_join_remote(game_folder, &file.dest)?;
        let expected_size = file.size;
        let expected_sha256 = file.sha256.clone();
        let expected_md5 = file.md5.clone();

        let handle = tokio::spawn(async move {
            let _permit = sem
                .acquire()
                .await
                .map_err(|e| format!("信号量错误: {}", e))?;

            download_file_parallel(ParallelDownloadRequest {
                client: &client,
                url: &url,
                dest: &dest,
                expected_size,
                expected_sha256: expected_sha256.as_deref(),
                expected_md5: Some(expected_md5.as_str()),
                shared_bytes: bytes,
                cancel_token: cancel,
            })
            .await?;
            done.fetch_add(1, Ordering::Relaxed);
            Ok::<(), String>(())
        });
        handles.push(handle);
    }

    // 等待所有下载完成
    let mut first_error: Option<String> = None;
    for handle in handles {
        match handle.await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                if first_error.is_none() {
                    first_error = Some(e);
                }
            }
            Err(e) => {
                if first_error.is_none() {
                    first_error = Some(format!("下载任务崩溃: {}", e));
                }
            }
        }
    }

    reporter.abort();

    if let Some(e) = first_error {
        return Err(e);
    }

    emit_progress(
        &app,
        task_id,
        DownloadProgressStats {
            finished_size: total_size,
            total_size,
            finished_count: total_count,
            total_count,
            ..DownloadProgressStats::default()
        },
        "下载完成",
    );
    info!("并行下载完成: {} 个文件", total_count);
    Ok(())
}

/// 并行友好的文件下载（断点续传 + AtomicU64 进度汇报）
async fn download_file_parallel(request: ParallelDownloadRequest<'_>) -> Result<(), String> {
    let ParallelDownloadRequest {
        client,
        url,
        dest,
        expected_size,
        expected_sha256,
        expected_md5,
        shared_bytes,
        cancel_token,
    } = request;

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("创建目录失败: {}", e))?;
    }

    let temp_path = dest.with_extension(format!(
        "{}.temp",
        dest.extension().and_then(|e| e.to_str()).unwrap_or("dl")
    ));

    // 断点续传检查
    let mut downloaded_bytes: u64 = 0;
    if temp_path.exists() {
        if let Ok(meta) = tokio::fs::metadata(&temp_path).await {
            downloaded_bytes = meta.len();
            shared_bytes.fetch_add(downloaded_bytes, Ordering::Relaxed);
        }
    }

    let mut req = client.get(url).header("User-Agent", "Mozilla/5.0");
    if downloaded_bytes > 0 {
        req = req.header("Range", format!("bytes={}-", downloaded_bytes));
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("请求失败 {}: {}", url, e))?;

    let status = resp.status().as_u16();
    match status {
        206 => {} // 断点续传
        200 => {
            if downloaded_bytes > 0 {
                shared_bytes.fetch_sub(downloaded_bytes, Ordering::Relaxed);
                downloaded_bytes = 0;
            }
        }
        416 => {
            // 已下载完毕
            if temp_path.exists() {
                hash_verify::verify_file_integrity(
                    &temp_path,
                    expected_size,
                    expected_sha256,
                    expected_md5,
                )
                .await?;
                tokio::fs::rename(&temp_path, dest)
                    .await
                    .map_err(|e| format!("重命名失败: {}", e))?;
                return Ok(());
            }
            return Err(format!("HTTP 416 无临时文件: {}", url));
        }
        _ => {
            return Err(format!("HTTP {}: {}", status, url));
        }
    }

    let mut file = if downloaded_bytes > 0 {
        tokio::fs::OpenOptions::new()
            .append(true)
            .open(&temp_path)
            .await
            .map_err(|e| format!("打开临时文件失败: {}", e))?
    } else {
        tokio::fs::File::create(&temp_path)
            .await
            .map_err(|e| format!("创建临时文件失败: {}", e))?
    };

    let mut stream = resp.bytes_stream();
    let mut last_cancel_check = std::time::Instant::now();

    while let Some(chunk_result) = stream.next().await {
        if last_cancel_check.elapsed() > std::time::Duration::from_secs(2) {
            if *cancel_token.lock().await {
                return Err("下载已取消".to_string());
            }
            last_cancel_check = std::time::Instant::now();
        }

        let chunk = chunk_result.map_err(|e| format!("流读取错误: {}", e))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("写入错误: {}", e))?;

        let len = chunk.len() as u64;
        shared_bytes.fetch_add(len, Ordering::Relaxed);
    }

    file.flush()
        .await
        .map_err(|e| format!("刷新缓冲失败: {}", e))?;
    drop(file);

    hash_verify::verify_file_integrity(&temp_path, expected_size, expected_sha256, expected_md5)
        .await?;

    tokio::fs::rename(&temp_path, dest)
        .await
        .map_err(|e| format!("重命名失败: {}", e))?;

    Ok(())
}

fn emit_progress(
    app: &AppHandle,
    task_id: &str,
    stats: DownloadProgressStats,
    current_file: impl Into<String>,
) {
    emit_game_download_snapshot(
        app,
        task_id,
        GameDownloadOperation::DownloadGame,
        "download",
        current_file,
        stats,
    );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_resource_url_trims_slashes_and_encodes_spaces() {
        let url = build_resource_url(
            "https://cdn.example.com/",
            "/game data/",
            "/voice/file name.zip",
        );

        assert_eq!(
            url,
            "https://cdn.example.com/game%20data/voice/file%20name.zip"
        );
    }

    #[test]
    fn urlencoded_safe_preserves_scheme_and_path_separators() {
        let encoded = urlencoded_safe("https://example.com/a b/c:d?e");
        assert_eq!(encoded, "https://example.com/a%20b/c:d%3Fe");
    }

    #[test]
    fn urlencoded_safe_without_scheme_returns_original_string() {
        assert_eq!(
            urlencoded_safe("relative path/file.zip"),
            "relative path/file.zip"
        );
    }
}
