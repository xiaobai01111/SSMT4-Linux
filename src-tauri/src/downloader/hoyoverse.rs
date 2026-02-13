use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::info;

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
    pub size: String,
    pub decompressed_size: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioPkg {
    pub language: String,
    pub url: String,
    pub md5: String,
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
    pub file_size: u64,
}

// ============================================================
// API 请求函数
// ============================================================

/// 获取 HoYoverse 游戏包信息
/// biz_prefix: 用于匹配游戏，如 "hkrpg_" (星穹铁道), "hk4e_" (原神), "nap_" (绝区零)
pub async fn fetch_game_packages(
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

    data.data
        .game_packages
        .into_iter()
        .find(|pkg| pkg.game.biz.starts_with(biz_prefix))
        .ok_or_else(|| {
            format!(
                "API 响应中未找到游戏 (biz prefix: {})",
                biz_prefix
            )
        })
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
        info!("资源列表: {} 个文件 (JSON 数组)", list.len());
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

    info!("资源列表: {} 个文件 (NDJSON)", files.len());
    Ok(files)
}

// ============================================================
// 辅助函数
// ============================================================

/// 判断 URL 是否属于 HoYoverse
pub fn is_hoyoverse_api(url: &str) -> bool {
    url.contains("mihoyo.com") || url.contains("hoyoverse.com")
}

/// 根据游戏 preset 返回 biz_prefix
pub fn biz_prefix_for_preset(preset: &str) -> &str {
    match preset {
        "SRMI" => "hkrpg_",
        "GIMI" => "hk4e_",
        "ZZMI" => "nap_",
        "HIMI" => "bh3_",
        _ => "",
    }
}

/// 读取本地版本（从 .version 文件或 launcherDownloadConfig.json）
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
    let content = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("序列化版本配置失败: {}", e))?;
    std::fs::write(&config_path, content)
        .map_err(|e| format!("写入版本配置失败: {}", e))?;

    Ok(())
}
