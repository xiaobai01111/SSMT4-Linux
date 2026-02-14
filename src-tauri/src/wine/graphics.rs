use std::path::{Path, PathBuf};
use tracing::{info, warn};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VulkanInfo {
    pub available: bool,
    pub version: Option<String>,
    pub driver: Option<String>,
    pub device_name: Option<String>,
}

// ============================================================
// DXVK 版本管理
// ============================================================

/// 本地已缓存的 DXVK 版本信息
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DxvkLocalVersion {
    pub version: String,
    /// 是否已解压（可直接安装）
    pub extracted: bool,
    /// 缓存目录路径
    pub path: PathBuf,
}

/// 远程可用的 DXVK 版本（GitHub Release）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DxvkRemoteVersion {
    pub version: String,
    pub tag_name: String,
    pub download_url: String,
    pub file_size: u64,
    pub published_at: String,
    /// 是否已在本地缓存
    pub is_local: bool,
}

/// 当前 Prefix 中安装的 DXVK 状态
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DxvkInstalledStatus {
    /// 是否检测到 DXVK DLL
    pub installed: bool,
    /// 匹配到的版本号（通过文件大小比对）
    pub version: Option<String>,
    /// 检测到的 DXVK DLL 列表
    pub dlls_found: Vec<String>,
}

/// 扫描本地缓存的 DXVK 版本（tools/dxvk/ 目录）
pub fn scan_local_dxvk_versions() -> Vec<DxvkLocalVersion> {
    let cache_dir = crate::utils::file_manager::get_tools_dir().join("dxvk");
    if !cache_dir.exists() {
        return Vec::new();
    }

    let mut versions = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            // 目录名格式: dxvk-{version}
            if let Some(ver) = name.strip_prefix("dxvk-") {
                let has_x64 = path.join("x64").exists();
                let has_x32 = path.join("x32").exists();
                versions.push(DxvkLocalVersion {
                    version: ver.to_string(),
                    extracted: has_x64 || has_x32,
                    path,
                });
            }
        }
    }

    // 同时检查 tar.gz 压缩包（已下载未解压）
    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            if name.starts_with("dxvk-") && name.ends_with(".tar.gz") {
                let ver = name
                    .strip_prefix("dxvk-")
                    .and_then(|s| s.strip_suffix(".tar.gz"))
                    .unwrap_or("");
                if !ver.is_empty() && !versions.iter().any(|v| v.version == ver) {
                    versions.push(DxvkLocalVersion {
                        version: ver.to_string(),
                        extracted: false,
                        path,
                    });
                }
            }
        }
    }

    versions.sort_by(|a, b| b.version.cmp(&a.version));
    versions
}

/// 检测 Prefix 中已安装的 DXVK 版本
pub fn detect_installed_dxvk(prefix_path: &Path) -> DxvkInstalledStatus {
    let system32 = prefix_path.join("drive_c").join("windows").join("system32");
    let dxvk_dlls = ["d3d9.dll", "d3d10core.dll", "d3d11.dll", "dxgi.dll"];
    let mut found_dlls = Vec::new();

    for dll in &dxvk_dlls {
        let dll_path = system32.join(dll);
        if dll_path.exists() {
            if let Ok(meta) = std::fs::metadata(&dll_path) {
                // DXVK DLL 通常 > 200KB，Wine 原版 < 100KB
                if meta.len() > 200_000 {
                    found_dlls.push(dll.to_string());
                }
            }
        }
    }

    let installed = !found_dlls.is_empty();

    // 多级版本检测：标记文件 → strings 提取 → 文件大小比对
    let version = if installed {
        read_dxvk_version_marker(prefix_path)
            .or_else(|| extract_dxvk_version_from_dll(&system32))
            .or_else(|| match_dxvk_version_by_size(&system32))
    } else {
        None
    };

    DxvkInstalledStatus {
        installed,
        version,
        dlls_found: found_dlls,
    }
}

/// 获取 DXVK 版本标记文件路径
fn dxvk_version_marker_path(prefix_path: &Path) -> PathBuf {
    prefix_path.join(".dxvk-version")
}

/// 读取安装时写入的 .dxvk-version 标记文件
fn read_dxvk_version_marker(prefix_path: &Path) -> Option<String> {
    let marker = dxvk_version_marker_path(prefix_path);
    std::fs::read_to_string(&marker)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// 写入 .dxvk-version 标记文件
fn write_dxvk_version_marker(prefix_path: &Path, version: &str) {
    let marker = dxvk_version_marker_path(prefix_path);
    let _ = std::fs::write(&marker, version);
}

/// 删除 .dxvk-version 标记文件
fn remove_dxvk_version_marker(prefix_path: &Path) {
    let marker = dxvk_version_marker_path(prefix_path);
    let _ = std::fs::remove_file(&marker);
}

/// 用 strings 命令从 DLL 中提取 DXVK 版本号（如 "dxvk-2.5.3"）
fn extract_dxvk_version_from_dll(system32: &Path) -> Option<String> {
    let dll_path = system32.join("dxgi.dll");
    if !dll_path.exists() {
        return None;
    }

    let output = std::process::Command::new("strings")
        .arg(&dll_path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // DXVK DLL 包含 "dxvk-X.Y.Z" 格式的版本字符串
    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(ver) = trimmed.strip_prefix("dxvk-") {
            // 验证格式：数字.数字[.数字...]
            if ver.chars().next().is_some_and(|c| c.is_ascii_digit())
                && ver.contains('.')
                && ver.len() <= 20
            {
                return Some(ver.to_string());
            }
        }
    }
    None
}

/// 通过 DLL 文件大小与本地缓存版本比对（兜底方案）
fn match_dxvk_version_by_size(system32: &Path) -> Option<String> {
    let local_versions = scan_local_dxvk_versions();
    let reference_dll = "dxgi.dll";

    let installed_size = std::fs::metadata(system32.join(reference_dll))
        .ok()
        .map(|m| m.len())?;

    for local in &local_versions {
        if !local.extracted {
            continue;
        }
        let cached_dll = local.path.join("x64").join(reference_dll);
        if let Ok(meta) = std::fs::metadata(&cached_dll) {
            if meta.len() == installed_size {
                return Some(local.version.clone());
            }
        }
    }
    None
}

/// 从 GitHub API 获取 DXVK 可用版本列表
pub async fn fetch_dxvk_releases(max_count: usize) -> Result<Vec<DxvkRemoteVersion>, String> {
    let url = format!(
        "https://api.github.com/repos/doitsujin/dxvk/releases?per_page={}",
        max_count
    );

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("User-Agent", "SSMT4/0.1")
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .map_err(|e| format!("请求 GitHub API 失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub API 返回 HTTP {}", resp.status()));
    }

    let releases: Vec<serde_json::Value> = resp
        .json()
        .await
        .map_err(|e| format!("解析 GitHub API 响应失败: {}", e))?;

    let local_versions = scan_local_dxvk_versions();
    let local_ver_set: std::collections::HashSet<String> =
        local_versions.iter().map(|v| v.version.clone()).collect();

    let mut result = Vec::new();
    for release in &releases {
        let tag_name = release
            .get("tag_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let version = tag_name.strip_prefix('v').unwrap_or(&tag_name).to_string();
        let published_at = release
            .get("published_at")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // 查找 tar.gz asset
        let assets = release.get("assets").and_then(|v| v.as_array());
        if let Some(assets) = assets {
            for asset in assets {
                let name = asset
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if name.ends_with(".tar.gz") && name.contains("dxvk") {
                    let download_url = asset
                        .get("browser_download_url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let file_size = asset
                        .get("size")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);

                    result.push(DxvkRemoteVersion {
                        version: version.clone(),
                        tag_name: tag_name.clone(),
                        download_url,
                        file_size,
                        published_at: published_at.clone(),
                        is_local: local_ver_set.contains(&version),
                    });
                    break;
                }
            }
        }
    }

    info!("获取到 {} 个 DXVK 远程版本", result.len());
    Ok(result)
}

pub fn check_vulkan() -> VulkanInfo {
    let output = std::process::Command::new("vulkaninfo")
        .arg("--summary")
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let version = extract_field(&stdout, "Vulkan Instance Version:");
            let driver = extract_field(&stdout, "driverName");
            let device = extract_field(&stdout, "deviceName");

            info!(
                "Vulkan available: version={:?}, driver={:?}",
                version, driver
            );
            VulkanInfo {
                available: true,
                version,
                driver,
                device_name: device,
            }
        }
        _ => {
            warn!("Vulkan not available or vulkaninfo not found");
            VulkanInfo {
                available: false,
                version: None,
                driver: None,
                device_name: None,
            }
        }
    }
}

fn extract_field(text: &str, field: &str) -> Option<String> {
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(field) {
            let value = rest.trim_start_matches([':', '=', ' '].as_ref()).trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

pub async fn install_dxvk(prefix_path: &Path, dxvk_version: &str) -> Result<String, String> {
    let cache_dir = crate::utils::file_manager::get_tools_dir().join("dxvk");
    crate::utils::file_manager::ensure_dir(&cache_dir)?;

    let archive_name = format!("dxvk-{}.tar.gz", dxvk_version);
    let archive_path = cache_dir.join(&archive_name);
    let extract_dir = cache_dir.join(format!("dxvk-{}", dxvk_version));

    // Download if not cached
    if !archive_path.exists() {
        let url = format!(
            "https://github.com/doitsujin/dxvk/releases/download/v{}/{}",
            dxvk_version, archive_name
        );
        info!("Downloading DXVK {} from {}", dxvk_version, url);
        download_tool(&url, &archive_path).await?;
    }

    // Extract if not already
    if !extract_dir.exists() {
        extract_tar_gz(&archive_path, &cache_dir)?;
    }

    // Copy DLLs to prefix
    let system32 = prefix_path.join("drive_c").join("windows").join("system32");
    let syswow64 = prefix_path.join("drive_c").join("windows").join("syswow64");

    let x64_dir = extract_dir.join("x64");
    let x32_dir = extract_dir.join("x32");

    let dlls = ["d3d9.dll", "d3d10core.dll", "d3d11.dll", "dxgi.dll"];

    if x64_dir.exists() && system32.exists() {
        for dll in &dlls {
            let src = x64_dir.join(dll);
            if src.exists() {
                std::fs::copy(&src, system32.join(dll))
                    .map_err(|e| format!("Failed to copy DXVK DLL {}: {}", dll, e))?;
            }
        }
    }

    if x32_dir.exists() && syswow64.exists() {
        for dll in &dlls {
            let src = x32_dir.join(dll);
            if src.exists() {
                std::fs::copy(&src, syswow64.join(dll))
                    .map_err(|e| format!("Failed to copy DXVK 32-bit DLL {}: {}", dll, e))?;
            }
        }
    }

    // 写入版本标记文件
    write_dxvk_version_marker(prefix_path, dxvk_version);

    info!(
        "Installed DXVK {} to {}",
        dxvk_version,
        prefix_path.display()
    );
    Ok(format!("DXVK {} installed", dxvk_version))
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

    // 删除版本标记文件
    remove_dxvk_version_marker(prefix_path);

    info!("Uninstalled DXVK from {}", prefix_path.display());
    Ok("DXVK uninstalled".to_string())
}

pub async fn install_vkd3d(prefix_path: &Path, vkd3d_version: &str) -> Result<String, String> {
    let cache_dir = crate::utils::file_manager::get_tools_dir().join("vkd3d");
    crate::utils::file_manager::ensure_dir(&cache_dir)?;

    let archive_name = format!("vkd3d-proton-{}.tar.zst", vkd3d_version);
    let archive_path = cache_dir.join(&archive_name);
    let extract_dir = cache_dir.join(format!("vkd3d-proton-{}", vkd3d_version));

    if !archive_path.exists() {
        let url = format!(
            "https://github.com/HansKristian-Work/vkd3d-proton/releases/download/v{}/{}",
            vkd3d_version, archive_name
        );
        info!("Downloading VKD3D-Proton {} from {}", vkd3d_version, url);
        download_tool(&url, &archive_path).await?;
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

    info!(
        "Installed VKD3D-Proton {} to {} ({} DLLs copied)",
        vkd3d_version,
        prefix_path.display(),
        copied
    );
    Ok(format!("VKD3D-Proton {} installed", vkd3d_version))
}

fn find_arch_dir(root: &Path, names: &[&str]) -> Option<PathBuf> {
    for entry in walkdir::WalkDir::new(root)
        .min_depth(1)
        .max_depth(6)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_dir() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if names.iter().any(|n| file_name == *n) {
            return Some(entry.path().to_path_buf());
        }
    }
    None
}

fn copy_vkd3d_dlls(src_dir: &Path, dst_dir: &Path) -> Result<usize, String> {
    let mut available: std::collections::HashMap<String, PathBuf> =
        std::collections::HashMap::new();
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
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
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

async fn download_tool(url: &str, dest: &Path) -> Result<(), String> {
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

    // 流式写入临时文件，避免大包全量驻留内存
    let mut downloaded: u64 = 0;
    let mut header_buf = [0u8; 6];
    let mut header_filled: usize = 0;

    {
        use futures_util::StreamExt;
        use tokio::io::AsyncWriteExt;
        let mut stream = resp.bytes_stream();
        let mut file = tokio::fs::File::create(dest)
            .await
            .map_err(|e| format!("Failed to create file {}: {}", dest.display(), e))?;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("Failed to read download stream: {}", e))?;
            if header_filled < 6 {
                let need = (6 - header_filled).min(chunk.len());
                header_buf[header_filled..header_filled + need].copy_from_slice(&chunk[..need]);
                header_filled += need;
            }
            file.write_all(&chunk).await
                .map_err(|e| format!("Failed to write chunk: {}", e))?;
            downloaded += chunk.len() as u64;
        }
        file.flush().await.map_err(|e| format!("Failed to flush file: {}", e))?;
    }

    // 完整性校验：最小大小（防止空文件/截断/HTML 错误页面）
    const MIN_TOOL_SIZE: u64 = 10_000; // 10KB
    if downloaded < MIN_TOOL_SIZE {
        tokio::fs::remove_file(dest).await.ok();
        return Err(format!(
            "下载的文件异常（大小 {} 字节，低于 {} 字节），疑似截断或错误页面: {}",
            downloaded, MIN_TOOL_SIZE, url
        ));
    }

    // 归档格式魔数校验（tar.gz/tar.xz/zip）
    let valid_archive = (header_filled >= 2 && header_buf[..2] == [0x1F, 0x8B])        // gzip
        || (header_filled >= 6 && header_buf[..6] == [0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00]) // xz
        || (header_filled >= 4 && header_buf[..4] == [0x50, 0x4B, 0x03, 0x04]);         // zip
    if !valid_archive {
        tokio::fs::remove_file(dest).await.ok();
        return Err(format!(
            "下载的文件不是有效归档格式（魔数不匹配），疑似损坏或被篡改: {}",
            url
        ));
    }

    Ok(())
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
    // Prefer GNU tar built-in zstd support, then fallback to external zstd filter mode.
    // 使用 tokio::process 避免阻塞 async 运行时，且支持取消
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
