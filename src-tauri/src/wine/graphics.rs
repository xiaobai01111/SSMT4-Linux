use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tracing::{info, warn};

/// 缓存上次成功获取的远程版本列表（按 variant 分别缓存）
static DXVK_VARIANT_CACHE: std::sync::OnceLock<Mutex<HashMap<String, Vec<DxvkRemoteVersion>>>> =
    std::sync::OnceLock::new();

fn get_variant_cache() -> &'static Mutex<HashMap<String, Vec<DxvkRemoteVersion>>> {
    DXVK_VARIANT_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

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
    /// 变体标识（由 Data-parameters/dxvk/*.json 定义）
    pub variant: String,
    /// 是否已解压（可直接安装）
    pub extracted: bool,
    /// 缓存目录路径
    pub path: PathBuf,
}

/// 远程可用的 DXVK 版本（GitHub Release）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DxvkRemoteVersion {
    pub version: String,
    /// 变体标识（由 Data-parameters/dxvk/*.json 定义）
    pub variant: String,
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

/// 本地已缓存的 VKD3D-Proton 版本信息
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Vkd3dLocalVersion {
    pub version: String,
    /// 是否已解压（可直接安装）
    pub extracted: bool,
    /// 缓存目录路径
    pub path: PathBuf,
}

/// 远程可用的 VKD3D-Proton 版本（GitHub Release）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Vkd3dRemoteVersion {
    pub version: String,
    pub tag_name: String,
    pub download_url: String,
    pub file_size: u64,
    pub published_at: String,
    /// 是否已在本地缓存
    pub is_local: bool,
}

/// 当前 Prefix 中安装的 VKD3D-Proton 状态
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Vkd3dInstalledStatus {
    /// 是否检测到 VKD3D DLL
    pub installed: bool,
    /// 匹配到的版本号（优先标记文件）
    pub version: Option<String>,
    /// 检测到的 VKD3D DLL 列表
    pub dlls_found: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DxvkCatalogSeed {
    #[serde(default = "default_dxvk_catalog_schema")]
    schema_version: u32,
    #[serde(default)]
    variants: Vec<DxvkVariantSource>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DxvkVariantSource {
    id: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    provider: String,
    #[serde(default)]
    repo: String,
    #[serde(default)]
    endpoint: String,
    #[serde(default)]
    asset_pattern: String,
    #[serde(default)]
    download_url_template: String,
    #[serde(default)]
    archive_name_template: String,
    #[serde(default)]
    include_prerelease: bool,
    #[serde(default = "default_dxvk_variant_enabled")]
    enabled: bool,
    #[serde(default)]
    note: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DxvkVariantIndex {
    #[serde(default = "default_dxvk_catalog_schema")]
    schema_version: u32,
    #[serde(default)]
    variants: Vec<DxvkVariantIndexItem>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DxvkVariantIndexItem {
    id: String,
    #[serde(default)]
    path: String,
    #[serde(default = "default_dxvk_variant_enabled")]
    enabled: bool,
}

fn default_dxvk_catalog_schema() -> u32 {
    1
}

fn default_dxvk_variant_enabled() -> bool {
    true
}

fn default_dxvk_variants() -> Vec<DxvkVariantSource> {
    vec![
        DxvkVariantSource {
            id: "dxvk".to_string(),
            display_name: "DXVK".to_string(),
            provider: "github_releases".to_string(),
            repo: "doitsujin/dxvk".to_string(),
            endpoint: String::new(),
            asset_pattern: "(?i)^dxvk-.*\\.tar\\.gz$".to_string(),
            download_url_template:
                "https://github.com/doitsujin/dxvk/releases/download/v{version}/{archive}"
                    .to_string(),
            archive_name_template: "dxvk-{version}.tar.gz".to_string(),
            include_prerelease: false,
            enabled: true,
            note: "fallback".to_string(),
        },
        DxvkVariantSource {
            id: "gplasync".to_string(),
            display_name: "DXVK-GPLAsync".to_string(),
            provider: "gitlab_tree".to_string(),
            repo: "Ph42oN/dxvk-gplasync".to_string(),
            endpoint:
                "https://gitlab.com/api/v4/projects/Ph42oN%2Fdxvk-gplasync/repository/tree?path=releases&per_page={limit}"
                    .to_string(),
            asset_pattern: "(?i)^dxvk-gplasync-.*\\.tar\\.gz$".to_string(),
            download_url_template:
                "https://gitlab.com/Ph42oN/dxvk-gplasync/-/raw/main/releases/{archive}"
                    .to_string(),
            archive_name_template: "dxvk-gplasync-v{version}.tar.gz".to_string(),
            include_prerelease: false,
            enabled: true,
            note: "fallback".to_string(),
        },
    ]
}

fn load_dxvk_variants_from_modules() -> Result<Vec<DxvkVariantSource>, String> {
    let index_raw = crate::utils::data_parameters::read_data_json("dxvk/index.json")?;
    let index = serde_json::from_str::<DxvkVariantIndex>(&index_raw)
        .map_err(|e| format!("解析 dxvk/index.json 失败: {}", e))?;

    let mut result = Vec::new();
    for item in index.variants {
        if !item.enabled {
            continue;
        }
        let id = item.id.trim();
        if id.is_empty() {
            continue;
        }
        let file = if item.path.trim().is_empty() {
            format!("{}.json", id)
        } else {
            item.path.trim().to_string()
        };
        let relative = format!("dxvk/{}", file.trim_start_matches(['/', '\\']));
        let raw = crate::utils::data_parameters::read_data_json(&relative)
            .map_err(|e| format!("读取 {} 失败: {}", relative, e))?;
        let mut variant = serde_json::from_str::<DxvkVariantSource>(&raw)
            .map_err(|e| format!("解析 {} 失败: {}", relative, e))?;
        if variant.id.trim().is_empty() {
            variant.id = id.to_string();
        }
        result.push(variant);
    }

    if result.is_empty() {
        return Err("dxvk/index.json 中没有可用 variants".to_string());
    }

    info!(
        "DXVK 模块化配置已加载: schema={}, variants={}",
        index.schema_version,
        result.len()
    );
    Ok(result)
}

static DXVK_VARIANTS: std::sync::OnceLock<Vec<DxvkVariantSource>> = std::sync::OnceLock::new();

fn load_dxvk_variants() -> &'static Vec<DxvkVariantSource> {
    DXVK_VARIANTS.get_or_init(|| {
        match load_dxvk_variants_from_modules() {
            Ok(variants) => return variants,
            Err(e) => {
                warn!("读取模块化 DXVK 配置失败，回退 catalog: {}", e);
            }
        }

        let json = match crate::utils::data_parameters::read_catalog_json("dxvk_catalog.json") {
            Ok(content) => content,
            Err(e) => {
                warn!("读取 DXVK catalog 失败，回退内置默认值: {}", e);
                return default_dxvk_variants();
            }
        };
        match serde_json::from_str::<DxvkCatalogSeed>(&json) {
            Ok(seed) => {
                info!(
                    "DXVK catalog 已加载: schema={}, variants={}",
                    seed.schema_version,
                    seed.variants.len()
                );
                if seed.variants.is_empty() {
                    default_dxvk_variants()
                } else {
                    seed.variants
                }
            }
            Err(e) => {
                warn!("解析 DXVK catalog 失败，回退内置默认值: {}", e);
                default_dxvk_variants()
            }
        }
    })
}

fn find_dxvk_variant(variant: &str) -> Option<DxvkVariantSource> {
    let key = variant.trim();
    if key.is_empty() {
        return None;
    }
    load_dxvk_variants()
        .iter()
        .find(|item| item.enabled && item.id.eq_ignore_ascii_case(key))
        .cloned()
}

fn variant_cache_key(variant: &str) -> String {
    variant.trim().to_ascii_lowercase()
}

fn get_cached_variant_versions(variant: &str) -> Vec<DxvkRemoteVersion> {
    let key = variant_cache_key(variant);
    if key.is_empty() {
        return Vec::new();
    }
    match get_variant_cache().lock() {
        Ok(cache) => cache.get(&key).cloned().unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

fn set_cached_variant_versions(variant: &str, versions: Vec<DxvkRemoteVersion>) {
    let key = variant_cache_key(variant);
    if key.is_empty() {
        return;
    }
    if let Ok(mut cache) = get_variant_cache().lock() {
        cache.insert(key, versions);
    }
}

fn render_source_template(template: &str, version: &str, archive: &str, limit: usize) -> String {
    template
        .replace("{version}", version)
        .replace("{archive}", archive)
        .replace("{limit}", &limit.to_string())
}

fn matches_asset_pattern(name: &str, pattern: &str) -> bool {
    let p = pattern.trim();
    if p.is_empty() {
        return name.ends_with(".tar.gz") && name.contains("dxvk");
    }
    if let Ok(re) = Regex::new(p) {
        return re.is_match(name);
    }
    name.contains(p)
}

fn extract_version_by_template(name: &str, archive_name_template: &str) -> Option<String> {
    let template = archive_name_template.trim();
    if template.is_empty() || !template.contains("{version}") {
        return None;
    }
    let mut parts = template.split("{version}");
    let prefix = parts.next().unwrap_or_default();
    let suffix = parts.next().unwrap_or_default();
    if parts.next().is_some() {
        return None;
    }
    if !name.starts_with(prefix) || !name.ends_with(suffix) {
        return None;
    }
    let start = prefix.len();
    let end = name.len().saturating_sub(suffix.len());
    if end <= start {
        return None;
    }
    let version = &name[start..end];
    if version.trim().is_empty() {
        None
    } else {
        Some(version.to_string())
    }
}

fn strip_archive_suffix(value: &str) -> &str {
    for suffix in [".tar.gz", ".tar.xz", ".tar.zst", ".zip"] {
        if let Some(stripped) = value.strip_suffix(suffix) {
            return stripped;
        }
    }
    value
}

fn is_supported_archive_name(name: &str) -> bool {
    [".tar.gz", ".tar.xz", ".tar.zst", ".zip"]
        .iter()
        .any(|suffix| name.ends_with(suffix))
}

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

    // 兼容历史命名规则
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

/// 扫描本地缓存的 DXVK 版本（tools/dxvk/ 目录）
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
            // 同一版本同时存在压缩包与已解压目录时，优先保留已解压条目
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
        for entry in entries.filter_map(|e| e.ok()) {
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

/// 检测 Prefix 中已安装的 DXVK 版本
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

    // 兼容历史安装：即便 DLL 检测失败，只要存在版本标记也视为已安装
    let installed = !found_dlls.is_empty() || marker_version.is_some();

    // 多级版本检测：标记文件 → 二进制搜索 → 文件大小比对
    let version = if installed {
        let v = marker_version.clone();
        if v.is_some() {
            info!("[DXVK] 版本来源: 标记文件 → {:?}", v);
            v
        } else {
            let v = extract_dxvk_version_from_dirs(&[system32.clone(), syswow64.clone()]);
            if v.is_some() {
                info!("[DXVK] 版本来源: DLL 二进制搜索 → {:?}", v);
                v
            } else {
                let v = match_dxvk_version_by_size(&[system32, syswow64]);
                if v.is_some() {
                    info!("[DXVK] 版本来源: 文件大小比对 → {:?}", v);
                } else {
                    warn!(
                        "[DXVK] 三层版本检测均失败（标记文件/二进制搜索/大小比对）prefix={}",
                        prefix_path.display()
                    );
                }
                v
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
    // 首选二进制版本特征，最可靠
    if extract_version_from_binary(path).is_some() {
        return true;
    }
    // 回退大小特征：降低阈值，避免误判“已安装但不识别”
    std::fs::metadata(path)
        .map(|m| m.len() >= 120_000)
        .unwrap_or(false)
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

/// 获取 VKD3D 版本标记文件路径
fn vkd3d_version_marker_path(prefix_path: &Path) -> PathBuf {
    prefix_path.join(".vkd3d-version")
}

/// 读取安装时写入的 .vkd3d-version 标记文件
fn read_vkd3d_version_marker(prefix_path: &Path) -> Option<String> {
    let marker = vkd3d_version_marker_path(prefix_path);
    std::fs::read_to_string(&marker)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// 写入 .vkd3d-version 标记文件
fn write_vkd3d_version_marker(prefix_path: &Path, version: &str) {
    let marker = vkd3d_version_marker_path(prefix_path);
    let _ = std::fs::write(&marker, version);
}

/// 删除 .vkd3d-version 标记文件
fn remove_vkd3d_version_marker(prefix_path: &Path) {
    let marker = vkd3d_version_marker_path(prefix_path);
    let _ = std::fs::remove_file(&marker);
}

fn parse_vkd3d_version_from_name(name: &str) -> Option<String> {
    strip_archive_suffix(name)
        .strip_prefix("vkd3d-proton-")
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// 扫描本地缓存的 VKD3D-Proton 版本（tools/vkd3d/ 目录）
pub fn scan_local_vkd3d_versions() -> Vec<Vkd3dLocalVersion> {
    let cache_dir = crate::utils::file_manager::get_tools_dir().join("vkd3d");
    if !cache_dir.exists() {
        return Vec::new();
    }

    let mut by_version: HashMap<String, Vkd3dLocalVersion> = HashMap::new();
    let mut upsert = |version: String, extracted: bool, path: PathBuf| {
        match by_version.get_mut(&version) {
            Some(existing) => {
                // 同一版本同时存在压缩包与已解压目录时，优先保留已解压条目
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
        }
    };

    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
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

/// 检测 Prefix 中已安装的 VKD3D-Proton 版本
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

/// 通过 DLL 文件大小与本地缓存版本比对（兜底方案）
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

/// 直接读取 DLL 二进制数据，搜索 "dxvk-X.Y.Z" 版本字符串
///
/// 不依赖外部 `strings` 命令，兼容所有 Linux 发行版。
/// DXVK DLL 中嵌入了 ASCII 版本标识（如 "dxvk-2.5.3"），
/// 扫描二进制内容匹配 `dxvk-` 前缀 + 语义版本号格式。
fn extract_dxvk_version_from_dirs(dirs: &[PathBuf]) -> Option<String> {
    for dir in dirs {
        // 按优先级尝试多个 DLL（dxgi.dll 最常见）
        for dll_name in &["dxgi.dll", "d3d11.dll", "d3d9.dll"] {
            let dll_path = dir.join(dll_name);
            if let Some(ver) = extract_version_from_binary(&dll_path) {
                return Some(ver);
            }
        }
    }
    None
}

/// 从单个 PE 二进制文件中搜索 dxvk 版本字符串
fn extract_version_from_binary(path: &Path) -> Option<String> {
    let data = std::fs::read(path).ok()?;
    let needle = b"dxvk-";

    // 滑动窗口搜索 "dxvk-" 前缀
    for i in 0..data.len().saturating_sub(needle.len()) {
        if &data[i..i + needle.len()] != needle {
            continue;
        }

        // 提取 "dxvk-" 之后的版本号部分（最多 20 字节）
        let start = i + needle.len();
        let end = (start + 20).min(data.len());
        let candidate = &data[start..end];

        // 版本号必须以数字开头
        if candidate.is_empty() || !candidate[0].is_ascii_digit() {
            continue;
        }

        // 收集有效的版本号字符：数字和点
        let ver_bytes: Vec<u8> = candidate
            .iter()
            .take_while(|&&b| b.is_ascii_digit() || b == b'.')
            .copied()
            .collect();

        let ver = String::from_utf8(ver_bytes).ok()?;

        // 校验：至少包含一个点，不以点结尾
        if ver.contains('.') && !ver.ends_with('.') && ver.len() >= 3 {
            return Some(ver);
        }
    }
    None
}

/// 通过 DLL 文件大小与本地缓存版本比对（兜底方案）
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

/// 从 GitHub releases 获取 DXVK 版本列表
async fn fetch_dxvk_from_repo(
    source: &DxvkVariantSource,
    max_count: usize,
    local_versions: &[DxvkLocalVersion],
    github_token: Option<&str>,
) -> Result<Vec<DxvkRemoteVersion>, String> {
    let repo = source.repo.trim();
    if repo.is_empty() {
        return Err(format!("DXVK source '{}' 缺少 repo", source.id));
    }

    let url = format!(
        "https://api.github.com/repos/{}/releases?per_page={}",
        repo, max_count
    );

    let client = reqwest::Client::new();
    let mut req = client
        .get(&url)
        .header("User-Agent", "SSMT4/0.1")
        .header("Accept", "application/vnd.github.v3+json");
    if let Some(token) = github_token.map(|v| v.trim()).filter(|v| !v.is_empty()) {
        req = req.bearer_auth(token);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("请求 {} 失败: {}", repo, e))?;

    if !resp.status().is_success() {
        return Err(format!("{} API 返回 HTTP {}", repo, resp.status()));
    }

    let releases: Vec<serde_json::Value> = resp
        .json()
        .await
        .map_err(|e| format!("解析 {} 响应失败: {}", repo, e))?;

    let mut result = Vec::new();
    for release in &releases {
        let prerelease = release
            .get("prerelease")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if prerelease && !source.include_prerelease {
            continue;
        }

        let tag_name = release
            .get("tag_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let published_at = release
            .get("published_at")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // 查找 tar.gz asset
        let assets = release.get("assets").and_then(|v| v.as_array());
        if let Some(assets) = assets {
            for asset in assets {
                let name = asset.get("name").and_then(|v| v.as_str()).unwrap_or("");
                if matches_asset_pattern(name, &source.asset_pattern) {
                    let download_url = asset
                        .get("browser_download_url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let file_size = asset.get("size").and_then(|v| v.as_u64()).unwrap_or(0);

                    // 从 asset 文件名提取版本号
                    let version = extract_version_from_asset(name, source).unwrap_or_else(|| {
                        tag_name
                            .strip_prefix('v')
                            .or_else(|| tag_name.strip_prefix("gplasync-v"))
                            .unwrap_or(&tag_name)
                            .to_string()
                    });

                    let is_local = local_versions
                        .iter()
                        .any(|v| v.version == version && v.variant == source.id);

                    result.push(DxvkRemoteVersion {
                        version,
                        variant: source.id.clone(),
                        tag_name: tag_name.clone(),
                        download_url,
                        file_size,
                        published_at: published_at.clone(),
                        is_local,
                    });
                    break;
                }
            }
        }
    }

    Ok(result)
}

/// 从 asset 文件名提取版本号
fn extract_version_from_asset(name: &str, source: &DxvkVariantSource) -> Option<String> {
    if let Some(v) = extract_version_by_template(name, &source.archive_name_template) {
        return Some(v);
    }

    let stem = name.strip_suffix(".tar.gz")?;
    match source.id.to_ascii_lowercase().as_str() {
        "gplasync" => {
            // dxvk-gplasync-v2.5.1 → 2.5.1
            stem.strip_prefix("dxvk-gplasync-v")
                .or_else(|| stem.strip_prefix("dxvk-gplasync-"))
                .map(|s| s.to_string())
        }
        _ => {
            // dxvk-2.5.1 → 2.5.1
            stem.strip_prefix("dxvk-").map(|s| s.to_string())
        }
    }
}

/// 从 GitLab 仓库 releases/ 目录获取 DXVK-GPLAsync 版本列表
async fn fetch_dxvk_gplasync_from_gitlab(
    source: &DxvkVariantSource,
    max_count: usize,
    local_versions: &[DxvkLocalVersion],
) -> Result<Vec<DxvkRemoteVersion>, String> {
    let endpoint = source.endpoint.trim();
    if endpoint.is_empty() {
        return Err(format!("DXVK source '{}' 缺少 endpoint", source.id));
    }

    // GitLab API: 列出 releases/ 目录下的文件
    let url = render_source_template(endpoint, "", "", max_count);

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("User-Agent", "SSMT4/0.1")
        .send()
        .await
        .map_err(|e| format!("请求 GitLab API 失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("GitLab API 返回 HTTP {}", resp.status()));
    }

    let files: Vec<serde_json::Value> = resp
        .json()
        .await
        .map_err(|e| format!("解析 GitLab 响应失败: {}", e))?;

    let mut result = Vec::new();
    for file in &files {
        let name = file.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if !matches_asset_pattern(name, &source.asset_pattern) {
            continue;
        }

        let version = match extract_version_from_asset(name, source) {
            Some(v) => v,
            None => continue,
        };

        let download_url = {
            let template = source.download_url_template.trim();
            if template.is_empty() {
                return Err(format!(
                    "DXVK source '{}' 缺少 downloadUrlTemplate",
                    source.id
                ));
            }
            render_source_template(template, &version, name, max_count)
        };

        let is_local = local_versions
            .iter()
            .any(|v| v.version == version && v.variant == source.id);

        result.push(DxvkRemoteVersion {
            version,
            variant: source.id.clone(),
            tag_name: name.to_string(),
            download_url,
            file_size: 0, // GitLab tree API 不返回文件大小
            published_at: String::new(),
            is_local,
        });
    }

    // 按版本号降序排列
    result.sort_by(|a, b| b.version.cmp(&a.version));
    if result.len() > max_count {
        result.truncate(max_count);
    }

    Ok(result)
}

pub async fn fetch_dxvk_releases(
    max_count: usize,
    github_token: Option<&str>,
) -> Result<Vec<DxvkRemoteVersion>, String> {
    let local_versions = scan_local_dxvk_versions();

    let mut all = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    let variants: Vec<DxvkVariantSource> = load_dxvk_variants()
        .iter()
        .filter(|v| v.enabled && !v.id.trim().is_empty())
        .cloned()
        .collect();

    for source in variants {
        let provider = source.provider.trim().to_ascii_lowercase();
        let display_name = if source.display_name.trim().is_empty() {
            source.id.clone()
        } else {
            source.display_name.clone()
        };
        if !source.note.trim().is_empty() {
            info!(
                "DXVK source loaded: {} ({}) - {}",
                display_name, source.provider, source.note
            );
        } else {
            info!("DXVK source loaded: {} ({})", display_name, source.provider);
        }

        let fetch_result = match provider.as_str() {
            "github_releases" => {
                fetch_dxvk_from_repo(&source, max_count, &local_versions, github_token).await
            }
            "gitlab_tree" => {
                fetch_dxvk_gplasync_from_gitlab(&source, max_count, &local_versions).await
            }
            other => Err(format!(
                "DXVK source '{}' 使用了不支持的 provider: {}",
                source.id, other
            )),
        };

        match fetch_result {
            Ok(v) => {
                set_cached_variant_versions(&source.id, v.clone());
                all.extend(v);
            }
            Err(e) => {
                warn!("获取 {} 版本失败: {}，尝试使用缓存", display_name, e);
                let cached = get_cached_variant_versions(&source.id);
                if !cached.is_empty() {
                    all.extend(cached);
                    warnings.push(format!("{}: {} (使用缓存)", display_name, e));
                    continue;
                }
                warnings.push(format!("{}: {}", display_name, e));
            }
        }
    }

    if !warnings.is_empty() {
        warn!("DXVK 版本获取警告: {:?}", warnings);
    }
    info!("获取到 {} 个 DXVK 远程版本", all.len());
    Ok(all)
}

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

pub async fn fetch_vkd3d_releases(
    max_count: usize,
    github_token: Option<&str>,
) -> Result<Vec<Vkd3dRemoteVersion>, String> {
    let local_versions = scan_local_vkd3d_versions();
    let url = format!(
        "https://api.github.com/repos/HansKristian-Work/vkd3d-proton/releases?per_page={}",
        max_count
    );

    let client = reqwest::Client::new();
    let mut req = client
        .get(&url)
        .header("User-Agent", "SSMT4/0.1")
        .header("Accept", "application/vnd.github.v3+json");
    if let Some(token) = github_token.map(|v| v.trim()).filter(|v| !v.is_empty()) {
        req = req.bearer_auth(token);
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("请求 VKD3D 远程版本失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("VKD3D API 返回 HTTP {}", resp.status()));
    }

    let releases: Vec<serde_json::Value> = resp
        .json()
        .await
        .map_err(|e| format!("解析 VKD3D 远程版本失败: {}", e))?;

    let mut result = Vec::new();
    for release in releases {
        let prerelease = release
            .get("prerelease")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if prerelease {
            continue;
        }

        let tag_name = release
            .get("tag_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let published_at = release
            .get("published_at")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let Some(assets) = release.get("assets").and_then(|v| v.as_array()) else {
            continue;
        };
        let Some(asset) = assets.iter().find(|asset| {
            let name = asset.get("name").and_then(|v| v.as_str()).unwrap_or("");
            name.starts_with("vkd3d-proton-") && name.ends_with(".tar.zst")
        }) else {
            continue;
        };

        let name = asset.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let version = parse_vkd3d_version_from_name(name).unwrap_or_else(|| {
            tag_name
                .trim_start_matches('v')
                .trim()
                .to_string()
        });
        let download_url = asset
            .get("browser_download_url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let file_size = asset.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
        let is_local = local_versions.iter().any(|v| v.version == version);

        result.push(Vkd3dRemoteVersion {
            version,
            tag_name,
            download_url,
            file_size,
            published_at,
            is_local,
        });
    }

    result.sort_by(|a, b| b.version.cmp(&a.version));
    if result.len() > max_count {
        result.truncate(max_count);
    }
    info!("获取到 {} 个 VKD3D 远程版本", result.len());
    Ok(result)
}

/// 仅下载并解压 VKD3D-Proton 到本地缓存（不安装到任何 Prefix）
pub async fn download_vkd3d_only(vkd3d_version: &str) -> Result<String, String> {
    let cache_dir = crate::utils::file_manager::get_tools_dir().join("vkd3d");
    crate::utils::file_manager::ensure_dir(&cache_dir)?;

    let (archive_name, extract_dir_name) = vkd3d_names(vkd3d_version);
    let archive_path = cache_dir.join(&archive_name);
    let extract_dir = cache_dir.join(&extract_dir_name);

    if !archive_path.exists() {
        let url = vkd3d_download_url(vkd3d_version, &archive_name);
        info!("Downloading VKD3D-Proton {} from {}", vkd3d_version, url);
        download_tool(&url, &archive_path).await?;
    }

    if !extract_dir.exists() {
        extract_tar_zst(&archive_path, &cache_dir).await?;
    }

    info!(
        "VKD3D-Proton {} 已缓存到 {}",
        vkd3d_version,
        extract_dir.display()
    );
    Ok(format!("VKD3D-Proton {} 下载完成", vkd3d_version))
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

/// 根据 variant 获取 DXVK 的 archive 名称和 extract 目录名
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

/// 根据 variant 构造 DXVK 下载 URL
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

/// 仅下载并解压 DXVK 到本地缓存（不安装到任何 Prefix）
pub async fn download_dxvk_only(dxvk_version: &str, variant: &str) -> Result<String, String> {
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
        download_tool(&url, &archive_path).await?;
    }

    if !extract_dir.exists() {
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

    // Download if not cached
    if !archive_path.exists() {
        let url = dxvk_download_url(dxvk_version, variant, &archive_name);
        info!(
            "Downloading DXVK {} ({}) from {}",
            dxvk_version, variant, url
        );
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

    if !x64_dir.exists() {
        return Err(format!("DXVK 解压目录缺少 x64/: {}", extract_dir.display()));
    }

    // 目标目录不存在时自动创建（prefix 可能尚未被 Wine 初始化）
    std::fs::create_dir_all(&system32).map_err(|e| format!("创建 system32 目录失败: {}", e))?;

    let dlls = ["d3d9.dll", "d3d10core.dll", "d3d11.dll", "dxgi.dll"];
    let mut copied: usize = 0;

    for dll in &dlls {
        let src = x64_dir.join(dll);
        if src.exists() {
            std::fs::copy(&src, system32.join(dll))
                .map_err(|e| format!("Failed to copy DXVK DLL {}: {}", dll, e))?;
            copied += 1;
        }
    }

    // 32-bit DLLs（可选，syswow64 不存在时跳过）
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

    // 写入版本标记文件
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

    // 删除版本标记文件
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
            file.write_all(&chunk)
                .await
                .map_err(|e| format!("Failed to write chunk: {}", e))?;
            downloaded += chunk.len() as u64;
        }
        file.flush()
            .await
            .map_err(|e| format!("Failed to flush file: {}", e))?;
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

    // 归档格式魔数校验（tar.gz/tar.xz/tar.zst/zip）
    let valid_archive = (header_filled >= 2 && header_buf[..2] == [0x1F, 0x8B])        // gzip
        || (header_filled >= 6 && header_buf[..6] == [0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00]) // xz
        || (header_filled >= 4 && header_buf[..4] == [0x28, 0xB5, 0x2F, 0xFD]) // zstd
        || (header_filled >= 4 && header_buf[..4] == [0x50, 0x4B, 0x03, 0x04]); // zip
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
