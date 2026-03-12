use crate::configs::app_config;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

// ============================================================
// 原版 XXMI 项目的 GitHub 包源
// ============================================================

/// XXMI 包源定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XxmiPackageSource {
    pub id: String,
    pub display_name: String,
    pub github_repo: String,
    pub asset_prefix: String,
}

/// 所有已知的 XXMI 包源（repo owner 和 asset prefix 与原版 XXMI-Launcher 一致）
pub fn known_package_sources() -> Vec<XxmiPackageSource> {
    vec![
        XxmiPackageSource {
            id: "xxmi-libs".to_string(),
            display_name: "XXMI 核心库 (3DMigoto DLL)".to_string(),
            github_repo: "SpectrumQT/XXMI-Libs-Package".to_string(),
            asset_prefix: "XXMI-PACKAGE".to_string(),
        },
        XxmiPackageSource {
            id: "wwmi".to_string(),
            display_name: "WWMI (鸣潮)".to_string(),
            github_repo: "SpectrumQT/WWMI-Package".to_string(),
            asset_prefix: "WWMI-PACKAGE".to_string(),
        },
        XxmiPackageSource {
            id: "srmi".to_string(),
            display_name: "SRMI (星穹铁道)".to_string(),
            github_repo: "SpectrumQT/SRMI-Package".to_string(),
            asset_prefix: "SRMI-TEST-PACKAGE".to_string(),
        },
        XxmiPackageSource {
            id: "gimi".to_string(),
            display_name: "GIMI (原神)".to_string(),
            github_repo: "SilentNightSound/GIMI-Package".to_string(),
            asset_prefix: "GIMI-PACKAGE".to_string(),
        },
        XxmiPackageSource {
            id: "zzmi".to_string(),
            display_name: "ZZMI (绝区零)".to_string(),
            github_repo: "leotorrez/ZZMI-Package".to_string(),
            asset_prefix: "ZZMI-PACKAGE".to_string(),
        },
        XxmiPackageSource {
            id: "himi".to_string(),
            display_name: "HIMI (崩坏3)".to_string(),
            github_repo: "leotorrez/HIMI-Package".to_string(),
            asset_prefix: "HIMI-PACKAGE".to_string(),
        },
        XxmiPackageSource {
            id: "efmi".to_string(),
            display_name: "EFMI (明日方舟:终末地)".to_string(),
            github_repo: "SpectrumQT/EFMI-Package".to_string(),
            asset_prefix: "EFMI-PACKAGE".to_string(),
        },
    ]
}

/// 3Dmigoto-data 本地根目录
pub fn migoto_data_dir() -> PathBuf {
    app_config::get_app_data_dir().join("3Dmigoto-data")
}

/// 包缓存目录（存放下载的 zip 和解压后的文件）
fn packages_cache_dir() -> PathBuf {
    migoto_data_dir().join("Packages")
}

// ============================================================
// 数据结构
// ============================================================

#[derive(Debug, Clone, Serialize)]
pub struct XxmiRemoteVersion {
    pub source_id: String,
    pub source_name: String,
    pub version: String,
    pub tag: String,
    pub published_at: String,
    pub download_url: String,
    pub asset_name: String,
    pub asset_size: u64,
    pub installed: bool,
    pub body: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct XxmiLocalPackage {
    pub source_id: String,
    pub source_name: String,
    pub version: String,
    pub extracted_path: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct XxmiLocalStatus {
    pub packages: Vec<XxmiLocalPackage>,
}

// ============================================================
// GitHub API 响应（只取需要的字段）
// ============================================================

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    published_at: Option<String>,
    assets: Vec<GhAsset>,
    body: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    size: u64,
    browser_download_url: String,
}

// ============================================================
// 扫描本地已安装的包
// ============================================================

pub fn scan_local_xxmi_packages() -> XxmiLocalStatus {
    let cache_dir = packages_cache_dir();
    let sources = known_package_sources();
    let mut packages = Vec::new();

    for source in &sources {
        let source_dir = cache_dir.join(&source.id);
        if !source_dir.exists() {
            continue;
        }
        // 扫描已解压的版本目录
        if let Ok(entries) = std::fs::read_dir(&source_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let dir_name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    // 目录名即版本号（如 "v0.7.9"）
                    if dir_name.starts_with('v')
                        || dir_name
                            .chars()
                            .next()
                            .map_or(false, |c| c.is_ascii_digit())
                    {
                        let size = dir_size(&path);
                        packages.push(XxmiLocalPackage {
                            source_id: source.id.clone(),
                            source_name: source.display_name.clone(),
                            version: dir_name,
                            extracted_path: path.to_string_lossy().to_string(),
                            size_bytes: size,
                        });
                    }
                }
            }
        }
    }

    XxmiLocalStatus { packages }
}

fn dir_size(path: &Path) -> u64 {
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

// ============================================================
// 从 GitHub 获取远程版本列表
// ============================================================

pub async fn fetch_xxmi_remote_versions(
    source_id: &str,
    max_count: usize,
    github_token: Option<&str>,
) -> Result<Vec<XxmiRemoteVersion>, String> {
    let source = known_package_sources()
        .into_iter()
        .find(|s| s.id == source_id)
        .ok_or_else(|| format!("未知的包源 ID: {}", source_id))?;

    let url = format!(
        "https://api.github.com/repos/{}/releases?per_page={}",
        source.github_repo, max_count
    );

    let client = reqwest::Client::new();
    let mut req = client
        .get(&url)
        .header("User-Agent", "SSMT4/0.1")
        .header("Accept", "application/vnd.github+json");
    if let Some(token) = github_token {
        if !token.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", token));
        }
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("获取 {} 版本列表失败: {}", source.display_name, e))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub API 返回 HTTP {}: {}", resp.status(), url));
    }

    let releases: Vec<GhRelease> = resp
        .json()
        .await
        .map_err(|e| format!("解析 GitHub releases JSON 失败: {}", e))?;

    // 获取本地已安装版本
    let local = scan_local_xxmi_packages();
    let local_versions: std::collections::HashSet<String> = local
        .packages
        .iter()
        .filter(|p| p.source_id == source_id)
        .map(|p| p.version.clone())
        .collect();

    let mut versions = Vec::new();
    for rel in releases {
        // 找到 zip 资产（排除 Manifest.json）
        let zip_asset = rel
            .assets
            .iter()
            .find(|a| a.name.starts_with(&source.asset_prefix) && a.name.ends_with(".zip"));

        if let Some(asset) = zip_asset {
            let installed = local_versions.contains(&rel.tag_name);
            versions.push(XxmiRemoteVersion {
                source_id: source.id.clone(),
                source_name: source.display_name.clone(),
                version: rel.tag_name.clone(),
                tag: rel.tag_name.clone(),
                published_at: rel.published_at.unwrap_or_default(),
                download_url: asset.browser_download_url.clone(),
                asset_name: asset.name.clone(),
                asset_size: asset.size,
                installed,
                body: rel.body.unwrap_or_default(),
            });
        }
    }

    info!(
        "获取到 {} 个 {} 远程版本",
        versions.len(),
        source.display_name
    );
    Ok(versions)
}

// ============================================================
// 下载并解压 XXMI 包
// ============================================================

pub async fn download_xxmi_package(
    source_id: &str,
    version: &str,
    download_url: &str,
) -> Result<String, String> {
    let source = known_package_sources()
        .into_iter()
        .find(|s| s.id == source_id)
        .ok_or_else(|| format!("未知的包源 ID: {}", source_id))?;

    let cache_dir = packages_cache_dir().join(&source.id);
    std::fs::create_dir_all(&cache_dir).map_err(|e| format!("创建包缓存目录失败: {}", e))?;

    let zip_name = format!("{}-{}.zip", source.asset_prefix, version);
    let zip_path = cache_dir.join(&zip_name);
    let extract_dir = cache_dir.join(version);

    // 如果已解压，直接返回
    if extract_dir.exists() {
        return Ok(format!(
            "{} {} 已存在，无需重复下载",
            source.display_name, version
        ));
    }

    // 下载 zip
    if !zip_path.exists() {
        info!(
            "正在下载 {} {} 从 {}",
            source.display_name, version, download_url
        );
        download_file(download_url, &zip_path).await?;
    }

    // 解压 zip
    info!(
        "正在解压 {} {} 到 {}",
        source.display_name,
        version,
        extract_dir.display()
    );
    extract_zip(&zip_path, &extract_dir)?;

    // 解压成功后删除 zip 文件（节省空间）
    if let Err(e) = std::fs::remove_file(&zip_path) {
        warn!("清理 zip 文件失败（非致命）: {}", e);
    }

    Ok(format!("{} {} 下载完成", source.display_name, version))
}

/// 将已下载的包部署到指定的 importer 目录
pub fn deploy_xxmi_package(
    source_id: &str,
    version: &str,
    target_dir: &str,
) -> Result<String, String> {
    let cache_dir = packages_cache_dir().join(source_id);
    let extract_dir = cache_dir.join(version);

    if !extract_dir.exists() {
        return Err(format!("包 {} {} 尚未下载，请先下载", source_id, version));
    }

    let target = Path::new(target_dir);
    std::fs::create_dir_all(target).map_err(|e| format!("创建目标目录失败: {}", e))?;

    // 递归复制解压目录的内容到目标
    copy_dir_contents(&extract_dir, target)?;

    info!("已部署包 {} {} 到 {}", source_id, version, target_dir);
    Ok(format!("部署完成: {} -> {}", version, target_dir))
}

/// 删除本地已下载的包
pub fn delete_local_xxmi_package(source_id: &str, version: &str) -> Result<String, String> {
    let cache_dir = packages_cache_dir().join(source_id);
    let extract_dir = cache_dir.join(version);
    let zip_name_candidates = vec![format!("{}.zip", version)];

    if extract_dir.exists() {
        std::fs::remove_dir_all(&extract_dir).map_err(|e| format!("删除包目录失败: {}", e))?;
    }

    // 也尝试删除 zip
    for name in zip_name_candidates {
        let zip_path = cache_dir.join(&name);
        if zip_path.exists() {
            std::fs::remove_file(&zip_path).ok();
        }
    }

    Ok(format!("已删除 {} {}", source_id, version))
}

// ============================================================
// 工具函数
// ============================================================

async fn download_file(url: &str, dest: &Path) -> Result<(), String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .header("User-Agent", "SSMT4/0.1")
        .send()
        .await
        .map_err(|e| format!("下载失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}: {}", resp.status(), url));
    }

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;

    // 最小大小校验
    if bytes.len() < 1000 {
        return Err(format!(
            "下载文件异常（仅 {} 字节），疑似错误页面: {}",
            bytes.len(),
            url
        ));
    }

    // zip 魔数校验
    if bytes.len() >= 4 && &bytes[..4] != b"PK\x03\x04" {
        return Err(format!("下载的文件不是有效 ZIP 格式: {}", url));
    }

    std::fs::write(dest, &bytes).map_err(|e| format!("写入文件失败: {}", e))?;

    info!("下载完成: {} ({} bytes)", dest.display(), bytes.len());
    Ok(())
}

fn extract_zip(zip_path: &Path, dest: &Path) -> Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|e| format!("打开 ZIP 失败: {}", e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("解析 ZIP 失败: {}", e))?;

    std::fs::create_dir_all(dest).map_err(|e| format!("创建解压目录失败: {}", e))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("读取 ZIP 条目失败: {}", e))?;

        let name = entry.name().to_string();
        // 安全检查：防止路径穿越
        if name.contains("..") {
            continue;
        }

        let out_path = dest.join(&name);

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path).ok();
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let mut outfile = std::fs::File::create(&out_path)
                .map_err(|e| format!("创建文件失败 {}: {}", name, e))?;
            std::io::copy(&mut entry, &mut outfile)
                .map_err(|e| format!("解压文件失败 {}: {}", name, e))?;
        }
    }

    Ok(())
}

fn copy_dir_contents(src: &Path, dst: &Path) -> Result<(), String> {
    for entry in walkdir::WalkDir::new(src)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let rel = entry
            .path()
            .strip_prefix(src)
            .map_err(|e| format!("路径处理失败: {}", e))?;
        let target = dst.join(rel);

        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target).ok();
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            std::fs::copy(entry.path(), &target)
                .map_err(|e| format!("复制文件失败 {}: {}", rel.display(), e))?;
        }
    }
    Ok(())
}
