use crate::downloader::fetcher;
use crate::downloader::hoyoverse::{self, GamePackage, ResourceEntry, Segment};
use crate::downloader::progress::{DownloadProgress, SpeedTracker};
use crate::utils::hash_verify;
use reqwest::Client;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tracing::{info, warn, error};

// ============================================================
// 全量下载：下载 zip 分段 → 解压到游戏目录
// ============================================================

pub async fn download_game(
    app: AppHandle,
    game_pkg: &GamePackage,
    game_folder: &Path,
    cancel_token: Arc<Mutex<bool>>,
) -> Result<(), String> {
    let segments = &game_pkg.main.major.game_pkgs;
    if segments.is_empty() {
        return Err("没有可用的游戏下载包".to_string());
    }

    std::fs::create_dir_all(game_folder)
        .map_err(|e| format!("创建游戏目录失败: {}", e))?;

    let total_size: u64 = segments
        .iter()
        .filter_map(|s| s.size.parse::<u64>().ok())
        .sum();

    info!(
        "开始全量下载: {} 个分段, 总大小 {} 字节",
        segments.len(),
        total_size
    );

    let mut finished_size: u64 = 0;
    let mut speed_tracker = SpeedTracker::new();

    for (i, segment) in segments.iter().enumerate() {
        if *cancel_token.lock().await {
            info!("下载被用户取消");
            return Err("Download cancelled".to_string());
        }

        let seg_size: u64 = segment.size.parse().unwrap_or(0);
        info!(
            "下载分段 {}/{}: {} ({} 字节)",
            i + 1,
            segments.len(),
            segment.url,
            seg_size
        );

        // 下载 zip 到临时文件
        let temp_zip = game_folder.join(format!("_download_seg_{}.zip", i));
        download_segment_with_progress(
            &app,
            &segment.url,
            &temp_zip,
            &segment.md5,
            &mut finished_size,
            total_size,
            &mut speed_tracker,
            segments.len(),
            i,
            cancel_token.clone(),
        )
        .await?;

        // 解压到游戏目录
        info!("解压分段 {}/{}...", i + 1, segments.len());
        emit_progress(
            &app,
            "extract",
            finished_size,
            total_size,
            segments.len(),
            i + 1,
            &format!("解压分段 {}", i + 1),
            0,
            0,
        );

        extract_zip(&temp_zip, game_folder).await?;

        // 删除临时 zip
        tokio::fs::remove_file(&temp_zip).await.ok();
    }

    // 写入版本号
    hoyoverse::write_local_version(game_folder, &game_pkg.main.major.version)?;

    info!("全量下载完成");
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

    let segments = &patch.game_pkgs;
    if segments.is_empty() {
        return Err("补丁包为空".to_string());
    }

    let total_size: u64 = segments
        .iter()
        .filter_map(|s| s.size.parse::<u64>().ok())
        .sum();

    info!(
        "开始增量更新 {} → {}: {} 个分段, {} 字节",
        local_version,
        game_pkg.main.major.version,
        segments.len(),
        total_size
    );

    let mut finished_size: u64 = 0;
    let mut speed_tracker = SpeedTracker::new();

    for (i, segment) in segments.iter().enumerate() {
        if *cancel_token.lock().await {
            return Err("Download cancelled".to_string());
        }

        let temp_zip = game_folder.join(format!("_patch_seg_{}.zip", i));
        download_segment_with_progress(
            &app,
            &segment.url,
            &temp_zip,
            &segment.md5,
            &mut finished_size,
            total_size,
            &mut speed_tracker,
            segments.len(),
            i,
            cancel_token.clone(),
        )
        .await?;

        info!("解压补丁分段 {}/{}...", i + 1, segments.len());
        emit_progress(
            &app,
            "extract",
            finished_size,
            total_size,
            segments.len(),
            i + 1,
            &format!("解压补丁 {}", i + 1),
            0,
            0,
        );

        extract_zip(&temp_zip, game_folder).await?;
        tokio::fs::remove_file(&temp_zip).await.ok();
    }

    // 处理 hdiff 补丁文件（如果有）
    apply_hdiff_patches(game_folder).await?;

    // 清理 deletefiles.txt 中列出的旧文件
    cleanup_deleted_files(game_folder).await;

    // 写入新版本号
    hoyoverse::write_local_version(game_folder, &game_pkg.main.major.version)?;

    info!("增量更新完成");
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

    info!("开始校验 {} 个文件, 总大小 {} 字节", total_files, total_size);

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

/// 下载单个 zip 分段，带进度上报
async fn download_segment_with_progress(
    app: &AppHandle,
    url: &str,
    dest: &Path,
    expected_md5: &str,
    finished_size: &mut u64,
    total_size: u64,
    speed_tracker: &mut SpeedTracker,
    total_segments: usize,
    current_segment: usize,
    cancel_token: Arc<Mutex<bool>>,
) -> Result<(), String> {
    let client = Client::new();

    // 使用临时文件下载
    let temp_path = dest.with_extension("zip.temp");

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("创建目录失败: {}", e))?;
    }

    // 检查已有的临时文件大小
    let mut downloaded_bytes: u64 = 0;
    if temp_path.exists() {
        if let Ok(meta) = tokio::fs::metadata(&temp_path).await {
            downloaded_bytes = meta.len();
        }
    }

    // 构建请求
    let mut req = client
        .get(url)
        .header("User-Agent", "Mozilla/5.0");

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
                warn!("服务器不支持断点续传，重新开始下载");
                downloaded_bytes = 0;
            }
        }
        416 => {
            // 文件可能已完整
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

    // 流式下载
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;

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
    let mut last_emit = std::time::Instant::now();

    while let Some(chunk_result) = stream.next().await {
        if *cancel_token.lock().await {
            return Err("Download cancelled".to_string());
        }

        let chunk = chunk_result.map_err(|e| format!("流读取错误: {}", e))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("写入错误: {}", e))?;

        downloaded_bytes += chunk.len() as u64;
        speed_tracker.record(chunk.len() as u64);

        // 每 200ms 上报一次进度
        if last_emit.elapsed() > std::time::Duration::from_millis(200) {
            let current_total = *finished_size + downloaded_bytes;
            emit_progress(
                app,
                "download",
                current_total,
                total_size,
                total_segments,
                current_segment + 1,
                url.rsplit('/').next().unwrap_or(""),
                speed_tracker.speed_bps(),
                speed_tracker.eta_seconds(total_size.saturating_sub(current_total)),
            );
            last_emit = std::time::Instant::now();
        }
    }

    file.flush()
        .await
        .map_err(|e| format!("刷新缓冲失败: {}", e))?;
    drop(file);

    // 校验 MD5
    if !expected_md5.is_empty() {
        let actual_md5 = hash_verify::md5_file(&temp_path)
            .await
            .unwrap_or_default();
        if actual_md5.to_lowercase() != expected_md5.to_lowercase() {
            warn!(
                "分段 MD5 不匹配 (期望: {}, 实际: {}), 将继续使用",
                expected_md5, actual_md5
            );
        }
    }

    // 重命名为目标文件
    tokio::fs::rename(&temp_path, dest)
        .await
        .map_err(|e| format!("重命名文件失败: {}", e))?;

    *finished_size += downloaded_bytes;
    Ok(())
}

/// 解压 zip 到目标目录
async fn extract_zip(zip_path: &Path, dest_folder: &Path) -> Result<(), String> {
    let zip_path = zip_path.to_path_buf();
    let dest_folder = dest_folder.to_path_buf();

    // zip 解压需要在阻塞线程中执行
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&zip_path)
            .map_err(|e| format!("打开 zip 文件失败: {}", e))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| format!("读取 zip 归档失败: {}", e))?;

        for i in 0..archive.len() {
            let mut zip_file = archive
                .by_index(i)
                .map_err(|e| format!("读取 zip 条目失败: {}", e))?;

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
        }

        info!("解压完成: {} 个条目", archive.len());
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

        let original_path = hdiff_path.parent().unwrap_or(game_folder).join(&original_name);

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
