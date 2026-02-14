use crate::downloader::hoyoverse::{self, GamePackage, Segment};
use crate::downloader::progress::{DownloadProgress, SpeedTracker};
use crate::utils::hash_verify;
use futures_util::StreamExt;
use reqwest::Client;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

/// 最大并行下载数
const MAX_CONCURRENT_DOWNLOADS: usize = 4;

// ============================================================
// 全量下载：下载压缩包 → 解压到游戏目录
// 支持 7z 分卷（.7z.001）、单文件 7z、zip
// ============================================================

pub async fn download_game(
    app: AppHandle,
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

    // 收集所有需要下载的包：游戏本体 + 语言包
    let audio_segments = collect_audio_segments(&game_pkg.main.major.audio_pkgs, languages);
    let game_pkg_count = segments.len();

    let all_tasks: Vec<DownloadTask> = segments
        .iter()
        .enumerate()
        .map(|(i, s)| DownloadTask {
            url: s.url.clone(),
            md5: s.md5.clone(),
            size: s.size.clone(),
            label: format!("游戏本体 {}/{}", i + 1, segments.len()),
            filename: filename_from_url(&s.url),
        })
        .chain(audio_segments.iter().map(|(lang, seg)| DownloadTask {
            url: seg.url.clone(),
            md5: seg.md5.clone(),
            size: seg.size.clone(),
            label: format!("语言包 ({})", lang),
            filename: filename_from_url(&seg.url),
        }))
        .collect();

    let total_size: u64 = all_tasks
        .iter()
        .filter_map(|s| s.size.parse::<u64>().ok())
        .sum();
    let total_count = all_tasks.len();

    info!(
        "开始全量下载: {} 个包 (含 {} 个语言包), 总大小 {} 字节",
        total_count,
        audio_segments.len(),
        total_size
    );

    // 加载下载状态（JSON 缓存）
    let mut state = read_download_state(game_folder);
    if state.expected_total_size != 0 && state.expected_total_size != total_size {
        info!("下载任务变更 (total_size 不同)，重置状态");
        state = DownloadState::default();
    }
    state.expected_total_size = total_size;

    // ===== 阶段 1: 全部下载 =====
    // 1a: 预检查（缓存/迁移/MD5校验）- 顺序执行
    let mut cached_size: u64 = 0;
    let mut to_download: Vec<usize> = Vec::new();

    for (i, task) in all_tasks.iter().enumerate() {
        if *cancel_token.lock().await {
            return Err("Download cancelled".to_string());
        }

        let dest = game_folder.join(&task.filename);

        // 检查 JSON 缓存（key = 文件名）
        if let Some(cached_md5) = state.checksums.get(&task.filename) {
            if !task.md5.is_empty() && cached_md5.to_lowercase() == task.md5.to_lowercase() {
                let file_size = if dest.exists() {
                    tokio::fs::metadata(&dest).await.map(|m| m.len()).unwrap_or(0)
                } else {
                    task.size.parse::<u64>().unwrap_or(0)
                };
                cached_size += file_size;
                info!("分段 {} ({}) 已缓存，跳过下载", i, task.filename);
                emit_progress(
                    &app, "download", cached_size, total_size,
                    total_count, i + 1,
                    &format!("已缓存，跳过 {}", task.label),
                    0, 0,
                );
                continue;
            }
        }

        // 迁移旧格式文件: _download_N.zip → 原始文件名
        let old_file = game_folder.join(format!("_download_{}.zip", i));
        if !dest.exists() && old_file.exists() && !task.md5.is_empty() {
            let file_size = tokio::fs::metadata(&old_file).await.map(|m| m.len()).unwrap_or(0);
            emit_progress(
                &app, "download", cached_size, total_size,
                total_count, i + 1,
                &format!("校验旧文件 {} ({:.1} MB)...", task.label, file_size as f64 / 1048576.0),
                0, 0,
            );
            let actual_md5 = hash_verify::md5_file(&old_file).await.unwrap_or_default();
            if actual_md5.to_lowercase() == task.md5.to_lowercase() {
                info!("迁移旧文件 {} → {}", old_file.display(), dest.display());
                tokio::fs::rename(&old_file, &dest).await.ok();
                state.checksums.insert(task.filename.clone(), task.md5.clone());
                save_download_state(game_folder, &state);
                cached_size += file_size;
                emit_progress(
                    &app, "download", cached_size, total_size,
                    total_count, i + 1,
                    &format!("已迁移，跳过 {}", task.label),
                    0, 0,
                );
                continue;
            } else {
                warn!("旧文件 MD5 不匹配，将重新下载");
            }
        }

        // 无缓存但正确文件名已存在 → 计算 MD5 补录
        if dest.exists() && !task.md5.is_empty() {
            let file_size = tokio::fs::metadata(&dest).await.map(|m| m.len()).unwrap_or(0);
            emit_progress(
                &app, "download", cached_size, total_size,
                total_count, i + 1,
                &format!("校验已有文件 {} ({:.1} MB)...", task.label, file_size as f64 / 1048576.0),
                0, 0,
            );
            let actual_md5 = hash_verify::md5_file(&dest).await.unwrap_or_default();
            if actual_md5.to_lowercase() == task.md5.to_lowercase() {
                info!("文件 {} MD5 匹配，补录缓存", task.filename);
                state.checksums.insert(task.filename.clone(), task.md5.clone());
                save_download_state(game_folder, &state);
                cached_size += file_size;
                emit_progress(
                    &app, "download", cached_size, total_size,
                    total_count, i + 1,
                    &format!("已缓存，跳过 {}", task.label),
                    0, 0,
                );
                continue;
            } else {
                warn!("文件 {} MD5 不匹配，将重新下载", task.filename);
            }
        }

        to_download.push(i);
    }

    // 1b: 并行下载
    if !to_download.is_empty() {
        info!(
            "开始并行下载 {} 个文件 (最多 {} 并发)",
            to_download.len(),
            MAX_CONCURRENT_DOWNLOADS
        );

        let client = Client::builder()
            .pool_max_idle_per_host(MAX_CONCURRENT_DOWNLOADS + 2)
            .build()
            .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

        let shared_bytes = Arc::new(AtomicU64::new(cached_size));
        let shared_done = Arc::new(AtomicUsize::new(total_count - to_download.len()));

        // 进度上报任务
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
                        &app,
                        "download",
                        current,
                        total_size,
                        total_count,
                        done.load(Ordering::Relaxed),
                        &format!("并行下载中 ({}路)", MAX_CONCURRENT_DOWNLOADS),
                        tracker.speed_bps(),
                        tracker.eta_seconds(remaining),
                    );
                }
            })
        };

        // 启动下载任务
        let sem = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_DOWNLOADS));
        let mut handles = Vec::new();

        for &idx in &to_download {
            let task = &all_tasks[idx];
            let sem = sem.clone();
            let client = client.clone();
            let cancel = cancel_token.clone();
            let bytes = shared_bytes.clone();
            let done = shared_done.clone();
            let dest = game_folder.join(&task.filename);
            let url = task.url.clone();
            let md5 = task.md5.clone();
            let filename = task.filename.clone();

            let handle = tokio::spawn(async move {
                let _permit = sem
                    .acquire()
                    .await
                    .map_err(|e| format!("信号量错误: {}", e))?;
                info!("开始下载: {}", filename);
                download_file_to_disk(&client, &url, &dest, &md5, bytes, cancel).await?;
                done.fetch_add(1, Ordering::Relaxed);
                let final_md5 = if !md5.is_empty() {
                    md5
                } else {
                    hash_verify::md5_file(&dest).await.unwrap_or_default()
                };
                info!("下载完成: {}", filename);
                Ok::<(String, String), String>((filename, final_md5))
            });
            handles.push(handle);
        }

        // 等待所有下载完成
        let mut first_error: Option<String> = None;
        for handle in handles {
            match handle.await {
                Ok(Ok((filename, md5))) => {
                    state.checksums.insert(filename, md5);
                    save_download_state(game_folder, &state);
                }
                Ok(Err(e)) => {
                    if first_error.is_none() {
                        first_error = Some(e);
                    }
                }
                Err(e) => {
                    if first_error.is_none() {
                        first_error = Some(format!("下载任务失败: {}", e));
                    }
                }
            }
        }

        reporter.abort();

        if let Some(e) = first_error {
            return Err(e);
        }
    }

    info!("全部 {} 个压缩包下载完成，开始安装...", total_count);

    // ===== 阶段 2: 安装 =====
    // 2a: 游戏本体（可能是 7z 分卷，只需从第一个 part 解压一次）
    let first_game_file = &all_tasks[0].filename;
    if !state.installed_archives.contains(first_game_file) {
        let first_part = game_folder.join(first_game_file);
        if first_part.exists() {
            info!("安装游戏本体 (从 {})", first_game_file);
            match extract_archive(
                &first_part, game_folder, &app, "游戏本体",
                total_count, 1,
            ).await {
                Ok(()) => {
                    // 删除所有游戏本体分段文件
                    for task in &all_tasks[..game_pkg_count] {
                        tokio::fs::remove_file(game_folder.join(&task.filename)).await.ok();
                    }
                    state.installed_archives.push(first_game_file.to_string());
                    save_download_state(game_folder, &state);
                }
                Err(e) => {
                    error!("游戏本体安装失败: {}", e);
                    // 清除所有游戏段缓存
                    for task in &all_tasks[..game_pkg_count] {
                        state.checksums.remove(&task.filename);
                        tokio::fs::remove_file(game_folder.join(&task.filename)).await.ok();
                    }
                    save_download_state(game_folder, &state);
                    return Err(format!(
                        "游戏本体安装失败: {}。已删除文件，请重新下载。", e
                    ));
                }
            }
        } else {
            warn!("游戏本体首段文件不存在: {}", first_part.display());
        }
    } else {
        info!("游戏本体已安装，跳过");
    }

    // 2b: 语言包（每个单独解压）
    for (idx, task) in all_tasks[game_pkg_count..].iter().enumerate() {
        if state.installed_archives.contains(&task.filename) {
            info!("语言包 {} 已安装，跳过", task.filename);
            continue;
        }

        if *cancel_token.lock().await {
            return Err("Download cancelled".to_string());
        }

        let archive = game_folder.join(&task.filename);
        if !archive.exists() {
            info!("语言包文件不存在 (可能已安装): {}", task.filename);
            continue;
        }

        info!("安装语言包: {}", task.filename);
        match extract_archive(
            &archive, game_folder, &app, &task.label,
            total_count, game_pkg_count + idx + 1,
        ).await {
            Ok(()) => {
                tokio::fs::remove_file(&archive).await.ok();
                state.installed_archives.push(task.filename.clone());
                save_download_state(game_folder, &state);
            }
            Err(e) => {
                error!("语言包安装失败: {}", e);
                tokio::fs::remove_file(&archive).await.ok();
                state.checksums.remove(&task.filename);
                save_download_state(game_folder, &state);
                return Err(format!(
                    "安装 {} 失败: {}。已删除文件，请重新下载。",
                    task.label, e
                ));
            }
        }
    }

    // 全部完成，清除状态文件
    clear_download_state(game_folder);

    // 写入版本号
    hoyoverse::write_local_version(game_folder, &game_pkg.main.major.version)?;

    info!("全量下载安装完成");
    Ok(())
}

// ============================================================
// 增量更新：下载补丁 zip → 解压覆盖
// ============================================================

pub async fn update_game(
    app: AppHandle,
    game_pkg: &GamePackage,
    local_version: &str,
    game_folder: &Path,
    languages: &[String],
    cancel_token: Arc<Mutex<bool>>,
) -> Result<(), String> {
    // 查找匹配当前版本的补丁
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

    // 收集所有需要下载的包：游戏补丁 + 语言包补丁
    let audio_segments = collect_audio_segments(&patch.audio_pkgs, languages);
    let patch_pkg_count = patch.game_pkgs.len();

    let all_tasks: Vec<DownloadTask> = patch
        .game_pkgs
        .iter()
        .enumerate()
        .map(|(i, s)| DownloadTask {
            url: s.url.clone(),
            md5: s.md5.clone(),
            size: s.size.clone(),
            label: format!("游戏补丁 {}/{}", i + 1, patch.game_pkgs.len()),
            filename: filename_from_url(&s.url),
        })
        .chain(audio_segments.iter().map(|(lang, seg)| DownloadTask {
            url: seg.url.clone(),
            md5: seg.md5.clone(),
            size: seg.size.clone(),
            label: format!("语言包补丁 ({})", lang),
            filename: filename_from_url(&seg.url),
        }))
        .collect();

    if all_tasks.is_empty() {
        return Err("补丁包为空".to_string());
    }

    let total_size: u64 = all_tasks
        .iter()
        .filter_map(|s| s.size.parse::<u64>().ok())
        .sum();
    let total_count = all_tasks.len();

    info!(
        "开始增量更新 {} → {}: {} 个包 (含 {} 个语言包补丁), {} 字节",
        local_version,
        game_pkg.main.major.version,
        total_count,
        audio_segments.len(),
        total_size
    );

    // 加载下载状态（JSON 缓存）
    let mut state = read_download_state(game_folder);
    if state.expected_total_size != 0 && state.expected_total_size != total_size {
        info!("更新任务变更，重置状态");
        state = DownloadState::default();
    }
    state.expected_total_size = total_size;

    // ===== 阶段 1: 全部下载 =====
    // 1a: 预检查 - 顺序执行
    let mut cached_size: u64 = 0;
    let mut to_download: Vec<usize> = Vec::new();

    for (i, task) in all_tasks.iter().enumerate() {
        if *cancel_token.lock().await {
            return Err("Download cancelled".to_string());
        }

        let dest = game_folder.join(&task.filename);

        // 检查 JSON 缓存
        if let Some(cached_md5) = state.checksums.get(&task.filename) {
            if !task.md5.is_empty() && cached_md5.to_lowercase() == task.md5.to_lowercase() {
                let file_size = if dest.exists() {
                    tokio::fs::metadata(&dest).await.map(|m| m.len()).unwrap_or(0)
                } else {
                    task.size.parse::<u64>().unwrap_or(0)
                };
                cached_size += file_size;
                info!("补丁 {} ({}) 已缓存，跳过下载", i, task.filename);
                emit_progress(
                    &app, "download", cached_size, total_size,
                    total_count, i + 1,
                    &format!("已缓存，跳过 {}", task.label),
                    0, 0,
                );
                continue;
            }
        }

        // 无缓存但文件已存在 → 计算 MD5 补录
        if dest.exists() && !task.md5.is_empty() {
            let file_size = tokio::fs::metadata(&dest).await.map(|m| m.len()).unwrap_or(0);
            emit_progress(
                &app, "download", cached_size, total_size,
                total_count, i + 1,
                &format!("校验已有文件 {} ({:.1} MB)...", task.label, file_size as f64 / 1048576.0),
                0, 0,
            );
            let actual_md5 = hash_verify::md5_file(&dest).await.unwrap_or_default();
            if actual_md5.to_lowercase() == task.md5.to_lowercase() {
                info!("补丁 {} MD5 匹配，补录缓存", task.filename);
                state.checksums.insert(task.filename.clone(), task.md5.clone());
                save_download_state(game_folder, &state);
                cached_size += file_size;
                emit_progress(
                    &app, "download", cached_size, total_size,
                    total_count, i + 1,
                    &format!("已缓存，跳过 {}", task.label),
                    0, 0,
                );
                continue;
            } else {
                warn!("补丁 {} MD5 不匹配，将重新下载", task.filename);
            }
        }

        to_download.push(i);
    }

    // 1b: 并行下载
    if !to_download.is_empty() {
        info!(
            "开始并行下载 {} 个补丁 (最多 {} 并发)",
            to_download.len(),
            MAX_CONCURRENT_DOWNLOADS
        );

        let client = Client::builder()
            .pool_max_idle_per_host(MAX_CONCURRENT_DOWNLOADS + 2)
            .build()
            .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

        let shared_bytes = Arc::new(AtomicU64::new(cached_size));
        let shared_done = Arc::new(AtomicUsize::new(total_count - to_download.len()));

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
                        &app,
                        "download",
                        current,
                        total_size,
                        total_count,
                        done.load(Ordering::Relaxed),
                        &format!("并行下载中 ({}路)", MAX_CONCURRENT_DOWNLOADS),
                        tracker.speed_bps(),
                        tracker.eta_seconds(remaining),
                    );
                }
            })
        };

        let sem = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_DOWNLOADS));
        let mut handles = Vec::new();

        for &idx in &to_download {
            let task = &all_tasks[idx];
            let sem = sem.clone();
            let client = client.clone();
            let cancel = cancel_token.clone();
            let bytes = shared_bytes.clone();
            let done = shared_done.clone();
            let dest = game_folder.join(&task.filename);
            let url = task.url.clone();
            let md5 = task.md5.clone();
            let filename = task.filename.clone();

            let handle = tokio::spawn(async move {
                let _permit = sem
                    .acquire()
                    .await
                    .map_err(|e| format!("信号量错误: {}", e))?;
                info!("开始下载: {}", filename);
                download_file_to_disk(&client, &url, &dest, &md5, bytes, cancel).await?;
                done.fetch_add(1, Ordering::Relaxed);
                let final_md5 = if !md5.is_empty() {
                    md5
                } else {
                    hash_verify::md5_file(&dest).await.unwrap_or_default()
                };
                info!("下载完成: {}", filename);
                Ok::<(String, String), String>((filename, final_md5))
            });
            handles.push(handle);
        }

        let mut first_error: Option<String> = None;
        for handle in handles {
            match handle.await {
                Ok(Ok((filename, md5))) => {
                    state.checksums.insert(filename, md5);
                    save_download_state(game_folder, &state);
                }
                Ok(Err(e)) => {
                    if first_error.is_none() {
                        first_error = Some(e);
                    }
                }
                Err(e) => {
                    if first_error.is_none() {
                        first_error = Some(format!("下载任务失败: {}", e));
                    }
                }
            }
        }

        reporter.abort();

        if let Some(e) = first_error {
            return Err(e);
        }
    }

    info!("全部 {} 个补丁包下载完成，开始安装...", total_count);

    // ===== 阶段 2: 安装 =====
    // 2a: 游戏补丁本体
    if patch_pkg_count > 0 {
        let first_patch_file = &all_tasks[0].filename;
        if !state.installed_archives.contains(first_patch_file) {
            let first_part = game_folder.join(first_patch_file);
            if first_part.exists() {
                info!("安装游戏补丁 (从 {})", first_patch_file);
                match extract_archive(
                    &first_part, game_folder, &app, "游戏补丁",
                    total_count, 1,
                ).await {
                    Ok(()) => {
                        for task in &all_tasks[..patch_pkg_count] {
                            tokio::fs::remove_file(game_folder.join(&task.filename)).await.ok();
                        }
                        state.installed_archives.push(first_patch_file.to_string());
                        save_download_state(game_folder, &state);
                    }
                    Err(e) => {
                        error!("游戏补丁安装失败: {}", e);
                        for task in &all_tasks[..patch_pkg_count] {
                            state.checksums.remove(&task.filename);
                            tokio::fs::remove_file(game_folder.join(&task.filename)).await.ok();
                        }
                        save_download_state(game_folder, &state);
                        return Err(format!("游戏补丁安装失败: {}。已删除文件，请重新下载。", e));
                    }
                }
            }
        } else {
            info!("游戏补丁已安装，跳过");
        }
    }

    // 2b: 语言包补丁（每个单独解压）
    for (idx, task) in all_tasks[patch_pkg_count..].iter().enumerate() {
        if state.installed_archives.contains(&task.filename) {
            info!("语言包补丁 {} 已安装，跳过", task.filename);
            continue;
        }

        if *cancel_token.lock().await {
            return Err("Download cancelled".to_string());
        }

        let archive = game_folder.join(&task.filename);
        if !archive.exists() {
            continue;
        }

        info!("安装语言包补丁: {}", task.filename);
        match extract_archive(
            &archive, game_folder, &app, &task.label,
            total_count, patch_pkg_count + idx + 1,
        ).await {
            Ok(()) => {
                tokio::fs::remove_file(&archive).await.ok();
                state.installed_archives.push(task.filename.clone());
                save_download_state(game_folder, &state);
            }
            Err(e) => {
                error!("语言包补丁安装失败: {}", e);
                tokio::fs::remove_file(&archive).await.ok();
                state.checksums.remove(&task.filename);
                save_download_state(game_folder, &state);
                return Err(format!("安装 {} 失败: {}。已删除文件，请重新下载。", task.label, e));
            }
        }
    }

    clear_download_state(game_folder);

    // 处理 hdiff 补丁文件（如果有）
    apply_hdiff_patches(game_folder).await?;

    // 清理 deletefiles.txt 中列出的旧文件
    cleanup_deleted_files(game_folder).await;

    // 写入新版本号
    hoyoverse::write_local_version(game_folder, &game_pkg.main.major.version)?;

    info!("增量更新安装完成");
    Ok(())
}

// ============================================================
// 文件校验：使用 res_list 对比 MD5
// ============================================================

pub async fn verify_game(
    app: AppHandle,
    game_pkg: &GamePackage,
    game_folder: &Path,
    cancel_token: Arc<Mutex<bool>>,
) -> Result<crate::downloader::verifier::VerifyResult, String> {
    let res_list_url = &game_pkg.main.major.res_list_url;

    let resource_list = hoyoverse::fetch_resource_list(res_list_url).await?;

    if resource_list.is_empty() {
        return Err("资源列表为空".to_string());
    }

    let total_files = resource_list.len();
    let total_size: u64 = resource_list.iter().map(|r| r.file_size).sum();

    info!(
        "开始校验 {} 个文件, 总大小 {} 字节",
        total_files, total_size
    );

    let mut verified_ok: usize = 0;
    let mut redownloaded: usize = 0;
    let mut failed: Vec<String> = Vec::new();
    let mut finished_size: u64 = 0;
    let mut speed_tracker = SpeedTracker::new();

    for (i, entry) in resource_list.iter().enumerate() {
        if *cancel_token.lock().await {
            return Err("Verification cancelled".to_string());
        }

        let file_path = game_folder.join(&entry.remote_name);
        let current_md5 = hash_verify::md5_file(&file_path).await.unwrap_or_default();

        if current_md5.to_lowercase() == entry.md5.to_lowercase() {
            verified_ok += 1;
        } else {
            warn!(
                "{} MD5 不匹配 (期望: {}, 实际: {})",
                entry.remote_name, entry.md5, current_md5
            );
            // 对于 HoYoverse 游戏，目前不支持单文件重下载（需要从 zip 中提取）
            // 记录为失败
            failed.push(entry.remote_name.clone());
        }

        finished_size += entry.file_size;
        speed_tracker.record(entry.file_size);

        let remaining = total_size.saturating_sub(finished_size);
        let progress = DownloadProgress {
            phase: "verify".to_string(),
            total_size,
            finished_size,
            total_count: total_files,
            finished_count: i + 1,
            current_file: entry.remote_name.clone(),
            speed_bps: speed_tracker.speed_bps(),
            eta_seconds: speed_tracker.eta_seconds(remaining),
        };
        app.emit("game-verify-progress", &progress).ok();
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
async fn download_file_to_disk(
    client: &Client,
    url: &str,
    dest: &Path,
    expected_md5: &str,
    shared_bytes: Arc<AtomicU64>,
    cancel_token: Arc<Mutex<bool>>,
) -> Result<(), String> {
    let temp_path = dest.with_file_name(
        format!(
            "{}.temp",
            dest.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("download")
        ),
    );

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
        }
    }

    let mut req = client.get(url).header("User-Agent", "Mozilla/5.0");
    if downloaded_bytes > 0 {
        req = req.header("Range", format!("bytes={}-", downloaded_bytes));
    }

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
                tokio::fs::rename(&temp_path, dest)
                    .await
                    .map_err(|e| format!("重命名临时文件失败: {}", e))?;
                return Ok(());
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
        downloaded_bytes += len;
        shared_bytes.fetch_add(len, Ordering::Relaxed);
    }

    file.flush()
        .await
        .map_err(|e| format!("刷新缓冲失败: {}", e))?;
    drop(file);

    // MD5 校验
    if !expected_md5.is_empty() {
        let actual_md5 = hash_verify::md5_file(&temp_path).await.unwrap_or_default();
        if actual_md5.to_lowercase() != expected_md5.to_lowercase() {
            warn!(
                "MD5 不匹配 (期望: {}, 实际: {}), 将继续使用",
                expected_md5, actual_md5
            );
        }
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
    app: &AppHandle,
    label: &str,
    total_segments: usize,
    current_segment: usize,
) -> Result<(), String> {
    let name = archive_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if name.contains(".7z") {
        extract_7z(archive_path, dest_folder, app, label, total_segments, current_segment).await
    } else {
        extract_zip(archive_path, dest_folder, app, label, total_segments, current_segment).await
    }
}

/// 使用系统 7z 命令解压（支持分卷 .7z.001 和单文件 .7z）
async fn extract_7z(
    archive_path: &Path,
    dest_folder: &Path,
    app: &AppHandle,
    label: &str,
    total_segments: usize,
    current_segment: usize,
) -> Result<(), String> {
    let label = label.to_string();
    let app = app.clone();
    let archive_path = archive_path.to_path_buf();
    let dest_folder = dest_folder.to_path_buf();

    // 发送安装开始事件
    {
        let progress = DownloadProgress {
            phase: "install".to_string(),
            total_size: 100,
            finished_size: 0,
            total_count: total_segments,
            finished_count: current_segment,
            current_file: format!("正在解压 {}...", label),
            speed_bps: 0,
            eta_seconds: 0,
        };
        app.emit("game-install-progress", &progress).ok();
    }

    info!("7z 解压: {} → {}", archive_path.display(), dest_folder.display());

    let mut child = tokio::process::Command::new("7z")
        .arg("x")
        .arg(&archive_path)
        .arg(format!("-o{}", dest_folder.display()))
        .arg("-aoa") // 覆盖已有文件
        .arg("-y")   // 自动确认
        .arg("-bsp1") // 进度输出到 stderr
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("执行 7z 命令失败: {}。请确保已安装 p7zip-full。", e))?;

    // 从 stderr 读取进度
    if let Some(stderr) = child.stderr.take() {
        let app_clone = app.clone();
        let label_clone = label.clone();
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
                        for part in text.split(|c: char| c == '\r' || c == '\n') {
                            let trimmed = part.trim();
                            if let Some(pos) = trimmed.find('%') {
                                let before = trimmed[..pos].trim();
                                // 取 % 前面的最后一段数字
                                let num_str = before.rsplit(|c: char| !c.is_ascii_digit()).next().unwrap_or("");
                                if let Ok(pct) = num_str.parse::<u64>() {
                                    if pct <= 100 {
                                        last_pct = pct;
                                    }
                                }
                            }
                        }
                        if last_report.elapsed() > std::time::Duration::from_millis(500) {
                            let progress = DownloadProgress {
                                phase: "install".to_string(),
                                total_size: 100,
                                finished_size: last_pct,
                                total_count: total_segments,
                                finished_count: current_segment,
                                current_file: format!("安装 {} ({}%)", label_clone, last_pct),
                                speed_bps: 0,
                                eta_seconds: 0,
                            };
                            app_clone.emit("game-install-progress", &progress).ok();
                            last_report = std::time::Instant::now();
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }

    let status = child.wait().await
        .map_err(|e| format!("等待 7z 进程失败: {}", e))?;

    if !status.success() {
        return Err(format!("7z 解压失败 (exit code: {:?})", status.code()));
    }

    // 发送完成事件
    {
        let progress = DownloadProgress {
            phase: "install".to_string(),
            total_size: 100,
            finished_size: 100,
            total_count: total_segments,
            finished_count: current_segment,
            current_file: format!("安装 {} 完成", label),
            speed_bps: 0,
            eta_seconds: 0,
        };
        app.emit("game-install-progress", &progress).ok();
    }

    info!("7z 解压完成: {}", archive_path.display());
    Ok(())
}

/// 解压 zip 到目标目录（带进度上报）
async fn extract_zip(
    zip_path: &Path,
    dest_folder: &Path,
    app: &AppHandle,
    segment_label: &str,
    total_segments: usize,
    current_segment: usize,
) -> Result<(), String> {
    let zip_path = zip_path.to_path_buf();
    let dest_folder = dest_folder.to_path_buf();
    let label = segment_label.to_string();
    let app = app.clone();

    // 立即发送安装开始事件（不等 500ms）
    {
        let progress = DownloadProgress {
            phase: "install".to_string(),
            total_size: 0,
            finished_size: 0,
            total_count: total_segments,
            finished_count: current_segment,
            current_file: format!("正在打开 {}...", label),
            speed_bps: 0,
            eta_seconds: 0,
        };
        app.emit("game-install-progress", &progress).ok();
    }

    // zip 解压需要在阻塞线程中执行
    tokio::task::spawn_blocking(move || {
        let file =
            std::fs::File::open(&zip_path).map_err(|e| format!("打开 zip 文件失败: {}", e))?;
        let file_len = file.metadata().map(|m| m.len()).unwrap_or(0);
        info!("开始解压: {} ({:.1} MB)", zip_path.display(), file_len as f64 / 1048576.0);

        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| format!("读取 zip 归档失败: {}", e))?;

        let entry_count = archive.len();
        let mut last_report = std::time::Instant::now();

        // 发送条目总数
        {
            let progress = DownloadProgress {
                phase: "install".to_string(),
                total_size: entry_count as u64,
                finished_size: 0,
                total_count: total_segments,
                finished_count: current_segment,
                current_file: format!("安装 {} (0%) 0/{}", label, entry_count),
                speed_bps: 0,
                eta_seconds: 0,
            };
            app.emit("game-install-progress", &progress).ok();
        }

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
                let pct = if entry_count > 0 { (i + 1) * 100 / entry_count } else { 0 };
                let progress = DownloadProgress {
                    phase: "install".to_string(),
                    total_size: entry_count as u64,
                    finished_size: (i + 1) as u64,
                    total_count: total_segments,
                    finished_count: current_segment,
                    current_file: format!("安装 {} ({}%) {}/{}", label, pct, i + 1, entry_count),
                    speed_bps: 0,
                    eta_seconds: 0,
                };
                app.emit("game-install-progress", &progress).ok();
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
            let file_path = game_folder.join(line);
            if file_path.exists() {
                info!("删除旧文件: {}", line);
                tokio::fs::remove_file(&file_path).await.ok();
            }
        }
    }

    tokio::fs::remove_file(&delete_list).await.ok();
}

// ============================================================
// JSON 下载状态文件：缓存 MD5 + 安装进度
// ============================================================

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const STATE_FILE: &str = "_download_state.json";

#[derive(Serialize, Deserialize, Default)]
struct DownloadState {
    /// 每个分段下载完成后缓存的 MD5 (key = 文件名，如 "StarRail_4.0.0.7z.001")
    checksums: HashMap<String, String>,
    /// 已安装（解压）完成的归档列表（文件名）
    installed_archives: Vec<String>,
    /// 预期总大小（用于校验是否同一批下载任务）
    expected_total_size: u64,
}

fn state_path(game_folder: &Path) -> PathBuf {
    game_folder.join(STATE_FILE)
}

fn read_download_state(game_folder: &Path) -> DownloadState {
    let path = state_path(game_folder);
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_download_state(game_folder: &Path, state: &DownloadState) {
    let path = state_path(game_folder);
    if let Ok(json) = serde_json::to_string_pretty(state) {
        std::fs::write(&path, json).ok();
    }
}

fn clear_download_state(game_folder: &Path) {
    std::fs::remove_file(state_path(game_folder)).ok();
    // 清理旧格式 checkpoint（如果存在）
    std::fs::remove_file(game_folder.join("_download_checkpoint")).ok();
}

// ============================================================
// 辅助结构体和函数
// ============================================================

/// 下载任务描述
struct DownloadTask {
    url: String,
    md5: String,
    size: String,
    label: String,
    /// 从 URL 提取的原始文件名（如 StarRail_4.0.0.7z.001, Chinese.7z）
    filename: String,
}

/// 从 URL 中提取文件名
fn filename_from_url(url: &str) -> String {
    url.rsplit('/').next().unwrap_or("unknown").to_string()
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
        .filter(|pkg| languages.iter().any(|lang| pkg.language == *lang))
        .map(|pkg| {
            (
                pkg.language.clone(),
                Segment {
                    url: pkg.url.clone(),
                    md5: pkg.md5.clone(),
                    size: pkg.size.clone(),
                    decompressed_size: pkg.decompressed_size.clone(),
                },
            )
        })
        .collect()
}

fn emit_progress(
    app: &AppHandle,
    phase: &str,
    finished_size: u64,
    total_size: u64,
    total_count: usize,
    finished_count: usize,
    current_file: &str,
    speed_bps: u64,
    eta_seconds: u64,
) {
    let progress = DownloadProgress {
        phase: phase.to_string(),
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
