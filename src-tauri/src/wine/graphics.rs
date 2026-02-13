use std::path::{Path, PathBuf};
use tracing::{info, warn};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VulkanInfo {
    pub available: bool,
    pub version: Option<String>,
    pub driver: Option<String>,
    pub device_name: Option<String>,
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
        extract_tar_zst(&archive_path, &cache_dir)?;
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
        warn!(
            "VKD3D archive has no x86/x32 directory, only 64-bit DLLs were installed"
        );
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
    let mut available: std::collections::HashMap<String, PathBuf> = std::collections::HashMap::new();
    for entry in std::fs::read_dir(src_dir)
        .map_err(|e| format!("Failed to read VKD3D directory {}: {}", src_dir.display(), e))?
    {
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

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(dest, &bytes)
        .map_err(|e| format!("Failed to write file {}: {}", dest.display(), e))
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

fn extract_tar_zst(archive: &Path, dest: &Path) -> Result<(), String> {
    // Prefer GNU tar built-in zstd support, then fallback to external zstd filter mode.
    let try_builtin = std::process::Command::new("tar")
        .arg("--zstd")
        .arg("-xf")
        .arg(archive)
        .arg("-C")
        .arg(dest)
        .status();

    match try_builtin {
        Ok(status) if status.success() => return Ok(()),
        _ => {}
    }

    let try_filter = std::process::Command::new("tar")
        .arg("-I")
        .arg("zstd")
        .arg("-xf")
        .arg(archive)
        .arg("-C")
        .arg(dest)
        .status()
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
