use super::install::{get_proton_install_dir, get_wine_runners_dir};
use crate::configs::wine_config::{ProtonVariant, WineArch, WineVersion};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Scan all available Wine and Proton versions on the system
pub fn scan_all_versions(custom_paths: &[String]) -> Vec<WineVersion> {
    let mut versions = Vec::new();

    versions.extend(scan_system_wine());
    versions.extend(scan_steam_proton());
    versions.extend(scan_all_compat_tools());
    versions.extend(scan_lutris_wine());
    versions.extend(scan_ssmt4_wine_runners());

    for path in custom_paths {
        versions.extend(scan_custom_path(Path::new(path)));
    }

    versions.sort_by(|a, b| {
        a.variant
            .to_string()
            .cmp(&b.variant.to_string())
            .then_with(|| b.version.cmp(&a.version))
    });

    info!("Found {} Wine/Proton versions", versions.len());
    versions
}

fn get_steam_root() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let candidates = [
        PathBuf::from(&home).join(".steam").join("steam"),
        PathBuf::from(&home)
            .join(".local")
            .join("share")
            .join("Steam"),
        PathBuf::from(&home)
            .join(".var")
            .join("app")
            .join("com.valvesoftware.Steam")
            .join(".steam")
            .join("steam"),
    ];
    candidates.into_iter().find(|p| p.exists())
}

fn get_compat_tools_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut canonical_set: Vec<PathBuf> = Vec::new();

    let mut try_add = |path: PathBuf| {
        if !path.exists() {
            return;
        }
        let canon = path.canonicalize().unwrap_or_else(|_| path.clone());
        if !canonical_set.contains(&canon) {
            canonical_set.push(canon);
            dirs.push(path);
        }
    };

    let data_proton = get_proton_install_dir();
    try_add(data_proton);

    if let Some(steam) = get_steam_root() {
        try_add(steam.join("compatibilitytools.d"));
    }
    if let Ok(home) = std::env::var("HOME") {
        let xdg = PathBuf::from(&home)
            .join(".local")
            .join("share")
            .join("Steam")
            .join("compatibilitytools.d");
        try_add(xdg);
    }
    dirs
}

fn scan_system_wine() -> Vec<WineVersion> {
    let mut versions = Vec::new();
    for name in &["wine", "wine64"] {
        if let Ok(path) = which::which(name) {
            let version = get_wine_version(&path).unwrap_or_else(|| "unknown".to_string());
            let id = format!("system-{}-{}", name, version);
            info!("Found system wine: {} ({})", path.display(), version);
            versions.push(WineVersion {
                id,
                name: format!("System {} {}", name, version),
                variant: ProtonVariant::SystemWine,
                path,
                version,
                arch: WineArch::Win64,
                supports_dxvk: false,
                timestamp: None,
            });
        }
    }
    versions
}

fn get_wine_version(wine_path: &Path) -> Option<String> {
    let output = std::process::Command::new(wine_path)
        .arg("--version")
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let version = stdout.trim().strip_prefix("wine-").unwrap_or(stdout.trim());
    Some(version.to_string())
}

fn scan_steam_proton() -> Vec<WineVersion> {
    let mut versions = Vec::new();
    let Some(steam_root) = get_steam_root() else {
        return versions;
    };

    let common_dir = steam_root.join("steamapps").join("common");
    if !common_dir.exists() {
        return versions;
    }

    let entries = match std::fs::read_dir(&common_dir) {
        Ok(e) => e,
        Err(_) => return versions,
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("Proton") {
            continue;
        }

        let dir = entry.path();
        let proton_bin = dir.join("proton");
        if !proton_bin.exists() {
            continue;
        }

        let variant = if name.contains("Experimental") {
            ProtonVariant::Experimental
        } else {
            ProtonVariant::Official
        };

        let (version, timestamp) = read_proton_version(&dir);
        let id = format!(
            "{}-{}",
            variant.to_string().to_lowercase().replace(' ', "-"),
            version
        );

        info!("Found Steam Proton: {} ({})", name, version);
        versions.push(WineVersion {
            id,
            name: name.clone(),
            variant,
            path: proton_bin,
            version,
            arch: WineArch::Win64,
            supports_dxvk: true,
            timestamp,
        });
    }
    versions
}

/// 单次遍历 compatibilitytools.d，按目录名前缀自动分类为对应的 ProtonVariant
fn scan_all_compat_tools() -> Vec<WineVersion> {
    let mut versions = Vec::new();
    for compat_dir in get_compat_tools_dirs() {
        if !compat_dir.exists() {
            continue;
        }
        let entries = match std::fs::read_dir(&compat_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let dir = entry.path();
            let proton_bin = dir.join("proton");
            if !proton_bin.exists() {
                continue;
            }

            let variant = classify_proton_variant(&name);
            let (version, timestamp) = read_proton_version(&dir);

            let id = if matches!(variant, ProtonVariant::Custom) {
                format!("custom-{}", name.to_lowercase().replace(' ', "-"))
            } else {
                format!(
                    "{}-{}",
                    variant
                        .to_string()
                        .to_lowercase()
                        .replace(' ', "-")
                        .replace('-', "_"),
                    version
                )
            };

            info!("Found {}: {} ({})", variant, name, version);
            versions.push(WineVersion {
                id,
                name: name.clone(),
                variant,
                path: proton_bin,
                version,
                arch: WineArch::Win64,
                supports_dxvk: true,
                timestamp,
            });
        }
    }
    versions
}

/// Try to classify unknown Proton variant by name
fn classify_proton_variant(name: &str) -> ProtonVariant {
    let lower = name.to_lowercase();
    if lower.contains("ge-proton") || lower.contains("geproton") {
        ProtonVariant::GEProton
    } else if lower.contains("dw-proton") || lower.contains("dwproton") {
        ProtonVariant::DWProton
    } else if lower.contains("tkg") {
        ProtonVariant::ProtonTKG
    } else if lower.contains("experimental") {
        ProtonVariant::Experimental
    } else if lower.starts_with("proton") {
        ProtonVariant::Official
    } else {
        ProtonVariant::Custom
    }
}

fn scan_lutris_wine() -> Vec<WineVersion> {
    let mut versions = Vec::new();
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => return versions,
    };

    let runners_dir = PathBuf::from(&home)
        .join(".local")
        .join("share")
        .join("lutris")
        .join("runners")
        .join("wine");

    if !runners_dir.exists() {
        return versions;
    }

    let entries = match std::fs::read_dir(&runners_dir) {
        Ok(e) => e,
        Err(_) => return versions,
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let wine_bin = entry.path().join("bin").join("wine64");
        let wine_bin_alt = entry.path().join("bin").join("wine");
        let wine_path = if wine_bin.exists() {
            wine_bin
        } else if wine_bin_alt.exists() {
            wine_bin_alt
        } else {
            continue;
        };

        let version = get_wine_version(&wine_path).unwrap_or(name.clone());
        let id = format!("lutris-{}", name.to_lowercase().replace(' ', "-"));

        info!("Found Lutris Wine: {} ({})", name, version);
        versions.push(WineVersion {
            id,
            name: format!("Lutris {}", name),
            variant: ProtonVariant::Lutris,
            path: wine_path,
            version,
            arch: WineArch::Win64,
            supports_dxvk: false,
            timestamp: None,
        });
    }
    versions
}

/// 扫描 SSMT4 自己下载的 Wine runners（<dataDir>/runners/wine/）
fn scan_ssmt4_wine_runners() -> Vec<WineVersion> {
    let mut versions = Vec::new();
    let runners_dir = get_wine_runners_dir();

    if !runners_dir.exists() {
        return versions;
    }

    let entries = match std::fs::read_dir(&runners_dir) {
        Ok(e) => e,
        Err(_) => return versions,
    };

    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();

        let proton_bin = entry.path().join("proton");
        if proton_bin.exists() {
            let (version, timestamp) = read_proton_version(&entry.path());
            let variant = classify_proton_variant(&name);
            let id = format!("ssmt4-proton-{}", name.to_lowercase().replace(' ', "-"));
            info!("Found SSMT4 Proton runner: {} ({})", name, version);
            versions.push(WineVersion {
                id,
                name: name.clone(),
                variant,
                path: proton_bin,
                version,
                arch: WineArch::Win64,
                supports_dxvk: true,
                timestamp,
            });
            continue;
        }

        let wine_bin = entry.path().join("bin").join("wine64");
        let wine_bin_alt = entry.path().join("bin").join("wine");
        let wine_path = if wine_bin.exists() {
            wine_bin
        } else if wine_bin_alt.exists() {
            wine_bin_alt
        } else {
            continue;
        };

        let version = get_wine_version(&wine_path).unwrap_or(name.clone());
        let id = format!("ssmt4-wine-{}", name.to_lowercase().replace(' ', "-"));

        info!("Found SSMT4 Wine runner: {} ({})", name, version);
        versions.push(WineVersion {
            id,
            name: name.clone(),
            variant: ProtonVariant::Custom,
            path: wine_path,
            version,
            arch: WineArch::Win64,
            supports_dxvk: true,
            timestamp: None,
        });
    }
    versions
}

/// 最大递归扫描深度，防止大目录/符号链接环路拖垮扫描
const SCAN_MAX_DEPTH: u32 = 3;

fn scan_custom_path(path: &Path) -> Vec<WineVersion> {
    let mut visited = std::collections::HashSet::new();
    scan_custom_path_inner(path, 0, &mut visited)
}

fn scan_custom_path_inner(
    path: &Path,
    depth: u32,
    visited: &mut std::collections::HashSet<PathBuf>,
) -> Vec<WineVersion> {
    let mut versions = Vec::new();
    if !path.exists() || !path.is_dir() {
        return versions;
    }

    let canonical = match path.canonicalize() {
        Ok(c) => c,
        Err(_) => return versions,
    };
    if !visited.insert(canonical) {
        warn!("跳过已访问的路径（符号链接环路？）: {}", path.display());
        return versions;
    }

    let proton_bin = path.join("proton");
    let wine_bin = path.join("bin").join("wine64");
    let wine_bin_alt = path.join("bin").join("wine");

    if proton_bin.exists() {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let (version, timestamp) = read_proton_version(path);
        let variant = classify_proton_variant(&name);
        let id = format!("custom-{}", name.to_lowercase().replace(' ', "-"));

        versions.push(WineVersion {
            id,
            name: format!("Custom: {}", name),
            variant,
            path: proton_bin,
            version,
            arch: WineArch::Win64,
            supports_dxvk: true,
            timestamp,
        });
    } else if wine_bin.exists() || wine_bin_alt.exists() {
        let actual = if wine_bin.exists() {
            wine_bin
        } else {
            wine_bin_alt
        };
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let version = get_wine_version(&actual).unwrap_or_else(|| "unknown".to_string());
        let id = format!("custom-wine-{}", name.to_lowercase().replace(' ', "-"));

        versions.push(WineVersion {
            id,
            name: format!("Custom: {}", name),
            variant: ProtonVariant::Custom,
            path: actual,
            version,
            arch: WineArch::Win64,
            supports_dxvk: false,
            timestamp: None,
        });
    } else if depth < SCAN_MAX_DEPTH {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    versions.extend(scan_custom_path_inner(&entry.path(), depth + 1, visited));
                }
            }
        }
    } else {
        warn!(
            "Wine 扫描达到最大深度 {}，跳过: {}",
            SCAN_MAX_DEPTH,
            path.display()
        );
    }
    versions
}

fn read_proton_version(dir: &Path) -> (String, Option<String>) {
    let version_file = dir.join("version");
    if version_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&version_file) {
            let content = content.trim();
            let parts: Vec<&str> = content.splitn(2, ' ').collect();
            if parts.len() == 2 {
                return (parts[1].to_string(), Some(parts[0].to_string()));
            }
            return (content.to_string(), None);
        }
    }

    let dir_name = dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    (dir_name, None)
}

pub fn find_steam_linux_runtime() -> Option<PathBuf> {
    let steam_root = get_steam_root()?;
    let sniper = steam_root
        .join("steamapps")
        .join("common")
        .join("SteamLinuxRuntime_sniper");
    let entry_point = sniper.join("_v2-entry-point");
    if entry_point.exists() {
        info!("Found SteamLinuxRuntime_sniper at {}", sniper.display());
        Some(sniper)
    } else {
        let soldier = steam_root
            .join("steamapps")
            .join("common")
            .join("SteamLinuxRuntime_soldier");
        let entry_point_soldier = soldier.join("_v2-entry-point");
        if entry_point_soldier.exists() {
            warn!(
                "sniper not found, falling back to soldier at {}",
                soldier.display()
            );
            Some(soldier)
        } else {
            warn!("No SteamLinuxRuntime found");
            None
        }
    }
}

pub fn get_steam_root_path() -> Option<PathBuf> {
    get_steam_root()
}
