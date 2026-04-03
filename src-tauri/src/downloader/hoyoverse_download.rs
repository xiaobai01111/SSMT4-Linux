use crate::downloader::hoyoverse::{self, GamePackage, Segment};
use crate::downloader::progress::{
    emit_game_download_snapshot, emit_game_download_state, DownloadProgress, DownloadProgressStats,
    SpeedTracker,
};
use crate::downloader::staging;
use crate::events::GameDownloadOperation;
use crate::utils::file_manager::{safe_join, safe_join_remote};
use crate::utils::hash_verify;
use futures_util::StreamExt;
use reqwest::Client;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::AppHandle;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tracing::{error, info, warn};

mod plan;

use plan::{execute_plan, DownloadPlan, PostInstall};

/// 最大并行下载数
const MAX_CONCURRENT_DOWNLOADS: usize = 4;
/// 单文件分片下载阈值（仅大文件启用）
const PARALLEL_CHUNK_THRESHOLD: u64 = 512 * 1024 * 1024;
/// 分片大小
const PARALLEL_CHUNK_SIZE: u64 = 16 * 1024 * 1024;
/// 单文件分片并发上限
const MAX_CONCURRENT_CHUNKS: usize = 8;
/// 文件校验并发上限（防止过度抢占 IO）
const MAX_VERIFY_CONCURRENCY: usize = 8;

struct DiskDownloadRequest<'a> {
    client: &'a Client,
    url: &'a str,
    dest: &'a Path,
    expected_size: u64,
    expected_sha256: &'a str,
    expected_md5: &'a str,
    shared_bytes: Arc<AtomicU64>,
    cancel_token: Arc<Mutex<bool>>,
    active_streams: Arc<AtomicUsize>,
}

#[derive(Clone)]
struct ExtractProgressContext {
    app: AppHandle,
    task_id: String,
    operation: GameDownloadOperation,
    label: String,
    total_segments: usize,
    current_segment: usize,
}

impl ExtractProgressContext {
    fn new(
        app: &AppHandle,
        task_id: &str,
        operation: GameDownloadOperation,
        label: &str,
        total_segments: usize,
        current_segment: usize,
    ) -> Self {
        Self {
            app: app.clone(),
            task_id: task_id.to_string(),
            operation,
            label: label.to_string(),
            total_segments,
            current_segment,
        }
    }

    fn emit_install_progress(
        &self,
        finished_size: u64,
        total_size: u64,
        current_file: impl Into<String>,
    ) {
        emit_operation_snapshot(
            &self.app,
            &self.task_id,
            self.operation,
            "install",
            current_file,
            DownloadProgressStats {
                finished_size,
                total_size,
                finished_count: self.current_segment,
                total_count: self.total_segments,
                ..DownloadProgressStats::default()
            },
        );
    }
}

fn emit_operation_progress(
    app: &AppHandle,
    task_id: &str,
    operation: GameDownloadOperation,
    progress: &DownloadProgress,
) {
    emit_game_download_state(app, task_id, operation, progress);
}

fn emit_operation_snapshot(
    app: &AppHandle,
    task_id: &str,
    operation: GameDownloadOperation,
    phase: &str,
    current_file: impl Into<String>,
    stats: DownloadProgressStats,
) {
    emit_game_download_snapshot(app, task_id, operation, phase, current_file, stats);
}

// ============================================================
// 全量下载：下载压缩包 → 解压到游戏目录
// ============================================================

pub async fn download_game(
    app: AppHandle,
    task_id: &str,
    game_pkg: &GamePackage,
    game_folder: &Path,
    languages: &[String],
    cancel_token: Arc<Mutex<bool>>,
) -> Result<(), String> {
    let segments = &game_pkg.main.major.game_pkgs;
    if segments.is_empty() {
        return Err("没有可用的游戏下载包".to_string());
    }

    std::fs::create_dir_all(game_folder).map_err(|e| format!("创建游戏目录失败: {}", e))?;

    let audio_segments = collect_audio_segments(&game_pkg.main.major.audio_pkgs, languages);

    let all_tasks: Vec<DownloadTask> = segments
        .iter()
        .enumerate()
        .map(|(i, s)| DownloadTask {
            url: s.url.clone(),
            md5: s.md5.clone(),
            sha256: s.sha256.clone(),
            size: s.size.clone(),
            label: format!("游戏本体 {}/{}", i + 1, segments.len()),
            filename: filename_from_url(&s.url),
        })
        .chain(audio_segments.iter().map(|(lang, seg)| DownloadTask {
            url: seg.url.clone(),
            md5: seg.md5.clone(),
            sha256: seg.sha256.clone(),
            size: seg.size.clone(),
            label: format!("语言包 ({})", lang),
            filename: filename_from_url(&seg.url),
        }))
        .collect();

    let plan = DownloadPlan {
        all_tasks,
        primary_pkg_count: segments.len(),
        primary_label: "游戏本体".to_string(),
        migrate_old_files: true,
        post_install: PostInstall::WriteVersion {
            version: game_pkg.main.major.version.clone(),
        },
    };

    execute_plan(
        app,
        task_id,
        GameDownloadOperation::DownloadGame,
        plan,
        game_folder,
        cancel_token,
    )
    .await
}

// ============================================================
// 增量更新：下载补丁 zip → 解压覆盖
// ============================================================

pub async fn update_game(
    app: AppHandle,
    task_id: &str,
    game_pkg: &GamePackage,
    local_version: &str,
    game_folder: &Path,
    languages: &[String],
    cancel_token: Arc<Mutex<bool>>,
) -> Result<(), String> {
    let patch = game_pkg
        .main
        .patches
        .iter()
        .find(|p| p.version == local_version)
        .ok_or_else(|| {
            format!(
                "未找到从版本 {} 到 {} 的增量补丁，请使用全量下载",
                local_version, game_pkg.main.major.version
            )
        })?;

    let audio_segments = collect_audio_segments(&patch.audio_pkgs, languages);

    let all_tasks: Vec<DownloadTask> = patch
        .game_pkgs
        .iter()
        .enumerate()
        .map(|(i, s)| DownloadTask {
            url: s.url.clone(),
            md5: s.md5.clone(),
            sha256: s.sha256.clone(),
            size: s.size.clone(),
            label: format!("游戏补丁 {}/{}", i + 1, patch.game_pkgs.len()),
            filename: filename_from_url(&s.url),
        })
        .chain(audio_segments.iter().map(|(lang, seg)| DownloadTask {
            url: seg.url.clone(),
            md5: seg.md5.clone(),
            sha256: seg.sha256.clone(),
            size: seg.size.clone(),
            label: format!("语言包补丁 ({})", lang),
            filename: filename_from_url(&seg.url),
        }))
        .collect();

    if all_tasks.is_empty() {
        return Err("补丁包为空".to_string());
    }

    let plan = DownloadPlan {
        all_tasks,
        primary_pkg_count: patch.game_pkgs.len(),
        primary_label: "游戏补丁".to_string(),
        migrate_old_files: false,
        post_install: PostInstall::PatchAndWriteVersion {
            version: game_pkg.main.major.version.clone(),
        },
    };

    execute_plan(
        app,
        task_id,
        GameDownloadOperation::UpdateGame,
        plan,
        game_folder,
        cancel_token,
    )
    .await
}

// ============================================================
// 文件校验：使用 res_list 对比 MD5
// ============================================================

pub async fn verify_game(
    app: AppHandle,
    task_id: &str,
    game_pkg: &GamePackage,
    game_folder: &Path,
    cancel_token: Arc<Mutex<bool>>,
) -> Result<crate::downloader::verifier::VerifyResult, String> {
    #[derive(Debug)]
    enum VerifyTaskState {
        Ok,
        Failed(String),
        Cancelled,
    }

    let res_list_url = &game_pkg.main.major.res_list_url;
    let mut verify_targets: Vec<VerifyTarget> = Vec::new();
    let mut verify_mode_label = "官方资源清单".to_string();

    match hoyoverse::fetch_resource_list(res_list_url).await {
        Ok(resource_list) if !resource_list.is_empty() => {
            for entry in resource_list {
                let file_path = match safe_join(game_folder, &entry.remote_name) {
                    Ok(p) => p,
                    Err(e) => {
                        warn!("跳过不安全的清单路径: {} ({})", entry.remote_name, e);
                        continue;
                    }
                };
                verify_targets.push(VerifyTarget {
                    display_name: entry.remote_name.clone(),
                    path: file_path,
                    expected_size: entry.file_size,
                    md5: entry.md5,
                    sha256: entry.sha256,
                });
            }
        }
        Ok(_) => {
            warn!("官方资源清单为空，回退到安装包缓存校验");
        }
        Err(e) => {
            warn!("解析官方资源清单失败，回退到安装包缓存校验: {}", e);
        }
    }

    if verify_targets.is_empty() {
        verify_mode_label = "本地 pkg_version 清单".to_string();
        verify_targets = collect_local_pkg_version_verify_targets(game_folder);
    }

    if verify_targets.is_empty() {
        verify_mode_label = "安装包缓存".to_string();
        verify_targets = collect_cached_archive_verify_targets(game_pkg, game_folder);
    }

    if verify_targets.is_empty() {
        return Err("无法获取官方校验清单，且未找到可校验的本地 pkg_version/安装包缓存文件。请切换服务器后刷新状态再试，或先使用官方启动器校验。".to_string());
    }

    let total_files = verify_targets.len();
    let total_size: u64 = verify_targets.iter().map(|r| r.expected_size).sum();

    info!(
        "开始校验 {} 个文件, 总大小 {} 字节, 模式={}",
        total_files, total_size, verify_mode_label
    );
    let verify_concurrency = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .max(2)
        .min(MAX_VERIFY_CONCURRENCY);
    info!("文件校验并发度: {}", verify_concurrency);

    let mut verified_ok: usize = 0;
    let redownloaded: usize = 0;
    let mut failed: Vec<String> = Vec::new();
    let mut finished_size: u64 = 0;
    let mut finished_count: usize = 0;
    let mut speed_tracker = SpeedTracker::new();
    let verify_stream = futures_util::stream::iter(verify_targets.into_iter().map(|entry| {
        let cancel_token = cancel_token.clone();
        async move {
            if *cancel_token.lock().await {
                return (
                    entry.display_name,
                    entry.expected_size,
                    VerifyTaskState::Cancelled,
                );
            }

            match hash_verify::verify_file_integrity(
                &entry.path,
                entry.expected_size,
                Some(entry.sha256.as_str()),
                Some(entry.md5.as_str()),
            )
            .await
            {
                Ok(_) => (entry.display_name, entry.expected_size, VerifyTaskState::Ok),
                Err(err) => (
                    entry.display_name,
                    entry.expected_size,
                    VerifyTaskState::Failed(err),
                ),
            }
        }
    }))
    .buffer_unordered(verify_concurrency);
    tokio::pin!(verify_stream);

    while let Some((display_name, expected_size, state)) = verify_stream.next().await {
        match state {
            VerifyTaskState::Ok => {
                verified_ok += 1;
            }
            VerifyTaskState::Failed(err) => {
                warn!("{} 校验不匹配: {}", display_name, err);
                // 对于 HoYoverse 游戏，目前不支持单文件重下载（需要从 zip 中提取）
                failed.push(display_name.clone());
            }
            VerifyTaskState::Cancelled => {
                return Err("Verification cancelled".to_string());
            }
        }

        finished_count += 1;
        finished_size += expected_size;
        speed_tracker.record(expected_size);

        let remaining = total_size.saturating_sub(finished_size);
        let progress = DownloadProgress {
            phase: "verify".to_string(),
            total_size,
            finished_size,
            total_count: total_files,
            finished_count,
            current_file: display_name,
            speed_bps: speed_tracker.speed_bps(),
            eta_seconds: speed_tracker.eta_seconds(remaining),
        };
        emit_operation_progress(&app, task_id, GameDownloadOperation::VerifyGame, &progress);
    }

    info!(
        "校验完成: 正常={}, 重新下载={}, 失败={}",
        verified_ok,
        redownloaded,
        failed.len()
    );

    Ok(crate::downloader::verifier::VerifyResult {
        total_files,
        verified_ok,
        redownloaded,
        failed,
    })
}

// ============================================================
// 内部辅助函数
// ============================================================

/// 并行友好的文件下载（支持断点续传），通过 AtomicU64 汇报进度
struct ActiveCounterGuard {
    counter: Arc<AtomicUsize>,
}

impl ActiveCounterGuard {
    fn new(counter: Arc<AtomicUsize>) -> Self {
        counter.fetch_add(1, Ordering::Relaxed);
        Self { counter }
    }
}

impl Drop for ActiveCounterGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::Relaxed);
    }
}

async fn supports_parallel_range_download(
    client: &Client,
    url: &str,
    active_streams: Arc<AtomicUsize>,
) -> bool {
    let _active_guard = ActiveCounterGuard::new(active_streams);
    match client
        .get(url)
        .header("User-Agent", "Mozilla/5.0")
        .header("Range", "bytes=0-0")
        .send()
        .await
    {
        Ok(resp) => resp.status().as_u16() == 206,
        Err(_) => false,
    }
}

async fn download_file_to_disk_chunked(
    client: &Client,
    url: &str,
    temp_path: &Path,
    expected_size: u64,
    shared_bytes: Arc<AtomicU64>,
    cancel_token: Arc<Mutex<bool>>,
    active_streams: Arc<AtomicUsize>,
) -> Result<(), String> {
    let file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(temp_path)
        .await
        .map_err(|e| format!("创建临时文件失败: {}", e))?;
    file.set_len(expected_size)
        .await
        .map_err(|e| format!("预分配临时文件失败: {}", e))?;
    drop(file);

    let chunk_count = ((expected_size + PARALLEL_CHUNK_SIZE - 1) / PARALLEL_CHUNK_SIZE) as usize;
    let sem = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_CHUNKS));
    let chunk_written = Arc::new(AtomicU64::new(0));
    let mut handles = Vec::with_capacity(chunk_count);

    for idx in 0..chunk_count {
        let sem = sem.clone();
        let cancel = cancel_token.clone();
        let client = client.clone();
        let active = active_streams.clone();
        let temp = temp_path.to_path_buf();
        let bytes_counter = shared_bytes.clone();
        let chunk_written_counter = chunk_written.clone();
        let url = url.to_string();
        let start = idx as u64 * PARALLEL_CHUNK_SIZE;
        let end = (start + PARALLEL_CHUNK_SIZE - 1).min(expected_size - 1);

        let handle = tokio::spawn(async move {
            let _permit = sem
                .acquire()
                .await
                .map_err(|e| format!("信号量错误: {}", e))?;
            if *cancel.lock().await {
                return Err("Download cancelled".to_string());
            }

            let range_header = format!("bytes={}-{}", start, end);
            let chunk_len = end - start + 1;

            let chunk = {
                let _active_guard = ActiveCounterGuard::new(active);
                let resp = client
                    .get(&url)
                    .header("User-Agent", "Mozilla/5.0")
                    .header("Range", range_header)
                    .send()
                    .await
                    .map_err(|e| format!("分片请求失败: {}", e))?;
                if resp.status().as_u16() != 206 {
                    return Err("RANGE_UNSUPPORTED".to_string());
                }
                let bytes = resp
                    .bytes()
                    .await
                    .map_err(|e| format!("分片读取失败: {}", e))?;
                if bytes.len() as u64 != chunk_len {
                    return Err(format!(
                        "分片长度异常: expected={}, actual={}",
                        chunk_len,
                        bytes.len()
                    ));
                }
                bytes
            };

            if *cancel.lock().await {
                return Err("Download cancelled".to_string());
            }

            let mut file = tokio::fs::OpenOptions::new()
                .write(true)
                .open(&temp)
                .await
                .map_err(|e| format!("打开临时文件失败: {}", e))?;
            file.seek(std::io::SeekFrom::Start(start))
                .await
                .map_err(|e| format!("分片 seek 失败: {}", e))?;
            file.write_all(&chunk)
                .await
                .map_err(|e| format!("分片写入失败: {}", e))?;

            let written = chunk.len() as u64;
            bytes_counter.fetch_add(written, Ordering::Relaxed);
            chunk_written_counter.fetch_add(written, Ordering::Relaxed);
            Ok::<(), String>(())
        });
        handles.push(handle);
    }

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
                    first_error = Some(format!("分片任务失败: {}", e));
                }
            }
        }
    }

    if let Some(err) = first_error {
        let wrote = chunk_written.load(Ordering::Relaxed);
        if wrote > 0 {
            shared_bytes.fetch_sub(wrote, Ordering::Relaxed);
        }
        tokio::fs::remove_file(temp_path).await.ok();
        return Err(err);
    }

    Ok(())
}

async fn promote_verified_temp_file(
    temp_path: &Path,
    dest: &Path,
    expected_size: u64,
    expected_sha256: Option<&str>,
    expected_md5: Option<&str>,
) -> Result<bool, String> {
    match hash_verify::verify_file_integrity(
        temp_path,
        expected_size,
        expected_sha256,
        expected_md5,
    )
    .await
    {
        Ok(_) => {
            tokio::fs::rename(temp_path, dest)
                .await
                .map_err(|e| format!("重命名临时文件失败: {}", e))?;
            Ok(true)
        }
        Err(err) => {
            warn!(
                "临时文件校验失败，删除损坏缓存后重新下载: {} ({})",
                temp_path.display(),
                err
            );
            tokio::fs::remove_file(temp_path).await.ok();
            Ok(false)
        }
    }
}

async fn download_file_to_disk(request: DiskDownloadRequest<'_>) -> Result<(), String> {
    let DiskDownloadRequest {
        client,
        url,
        dest,
        expected_size,
        expected_sha256,
        expected_md5,
        shared_bytes,
        cancel_token,
        active_streams,
    } = request;

    let temp_path = dest.with_file_name(format!(
        "{}.temp",
        dest.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("download")
    ));

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("创建目录失败: {}", e))?;
    }

    // 如果目标已存在（调用方已校验 MD5 不匹配），删除
    if dest.exists() {
        info!("删除旧文件准备重新下载: {}", dest.display());
        tokio::fs::remove_file(dest).await.ok();
    }

    // 断点续传
    let mut downloaded_bytes: u64 = 0;
    if temp_path.exists() {
        if let Ok(meta) = tokio::fs::metadata(&temp_path).await {
            downloaded_bytes = meta.len();
            info!(
                "断点续传 {} ({:.1} MB)",
                dest.display(),
                downloaded_bytes as f64 / 1048576.0
            );
            // 已有部分也计入共享进度
            shared_bytes.fetch_add(downloaded_bytes, Ordering::Relaxed);
            if expected_size > 0 && downloaded_bytes >= expected_size {
                if promote_verified_temp_file(
                    &temp_path,
                    dest,
                    expected_size,
                    Some(expected_sha256),
                    Some(expected_md5),
                )
                .await?
                {
                    return Ok(());
                }
                shared_bytes.fetch_sub(downloaded_bytes, Ordering::Relaxed);
                downloaded_bytes = 0;
            }
        }
    }

    if downloaded_bytes == 0 && expected_size >= PARALLEL_CHUNK_THRESHOLD {
        if supports_parallel_range_download(client, url, active_streams.clone()).await {
            info!(
                "启用分片并发下载: {} (size={:.1} MB, chunk={} MB, 并发={})",
                dest.display(),
                expected_size as f64 / 1048576.0,
                PARALLEL_CHUNK_SIZE / 1024 / 1024,
                MAX_CONCURRENT_CHUNKS
            );
            match download_file_to_disk_chunked(
                client,
                url,
                &temp_path,
                expected_size,
                shared_bytes.clone(),
                cancel_token.clone(),
                active_streams.clone(),
            )
            .await
            {
                Ok(()) => {
                    if let Err(err) = hash_verify::verify_file_integrity(
                        &temp_path,
                        expected_size,
                        Some(expected_sha256),
                        Some(expected_md5),
                    )
                    .await
                    {
                        tokio::fs::remove_file(&temp_path).await.ok();
                        return Err(format!("下载文件完整性校验失败: {} ({})", err, url));
                    }
                    tokio::fs::rename(&temp_path, dest)
                        .await
                        .map_err(|e| format!("重命名文件失败: {}", e))?;
                    return Ok(());
                }
                Err(e) => {
                    warn!("分片下载失败，回退到单连接流式下载: {}", e);
                }
            }
        }
    }

    let mut req = client.get(url).header("User-Agent", "Mozilla/5.0");
    if downloaded_bytes > 0 {
        req = req.header("Range", format!("bytes={}-", downloaded_bytes));
    }

    let _active_guard = ActiveCounterGuard::new(active_streams);
    let resp = req
        .send()
        .await
        .map_err(|e| format!("下载请求失败: {}", e))?;

    let status = resp.status().as_u16();
    match status {
        206 => {} // 断点续传
        200 => {
            if downloaded_bytes > 0 {
                // 回退已计入的字节
                shared_bytes.fetch_sub(downloaded_bytes, Ordering::Relaxed);
                warn!("服务器不支持断点续传，重新开始下载");
                downloaded_bytes = 0;
            }
        }
        416 => {
            if temp_path.exists() {
                if promote_verified_temp_file(
                    &temp_path,
                    dest,
                    expected_size,
                    Some(expected_sha256),
                    Some(expected_md5),
                )
                .await?
                {
                    return Ok(());
                }
                if downloaded_bytes > 0 {
                    shared_bytes.fetch_sub(downloaded_bytes, Ordering::Relaxed);
                }
                return Err(format!(
                    "HTTP 416 且临时文件校验失败，已清理损坏缓存: {}",
                    url
                ));
            }
            return Err(format!("HTTP 416 且无临时文件: {}", url));
        }
        _ => {
            return Err(format!("HTTP 错误 {} : {}", status, url));
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
        // 每秒检查一次取消状态（避免频繁锁竞争）
        if last_cancel_check.elapsed() > std::time::Duration::from_secs(1) {
            if *cancel_token.lock().await {
                return Err("Download cancelled".to_string());
            }
            last_cancel_check = std::time::Instant::now();
        }

        let chunk = chunk_result.map_err(|e: reqwest::Error| format!("流读取错误: {}", e))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("写入错误: {}", e))?;

        let len = chunk.len() as u64;
        #[allow(unused_assignments)]
        {
            downloaded_bytes += len;
        }
        shared_bytes.fetch_add(len, Ordering::Relaxed);
    }

    file.flush()
        .await
        .map_err(|e| format!("刷新缓冲失败: {}", e))?;
    drop(file);

    if let Err(err) = hash_verify::verify_file_integrity(
        &temp_path,
        expected_size,
        Some(expected_sha256),
        Some(expected_md5),
    )
    .await
    {
        tokio::fs::remove_file(&temp_path).await.ok();
        return Err(format!("下载文件完整性校验失败: {} ({})", err, url));
    }

    tokio::fs::rename(&temp_path, dest)
        .await
        .map_err(|e| format!("重命名文件失败: {}", e))?;

    Ok(())
}

/// 根据文件扩展名选择解压方式
async fn extract_archive(
    archive_path: &Path,
    dest_folder: &Path,
    progress_ctx: &ExtractProgressContext,
) -> Result<(), String> {
    let name = archive_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if name.contains(".7z") {
        extract_7z(archive_path, dest_folder, progress_ctx).await
    } else {
        extract_zip(archive_path, dest_folder, progress_ctx).await
    }
}

/// 使用系统 7z 命令解压（支持分卷 .7z.001 和单文件 .7z）
async fn extract_7z(
    archive_path: &Path,
    dest_folder: &Path,
    progress_ctx: &ExtractProgressContext,
) -> Result<(), String> {
    let archive_path = archive_path.to_path_buf();
    let dest_folder = dest_folder.to_path_buf();
    let progress_ctx = progress_ctx.clone();

    progress_ctx.emit_install_progress(0, 100, format!("正在解压 {}...", progress_ctx.label));

    info!(
        "7z 解压: {} → {}",
        archive_path.display(),
        dest_folder.display()
    );

    let mut child = tokio::process::Command::new("7z")
        .arg("x")
        .arg(&archive_path)
        .arg(format!("-o{}", dest_folder.display()))
        .arg("-aoa") // 覆盖已有文件
        .arg("-y") // 自动确认
        .arg("-bsp1") // 进度输出到 stderr
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("执行 7z 命令失败: {}。请确保已安装 p7zip-full。", e))?;

    // 从 stderr 读取进度
    if let Some(stderr) = child.stderr.take() {
        let progress_ctx_for_stderr = progress_ctx.clone();
        tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            let mut reader = stderr;
            let mut buf = vec![0u8; 4096];
            let mut last_report = std::time::Instant::now();
            let mut last_pct: u64 = 0;

            loop {
                match reader.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        let text = String::from_utf8_lossy(&buf[..n]);
                        // 解析 7z 进度百分比，格式如 " 42% 3 - filename" 或 "100%"
                        for part in text.split(['\r', '\n']) {
                            let trimmed = part.trim();
                            if let Some(pos) = trimmed.find('%') {
                                let before = trimmed[..pos].trim();
                                // 取 % 前面的最后一段数字
                                let num_str = before
                                    .rsplit(|c: char| !c.is_ascii_digit())
                                    .next()
                                    .unwrap_or("");
                                if let Ok(pct) = num_str.parse::<u64>() {
                                    if pct <= 100 {
                                        last_pct = pct;
                                    }
                                }
                            }
                        }
                        if last_report.elapsed() > std::time::Duration::from_millis(500) {
                            progress_ctx_for_stderr.emit_install_progress(
                                last_pct,
                                100,
                                format!("安装 {} ({}%)", progress_ctx_for_stderr.label, last_pct),
                            );
                            last_report = std::time::Instant::now();
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("等待 7z 进程失败: {}", e))?;

    if !status.success() {
        return Err(format!("7z 解压失败 (exit code: {:?})", status.code()));
    }

    // 发送完成事件
    progress_ctx.emit_install_progress(100, 100, format!("安装 {} 完成", progress_ctx.label));

    info!("7z 解压完成: {}", archive_path.display());
    Ok(())
}

/// 解压 zip 到目标目录（带进度上报）
async fn extract_zip(
    zip_path: &Path,
    dest_folder: &Path,
    progress_ctx: &ExtractProgressContext,
) -> Result<(), String> {
    let zip_path = zip_path.to_path_buf();
    let dest_folder = dest_folder.to_path_buf();
    let progress_ctx = progress_ctx.clone();

    progress_ctx.emit_install_progress(0, 0, format!("正在打开 {}...", progress_ctx.label));

    // zip 解压需要在阻塞线程中执行
    tokio::task::spawn_blocking(move || {
        let file =
            std::fs::File::open(&zip_path).map_err(|e| format!("打开 zip 文件失败: {}", e))?;
        let file_len = file.metadata().map(|m| m.len()).unwrap_or(0);
        info!(
            "开始解压: {} ({:.1} MB)",
            zip_path.display(),
            file_len as f64 / 1048576.0
        );

        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| format!("读取 zip 归档失败: {}", e))?;

        let entry_count = archive.len();
        let mut last_report = std::time::Instant::now();

        // 发送条目总数
        progress_ctx.emit_install_progress(
            0,
            entry_count as u64,
            format!("安装 {} (0%) 0/{}", progress_ctx.label, entry_count),
        );

        for i in 0..entry_count {
            let mut zip_file = archive
                .by_index(i)
                .map_err(|e| format!("读取 zip 条目 {} 失败: {}", i, e))?;

            let out_path = match zip_file.enclosed_name() {
                Some(path) => dest_folder.join(path),
                None => continue,
            };

            if zip_file.name().ends_with('/') {
                std::fs::create_dir_all(&out_path).ok();
            } else {
                if let Some(parent) = out_path.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                let mut outfile = std::fs::File::create(&out_path)
                    .map_err(|e| format!("创建文件失败 {}: {}", out_path.display(), e))?;
                std::io::copy(&mut zip_file, &mut outfile)
                    .map_err(|e| format!("解压文件失败 {}: {}", out_path.display(), e))?;
            }

            // 每 500ms 上报一次安装进度
            if last_report.elapsed() > std::time::Duration::from_millis(500) {
                let pct = if entry_count > 0 {
                    (i + 1) * 100 / entry_count
                } else {
                    0
                };
                progress_ctx.emit_install_progress(
                    (i + 1) as u64,
                    entry_count as u64,
                    format!(
                        "安装 {} ({}%) {}/{}",
                        progress_ctx.label,
                        pct,
                        i + 1,
                        entry_count
                    ),
                );
                last_report = std::time::Instant::now();
            }
        }

        info!("解压完成: {} 个条目", entry_count);
        Ok(())
    })
    .await
    .map_err(|e| format!("解压任务失败: {}", e))?
}

/// 处理 hdiff 补丁文件
async fn apply_hdiff_patches(game_folder: &Path) -> Result<(), String> {
    let hdiff_files = find_hdiff_files(game_folder);
    if hdiff_files.is_empty() {
        return Ok(());
    }

    info!("发现 {} 个 hdiff 补丁文件", hdiff_files.len());

    // 确保 hpatchz 可用
    let hpatchz = crate::downloader::incremental::ensure_hpatchz_public().await?;

    for hdiff_path in &hdiff_files {
        // hdiff 文件名格式: xxx.hdiff, 对应原文件 xxx
        let original_name = hdiff_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .replace(".hdiff", "");

        let original_path = hdiff_path
            .parent()
            .unwrap_or(game_folder)
            .join(&original_name);

        if !original_path.exists() {
            warn!("hdiff 原文件不存在: {}", original_path.display());
            tokio::fs::remove_file(hdiff_path).await.ok();
            continue;
        }

        let temp_output = original_path.with_extension("patched.tmp");

        info!(
            "应用 hdiff: {} + {} -> {}",
            original_path.display(),
            hdiff_path.display(),
            temp_output.display()
        );

        let output = tokio::process::Command::new(&hpatchz)
            .arg(&original_path)
            .arg(hdiff_path)
            .arg(&temp_output)
            .arg("-f")
            .output()
            .await
            .map_err(|e| format!("运行 hpatchz 失败: {}", e))?;

        if output.status.success() {
            tokio::fs::remove_file(&original_path).await.ok();
            tokio::fs::rename(&temp_output, &original_path).await.ok();
            tokio::fs::remove_file(hdiff_path).await.ok();
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("hpatchz 失败: {}", stderr);
            tokio::fs::remove_file(&temp_output).await.ok();
            tokio::fs::remove_file(hdiff_path).await.ok();
        }
    }

    Ok(())
}

/// 查找游戏目录中的 .hdiff 文件
fn find_hdiff_files(game_folder: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    if let Ok(walker) = walkdir::WalkDir::new(game_folder)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
    {
        for entry in walker {
            if entry.file_type().is_file() {
                if let Some(ext) = entry.path().extension() {
                    if ext == "hdiff" {
                        result.push(entry.path().to_path_buf());
                    }
                }
            }
        }
    }
    result
}

/// 清理 deletefiles.txt 中列出的旧文件
async fn cleanup_deleted_files(game_folder: &Path) {
    let delete_list = game_folder.join("deletefiles.txt");
    if !delete_list.exists() {
        return;
    }

    if let Ok(content) = tokio::fs::read_to_string(&delete_list).await {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let file_path = match safe_join_remote(game_folder, line) {
                Ok(p) => p,
                Err(e) => {
                    warn!("跳过不安全的删除路径: {} ({})", line, e);
                    continue;
                }
            };
            if file_path.exists() {
                info!("删除旧文件: {}", line);
                tokio::fs::remove_file(&file_path).await.ok();
            }
        }
    }

    tokio::fs::remove_file(&delete_list).await.ok();
}

// ============================================================
// 辅助结构体和函数
// ============================================================

/// 下载任务描述
struct DownloadTask {
    url: String,
    md5: String,
    sha256: String,
    size: String,
    label: String,
    /// 从 URL 提取的原始文件名（如 StarRail_4.0.0.7z.001, Chinese.7z）
    filename: String,
}

struct VerifyTarget {
    display_name: String,
    path: PathBuf,
    expected_size: u64,
    md5: String,
    sha256: String,
}

/// 构造 mtime 缓存键："filename:size:mtime_secs"
fn make_hash_cache_key(path: &Path, size: u64) -> String {
    let mtime = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .and_then(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .map_err(std::io::Error::other)
        })
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    format!("{}:{}:{}", name, size, mtime)
}

/// 从 URL 中提取文件名
fn filename_from_url(url: &str) -> String {
    url.rsplit('/').next().unwrap_or("unknown").to_string()
}

fn collect_cached_archive_verify_targets(
    game_pkg: &GamePackage,
    game_folder: &Path,
) -> Vec<VerifyTarget> {
    let mut targets = Vec::new();

    for (idx, seg) in game_pkg.main.major.game_pkgs.iter().enumerate() {
        let filename = filename_from_url(&seg.url);
        let path = game_folder.join(&filename);
        if !path.exists() {
            continue;
        }
        let expected_size = seg.size.parse::<u64>().unwrap_or(0);
        targets.push(VerifyTarget {
            display_name: format!("游戏本体包#{:02} ({})", idx + 1, filename),
            path,
            expected_size,
            md5: seg.md5.clone(),
            sha256: seg.sha256.clone(),
        });
    }

    for audio in &game_pkg.main.major.audio_pkgs {
        let filename = filename_from_url(&audio.url);
        let path = game_folder.join(&filename);
        if !path.exists() {
            continue;
        }
        let expected_size = audio.size.parse::<u64>().unwrap_or(0);
        targets.push(VerifyTarget {
            display_name: format!("语言包({}) ({})", audio.language, filename),
            path,
            expected_size,
            md5: audio.md5.clone(),
            sha256: audio.sha256.clone(),
        });
    }

    targets
}

fn find_local_pkg_version_file(game_folder: &Path) -> Option<PathBuf> {
    let direct = game_folder.join("pkg_version");
    if direct.is_file() {
        return Some(direct);
    }

    // 常见结构：<game_root>/<subdir>/pkg_version
    if let Ok(entries) = std::fs::read_dir(game_folder) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let candidate = path.join("pkg_version");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

fn collect_local_pkg_version_verify_targets(game_folder: &Path) -> Vec<VerifyTarget> {
    let Some(pkg_version_path) = find_local_pkg_version_file(game_folder) else {
        return Vec::new();
    };
    let Ok(content) = std::fs::read_to_string(&pkg_version_path) else {
        return Vec::new();
    };
    let base_dir = pkg_version_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| game_folder.to_path_buf());

    let mut targets = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<hoyoverse::ResourceEntry>(line) else {
            continue;
        };
        let Ok(path) = safe_join(&base_dir, &entry.remote_name) else {
            continue;
        };
        targets.push(VerifyTarget {
            display_name: entry.remote_name,
            path,
            expected_size: entry.file_size,
            md5: entry.md5,
            sha256: entry.sha256,
        });
    }
    targets
}

/// 从 audio_pkgs 中筛选用户选中的语言包
fn collect_audio_segments(
    audio_pkgs: &[crate::downloader::hoyoverse::AudioPkg],
    languages: &[String],
) -> Vec<(String, Segment)> {
    if languages.is_empty() {
        return Vec::new();
    }

    audio_pkgs
        .iter()
        .filter(|pkg| languages.contains(&pkg.language))
        .map(|pkg| {
            (
                pkg.language.clone(),
                Segment {
                    url: pkg.url.clone(),
                    md5: pkg.md5.clone(),
                    sha256: pkg.sha256.clone(),
                    size: pkg.size.clone(),
                    decompressed_size: pkg.decompressed_size.clone(),
                },
            )
        })
        .collect()
}

fn create_install_staging_dir(game_folder: &Path, tag: &str) -> PathBuf {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let tag: String = tag
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    game_folder.join(".ssmt4_staging").join(format!(
        "{}-{}",
        if tag.is_empty() { "install" } else { &tag },
        ts
    ))
}

fn normalize_digest(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_ascii_lowercase())
    }
}

fn digest_token_from_expected(expected_sha256: &str, expected_md5: &str) -> Option<String> {
    if let Some(sha) = normalize_digest(expected_sha256) {
        return Some(format!("sha256:{}", sha));
    }
    normalize_digest(expected_md5).map(|md5| format!("md5:{}", md5))
}

fn task_digest_token(task: &DownloadTask) -> Option<String> {
    digest_token_from_expected(&task.sha256, &task.md5)
}

fn task_has_checksum(task: &DownloadTask) -> bool {
    task_digest_token(task).is_some()
}

fn digest_token_matches(cached: &str, task: &DownloadTask) -> bool {
    let Some(expected_token) = task_digest_token(task) else {
        return false;
    };
    let cached_normalized = cached.trim().to_ascii_lowercase();
    if cached_normalized.contains(':') {
        return cached_normalized == expected_token;
    }
    // 兼容旧格式（仅存裸 md5）
    if let Some(expected_md5) = normalize_digest(&task.md5) {
        return cached_normalized == expected_md5;
    }
    false
}

async fn compute_digest_token_for_expected(
    path: &Path,
    expected_sha256: &str,
    expected_md5: &str,
) -> Result<String, String> {
    if normalize_digest(expected_sha256).is_some() {
        let actual = hash_verify::sha256_file(path).await?;
        return Ok(format!("sha256:{}", actual.to_ascii_lowercase()));
    }
    if normalize_digest(expected_md5).is_some() {
        let actual = hash_verify::md5_file(path).await?;
        return Ok(format!("md5:{}", actual.to_ascii_lowercase()));
    }
    Ok(String::new())
}

fn emit_progress(
    app: &AppHandle,
    task_id: &str,
    operation: GameDownloadOperation,
    phase: &str,
    stats: DownloadProgressStats,
    current_file: impl Into<String>,
) {
    emit_operation_snapshot(app, task_id, operation, phase, current_file, stats);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let unique = format!(
            "{}-{}-{}",
            prefix,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        std::env::temp_dir().join(unique)
    }

    #[test]
    fn filename_from_url_extracts_last_path_segment() {
        assert_eq!(
            filename_from_url("https://example.com/packages/game/audio_cn.7z.001"),
            "audio_cn.7z.001"
        );
    }

    #[test]
    fn collect_audio_segments_filters_to_requested_languages() {
        let audio_pkgs = vec![
            hoyoverse::AudioPkg {
                language: "Chinese".to_string(),
                url: "https://cdn.example.com/chs.7z".to_string(),
                md5: "md5-chs".to_string(),
                sha256: "sha-chs".to_string(),
                size: "10".to_string(),
                decompressed_size: "20".to_string(),
            },
            hoyoverse::AudioPkg {
                language: "English".to_string(),
                url: "https://cdn.example.com/en.7z".to_string(),
                md5: "md5-en".to_string(),
                sha256: "sha-en".to_string(),
                size: "30".to_string(),
                decompressed_size: "40".to_string(),
            },
        ];

        let segments = collect_audio_segments(
            &audio_pkgs,
            &["English".to_string(), "Japanese".to_string()],
        );

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].0, "English");
        assert_eq!(segments[0].1.url, "https://cdn.example.com/en.7z");
        assert_eq!(segments[0].1.sha256, "sha-en");
    }

    #[test]
    fn create_install_staging_dir_uses_hidden_root_and_sanitized_tag() {
        let game_folder = Path::new("/tmp/ssmt4-game");
        let staging_dir = create_install_staging_dir(game_folder, "global/cn mix");

        assert!(staging_dir.starts_with(game_folder.join(".ssmt4_staging")));
        let name = staging_dir
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap();
        assert!(name.starts_with("global_cn_mix-"));
    }

    #[test]
    fn digest_token_helpers_prefer_sha256_and_support_legacy_md5_cache() {
        let task = DownloadTask {
            url: "https://cdn.example.com/game.7z".to_string(),
            md5: "ABCD1234".to_string(),
            sha256: "EEFF0011".to_string(),
            size: "1024".to_string(),
            label: "game".to_string(),
            filename: "game.7z".to_string(),
        };

        assert_eq!(
            digest_token_from_expected("EEFF0011", "ABCD1234").as_deref(),
            Some("sha256:eeff0011")
        );
        assert_eq!(task_digest_token(&task).as_deref(), Some("sha256:eeff0011"));
        assert!(task_has_checksum(&task));
        assert!(digest_token_matches("sha256:EEFF0011", &task));

        let legacy_md5_task = DownloadTask {
            sha256: String::new(),
            ..task
        };
        assert!(digest_token_matches("abcd1234", &legacy_md5_task));
    }

    #[test]
    fn collect_local_pkg_version_verify_targets_reads_nested_pkg_version() {
        let game_folder = unique_temp_dir("ssmt4-pkg-version");
        let install_root = game_folder.join("game");
        let data_root = install_root.join("Game_Data");
        std::fs::create_dir_all(&data_root).unwrap();

        let pkg_version = data_root.join("pkg_version");
        let payload = hoyoverse::ResourceEntry {
            remote_name: "StreamingAssets/data.bin".to_string(),
            md5: "md5-test".to_string(),
            sha256: "sha-test".to_string(),
            file_size: 4096,
        };
        std::fs::write(
            &pkg_version,
            format!("{}\n", serde_json::to_string(&payload).unwrap()),
        )
        .unwrap();

        let targets = collect_local_pkg_version_verify_targets(&install_root);

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].display_name, "StreamingAssets/data.bin");
        assert_eq!(
            targets[0].path,
            data_root.join("StreamingAssets").join("data.bin")
        );
        assert_eq!(targets[0].expected_size, 4096);
        assert_eq!(targets[0].md5, "md5-test");
        assert_eq!(targets[0].sha256, "sha-test");

        let _ = std::fs::remove_dir_all(&game_folder);
    }

    #[tokio::test]
    async fn promote_verified_temp_file_removes_corrupt_complete_temp() {
        let dir = unique_temp_dir("ssmt4-corrupt-temp");
        std::fs::create_dir_all(&dir).unwrap();
        let temp_path = dir.join("game.7z.temp");
        let dest_path = dir.join("game.7z");
        std::fs::write(&temp_path, b"HELLO").unwrap();

        let promoted = promote_verified_temp_file(
            &temp_path,
            &dest_path,
            5,
            None,
            Some("5d41402abc4b2a76b9719d911017c592"),
        )
        .await
        .unwrap();

        assert!(!promoted);
        assert!(!temp_path.exists());
        assert!(!dest_path.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn promote_verified_temp_file_promotes_valid_complete_temp() {
        let dir = unique_temp_dir("ssmt4-valid-temp");
        std::fs::create_dir_all(&dir).unwrap();
        let temp_path = dir.join("game.7z.temp");
        let dest_path = dir.join("game.7z");
        std::fs::write(&temp_path, b"hello").unwrap();

        let promoted = promote_verified_temp_file(
            &temp_path,
            &dest_path,
            5,
            None,
            Some("5d41402abc4b2a76b9719d911017c592"),
        )
        .await
        .unwrap();

        assert!(promoted);
        assert!(!temp_path.exists());
        assert_eq!(std::fs::read(&dest_path).unwrap(), b"hello");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
