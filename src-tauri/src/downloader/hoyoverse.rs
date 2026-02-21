use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

// ============================================================
// HoYoverse getGamePackages API 数据结构
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse {
    pub retcode: i32,
    pub message: String,
    pub data: ApiData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiData {
    pub game_packages: Vec<GamePackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamePackage {
    pub game: GameId,
    pub main: GameInfo,
    pub pre_download: Option<PredownloadInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameId {
    pub id: String,
    pub biz: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameInfo {
    pub major: GameLatest,
    pub patches: Vec<GamePatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameLatest {
    pub version: String,
    pub game_pkgs: Vec<Segment>,
    pub audio_pkgs: Vec<AudioPkg>,
    pub res_list_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub url: String,
    pub md5: String,
    #[serde(default)]
    pub sha256: String,
    pub size: String,
    pub decompressed_size: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioPkg {
    pub language: String,
    pub url: String,
    pub md5: String,
    #[serde(default)]
    pub sha256: String,
    pub size: String,
    pub decompressed_size: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamePatch {
    pub version: String,
    pub game_pkgs: Vec<Segment>,
    pub audio_pkgs: Vec<AudioPkg>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredownloadInfo {
    pub major: Option<GameLatest>,
    pub patches: Vec<GamePatch>,
}

// ============================================================
// res_list 校验用的文件条目
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceEntry {
    pub remote_name: String,
    pub md5: String,
    #[serde(default)]
    pub sha256: String,
    pub file_size: u64,
}

// ============================================================
// API 请求函数
// ============================================================

/// 获取 HoYoverse 游戏包信息
/// biz_prefix: 用于匹配游戏，如 "hkrpg_" (星穹铁道), "hk4e_" (原神), "nap_" (绝区零)
pub async fn fetch_game_packages(api_url: &str, biz_prefix: &str) -> Result<GamePackage, String> {
    fetch_game_packages_via_hyp(api_url, biz_prefix).await
}

async fn fetch_game_packages_via_hyp(
    api_url: &str,
    biz_prefix: &str,
) -> Result<GamePackage, String> {
    let client = Client::new();
    let resp = client
        .get(api_url)
        .header("User-Agent", "Mozilla/5.0")
        .send()
        .await
        .map_err(|e| format!("HoYoverse API 请求失败: {}", e))?;

    let data: ApiResponse = resp
        .json()
        .await
        .map_err(|e| format!("HoYoverse API 解析失败: {}", e))?;

    if data.retcode != 0 {
        return Err(format!(
            "HoYoverse API 错误: {} (code: {})",
            data.message, data.retcode
        ));
    }

    let packages = data.data.game_packages;
    let exact_match = !biz_prefix.ends_with('_');
    let mut candidates: Vec<GamePackage> = packages
        .iter()
        .filter(|pkg| {
            if exact_match {
                pkg.game.biz == biz_prefix
            } else {
                pkg.game.biz.starts_with(biz_prefix)
            }
        })
        .cloned()
        .collect();

    // 兼容旧数据：历史上某些配置使用 "hk4e"/"hkrpg" 这种非 *_ 前缀，允许回退到前缀匹配。
    if candidates.is_empty() && exact_match {
        candidates = packages
            .into_iter()
            .filter(|pkg| pkg.game.biz.starts_with(biz_prefix))
            .collect();
    }

    if candidates.is_empty() {
        return Err(format!("API 响应中未找到游戏 (biz prefix: {})", biz_prefix));
    }

    let region_hint = infer_region_hint_from_api_url(api_url);
    if let Some(hint) = region_hint {
        let hinted: Vec<GamePackage> = candidates
            .iter()
            .filter(|pkg| biz_matches_region_hint(&pkg.game.biz, hint))
            .cloned()
            .collect();
        if !hinted.is_empty() {
            candidates = hinted;
        }
    }

    candidates.sort_by(|a, b| compare_version_desc(major_version_of(a), major_version_of(b)));

    let selected = candidates
        .into_iter()
        .next()
        .ok_or_else(|| format!("API 响应中未找到可用游戏包 (biz prefix: {})", biz_prefix))?;

    crate::log_info!(
        "HoYoverse package selected: biz={}, version={}, api={}",
        selected.game.biz,
        selected.main.major.version,
        api_url
    );
    Ok(selected)
}

fn major_version_of(pkg: &GamePackage) -> &str {
    pkg.main.major.version.trim()
}

fn infer_region_hint_from_api_url(api_url: &str) -> Option<&'static str> {
    if api_url.contains("hyp-api.mihoyo.com") {
        Some("cn")
    } else if api_url.contains("sg-hyp-api.hoyoverse.com") {
        Some("global")
    } else {
        None
    }
}

fn biz_matches_region_hint(biz: &str, hint: &str) -> bool {
    let biz = biz.trim().to_ascii_lowercase();
    let hint = hint.trim().to_ascii_lowercase();
    if hint == "cn" {
        return biz.ends_with("_cn");
    }
    if hint == "global" {
        return biz.ends_with("_global") || biz.ends_with("_overseas") || biz.ends_with("_os");
    }
    false
}

fn parse_version_segments(raw: &str) -> Vec<u32> {
    raw.split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<u32>().unwrap_or(0))
        .collect()
}

fn compare_version_desc(a: &str, b: &str) -> Ordering {
    let av = parse_version_segments(a);
    let bv = parse_version_segments(b);
    let max_len = av.len().max(bv.len());
    for i in 0..max_len {
        let ai = *av.get(i).unwrap_or(&0);
        let bi = *bv.get(i).unwrap_or(&0);
        if ai != bi {
            return bi.cmp(&ai);
        }
    }
    Ordering::Equal
}

/// 获取资源文件列表（用于校验）
pub async fn fetch_resource_list(res_list_url: &str) -> Result<Vec<ResourceEntry>, String> {
    if res_list_url.is_empty() {
        return Err("资源列表 URL 为空".to_string());
    }

    let client = Client::new();
    let resp = client
        .get(res_list_url)
        .header("User-Agent", "Mozilla/5.0")
        .send()
        .await
        .map_err(|e| format!("获取资源列表失败: {}", e))?;

    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取资源列表失败: {}", e))?;

    // 尝试解析为 JSON 数组
    if let Ok(list) = serde_json::from_str::<Vec<ResourceEntry>>(&text) {
        crate::log_info!("资源列表: {} 个文件 (JSON 数组)", list.len());
        return Ok(list);
    }

    // 尝试逐行解析 (NDJSON)
    let mut files = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(file) = serde_json::from_str::<ResourceEntry>(line) {
            files.push(file);
        }
    }

    if files.is_empty() {
        return Err("无法解析资源列表".to_string());
    }

    crate::log_info!("资源列表: {} 个文件 (NDJSON)", files.len());
    Ok(files)
}

// ============================================================
// 辅助函数
// ============================================================

/// 判断 URL 是否属于 HoYoverse
pub fn is_hoyoverse_api(url: &str) -> bool {
    url.contains("mihoyo.com") || url.contains("hoyoverse.com")
}

/// 根据游戏 preset 从配置中心读取 biz_prefix（无硬编码）
#[allow(dead_code)]
pub fn biz_prefix_for_preset(preset: &str) -> Option<String> {
    let canonical = crate::configs::game_identity::to_canonical_or_keep(preset);
    let preset = crate::configs::game_presets::get_preset(&canonical)?;
    preset.download_servers.iter().find_map(|server| {
        let biz = server.biz_prefix.trim();
        if biz.is_empty() {
            None
        } else {
            Some(biz.to_string())
        }
    })
}

/// 读取本地版本（从 .version 文件或 launcherDownloadConfig.json）
#[allow(dead_code)]
pub fn read_local_version(game_folder: &std::path::Path) -> Option<String> {
    // 优先读取 .version
    let version_file = game_folder.join(".version");
    if version_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&version_file) {
            let ver = content.trim().to_string();
            if !ver.is_empty() {
                return Some(ver);
            }
        }
    }

    // 回退到 launcherDownloadConfig.json
    let config_path = game_folder.join("launcherDownloadConfig.json");
    if config_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                return data
                    .get("version")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }
        }
    }

    None
}

/// 写入本地版本
pub fn write_local_version(game_folder: &std::path::Path, version: &str) -> Result<(), String> {
    // 写 .version
    std::fs::write(game_folder.join(".version"), version)
        .map_err(|e| format!("写入 .version 失败: {}", e))?;

    // 同时写 launcherDownloadConfig.json（兼容）
    let config = serde_json::json!({
        "version": version,
        "provider": "hoyoverse"
    });
    let config_path = game_folder.join("launcherDownloadConfig.json");
    let content =
        serde_json::to_string_pretty(&config).map_err(|e| format!("序列化版本配置失败: {}", e))?;
    std::fs::write(&config_path, content).map_err(|e| format!("写入版本配置失败: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_version_desc_prefers_newer() {
        assert_eq!(compare_version_desc("2.1.0", "2.0.9"), Ordering::Less);
        assert_eq!(compare_version_desc("1.9.9", "2.0.0"), Ordering::Greater);
        assert_eq!(compare_version_desc("3.0.0", "3.0"), Ordering::Equal);
    }

    #[test]
    fn region_hint_matches_expected_biz_suffix() {
        assert!(biz_matches_region_hint("hk4e_cn", "cn"));
        assert!(!biz_matches_region_hint("hk4e_global", "cn"));
        assert!(biz_matches_region_hint("hkrpg_global", "global"));
        assert!(biz_matches_region_hint("hkrpg_overseas", "global"));
        assert!(!biz_matches_region_hint("hkrpg_cn", "global"));
    }
}
