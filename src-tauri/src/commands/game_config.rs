use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use tauri::Manager;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    #[serde(flatten)]
    pub data: Value,
}

#[tauri::command]
pub fn load_game_config(app: tauri::AppHandle, game_name: &str) -> Result<Value, String> {
    let config_path = get_game_config_path(&app, game_name)?;
    if !config_path.exists() {
        return Err(format!("Config not found for game: {}", game_name));
    }
    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse config: {}", e))
}

#[tauri::command]
pub fn save_game_config(
    app: tauri::AppHandle,
    game_name: &str,
    config: Value,
) -> Result<(), String> {
    let config_path = get_game_config_path(&app, game_name)?;
    if let Some(parent) = config_path.parent() {
        crate::utils::file_manager::ensure_dir(parent)?;
    }
    let content = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    std::fs::write(&config_path, content)
        .map_err(|e| format!("Failed to write config: {}", e))?;
    info!("Saved config for game: {}", game_name);
    Ok(())
}

#[tauri::command]
pub fn create_new_config(
    app: tauri::AppHandle,
    new_name: &str,
    config: Option<Value>,
) -> Result<(), String> {
    let game_dir = get_game_dir(&app, new_name)?;
    crate::utils::file_manager::ensure_dir(&game_dir)?;

    let config_path = game_dir.join("Config.json");
    let final_config = config.unwrap_or_else(|| serde_json::json!({
        "name": new_name,
        "gamePath": "",
        "d3dxPath": "",
        "launcherEnabled": false,
        "launcherPath": "",
    }));

    let content = serde_json::to_string_pretty(&final_config)
        .map_err(|e| format!("Failed to serialize: {}", e))?;
    std::fs::write(&config_path, content)
        .map_err(|e| format!("Failed to create config: {}", e))?;

    info!("Created new config for game: {}", new_name);
    Ok(())
}

#[tauri::command]
pub fn delete_game_config_folder(app: tauri::AppHandle, game_name: &str) -> Result<(), String> {
    let game_dir = get_game_dir(&app, game_name)?;
    if game_dir.exists() {
        std::fs::remove_dir_all(&game_dir)
            .map_err(|e| format!("Failed to delete game folder: {}", e))?;
        info!("Deleted config folder for game: {}", game_name);
    }
    Ok(())
}

#[tauri::command]
pub fn set_game_icon(
    app: tauri::AppHandle,
    game_name: &str,
    file_path: &str,
) -> Result<String, String> {
    let game_dir = get_game_dir(&app, game_name)?;
    let dest = game_dir.join("Icon.png");
    std::fs::copy(file_path, &dest)
        .map_err(|e| format!("Failed to copy icon: {}", e))?;
    Ok(dest.to_string_lossy().to_string())
}

#[tauri::command]
pub fn set_game_background(
    app: tauri::AppHandle,
    game_name: &str,
    file_path: &str,
    bg_type: Option<String>,
) -> Result<String, String> {
    let _ = bg_type; // 保留参数兼容前端，暂不使用
    let game_dir = get_game_dir(&app, game_name)?;
    let ext = std::path::Path::new(file_path)
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let dest = game_dir.join(format!("Background.{}", ext));
    std::fs::copy(file_path, &dest)
        .map_err(|e| format!("Failed to copy background: {}", e))?;
    Ok(dest.to_string_lossy().to_string())
}

#[tauri::command]
pub fn update_game_background(
    app: tauri::AppHandle,
    game_name: &str,
    game_preset: &str,
    bg_type: Option<String>,
) -> Result<String, String> {
    let _ = bg_type;
    // 从预设游戏的资源目录中查找默认背景
    let preset_dir = get_game_dir(&app, game_preset)?;
    let bg_extensions = ["png", "jpg", "jpeg", "webp", "mp4", "webm"];
    for ext in &bg_extensions {
        let src = preset_dir.join(format!("Background.{}", ext));
        if src.exists() {
            let dest_dir = get_game_dir(&app, game_name)?;
            let dest = dest_dir.join(format!("Background.{}", ext));
            std::fs::copy(&src, &dest)
                .map_err(|e| format!("Failed to copy background: {}", e))?;
            return Ok(dest.to_string_lossy().to_string());
        }
    }
    Err(format!("No default background found for preset: {}", game_preset))
}

#[tauri::command]
pub async fn get_3dmigoto_latest_release(game_preset: String) -> Result<Value, String> {
    let repo_url = match game_preset.as_str() {
        "SRMI" => "https://api.github.com/repos/SilentNightSound/SR-Model-Importer/releases/latest",
        "WWMI" => "https://api.github.com/repos/SpectrumQT/WWMI/releases/latest",
        "ZZMI" => "https://api.github.com/repos/leotorrez/ZZ-Model-Importer/releases/latest",
        "HIMI" => "https://api.github.com/repos/SilentNightSound/HI-Model-Importer/releases/latest",
        _ => "https://api.github.com/repos/SilentNightSound/GI-Model-Importer/releases/latest",
    };

    let client = reqwest::Client::new();
    let resp = client
        .get(repo_url)
        .header("User-Agent", "SSMT4/0.1")
        .send()
        .await
        .map_err(|e| format!("GitHub API request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub API returned {}", resp.status()));
    }

    let data: Value = resp.json()
        .await
        .map_err(|e| format!("Failed to parse GitHub response: {}", e))?;

    // 从 GitHub Release API 响应中提取字段，构造前端期望的 UpdateInfo
    let version = data.get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let description = data.get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let download_url = data.get("assets")
        .and_then(|a| a.as_array())
        .and_then(|arr| arr.first())
        .and_then(|asset| asset.get("browser_download_url"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    Ok(serde_json::json!({
        "version": version,
        "description": description,
        "downloadUrl": download_url
    }))
}

#[tauri::command]
pub async fn install_3dmigoto_update(
    app: tauri::AppHandle,
    download_url: String,
    game_name: String,
) -> Result<String, String> {
    let game_dir = get_game_dir(&app, &game_name)?;
    let cache_dir = crate::configs::app_config::get_app_cache_dir();
    crate::utils::file_manager::ensure_dir(&cache_dir)?;

    let zip_path = cache_dir.join("3dmigoto_update.zip");

    // Download
    let client = reqwest::Client::new();
    let resp = client
        .get(&download_url)
        .header("User-Agent", "SSMT4/0.1")
        .send()
        .await
        .map_err(|e| format!("Download failed: {}", e))?;

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Failed to read download: {}", e))?;

    std::fs::write(&zip_path, &bytes)
        .map_err(|e| format!("Failed to save zip: {}", e))?;

    // Extract
    let file = std::fs::File::open(&zip_path)
        .map_err(|e| format!("Failed to open zip: {}", e))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Failed to read zip: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {}", e))?;
        let name = file.name().to_string();

        if name.ends_with('/') {
            let dir = game_dir.join(&name);
            std::fs::create_dir_all(&dir).ok();
        } else {
            let dest = game_dir.join(&name);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let mut out = std::fs::File::create(&dest)
                .map_err(|e| format!("Failed to create file: {}", e))?;
            std::io::copy(&mut file, &mut out)
                .map_err(|e| format!("Failed to extract file: {}", e))?;
        }
    }

    // Cleanup
    std::fs::remove_file(&zip_path).ok();
    info!("Installed 3Dmigoto update for game: {}", game_name);
    Ok("Update installed".to_string())
}

fn get_game_dir(app: &tauri::AppHandle, game_name: &str) -> Result<PathBuf, String> {
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to get resource dir: {}", e))?;
    let prod_path = resource_dir.join("resources").join("Games").join(game_name);
    if prod_path.exists() {
        return Ok(prod_path);
    }

    // 开发模式回退
    let dev_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("Games")
        .join(game_name);
    if dev_path.exists() {
        return Ok(dev_path);
    }

    Ok(prod_path)
}

fn get_game_config_path(app: &tauri::AppHandle, game_name: &str) -> Result<PathBuf, String> {
    Ok(get_game_dir(app, game_name)?.join("Config.json"))
}
