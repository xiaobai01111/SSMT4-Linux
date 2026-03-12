use crate::configs::wine_config::WineVersion;
use tracing::{info, warn};

fn normalize_asset_name_tokens(name: &str) -> String {
    let mut normalized = String::with_capacity(name.len() + 2);
    normalized.push(' ');
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
        } else {
            normalized.push(' ');
        }
    }
    normalized.push(' ');
    normalized
}

fn is_supported_x86_64_proton_asset(name: &str) -> bool {
    let normalized = normalize_asset_name_tokens(name);
    if [" x86 64 ", " amd64 ", " x64 ", " win64 ", " 64 bit "]
        .iter()
        .any(|token| normalized.contains(token))
    {
        return true;
    }

    ![
        " arm64 ",
        " aarch64 ",
        " armv6 ",
        " armv7 ",
        " armv8 ",
        " armhf ",
        " armel ",
        " arm ",
        " i386 ",
        " i686 ",
        " win32 ",
        " 32 bit ",
        " x86 ",
    ]
    .iter()
    .any(|token| normalized.contains(token))
}

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

    let (ge, wine_ge, kron4ek, dw) = tokio::join!(
        fetch_github_releases(
            &client,
            "GloriousEggroll/proton-ge-custom",
            "GE-Proton",
            15,
            installed
        ),
        fetch_github_releases(
            &client,
            "GloriousEggroll/wine-ge-custom",
            "Wine-GE",
            10,
            installed
        ),
        fetch_github_releases(&client, "Kron4ek/Wine-Builds", "Wine-Builds", 10, installed),
        fetch_github_releases(
            &client,
            "AUNaseef/proton-ge-custom",
            "DW-Proton",
            10,
            installed
        ),
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

    let installed_tags: std::collections::HashSet<String> =
        installed.iter().map(|v| v.name.clone()).collect();

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
                let name = asset.get("name").and_then(|v| v.as_str()).unwrap_or("");
                if (name.ends_with(".tar.gz") || name.ends_with(".tar.xz"))
                    && is_supported_x86_64_proton_asset(name)
                {
                    let download_url = asset
                        .get("browser_download_url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let file_size = asset.get("size").and_then(|v| v.as_u64()).unwrap_or(0);

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
