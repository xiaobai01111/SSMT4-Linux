use std::path::PathBuf;
use tracing::info;

const JADEITE_API_URL: &str =
    "https://codeberg.org/api/v1/repos/mkrsym1/jadeite/releases/latest";

/// 获取 jadeite 最新版本信息
#[tauri::command]
pub async fn get_jadeite_status(game_name: String) -> Result<serde_json::Value, String> {
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
    let patch_dir = resolve_patch_dir(&game_name)?;
    std::fs::create_dir_all(&patch_dir)
        .map_err(|e| format!("创建 patch 目录失败: {}", e))?;

    // 获取最新 release 信息
    info!("[jadeite] 正在获取最新版本...");
    let client = reqwest::Client::new();
    let resp: serde_json::Value = client
        .get(JADEITE_API_URL)
        .header("User-Agent", "SSMT4")
        .send()
        .await
        .map_err(|e| format!("请求 jadeite API 失败: {}", e))?
        .json()
        .await
        .map_err(|e| format!("解析 jadeite API 响应失败: {}", e))?;

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

    // 下载 zip
    let zip_path = patch_dir.join("jadeite.zip");
    let response = client
        .get(download_url)
        .header("User-Agent", "SSMT4")
        .send()
        .await
        .map_err(|e| format!("下载 jadeite 失败: {}", e))?;

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
            file.write_all(&chunk).await
                .map_err(|e| format!("写入 jadeite.zip 失败: {}", e))?;
            total += chunk.len() as u64;
        }
        file.flush().await.map_err(|e| format!("刷新 jadeite.zip 失败: {}", e))?;
        info!("[jadeite] 下载完成 ({} bytes)，正在解压...", total);
    }

    // 解压 zip
    let file = std::fs::File::open(&zip_path)
        .map_err(|e| format!("打开 jadeite.zip 失败: {}", e))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("解析 jadeite.zip 失败: {}", e))?;

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
            std::io::copy(&mut entry, &mut outfile)
                .map_err(|e| format!("解压文件失败: {}", e))?;
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

/// 解析 patch 目录路径（游戏根目录/patch/）
fn resolve_patch_dir(game_name: &str) -> Result<PathBuf, String> {
    let config_json = crate::configs::database::get_game_config(game_name)
        .ok_or_else(|| format!("未找到游戏 {} 的配置", game_name))?;
    let data: serde_json::Value = serde_json::from_str(&config_json)
        .map_err(|e| format!("解析游戏配置失败: {}", e))?;
    let game_folder = data
        .pointer("/other/gameFolder")
        .and_then(|v| v.as_str())
        .ok_or("游戏配置中未设置 gameFolder")?;

    let game_folder_path = PathBuf::from(game_folder);
    let game_root = game_folder_path
        .parent()
        .ok_or("无法获取游戏根目录")?;

    Ok(game_root.join("patch"))
}
