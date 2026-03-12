use super::*;
use crate::events::{
    emit_component_download_progress, ComponentDownloadPhase, ComponentDownloadProgressEvent,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

fn vkd3d_names(version: &str) -> (String, String) {
    let archive_name = format!("vkd3d-proton-{}.tar.zst", version);
    let extract_dir_name = archive_name
        .strip_suffix(".tar.zst")
        .unwrap_or(&archive_name)
        .to_string();
    (archive_name, extract_dir_name)
}

fn vkd3d_download_url(version: &str, archive_name: &str) -> String {
    format!(
        "https://github.com/HansKristian-Work/vkd3d-proton/releases/download/v{}/{}",
        version, archive_name
    )
}

fn dxvk_variant_display_name(variant: &str) -> String {
    if let Some(source) = find_dxvk_variant(variant) {
        if !source.display_name.trim().is_empty() {
            return source.display_name;
        }
        if !source.id.trim().is_empty() {
            return source.id;
        }
    }
    if variant.eq_ignore_ascii_case("gplasync") {
        return "DXVK-GPLAsync".to_string();
    }
    "DXVK".to_string()
}

fn dxvk_names(version: &str, variant: &str) -> (String, String) {
    let archive_name = find_dxvk_variant(variant)
        .map(|source| {
            let template = source.archive_name_template.trim();
            if template.is_empty() {
                String::new()
            } else {
                render_source_template(template, version, "", 0)
            }
        })
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| match variant {
            "gplasync" => format!("dxvk-gplasync-v{}.tar.gz", version),
            _ => format!("dxvk-{}.tar.gz", version),
        });

    let extract_dir_name = archive_name
        .strip_suffix(".tar.gz")
        .or_else(|| archive_name.strip_suffix(".tar.xz"))
        .or_else(|| archive_name.strip_suffix(".zip"))
        .unwrap_or(&archive_name)
        .to_string();

    (archive_name, extract_dir_name)
}

fn dxvk_download_url(version: &str, variant: &str, archive_name: &str) -> String {
    if let Some(source) = find_dxvk_variant(variant) {
        let template = source.download_url_template.trim();
        if !template.is_empty() {
            return render_source_template(template, version, archive_name, 0);
        }

        if source.provider.eq_ignore_ascii_case("github_releases") && !source.repo.trim().is_empty()
        {
            return format!(
                "https://github.com/{}/releases/download/v{}/{}",
                source.repo.trim(),
                version,
                archive_name
            );
        }
    }

    match variant {
        "gplasync" => {
            format!(
                "https://gitlab.com/Ph42oN/dxvk-gplasync/-/raw/main/releases/{}",
                archive_name
            )
        }
        _ => {
            format!(
                "https://github.com/doitsujin/dxvk/releases/download/v{}/{}",
                version, archive_name
            )
        }
    }
}

pub async fn download_vkd3d_only(
    vkd3d_version: &str,
    app: Option<tauri::AppHandle>,
) -> Result<String, String> {
    let cache_dir = crate::utils::file_manager::get_tools_dir().join("vkd3d");
    crate::utils::file_manager::ensure_dir(&cache_dir)?;

    let (archive_name, extract_dir_name) = vkd3d_names(vkd3d_version);
    let archive_path = cache_dir.join(&archive_name);
    let extract_dir = cache_dir.join(&extract_dir_name);
    let component_id = format!("vkd3d:{}", vkd3d_version);
    let component_name = format!("VKD3D-Proton {}", vkd3d_version);

    if !archive_path.exists() {
        let url = vkd3d_download_url(vkd3d_version, &archive_name);
        info!("Downloading VKD3D-Proton {} from {}", vkd3d_version, url);
        download_tool(
            &url,
            &archive_path,
            app.as_ref(),
            Some(&component_id),
            Some(&component_name),
        )
        .await?;
    }

    if !extract_dir.exists() {
        if let Some(app) = app.as_ref() {
            emit_component_download_progress(
                app,
                &ComponentDownloadProgressEvent {
                    component_id: component_id.clone(),
                    component_name: Some(component_name.clone()),
                    phase: ComponentDownloadPhase::Extracting,
                    downloaded: 0,
                    total: 0,
                },
            );
        }
        extract_tar_zst(&archive_path, &cache_dir).await?;
    }

    info!(
        "VKD3D-Proton {} 已缓存到 {}",
        vkd3d_version,
        extract_dir.display()
    );
    Ok(format!("VKD3D-Proton {} 下载完成", vkd3d_version))
}

pub async fn download_dxvk_only(
    dxvk_version: &str,
    variant: &str,
    app: Option<tauri::AppHandle>,
) -> Result<String, String> {
    let cache_dir = crate::utils::file_manager::get_tools_dir().join("dxvk");
    crate::utils::file_manager::ensure_dir(&cache_dir)?;

    let (archive_name, extract_dir_name) = dxvk_names(dxvk_version, variant);
    let archive_path = cache_dir.join(&archive_name);
    let extract_dir = cache_dir.join(&extract_dir_name);
    let component_id = format!("dxvk:{}:{}", variant, dxvk_version);
    let component_name = format!("{} {}", dxvk_variant_display_name(variant), dxvk_version);

    if !archive_path.exists() {
        let url = dxvk_download_url(dxvk_version, variant, &archive_name);
        info!(
            "Downloading DXVK {} ({}) from {}",
            dxvk_version, variant, url
        );
        download_tool(
            &url,
            &archive_path,
            app.as_ref(),
            Some(&component_id),
            Some(&component_name),
        )
        .await?;
    }

    if !extract_dir.exists() {
        if let Some(app) = app.as_ref() {
            emit_component_download_progress(
                app,
                &ComponentDownloadProgressEvent {
                    component_id: component_id.clone(),
                    component_name: Some(component_name.clone()),
                    phase: ComponentDownloadPhase::Extracting,
                    downloaded: 0,
                    total: 0,
                },
            );
        }
        extract_tar_gz(&archive_path, &cache_dir)?;
    }

    let label = dxvk_variant_display_name(variant);
    info!(
        "{} {} 已缓存到 {}",
        label,
        dxvk_version,
        extract_dir.display()
    );
    Ok(format!("{} {} 下载完成", label, dxvk_version))
}

pub async fn install_dxvk(
    prefix_path: &Path,
    dxvk_version: &str,
    variant: &str,
) -> Result<String, String> {
    let cache_dir = crate::utils::file_manager::get_tools_dir().join("dxvk");
    crate::utils::file_manager::ensure_dir(&cache_dir)?;

    let (archive_name, extract_dir_name) = dxvk_names(dxvk_version, variant);
    let archive_path = cache_dir.join(&archive_name);
    let extract_dir = cache_dir.join(&extract_dir_name);

    if !archive_path.exists() {
        let url = dxvk_download_url(dxvk_version, variant, &archive_name);
        info!(
            "Downloading DXVK {} ({}) from {}",
            dxvk_version, variant, url
        );
        download_tool(&url, &archive_path, None, None, None).await?;
    }

    if !extract_dir.exists() {
        extract_tar_gz(&archive_path, &cache_dir)?;
    }

    let system32 = prefix_path.join("drive_c").join("windows").join("system32");
    let syswow64 = prefix_path.join("drive_c").join("windows").join("syswow64");

    let x64_dir = extract_dir.join("x64");
    let x32_dir = extract_dir.join("x32");

    if !x64_dir.exists() {
        return Err(format!("DXVK 解压目录缺少 x64/: {}", extract_dir.display()));
    }

    std::fs::create_dir_all(&system32).map_err(|e| format!("创建 system32 目录失败: {}", e))?;

    let dlls = ["d3d9.dll", "d3d10core.dll", "d3d11.dll", "dxgi.dll"];
    let mut copied = 0usize;

    for dll in &dlls {
        let src = x64_dir.join(dll);
        if src.exists() {
            std::fs::copy(&src, system32.join(dll))
                .map_err(|e| format!("Failed to copy DXVK DLL {}: {}", dll, e))?;
            copied += 1;
        }
    }

    if x32_dir.exists() {
        if !syswow64.exists() {
            std::fs::create_dir_all(&syswow64).ok();
        }
        if syswow64.exists() {
            for dll in &dlls {
                let src = x32_dir.join(dll);
                if src.exists() {
                    std::fs::copy(&src, syswow64.join(dll))
                        .map_err(|e| format!("Failed to copy DXVK 32-bit DLL {}: {}", dll, e))?;
                }
            }
        }
    }

    if copied == 0 {
        let label = dxvk_variant_display_name(variant);
        return Err(format!(
            "{} {} 安装失败：x64 目录中未找到任何 DLL",
            label, dxvk_version
        ));
    }

    write_dxvk_version_marker(prefix_path, dxvk_version);
    let label = dxvk_variant_display_name(variant);

    info!(
        "Installed {} {} to {} ({} DLLs copied)",
        label,
        dxvk_version,
        prefix_path.display(),
        copied
    );
    Ok(format!(
        "{} {} 安装完成（{} 个 DLL）",
        label, dxvk_version, copied
    ))
}

pub fn uninstall_dxvk(prefix_path: &Path) -> Result<String, String> {
    let system32 = prefix_path.join("drive_c").join("windows").join("system32");
    let syswow64 = prefix_path.join("drive_c").join("windows").join("syswow64");
    let dlls = ["d3d9.dll", "d3d10core.dll", "d3d11.dll", "dxgi.dll"];

    for dll in &dlls {
        let path = system32.join(dll);
        if path.exists() {
            std::fs::remove_file(&path).ok();
        }
        let path = syswow64.join(dll);
        if path.exists() {
            std::fs::remove_file(&path).ok();
        }
    }

    remove_dxvk_version_marker(prefix_path);

    info!("Uninstalled DXVK from {}", prefix_path.display());
    Ok("DXVK uninstalled".to_string())
}

pub async fn install_vkd3d(prefix_path: &Path, vkd3d_version: &str) -> Result<String, String> {
    let cache_dir = crate::utils::file_manager::get_tools_dir().join("vkd3d");
    crate::utils::file_manager::ensure_dir(&cache_dir)?;

    let (archive_name, extract_dir_name) = vkd3d_names(vkd3d_version);
    let archive_path = cache_dir.join(&archive_name);
    let extract_dir = cache_dir.join(&extract_dir_name);

    if !archive_path.exists() {
        let url = vkd3d_download_url(vkd3d_version, &archive_name);
        info!("Downloading VKD3D-Proton {} from {}", vkd3d_version, url);
        download_tool(&url, &archive_path, None, None, None).await?;
    }

    if !extract_dir.exists() {
        extract_tar_zst(&archive_path, &cache_dir).await?;
    }

    let system32 = prefix_path.join("drive_c").join("windows").join("system32");
    let syswow64 = prefix_path.join("drive_c").join("windows").join("syswow64");
    std::fs::create_dir_all(&system32)
        .map_err(|e| format!("Failed to create system32 directory: {}", e))?;
    std::fs::create_dir_all(&syswow64)
        .map_err(|e| format!("Failed to create syswow64 directory: {}", e))?;

    let search_root = if extract_dir.exists() {
        extract_dir.clone()
    } else {
        cache_dir.clone()
    };

    let x64_dir = find_arch_dir(&search_root, &["x64"]).ok_or_else(|| {
        format!(
            "VKD3D archive missing x64 directory under {}",
            search_root.display()
        )
    })?;
    let x86_dir = find_arch_dir(&search_root, &["x86", "x32", "i386"]);

    let mut copied = 0usize;
    copied += copy_vkd3d_dlls(&x64_dir, &system32)?;
    if let Some(x86_dir) = x86_dir {
        copied += copy_vkd3d_dlls(&x86_dir, &syswow64)?;
    } else {
        warn!("VKD3D archive has no x86/x32 directory, only 64-bit DLLs were installed");
    }

    if copied == 0 {
        return Err(format!(
            "VKD3D archive extracted but no target DLLs found in {}",
            extract_dir.display()
        ));
    }

    write_vkd3d_version_marker(prefix_path, vkd3d_version);

    info!(
        "Installed VKD3D-Proton {} to {} ({} DLLs copied)",
        vkd3d_version,
        prefix_path.display(),
        copied
    );
    Ok(format!(
        "VKD3D-Proton {} 安装完成（{} 个 DLL）",
        vkd3d_version, copied
    ))
}

pub fn uninstall_vkd3d(prefix_path: &Path) -> Result<String, String> {
    let system32 = prefix_path.join("drive_c").join("windows").join("system32");
    let syswow64 = prefix_path.join("drive_c").join("windows").join("syswow64");
    let dlls = ["d3d12.dll", "d3d12core.dll", "dxil.dll"];

    for dll in &dlls {
        let path = system32.join(dll);
        if path.exists() {
            std::fs::remove_file(&path).ok();
        }
        let path = syswow64.join(dll);
        if path.exists() {
            std::fs::remove_file(&path).ok();
        }
    }

    remove_vkd3d_version_marker(prefix_path);
    info!("Uninstalled VKD3D-Proton from {}", prefix_path.display());
    Ok("VKD3D-Proton 已卸载".to_string())
}

pub fn delete_local_dxvk_version(dxvk_version: &str, variant: &str) -> Result<String, String> {
    let cache_dir = crate::utils::file_manager::get_tools_dir().join("dxvk");
    let (archive_name, extract_dir_name) = dxvk_names(dxvk_version, variant);
    let archive_path = cache_dir.join(&archive_name);
    let extract_dir = cache_dir.join(&extract_dir_name);

    let mut removed = 0usize;
    if archive_path.exists() {
        std::fs::remove_file(&archive_path).map_err(|e| format!("删除 DXVK 压缩包失败: {}", e))?;
        removed += 1;
    }
    if extract_dir.exists() {
        std::fs::remove_dir_all(&extract_dir).map_err(|e| format!("删除 DXVK 目录失败: {}", e))?;
        removed += 1;
    }

    if removed == 0 {
        return Err(format!(
            "未找到可删除的 DXVK 缓存：{} {}",
            dxvk_variant_display_name(variant),
            dxvk_version
        ));
    }

    Ok(format!(
        "{} {} 已删除",
        dxvk_variant_display_name(variant),
        dxvk_version
    ))
}

pub fn delete_local_vkd3d_version(vkd3d_version: &str) -> Result<String, String> {
    let cache_dir = crate::utils::file_manager::get_tools_dir().join("vkd3d");
    let (archive_name, extract_dir_name) = vkd3d_names(vkd3d_version);
    let archive_path = cache_dir.join(&archive_name);
    let extract_dir = cache_dir.join(&extract_dir_name);

    let mut removed = 0usize;
    if archive_path.exists() {
        std::fs::remove_file(&archive_path).map_err(|e| format!("删除 VKD3D 压缩包失败: {}", e))?;
        removed += 1;
    }
    if extract_dir.exists() {
        std::fs::remove_dir_all(&extract_dir).map_err(|e| format!("删除 VKD3D 目录失败: {}", e))?;
        removed += 1;
    }

    if removed == 0 {
        return Err(format!("未找到可删除的 VKD3D 缓存：{}", vkd3d_version));
    }

    Ok(format!("VKD3D-Proton {} 已删除", vkd3d_version))
}

async fn download_tool(
    url: &str,
    dest: &Path,
    app: Option<&tauri::AppHandle>,
    component_id: Option<&str>,
    component: Option<&str>,
) -> Result<(), String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .header("User-Agent", "SSMT4/0.1")
        .send()
        .await
        .map_err(|e| format!("Download failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}: {}", resp.status(), url));
    }

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let normalized_component_id = component_id
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.to_string())
        .or_else(|| {
            component
                .filter(|value| !value.trim().is_empty())
                .map(|value| value.to_string())
        });
    let normalized_component_name = component
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.to_string());

    let emit_progress = |phase: ComponentDownloadPhase, downloaded: u64, total: u64| {
        let Some(app) = app else {
            return;
        };
        let Some(component_id) = normalized_component_id.as_ref() else {
            return;
        };
        emit_component_download_progress(
            app,
            &ComponentDownloadProgressEvent {
                component_id: component_id.clone(),
                component_name: normalized_component_name.clone(),
                phase,
                downloaded,
                total,
            },
        );
    };

    let mut downloaded = 0u64;
    let mut header_buf = [0u8; 6];
    let mut header_filled = 0usize;
    let total = resp.content_length().unwrap_or(0);

    emit_progress(ComponentDownloadPhase::Downloading, 0, total);

    {
        use futures_util::StreamExt;
        use tokio::io::AsyncWriteExt;

        let mut stream = resp.bytes_stream();
        let mut file = tokio::fs::File::create(dest)
            .await
            .map_err(|e| format!("Failed to create file {}: {}", dest.display(), e))?;
        let mut last_emit = std::time::Instant::now();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("Failed to read download stream: {}", e))?;
            if header_filled < 6 {
                let need = (6 - header_filled).min(chunk.len());
                header_buf[header_filled..header_filled + need].copy_from_slice(&chunk[..need]);
                header_filled += need;
            }
            file.write_all(&chunk)
                .await
                .map_err(|e| format!("Failed to write chunk: {}", e))?;
            downloaded += chunk.len() as u64;
            if last_emit.elapsed() >= std::time::Duration::from_millis(200) {
                emit_progress(ComponentDownloadPhase::Downloading, downloaded, total);
                last_emit = std::time::Instant::now();
            }
        }
        file.flush()
            .await
            .map_err(|e| format!("Failed to flush file: {}", e))?;
    }

    emit_progress(ComponentDownloadPhase::Downloading, downloaded, total);

    const MIN_TOOL_SIZE: u64 = 10_000;
    if downloaded < MIN_TOOL_SIZE {
        tokio::fs::remove_file(dest).await.ok();
        return Err(format!(
            "下载的文件异常（大小 {} 字节，低于 {} 字节），疑似截断或错误页面: {}",
            downloaded, MIN_TOOL_SIZE, url
        ));
    }

    let valid_archive = (header_filled >= 2 && header_buf[..2] == [0x1F, 0x8B])
        || (header_filled >= 6 && header_buf[..6] == [0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00])
        || (header_filled >= 4 && header_buf[..4] == [0x28, 0xB5, 0x2F, 0xFD])
        || (header_filled >= 4 && header_buf[..4] == [0x50, 0x4B, 0x03, 0x04]);
    if !valid_archive {
        tokio::fs::remove_file(dest).await.ok();
        return Err(format!(
            "下载的文件不是有效归档格式（魔数不匹配），疑似损坏或被篡改: {}",
            url
        ));
    }

    emit_progress(ComponentDownloadPhase::Extracting, downloaded, total);
    Ok(())
}

fn copy_vkd3d_dlls(src_dir: &Path, dst_dir: &Path) -> Result<usize, String> {
    let mut available: HashMap<String, PathBuf> = HashMap::new();
    for entry in std::fs::read_dir(src_dir).map_err(|e| {
        format!(
            "Failed to read VKD3D directory {}: {}",
            src_dir.display(),
            e
        )
    })? {
        let entry = entry.map_err(|e| format!("Failed to read VKD3D directory entry: {}", e))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
            available.insert(name.to_ascii_lowercase(), path);
        }
    }

    let dlls = ["d3d12.dll", "d3d12core.dll", "dxil.dll"];
    let mut copied = 0usize;
    for dll in &dlls {
        if let Some(src) = available.get(*dll) {
            std::fs::copy(src, dst_dir.join(dll))
                .map_err(|e| format!("Failed to copy VKD3D DLL {}: {}", dll, e))?;
            copied += 1;
        }
    }

    Ok(copied)
}

fn extract_tar_gz(archive: &Path, dest: &Path) -> Result<(), String> {
    let file =
        std::fs::File::open(archive).map_err(|e| format!("Failed to open archive: {}", e))?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(gz);
    archive
        .unpack(dest)
        .map_err(|e| format!("Failed to extract archive: {}", e))
}

async fn extract_tar_zst(archive: &Path, dest: &Path) -> Result<(), String> {
    let try_builtin = tokio::process::Command::new("tar")
        .arg("--zstd")
        .arg("-xf")
        .arg(archive)
        .arg("-C")
        .arg(dest)
        .status()
        .await;

    match try_builtin {
        Ok(status) if status.success() => return Ok(()),
        _ => {}
    }

    let try_filter = tokio::process::Command::new("tar")
        .arg("-I")
        .arg("zstd")
        .arg("-xf")
        .arg(archive)
        .arg("-C")
        .arg(dest)
        .status()
        .await
        .map_err(|e| format!("Failed to run tar for zstd extraction: {}", e))?;

    if try_filter.success() {
        Ok(())
    } else {
        Err(format!(
            "Failed to extract zstd archive {} with tar",
            archive.display()
        ))
    }
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
    fn vkd3d_naming_and_url_are_stable() {
        let (archive, dir) = vkd3d_names("2.8");
        assert_eq!(archive, "vkd3d-proton-2.8.tar.zst");
        assert_eq!(dir, "vkd3d-proton-2.8");
        assert_eq!(
            vkd3d_download_url("2.8", &archive),
            "https://github.com/HansKristian-Work/vkd3d-proton/releases/download/v2.8/vkd3d-proton-2.8.tar.zst"
        );
    }

    #[test]
    fn dxvk_names_and_url_fallback_for_unknown_variant() {
        let (archive, dir) = dxvk_names("2.4", "unknown");
        assert_eq!(archive, "dxvk-2.4.tar.gz");
        assert_eq!(dir, "dxvk-2.4");
        assert_eq!(
            dxvk_download_url("2.4", "unknown", &archive),
            "https://github.com/doitsujin/dxvk/releases/download/v2.4/dxvk-2.4.tar.gz"
        );
        assert_eq!(dxvk_variant_display_name("unknown"), "DXVK");
    }

    #[test]
    fn dxvk_gplasync_fallback_names_and_url_remain_compatible() {
        let (archive, dir) = dxvk_names("2.4", "gplasync");
        assert!(archive.starts_with("dxvk-gplasync-"));
        assert_eq!(dir, archive.trim_end_matches(".tar.gz"));
        assert!(
            dxvk_download_url("2.4", "gplasync", &archive).contains(&archive),
            "download url should embed archive name"
        );
        assert_eq!(dxvk_variant_display_name("gplasync"), "DXVK-GPLAsync");
    }

    #[test]
    fn uninstall_dxvk_and_vkd3d_remove_dlls_and_markers() {
        let prefix = unique_temp_dir("ssmt4-graphics-uninstall");
        let system32 = prefix.join("drive_c").join("windows").join("system32");
        let syswow64 = prefix.join("drive_c").join("windows").join("syswow64");
        std::fs::create_dir_all(&system32).unwrap();
        std::fs::create_dir_all(&syswow64).unwrap();

        for dll in ["d3d9.dll", "d3d10core.dll", "d3d11.dll", "dxgi.dll"] {
            std::fs::write(system32.join(dll), b"dxvk").unwrap();
            std::fs::write(syswow64.join(dll), b"dxvk32").unwrap();
        }
        for dll in ["d3d12.dll", "d3d12core.dll", "dxil.dll"] {
            std::fs::write(system32.join(dll), b"vkd3d").unwrap();
            std::fs::write(syswow64.join(dll), b"vkd3d32").unwrap();
        }

        write_dxvk_version_marker(&prefix, "2.4");
        write_vkd3d_version_marker(&prefix, "2.8");

        uninstall_dxvk(&prefix).unwrap();
        uninstall_vkd3d(&prefix).unwrap();

        for dll in ["d3d9.dll", "d3d10core.dll", "d3d11.dll", "dxgi.dll"] {
            assert!(!system32.join(dll).exists());
            assert!(!syswow64.join(dll).exists());
        }
        for dll in ["d3d12.dll", "d3d12core.dll", "dxil.dll"] {
            assert!(!system32.join(dll).exists());
            assert!(!syswow64.join(dll).exists());
        }
        assert_eq!(read_dxvk_version_marker(&prefix), None);
        assert_eq!(read_vkd3d_version_marker(&prefix), None);

        let _ = std::fs::remove_dir_all(prefix);
    }

    #[test]
    fn copy_vkd3d_dlls_is_case_insensitive_and_counts_copied_files() {
        let root = unique_temp_dir("ssmt4-vkd3d-copy");
        let src = root.join("src");
        let dst = root.join("dst");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dst).unwrap();

        std::fs::write(src.join("D3D12.DLL"), b"a").unwrap();
        std::fs::write(src.join("DxIl.DlL"), b"b").unwrap();
        std::fs::write(src.join("readme.txt"), b"ignore").unwrap();

        let copied = copy_vkd3d_dlls(&src, &dst).unwrap();

        assert_eq!(copied, 2);
        assert!(dst.join("d3d12.dll").exists());
        assert!(dst.join("dxil.dll").exists());
        assert!(!dst.join("d3d12core.dll").exists());

        let _ = std::fs::remove_dir_all(root);
    }
}
