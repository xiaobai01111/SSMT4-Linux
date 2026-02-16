use crate::configs::{database, wine_config::WineVersion};
use crate::wine::detector;
use futures_util::future::join_all;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtonFamily {
    pub family_key: String,
    pub display_name: String,
    pub enabled: bool,
    pub sort_order: i64,
    #[serde(default)]
    pub detect_patterns: Vec<String>,
    #[serde(default)]
    pub builtin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtonSource {
    pub id: Option<i64>,
    pub family_key: String,
    pub provider: String,
    pub repo: String,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub url_template: String,
    #[serde(default = "default_asset_index")]
    pub asset_index: i64,
    pub asset_pattern: String,
    pub tag_pattern: String,
    pub max_count: i64,
    #[serde(default)]
    pub include_prerelease: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtonCatalog {
    pub families: Vec<ProtonFamily>,
    pub sources: Vec<ProtonSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtonLocalVersionItem {
    pub id: String,
    pub name: String,
    pub variant: String,
    pub path: String,
    pub version: String,
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtonRemoteVersionItem {
    pub tag: String,
    pub version: String,
    pub variant: String,
    pub download_url: String,
    pub file_size: u64,
    pub published_at: String,
    pub installed: bool,
    pub source_repo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtonFamilyLocalGroup {
    pub family_key: String,
    pub display_name: String,
    pub items: Vec<ProtonLocalVersionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtonFamilyRemoteGroup {
    pub family_key: String,
    pub display_name: String,
    pub items: Vec<ProtonRemoteVersionItem>,
}

fn default_true() -> bool {
    true
}

fn default_asset_index() -> i64 {
    -1
}

pub fn load_catalog_from_db() -> Result<ProtonCatalog, String> {
    let mut families = Vec::new();
    for row in database::list_proton_family_rows() {
        let patterns =
            serde_json::from_str::<Vec<String>>(&row.detect_patterns_json).unwrap_or_default();
        families.push(ProtonFamily {
            family_key: row.family_key,
            display_name: row.display_name,
            enabled: row.enabled,
            sort_order: row.sort_order,
            detect_patterns: patterns,
            builtin: row.builtin,
        });
    }

    let mut sources = Vec::new();
    for row in database::list_proton_source_rows() {
        sources.push(ProtonSource {
            id: row.id,
            family_key: row.family_key,
            provider: row.provider,
            repo: row.repo,
            endpoint: row.endpoint,
            url_template: row.url_template,
            asset_index: row.asset_index,
            asset_pattern: row.asset_pattern,
            tag_pattern: row.tag_pattern,
            max_count: row.max_count,
            include_prerelease: row.include_prerelease,
            enabled: row.enabled,
            note: row.note,
        });
    }

    families.sort_by(|a, b| {
        a.sort_order
            .cmp(&b.sort_order)
            .then_with(|| a.family_key.cmp(&b.family_key))
    });

    Ok(ProtonCatalog { families, sources })
}

pub fn save_catalog_to_db(catalog: &ProtonCatalog) -> Result<(), String> {
    validate_catalog(catalog)?;

    let family_rows = catalog
        .families
        .iter()
        .map(|f| {
            let detect_patterns_json = serde_json::to_string(&f.detect_patterns)
                .map_err(|e| format!("序列化 detect_patterns 失败 ({}): {}", f.family_key, e))?;
            Ok(database::ProtonFamilyRecord {
                family_key: f.family_key.trim().to_string(),
                display_name: f.display_name.trim().to_string(),
                enabled: f.enabled,
                sort_order: f.sort_order,
                detect_patterns_json,
                builtin: f.builtin,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let source_rows = catalog
        .sources
        .iter()
        .map(|s| database::ProtonSourceRecord {
            id: s.id,
            family_key: s.family_key.trim().to_string(),
            provider: s.provider.trim().to_string(),
            repo: s.repo.trim().to_string(),
            endpoint: s.endpoint.trim().to_string(),
            url_template: s.url_template.trim().to_string(),
            asset_index: s.asset_index,
            asset_pattern: s.asset_pattern.trim().to_string(),
            tag_pattern: s.tag_pattern.trim().to_string(),
            max_count: s.max_count,
            include_prerelease: s.include_prerelease,
            enabled: s.enabled,
            note: s.note.trim().to_string(),
        })
        .collect::<Vec<_>>();

    database::replace_proton_catalog_rows(&family_rows, &source_rows)
}

fn validate_catalog(catalog: &ProtonCatalog) -> Result<(), String> {
    if catalog.families.is_empty() {
        return Err("Proton 家族列表不能为空".to_string());
    }

    let mut family_keys = HashSet::new();
    for family in &catalog.families {
        let key = family.family_key.trim();
        if key.is_empty() {
            return Err("family_key 不能为空".to_string());
        }
        if !is_valid_family_key(key) {
            return Err(format!("family_key 非法: {}", key));
        }
        if !family_keys.insert(key.to_lowercase()) {
            return Err(format!("family_key 重复: {}", key));
        }
        if family.display_name.trim().is_empty() {
            return Err(format!("display_name 不能为空: {}", key));
        }
    }

    for source in &catalog.sources {
        let family_key = source.family_key.trim();
        if !family_keys.contains(&family_key.to_lowercase()) {
            return Err(format!("source.family_key 不存在: {}", family_key));
        }
        let provider = source.provider.trim();
        if !matches!(
            provider,
            "github_releases" | "forgejo_releases" | "github_actions"
        ) {
            return Err(format!("暂不支持 provider: {}", source.provider));
        }

        let repo = source.repo.trim();
        let endpoint = source.endpoint.trim();
        if !repo.is_empty() && !is_valid_repo(repo) && !is_valid_url(repo) {
            return Err(format!("repo 格式非法: {}", source.repo));
        }
        if !endpoint.is_empty() && !is_valid_url(endpoint) {
            return Err(format!("endpoint 非法: {}", source.endpoint));
        }
        if provider == "github_releases" && repo.is_empty() && endpoint.is_empty() {
            return Err("github_releases 需要 repo 或 endpoint".to_string());
        }
        if provider == "forgejo_releases" && endpoint.is_empty() {
            return Err("forgejo_releases 需要 endpoint".to_string());
        }
        if provider == "github_actions" {
            if endpoint.is_empty() {
                return Err("github_actions 需要 endpoint".to_string());
            }
            if source.url_template.trim().is_empty() {
                return Err("github_actions 需要 url_template".to_string());
            }
            if !source.url_template.contains("{id}") {
                return Err("github_actions url_template 必须包含 {id}".to_string());
            }
        }

        if source.max_count <= 0 || source.max_count > 100 {
            return Err(format!("max_count 超出范围(1-100): {}", source.max_count));
        }
        if source.asset_index < -1 || source.asset_index > 100 {
            return Err(format!("asset_index 超出范围(-1-100): {}", source.asset_index));
        }
    }

    Ok(())
}

fn is_valid_family_key(key: &str) -> bool {
    key.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn is_valid_repo(repo: &str) -> bool {
    let mut parts = repo.split('/');
    let owner = parts.next().unwrap_or("").trim();
    let name = parts.next().unwrap_or("").trim();
    parts.next().is_none() && !owner.is_empty() && !name.is_empty()
}

fn is_valid_url(value: &str) -> bool {
    value.starts_with("https://") || value.starts_with("http://")
}

fn matches_pattern(input: &str, pattern: &str) -> bool {
    let p = pattern.trim();
    if p.is_empty() {
        return false;
    }

    if let Ok(re) = Regex::new(p) {
        return re.is_match(input);
    }

    input.to_lowercase().contains(&p.to_lowercase())
}

pub fn classify_local_runner(name: &str, families: &[ProtonFamily]) -> String {
    let input = name.trim();
    for family in families.iter().filter(|f| f.enabled) {
        if family.family_key.eq_ignore_ascii_case("custom") {
            continue;
        }
        if family
            .detect_patterns
            .iter()
            .any(|pattern| matches_pattern(input, pattern))
        {
            return family.family_key.clone();
        }
    }

    if families
        .iter()
        .any(|f| f.enabled && f.family_key.eq_ignore_ascii_case("custom"))
    {
        return "custom".to_string();
    }

    families
        .iter()
        .find(|f| f.enabled)
        .map(|f| f.family_key.clone())
        .unwrap_or_default()
}

pub fn scan_local_grouped(custom_paths: &[String]) -> Result<Vec<ProtonFamilyLocalGroup>, String> {
    let catalog = load_catalog_from_db()?;
    let versions = detector::scan_all_versions(custom_paths);

    let mut groups: Vec<ProtonFamilyLocalGroup> = catalog
        .families
        .iter()
        .filter(|f| f.enabled)
        .map(|family| ProtonFamilyLocalGroup {
            family_key: family.family_key.clone(),
            display_name: family.display_name.clone(),
            items: Vec::new(),
        })
        .collect();

    let mut idx_map: HashMap<String, usize> = HashMap::new();
    for (idx, group) in groups.iter().enumerate() {
        idx_map.insert(group.family_key.to_lowercase(), idx);
    }

    for version in versions {
        let family_key = classify_local_runner(&version.name, &catalog.families);
        let idx = idx_map
            .get(&family_key.to_lowercase())
            .copied()
            .or_else(|| idx_map.get("custom").copied());
        if let Some(i) = idx {
            groups[i].items.push(ProtonLocalVersionItem {
                id: version.id,
                name: version.name,
                variant: version.variant.to_string().to_lowercase(),
                path: version.path.to_string_lossy().to_string(),
                version: version.version,
                timestamp: version.timestamp,
            });
        }
    }

    for group in &mut groups {
        group.items.sort_by(|a, b| a.name.cmp(&b.name));
    }

    Ok(groups)
}

pub async fn fetch_remote_by_catalog(
    catalog: &ProtonCatalog,
    installed: &[WineVersion],
    github_token: Option<&str>,
) -> Result<Vec<ProtonFamilyRemoteGroup>, String> {
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let mut groups: Vec<ProtonFamilyRemoteGroup> = catalog
        .families
        .iter()
        .filter(|f| f.enabled)
        .map(|family| ProtonFamilyRemoteGroup {
            family_key: family.family_key.clone(),
            display_name: family.display_name.clone(),
            items: Vec::new(),
        })
        .collect();

    let mut idx_map: HashMap<String, usize> = HashMap::new();
    for (idx, group) in groups.iter().enumerate() {
        idx_map.insert(group.family_key.to_lowercase(), idx);
    }

    let installed_set: HashSet<String> = installed
        .iter()
        .flat_map(|v| {
            [
                v.name.to_lowercase(),
                v.version.to_lowercase(),
                v.id.to_lowercase(),
            ]
        })
        .collect();

    let sources = catalog
        .sources
        .iter()
        .filter(|s| s.enabled)
        .cloned()
        .collect::<Vec<_>>();
    let tasks = sources
        .iter()
        .map(|source| fetch_source_releases(&client, source, &installed_set, github_token));

    let results = join_all(tasks).await;
    let mut failures = Vec::new();
    for result in results {
        match result {
            Ok((family_key, mut items)) => {
                if let Some(i) = idx_map.get(&family_key.to_lowercase()).copied() {
                    groups[i].items.append(&mut items);
                }
            }
            Err(e) => failures.push(e),
        }
    }

    for group in &mut groups {
        let mut dedup = HashSet::new();
        group.items.retain(|item| {
            let key = format!(
                "{}:{}",
                item.source_repo.to_lowercase(),
                item.tag.to_lowercase()
            );
            dedup.insert(key)
        });
        group.items.sort_by(|a, b| {
            b.published_at
                .cmp(&a.published_at)
                .then_with(|| a.tag.cmp(&b.tag))
        });
    }

    for group in &mut groups {
        if !group.items.is_empty() {
            continue;
        }
        let mut locals = installed
            .iter()
            .filter(|v| {
                classify_local_runner(&v.name, &catalog.families)
                    .eq_ignore_ascii_case(&group.family_key)
            })
            .collect::<Vec<_>>();
        if locals.is_empty() && group.family_key.eq_ignore_ascii_case("custom") {
            locals = installed.iter().collect::<Vec<_>>();
        }
        locals.sort_by(|a, b| b.timestamp.cmp(&a.timestamp).then_with(|| a.name.cmp(&b.name)));
        for v in locals {
            group.items.push(ProtonRemoteVersionItem {
                tag: v.name.clone(),
                version: v.version.clone(),
                variant: group.family_key.clone(),
                download_url: String::new(),
                file_size: 0,
                published_at: v.timestamp.clone().unwrap_or_default(),
                installed: true,
                source_repo: "local-installed".to_string(),
            });
        }
    }

    let total_items: usize = groups.iter().map(|g| g.items.len()).sum();
    if !failures.is_empty() {
        let joined = failures.join(" | ");
        if total_items == 0 {
            warn!("Proton 远程源全部失败: {}", joined);
        } else {
            info!("部分 Proton 远程源失败（已忽略）: {}", joined);
        }
    }

    Ok(groups)
}

async fn fetch_source_releases(
    client: &reqwest::Client,
    source: &ProtonSource,
    installed_set: &HashSet<String>,
    github_token: Option<&str>,
) -> Result<(String, Vec<ProtonRemoteVersionItem>), String> {
    if source.provider == "github_actions" {
        return fetch_source_github_actions(client, source, github_token).await;
    }

    if !matches!(
        source.provider.as_str(),
        "github_releases" | "forgejo_releases"
    ) {
        return Err(format!("不支持的 provider: {}", source.provider));
    }

    let per_page = source.max_count.clamp(1, 100) as usize;
    let api_urls = release_api_candidates(source, per_page);
    if api_urls.is_empty() {
        return Err(format!(
            "source 配置无有效 releases endpoint: provider={}, repo={}, endpoint={}",
            source.provider, source.repo, source.endpoint
        ));
    }

    let mut response = None;
    let mut errors: Vec<String> = Vec::new();

    for api_url in &api_urls {
        let mut req = client
            .get(api_url)
            .header("User-Agent", "SSMT4/0.1")
            .header("Accept", "application/json");
        if source.provider == "github_releases" {
            req = req.header("Accept", "application/vnd.github.v3+json");
        }
        if source.provider == "github_releases" {
            if let Some(token) = github_token.map(|v| v.trim()).filter(|v| !v.is_empty()) {
                req = req.bearer_auth(token);
            }
        }

        match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                response = Some(resp);
                break;
            }
            Ok(resp) => {
                errors.push(format!("{} => HTTP {}", api_url, resp.status()));
            }
            Err(e) => {
                errors.push(format!("{} => {}", api_url, e));
            }
        }
    }

    let response = response.ok_or_else(|| {
        let summary = if errors.is_empty() {
            "unknown error".to_string()
        } else {
            errors.join("; ")
        };
        let src = if !source.repo.trim().is_empty() {
            source.repo.clone()
        } else {
            source.endpoint.clone()
        };
        format!("请求 {} 失败: {}", src, summary)
    })?;
    let releases: Vec<serde_json::Value> = response.json().await.map_err(|e| {
        let src = if !source.repo.trim().is_empty() {
            source.repo.clone()
        } else {
            source.endpoint.clone()
        };
        format!("解析 {} 响应失败: {}", src, e)
    })?;

    let mut items = Vec::new();
    let mut logged_asset_index_fallback = false;
    for release in releases {
        let prerelease = release
            .get("prerelease")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if prerelease && !source.include_prerelease {
            continue;
        }

        let tag = release
            .get("tag_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if tag.is_empty() {
            continue;
        }
        if !matches_pattern(&tag, &source.tag_pattern) {
            continue;
        }

        let published_at = release
            .get("published_at")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if let Some(assets) = release.get("assets").and_then(|v| v.as_array()) {
            let pick_by_pattern = || {
                assets.iter().find(|asset| {
                    let asset_name = asset.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    matches_pattern(asset_name, &source.asset_pattern)
                })
            };

            let selected_asset = if source.asset_index >= 0 {
                let direct = assets.get(source.asset_index as usize);
                if direct.is_none() && !logged_asset_index_fallback {
                    info!(
                        "Proton source asset_index 越界，回退匹配: family={}, repo={}, asset_index={}, assets_len={}",
                        source.family_key,
                        if source.repo.trim().is_empty() {
                            source.endpoint.as_str()
                        } else {
                            source.repo.as_str()
                        },
                        source.asset_index,
                        assets.len()
                    );
                    logged_asset_index_fallback = true;
                }
                direct.or_else(pick_by_pattern).or_else(|| assets.first())
            } else {
                pick_by_pattern().or_else(|| assets.first())
            };

            if let Some(asset) = selected_asset {
                let download_url = asset
                    .get("browser_download_url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if !download_url.is_empty() {
                    let file_size = asset.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
                    let tag_lower = tag.to_lowercase();
                    let installed = installed_set.contains(&tag_lower)
                        || installed_set.iter().any(|item| item.contains(&tag_lower));

                    items.push(ProtonRemoteVersionItem {
                        tag: tag.clone(),
                        version: tag.clone(),
                        variant: source.family_key.clone(),
                        download_url,
                        file_size,
                        published_at: published_at.clone(),
                        installed,
                        source_repo: if !source.repo.trim().is_empty() {
                            source.repo.clone()
                        } else {
                            source.endpoint.clone()
                        },
                    });
                }
            }
        }
    }

    Ok((source.family_key.clone(), items))
}

fn release_api_candidates(source: &ProtonSource, per_page: usize) -> Vec<String> {
    let mut urls = Vec::new();

    let endpoint = source.endpoint.trim();
    if !endpoint.is_empty() {
        urls.push(add_or_replace_query_param(
            endpoint,
            "per_page",
            &per_page.to_string(),
        ));
    } else if is_valid_url(source.repo.trim()) {
        urls.push(add_or_replace_query_param(
            source.repo.trim(),
            "per_page",
            &per_page.to_string(),
        ));
    } else if !source.repo.trim().is_empty() {
        urls.push(format!(
            "https://api.github.com/repos/{}/releases?per_page={}",
            source.repo.trim(),
            per_page
        ));
    }

    if source.provider == "github_releases" {
        if let Some(primary) = urls.first() {
            if primary.contains("api.github.com") {
                urls.push(primary.replace("api.github.com", "ghp.ci/api.github.com"));
            }
        }
    }

    urls
}

fn add_or_replace_query_param(url: &str, key: &str, value: &str) -> String {
    if url.contains('?') {
        format!("{}&{}={}", url, key, value)
    } else {
        format!("{}?{}={}", url, key, value)
    }
}

async fn fetch_source_github_actions(
    client: &reqwest::Client,
    source: &ProtonSource,
    github_token: Option<&str>,
) -> Result<(String, Vec<ProtonRemoteVersionItem>), String> {
    let endpoint = source.endpoint.trim();
    if endpoint.is_empty() {
        return Err("github_actions source endpoint 为空".to_string());
    }
    if source.url_template.trim().is_empty() || !source.url_template.contains("{id}") {
        return Err("github_actions source url_template 无效".to_string());
    }

    let per_page = source.max_count.clamp(1, 100).to_string();
    let api_url = add_or_replace_query_param(endpoint, "per_page", &per_page);
    let mut req = client
        .get(&api_url)
        .header("User-Agent", "SSMT4/0.1")
        .header("Accept", "application/vnd.github.v3+json");
    if let Some(token) = github_token.map(|v| v.trim()).filter(|v| !v.is_empty()) {
        req = req.bearer_auth(token);
    }

    let response = req
        .send()
        .await
        .map_err(|e| format!("请求 actions 失败: {}", e))?;
    if !response.status().is_success() {
        return Err(format!("请求 actions 失败: HTTP {}", response.status()));
    }

    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("解析 actions 响应失败: {}", e))?;
    let runs = payload
        .get("workflow_runs")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "actions 响应缺少 workflow_runs".to_string())?;

    let mut items = Vec::new();
    for run in runs {
        let status = run.get("status").and_then(|v| v.as_str()).unwrap_or("");
        let conclusion = run.get("conclusion").and_then(|v| v.as_str()).unwrap_or("");
        if status != "completed" || conclusion != "success" {
            continue;
        }

        let id = run.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
        if id <= 0 {
            continue;
        }

        let run_number = run
            .get("run_number")
            .and_then(|v| v.as_i64())
            .map(|v| v.to_string())
            .unwrap_or_else(|| id.to_string());
        let tag = format!("run-{}", run_number);
        if !matches_pattern(&tag, &source.tag_pattern) {
            continue;
        }

        let published_at = run
            .get("created_at")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let download_url = source.url_template.replace("{id}", &id.to_string());

        items.push(ProtonRemoteVersionItem {
            tag: tag.clone(),
            version: tag,
            variant: source.family_key.clone(),
            download_url,
            file_size: 0,
            published_at,
            installed: false,
            source_repo: source.repo.clone(),
        });
    }

    Ok((source.family_key.clone(), items))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_patterns_classify_expected() {
        let families = vec![
            ProtonFamily {
                family_key: "ge-proton".to_string(),
                display_name: "GE-Proton".to_string(),
                enabled: true,
                sort_order: 0,
                detect_patterns: vec!["(?i)ge-proton".to_string()],
                builtin: true,
            },
            ProtonFamily {
                family_key: "custom".to_string(),
                display_name: "Custom".to_string(),
                enabled: true,
                sort_order: 1,
                detect_patterns: vec![],
                builtin: true,
            },
        ];

        assert_eq!(
            classify_local_runner("GE-Proton10-30", &families),
            "ge-proton".to_string()
        );
        assert_eq!(
            classify_local_runner("unknown-runner", &families),
            "custom".to_string()
        );
    }

    #[test]
    fn save_validation_repo_format() {
        let catalog = ProtonCatalog {
            families: vec![ProtonFamily {
                family_key: "ge-proton".to_string(),
                display_name: "GE-Proton".to_string(),
                enabled: true,
                sort_order: 0,
                detect_patterns: vec![],
                builtin: true,
            }],
            sources: vec![ProtonSource {
                id: None,
                family_key: "ge-proton".to_string(),
                provider: "github_releases".to_string(),
                repo: "invalid_repo".to_string(),
                endpoint: String::new(),
                url_template: String::new(),
                asset_index: -1,
                asset_pattern: "(?i)\\.tar\\.(gz|xz)$".to_string(),
                tag_pattern: ".*".to_string(),
                max_count: 15,
                include_prerelease: false,
                enabled: true,
                note: String::new(),
            }],
        };

        assert!(validate_catalog(&catalog).is_err());
    }
}
