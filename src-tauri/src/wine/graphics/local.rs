use super::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

fn detect_local_variant_version(
    name: &str,
    is_dir: bool,
    variants: &[DxvkVariantSource],
) -> Option<(String, String)> {
    let mut best_match: Option<(usize, String, String)> = None;

    for source in variants {
        if !source.enabled || source.id.trim().is_empty() {
            continue;
        }
        let template = source.archive_name_template.trim();
        if template.is_empty() {
            continue;
        }

        let mut candidates: Vec<(String, &str)> = Vec::new();
        if is_dir {
            if let Some(version) = extract_version_by_template(name, template) {
                candidates.push((version, template));
            }
            let stripped_template = strip_archive_suffix(template);
            if stripped_template != template {
                if let Some(version) = extract_version_by_template(name, stripped_template) {
                    candidates.push((version, stripped_template));
                }
            }
        } else {
            if let Some(version) = extract_version_by_template(name, template) {
                candidates.push((version, template));
            }
            let stripped_name = strip_archive_suffix(name);
            let stripped_template = strip_archive_suffix(template);
            if let Some(version) = extract_version_by_template(stripped_name, stripped_template) {
                candidates.push((version, stripped_template));
            }
        }

        for (version, matched_template) in candidates {
            if !version.trim().is_empty() {
                let specificity = matched_template.replace("{version}", "").len();
                let should_replace = match best_match.as_ref() {
                    Some((score, _, _)) => specificity > *score,
                    None => true,
                };
                if should_replace {
                    best_match = Some((specificity, source.id.clone(), version));
                }
            }
        }
    }

    if let Some((_, variant, version)) = best_match {
        return Some((variant, version));
    }

    let normalized = strip_archive_suffix(name);
    if let Some(version) = normalized
        .strip_prefix("dxvk-gplasync-v")
        .or_else(|| normalized.strip_prefix("dxvk-gplasync-"))
    {
        if !version.trim().is_empty() {
            return Some(("gplasync".to_string(), version.to_string()));
        }
    }
    if let Some(version) = normalized.strip_prefix("dxvk-") {
        if !version.trim().is_empty() {
            return Some(("dxvk".to_string(), version.to_string()));
        }
    }
    None
}

fn is_supported_archive_name(name: &str) -> bool {
    [".tar.gz", ".tar.xz", ".tar.zst", ".zip"]
        .iter()
        .any(|suffix| name.ends_with(suffix))
}

pub fn scan_local_dxvk_versions() -> Vec<DxvkLocalVersion> {
    let cache_dir = crate::utils::file_manager::get_tools_dir().join("dxvk");
    if !cache_dir.exists() {
        return Vec::new();
    }

    let variants = load_dxvk_variants();
    let mut by_key: HashMap<(String, String), DxvkLocalVersion> = HashMap::new();

    let mut upsert = |variant: String, version: String, extracted: bool, path: PathBuf| match by_key
        .get_mut(&(variant.clone(), version.clone()))
    {
        Some(existing) => {
            if extracted && !existing.extracted {
                existing.extracted = true;
                existing.path = path;
            }
        }
        None => {
            by_key.insert(
                (variant.clone(), version.clone()),
                DxvkLocalVersion {
                    version,
                    variant,
                    extracted,
                    path,
                },
            );
        }
    };

    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
        for entry in entries.filter_map(|entry| entry.ok()) {
            let path = entry.path();
            let is_dir = path.is_dir();
            let is_file = path.is_file();
            if !is_dir && !is_file {
                continue;
            }

            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if name.trim().is_empty() {
                continue;
            }

            if is_file && !is_supported_archive_name(&name) {
                continue;
            }

            let Some((variant, version)) = detect_local_variant_version(&name, is_dir, variants)
            else {
                continue;
            };

            let extracted = if is_dir {
                path.join("x64").exists() || path.join("x32").exists()
            } else {
                false
            };

            upsert(variant, version, extracted, path);
        }
    }

    let mut versions: Vec<DxvkLocalVersion> = by_key.into_values().collect();
    versions.sort_by(|a, b| {
        b.version
            .cmp(&a.version)
            .then_with(|| a.variant.cmp(&b.variant))
    });
    versions
}

pub fn detect_installed_dxvk(prefix_path: &Path) -> DxvkInstalledStatus {
    let system32 = prefix_path.join("drive_c").join("windows").join("system32");
    let syswow64 = prefix_path.join("drive_c").join("windows").join("syswow64");
    let dxvk_dlls = ["d3d9.dll", "d3d10core.dll", "d3d11.dll", "dxgi.dll"];
    let mut found_dlls = Vec::new();
    let marker_version = read_dxvk_version_marker(prefix_path);

    for dll in &dxvk_dlls {
        let mut matched = false;
        let candidates = [system32.join(dll), syswow64.join(dll)];
        for dll_path in &candidates {
            if !dll_path.exists() {
                continue;
            }
            if looks_like_dxvk_dll(dll_path) {
                matched = true;
                break;
            }
        }
        if matched {
            found_dlls.push(dll.to_string());
        }
    }

    let installed = !found_dlls.is_empty() || marker_version.is_some();
    let version = if installed {
        let version_from_marker = marker_version.clone();
        if version_from_marker.is_some() {
            info!("[DXVK] 版本来源: 标记文件 → {:?}", version_from_marker);
            version_from_marker
        } else {
            let version_from_binary =
                extract_dxvk_version_from_dirs(&[system32.clone(), syswow64.clone()]);
            if version_from_binary.is_some() {
                info!(
                    "[DXVK] 版本来源: DLL 二进制搜索 → {:?}",
                    version_from_binary
                );
                version_from_binary
            } else {
                let version_from_size = match_dxvk_version_by_size(&[system32, syswow64]);
                if version_from_size.is_some() {
                    info!("[DXVK] 版本来源: 文件大小比对 → {:?}", version_from_size);
                } else {
                    warn!(
                        "[DXVK] 三层版本检测均失败（标记文件/二进制搜索/大小比对）prefix={}",
                        prefix_path.display()
                    );
                }
                version_from_size
            }
        }
    } else {
        None
    };

    DxvkInstalledStatus {
        installed,
        version,
        dlls_found: found_dlls,
    }
}

fn looks_like_dxvk_dll(path: &Path) -> bool {
    if extract_version_from_binary(path).is_some() {
        return true;
    }
    std::fs::metadata(path)
        .map(|meta| meta.len() >= 120_000)
        .unwrap_or(false)
}

pub fn scan_local_vkd3d_versions() -> Vec<Vkd3dLocalVersion> {
    let cache_dir = crate::utils::file_manager::get_tools_dir().join("vkd3d");
    if !cache_dir.exists() {
        return Vec::new();
    }

    let mut by_version: HashMap<String, Vkd3dLocalVersion> = HashMap::new();
    let mut upsert =
        |version: String, extracted: bool, path: PathBuf| match by_version.get_mut(&version) {
            Some(existing) => {
                if extracted && !existing.extracted {
                    existing.extracted = true;
                    existing.path = path;
                }
            }
            None => {
                by_version.insert(
                    version.clone(),
                    Vkd3dLocalVersion {
                        version,
                        extracted,
                        path,
                    },
                );
            }
        };

    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
        for entry in entries.filter_map(|entry| entry.ok()) {
            let path = entry.path();
            let is_dir = path.is_dir();
            let is_file = path.is_file();
            if !is_dir && !is_file {
                continue;
            }

            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if name.trim().is_empty() {
                continue;
            }

            if is_file && !name.ends_with(".tar.zst") {
                continue;
            }

            let Some(version) = parse_vkd3d_version_from_name(&name) else {
                continue;
            };

            let extracted = if is_dir {
                path.join("x64").exists()
                    || find_arch_dir(&path, &["x64"]).is_some()
                    || path.join("x86").exists()
                    || path.join("x32").exists()
            } else {
                false
            };

            upsert(version, extracted, path);
        }
    }

    let mut versions: Vec<Vkd3dLocalVersion> = by_version.into_values().collect();
    versions.sort_by(|a, b| b.version.cmp(&a.version));
    versions
}

pub fn detect_installed_vkd3d(prefix_path: &Path) -> Vkd3dInstalledStatus {
    let system32 = prefix_path.join("drive_c").join("windows").join("system32");
    let syswow64 = prefix_path.join("drive_c").join("windows").join("syswow64");
    let vkd3d_dlls = ["d3d12.dll", "d3d12core.dll", "dxil.dll"];
    let mut found_dlls = Vec::new();
    let marker_version = read_vkd3d_version_marker(prefix_path);

    for dll in &vkd3d_dlls {
        if system32.join(dll).exists() || syswow64.join(dll).exists() {
            found_dlls.push(dll.to_string());
        }
    }

    let installed = !found_dlls.is_empty() || marker_version.is_some();
    let version = if installed {
        if marker_version.is_some() {
            marker_version
        } else {
            match_vkd3d_version_by_size(&[system32, syswow64])
        }
    } else {
        None
    };

    Vkd3dInstalledStatus {
        installed,
        version,
        dlls_found: found_dlls,
    }
}

fn match_vkd3d_version_by_size(dirs: &[PathBuf]) -> Option<String> {
    let local_versions = scan_local_vkd3d_versions();
    let reference_dlls = ["d3d12.dll", "d3d12core.dll", "dxil.dll"];

    let mut installed_size = None;
    for dir in dirs {
        for dll in &reference_dlls {
            let path = dir.join(dll);
            if let Ok(meta) = std::fs::metadata(path) {
                installed_size = Some(meta.len());
                break;
            }
        }
        if installed_size.is_some() {
            break;
        }
    }
    let installed_size = installed_size?;

    for local in &local_versions {
        if !local.extracted {
            continue;
        }

        let x64_dir =
            find_arch_dir(&local.path, &["x64"]).unwrap_or_else(|| local.path.join("x64"));
        for dll in &reference_dlls {
            let cached_dll = x64_dir.join(dll);
            if let Ok(meta) = std::fs::metadata(&cached_dll) {
                if meta.len() == installed_size {
                    return Some(local.version.clone());
                }
            }
        }
    }
    None
}

fn extract_dxvk_version_from_dirs(dirs: &[PathBuf]) -> Option<String> {
    for dir in dirs {
        for dll_name in &["dxgi.dll", "d3d11.dll", "d3d9.dll"] {
            let dll_path = dir.join(dll_name);
            if let Some(version) = extract_version_from_binary(&dll_path) {
                return Some(version);
            }
        }
    }
    None
}

fn extract_version_from_binary(path: &Path) -> Option<String> {
    let data = std::fs::read(path).ok()?;
    let needle = b"dxvk-";

    for index in 0..data.len().saturating_sub(needle.len()) {
        if &data[index..index + needle.len()] != needle {
            continue;
        }

        let start = index + needle.len();
        let end = (start + 20).min(data.len());
        let candidate = &data[start..end];

        if candidate.is_empty() || !candidate[0].is_ascii_digit() {
            continue;
        }

        let version_bytes: Vec<u8> = candidate
            .iter()
            .take_while(|&&value| value.is_ascii_digit() || value == b'.')
            .copied()
            .collect();

        let version = String::from_utf8(version_bytes).ok()?;
        if version.contains('.') && !version.ends_with('.') && version.len() >= 3 {
            return Some(version);
        }
    }
    None
}

fn match_dxvk_version_by_size(dirs: &[PathBuf]) -> Option<String> {
    let local_versions = scan_local_dxvk_versions();
    let reference_dll = "dxgi.dll";

    let mut installed_size = None;
    for dir in dirs {
        let path = dir.join(reference_dll);
        if let Ok(meta) = std::fs::metadata(path) {
            installed_size = Some(meta.len());
            break;
        }
    }
    let installed_size = installed_size?;

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

    fn sample_variants() -> Vec<DxvkVariantSource> {
        vec![
            DxvkVariantSource {
                id: "dxvk".to_string(),
                display_name: "DXVK".to_string(),
                provider: "github_releases".to_string(),
                repo: "doitsujin/dxvk".to_string(),
                endpoint: String::new(),
                asset_pattern: "(?i)^dxvk-.*\\.tar\\.gz$".to_string(),
                download_url_template: String::new(),
                archive_name_template: "dxvk-{version}.tar.gz".to_string(),
                include_prerelease: false,
                enabled: true,
                note: String::new(),
            },
            DxvkVariantSource {
                id: "gplasync".to_string(),
                display_name: "DXVK-GPLAsync".to_string(),
                provider: "gitlab_tree".to_string(),
                repo: "Ph42oN/dxvk-gplasync".to_string(),
                endpoint: String::new(),
                asset_pattern: "(?i)^dxvk-gplasync-.*\\.tar\\.gz$".to_string(),
                download_url_template: String::new(),
                archive_name_template: "dxvk-gplasync-v{version}.tar.gz".to_string(),
                include_prerelease: false,
                enabled: true,
                note: String::new(),
            },
        ]
    }

    #[test]
    fn detect_local_variant_version_prefers_matching_template() {
        let variants = sample_variants();

        let detected =
            detect_local_variant_version("dxvk-gplasync-v2.4.tar.gz", false, &variants).unwrap();

        assert_eq!(detected.0, "gplasync");
        assert_eq!(detected.1, "2.4");
    }

    #[test]
    fn supported_archive_name_filters_known_suffixes() {
        assert!(is_supported_archive_name("dxvk-2.4.tar.gz"));
        assert!(is_supported_archive_name("dxvk-2.4.zip"));
        assert!(!is_supported_archive_name("dxvk-2.4.7z"));
    }

    #[test]
    fn extract_version_from_binary_reads_embedded_dxvk_marker() {
        let root = unique_temp_dir("ssmt4-dxvk-binary");
        std::fs::create_dir_all(&root).unwrap();
        let dll_path = root.join("dxgi.dll");
        let payload = b"random-bytes dxvk-2.5.1 more-bytes";
        std::fs::write(&dll_path, payload).unwrap();

        assert_eq!(
            extract_version_from_binary(&dll_path).as_deref(),
            Some("2.5.1")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn extract_field_supports_colon_and_equals_formats() {
        let text = "\
Vulkan Instance Version: 1.3.280\n\
driverName = RADV\n\
deviceName: AMD Radeon\n";

        assert_eq!(
            extract_field(text, "Vulkan Instance Version:").as_deref(),
            Some("1.3.280")
        );
        assert_eq!(extract_field(text, "driverName").as_deref(), Some("RADV"));
        assert_eq!(
            extract_field(text, "deviceName").as_deref(),
            Some("AMD Radeon")
        );
    }
}
