use crate::downloader::cdn::{self, LauncherInfo, ResourceIndex};
use crate::downloader::fetcher;
use crate::downloader::progress::DownloadProgress;
use crate::downloader::staging;
use crate::events::{
    emit_game_download_progress, GameDownloadOperation, GameDownloadProgressEvent,
};
use crate::utils::file_manager::safe_join_remote;
use crate::utils::hash_verify;
use reqwest::Client;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::Mutex;
use tracing::{info, warn};

fn emit_update_progress(
    app: &AppHandle,
    task_id: &str,
    operation: GameDownloadOperation,
    progress: &DownloadProgress,
) {
    emit_game_download_progress(
        app,
        &GameDownloadProgressEvent::from_progress(task_id, operation, progress),
    );
}

/// Full comparison update — mirrors LutheringLaves.py `update_game`
/// Compares checksum of each file (prefer SHA256) and re-downloads mismatches.
pub async fn update_game_full(
    app: AppHandle,
    task_id: &str,
    launcher_info: &LauncherInfo,
    resource_index: &ResourceIndex,
    game_folder: &Path,
    cancel_token: Arc<Mutex<bool>>,
) -> Result<(), String> {
    let client = Client::new();
    let total_count = resource_index.resource.len();
    let total_size: u64 = resource_index.resource.iter().map(|r| r.size).sum();

    info!(
        "Starting full comparison update: {} files, {} bytes",
        total_count, total_size
    );

    let mut finished_size: u64 = 0;
    let mut finished_count: usize = 0;
    let start_time = std::time::Instant::now();

    for file in &resource_index.resource {
        if *cancel_token.lock().await {
            return Err("Update cancelled".to_string());
        }

        let file_path = match safe_join_remote(game_folder, &file.dest) {
            Ok(p) => p,
            Err(e) => {
                warn!("跳过不安全的清单路径: {} ({})", file.dest, e);
                continue;
            }
        };

        let verified = hash_verify::verify_file_integrity(
            &file_path,
            file.size,
            file.sha256.as_deref(),
            Some(file.md5.as_str()),
        )
        .await;
        if verified.is_ok() {
            info!("{} checksum match, skipping", file.dest);
            finished_size += file.size;
            finished_count += 1;

            let elapsed_secs = start_time.elapsed().as_secs_f64();
            let speed = if elapsed_secs > 1.0 {
                (finished_size as f64 / elapsed_secs) as u64
            } else {
                0
            };
            let remaining = total_size.saturating_sub(finished_size);
            let eta = if speed > 0 { remaining / speed } else { 0 };
            let progress = DownloadProgress {
                phase: "update".to_string(),
                total_size,
                finished_size,
                total_count,
                finished_count,
                current_file: file.dest.clone(),
                speed_bps: speed,
                eta_seconds: eta,
            };
            emit_update_progress(&app, task_id, GameDownloadOperation::UpdateGame, &progress);
            continue;
        }

        warn!(
            "{} checksum mismatch, re-downloading: {}",
            file.dest,
            verified.err().unwrap_or_else(|| "unknown".to_string())
        );

        let download_url = build_resource_url(
            &launcher_info.cdn_url,
            &launcher_info.resources_base_path,
            &file.dest,
        );

        fetcher::download_with_resume(&client, &download_url, &file_path, true, None, None).await?;

        finished_size += file.size;
        finished_count += 1;

        let elapsed_secs = start_time.elapsed().as_secs_f64();
        let speed = if elapsed_secs > 1.0 {
            (finished_size as f64 / elapsed_secs) as u64
        } else {
            0
        };
        let remaining = total_size.saturating_sub(finished_size);
        let eta = if speed > 0 { remaining / speed } else { 0 };
        let progress = DownloadProgress {
            phase: "update".to_string(),
            total_size,
            finished_size,
            total_count,
            finished_count,
            current_file: file.dest.clone(),
            speed_bps: speed,
            eta_seconds: eta,
        };
        emit_update_progress(&app, task_id, GameDownloadOperation::UpdateGame, &progress);
    }

    info!("Full comparison update complete");
    Ok(())
}

/// Incremental patch update — mirrors LutheringLaves.py `download_patch` + `merge_patch`
pub async fn update_game_patch(
    app: AppHandle,
    task_id: &str,
    launcher_info: &LauncherInfo,
    local_version: &str,
    game_folder: &Path,
    cancel_token: Arc<Mutex<bool>>,
) -> Result<(), String> {
    // Find matching patch config
    let patch = launcher_info
        .patch_configs
        .iter()
        .find(|p| p.version == local_version)
        .ok_or_else(|| format!("No patch config found for version {}", local_version))?;

    info!(
        "Starting incremental patch from {} to latest",
        local_version
    );

    let client = Client::new();

    // Fetch patch resource index
    let patch_index_url = cdn::join_url(&launcher_info.cdn_url, &patch.index_file);
    let patch_index: crate::downloader::cdn::ResourceIndex = {
        let resp_text = client
            .get(&patch_index_url)
            .header("User-Agent", "Mozilla/5.0")
            .send()
            .await
            .map_err(|e| format!("Failed to fetch patch index: {}", e))?
            .text()
            .await
            .map_err(|e| format!("Failed to read patch index: {}", e))?;

        let data: serde_json::Value = serde_json::from_str(&resp_text)
            .map_err(|e| format!("Failed to parse patch index: {}", e))?;

        let resources = data
            .get("resource")
            .and_then(|v| v.as_array())
            .ok_or("Missing resource in patch index")?;

        let resource_list = resources
            .iter()
            .filter_map(|r| {
                Some(cdn::ResourceFile {
                    dest: r.get("dest")?.as_str()?.to_string(),
                    md5: r
                        .get("md5")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    sha256: r
                        .get("sha256")
                        .or_else(|| r.get("sha_256"))
                        .or_else(|| r.get("sha256sum"))
                        .or_else(|| r.get("sha256Sum"))
                        .and_then(|v| v.as_str())
                        .map(str::trim)
                        .filter(|v| !v.is_empty())
                        .map(ToString::to_string),
                    size: r
                        .get("size")?
                        .as_u64()
                        .or_else(|| r.get("size")?.as_str()?.parse().ok())?,
                })
            })
            .collect();

        cdn::ResourceIndex {
            resource: resource_list,
        }
    };
    let total_count = patch_index.resource.len();
    let total_size: u64 = patch_index.resource.iter().map(|f| f.size).sum();
    let mut finished_count: usize = 0;
    let mut finished_size: u64 = 0;
    let start_time = std::time::Instant::now();

    // Create temp folder
    let temp_folder = game_folder
        .parent()
        .unwrap_or(game_folder)
        .join("temp_folder");
    tokio::fs::create_dir_all(&temp_folder)
        .await
        .map_err(|e| format!("Failed to create temp folder: {}", e))?;

    // Download patch files
    let mut krdiff_path: Option<PathBuf> = None;

    for file in &patch_index.resource {
        if *cancel_token.lock().await {
            cleanup_temp(&temp_folder).await;
            return Err("Patch update cancelled".to_string());
        }

        // Files with fromFolder go to temp_folder, krdiff files go to base_dir
        let download_url = cdn::join_url(
            &launcher_info.cdn_url,
            &format!("{}/{}", patch.base_url, file.dest),
        );

        if file.dest.ends_with(".krdiff") || file.dest.ends_with(".hdiff") {
            let dest_path = game_folder.parent().unwrap_or(game_folder).join(&file.dest);
            fetcher::download_with_resume(&client, &download_url, &dest_path, false, None, None)
                .await?;
            hash_verify::verify_file_integrity(
                &dest_path,
                file.size,
                file.sha256.as_deref(),
                Some(file.md5.as_str()),
            )
            .await?;
            krdiff_path = Some(dest_path);
        } else {
            let dest_path = temp_folder.join(&file.dest);
            fetcher::download_with_resume(&client, &download_url, &dest_path, false, None, None)
                .await?;
            hash_verify::verify_file_integrity(
                &dest_path,
                file.size,
                file.sha256.as_deref(),
                Some(file.md5.as_str()),
            )
            .await?;
        }
        finished_count += 1;
        finished_size = finished_size.saturating_add(file.size);

        let elapsed_secs = start_time.elapsed().as_secs_f64();
        let speed = if elapsed_secs > 1.0 {
            (finished_size as f64 / elapsed_secs) as u64
        } else {
            0
        };
        let remaining = total_size.saturating_sub(finished_size);
        let eta = if speed > 0 { remaining / speed } else { 0 };

        let progress = DownloadProgress {
            phase: "patch".to_string(),
            total_size,
            finished_size,
            total_count,
            finished_count,
            current_file: file.dest.clone(),
            speed_bps: speed,
            eta_seconds: eta,
        };
        emit_update_progress(
            &app,
            task_id,
            GameDownloadOperation::UpdateGamePatch,
            &progress,
        );
    }

    // Run hpatchz if needed
    if let Some(diff_path) = &krdiff_path {
        run_hpatchz(diff_path, game_folder, &temp_folder).await?;
    }

    // Merge temp files into game folder
    merge_temp_to_game(&temp_folder, game_folder).await?;

    // Cleanup
    cleanup_temp(&temp_folder).await;
    if let Some(diff_path) = &krdiff_path {
        tokio::fs::remove_file(diff_path).await.ok();
    }

    info!("Incremental patch update complete");
    Ok(())
}

async fn run_hpatchz(
    patch_path: &Path,
    original_path: &Path,
    output_path: &Path,
) -> Result<(), String> {
    let hpatchz = ensure_hpatchz().await?;
    // 执行前再次做完整性检查，降低运行时被替换二进制的风险。
    verify_hpatchz_integrity(&hpatchz).await?;

    info!(
        "Running hpatchz: {} {} {} -f",
        original_path.display(),
        patch_path.display(),
        output_path.display()
    );

    let output = tokio::process::Command::new(&hpatchz)
        .arg(original_path)
        .arg(patch_path)
        .arg(output_path)
        .arg("-f")
        .output()
        .await
        .map_err(|e| format!("Failed to run hpatchz: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("hpatchz failed: {}", stderr));
    }

    Ok(())
}

pub async fn ensure_hpatchz_public() -> Result<PathBuf, String> {
    ensure_hpatchz().await
}

/// hpatchz 允许的 SHA256 白名单（多版本兼容）
/// 新版本发布时在此追加哈希即可。
/// 默认校验顺序为：可信 sidecar -> 白名单 -> 不安全环境变量放行。
/// 若 sidecar 与白名单都不存在，则默认 fail-closed。
const HPATCHZ_ALLOWED_SHA256: &[&str] = &[
    // 如需固定版本，请在此填入已知的 SHA256 哈希（小写十六进制）
    // 例如: "a1b2c3d4e5f6..."
    // 留空时可通过环境变量 SSMT4_HPATCHZ_ALLOWED_SHA256 传入（逗号分隔）
];

/// 固定到已验证的官方 HDiffPatch 发行包，避免 `latest` 漂移导致默认安装失效。
/// SHA256 取自官方发布页的 linux64 zip 资产摘要。
const HPATCHZ_OFFICIAL_RELEASE_VERSION: &str = "v4.12.2";
const HPATCHZ_OFFICIAL_RELEASE_ZIP_NAME: &str = "hdiffpatch_v4.12.2_bin_linux64.zip";
const HPATCHZ_OFFICIAL_RELEASE_ZIP_SHA256: &str =
    "432886341e6099adeb5223427454e9bc0b565f81edfbe2e3387f804611c07e4a";

/// hpatchz 最小合理大小（字节），低于此值视为损坏/截断
const HPATCHZ_MIN_SIZE: u64 = 100_000; // ~100KB

/// 仅供受信环境应急使用：允许在未配置 SHA256 白名单时继续执行 hpatchz。
/// 默认关闭（fail-closed）。
const HPATCHZ_ALLOW_UNVERIFIED_ENV: &str = "SSMT4_HPATCHZ_ALLOW_UNVERIFIED";

async fn ensure_hpatchz() -> Result<PathBuf, String> {
    let tools_dir = crate::utils::file_manager::get_tools_dir();
    let hpatchz_path = tools_dir.join("hpatchz");
    let sidecar_path = hpatchz_hash_sidecar_path(&hpatchz_path);

    if hpatchz_path.exists() {
        match verify_hpatchz_integrity(&hpatchz_path).await {
            Ok(()) => return Ok(hpatchz_path),
            Err(err) => {
                warn!(
                    "现有 hpatchz 校验失败，准备重新下载官方发行包: {} ({})",
                    hpatchz_path.display(),
                    err
                );
                std::fs::remove_file(&hpatchz_path).ok();
                std::fs::remove_file(&sidecar_path).ok();
            }
        }
    }

    crate::utils::file_manager::ensure_dir(&tools_dir)?;

    let zip_path = tools_dir.join(HPATCHZ_OFFICIAL_RELEASE_ZIP_NAME);
    let github_url = format!(
        "https://github.com/sisong/HDiffPatch/releases/download/{}/{}",
        HPATCHZ_OFFICIAL_RELEASE_VERSION, HPATCHZ_OFFICIAL_RELEASE_ZIP_NAME
    );
    let client = Client::new();

    info!(
        "Downloading hpatchz official release asset from GitHub: {}",
        github_url
    );
    fetcher::download_simple(&client, &github_url, &zip_path).await?;

    verify_hpatchz_release_zip(&zip_path).await?;
    extract_hpatchz_from_release_zip(&zip_path, &hpatchz_path).await?;
    let actual_hash = crate::utils::hash_verify::sha256_file(&hpatchz_path).await?;
    write_hpatchz_hash_sidecar(&hpatchz_path, &actual_hash)?;
    verify_hpatchz_integrity(&hpatchz_path).await?;
    std::fs::remove_file(&zip_path).ok();

    // chmod +x
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&hpatchz_path)
            .map_err(|e| format!("Failed to get hpatchz metadata: {}", e))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hpatchz_path, perms)
            .map_err(|e| format!("Failed to chmod hpatchz: {}", e))?;
    }

    info!("hpatchz downloaded and ready at {}", hpatchz_path.display());
    Ok(hpatchz_path)
}

/// 验证 hpatchz 二进制完整性
async fn verify_hpatchz_integrity(path: &Path) -> Result<(), String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("无法读取 hpatchz 元数据: {}", e))?;
    let sidecar_path = hpatchz_hash_sidecar_path(path);

    // 1. 大小校验
    if meta.len() < HPATCHZ_MIN_SIZE {
        std::fs::remove_file(path).ok();
        std::fs::remove_file(&sidecar_path).ok();
        return Err(format!(
            "hpatchz 文件异常（大小 {} 字节，低于最小阈值 {}），已删除",
            meta.len(),
            HPATCHZ_MIN_SIZE
        ));
    }

    // 2. SHA256 哈希校验（默认 fail-closed）
    let actual_hash = crate::utils::hash_verify::sha256_file(path).await?;
    if let Some(expected_hash) = read_hpatchz_hash_sidecar(path) {
        if actual_hash != expected_hash {
            std::fs::remove_file(path).ok();
            std::fs::remove_file(&sidecar_path).ok();
            return Err(format!(
                "hpatchz SHA256 校验失败（实际: {}，期望: {}），已删除",
                actual_hash, expected_hash
            ));
        }
        info!("hpatchz SHA256 sidecar 校验通过: {}", actual_hash);
    } else {
        let allowed_sha256 = collect_hpatchz_allowed_sha256();
        if !allowed_sha256.is_empty() {
            if !allowed_sha256.iter().any(|h| h == &actual_hash) {
                std::fs::remove_file(path).ok();
                std::fs::remove_file(&sidecar_path).ok();
                return Err(format!(
                    "hpatchz SHA256 校验失败（实际: {}），不在允许列表中，已删除",
                    actual_hash
                ));
            }
            info!("hpatchz SHA256 校验通过: {}", actual_hash);
        } else {
            if allow_unverified_hpatchz() {
                warn!(
                    "未配置 hpatchz SHA256 白名单，但 {}=1，已按不安全模式放行。请仅在受信环境临时使用。",
                    HPATCHZ_ALLOW_UNVERIFIED_ENV
                );
            } else {
                std::fs::remove_file(path).ok();
                std::fs::remove_file(&sidecar_path).ok();
                return Err(format!(
                    "未配置 hpatchz SHA256 白名单，且不存在可信 sidecar，已拒绝执行并删除文件。请配置允许列表（SSMT4_HPATCHZ_ALLOWED_SHA256），或仅在受信环境设置 {}=1 临时放行。",
                    HPATCHZ_ALLOW_UNVERIFIED_ENV
                ));
            }
        }
    }

    // 3. ELF 魔数校验（Linux 可执行文件基本验证）
    #[cfg(unix)]
    {
        let header = std::fs::read(path).map_err(|e| format!("无法读取 hpatchz: {}", e))?;
        if header.len() < 4 || &header[..4] != b"\x7fELF" {
            std::fs::remove_file(path).ok();
            std::fs::remove_file(&sidecar_path).ok();
            return Err("hpatchz 不是有效的 ELF 可执行文件，已删除".to_string());
        }
    }

    Ok(())
}

fn hpatchz_hash_sidecar_path(path: &Path) -> PathBuf {
    path.with_extension("sha256")
}

fn read_hpatchz_hash_sidecar(path: &Path) -> Option<String> {
    std::fs::read_to_string(hpatchz_hash_sidecar_path(path))
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
}

fn write_hpatchz_hash_sidecar(path: &Path, sha256: &str) -> Result<(), String> {
    let sidecar_path = hpatchz_hash_sidecar_path(path);
    std::fs::write(
        &sidecar_path,
        format!("{}\n", sha256.trim().to_ascii_lowercase()),
    )
    .map_err(|e| format!("写入 hpatchz SHA256 sidecar 失败: {}", e))
}

async fn verify_hpatchz_release_zip(path: &Path) -> Result<(), String> {
    let actual_hash = crate::utils::hash_verify::sha256_file(path).await?;
    if actual_hash != HPATCHZ_OFFICIAL_RELEASE_ZIP_SHA256 {
        std::fs::remove_file(path).ok();
        return Err(format!(
            "官方 hpatchz ZIP SHA256 校验失败（实际: {}，期望: {}），已删除",
            actual_hash, HPATCHZ_OFFICIAL_RELEASE_ZIP_SHA256
        ));
    }
    Ok(())
}

async fn extract_hpatchz_from_release_zip(zip_path: &Path, dest_path: &Path) -> Result<(), String> {
    let zip_path = zip_path.to_path_buf();
    let dest_path = dest_path.to_path_buf();

    tokio::task::spawn_blocking(move || {
        extract_hpatchz_from_release_zip_blocking(&zip_path, &dest_path)
    })
    .await
    .map_err(|e| format!("等待 hpatchz ZIP 解压任务失败: {}", e))?
}

fn extract_hpatchz_from_release_zip_blocking(
    zip_path: &Path,
    dest_path: &Path,
) -> Result<(), String> {
    let file =
        std::fs::File::open(zip_path).map_err(|e| format!("打开 hpatchz ZIP 失败: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("解析 hpatchz ZIP 失败: {}", e))?;

    let Some(parent) = dest_path.parent() else {
        return Err("hpatchz 目标路径缺少父目录".to_string());
    };
    std::fs::create_dir_all(parent).map_err(|e| format!("创建 tools 目录失败: {}", e))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("读取 hpatchz ZIP 条目失败: {}", e))?;
        let Some(relative_path) = entry.enclosed_name().map(|p| p.to_path_buf()) else {
            continue;
        };
        if entry.is_dir() {
            continue;
        }
        if relative_path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "hpatchz")
        {
            let mut outfile = std::fs::File::create(dest_path)
                .map_err(|e| format!("创建 hpatchz 失败: {}", e))?;
            std::io::copy(&mut entry, &mut outfile)
                .map_err(|e| format!("写入 hpatchz 失败: {}", e))?;
            return Ok(());
        }
    }

    Err("官方 hpatchz ZIP 中未找到 hpatchz 可执行文件".to_string())
}

fn collect_hpatchz_allowed_sha256() -> Vec<String> {
    let mut allowed: Vec<String> = HPATCHZ_ALLOWED_SHA256
        .iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    if let Ok(env_value) = std::env::var("SSMT4_HPATCHZ_ALLOWED_SHA256") {
        allowed.extend(
            env_value
                .split(',')
                .map(str::trim)
                .map(|s| s.to_ascii_lowercase())
                .filter(|s| !s.is_empty()),
        );
    }
    allowed.sort();
    allowed.dedup();
    allowed
}

fn allow_unverified_hpatchz() -> bool {
    std::env::var(HPATCHZ_ALLOW_UNVERIFIED_ENV)
        .map(|v| parse_env_bool(&v))
        .unwrap_or(false)
}

fn parse_env_bool(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

async fn merge_temp_to_game(temp_folder: &Path, game_folder: &Path) -> Result<(), String> {
    staging::merge_staging_tree_atomically(temp_folder, game_folder, "incremental_patch").await
}

async fn cleanup_temp(temp_folder: &Path) {
    if temp_folder.exists() {
        tokio::fs::remove_dir_all(temp_folder).await.ok();
    }
}

fn build_resource_url(cdn_url: &str, resources_base_path: &str, dest: &str) -> String {
    let base = cdn_url.trim_end_matches('/');
    let mid = resources_base_path.trim_matches('/');
    let file = dest.trim_start_matches('/');
    format!("{}/{}/{}", base, mid, file)
}

#[cfg(test)]
mod tests {
    use super::{
        extract_hpatchz_from_release_zip_blocking, hpatchz_hash_sidecar_path, parse_env_bool,
        verify_hpatchz_integrity, write_hpatchz_hash_sidecar,
    };
    use crate::utils::hash_verify;
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ssmt4-hpatchz-{label}-{nonce}"));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn fake_elf_bytes(len: usize) -> Vec<u8> {
        let mut bytes = vec![0u8; len.max(4)];
        bytes[..4].copy_from_slice(b"\x7fELF");
        bytes[4..].fill(b'X');
        bytes
    }

    #[test]
    fn parse_env_bool_accepts_truthy_values() {
        assert!(parse_env_bool("1"));
        assert!(parse_env_bool("true"));
        assert!(parse_env_bool("YES"));
        assert!(parse_env_bool(" on "));
    }

    #[test]
    fn parse_env_bool_rejects_non_truthy_values() {
        assert!(!parse_env_bool(""));
        assert!(!parse_env_bool("0"));
        assert!(!parse_env_bool("false"));
        assert!(!parse_env_bool("no"));
        assert!(!parse_env_bool("random"));
    }

    #[tokio::test]
    async fn verify_hpatchz_integrity_accepts_matching_sidecar_hash() {
        let dir = unique_temp_dir("sidecar");
        let binary_path = dir.join("hpatchz");
        std::fs::write(&binary_path, fake_elf_bytes(120_000)).expect("write hpatchz");
        let sha256 = hash_verify::sha256_file(&binary_path)
            .await
            .expect("hash hpatchz");
        write_hpatchz_hash_sidecar(&binary_path, &sha256).expect("write sidecar");

        verify_hpatchz_integrity(&binary_path)
            .await
            .expect("verify hpatchz with sidecar");

        assert_eq!(
            std::fs::read_to_string(hpatchz_hash_sidecar_path(&binary_path))
                .expect("read sidecar")
                .trim(),
            sha256
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn extract_hpatchz_from_release_zip_blocking_finds_nested_binary() {
        let dir = unique_temp_dir("zip");
        let zip_path = dir.join("hpatchz.zip");
        let binary_path = dir.join("out").join("hpatchz");

        let file = std::fs::File::create(&zip_path).expect("create zip");
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::FileOptions::default();
        writer
            .start_file("nested/bin/hpatchz", options)
            .expect("start hpatchz entry");
        writer
            .write_all(&fake_elf_bytes(256))
            .expect("write hpatchz entry");
        writer
            .start_file("nested/bin/hdiffz", options)
            .expect("start hdiffz entry");
        writer.write_all(b"other").expect("write other entry");
        writer.finish().expect("finish zip");

        extract_hpatchz_from_release_zip_blocking(&zip_path, &binary_path)
            .expect("extract hpatchz");

        assert!(binary_path.exists());
        assert_eq!(
            std::fs::read(&binary_path).expect("read extracted hpatchz")[..4],
            *b"\x7fELF"
        );

        let _ = std::fs::remove_dir_all(dir);
    }
}
