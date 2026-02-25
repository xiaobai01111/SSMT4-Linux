use crate::configs::database as db;
use crate::downloader::cdn::{LauncherInfo, ResourceFile, ResourceIndex};
use crate::downloader::fetcher;
use crate::downloader::progress::{DownloadProgress, SpeedTracker};
use crate::utils::file_manager::{safe_join, safe_join_remote};
use crate::utils::hash_verify;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter};
use tokio::sync::{Mutex, Semaphore};
use tracing::{error, info, warn};

const MAX_REPAIR_CONCURRENT_DOWNLOADS: usize = 4;
const REPAIR_PROGRESS_INTERVAL_MS: u64 = 250;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VerifyResult {
    pub total_files: usize,
    pub verified_ok: usize,
    pub redownloaded: usize,
    pub failed: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RepairResult {
    pub requested_files: usize,
    pub repaired_ok: usize,
    pub failed: Vec<String>,
}

#[derive(Debug, Clone)]
struct RepairTarget {
    dest: String,
    md5: String,
    sha256: Option<String>,
    size: u64,
}

struct DoneCounterGuard {
    counter: Arc<AtomicUsize>,
}

impl DoneCounterGuard {
    fn new(counter: Arc<AtomicUsize>) -> Self {
        Self { counter }
    }
}

impl Drop for DoneCounterGuard {
    fn drop(&mut self) {
        self.counter.fetch_add(1, Ordering::Relaxed);
    }
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

/// Verify game files — 两阶段校验 + 哈希缓存 + 并发
pub async fn verify_game_files(
    app: AppHandle,
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

    // ---- 第一阶段：快速 exists + size 筛选（纯校验，不重下载）----
    let mut need_hash: Vec<(usize, &ResourceFile)> = Vec::new();
    let mut phase1_failed: Vec<(String, u64)> = Vec::new();

    for (i, file) in resource_index.resource.iter().enumerate() {
        let file_path = match safe_join_remote(game_folder, &file.dest) {
            Ok(p) => p,
            Err(e) => {
                warn!("不安全的清单路径（计入失败）: {} ({})", file.dest, e);
                phase1_failed.push((format!("{} (unsafe path: {})", file.dest, e), file.size));
                continue;
            }
        };
        match tokio::fs::metadata(&file_path).await {
            Ok(meta) if meta.len() == file.size => {
                // size 匹配，需要进一步做 MD5 校验
                need_hash.push((i, file));
            }
            Ok(meta) => {
                // 文件存在但 size 不对，直接标记异常
                warn!(
                    "{} size mismatch (expected: {}, got: {})",
                    file.dest,
                    file.size,
                    meta.len()
                );
                phase1_failed.push((file.dest.clone(), file.size));
            }
            Err(_) => {
                // 文件不存在，直接标记异常
                warn!("{} 文件不存在", file.dest);
                phase1_failed.push((file.dest.clone(), file.size));
            }
        }
    }

    info!(
        "第一阶段完成: {} 个文件需 MD5/SHA256 校验, {} 个文件已标记异常（缺失/size 不匹配）",
        need_hash.len(),
        phase1_failed.len()
    );

    // 共享计数器（原子操作，并发安全）
    let phase1_failed_count = phase1_failed.len();
    let phase1_failed_size: u64 = phase1_failed.iter().map(|(_, size)| *size).sum();
    let verified_ok = Arc::new(AtomicUsize::new(0));
    let failed = Arc::new(Mutex::new(
        phase1_failed
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>(),
    ));
    let finished_size = Arc::new(AtomicU64::new(phase1_failed_size));
    let finished_count = Arc::new(AtomicUsize::new(phase1_failed_count));
    let speed_tracker = Arc::new(Mutex::new(SpeedTracker::new()));

    // 发射阶段一进度（防止 UI 长时间停在 0%）
    if phase1_failed_count > 0 {
        let mut st = speed_tracker.lock().await;
        let remaining = total_size.saturating_sub(phase1_failed_size);
        let progress = DownloadProgress {
            phase: "verify".to_string(),
            total_size,
            finished_size: phase1_failed_size,
            total_count: total_files,
            finished_count: phase1_failed_count,
            current_file: format!("已标记 {} 个异常文件", phase1_failed_count),
            speed_bps: st.speed_bps(),
            eta_seconds: st.eta_seconds(remaining),
        };
        app.emit("game-verify-progress", &progress).ok();
    }

    // 并发控制（SSD 友好，避免过度并发导致 IO 抖动）
    let semaphore = Arc::new(Semaphore::new(4));

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
        let verified_ok_c = verified_ok.clone();
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
                failed_c.lock().await.push(file_dest.clone());
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
    let fail = failed.lock().await.clone();
    let fs = finished_size.load(Ordering::Relaxed);
    let fc = finished_count.load(Ordering::Relaxed);

    // 最终进度兜底
    let mut st = speed_tracker.lock().await;
    let progress = DownloadProgress {
        phase: "verify".to_string(),
        total_size,
        finished_size: fs,
        total_count: total_files,
        finished_count: fc,
        current_file: "校验完成".to_string(),
        speed_bps: st.speed_bps(),
        eta_seconds: 0,
    };
    app.emit("game-verify-progress", &progress).ok();

    info!(
        "校验完成(纯校验模式): ok={}, redownloaded=0, failed={}",
        ok,
        fail.len()
    );

    Ok(VerifyResult {
        total_files,
        verified_ok: ok,
        redownloaded: 0,
        failed: fail,
    })
}

/// 仅修复指定异常文件（Kuro 清单）
pub async fn repair_game_files(
    app: AppHandle,
    launcher_info: &LauncherInfo,
    resource_index: &ResourceIndex,
    game_folder: &Path,
    files: &[String],
    cancel_token: Arc<Mutex<bool>>,
) -> Result<RepairResult, String> {
    if files.is_empty() {
        return Err("未提供需要修复的异常文件列表".to_string());
    }

    let mut resource_map: std::collections::HashMap<&str, &ResourceFile> =
        std::collections::HashMap::new();
    for item in &resource_index.resource {
        resource_map.insert(item.dest.as_str(), item);
    }

    let mut targets: Vec<RepairTarget> = Vec::new();
    let mut failed: Vec<String> = Vec::new();

    for raw in files {
        let key = raw.trim();
        if key.is_empty() {
            continue;
        }
        if let Some(item) = resource_map.get(key) {
            targets.push(RepairTarget {
                dest: item.dest.clone(),
                md5: item.md5.clone(),
                sha256: item.sha256.clone(),
                size: item.size,
            });
        } else {
            failed.push(format!("{} (当前清单中不存在)", key));
        }
    }

    if targets.is_empty() {
        return Ok(RepairResult {
            requested_files: files.len(),
            repaired_ok: 0,
            failed,
        });
    }

    let total_size: u64 = targets.iter().map(|f| f.size).sum();
    let total_count = targets.len();
    let repair_started_at = Instant::now();
    let repaired_ok = Arc::new(AtomicUsize::new(0));
    let finished_count = Arc::new(AtomicUsize::new(0));
    let streamed_downloaded = Arc::new(AtomicU64::new(0));
    let cancelled_shared = Arc::new(AtomicBool::new(false));
    let current_file = Arc::new(Mutex::new("准备修复异常文件".to_string()));
    let failed_shared = Arc::new(Mutex::new(failed));
    info!(
        "开始修复异常文件: requested={}, targets={}, total_size={} bytes, 并发={}",
        files.len(),
        total_count,
        total_size,
        MAX_REPAIR_CONCURRENT_DOWNLOADS
    );

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0")
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    // 先发一个初始进度，避免前端长时间显示“等待进度数据”
    let init_progress = DownloadProgress {
        phase: "verify".to_string(),
        total_size,
        finished_size: 0,
        total_count,
        finished_count: 0,
        current_file: "准备修复异常文件".to_string(),
        speed_bps: 0,
        eta_seconds: 0,
    };
    app.emit("game-verify-progress", &init_progress).ok();
    // 周期性上报进度（并发下载场景下统一汇总）
    let progress_app = app.clone();
    let progress_done = finished_count.clone();
    let progress_bytes = streamed_downloaded.clone();
    let progress_current = current_file.clone();
    let progress_handle = tokio::spawn(async move {
        let mut speed_tracker = SpeedTracker::new();
        let mut last_bytes = 0u64;
        loop {
            let done = progress_done.load(Ordering::Relaxed);
            let finished_size = progress_bytes.load(Ordering::Relaxed).min(total_size);
            let delta = finished_size.saturating_sub(last_bytes);
            last_bytes = finished_size;
            speed_tracker.record(delta);
            let remaining = total_size.saturating_sub(finished_size);
            let current = progress_current.lock().await.clone();
            let progress = DownloadProgress {
                phase: "verify".to_string(),
                total_size,
                finished_size,
                total_count,
                finished_count: done,
                current_file: current,
                speed_bps: speed_tracker.speed_bps(),
                eta_seconds: speed_tracker.eta_seconds(remaining),
            };
            progress_app.emit("game-verify-progress", &progress).ok();
            if done >= total_count {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(
                REPAIR_PROGRESS_INTERVAL_MS,
            ))
            .await;
        }
    });

    let sem = Arc::new(Semaphore::new(MAX_REPAIR_CONCURRENT_DOWNLOADS));
    let mut handles = Vec::with_capacity(total_count);
    for item in targets {
        let permit_sem = sem.clone();
        let client_c = client.clone();
        let launcher_cdn = launcher_info.cdn_url.clone();
        let launcher_base = launcher_info.resources_base_path.clone();
        let game_root = game_folder.to_path_buf();
        let cancel_c = cancel_token.clone();
        let repaired_ok_c = repaired_ok.clone();
        let done_c = finished_count.clone();
        let bytes_c = streamed_downloaded.clone();
        let current_c = current_file.clone();
        let failed_c = failed_shared.clone();
        let cancelled_c = cancelled_shared.clone();

        let handle = tokio::spawn(async move {
            let _permit = match permit_sem.acquire_owned().await {
                Ok(p) => p,
                Err(_) => return,
            };
            let _done_guard = DoneCounterGuard::new(done_c.clone());

            if *cancel_c.lock().await {
                cancelled_c.store(true, Ordering::Relaxed);
                return;
            }

            {
                let mut guard = current_c.lock().await;
                *guard = item.dest.clone();
            }

            let file_path = match safe_join(&game_root, &item.dest) {
                Ok(p) => p,
                Err(e) => {
                    failed_c
                        .lock()
                        .await
                        .push(format!("{} (unsafe path: {})", item.dest, e));
                    return;
                }
            };

            let url = build_resource_url(&launcher_cdn, &launcher_base, &item.dest);
            let bytes_cb = bytes_c.clone();
            let progress_callback = move |delta: u64| {
                bytes_cb.fetch_add(delta, Ordering::Relaxed);
            };
            let download_result = fetcher::download_with_resume(
                &client_c,
                &url,
                &file_path,
                true,
                Some(&progress_callback),
                Some(cancel_c.clone()),
            )
            .await;

            if let Err(e) = download_result {
                if e.to_ascii_lowercase().contains("cancel") {
                    cancelled_c.store(true, Ordering::Relaxed);
                    return;
                }
                failed_c.lock().await.push(item.dest.clone());
                return;
            }

            if *cancel_c.lock().await {
                cancelled_c.store(true, Ordering::Relaxed);
                return;
            }

            {
                let verify_ok = hash_verify::verify_file_integrity(
                    &file_path,
                    item.size,
                    item.sha256.as_deref(),
                    Some(item.md5.as_str()),
                )
                .await
                .is_ok();
                if verify_ok {
                    repaired_ok_c.fetch_add(1, Ordering::Relaxed);
                } else {
                    failed_c.lock().await.push(item.dest.clone());
                }
            }
        });
        handles.push(handle);
    }

    for h in handles {
        let _ = h.await;
    }
    let _ = progress_handle.await;

    if cancelled_shared.load(Ordering::Relaxed) || *cancel_token.lock().await {
        info!("修复任务已取消");
        return Err("Repair cancelled".to_string());
    }

    let final_progress = DownloadProgress {
        phase: "verify".to_string(),
        total_size,
        finished_size: total_size,
        total_count,
        finished_count: total_count,
        current_file: "修复完成".to_string(),
        speed_bps: 0,
        eta_seconds: 0,
    };
    app.emit("game-verify-progress", &final_progress).ok();

    let repaired_ok = repaired_ok.load(Ordering::Relaxed);
    let failed = failed_shared.lock().await.clone();
    let elapsed = repair_started_at.elapsed().as_secs_f64().max(0.001);
    let avg_bps = (streamed_downloaded.load(Ordering::Relaxed) as f64 / elapsed) as u64;
    info!(
        "修复完成: requested={}, repaired_ok={}, failed={}, avg_speed={} B/s",
        files.len(),
        repaired_ok,
        failed.len(),
        avg_bps
    );

    Ok(RepairResult {
        requested_files: files.len(),
        repaired_ok,
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
