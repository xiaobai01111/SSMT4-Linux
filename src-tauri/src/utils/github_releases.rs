use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
struct GitHubReleaseWire {
    tag_name: String,
    published_at: Option<String>,
    assets: Vec<GitHubAssetWire>,
    body: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct GitHubAssetWire {
    name: String,
    size: u64,
    browser_download_url: String,
}

#[derive(Debug, Clone)]
pub struct GitHubReleaseAsset {
    pub name: String,
    pub size: u64,
    pub download_url: String,
}

#[derive(Debug, Clone)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub published_at: String,
    pub body: String,
    pub assets: Vec<GitHubReleaseAsset>,
}

pub async fn fetch_repo_releases(
    repo: &str,
    per_page: usize,
    github_token: Option<&str>,
) -> Result<Vec<GitHubRelease>, String> {
    let url = format!("https://api.github.com/repos/{repo}/releases?per_page={per_page}");

    let client = reqwest::Client::new();
    let mut req = client
        .get(&url)
        .header("User-Agent", "SSMT4/0.1")
        .header("Accept", "application/vnd.github+json");
    if let Some(token) = github_token.filter(|token| !token.is_empty()) {
        req = req.header("Authorization", format!("Bearer {}", token));
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("获取 GitHub release 列表失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub API 返回 HTTP {}: {}", resp.status(), url));
    }

    let releases = resp
        .json::<Vec<GitHubReleaseWire>>()
        .await
        .map_err(|e| format!("解析 GitHub releases JSON 失败: {}", e))?;

    Ok(releases
        .into_iter()
        .map(|release| GitHubRelease {
            tag_name: release.tag_name,
            published_at: release.published_at.unwrap_or_default(),
            body: release.body.unwrap_or_default(),
            assets: release
                .assets
                .into_iter()
                .map(|asset| GitHubReleaseAsset {
                    name: asset.name,
                    size: asset.size,
                    download_url: asset.browser_download_url,
                })
                .collect(),
        })
        .collect())
}
