use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tracing::{info, warn};

// ============================================================
// Snowbreak manifest 数据结构
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub version: String,
    #[serde(rename = "projectVersion")]
    pub project_version: Option<String>,
    #[serde(rename = "pathOffset")]
    pub path_offset: String,
    pub paks: Vec<PakEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PakEntry {
    pub name: String,
    pub hash: String,
    #[serde(rename = "sizeInBytes")]
    pub size_in_bytes: u64,
    #[serde(rename = "fastVerify")]
    pub fast_verify: Option<String>,
    #[serde(rename = "bPrimary")]
    pub b_primary: Option<bool>,
}

// ============================================================
// Launcher manifest（从 GitHub 获取，包含 CDN 域名和 URL 模板）
// ============================================================

const LAUNCHER_MANIFEST_URL: &str =
    "https://leayal.github.io/SnowBreakLauncher-Dotnet/publish/v2/launcher-manifest.json";

/// 默认 CDN 域名（launcher manifest 不可用时回退）
const DEFAULT_CDN_DOMAINS: &[&str] = &[
    "snowbreak-dl.amazingseasuncdn.com",
    "snowbreak-dl-akm.amazingseasuncdn.com",
    "snowbreak-dl-cy.amazingseasuncdn.com",
];

/// 官方已知路径模板（按新版本优先）
const OFFICIAL_URL_TEMPLATES: &[&str] = &[
    "https://{0}/d600db9654fdbf29cb734c885ea5ca67/PC/updates/",
    "https://{0}/118c343979b2407f4a6b3ad2b84d6d79/PC/updates/",
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourcePolicy {
    OfficialFirst,
    CommunityFirst,
}

impl SourcePolicy {
    pub fn from_str(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "community_first" => Self::CommunityFirst,
            _ => Self::OfficialFirst,
        }
    }

    fn ordered_sources(self) -> [SourceKind; 2] {
        match self {
            Self::OfficialFirst => [SourceKind::Official, SourceKind::Community],
            Self::CommunityFirst => [SourceKind::Community, SourceKind::Official],
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum SourceKind {
    Official,
    Community,
}

impl SourceKind {
    fn label(self) -> &'static str {
        match self {
            Self::Official => "official",
            Self::Community => "community",
        }
    }
}

/// 运行时解析出的 CDN base URL 列表（例如 https://domain/hash/PC/updates/）
pub struct ResolvedCdn {
    pub base_urls: Vec<String>,
}

impl ResolvedCdn {
    /// 构建 manifest.json 完整 URL
    pub fn manifest_url(&self, idx: usize) -> String {
        format!("{}manifest.json", self.base_urls[idx])
    }

    /// 构建文件下载 URL
    pub fn file_url(&self, idx: usize, path_offset: &str, file_hash: &str) -> String {
        format!("{}{}/{}", self.base_urls[idx], path_offset, file_hash)
    }

    pub fn len(&self) -> usize {
        self.base_urls.len()
    }
}

/// 从 GitHub launcher-manifest.json 解析 CDN base URL 列表
async fn resolve_cdn_from_community_manifest(client: &Client) -> Result<ResolvedCdn, String> {
    info!(
        "[Snowbreak] 获取 launcher manifest: {}",
        LAUNCHER_MANIFEST_URL
    );

    let resp = client
        .get(LAUNCHER_MANIFEST_URL)
        .send()
        .await
        .map_err(|e| format!("获取 launcher manifest 失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("launcher manifest HTTP {}", resp.status()));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析 launcher manifest 失败: {}", e))?;

    // 获取域名列表
    let domains: Vec<String> = data
        .get("domains")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_else(|| DEFAULT_CDN_DOMAINS.iter().map(|s| s.to_string()).collect());

    // 获取 URL 模板（default 字段，格式如 "https://{0}/hash/PC/updates/"）
    let url_template = data
        .get("default")
        .and_then(|v| v.as_str())
        .unwrap_or("https://{0}/d600db9654fdbf29cb734c885ea5ca67/PC/updates/");

    // 用域名替换 {0} 生成完整 base URL 列表
    let base_urls: Vec<String> = domains
        .iter()
        .map(|domain| {
            let url = url_template.replace("{0}", domain);
            if url.ends_with('/') {
                url
            } else {
                format!("{}/", url)
            }
        })
        .collect();

    info!(
        "[Snowbreak] 解析到 {} 个 CDN: {:?}",
        base_urls.len(),
        base_urls
    );

    Ok(ResolvedCdn { base_urls })
}

/// 使用内置官方域名和模板构建 CDN 列表
fn resolve_cdn_from_official_defaults() -> ResolvedCdn {
    let mut base_urls = Vec::new();
    let mut seen = HashSet::new();

    for template in OFFICIAL_URL_TEMPLATES {
        for domain in DEFAULT_CDN_DOMAINS {
            let url = template.replace("{0}", domain);
            let normalized = if url.ends_with('/') {
                url
            } else {
                format!("{}/", url)
            };
            if seen.insert(normalized.clone()) {
                base_urls.push(normalized);
            }
        }
    }

    info!(
        "[Snowbreak] 使用官方内置 CDN 配置，数量: {}",
        base_urls.len()
    );
    ResolvedCdn { base_urls }
}

async fn fetch_manifest_from_cdn(
    client: &Client,
    cdn: &ResolvedCdn,
    source: SourceKind,
) -> Result<Manifest, String> {
    let mut last_err = String::new();
    for idx in 0..cdn.len() {
        let url = cdn.manifest_url(idx);
        info!(
            "[Snowbreak] [{}] 尝试获取 manifest: {}",
            source.label(),
            url
        );

        match client.get(&url).send().await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    last_err = format!("HTTP {} from {}", resp.status(), url);
                    warn!("[Snowbreak] {}", last_err);
                    continue;
                }
                match resp.json::<Manifest>().await {
                    Ok(manifest) => {
                        info!(
                            "[Snowbreak] [{}] manifest 获取成功: version={}, paks={}",
                            source.label(),
                            manifest.version,
                            manifest.paks.len(),
                        );
                        return Ok(manifest);
                    }
                    Err(e) => {
                        last_err = format!("解析 manifest 失败: {}", e);
                        warn!("[Snowbreak] {}", last_err);
                        continue;
                    }
                }
            }
            Err(e) => {
                last_err = format!("请求失败: {}", e);
                warn!("[Snowbreak] {}", last_err);
                continue;
            }
        }
    }
    Err(format!(
        "[{}] 所有 CDN 均不可用，最后错误: {}",
        source.label(),
        last_err
    ))
}

// ============================================================
// API 请求
// ============================================================

/// 从多个 CDN 轮询获取 manifest，返回 (manifest, 解析后的CDN信息)
pub async fn fetch_manifest_with_policy(
    policy: SourcePolicy,
) -> Result<(Manifest, ResolvedCdn), String> {
    let client = Client::builder()
        .user_agent("Mozilla/5.0")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let mut errors = Vec::new();
    for source in policy.ordered_sources() {
        let cdn_result = match source {
            SourceKind::Official => Ok(resolve_cdn_from_official_defaults()),
            SourceKind::Community => resolve_cdn_from_community_manifest(&client).await,
        };

        let cdn = match cdn_result {
            Ok(cdn) => cdn,
            Err(e) => {
                warn!("[Snowbreak] [{}] 解析 CDN 失败: {}", source.label(), e);
                errors.push(format!("[{}] {}", source.label(), e));
                continue;
            }
        };

        match fetch_manifest_from_cdn(&client, &cdn, source).await {
            Ok(manifest) => return Ok((manifest, cdn)),
            Err(e) => {
                errors.push(e);
                continue;
            }
        }
    }

    Err(format!("所有来源均不可用: {}", errors.join(" | ")))
}

pub async fn fetch_manifest() -> Result<(Manifest, ResolvedCdn), String> {
    fetch_manifest_with_policy(SourcePolicy::OfficialFirst).await
}

/// 判断 URL 是否属于 Snowbreak
pub fn is_snowbreak_api(url: &str) -> bool {
    url.contains("amazingseasuncdn.com") || url.contains("snowbreak")
}

/// 读取本地 manifest 版本
pub fn read_local_version(game_folder: &std::path::Path) -> Option<String> {
    let manifest_path = game_folder.join("manifest.json");
    if manifest_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&manifest_path) {
            if let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&content) {
                return manifest
                    .get("version")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }
        }
    }
    None
}

/// 保存本地 manifest（下载/更新完成后保存）
pub fn save_local_manifest(
    game_folder: &std::path::Path,
    manifest: &Manifest,
) -> Result<(), String> {
    let path = game_folder.join("manifest.json");
    let content = serde_json::to_string_pretty(manifest)
        .map_err(|e| format!("序列化 manifest 失败: {}", e))?;
    std::fs::write(&path, content).map_err(|e| format!("写入 manifest 失败: {}", e))?;
    Ok(())
}
