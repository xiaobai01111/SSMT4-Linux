use super::*;
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

/// 下载计划：描述"下载什么"和"安装策略"
pub(super) struct DownloadPlan {
    pub(super) all_tasks: Vec<DownloadTask>,
    /// 前 N 个任务属于主包（游戏本体/补丁），其余为语言包
    pub(super) primary_pkg_count: usize,
    /// 主包显示标签（如 "游戏本体" / "游戏补丁"）
    pub(super) primary_label: String,
    /// 是否尝试迁移旧格式文件（_download_N.zip）— 仅全量下载需要
    pub(super) migrate_old_files: bool,
    /// 安装完成后的收尾动作
    pub(super) post_install: PostInstall,
}

/// 安装完成后的收尾策略
pub(super) enum PostInstall {
    /// 全量下载：只写版本号
    WriteVersion { version: String },
    /// 增量更新：应用 hdiff + 清理 deletefiles + 写版本号
    PatchAndWriteVersion { version: String },
}

/// 统一流水线：预检 → 并行下载 → 安装主包 → 安装语言包 → 收尾
pub(super) async fn execute_plan(
    app: AppHandle,
    task_id: &str,
    operation: GameDownloadOperation,
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

        if let Some(cached_digest) = sw.state.checksums.get(&task.filename) {
            if digest_token_matches(cached_digest, task) {
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
                    task_id,
                    operation,
                    "download",
                    DownloadProgressStats {
                        finished_size: cached_size,
                        total_size,
                        finished_count: i + 1,
                        total_count,
                        ..DownloadProgressStats::default()
                    },
                    format!("已缓存，跳过 {}", task.label),
                );
                continue;
            }
        }

        if migrate_old_files && !dest.exists() && task_has_checksum(task) {
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

        if dest.exists() && task_has_checksum(task) {
            let file_size = tokio::fs::metadata(&dest)
                .await
                .map(|m| m.len())
                .unwrap_or(0);
            needs_hash.push((i, dest, file_size, false));
            continue;
        }

        if dest.exists() && !task_has_checksum(task) {
            warn!(
                "{} 缺少校验哈希，已禁用 size-only 跳过，将强制重下",
                task.filename
            );
        }

        to_download.push(i);
    }

    if !needs_hash.is_empty() {
        const HASH_CONCURRENCY: usize = 4;

        let mut still_needs_hash: Vec<(usize, PathBuf, u64, bool)> = Vec::new();
        for (idx, path, file_size, is_old) in needs_hash {
            let cache_key = make_hash_cache_key(&path, file_size);
            if let Some(cached) = sw.state.file_hashes.get(&cache_key) {
                let task = &all_tasks[idx];
                if digest_token_matches(cached, task) {
                    if is_old {
                        let dest = safe_join(game_folder, &task.filename)?;
                        info!(
                            "迁移旧文件 (mtime缓存) {} → {}",
                            path.display(),
                            dest.display()
                        );
                        tokio::fs::rename(&path, &dest).await.ok();
                    }
                    if let Some(digest_token) = task_digest_token(task) {
                        sw.state
                            .checksums
                            .insert(task.filename.clone(), digest_token);
                    }
                    sw.mark_dirty();
                    cached_size += file_size;
                    info!("{} mtime缓存命中，跳过哈希", task.filename);
                    emit_progress(
                        &app,
                        task_id,
                        operation,
                        "download",
                        DownloadProgressStats {
                            finished_size: cached_size,
                            total_size,
                            finished_count: idx + 1,
                            total_count,
                            ..DownloadProgressStats::default()
                        },
                        format!("已缓存，跳过 {}", task.label),
                    );
                    continue;
                }
            }
            still_needs_hash.push((idx, path, file_size, is_old));
        }

        if !still_needs_hash.is_empty() {
            info!(
                "预检：{} 个文件需要哈希校验 ({}路并发)",
                still_needs_hash.len(),
                HASH_CONCURRENCY
            );

            let hash_sem = Arc::new(tokio::sync::Semaphore::new(HASH_CONCURRENCY));
            let mut hash_handles = Vec::new();

            for (idx, path, file_size, is_old) in still_needs_hash {
                let sem = hash_sem.clone();
                let cancel = cancel_token.clone();
                let expected_sha256 = all_tasks[idx].sha256.clone();
                let expected_md5 = all_tasks[idx].md5.clone();
                let handle = tokio::spawn(async move {
                    let _permit = sem.acquire().await.map_err(|e| format!("{}", e))?;
                    if *cancel.lock().await {
                        return Err("Download cancelled".to_string());
                    }
                    let digest_token = compute_digest_token_for_expected(
                        &path,
                        expected_sha256.as_str(),
                        expected_md5.as_str(),
                    )
                    .await?;
                    Ok::<(usize, PathBuf, u64, bool, String), String>((
                        idx,
                        path,
                        file_size,
                        is_old,
                        digest_token,
                    ))
                });
                hash_handles.push(handle);
            }

            for handle in hash_handles {
                let (idx, path, file_size, is_old, actual_digest_token) = match handle.await {
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

                let cache_key = make_hash_cache_key(&path, file_size);
                sw.state
                    .file_hashes
                    .insert(cache_key, actual_digest_token.clone());

                let task = &all_tasks[idx];
                if digest_token_matches(&actual_digest_token, task) {
                    if is_old {
                        let dest = safe_join(game_folder, &task.filename)?;
                        info!("迁移旧文件 {} → {}", path.display(), dest.display());
                        tokio::fs::rename(&path, &dest).await.ok();
                    }
                    if let Some(digest_token) = task_digest_token(task) {
                        sw.state
                            .checksums
                            .insert(task.filename.clone(), digest_token);
                    }
                    sw.mark_dirty();
                    cached_size += file_size;
                    info!("{} 哈希匹配，补录缓存", task.filename);
                    emit_progress(
                        &app,
                        task_id,
                        operation,
                        "download",
                        DownloadProgressStats {
                            finished_size: cached_size,
                            total_size,
                            finished_count: idx + 1,
                            total_count,
                            ..DownloadProgressStats::default()
                        },
                        format!("已缓存，跳过 {}", task.label),
                    );
                } else {
                    if is_old {
                        warn!("旧文件 {} 哈希不匹配，将重新下载", path.display());
                    } else {
                        warn!("{} 哈希不匹配，将重新下载", task.filename);
                    }
                    to_download.push(idx);
                }
            }
        }
    }

    sw.flush();

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
        let shared_active_streams = Arc::new(AtomicUsize::new(0));

        let reporter = {
            let app = app.clone();
            let task_id = task_id.to_string();
            let bytes = shared_bytes.clone();
            let done = shared_done.clone();
            let cancel = cancel_token.clone();
            let active_streams = shared_active_streams.clone();
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
                        operation,
                        "download",
                        DownloadProgressStats {
                            finished_size: current,
                            total_size,
                            finished_count: done.load(Ordering::Relaxed),
                            total_count,
                            speed_bps: tracker.speed_bps(),
                            eta_seconds: tracker.eta_seconds(remaining),
                        },
                        format!(
                            "并行下载中 (活跃连接: {}，任务并发上限: {})",
                            active_streams.load(Ordering::Relaxed),
                            MAX_CONCURRENT_DOWNLOADS
                        ),
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
            let active_streams = shared_active_streams.clone();
            let dest = safe_join(game_folder, &task.filename)?;
            let url = task.url.clone();
            let md5 = task.md5.clone();
            let sha256 = task.sha256.clone();
            let expected_size = task.size.parse::<u64>().unwrap_or(0);
            let filename = task.filename.clone();

            let handle = tokio::spawn(async move {
                let _permit = sem
                    .acquire()
                    .await
                    .map_err(|e| format!("信号量错误: {}", e))?;
                info!("开始下载: {}", filename);

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
                    match download_file_to_disk(DiskDownloadRequest {
                        client: &client,
                        url: &url,
                        dest: &dest,
                        expected_size,
                        expected_sha256: &sha256,
                        expected_md5: &md5,
                        shared_bytes: bytes.clone(),
                        cancel_token: cancel.clone(),
                        active_streams: active_streams.clone(),
                    })
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
                let final_digest = if let Some(token) = digest_token_from_expected(&sha256, &md5) {
                    token
                } else {
                    compute_digest_token_for_expected(&dest, &sha256, &md5).await?
                };
                info!("下载完成: {}", filename);
                Ok::<(String, String), String>((filename, final_digest))
            });
            handles.push(handle);
        }

        let mut first_error: Option<String> = None;
        for handle in handles {
            match handle.await {
                Ok(Ok((filename, digest_token))) => {
                    sw.state.checksums.insert(filename, digest_token);
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
        sw.flush();

        if let Some(e) = first_error {
            return Err(e);
        }
    }

    info!("全部 {} 个包下载完成，开始安装...", total_count);

    if primary_pkg_count > 0 {
        let first_file = &all_tasks[0].filename;
        if !sw.state.installed_archives.contains(first_file) {
            let first_part = safe_join(game_folder, first_file)?;
            if first_part.exists() {
                info!("安装{} (从 {})", primary_label, first_file);
                let primary_staging = create_install_staging_dir(game_folder, "primary");
                tokio::fs::create_dir_all(&primary_staging)
                    .await
                    .map_err(|e| format!("创建安装 staging 目录失败: {}", e))?;
                let primary_extract_ctx = ExtractProgressContext::new(
                    &app,
                    task_id,
                    operation,
                    &primary_label,
                    total_count,
                    1,
                );
                match extract_archive(&first_part, &primary_staging, &primary_extract_ctx).await {
                    Ok(()) => {
                        if let Err(e) = staging::merge_staging_tree_atomically(
                            &primary_staging,
                            game_folder,
                            "hoyoverse_primary_install",
                        )
                        .await
                        {
                            tokio::fs::remove_dir_all(&primary_staging).await.ok();
                            error!("{}安装提交失败: {}", primary_label, e);
                            for task in &all_tasks[..primary_pkg_count] {
                                sw.state.checksums.remove(&task.filename);
                                if let Ok(p) = safe_join(game_folder, &task.filename) {
                                    tokio::fs::remove_file(p).await.ok();
                                }
                            }
                            sw.flush();
                            return Err(format!(
                                "{}安装提交失败: {}。已回滚并删除安装包，请重新下载。",
                                primary_label, e
                            ));
                        }
                        tokio::fs::remove_dir_all(&primary_staging).await.ok();
                        for task in &all_tasks[..primary_pkg_count] {
                            if let Ok(p) = safe_join(game_folder, &task.filename) {
                                tokio::fs::remove_file(p).await.ok();
                            }
                        }
                        sw.state.installed_archives.push(first_file.to_string());
                        sw.flush();
                    }
                    Err(e) => {
                        tokio::fs::remove_dir_all(&primary_staging).await.ok();
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
        let lang_staging = create_install_staging_dir(game_folder, &task.label);
        tokio::fs::create_dir_all(&lang_staging)
            .await
            .map_err(|e| format!("创建安装 staging 目录失败: {}", e))?;
        let lang_extract_ctx = ExtractProgressContext::new(
            &app,
            task_id,
            operation,
            &task.label,
            total_count,
            primary_pkg_count + idx + 1,
        );
        match extract_archive(&archive, &lang_staging, &lang_extract_ctx).await {
            Ok(()) => {
                if let Err(e) = staging::merge_staging_tree_atomically(
                    &lang_staging,
                    game_folder,
                    "hoyoverse_lang_install",
                )
                .await
                {
                    tokio::fs::remove_dir_all(&lang_staging).await.ok();
                    error!("{} 安装提交失败: {}", task.label, e);
                    tokio::fs::remove_file(&archive).await.ok();
                    sw.state.checksums.remove(&task.filename);
                    sw.flush();
                    return Err(format!(
                        "安装 {} 提交失败: {}。已回滚并删除文件，请重新下载。",
                        task.label, e
                    ));
                }
                tokio::fs::remove_dir_all(&lang_staging).await.ok();
                tokio::fs::remove_file(&archive).await.ok();
                sw.state.installed_archives.push(task.filename.clone());
                sw.flush();
            }
            Err(e) => {
                tokio::fs::remove_dir_all(&lang_staging).await.ok();
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
