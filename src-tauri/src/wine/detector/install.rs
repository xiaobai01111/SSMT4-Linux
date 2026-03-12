use crate::events::{
    emit_component_download_progress, ComponentDownloadPhase, ComponentDownloadProgressEvent,
};
use futures_util::StreamExt;
use std::path::PathBuf;
use tracing::info;

/// 获取 Wine runners 安装目录（跟随自定义数据目录）
pub(crate) fn get_wine_runners_dir() -> PathBuf {
    crate::configs::app_config::get_app_data_dir()
        .join("runners")
        .join("wine")
}

/// 获取 Proton 安装目录（优先自定义数据目录）
pub(crate) fn get_proton_install_dir() -> PathBuf {
    crate::configs::app_config::get_app_data_dir().join("proton")
}

pub fn delete_local_proton(path: &str) -> Result<String, String> {
    let target = PathBuf::from(path);
    if !target.exists() {
        return Err(format!("目标不存在: {}", target.display()));
    }

    let canonical = target
        .canonicalize()
        .map_err(|e| format!("解析路径失败: {}", e))?;
    let proton_root = get_proton_install_dir()
        .canonicalize()
        .unwrap_or_else(|_| get_proton_install_dir());
    let wine_root = get_wine_runners_dir()
        .canonicalize()
        .unwrap_or_else(|_| get_wine_runners_dir());

    if !canonical.starts_with(&proton_root) && !canonical.starts_with(&wine_root) {
        return Err("仅允许删除 SSMT4 下载并管理的 Proton/Wine 版本".to_string());
    }

    let display_name = canonical
        .file_name()
        .map(|v| v.to_string_lossy().to_string())
        .unwrap_or_else(|| canonical.display().to_string());

    if canonical.is_dir() {
        std::fs::remove_dir_all(&canonical).map_err(|e| format!("删除目录失败: {}", e))?;
    } else {
        std::fs::remove_file(&canonical).map_err(|e| format!("删除文件失败: {}", e))?;
    }

    Ok(format!("{} 已删除", display_name))
}

/// 下载并安装 Wine/Proton 版本
/// variant 为 "Wine-GE" / "Wine-Builds" 时安装到 <dataDir>/runners/wine/
/// variant 为 "GE-Proton" / "DW-Proton" 时安装到 <dataDir>/proton/
pub async fn download_and_install_proton(
    download_url: &str,
    tag: &str,
    variant: &str,
    app: Option<tauri::AppHandle>,
) -> Result<String, String> {
    let is_wine = variant.starts_with("Wine");
    let install_dir = if is_wine {
        get_wine_runners_dir()
    } else {
        get_proton_install_dir()
    };
    std::fs::create_dir_all(&install_dir).map_err(|e| format!("创建目录失败: {}", e))?;

    let kind = if is_wine { "Wine" } else { "Proton" };

    let mirrors = [
        download_url.to_string(),
        download_url.replace("github.com", "ghp.ci"),
        download_url.replace("github.com", "gh-proxy.com/github.com"),
    ];

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let mut last_err = String::new();
    let mut resp_ok = None;

    for url in &mirrors {
        info!("下载 {} {} 从 {}", kind, tag, url);
        match client
            .get(url)
            .header("User-Agent", "SSMT4/0.1")
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => {
                resp_ok = Some(r);
                break;
            }
            Ok(r) => {
                last_err = format!("HTTP {}: {}", r.status(), url);
                tracing::warn!("镜像 {} 返回 HTTP {}，尝试下一个", url, r.status());
            }
            Err(e) => {
                last_err = format!("{}: {}", url, e);
                tracing::warn!("镜像 {} 连接失败: {}，尝试下一个", url, e);
            }
        }
    }

    let resp = resp_ok.ok_or_else(|| format!("所有镜像均下载失败，最后错误: {}", last_err))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}: {}", resp.status(), download_url));
    }

    let total_size = resp.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;
    let mut stream = resp.bytes_stream();
    let component_id = format!("proton:{}:{}", if is_wine { "wine" } else { "proton" }, tag);
    let component_name = format!("{} {}", kind, tag);

    let emit_progress = |phase: &str, current: u64, total: u64| {
        if let Some(ref a) = app {
            let phase = match phase {
                "extracting" => ComponentDownloadPhase::Extracting,
                "done" => ComponentDownloadPhase::Done,
                _ => ComponentDownloadPhase::Downloading,
            };
            emit_component_download_progress(
                a,
                &ComponentDownloadProgressEvent {
                    component_id: component_id.clone(),
                    component_name: Some(component_name.clone()),
                    phase,
                    downloaded: current,
                    total,
                },
            );
        }
    };

    let url_path = download_url
        .split('?')
        .next()
        .unwrap_or(download_url)
        .to_lowercase();
    let ext = if url_path.ends_with(".tar.xz") {
        "tar.xz"
    } else if url_path.ends_with(".zip") {
        "zip"
    } else {
        "tar.gz"
    };
    let tmp_file = install_dir.join(format!("{}.{}", tag, ext));
    std::fs::create_dir_all(&install_dir).map_err(|e| format!("创建目录失败: {}", e))?;

    let mut file = tokio::fs::File::create(&tmp_file)
        .await
        .map_err(|e| format!("创建临时文件失败: {}", e))?;
    use tokio::io::AsyncWriteExt;

    let mut header_buf = [0u8; 6];
    let mut header_filled: usize = 0;

    emit_progress("downloading", 0, total_size);
    let mut last_emit = std::time::Instant::now();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("下载流读取失败: {}", e))?;
        if header_filled < 6 {
            let need = (6 - header_filled).min(chunk.len());
            header_buf[header_filled..header_filled + need].copy_from_slice(&chunk[..need]);
            header_filled += need;
        }
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("写入临时文件失败: {}", e))?;
        downloaded += chunk.len() as u64;
        if last_emit.elapsed() >= std::time::Duration::from_millis(200) {
            emit_progress("downloading", downloaded, total_size);
            last_emit = std::time::Instant::now();
        }
    }
    emit_progress("downloading", downloaded, total_size);
    file.flush()
        .await
        .map_err(|e| format!("刷新临时文件失败: {}", e))?;
    drop(file);

    const MIN_ARCHIVE_SIZE: u64 = 1_000_000;
    if downloaded < MIN_ARCHIVE_SIZE {
        tokio::fs::remove_file(&tmp_file).await.ok();
        return Err(format!(
            "{} {} 下载异常：文件大小 {} 字节，低于最小阈值 {} 字节，疑似截断或损坏",
            kind, tag, downloaded, MIN_ARCHIVE_SIZE
        ));
    }

    let valid_header = if ext == "tar.xz" {
        header_filled >= 6 && header_buf[..6] == [0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00]
    } else if ext == "zip" {
        header_filled >= 2 && header_buf[..2] == [0x50, 0x4B]
    } else {
        header_filled >= 2 && header_buf[..2] == [0x1F, 0x8B]
    };
    if !valid_header {
        tokio::fs::remove_file(&tmp_file).await.ok();
        return Err(format!(
            "{} {} 下载的文件不是有效的归档格式（魔数不匹配），疑似损坏或被篡改",
            kind, tag
        ));
    }
    info!("{} {} 下载完整性校验通过（{} 字节）", kind, tag, downloaded);

    emit_progress("extracting", 0, 0);
    info!("解压 {} 到 {}", tmp_file.display(), install_dir.display());
    let status = if ext == "zip" {
        tokio::process::Command::new("unzip")
            .arg("-o")
            .arg(&tmp_file)
            .arg("-d")
            .arg(&install_dir)
            .status()
            .await
            .map_err(|e| format!("解压 zip 失败: {}。请确保已安装 unzip。", e))?
    } else {
        let tar_flag = if ext == "tar.xz" { "-xf" } else { "-xzf" };
        tokio::process::Command::new("tar")
            .arg(tar_flag)
            .arg(&tmp_file)
            .arg("-C")
            .arg(&install_dir)
            .status()
            .await
            .map_err(|e| format!("解压失败: {}", e))?
    };

    if !status.success() {
        return Err(format!("解压 {} 失败", tmp_file.display()));
    }

    tokio::fs::remove_file(&tmp_file).await.ok();

    emit_progress("done", total_size, total_size);
    info!("{} {} 安装完成 → {}", kind, tag, install_dir.display());
    Ok(format!("{} {} 安装完成", kind, tag))
}
