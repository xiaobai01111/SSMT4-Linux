use crate::configs::database as db;
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
    // 优先从 SQLite 读取
    if let Some(json_str) = db::get_game_config(game_name) {
        return serde_json::from_str(&json_str).map_err(|e| format!("解析游戏配置失败: {}", e));
    }

    // 回退到文件系统（兼容资源目录中的 Config.json）
    let config_path = get_game_config_path(&app, game_name)?;
    if !config_path.exists() {
        return Err(format!("Config not found for game: {}", game_name));
    }
    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config: {}", e))?;
    let val: Value =
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse config: {}", e))?;

    // 迁移到 SQLite
    db::set_game_config(game_name, &content);
    info!("从文件迁移游戏配置到 SQLite: {}", game_name);
    Ok(val)
}

#[tauri::command]
pub fn save_game_config(
    _app: tauri::AppHandle,
    game_name: &str,
    config: Value,
) -> Result<(), String> {
    let content = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    db::set_game_config(game_name, &content);
    info!("Saved config for game: {}", game_name);
    Ok(())
}

#[tauri::command]
pub fn create_new_config(
    app: tauri::AppHandle,
    new_name: &str,
    config: Option<Value>,
) -> Result<(), String> {
    let game_dir = get_writable_game_dir(&app, new_name)?;
    crate::utils::file_manager::ensure_dir(&game_dir)?;

    let config_path = game_dir.join("Config.json");
    let final_config = config.unwrap_or_else(|| {
        serde_json::json!({
            "name": new_name,
            "gamePath": "",
            "d3dxPath": "",
            "launcherEnabled": false,
            "launcherPath": "",
        })
    });

    let content = serde_json::to_string_pretty(&final_config)
        .map_err(|e| format!("Failed to serialize: {}", e))?;
    std::fs::write(&config_path, content).map_err(|e| format!("Failed to create config: {}", e))?;

    info!("Created new config for game: {}", new_name);
    Ok(())
}

#[tauri::command]
pub fn delete_game_config_folder(app: tauri::AppHandle, game_name: &str) -> Result<(), String> {
    let user_games_dir = get_user_games_dir()?;
    if let Some(game_dir) = find_game_dir_by_logic_name(&user_games_dir, game_name) {
        if game_dir.exists() {
            std::fs::remove_dir_all(&game_dir)
                .map_err(|e| format!("Failed to delete game folder: {}", e))?;
            info!("Deleted config folder for game: {}", game_name);
        }
    }
    // 同时清理 SQLite 中的游戏配置
    crate::configs::database::delete_game_config(game_name);
    // 从 hidden_games.json 中移除
    let _ = super::game_scanner::set_game_visibility(app, game_name.to_string(), false);
    Ok(())
}

#[tauri::command]
pub fn reset_game_background(app: tauri::AppHandle, game_name: &str) -> Result<(), String> {
    let game_dir = get_writable_game_dir(&app, game_name)?;
    let bg_extensions = ["png", "jpg", "jpeg", "webp", "mp4", "webm", "ogg", "mov"];
    for ext in &bg_extensions {
        let path = game_dir.join(format!("Background.{}", ext));
        if path.exists() {
            std::fs::remove_file(&path).ok();
            info!("Removed custom background: {}", path.display());
        }
    }
    Ok(())
}

#[tauri::command]
pub fn set_game_icon(
    app: tauri::AppHandle,
    game_name: &str,
    file_path: &str,
) -> Result<String, String> {
    let game_dir = get_writable_game_dir(&app, game_name)?;
    let dest = game_dir.join("Icon.png");
    std::fs::copy(file_path, &dest).map_err(|e| format!("Failed to copy icon: {}", e))?;
    Ok(dest.to_string_lossy().to_string())
}

#[tauri::command]
pub fn set_game_background(
    app: tauri::AppHandle,
    game_name: &str,
    file_path: &str,
    bg_type: Option<String>,
) -> Result<String, String> {
    let game_dir = get_writable_game_dir(&app, game_name)?;
    let ext = std::path::Path::new(file_path)
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let dest = game_dir.join(format!("Background.{}", ext));
    std::fs::copy(file_path, &dest).map_err(|e| format!("Failed to copy background: {}", e))?;

    // 同步更新 Config.json 中的 backgroundType
    if let Some(bt) = &bg_type {
        let mut config = load_game_config(app.clone(), game_name).unwrap_or(serde_json::json!({}));
        if let Some(basic) = config.get_mut("basic") {
            basic
                .as_object_mut()
                .map(|obj| obj.insert("backgroundType".to_string(), serde_json::json!(bt)));
        } else {
            config.as_object_mut().map(|obj| {
                obj.insert(
                    "basic".to_string(),
                    serde_json::json!({"backgroundType": bt}),
                )
            });
        }
        let content = serde_json::to_string_pretty(&config).unwrap_or_default();
        db::set_game_config(game_name, &content);
        info!("Updated backgroundType to {} for {}", bt, game_name);
    }

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
            let dest_dir = get_writable_game_dir(&app, game_name)?;
            let dest = dest_dir.join(format!("Background.{}", ext));
            std::fs::copy(&src, &dest).map_err(|e| format!("Failed to copy background: {}", e))?;
            return Ok(dest.to_string_lossy().to_string());
        }
    }
    Err(format!(
        "No default background found for preset: {}",
        game_preset
    ))
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

    let data: Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse GitHub response: {}", e))?;

    // 从 GitHub Release API 响应中提取字段，构造前端期望的 UpdateInfo
    let version = data
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let description = data
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let download_url = data
        .get("assets")
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
    let game_dir = get_writable_game_dir(&app, &game_name)?;
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

    std::fs::write(&zip_path, &bytes).map_err(|e| format!("Failed to save zip: {}", e))?;

    // Extract
    let file = std::fs::File::open(&zip_path).map_err(|e| format!("Failed to open zip: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Failed to read zip: {}", e))?;

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

fn get_user_games_dir() -> Result<PathBuf, String> {
    let games_dir = crate::utils::file_manager::get_global_games_dir();
    crate::utils::file_manager::ensure_dir(&games_dir)?;
    Ok(games_dir)
}

fn get_resource_games_dirs(app: &tauri::AppHandle) -> Result<Vec<PathBuf>, String> {
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to get resource dir: {}", e))?;

    let mut candidates = Vec::new();
    let prod = resource_dir.join("resources").join("Games");
    if prod.exists() {
        candidates.push(prod);
    }

    let dev = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("Games");
    if dev.exists() {
        candidates.push(dev);
    }

    Ok(candidates)
}

fn find_game_dir_by_logic_name(games_dir: &std::path::Path, game_name: &str) -> Option<PathBuf> {
    let direct = games_dir.join(game_name);
    if direct.exists() {
        return Some(direct);
    }

    if let Ok(entries) = std::fs::read_dir(games_dir) {
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let config_path = entry.path().join("Config.json");
            if !config_path.exists() {
                continue;
            }

            if let Ok(content) = std::fs::read_to_string(&config_path) {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                    let logic_name = data
                        .get("LogicName")
                        .or_else(|| data.get("GamePreset"))
                        .and_then(|v| v.as_str());
                    if logic_name == Some(game_name) {
                        return Some(entry.path());
                    }
                }
            }
        }
    }

    None
}

fn find_game_dir_in_candidates(candidates: &[PathBuf], game_name: &str) -> Option<PathBuf> {
    for games_dir in candidates {
        if let Some(found) = find_game_dir_by_logic_name(games_dir, game_name) {
            return Some(found);
        }
    }
    None
}

fn get_game_dir(app: &tauri::AppHandle, game_name: &str) -> Result<PathBuf, String> {
    let user_games_dir = get_user_games_dir()?;
    if let Some(found) = find_game_dir_by_logic_name(&user_games_dir, game_name) {
        return Ok(found);
    }

    let resource_dirs = get_resource_games_dirs(app)?;
    if let Some(found) = find_game_dir_in_candidates(&resource_dirs, game_name) {
        return Ok(found);
    }

    Ok(user_games_dir.join(game_name))
}

fn get_writable_game_dir(app: &tauri::AppHandle, game_name: &str) -> Result<PathBuf, String> {
    let user_games_dir = get_user_games_dir()?;

    if let Some(found) = find_game_dir_by_logic_name(&user_games_dir, game_name) {
        return Ok(found);
    }

    let resource_dirs = get_resource_games_dirs(app)?;
    if let Some(src_dir) = find_game_dir_in_candidates(&resource_dirs, game_name) {
        let folder_name = src_dir
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| game_name.to_string());
        let dst_dir = user_games_dir.join(folder_name);

        if !dst_dir.exists() {
            crate::utils::file_manager::copy_dir_recursive(&src_dir, &dst_dir)?;
            info!(
                "Copied game resources to writable dir: {} -> {}",
                src_dir.display(),
                dst_dir.display()
            );
        }

        return Ok(dst_dir);
    }

    let dst_dir = user_games_dir.join(game_name);
    crate::utils::file_manager::ensure_dir(&dst_dir)?;
    Ok(dst_dir)
}

fn get_game_config_path(app: &tauri::AppHandle, game_name: &str) -> Result<PathBuf, String> {
    Ok(get_game_dir(app, game_name)?.join("Config.json"))
}
