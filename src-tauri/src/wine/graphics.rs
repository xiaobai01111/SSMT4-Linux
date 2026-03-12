use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tracing::{info, warn};

#[path = "graphics/install.rs"]
mod install;
#[path = "graphics/local.rs"]
mod local;
#[path = "graphics/remote.rs"]
mod remote;

pub use install::{
    delete_local_dxvk_version, delete_local_vkd3d_version, download_dxvk_only, download_vkd3d_only,
    install_dxvk, install_vkd3d, uninstall_dxvk, uninstall_vkd3d,
};
pub use local::{
    check_vulkan, detect_installed_dxvk, detect_installed_vkd3d, scan_local_dxvk_versions,
    scan_local_vkd3d_versions,
};
pub use remote::{fetch_dxvk_releases, fetch_vkd3d_releases};

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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DxvkLocalVersion {
    pub version: String,
    pub variant: String,
    pub extracted: bool,
    pub path: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DxvkRemoteVersion {
    pub version: String,
    pub variant: String,
    pub tag_name: String,
    pub download_url: String,
    pub file_size: u64,
    pub published_at: String,
    pub is_local: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DxvkInstalledStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub dlls_found: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Vkd3dLocalVersion {
    pub version: String,
    pub extracted: bool,
    pub path: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Vkd3dRemoteVersion {
    pub version: String,
    pub tag_name: String,
    pub download_url: String,
    pub file_size: u64,
    pub published_at: String,
    pub is_local: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Vkd3dInstalledStatus {
    pub installed: bool,
    pub version: Option<String>,
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

fn dxvk_version_marker_path(prefix_path: &Path) -> PathBuf {
    prefix_path.join(".dxvk-version")
}

fn read_dxvk_version_marker(prefix_path: &Path) -> Option<String> {
    let marker = dxvk_version_marker_path(prefix_path);
    std::fs::read_to_string(&marker)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn write_dxvk_version_marker(prefix_path: &Path, version: &str) {
    let marker = dxvk_version_marker_path(prefix_path);
    let _ = std::fs::write(&marker, version);
}

fn remove_dxvk_version_marker(prefix_path: &Path) {
    let marker = dxvk_version_marker_path(prefix_path);
    let _ = std::fs::remove_file(&marker);
}

fn vkd3d_version_marker_path(prefix_path: &Path) -> PathBuf {
    prefix_path.join(".vkd3d-version")
}

fn read_vkd3d_version_marker(prefix_path: &Path) -> Option<String> {
    let marker = vkd3d_version_marker_path(prefix_path);
    std::fs::read_to_string(&marker)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn write_vkd3d_version_marker(prefix_path: &Path, version: &str) {
    let marker = vkd3d_version_marker_path(prefix_path);
    let _ = std::fs::write(&marker, version);
}

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
        if names.iter().any(|name| file_name == *name) {
            return Some(entry.path().to_path_buf());
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

    #[test]
    fn render_source_template_replaces_all_placeholders() {
        let rendered = render_source_template(
            "https://example.com/{version}/{archive}?limit={limit}",
            "2.4",
            "dxvk-2.4.tar.gz",
            25,
        );

        assert_eq!(rendered, "https://example.com/2.4/dxvk-2.4.tar.gz?limit=25");
    }

    #[test]
    fn matches_asset_pattern_supports_regex_empty_and_substring_modes() {
        assert!(matches_asset_pattern(
            "dxvk-2.4.tar.gz",
            "(?i)^dxvk-.*\\.tar\\.gz$"
        ));
        assert!(matches_asset_pattern("dxvk-gplasync-2.4.tar.gz", ""));
        assert!(matches_asset_pattern(
            "custom-dxvk-build.tar.gz",
            "dxvk-build"
        ));
        assert!(!matches_asset_pattern("vkd3d-proton-2.0.tar.zst", ""));
    }

    #[test]
    fn extract_version_template_and_strip_archive_suffix_work_together() {
        assert_eq!(
            extract_version_by_template(
                "dxvk-gplasync-v2.4.tar.gz",
                "dxvk-gplasync-v{version}.tar.gz"
            )
            .as_deref(),
            Some("2.4")
        );
        assert_eq!(
            strip_archive_suffix("vkd3d-proton-2.8.tar.zst"),
            "vkd3d-proton-2.8"
        );
        assert_eq!(
            parse_vkd3d_version_from_name("vkd3d-proton-2.8.tar.zst").as_deref(),
            Some("2.8")
        );
    }

    #[test]
    fn dxvk_and_vkd3d_marker_roundtrip() {
        let prefix_dir = unique_temp_dir("ssmt4-graphics-marker");
        std::fs::create_dir_all(&prefix_dir).unwrap();

        write_dxvk_version_marker(&prefix_dir, "2.4");
        write_vkd3d_version_marker(&prefix_dir, "2.8");

        assert_eq!(
            read_dxvk_version_marker(&prefix_dir).as_deref(),
            Some("2.4")
        );
        assert_eq!(
            read_vkd3d_version_marker(&prefix_dir).as_deref(),
            Some("2.8")
        );

        remove_dxvk_version_marker(&prefix_dir);
        remove_vkd3d_version_marker(&prefix_dir);

        assert_eq!(read_dxvk_version_marker(&prefix_dir), None);
        assert_eq!(read_vkd3d_version_marker(&prefix_dir), None);

        let _ = std::fs::remove_dir_all(prefix_dir);
    }

    #[test]
    fn find_arch_dir_returns_nested_matching_directory() {
        let root = unique_temp_dir("ssmt4-graphics-arch");
        let nested = root.join("release").join("bin").join("x64");
        std::fs::create_dir_all(&nested).unwrap();

        assert_eq!(find_arch_dir(&root, &["x64", "x32"]), Some(nested.clone()));

        let _ = std::fs::remove_dir_all(root);
    }
}
