use super::*;
use tracing::{info, warn};

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
    if let Some(token) = github_token
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
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
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        if prerelease && !source.include_prerelease {
            continue;
        }

        let tag_name = release
            .get("tag_name")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string();

        let published_at = release
            .get("published_at")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string();

        let assets = release.get("assets").and_then(|value| value.as_array());
        if let Some(assets) = assets {
            for asset in assets {
                let name = asset
                    .get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                if matches_asset_pattern(name, &source.asset_pattern) {
                    let download_url = asset
                        .get("browser_download_url")
                        .and_then(|value| value.as_str())
                        .unwrap_or("")
                        .to_string();
                    let file_size = asset
                        .get("size")
                        .and_then(|value| value.as_u64())
                        .unwrap_or(0);

                    let version = extract_version_from_asset(name, source).unwrap_or_else(|| {
                        tag_name
                            .strip_prefix('v')
                            .or_else(|| tag_name.strip_prefix("gplasync-v"))
                            .unwrap_or(&tag_name)
                            .to_string()
                    });

                    let is_local = local_versions
                        .iter()
                        .any(|value| value.version == version && value.variant == source.id);

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

fn extract_version_from_asset(name: &str, source: &DxvkVariantSource) -> Option<String> {
    if let Some(version) = extract_version_by_template(name, &source.archive_name_template) {
        return Some(version);
    }

    let stem = name.strip_suffix(".tar.gz")?;
    match source.id.to_ascii_lowercase().as_str() {
        "gplasync" => stem
            .strip_prefix("dxvk-gplasync-v")
            .or_else(|| stem.strip_prefix("dxvk-gplasync-"))
            .map(|value| value.to_string()),
        _ => stem.strip_prefix("dxvk-").map(|value| value.to_string()),
    }
}

async fn fetch_dxvk_gplasync_from_gitlab(
    source: &DxvkVariantSource,
    max_count: usize,
    local_versions: &[DxvkLocalVersion],
) -> Result<Vec<DxvkRemoteVersion>, String> {
    let endpoint = source.endpoint.trim();
    if endpoint.is_empty() {
        return Err(format!("DXVK source '{}' 缺少 endpoint", source.id));
    }

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
        let name = file
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        if !matches_asset_pattern(name, &source.asset_pattern) {
            continue;
        }

        let version = match extract_version_from_asset(name, source) {
            Some(version) => version,
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
            .any(|value| value.version == version && value.variant == source.id);

        result.push(DxvkRemoteVersion {
            version,
            variant: source.id.clone(),
            tag_name: name.to_string(),
            download_url,
            file_size: 0,
            published_at: String::new(),
            is_local,
        });
    }

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
        .filter(|variant| variant.enabled && !variant.id.trim().is_empty())
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
            Ok(versions) => {
                set_cached_variant_versions(&source.id, versions.clone());
                all.extend(versions);
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
    if let Some(token) = github_token
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
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
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        if prerelease {
            continue;
        }

        let tag_name = release
            .get("tag_name")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string();
        let published_at = release
            .get("published_at")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string();

        let Some(assets) = release.get("assets").and_then(|value| value.as_array()) else {
            continue;
        };
        let Some(asset) = assets.iter().find(|asset| {
            let name = asset
                .get("name")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            name.starts_with("vkd3d-proton-") && name.ends_with(".tar.zst")
        }) else {
            continue;
        };

        let name = asset
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let version = parse_vkd3d_version_from_name(name)
            .unwrap_or_else(|| tag_name.trim_start_matches('v').trim().to_string());
        let download_url = asset
            .get("browser_download_url")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string();
        let file_size = asset
            .get("size")
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        let is_local = local_versions.iter().any(|value| value.version == version);

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

#[cfg(test)]
mod tests {
    use super::*;

    fn dxvk_source(id: &str, template: &str) -> DxvkVariantSource {
        DxvkVariantSource {
            id: id.to_string(),
            display_name: id.to_string(),
            provider: "github_releases".to_string(),
            repo: "example/repo".to_string(),
            endpoint: String::new(),
            asset_pattern: String::new(),
            download_url_template: String::new(),
            archive_name_template: template.to_string(),
            include_prerelease: false,
            enabled: true,
            note: String::new(),
        }
    }

    #[test]
    fn extract_version_from_asset_uses_template_for_dxvk() {
        let source = dxvk_source("dxvk", "dxvk-{version}.tar.gz");

        assert_eq!(
            extract_version_from_asset("dxvk-2.4.tar.gz", &source).as_deref(),
            Some("2.4")
        );
    }

    #[test]
    fn extract_version_from_asset_falls_back_for_gplasync_names() {
        let source = dxvk_source("gplasync", "dxvk-gplasync-v{version}.tar.gz");

        assert_eq!(
            extract_version_from_asset("dxvk-gplasync-v2.4.tar.gz", &source).as_deref(),
            Some("2.4")
        );
        assert_eq!(
            extract_version_from_asset("dxvk-gplasync-2.5.tar.gz", &source).as_deref(),
            Some("2.5")
        );
    }

    #[test]
    fn extract_version_from_asset_returns_none_for_unmatched_name() {
        let source = dxvk_source("dxvk", "dxvk-{version}.tar.gz");

        assert_eq!(
            extract_version_from_asset("vkd3d-proton-2.8.tar.zst", &source),
            None
        );
    }
}
