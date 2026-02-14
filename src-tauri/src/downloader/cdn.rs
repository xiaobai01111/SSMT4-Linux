use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherInfo {
    pub version: String,
    pub resources_base_path: String,
    pub cdn_url: String,
    pub index_file_url: String,
    pub patch_configs: Vec<PatchConfig>,
    pub raw: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchConfig {
    pub version: String,
    pub base_url: String,
    pub index_file: String,
    pub ext: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceFile {
    pub dest: String,
    pub md5: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceIndex {
    pub resource: Vec<ResourceFile>,
}

pub async fn fetch_launcher_info(launcher_api: &str) -> Result<LauncherInfo, String> {
    let client = Client::new();
    let data = fetch_json(&client, launcher_api).await?;

    let default = data
        .get("default")
        .ok_or("Missing 'default' field in launcher info")?;

    let version = default
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or("Missing version")?
        .to_string();

    let resources_base_path = default
        .get("resourcesBasePath")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let config = default.get("config").ok_or("Missing config")?;

    let index_file_url = config
        .get("indexFile")
        .and_then(|v| v.as_str())
        .ok_or("Missing indexFile")?
        .to_string();

    let cdn_list = default
        .get("cdnList")
        .and_then(|v| v.as_array())
        .ok_or("Missing cdnList")?;

    let cdn_url = select_best_cdn(cdn_list)?;

    let patch_configs = config
        .get("patchConfig")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|p| {
                    Some(PatchConfig {
                        version: p.get("version")?.as_str()?.to_string(),
                        base_url: p.get("baseUrl")?.as_str()?.to_string(),
                        index_file: p.get("indexFile")?.as_str()?.to_string(),
                        ext: p.get("ext")?.as_array()?.clone(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    info!("Launcher info: version={}, cdn={}", version, cdn_url);

    Ok(LauncherInfo {
        version,
        resources_base_path,
        cdn_url,
        index_file_url,
        patch_configs,
        raw: data,
    })
}

pub async fn fetch_resource_index(
    cdn_url: &str,
    index_file_path: &str,
) -> Result<ResourceIndex, String> {
    let client = Client::new();
    let url = join_url(cdn_url, index_file_path);
    let data = fetch_json(&client, &url).await?;

    let resources = data
        .get("resource")
        .and_then(|v| v.as_array())
        .ok_or("Missing 'resource' in index")?;

    let resource_list: Vec<ResourceFile> = resources
        .iter()
        .filter_map(|r| {
            Some(ResourceFile {
                dest: r.get("dest")?.as_str()?.to_string(),
                md5: r.get("md5")?.as_str()?.to_string(),
                size: r
                    .get("size")?
                    .as_u64()
                    .or_else(|| r.get("size")?.as_str()?.parse::<u64>().ok())?,
            })
        })
        .collect();

    info!("Resource index: {} files", resource_list.len());
    Ok(ResourceIndex {
        resource: resource_list,
    })
}

fn select_best_cdn(cdn_list: &[Value]) -> Result<String, String> {
    let available: Vec<&Value> = cdn_list
        .iter()
        .filter(|node| {
            node.get("K1").and_then(|v| v.as_i64()) == Some(1)
                && node.get("K2").and_then(|v| v.as_i64()) == Some(1)
        })
        .collect();

    if available.is_empty() {
        return Err("No available CDN nodes".to_string());
    }

    let best = available
        .iter()
        .max_by_key(|n| n.get("P").and_then(|v| v.as_i64()).unwrap_or(0))
        .ok_or("Failed to select CDN")?;

    best.get("url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "CDN node missing url".to_string())
}

async fn fetch_json(client: &Client, url: &str) -> Result<Value, String> {
    info!("Fetching JSON: {}", url);

    // 带超时的客户端（防止无限等待）
    let retry_client = Client::builder()
        .user_agent("Mozilla/5.0")
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| client.clone());

    let max_retries = 3;
    let mut last_err = String::new();

    for attempt in 1..=max_retries {
        match retry_client
            .get(url)
            .send()
            .await
        {
            Ok(resp) => {
                if !resp.status().is_success() {
                    last_err = format!("HTTP {}: {}", resp.status(), url);
                    warn!("fetch_json 第 {}/{} 次失败: {}", attempt, max_retries, last_err);
                    if attempt < max_retries {
                        tokio::time::sleep(std::time::Duration::from_secs(2 * attempt as u64)).await;
                        continue;
                    }
                    return Err(last_err);
                }

                let bytes = resp
                    .bytes()
                    .await
                    .map_err(|e| format!("Failed to read response body: {}", e))?;

                let text = String::from_utf8(bytes.to_vec())
                    .or_else(|_| {
                        let (decoded, _, _) = encoding_rs::GBK.decode(&bytes);
                        Ok::<String, String>(decoded.into_owned())
                    })
                    .map_err(|e| format!("Failed to decode response: {}", e))?;

                return serde_json::from_str(&text).map_err(|e| {
                    tracing::error!(
                        "Failed to parse JSON from {}: {} (first 200 chars: {:?})",
                        url,
                        e,
                        &text[..text.len().min(200)]
                    );
                    format!("Failed to parse JSON: {}", e)
                });
            }
            Err(e) => {
                last_err = format!("HTTP request failed: {}", e);
                warn!("fetch_json 第 {}/{} 次失败: {} ({})", attempt, max_retries, last_err, url);
                if attempt < max_retries {
                    tokio::time::sleep(std::time::Duration::from_secs(2 * attempt as u64)).await;
                    continue;
                }
            }
        }
    }

    tracing::error!("fetch_json 全部 {} 次重试失败: {} ({})", max_retries, last_err, url);
    Err(format!("下载失败: {} ({})", last_err, url))
}

pub fn join_url(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    format!("{}/{}", base, path)
}
