use crate::downloader::progress::{DownloadProgress, SpeedTracker};
use crate::downloader::snowbreak::{self, Manifest, ResolvedCdn};
use crate::utils::file_manager::{safe_join, safe_join_remote};
use futures_util::StreamExt;
use reqwest::Client;
use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

// ============================================================
// 全量下载 / 更新
// ============================================================

/// 下载或更新游戏：对比本地 manifest，只下载缺失或哈希不匹配的文件
pub async fn download_or_update_game(
    app: AppHandle,
    game_folder: &Path,
    source_policy: snowbreak::SourcePolicy,
    cancel_token: Arc<Mutex<bool>>,
) -> Result<(), String> {
    // 1. 获取远程 manifest（自动 CDN 轮询）
    let (remote_manifest, cdn) = snowbreak::fetch_manifest_with_policy(source_policy).await?;
    crate::log_info!(
        "[Snowbreak] 远程版本: {}, 文件数: {}",
        remote_manifest.version,
        remote_manifest.paks.len(),
    );

    // 2. 读取本地 manifest（如果存在）
    let local_manifest = load_local_manifest(game_folder);

    // 3. 计算需要下载的文件
    let tasks = compute_download_tasks(&remote_manifest, &local_manifest, game_folder);
    if tasks.is_empty() {
        crate::log_info!("[Snowbreak] 所有文件已是最新");
        snowbreak::save_local_manifest(game_folder, &remote_manifest)?;
        return Ok(());
    }

    let total_size: u64 = tasks.iter().map(|t| t.size).sum();
    let total_count = tasks.len();
    crate::log_info!(
        "[Snowbreak] 需下载 {} 个文件, 总大小 {:.1} MB",
        total_count,
        total_size as f64 / 1024.0 / 1024.0
    );

    // 4. 创建游戏目录
    std::fs::create_dir_all(game_folder).map_err(|e| format!("创建游戏目录失败: {}", e))?;

    // 5. 逐文件下载（CDN 轮询重试）
    let client = Client::builder()
        .user_agent("Mozilla/5.0")
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let mut finished_size: u64 = 0;
    let mut speed_tracker = SpeedTracker::new();

    for (finished_count, task) in tasks.iter().enumerate() {
        if *cancel_token.lock().await {
            return Err("Download cancelled".to_string());
        }

        let dest = safe_join_remote(game_folder, &task.name)?;
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("创建目录失败: {}", e))?;
        }

        // CDN 轮询下载
        download_file_with_cdn_fallback(
            &app,
            &client,
            &cdn,
            &remote_manifest.path_offset,
            &task.hash,
            &task.name,
            task.size,
            &dest,
            &mut finished_size,
            total_size,
            total_count,
            finished_count,
            &mut speed_tracker,
            cancel_token.clone(),
        )
        .await?;

        finished_size += task.size;
    }

    // 6. 保存本地 manifest
    snowbreak::save_local_manifest(game_folder, &remote_manifest)?;

    emit_progress(
        &app,
        "完成",
        total_size,
        total_size,
        total_count,
        total_count,
        "下载完成",
        0,
        0,
    );

    crate::log_info!("[Snowbreak] 下载完成, 版本: {}", remote_manifest.version);
    Ok(())
}

// ============================================================
// 文件校验
// ============================================================

pub async fn verify_game(
    app: AppHandle,
    game_folder: &Path,
    source_policy: snowbreak::SourcePolicy,
    cancel_token: Arc<Mutex<bool>>,
) -> Result<crate::downloader::verifier::VerifyResult, String> {
    let (remote_manifest, cdn) = snowbreak::fetch_manifest_with_policy(source_policy).await?;

    let total_files = remote_manifest.paks.len();
    let mut verified_ok: usize = 0;
    let mut redownloaded: usize = 0;
    let mut failed: Vec<String> = Vec::new();
    let mut speed_tracker = SpeedTracker::new();

    let client = Client::builder()
        .user_agent("Mozilla/5.0")
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    for (idx, pak) in remote_manifest.paks.iter().enumerate() {
        if *cancel_token.lock().await {
            return Err("Verify cancelled".to_string());
        }

        let file_path = match safe_join(game_folder, &pak.name) {
            Ok(p) => p,
            Err(e) => {
                crate::log_warn!("跳过不安全的清单路径: {} ({})", pak.name, e);
                continue;
            }
        };
        let file_ok = if file_path.exists() {
            // 使用 MD5 校验
            match crate::utils::hash_verify::md5_file(&file_path).await {
                Ok(hash) => hash.eq_ignore_ascii_case(&pak.hash),
                Err(_) => false,
            }
        } else {
            false
        };

        emit_progress(
            &app,
            "校验",
            total_files as u64,
            (idx + 1) as u64,
            total_files,
            idx + 1,
            &pak.name,
            0,
            0,
        );

        if file_ok {
            verified_ok += 1;
        } else {
            // 重新下载
            if let Some(parent) = file_path.parent() {
                tokio::fs::create_dir_all(parent).await.ok();
            }

            let mut dummy_finished: u64 = 0;
            match download_file_with_cdn_fallback(
                &app,
                &client,
                &cdn,
                &remote_manifest.path_offset,
                &pak.hash,
                &pak.name,
                pak.size_in_bytes,
                &file_path,
                &mut dummy_finished,
                0,
                total_files,
                idx,
                &mut speed_tracker,
                cancel_token.clone(),
            )
            .await
            {
                Ok(_) => redownloaded += 1,
                Err(e) => {
                    crate::log_warn!("[Snowbreak] 重下失败 {}: {}", pak.name, e);
                    failed.push(pak.name.clone());
                }
            }
        }
    }

    // 保存最新 manifest
    snowbreak::save_local_manifest(game_folder, &remote_manifest)?;

    Ok(crate::downloader::verifier::VerifyResult {
        total_files,
        verified_ok,
        redownloaded,
        failed,
    })
}

// ============================================================
// 内部辅助
// ============================================================

struct DownloadTask {
    name: String,
    hash: String,
    size: u64,
}

/// 对比远程和本地 manifest，返回需要下载的文件列表
fn compute_download_tasks(
    remote: &Manifest,
    local: &Option<Manifest>,
    game_folder: &Path,
) -> Vec<DownloadTask> {
    let local_map: std::collections::HashMap<String, String> = local
        .as_ref()
        .map(|m| {
            m.paks
                .iter()
                .map(|p| (p.name.clone(), p.hash.clone()))
                .collect()
        })
        .unwrap_or_default();

    remote
        .paks
        .iter()
        .filter(|pak| {
            let file_path = match safe_join(game_folder, &pak.name) {
                Ok(p) => p,
                Err(_) => return false, // 路径不安全，跳过
            };
            // 文件不存在 -> 需要下载
            if !file_path.exists() {
                return true;
            }
            // 本地 manifest 中哈希不匹配 -> 需要下载
            match local_map.get(&pak.name) {
                Some(local_hash) => !local_hash.eq_ignore_ascii_case(&pak.hash),
                None => true, // 本地 manifest 中没有此文件
            }
        })
        .map(|pak| DownloadTask {
            name: pak.name.clone(),
            hash: pak.hash.clone(),
            size: pak.size_in_bytes,
        })
        .collect()
}

/// 加载本地 manifest
fn load_local_manifest(game_folder: &Path) -> Option<Manifest> {
    let path = game_folder.join("manifest.json");
    if !path.exists() {
        return None;
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

/// 从多个 CDN 轮询下载单个文件
#[allow(clippy::too_many_arguments)]
async fn download_file_with_cdn_fallback(
    app: &AppHandle,
    client: &Client,
    cdn: &ResolvedCdn,
    path_offset: &str,
    file_hash: &str,
    file_name: &str,
    _file_size: u64,
    dest: &Path,
    finished_size: &mut u64,
    total_size: u64,
    total_count: usize,
    current_idx: usize,
    speed_tracker: &mut SpeedTracker,
    cancel_token: Arc<Mutex<bool>>,
) -> Result<(), String> {
    let mut last_err = String::new();

    for idx in 0..cdn.len() {
        let url = cdn.file_url(idx, path_offset, file_hash);

        match download_single_file(
            app,
            client,
            &url,
            file_name,
            dest,
            finished_size,
            total_size,
            total_count,
            current_idx,
            speed_tracker,
            cancel_token.clone(),
        )
        .await
        {
            Ok(_) => return Ok(()),
            Err(e) => {
                if e.contains("cancelled") {
                    return Err(e);
                }
                last_err = format!("{} (cdn #{})", e, idx);
                crate::log_warn!("[Snowbreak] CDN #{} 下载失败: {}, 尝试下一个", idx, e);
                continue;
            }
        }
    }

    Err(format!("所有 CDN 下载失败 [{}]: {}", file_name, last_err))
}

/// 下载单个文件（带进度上报）
#[allow(clippy::too_many_arguments)]
async fn download_single_file(
    app: &AppHandle,
    client: &Client,
    url: &str,
    file_name: &str,
    dest: &Path,
    finished_size: &mut u64,
    total_size: u64,
    total_count: usize,
    current_idx: usize,
    speed_tracker: &mut SpeedTracker,
    cancel_token: Arc<Mutex<bool>>,
) -> Result<(), String> {
    let temp_path = dest.with_extension("tmp");

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let mut file = tokio::fs::File::create(&temp_path)
        .await
        .map_err(|e| format!("创建临时文件失败: {}", e))?;

    let mut stream = resp.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_emit = std::time::Instant::now();

    while let Some(chunk_result) = stream.next().await {
        if *cancel_token.lock().await {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err("Download cancelled".to_string());
        }

        let chunk = chunk_result.map_err(|e: reqwest::Error| format!("流读取错误: {}", e))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("写入错误: {}", e))?;

        downloaded += chunk.len() as u64;
        speed_tracker.record(chunk.len() as u64);

        if last_emit.elapsed() > std::time::Duration::from_millis(200) {
            let current_total = *finished_size + downloaded;
            let speed = speed_tracker.speed_bps();
            let remaining = total_size.saturating_sub(current_total);

            emit_progress(
                app,
                "下载",
                total_size,
                current_total,
                total_count,
                current_idx + 1,
                file_name,
                speed,
                speed_tracker.eta_seconds(remaining),
            );
            last_emit = std::time::Instant::now();
        }
    }

    file.flush()
        .await
        .map_err(|e| format!("flush 失败: {}", e))?;
    drop(file);

    // 重命名临时文件
    tokio::fs::rename(&temp_path, dest)
        .await
        .map_err(|e| format!("重命名失败: {}", e))?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn emit_progress(
    app: &AppHandle,
    phase: &str,
    total_size: u64,
    finished_size: u64,
    total_count: usize,
    finished_count: usize,
    current_file: &str,
    speed_bps: u64,
    eta_seconds: u64,
) {
    let _ = app.emit(
        "game-download-progress",
        DownloadProgress {
            phase: phase.to_string(),
            total_size,
            finished_size,
            total_count,
            finished_count,
            current_file: current_file.to_string(),
            speed_bps,
            eta_seconds,
        },
    );
}
