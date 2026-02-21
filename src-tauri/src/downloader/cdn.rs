use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    #[serde(default)]
    pub sha256: Option<String>,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceIndex {
    pub resource: Vec<ResourceFile>,
}

pub async fn fetch_launcher_info(launcher_api: &str) -> Result<LauncherInfo, String> {
    let client = Client::new();
    let data = fetch_json(&client, launcher_api).await?;

    let default = find_launcher_default_payload(&data).ok_or_else(|| {
        if looks_like_launcher_installer_payload(&data) {
            "Launcher API is launcher-installer payload (version/exe_url), not full-game payload; use launcher_installer mode".to_string()
        } else {
            let keys = data
                .as_object()
                .map(|m| m.keys().cloned().collect::<Vec<_>>().join(","))
                .unwrap_or_else(|| "<non-object>".to_string());
            format!(
                "Missing 'default' field in launcher info (top-level keys: {})",
                keys
            )
        }
    })?;

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

    crate::log_info!("Launcher info: version={}, cdn={}", version, cdn_url);

    Ok(LauncherInfo {
        version,
        resources_base_path,
        cdn_url,
        index_file_url,
        patch_configs,
        raw: data,
    })
}

fn looks_like_launcher_default_payload(node: &Value) -> bool {
    node.get("version").and_then(|v| v.as_str()).is_some()
        && node.get("config").and_then(|v| v.as_object()).is_some()
        && node.get("cdnList").and_then(|v| v.as_array()).is_some()
}

fn looks_like_launcher_installer_payload(root: &Value) -> bool {
    let candidate = root.get("rsp").unwrap_or(root);
    candidate.get("version").and_then(|v| v.as_str()).is_some()
        && candidate
            .get("exe_url")
            .or_else(|| candidate.get("exeUrl"))
            .and_then(|v| v.as_str())
            .is_some()
}

fn get_nested<'a>(root: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = root;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn find_launcher_default_payload<'a>(root: &'a Value) -> Option<&'a Value> {
    if looks_like_launcher_default_payload(root) {
        return Some(root);
    }

    // Common wrapper layouts used by launcher APIs.
    let candidate_paths: &[&[&str]] = &[
        &["default"],
        &["rsp", "default"],
        &["data", "default"],
        &["result", "default"],
        &["rsp", "data", "default"],
        &["data", "rsp", "default"],
        &["result", "data", "default"],
        &["payload", "default"],
        &["rsp"],
        &["data"],
        &["result"],
        &["payload"],
    ];

    for path in candidate_paths {
        if let Some(node) = get_nested(root, path) {
            if looks_like_launcher_default_payload(node) {
                return Some(node);
            }
        }
    }

    None
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
        .enumerate()
        .map(|(idx, r)| {
            let sha256 = r
                .get("sha256")
                .or_else(|| r.get("sha_256"))
                .or_else(|| r.get("sha256sum"))
                .or_else(|| r.get("sha256Sum"))
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(ToString::to_string);

            let md5 = r
                .get("md5")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(ToString::to_string)
                .unwrap_or_default();

            let dest = r
                .get("dest")
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("resource[{}] missing dest", idx))?
                .to_string();

            if sha256.is_none() && md5.is_empty() {
                return Err(format!(
                    "resource[{}] ({}) missing checksum metadata (need sha256 or md5)",
                    idx, dest
                ));
            }

            let size = r
                .get("size")
                .and_then(|v| v.as_u64().or_else(|| v.as_str()?.parse::<u64>().ok()))
                .ok_or_else(|| format!("resource[{}] ({}) missing/invalid size", idx, dest))?;

            Ok(ResourceFile {
                dest,
                md5,
                sha256,
                size,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    crate::log_info!("Resource index: {} files", resource_list.len());
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
    crate::log_info!("Fetching JSON: {}", url);

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
        match retry_client.get(url).send().await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    last_err = format!("HTTP {}: {}", resp.status(), url);
                    crate::log_warn!(
                        "fetch_json 第 {}/{} 次失败: {}",
                        attempt,
                        max_retries,
                        last_err
                    );
                    if attempt < max_retries {
                        tokio::time::sleep(std::time::Duration::from_secs(2 * attempt as u64))
                            .await;
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
                    crate::log_error!(
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
                crate::log_warn!(
                    "fetch_json 第 {}/{} 次失败: {} ({})",
                    attempt,
                    max_retries,
                    last_err,
                    url
                );
                if attempt < max_retries {
                    tokio::time::sleep(std::time::Duration::from_secs(2 * attempt as u64)).await;
                    continue;
                }
            }
        }
    }

    crate::log_error!(
        "fetch_json 全部 {} 次重试失败: {} ({})",
        max_retries,
        last_err,
        url
    );
    Err(format!("下载失败: {} ({})", last_err, url))
}

pub fn join_url(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    format!("{}/{}", base, path)
}

#[cfg(test)]
mod tests {
    use super::find_launcher_default_payload;
    use serde_json::json;

    fn make_default_node() -> serde_json::Value {
        json!({
            "version": "1.0.0",
            "cdnList": [{"url": "https://cdn.example.com", "K1": 1, "K2": 1, "P": 1}],
            "config": {"indexFile": "index.json", "patchConfig": []}
        })
    }

    #[test]
    fn launcher_default_payload_supports_top_level_default() {
        let data = json!({ "default": make_default_node() });
        let node = find_launcher_default_payload(&data).expect("default payload");
        assert_eq!(node.get("version").and_then(|v| v.as_str()), Some("1.0.0"));
    }

    #[test]
    fn launcher_default_payload_supports_rsp_default_wrapper() {
        let data = json!({ "rsp": { "default": make_default_node() } });
        let node = find_launcher_default_payload(&data).expect("wrapped default payload");
        assert_eq!(node.get("version").and_then(|v| v.as_str()), Some("1.0.0"));
    }

    #[test]
    fn launcher_default_payload_supports_rsp_data_default_wrapper() {
        let data = json!({ "rsp": { "data": { "default": make_default_node() } } });
        let node = find_launcher_default_payload(&data).expect("deep wrapped default payload");
        assert_eq!(node.get("version").and_then(|v| v.as_str()), Some("1.0.0"));
    }
}
