use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::Manager;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInfo {
    pub name: String,
    pub display_name: String,
    pub icon_path: String,
    pub bg_path: String,
    pub bg_video_path: Option<String>,
    pub bg_type: String,
    pub show_sidebar: bool,
}

fn get_chinese_display_name(folder_name: &str) -> &str {
    match folder_name {
        "GIMI" => "原神",
        "SRMI" => "崩坏：星穹铁道",
        "ZZMI" => "绝区零",
        "WWMI" => "鸣潮",
        "WuWa" => "鸣潮 (WuWa)",
        "HIMI" => "崩坏3",
        "EFMI" => "尘白禁区",
        "AEMI" => "永劫无间",
        "GF2" => "少女前线2：追放",
        "SnowBreak" => "尘白禁区",
        "BloodySpell" => "嗜血印",
        "Nioh2" => "仁王2",
        "MiSide" => "MiSide",
        "IdentityV" => "第五人格",
        "IdentityV2" => "第五人格2",
        "AILIMIT" => "AI LIMIT",
        "DOAV" => "死或生：Venus Vacation",
        "YYSLS" => "阴阳师：神令师",
        _ => folder_name,
    }
}

#[tauri::command]
pub fn scan_games(app: tauri::AppHandle) -> Result<Vec<GameInfo>, String> {
    let games_dir = get_games_dir(&app)?;

    if !games_dir.exists() {
        return Ok(Vec::new());
    }

    let hidden_games = load_hidden_games(&games_dir);

    let mut games = Vec::new();
    let entries = std::fs::read_dir(&games_dir)
        .map_err(|e| format!("Failed to read games dir: {}", e))?;

    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        if hidden_games.contains(&name) {
            continue;
        }

        let game_path = entry.path();
        let icon_path = game_path.join("Icon.png");
        let config_path = game_path.join("Config.json");

        // Determine background type and display name from Config.json
        let mut bg_type = "Image".to_string();
        let mut display_name = String::new();
        if config_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(bt) = data.get("basic")
                        .and_then(|b| b.get("backgroundType"))
                        .and_then(|v| v.as_str())
                    {
                        bg_type = bt.to_string();
                    }
                    if let Some(dn) = data.get("DisplayName")
                        .and_then(|v| v.as_str())
                    {
                        display_name = dn.to_string();
                    }
                }
            }
        }
        // 回退：使用内置中文名映射
        if display_name.is_empty() {
            display_name = get_chinese_display_name(&name).to_string();
        }

        // Find background image (png, jpg, webp)
        let bg_path = find_background_image(&game_path)
            .unwrap_or_default();

        // Find background video (mp4, webm)
        let bg_video_path = find_background_video(&game_path);

        games.push(GameInfo {
            name: name.clone(),
            display_name,
            icon_path: if icon_path.exists() {
                icon_path.to_string_lossy().to_string()
            } else {
                String::new()
            },
            bg_path,
            bg_video_path,
            bg_type,
            show_sidebar: true,
        });
    }

    games.sort_by(|a, b| a.name.cmp(&b.name));
    info!("Scanned {} games", games.len());
    Ok(games)
}

fn find_background_image(game_dir: &PathBuf) -> Option<String> {
    for ext in &["png", "jpg", "jpeg", "webp"] {
        let path = game_dir.join(format!("Background.{}", ext));
        if path.exists() {
            return Some(path.to_string_lossy().to_string());
        }
    }
    None
}

fn find_background_video(game_dir: &PathBuf) -> Option<String> {
    for ext in &["mp4", "webm"] {
        let path = game_dir.join(format!("Background.{}", ext));
        if path.exists() {
            return Some(path.to_string_lossy().to_string());
        }
    }
    None
}

#[tauri::command]
pub fn set_game_visibility(
    app: tauri::AppHandle,
    game_name: String,
    hidden: bool,
) -> Result<(), String> {
    let games_dir = get_games_dir(&app)?;
    let hidden_path = games_dir.join("hidden_games.json");

    let mut hidden_games = load_hidden_games(&games_dir);

    if hidden {
        if !hidden_games.contains(&game_name) {
            hidden_games.push(game_name);
        }
    } else {
        hidden_games.retain(|g| g != &game_name);
    }

    let content = serde_json::to_string_pretty(&hidden_games)
        .map_err(|e| format!("Failed to serialize: {}", e))?;
    std::fs::write(&hidden_path, content)
        .map_err(|e| format!("Failed to write hidden games: {}", e))?;

    Ok(())
}

fn get_games_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to get resource dir: {}", e))?;
    let prod_path = resource_dir.join("resources").join("Games");
    if prod_path.exists() {
        return Ok(prod_path);
    }

    // 开发模式回退
    let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("Games");
    if dev_path.exists() {
        return Ok(dev_path);
    }

    Ok(prod_path)
}

fn load_hidden_games(games_dir: &PathBuf) -> Vec<String> {
    let hidden_path = games_dir.join("hidden_games.json");
    if hidden_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&hidden_path) {
            if let Ok(games) = serde_json::from_str::<Vec<String>>(&content) {
                return games;
            }
        }
    }
    Vec::new()
}
