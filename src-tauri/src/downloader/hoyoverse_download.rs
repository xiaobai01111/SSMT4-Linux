use crate::downloader::hoyoverse::{self, GamePackage, Segment};
use crate::downloader::progress::{DownloadProgress, SpeedTracker};
use crate::utils::file_manager::{safe_join, safe_join_remote};
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

    let audio_segments = collect_audio_segments(&game_pkg.main.major.audio_pkgs, languages);

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

    let plan = DownloadPlan {
        all_tasks,
        primary_pkg_count: segments.len(),
        primary_label: "游戏本体".to_string(),
        migrate_old_files: true,
        post_install: PostInstall::WriteVersion {
            version: game_pkg.main.major.version.clone(),
        },
    };

    execute_plan(app, plan, game_folder, cancel_token).await
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

    let plan = DownloadPlan {
        all_tasks,
        primary_pkg_count: patch.game_pkgs.len(),
        primary_label: "游戏补丁".to_string(),
        migrate_old_files: false,
        post_install: PostInstall::PatchAndWriteVersion {
            version: game_pkg.main.major.version.clone(),
        },
    };

    execute_plan(app, plan, game_folder, cancel_token).await
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
    let redownloaded: usize = 0;
    let mut failed: Vec<String> = Vec::new();
    let mut finished_size: u64 = 0;
    let mut speed_tracker = SpeedTracker::new();

    for (i, entry) in resource_list.iter().enumerate() {
        if *cancel_token.lock().await {
            return Err("Verification cancelled".to_string());
        }

        let file_path = match safe_join(game_folder, &entry.remote_name) {
            Ok(p) => p,
            Err(e) => {
                warn!("跳过不安全的清单路径: {} ({})", entry.remote_name, e);
                continue;
            }
        };
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

    // MD5 校验：不匹配则删除临时文件并返回错误，由调用方重试
    if !expected_md5.is_empty() {
        let actual_md5 = hash_verify::md5_file(&temp_path).await.unwrap_or_default();
        if actual_md5.to_lowercase() != expected_md5.to_lowercase() {
            // 删除损坏的临时文件，确保下次不会断点续传坏数据
            tokio::fs::remove_file(&temp_path).await.ok();
            return Err(format!(
                "MD5 不匹配 (期望: {}, 实际: {}): {}",
                expected_md5, actual_md5, url
            ));
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
    let name = archive_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if name.contains(".7z") {
        extract_7z(
            archive_path,
            dest_folder,
            app,
            label,
            total_segments,
            current_segment,
        )
        .await
    } else {
        extract_zip(
            archive_path,
            dest_folder,
            app,
            label,
            total_segments,
            current_segment,
        )
        .await
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

    let status = child
        .wait()
        .await
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
                let pct = if entry_count > 0 {
                    (i + 1) * 100 / entry_count
                } else {
                    0
                };
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
// JSON 下载状态文件：缓存 MD5 + 安装进度
// 内存态 + 节流落盘（≤1s 一次）+ 原子写入（tmp → rename）
// ============================================================

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const STATE_FILE: &str = "_download_state.json";
/// 节流间隔：两次磁盘写入之间的最小间隔
const FLUSH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);

#[derive(Serialize, Deserialize, Default)]
struct DownloadState {
    /// 每个分段下载完成后缓存的 MD5 (key = 文件名，如 "StarRail_4.0.0.7z.001")
    checksums: HashMap<String, String>,
    /// 已安装（解压）完成的归档列表（文件名）
    installed_archives: Vec<String>,
    /// 预期总大小（用于校验是否同一批下载任务）
    expected_total_size: u64,
    /// 轻量 MD5 缓存："filename:size:mtime_secs" → md5
    /// 避免文件未变更时重复计算哈希
    #[serde(default)]
    file_hashes: HashMap<String, String>,
}

/// 带节流的状态写入器：内存中持有 DownloadState，按需落盘
struct StateWriter {
    state: DownloadState,
    game_folder: PathBuf,
    dirty: bool,
    last_flush: std::time::Instant,
}

impl StateWriter {
    /// 从磁盘加载已有状态（或返回默认值）
    fn load(game_folder: &Path) -> Self {
        let path = game_folder.join(STATE_FILE);
        let state = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self {
            state,
            game_folder: game_folder.to_path_buf(),
            dirty: false,
            last_flush: std::time::Instant::now(),
        }
    }

    /// 标记脏并节流写盘：距上次写盘 ≥1s 才真正落盘
    fn mark_dirty(&mut self) {
        self.dirty = true;
        if self.last_flush.elapsed() >= FLUSH_INTERVAL {
            self.flush();
        }
    }

    /// 立即落盘（原子写入：写 .tmp → rename）
    fn flush(&mut self) {
        if !self.dirty {
            return;
        }
        let path = self.game_folder.join(STATE_FILE);
        let tmp = self.game_folder.join(format!("{}.tmp", STATE_FILE));
        if let Ok(json) = serde_json::to_string(&self.state) {
            if std::fs::write(&tmp, &json).is_ok() {
                std::fs::rename(&tmp, &path).ok();
            }
        }
        self.dirty = false;
        self.last_flush = std::time::Instant::now();
    }

    /// 清除状态文件
    fn clear(self) {
        let path = self.game_folder.join(STATE_FILE);
        std::fs::remove_file(&path).ok();
        let tmp = self.game_folder.join(format!("{}.tmp", STATE_FILE));
        std::fs::remove_file(&tmp).ok();
        // 清理旧格式 checkpoint（如果存在）
        std::fs::remove_file(self.game_folder.join("_download_checkpoint")).ok();
    }
}

// ============================================================
// 统一下载-安装流水线
// ============================================================

/// 下载计划：描述"下载什么"和"安装策略"
struct DownloadPlan {
    all_tasks: Vec<DownloadTask>,
    /// 前 N 个任务属于主包（游戏本体/补丁），其余为语言包
    primary_pkg_count: usize,
    /// 主包显示标签（如 "游戏本体" / "游戏补丁"）
    primary_label: String,
    /// 是否尝试迁移旧格式文件（_download_N.zip）— 仅全量下载需要
    migrate_old_files: bool,
    /// 安装完成后的收尾动作
    post_install: PostInstall,
}

/// 安装完成后的收尾策略
enum PostInstall {
    /// 全量下载：只写版本号
    WriteVersion { version: String },
    /// 增量更新：应用 hdiff + 清理 deletefiles + 写版本号
    PatchAndWriteVersion { version: String },
}

/// 统一流水线：预检 → 并行下载 → 安装主包 → 安装语言包 → 收尾
async fn execute_plan(
    app: AppHandle,
    plan: DownloadPlan,
    game_folder: &Path,
    cancel_token: Arc<Mutex<bool>>,
) -> Result<(), String> {
    let DownloadPlan {
        all_tasks,
        primary_pkg_count,
        primary_label,
        migrate_old_files,
        post_install,
    } = plan;

    if all_tasks.is_empty() {
        return Err("下载任务列表为空".to_string());
    }

    let total_size: u64 = all_tasks
        .iter()
        .filter_map(|s| s.size.parse::<u64>().ok())
        .sum();
    let total_count = all_tasks.len();

    info!(
        "execute_plan: {} 个包 (主包 {}×{}), 总大小 {} 字节",
        total_count, primary_label, primary_pkg_count, total_size
    );

    // 加载下载状态（内存态 + 节流落盘）
    let mut sw = StateWriter::load(game_folder);
    if sw.state.expected_total_size != 0 && sw.state.expected_total_size != total_size {
        info!("下载任务变更 (total_size 不同)，重置状态");
        sw.state = DownloadState::default();
    }
    sw.state.expected_total_size = total_size;

    // ===== 阶段 1a: 预检查 =====
    // 第一遍（快速串行）：JSON 缓存命中 → 跳过；无缓存且文件存在 → 收集待哈希
    let mut cached_size: u64 = 0;
    let mut to_download: Vec<usize> = Vec::new();
    // (task_index, file_path, file_size, is_old_file) — 需要做 MD5 校验的条目
    let mut needs_hash: Vec<(usize, PathBuf, u64, bool)> = Vec::new();

    for (i, task) in all_tasks.iter().enumerate() {
        if *cancel_token.lock().await {
            sw.flush();
            return Err("Download cancelled".to_string());
        }

        let dest = safe_join(game_folder, &task.filename)?;

        // 检查 JSON 缓存（纯内存，无 I/O）
        if let Some(cached_md5) = sw.state.checksums.get(&task.filename) {
            if !task.md5.is_empty() && cached_md5.to_lowercase() == task.md5.to_lowercase() {
                let file_size = if dest.exists() {
                    tokio::fs::metadata(&dest)
                        .await
                        .map(|m| m.len())
                        .unwrap_or(0)
                } else {
                    task.size.parse::<u64>().unwrap_or(0)
                };
                cached_size += file_size;
                info!("{} ({}) 已缓存，跳过下载", task.label, task.filename);
                emit_progress(
                    &app,
                    "download",
                    cached_size,
                    total_size,
                    total_count,
                    i + 1,
                    &format!("已缓存，跳过 {}", task.label),
                    0,
                    0,
                );
                continue;
            }
        }

        // 迁移旧格式文件：检查旧文件是否存在（仅全量下载）
        if migrate_old_files && !dest.exists() && !task.md5.is_empty() {
            let old_file = game_folder.join(format!("_download_{}.zip", i));
            if old_file.exists() {
                let file_size = tokio::fs::metadata(&old_file)
                    .await
                    .map(|m| m.len())
                    .unwrap_or(0);
                needs_hash.push((i, old_file, file_size, true));
                continue;
            }
        }

        // 文件已存在但无缓存 → 需要哈希校验
        if dest.exists() && !task.md5.is_empty() {
            let file_size = tokio::fs::metadata(&dest)
                .await
                .map(|m| m.len())
                .unwrap_or(0);
            needs_hash.push((i, dest, file_size, false));
            continue;
        }

        // 文件不存在且无缓存 → 需要下载
        to_download.push(i);
    }

    // 第二遍（受控并发）：对 needs_hash 中的文件做 MD5 校验
    if !needs_hash.is_empty() {
        const HASH_CONCURRENCY: usize = 4;

        // 先用 mtime 缓存快速过滤
        let mut still_needs_hash: Vec<(usize, PathBuf, u64, bool)> = Vec::new();
        for (idx, path, file_size, is_old) in needs_hash {
            let cache_key = make_hash_cache_key(&path, file_size);
            if let Some(cached) = sw.state.file_hashes.get(&cache_key) {
                let task = &all_tasks[idx];
                if !task.md5.is_empty() && cached.to_lowercase() == task.md5.to_lowercase() {
                    // mtime 缓存命中且 MD5 匹配
                    if is_old {
                        let dest = safe_join(game_folder, &task.filename)?;
                        info!(
                            "迁移旧文件 (mtime缓存) {} → {}",
                            path.display(),
                            dest.display()
                        );
                        tokio::fs::rename(&path, &dest).await.ok();
                    }
                    sw.state
                        .checksums
                        .insert(task.filename.clone(), task.md5.clone());
                    sw.mark_dirty();
                    cached_size += file_size;
                    info!("{} mtime缓存命中，跳过哈希", task.filename);
                    emit_progress(
                        &app,
                        "download",
                        cached_size,
                        total_size,
                        total_count,
                        idx + 1,
                        &format!("已缓存，跳过 {}", task.label),
                        0,
                        0,
                    );
                    continue;
                }
            }
            still_needs_hash.push((idx, path, file_size, is_old));
        }

        if !still_needs_hash.is_empty() {
            info!(
                "预检：{} 个文件需要 MD5 校验 ({}路并发)",
                still_needs_hash.len(),
                HASH_CONCURRENCY
            );

            let hash_sem = Arc::new(tokio::sync::Semaphore::new(HASH_CONCURRENCY));
            let mut hash_handles = Vec::new();

            for (idx, path, file_size, is_old) in still_needs_hash {
                let sem = hash_sem.clone();
                let cancel = cancel_token.clone();
                let handle = tokio::spawn(async move {
                    let _permit = sem.acquire().await.map_err(|e| format!("{}", e))?;
                    if *cancel.lock().await {
                        return Err("Download cancelled".to_string());
                    }
                    let md5 = hash_verify::md5_file(&path).await.unwrap_or_default();
                    Ok::<(usize, PathBuf, u64, bool, String), String>((
                        idx, path, file_size, is_old, md5,
                    ))
                });
                hash_handles.push(handle);
            }

            for handle in hash_handles {
                let (idx, path, file_size, is_old, actual_md5) = match handle.await {
                    Ok(Ok(v)) => v,
                    Ok(Err(e)) => {
                        sw.flush();
                        return Err(e);
                    }
                    Err(e) => {
                        sw.flush();
                        return Err(format!("哈希任务失败: {}", e));
                    }
                };

                // 写入 mtime 缓存，无论匹配与否都记录（下次可快速判断）
                let cache_key = make_hash_cache_key(&path, file_size);
                sw.state.file_hashes.insert(cache_key, actual_md5.clone());

                let task = &all_tasks[idx];
                if actual_md5.to_lowercase() == task.md5.to_lowercase() {
                    // MD5 匹配
                    if is_old {
                        let dest = safe_join(game_folder, &task.filename)?;
                        info!("迁移旧文件 {} → {}", path.display(), dest.display());
                        tokio::fs::rename(&path, &dest).await.ok();
                    }
                    sw.state
                        .checksums
                        .insert(task.filename.clone(), task.md5.clone());
                    sw.mark_dirty();
                    cached_size += file_size;
                    info!("{} MD5 匹配，补录缓存", task.filename);
                    emit_progress(
                        &app,
                        "download",
                        cached_size,
                        total_size,
                        total_count,
                        idx + 1,
                        &format!("已缓存，跳过 {}", task.label),
                        0,
                        0,
                    );
                } else {
                    // MD5 不匹配 → 需要下载
                    if is_old {
                        warn!("旧文件 {} MD5 不匹配，将重新下载", path.display());
                    } else {
                        warn!("{} MD5 不匹配，将重新下载", task.filename);
                    }
                    to_download.push(idx);
                }
            }
        } // end if !still_needs_hash.is_empty()
    }

    // 预检结束，确保脏数据落盘
    sw.flush();

    // ===== 阶段 1b: 并行下载 =====
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
            let dest = safe_join(game_folder, &task.filename)?;
            let url = task.url.clone();
            let md5 = task.md5.clone();
            let filename = task.filename.clone();

            let handle = tokio::spawn(async move {
                let _permit = sem
                    .acquire()
                    .await
                    .map_err(|e| format!("信号量错误: {}", e))?;
                info!("开始下载: {}", filename);

                // 流错误自动重试（最多 3 次），利用断点续传从断点继续
                const MAX_RETRIES: u32 = 3;
                let mut last_err = String::new();
                let temp_name = format!(
                    "{}.temp",
                    dest.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("download")
                );
                let temp_path = dest.with_file_name(&temp_name);
                for attempt in 0..=MAX_RETRIES {
                    if attempt > 0 {
                        // 重试前回退 temp 文件大小：download_file_to_disk 内部会重新 fetch_add
                        if let Ok(meta) = tokio::fs::metadata(&temp_path).await {
                            bytes.fetch_sub(meta.len(), Ordering::Relaxed);
                        }
                        warn!(
                            "{} 下载失败，第 {}/{} 次重试: {}",
                            filename, attempt, MAX_RETRIES, last_err
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(
                            2u64.pow(attempt.min(3)),
                        ))
                        .await;
                    }
                    match download_file_to_disk(
                        &client,
                        &url,
                        &dest,
                        &md5,
                        bytes.clone(),
                        cancel.clone(),
                    )
                    .await
                    {
                        Ok(()) => {
                            if attempt > 0 {
                                info!("{} 重试成功 (第 {} 次)", filename, attempt);
                            }
                            last_err.clear();
                            break;
                        }
                        Err(e) => {
                            if e.contains("cancelled") {
                                return Err(e);
                            }
                            last_err = e;
                        }
                    }
                }
                if !last_err.is_empty() {
                    return Err(format!("{} (已重试 {} 次)", last_err, MAX_RETRIES));
                }

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

        // 等待所有下载完成（节流写盘）
        let mut first_error: Option<String> = None;
        for handle in handles {
            match handle.await {
                Ok(Ok((filename, md5))) => {
                    sw.state.checksums.insert(filename, md5);
                    sw.mark_dirty();
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

        // 下载阶段结束，强制落盘
        sw.flush();

        if let Some(e) = first_error {
            return Err(e);
        }
    }

    info!("全部 {} 个包下载完成，开始安装...", total_count);

    // ===== 阶段 2a: 安装主包（可能是分卷，只需从第一个 part 解压一次）=====
    if primary_pkg_count > 0 {
        let first_file = &all_tasks[0].filename;
        if !sw.state.installed_archives.contains(first_file) {
            let first_part = safe_join(game_folder, first_file)?;
            if first_part.exists() {
                info!("安装{} (从 {})", primary_label, first_file);
                match extract_archive(
                    &first_part,
                    game_folder,
                    &app,
                    &primary_label,
                    total_count,
                    1,
                )
                .await
                {
                    Ok(()) => {
                        for task in &all_tasks[..primary_pkg_count] {
                            if let Ok(p) = safe_join(game_folder, &task.filename) {
                                tokio::fs::remove_file(p).await.ok();
                            }
                        }
                        sw.state.installed_archives.push(first_file.to_string());
                        sw.flush();
                    }
                    Err(e) => {
                        error!("{}安装失败: {}", primary_label, e);
                        for task in &all_tasks[..primary_pkg_count] {
                            sw.state.checksums.remove(&task.filename);
                            if let Ok(p) = safe_join(game_folder, &task.filename) {
                                tokio::fs::remove_file(p).await.ok();
                            }
                        }
                        sw.flush();
                        return Err(format!(
                            "{}安装失败: {}。已删除文件，请重新下载。",
                            primary_label, e
                        ));
                    }
                }
            } else {
                warn!("{}首段文件不存在: {}", primary_label, first_part.display());
            }
        } else {
            info!("{}已安装，跳过", primary_label);
        }
    }

    // ===== 阶段 2b: 安装语言包（每个单独解压）=====
    for (idx, task) in all_tasks[primary_pkg_count..].iter().enumerate() {
        if sw.state.installed_archives.contains(&task.filename) {
            info!("{} 已安装，跳过", task.label);
            continue;
        }

        if *cancel_token.lock().await {
            sw.flush();
            return Err("Download cancelled".to_string());
        }

        let archive = safe_join(game_folder, &task.filename)?;
        if !archive.exists() {
            info!("语言包文件不存在 (可能已安装): {}", task.filename);
            continue;
        }

        info!("安装 {}", task.label);
        match extract_archive(
            &archive,
            game_folder,
            &app,
            &task.label,
            total_count,
            primary_pkg_count + idx + 1,
        )
        .await
        {
            Ok(()) => {
                tokio::fs::remove_file(&archive).await.ok();
                sw.state.installed_archives.push(task.filename.clone());
                sw.flush();
            }
            Err(e) => {
                error!("{} 安装失败: {}", task.label, e);
                tokio::fs::remove_file(&archive).await.ok();
                sw.state.checksums.remove(&task.filename);
                sw.flush();
                return Err(format!(
                    "安装 {} 失败: {}。已删除文件，请重新下载。",
                    task.label, e
                ));
            }
        }
    }

    // ===== 阶段 3: 收尾 =====
    sw.clear();

    match post_install {
        PostInstall::WriteVersion { version } => {
            hoyoverse::write_local_version(game_folder, &version)?;
            info!("全量下载安装完成");
        }
        PostInstall::PatchAndWriteVersion { version } => {
            apply_hdiff_patches(game_folder).await?;
            cleanup_deleted_files(game_folder).await;
            hoyoverse::write_local_version(game_folder, &version)?;
            info!("增量更新安装完成");
        }
    }

    Ok(())
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
                    size: pkg.size.clone(),
                    decompressed_size: pkg.decompressed_size.clone(),
                },
            )
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
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
