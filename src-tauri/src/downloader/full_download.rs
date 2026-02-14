use crate::downloader::cdn::{LauncherInfo, ResourceIndex, ResourceFile};
use crate::downloader::progress::{DownloadProgress, SpeedTracker};
use crate::utils::file_manager::safe_join_remote;
use futures_util::StreamExt;
use reqwest::Client;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// 最大并行下载数（小文件多时高并发效果好）
const MAX_CONCURRENT: usize = 64;

/// Full game download — 高并发并行下载
pub async fn download_game(
    app: AppHandle,
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
        let file_path = game_folder.join(&file.dest);
        if file_path.exists() {
            if let Ok(meta) = tokio::fs::metadata(&file_path).await {
                if meta.len() == file.size {
                    cached_size += file.size;
                    continue;
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
        emit_progress(&app, cached_size, total_size, total_count, total_count, "全部完成", 0, 0);
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
                    &app, current, total_size,
                    total_count, done.load(Ordering::Relaxed),
                    &format!("并行下载中 ({}路)", MAX_CONCURRENT),
                    tracker.speed_bps(),
                    tracker.eta_seconds(remaining),
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

        let handle = tokio::spawn(async move {
            let _permit = sem
                .acquire()
                .await
                .map_err(|e| format!("信号量错误: {}", e))?;

            download_file_parallel(&client, &url, &dest, expected_size, bytes, cancel).await?;
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

    emit_progress(&app, total_size, total_size, total_count, total_count, "下载完成", 0, 0);
    info!("并行下载完成: {} 个文件", total_count);
    Ok(())
}

/// 并行友好的文件下载（断点续传 + AtomicU64 进度汇报）
async fn download_file_parallel(
    client: &Client,
    url: &str,
    dest: &PathBuf,
    expected_size: u64,
    shared_bytes: Arc<AtomicU64>,
    cancel_token: Arc<Mutex<bool>>,
) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("创建目录失败: {}", e))?;
    }

    let temp_path = dest.with_extension(
        format!(
            "{}.temp",
            dest.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("dl")
        ),
    );

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

    // 大小校验
    if expected_size > 0 {
        let actual_size = tokio::fs::metadata(&temp_path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        if actual_size != expected_size {
            warn!(
                "文件大小不匹配 {}: 期望 {} 实际 {}",
                dest.display(), expected_size, actual_size
            );
        }
    }

    tokio::fs::rename(&temp_path, dest)
        .await
        .map_err(|e| format!("重命名失败: {}", e))?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn emit_progress(
    app: &AppHandle,
    finished_size: u64,
    total_size: u64,
    total_count: usize,
    finished_count: usize,
    current_file: &str,
    speed_bps: u64,
    eta_seconds: u64,
) {
    let progress = DownloadProgress {
        phase: "download".to_string(),
        total_size,
        finished_size,
        total_count,
        finished_count,
        current_file: current_file.to_string(),
        speed_bps,
        eta_seconds,
    };
    app.emit("game-download-progress", &progress).ok();
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
