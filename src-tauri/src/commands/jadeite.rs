use std::path::PathBuf;
use std::time::Duration;
use tracing::{info, warn};

/// 默认 API URL 列表（按优先级尝试）
const JADEITE_API_URLS: &[&str] = &[
    "https://codeberg.org/api/v1/repos/mkrsym1/jadeite/releases/latest",
];

/// 用户自定义镜像 URL 的数据库设置 key
const JADEITE_MIRROR_KEY: &str = "jadeite.mirror_api_url";

/// 每个 URL 的最大重试次数
const MAX_RETRIES: u32 = 2;

fn build_api_urls() -> Vec<String> {
    let mut urls: Vec<String> = Vec::new();

    // 优先使用用户自定义镜像
    if let Some(mirror) = crate::configs::database::get_setting(JADEITE_MIRROR_KEY) {
        let mirror = mirror.trim().to_string();
        if !mirror.is_empty() {
            info!("[jadeite] 使用自定义镜像: {}", mirror);
            urls.push(mirror);
        }
    }

    // 追加默认 URL
    for url in JADEITE_API_URLS {
        urls.push(url.to_string());
    }
    urls
}

fn build_http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))
}

async fn fetch_release_info(
    client: &reqwest::Client,
) -> Result<serde_json::Value, String> {
    let urls = build_api_urls();
    let mut last_error = String::from("无可用 API URL");

    for url in &urls {
        for attempt in 1..=MAX_RETRIES {
            info!(
                "[jadeite] 尝试获取版本信息: {} (第 {}/{} 次)",
                url, attempt, MAX_RETRIES
            );
            match client
                .get(url)
                .header("User-Agent", "SSMT4")
                .send()
                .await
            {
                Ok(response) => match response.json::<serde_json::Value>().await {
                    Ok(data) => {
                        if data.get("tag_name").is_some() {
                            return Ok(data);
                        }
                        last_error = format!("API 返回数据缺少 tag_name: {}", url);
                        warn!("[jadeite] {}", last_error);
                    }
                    Err(e) => {
                        last_error = format!("解析响应失败 ({}): {}", url, e);
                        warn!("[jadeite] {}", last_error);
                    }
                },
                Err(e) => {
                    last_error = format!("连接失败 ({}): {}", url, e);
                    warn!("[jadeite] {}", last_error);
                }
            }

            if attempt < MAX_RETRIES {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }

    Err(format!(
        "安装 jadeite 失败: {}。\n\n\
        可能原因：网络无法访问 codeberg.org（DNS 污染或 GFW 阻断）。\n\
        解决方法：\n\
        1. 检查 DNS 设置，确保 codeberg.org 解析到正确 IP（217.197.84.140）\n\
        2. 使用代理或 VPN 后重试\n\
        3. 手动下载 jadeite 并解压到 patch 目录",
        last_error
    ))
}

/// 获取 jadeite 最新版本信息
#[tauri::command]
pub async fn get_jadeite_status(game_name: String) -> Result<serde_json::Value, String> {
    let game_name = crate::configs::game_identity::to_canonical_or_keep(&game_name);
    let patch_dir = resolve_patch_dir(&game_name)?;
    let exe_path = patch_dir.join("jadeite.exe");
    let version_path = patch_dir.join(".version");

    let installed = exe_path.exists();
    let local_version = if version_path.exists() {
        std::fs::read_to_string(&version_path).ok()
    } else {
        None
    };

    Ok(serde_json::json!({
        "installed": installed,
        "localVersion": local_version,
        "patchDir": patch_dir.to_string_lossy(),
    }))
}

/// 下载并安装最新版 jadeite
#[tauri::command]
pub async fn install_jadeite(_app: tauri::AppHandle, game_name: String) -> Result<String, String> {
    let game_name = crate::configs::game_identity::to_canonical_or_keep(&game_name);
    let patch_dir = resolve_patch_dir(&game_name)?;
    std::fs::create_dir_all(&patch_dir).map_err(|e| format!("创建 patch 目录失败: {}", e))?;

    info!("[jadeite] 正在获取最新版本...");
    let client = build_http_client()?;
    let resp = fetch_release_info(&client).await?;

    let tag = resp
        .get("tag_name")
        .and_then(|v| v.as_str())
        .ok_or("无法获取 jadeite 版本号")?;

    let download_url = resp
        .get("assets")
        .and_then(|a| a.as_array())
        .and_then(|a| a.first())
        .and_then(|a| a.get("browser_download_url"))
        .and_then(|u| u.as_str())
        .ok_or("无法获取 jadeite 下载地址")?;

    info!("[jadeite] 版本: {}, 下载: {}", tag, download_url);

    // 下载 zip（带重试）
    let zip_path = patch_dir.join("jadeite.zip");
    let mut response = None;
    for attempt in 1..=MAX_RETRIES {
        info!("[jadeite] 下载中... (第 {}/{} 次)", attempt, MAX_RETRIES);
        match client
            .get(download_url)
            .header("User-Agent", "SSMT4")
            .send()
            .await
        {
            Ok(r) => {
                response = Some(r);
                break;
            }
            Err(e) => {
                warn!("[jadeite] 下载失败 (第 {} 次): {}", attempt, e);
                if attempt == MAX_RETRIES {
                    return Err(format!("下载 jadeite 失败（已重试 {} 次）: {}", MAX_RETRIES, e));
                }
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    }
    let response = response.unwrap();

    // 流式写入临时文件，避免全量驻留内存
    {
        use futures_util::StreamExt;
        use tokio::io::AsyncWriteExt;
        let mut stream = response.bytes_stream();
        let mut file = tokio::fs::File::create(&zip_path)
            .await
            .map_err(|e| format!("创建 jadeite.zip 失败: {}", e))?;
        let mut total: u64 = 0;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("读取 jadeite 数据流失败: {}", e))?;
            file.write_all(&chunk)
                .await
                .map_err(|e| format!("写入 jadeite.zip 失败: {}", e))?;
            total += chunk.len() as u64;
        }
        file.flush()
            .await
            .map_err(|e| format!("刷新 jadeite.zip 失败: {}", e))?;
        info!("[jadeite] 下载完成 ({} bytes)，正在解压...", total);
    }

    // 解压 zip
    let file =
        std::fs::File::open(&zip_path).map_err(|e| format!("打开 jadeite.zip 失败: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("解析 jadeite.zip 失败: {}", e))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("读取 zip 条目失败: {}", e))?;

        let out_path = match entry.enclosed_name() {
            Some(p) => patch_dir.join(p),
            None => continue,
        };

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path).ok();
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let mut outfile = std::fs::File::create(&out_path)
                .map_err(|e| format!("创建文件失败 {}: {}", out_path.display(), e))?;
            std::io::copy(&mut entry, &mut outfile).map_err(|e| format!("解压文件失败: {}", e))?;
        }
    }

    // 清理 zip
    std::fs::remove_file(&zip_path).ok();

    // 写入版本号
    let version_str = tag.strip_prefix('v').unwrap_or(tag);
    std::fs::write(patch_dir.join(".version"), version_str)
        .map_err(|e| format!("写入版本号失败: {}", e))?;

    info!("[jadeite] 安装完成: {}", version_str);

    Ok(format!("jadeite {} 安装成功", version_str))
}

/// 解析 patch 目录路径（gameRoot/patch/）
///
/// 优先从 gameFolder（游戏数据子目录）的父目录推导游戏根目录，
/// 回退到 gamePath（可执行文件）向上两级推导。
pub fn resolve_patch_dir(game_name: &str) -> Result<PathBuf, String> {
    let game_name = crate::configs::game_identity::to_canonical_or_keep(game_name);
    let config_json = crate::configs::database::get_game_config(&game_name)
        .ok_or_else(|| format!("未找到游戏 {} 的配置", game_name))?;
    let data: serde_json::Value =
        serde_json::from_str(&config_json).map_err(|e| format!("解析游戏配置失败: {}", e))?;

    // 优先：gameFolder 的父目录
    if let Some(game_folder) = data
        .pointer("/other/gameFolder")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        if let Some(game_root) = PathBuf::from(game_folder).parent() {
            return Ok(game_root.join("patch"));
        }
    }

    // 回退：gamePath 向上两级（exe → 数据子目录 → 游戏根目录）
    if let Some(game_path) = data
        .pointer("/other/gamePath")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        if let Some(game_root) = PathBuf::from(game_path).parent().and_then(|p| p.parent()) {
            return Ok(game_root.join("patch"));
        }
    }

    Err("游戏配置中未设置 gameFolder 或 gamePath，无法确定 patch 目录".to_string())
}
