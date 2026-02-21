use crate::configs::database as db;
use crate::downloader::cdn::{LauncherInfo, ResourceFile, ResourceIndex};
use crate::downloader::fetcher;
use crate::downloader::progress::{DownloadProgress, SpeedTracker};
use crate::utils::file_manager::{safe_join, safe_join_remote};
use crate::utils::hash_verify;
use reqwest::Client;
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::{Mutex, Semaphore};
use tracing::{error, info, warn};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VerifyResult {
    pub total_files: usize,
    pub verified_ok: usize,
    pub redownloaded: usize,
    pub failed: Vec<String>,
}

/// 获取文件的 mtime（秒级时间戳）
fn get_mtime_sec(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 带缓存的 MD5 计算：先查缓存（size+mtime 匹配则复用），否则计算并写入缓存
///
/// DB 读写通过 spawn_blocking 执行，避免同步 Mutex + SQLite I/O 阻塞 async 运行时
async fn md5_with_cache(path: &Path, expected_size: u64) -> String {
    let path_str = path.to_string_lossy().to_string();
    let mtime = get_mtime_sec(path);

    // 查缓存（spawn_blocking 避免阻塞 async 运行时）
    {
        let ps = path_str.clone();
        let size = expected_size as i64;
        let mt = mtime;
        if let Ok(Some(cached)) =
            tokio::task::spawn_blocking(move || db::get_cached_md5(&ps, size, mt)).await
        {
            return cached;
        }
    }

    // 计算 MD5
    let md5 = hash_verify::md5_file(path).await.unwrap_or_default();

    // 写入缓存（spawn_blocking）
    if !md5.is_empty() {
        let ps = path_str;
        let size = expected_size as i64;
        let mt = mtime;
        let md5_c = md5.clone();
        tokio::task::spawn_blocking(move || db::set_cached_md5(&ps, size, mt, &md5_c))
            .await
            .ok();
    }

    md5
}

/// 单个文件的校验结果
#[allow(dead_code)]
enum FileVerifyResult {
    Ok,
    Redownloaded,
    Failed(String),
    SizeMismatchRedownloaded,
    SizeMismatchFailed(String),
}

/// Verify game files — 两阶段校验 + 哈希缓存 + 并发
pub async fn verify_game_files(
    app: AppHandle,
    launcher_info: &LauncherInfo,
    resource_index: &ResourceIndex,
    game_folder: &Path,
    cancel_token: Arc<Mutex<bool>>,
) -> Result<VerifyResult, String> {
    let total_files = resource_index.resource.len();
    let total_size: u64 = resource_index.resource.iter().map(|r| r.size).sum();

    info!(
        "开始校验 {} 个文件（总计 {} bytes）",
        total_files, total_size
    );

    // 清理无效 pak 文件
    remove_invalid_paks(game_folder, resource_index).await;

    // ---- 第一阶段：快速 exists + size 筛选 ----
    let mut need_hash: Vec<(usize, &ResourceFile)> = Vec::new();
    let mut need_download: Vec<(usize, &ResourceFile)> = Vec::new();
    let mut _phase1_skipped: u64 = 0;
    let mut unsafe_paths: Vec<String> = Vec::new();

    for (i, file) in resource_index.resource.iter().enumerate() {
        let file_path = match safe_join_remote(game_folder, &file.dest) {
            Ok(p) => p,
            Err(e) => {
                warn!("不安全的清单路径（计入失败）: {} ({})", file.dest, e);
                unsafe_paths.push(format!("{} (unsafe path: {})", file.dest, e));
                continue;
            }
        };
        match tokio::fs::metadata(&file_path).await {
            Ok(meta) if meta.len() == file.size => {
                // size 匹配，需要进一步做 MD5 校验
                need_hash.push((i, file));
            }
            Ok(meta) => {
                // 文件存在但 size 不对，直接标记重下
                warn!(
                    "{} size mismatch (expected: {}, got: {})",
                    file.dest,
                    file.size,
                    meta.len()
                );
                need_download.push((i, file));
                _phase1_skipped += file.size;
            }
            Err(_) => {
                // 文件不存在，直接标记重下
                warn!("{} 文件不存在", file.dest);
                need_download.push((i, file));
                _phase1_skipped += file.size;
            }
        }
    }

    info!(
        "第一阶段完成: {} 个文件需 MD5 校验, {} 个文件需重新下载（size 不匹配/不存在）",
        need_hash.len(),
        need_download.len()
    );

    // 共享计数器（原子操作，并发安全）
    let verified_ok = Arc::new(AtomicUsize::new(0));
    let redownloaded = Arc::new(AtomicUsize::new(0));
    let failed = Arc::new(Mutex::new(Vec::<String>::new()));
    let finished_size = Arc::new(AtomicU64::new(0));
    let finished_count = Arc::new(AtomicUsize::new(0));
    let speed_tracker = Arc::new(Mutex::new(SpeedTracker::new()));

    // 将第一阶段发现的不安全路径计入 failed 和 finished_count
    if !unsafe_paths.is_empty() {
        warn!("{} 个清单条目因路径不安全被标记为失败", unsafe_paths.len());
        failed.lock().await.extend(unsafe_paths);
    }

    // 并发控制（SSD 友好，避免过度并发导致 IO 抖动）
    let semaphore = Arc::new(Semaphore::new(4));
    let client = Arc::new(Client::new());

    // ---- 第二阶段：并发哈希校验（优先 SHA256，回退 MD5） ----
    let mut hash_tasks = Vec::new();

    for (_idx, file) in need_hash {
        if *cancel_token.lock().await {
            return Err("Verification cancelled".to_string());
        }

        let sem = semaphore.clone();
        let file_dest = file.dest.clone();
        let file_md5 = file.md5.clone();
        let file_sha256 = file.sha256.clone().unwrap_or_default();
        let file_size = file.size;
        let file_path = match safe_join(game_folder, &file.dest) {
            Ok(p) => p,
            Err(e) => {
                warn!("不安全的清单路径（计入失败）: {} ({})", file.dest, e);
                failed
                    .lock()
                    .await
                    .push(format!("{} (unsafe path: {})", file.dest, e));
                finished_count.fetch_add(1, Ordering::Relaxed);
                continue;
            }
        };
        let app_c = app.clone();
        let launcher_info_cdn = launcher_info.cdn_url.clone();
        let launcher_info_base = launcher_info.resources_base_path.clone();
        let client_c = client.clone();
        let verified_ok_c = verified_ok.clone();
        let redownloaded_c = redownloaded.clone();
        let failed_c = failed.clone();
        let finished_size_c = finished_size.clone();
        let finished_count_c = finished_count.clone();
        let speed_tracker_c = speed_tracker.clone();
        let cancel_c = cancel_token.clone();

        let task = tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();

            if *cancel_c.lock().await {
                return;
            }

            let hash_match = if !file_sha256.trim().is_empty() {
                match hash_verify::verify_file_integrity(
                    &file_path,
                    file_size,
                    Some(&file_sha256),
                    None,
                )
                .await
                {
                    Ok(_) => true,
                    Err(err) => {
                        warn!("{} SHA256 校验失败: {}", file_dest, err);
                        false
                    }
                }
            } else {
                let current_md5 = md5_with_cache(&file_path, file_size).await;
                if current_md5 == file_md5 {
                    true
                } else {
                    warn!(
                        "{} MD5 mismatch (expected: {}, got: {})",
                        file_dest, file_md5, current_md5
                    );
                    false
                }
            };

            if hash_match {
                verified_ok_c.fetch_add(1, Ordering::Relaxed);
            } else {
                // 重下
                let url = build_resource_url(&launcher_info_cdn, &launcher_info_base, &file_dest);
                match redownload_and_verify(
                    &client_c,
                    &url,
                    &file_path,
                    file_sha256.as_str(),
                    file_md5.as_str(),
                    file_size,
                )
                .await
                {
                    true => {
                        redownloaded_c.fetch_add(1, Ordering::Relaxed);
                    }
                    false => {
                        failed_c.lock().await.push(file_dest.clone());
                    }
                }
            }

            finished_size_c.fetch_add(file_size, Ordering::Relaxed);
            let count = finished_count_c.fetch_add(1, Ordering::Relaxed) + 1;
            speed_tracker_c.lock().await.record(file_size);

            // 发射进度（每 10 个文件或最后一个）
            if count.is_multiple_of(10) || count == total_files {
                let fs = finished_size_c.load(Ordering::Relaxed);
                let remaining = total_size.saturating_sub(fs);
                let mut st = speed_tracker_c.lock().await;
                let progress = DownloadProgress {
                    phase: "verify".to_string(),
                    total_size,
                    finished_size: fs,
                    total_count: total_files,
                    finished_count: count,
                    current_file: file_dest,
                    speed_bps: st.speed_bps(),
                    eta_seconds: st.eta_seconds(remaining),
                };
                app_c.emit("game-verify-progress", &progress).ok();
            }
        });

        hash_tasks.push(task);
    }

    // ---- 处理需重下的文件（size 不匹配/不存在）----
    for (_idx, file) in need_download {
        if *cancel_token.lock().await {
            return Err("Verification cancelled".to_string());
        }

        let sem = semaphore.clone();
        let file_dest = file.dest.clone();
        let file_md5 = file.md5.clone();
        let file_sha256 = file.sha256.clone().unwrap_or_default();
        let file_size = file.size;
        let file_path = match safe_join(game_folder, &file.dest) {
            Ok(p) => p,
            Err(e) => {
                warn!("不安全的清单路径（计入失败）: {} ({})", file.dest, e);
                failed
                    .lock()
                    .await
                    .push(format!("{} (unsafe path: {})", file.dest, e));
                finished_count.fetch_add(1, Ordering::Relaxed);
                continue;
            }
        };
        let app_c = app.clone();
        let launcher_info_cdn = launcher_info.cdn_url.clone();
        let launcher_info_base = launcher_info.resources_base_path.clone();
        let client_c = client.clone();
        let redownloaded_c = redownloaded.clone();
        let failed_c = failed.clone();
        let finished_size_c = finished_size.clone();
        let finished_count_c = finished_count.clone();
        let speed_tracker_c = speed_tracker.clone();
        let cancel_c = cancel_token.clone();

        let task = tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();

            if *cancel_c.lock().await {
                return;
            }

            let url = build_resource_url(&launcher_info_cdn, &launcher_info_base, &file_dest);
            match redownload_and_verify(
                &client_c,
                &url,
                &file_path,
                file_sha256.as_str(),
                file_md5.as_str(),
                file_size,
            )
            .await
            {
                true => {
                    redownloaded_c.fetch_add(1, Ordering::Relaxed);
                }
                false => {
                    failed_c.lock().await.push(file_dest.clone());
                }
            }

            finished_size_c.fetch_add(file_size, Ordering::Relaxed);
            let count = finished_count_c.fetch_add(1, Ordering::Relaxed) + 1;
            speed_tracker_c.lock().await.record(file_size);

            if count.is_multiple_of(10) || count == total_files {
                let fs = finished_size_c.load(Ordering::Relaxed);
                let remaining = total_size.saturating_sub(fs);
                let mut st = speed_tracker_c.lock().await;
                let progress = DownloadProgress {
                    phase: "verify".to_string(),
                    total_size,
                    finished_size: fs,
                    total_count: total_files,
                    finished_count: count,
                    current_file: file_dest,
                    speed_bps: st.speed_bps(),
                    eta_seconds: st.eta_seconds(remaining),
                };
                app_c.emit("game-verify-progress", &progress).ok();
            }
        });

        hash_tasks.push(task);
    }

    // 等待所有任务完成，显式处理 JoinError（panic/取消）
    for task in hash_tasks {
        if let Err(e) = task.await {
            let msg = if e.is_panic() {
                format!("校验子任务 panic: {}", e)
            } else {
                format!("校验子任务被取消: {}", e)
            };
            error!("{}", msg);
            failed.lock().await.push(msg);
        }
    }

    let ok = verified_ok.load(Ordering::Relaxed);
    let redl = redownloaded.load(Ordering::Relaxed);
    let fail = failed.lock().await.clone();

    info!(
        "校验完成: ok={}, redownloaded={}, failed={}",
        ok,
        redl,
        fail.len()
    );

    Ok(VerifyResult {
        total_files,
        verified_ok: ok,
        redownloaded: redl,
        failed: fail,
    })
}

/// 重下文件并校验完整性（size + SHA256/MD5）
async fn redownload_and_verify(
    client: &Client,
    url: &str,
    file_path: &Path,
    expected_sha256: &str,
    expected_md5: &str,
    file_size: u64,
) -> bool {
    if let Err(e) = fetcher::download_with_resume(client, url, file_path, true, None).await {
        error!("Failed to re-download {}: {}", file_path.display(), e);
        return false;
    }

    let verify = hash_verify::verify_file_integrity(
        file_path,
        file_size,
        Some(expected_sha256),
        Some(expected_md5),
    )
    .await;
    match verify {
        Ok(crate::utils::hash_verify::VerifiedHashAlgo::Md5) => {
            let path_str = file_path.to_string_lossy().to_string();
            let mtime = get_mtime_sec(file_path);
            if !expected_md5.trim().is_empty() {
                db::set_cached_md5(&path_str, file_size as i64, mtime, expected_md5);
            }
            info!("{} MD5 OK after re-download", file_path.display());
            true
        }
        Ok(crate::utils::hash_verify::VerifiedHashAlgo::Sha256) => {
            info!("{} SHA256 OK after re-download", file_path.display());
            true
        }
        Err(err) => {
            error!(
                "{} still checksum mismatch after re-download: {}",
                file_path.display(),
                err
            );
            false
        }
    }
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
        .filter_map(|r| r.dest.split('/').next_back().map(|s| s.to_string()))
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
