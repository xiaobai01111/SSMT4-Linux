use crate::configs::wine_config::{ProtonVariant, WineArch, WineVersion};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Scan all available Wine and Proton versions on the system
pub fn scan_all_versions(custom_paths: &[String]) -> Vec<WineVersion> {
    let mut versions = Vec::new();

    // System Wine
    versions.extend(scan_system_wine());

    // Steam official Proton
    versions.extend(scan_steam_proton());

    // compatibilitytools.d 单次遍历，按前缀自动分类
    versions.extend(scan_all_compat_tools());

    // Lutris Wine runners
    versions.extend(scan_lutris_wine());

    // SSMT4 自己下载的 Wine runners
    versions.extend(scan_ssmt4_wine_runners());

    // Custom paths
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

/// 扫描 SSMT4 自己下载的 Wine runners（~/.local/share/ssmt4/runners/wine/）
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

        // 检查是否包含 proton 脚本（说明是 Proton 不是 Wine）
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

        // Wine 二进制
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
    visited: &mut std::collections::HashSet<std::path::PathBuf>,
) -> Vec<WineVersion> {
    let mut versions = Vec::new();
    if !path.exists() || !path.is_dir() {
        return versions;
    }

    // 符号链接环路保护：canonicalize 后去重
    let canonical = match path.canonicalize() {
        Ok(c) => c,
        Err(_) => return versions,
    };
    if !visited.insert(canonical) {
        warn!("跳过已访问的路径（符号链接环路？）: {}", path.display());
        return versions;
    }

    // Check if this path itself is a proton/wine directory
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
        // Scan subdirectories（限深度）
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    versions.extend(scan_custom_path_inner(&entry.path(), depth + 1, visited));
                }
            }
        }
    } else {
        warn!("Wine 扫描达到最大深度 {}，跳过: {}", SCAN_MAX_DEPTH, path.display());
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

    // Fallback: try to extract version from directory name
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
        // Also check soldier as fallback
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

// ============================================================
// 远程 Wine/Proton 版本获取与下载
// ============================================================

/// 远程可用的 Proton/Wine 版本
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RemoteWineVersion {
    pub tag: String,
    pub version: String,
    pub variant: String,
    pub download_url: String,
    pub file_size: u64,
    pub published_at: String,
    /// 本地是否已安装
    pub installed: bool,
}

/// 从 GitHub 获取远程可用的 Wine/Proton 版本列表
/// 复用单个 HTTP 客户端，并发请求所有仓库
pub async fn fetch_remote_proton_versions(
    installed: &[WineVersion],
) -> Result<Vec<RemoteWineVersion>, String> {
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    // 并发请求所有仓库
    let (ge, wine_ge, kron4ek, dw) = tokio::join!(
        fetch_github_releases(&client, "GloriousEggroll/proton-ge-custom", "GE-Proton", 15, installed),
        fetch_github_releases(&client, "GloriousEggroll/wine-ge-custom", "Wine-GE", 10, installed),
        fetch_github_releases(&client, "Kron4ek/Wine-Builds", "Wine-Builds", 10, installed),
        fetch_github_releases(&client, "AUNaseef/proton-ge-custom", "DW-Proton", 10, installed),
    );

    let mut all = Vec::new();
    all.extend(ge.unwrap_or_default());
    all.extend(wine_ge.unwrap_or_default());
    all.extend(kron4ek.unwrap_or_default());
    all.extend(dw.unwrap_or_default());

    info!("获取到 {} 个远程 Wine/Proton 版本", all.len());
    Ok(all)
}

async fn fetch_github_releases(
    client: &reqwest::Client,
    repo: &str,
    variant: &str,
    max_count: usize,
    installed: &[WineVersion],
) -> Result<Vec<RemoteWineVersion>, String> {
    let url = format!(
        "https://api.github.com/repos/{}/releases?per_page={}",
        repo, max_count
    );

    // GitHub API 也尝试镜像
    let api_urls = [
        url.clone(),
        url.replace("api.github.com", "ghp.ci/api.github.com"),
    ];

    let mut resp = None;
    let mut last_err = String::new();
    for api_url in &api_urls {
        match client
            .get(api_url)
            .header("User-Agent", "SSMT4/0.1")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => {
                resp = Some(r);
                break;
            }
            Ok(r) => {
                last_err = format!("HTTP {}", r.status());
                warn!("GitHub API {} 返回 {}，尝试下一个", api_url, r.status());
            }
            Err(e) => {
                last_err = format!("{}", e);
                warn!("GitHub API {} 失败: {}，尝试下一个", api_url, e);
            }
        }
    }

    let resp = resp.ok_or_else(|| format!("请求 GitHub API 失败: {}", last_err))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub API 返回 HTTP {}", resp.status()));
    }

    let releases: Vec<serde_json::Value> = resp
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    let installed_tags: std::collections::HashSet<String> = installed
        .iter()
        .map(|v| v.name.clone())
        .collect();

    let mut result = Vec::new();
    for release in &releases {
        let tag = release
            .get("tag_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let published_at = release
            .get("published_at")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let assets = release.get("assets").and_then(|v| v.as_array());
        if let Some(assets) = assets {
            for asset in assets {
                let name = asset
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if name.ends_with(".tar.gz") || name.ends_with(".tar.xz") {
                    let download_url = asset
                        .get("browser_download_url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let file_size = asset
                        .get("size")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);

                    let is_installed = installed_tags.contains(&tag);

                    result.push(RemoteWineVersion {
                        tag: tag.clone(),
                        version: tag.clone(),
                        variant: variant.to_string(),
                        download_url,
                        file_size,
                        published_at: published_at.clone(),
                        installed: is_installed,
                    });
                    break;
                }
            }
        }
    }
    Ok(result)
}

/// 获取 Wine runners 安装目录（Lutris 风格）
fn get_wine_runners_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(&home)
        .join(".local")
        .join("share")
        .join("ssmt4")
        .join("runners")
        .join("wine")
}

/// 获取 Proton 安装目录
fn get_proton_install_dir() -> PathBuf {
    get_compat_tools_dirs()
        .into_iter()
        .next()
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_default();
            PathBuf::from(&home)
                .join(".local")
                .join("share")
                .join("Steam")
                .join("compatibilitytools.d")
        })
}

/// 下载并安装 Wine/Proton 版本
/// variant 为 "Wine-GE" / "Wine-Builds" 时安装到 wine runners 目录
/// variant 为 "GE-Proton" / "DW-Proton" 时安装到 compatibilitytools.d
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
    std::fs::create_dir_all(&install_dir)
        .map_err(|e| format!("创建目录失败: {}", e))?;

    let kind = if is_wine { "Wine" } else { "Proton" };

    // GitHub 镜像加速（国内网络友好）
    let mirrors = [
        download_url.to_string(),
        download_url.replace("github.com", "ghp.ci"),
        download_url.replace(
            "github.com",
            "gh-proxy.com/github.com",
        ),
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
                warn!("镜像 {} 返回 HTTP {}，尝试下一个", url, r.status());
            }
            Err(e) => {
                last_err = format!("{}: {}", url, e);
                warn!("镜像 {} 连接失败: {}，尝试下一个", url, e);
            }
        }
    }

    let resp = resp_ok.ok_or_else(|| format!("所有镜像均下载失败，最后错误: {}", last_err))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}: {}", resp.status(), download_url));
    }

    // 流式下载到临时文件，避免大包全量驻留内存
    let total_size = resp.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;
    let mut stream = resp.bytes_stream();
    use futures_util::StreamExt;

    let emit_progress = |phase: &str, current: u64, total: u64| {
        if let Some(ref a) = app {
            use tauri::Emitter;
            a.emit("component-download-progress", serde_json::json!({
                "component": format!("{} {}", kind, tag),
                "phase": phase,
                "downloaded": current,
                "total": total,
            })).ok();
        }
    };

    let ext = if download_url.ends_with(".tar.xz") {
        "tar.xz"
    } else {
        "tar.gz"
    };
    let tmp_file = install_dir.join(format!("{}.{}", tag, ext));
    std::fs::create_dir_all(&install_dir)
        .map_err(|e| format!("创建目录失败: {}", e))?;

    // 流式写入：边下载边写磁盘，内存仅占一个 chunk
    let mut file = tokio::fs::File::create(&tmp_file)
        .await
        .map_err(|e| format!("创建临时文件失败: {}", e))?;
    use tokio::io::AsyncWriteExt;

    // 保存前 6 字节用于魔数校验
    let mut header_buf = [0u8; 6];
    let mut header_filled: usize = 0;

    emit_progress("downloading", 0, total_size);
    let mut last_emit = std::time::Instant::now();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("下载流读取失败: {}", e))?;
        // 填充 header 校验缓冲区
        if header_filled < 6 {
            let need = (6 - header_filled).min(chunk.len());
            header_buf[header_filled..header_filled + need].copy_from_slice(&chunk[..need]);
            header_filled += need;
        }
        file.write_all(&chunk).await
            .map_err(|e| format!("写入临时文件失败: {}", e))?;
        downloaded += chunk.len() as u64;
        // 节流：每 200ms 上报一次进度
        if last_emit.elapsed() >= std::time::Duration::from_millis(200) {
            emit_progress("downloading", downloaded, total_size);
            last_emit = std::time::Instant::now();
        }
    }
    // 确保最终进度上报
    emit_progress("downloading", downloaded, total_size);
    file.flush().await.map_err(|e| format!("刷新临时文件失败: {}", e))?;
    drop(file);

    // 下载完整性校验
    const MIN_ARCHIVE_SIZE: u64 = 1_000_000; // 1MB
    if downloaded < MIN_ARCHIVE_SIZE {
        tokio::fs::remove_file(&tmp_file).await.ok();
        return Err(format!(
            "{} {} 下载异常：文件大小 {} 字节，低于最小阈值 {} 字节，疑似截断或损坏",
            kind, tag, downloaded, MIN_ARCHIVE_SIZE
        ));
    }
    // tar 归档魔数校验（xz: 0xFD377A585A00, gzip: 0x1F8B）
    let valid_header = if download_url.ends_with(".tar.xz") {
        header_filled >= 6 && header_buf[..6] == [0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00]
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

    // 解压（异步子进程，不阻塞 tokio 运行时）
    emit_progress("extracting", 0, 0);
    info!("解压 {} 到 {}", tmp_file.display(), install_dir.display());
    let tar_flag = if ext == "tar.xz" { "-xf" } else { "-xzf" };
    let status = tokio::process::Command::new("tar")
        .arg(tar_flag)
        .arg(&tmp_file)
        .arg("-C")
        .arg(&install_dir)
        .status()
        .await
        .map_err(|e| format!("解压失败: {}", e))?;

    if !status.success() {
        return Err(format!("解压 {} 失败", tmp_file.display()));
    }

    // 删除临时文件
    tokio::fs::remove_file(&tmp_file).await.ok();

    emit_progress("done", total_size, total_size);
    info!("{} {} 安装完成 → {}", kind, tag, install_dir.display());
    Ok(format!("{} {} 安装完成", kind, tag))
}
