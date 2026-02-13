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
        "HIMI" => "崩坏3",
        "EFMI" => "尘白禁区",
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
    let mut all_dir_names: Vec<String> = Vec::new();
    let entries = std::fs::read_dir(&games_dir)
        .map_err(|e| format!("Failed to read games dir: {}", e))?;

    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }

        let folder_name = entry.file_name().to_string_lossy().to_string();
        let game_path = entry.path();
        let icon_path = game_path.join("Icon.png");
        let config_path = game_path.join("Config.json");

        // 从文件系统 Config.json 读取 LogicName 作为游戏标识
        let fs_config: Option<serde_json::Value> = if config_path.exists() {
            std::fs::read_to_string(&config_path).ok()
                .and_then(|s| serde_json::from_str(&s).ok())
        } else {
            None
        };

        // game_id = LogicName || GamePreset || 文件夹名
        let game_id = fs_config.as_ref()
            .and_then(|v| v.get("LogicName").or_else(|| v.get("GamePreset")))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| folder_name.clone());

        all_dir_names.push(game_id.clone());
        let is_hidden = hidden_games.contains(&game_id);

        // 优先从 SQLite 读取（用户修改保存在此），回退到文件系统 Config.json
        let mut bg_type = "Image".to_string();
        let mut display_name = String::new();

        let config_data: Option<serde_json::Value> = crate::configs::database::get_game_config(&game_id)
            .and_then(|s| serde_json::from_str(&s).ok())
            .or_else(|| fs_config.clone());

        if let Some(data) = &config_data {
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
        // 回退：使用内置中文名映射
        if display_name.is_empty() {
            display_name = get_chinese_display_name(&game_id).to_string();
        }

        // 首次运行：如果 SQLite 中没有该游戏配置，从文件迁移
        if crate::configs::database::get_game_config(&game_id).is_none() {
            if let Some(ref data) = fs_config {
                let content = serde_json::to_string_pretty(data).unwrap_or_default();
                crate::configs::database::set_game_config(&game_id, &content);
                info!("从文件迁移游戏配置到 SQLite: {}", game_id);
            }
        }

        // Find background image (png, jpg, webp)
        let bg_path = find_background_image(&game_path)
            .unwrap_or_default();

        // Find background video (mp4, webm)
        let bg_video_path = find_background_video(&game_path);

        games.push(GameInfo {
            name: game_id.clone(),
            display_name,
            icon_path: if icon_path.exists() {
                icon_path.to_string_lossy().to_string()
            } else {
                String::new()
            },
            bg_path,
            bg_video_path,
            bg_type,
            show_sidebar: !is_hidden,
        });
    }

    games.sort_by(|a, b| a.name.cmp(&b.name));

    // 同步 SQLite：仅清理已不存在的游戏配置
    let db_names = crate::configs::database::list_game_names();
    for db_name in &db_names {
        if !all_dir_names.contains(db_name) {
            crate::configs::database::delete_game_config(db_name);
            info!("已清理过期游戏配置: {}", db_name);
        }
    }

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

// ============================================================
// 游戏配置模板（独立文件夹，不随软件打包）
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameTemplateInfo {
    /// 模板文件夹名（英文游戏名，如 GenshinImpact）
    pub name: String,
    /// 游戏代码名（从 Config.json 的 LogicName 读取，如 GIMI）
    pub game_id: String,
    pub display_name: String,
    pub icon_path: String,
    pub has_icon: bool,
    /// 该游戏在 Games 目录中是否已存在
    pub already_exists: bool,
}

/// 获取模板文件夹路径（数据目录/GameTemplates）
fn get_templates_dir() -> PathBuf {
    let data_dir = crate::configs::app_config::get_app_data_dir();
    data_dir.join("GameTemplates")
}

/// 返回模板文件夹路径，前端可用于打开文件夹
#[tauri::command]
pub fn get_game_templates_dir() -> Result<String, String> {
    let dir = get_templates_dir();
    crate::utils::file_manager::ensure_dir(&dir)
        .map_err(|e| format!("创建模板文件夹失败: {}", e))?;
    Ok(dir.to_string_lossy().to_string())
}

/// 扫描可用的游戏配置模板
#[tauri::command]
pub fn list_game_templates(app: tauri::AppHandle) -> Result<Vec<GameTemplateInfo>, String> {
    let templates_dir = get_templates_dir();
    if !templates_dir.exists() {
        crate::utils::file_manager::ensure_dir(&templates_dir).ok();
        return Ok(Vec::new());
    }

    let games_dir = get_games_dir(&app)?;

    let entries = std::fs::read_dir(&templates_dir)
        .map_err(|e| format!("读取模板目录失败: {}", e))?;

    let mut templates = Vec::new();
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let config_path = entry.path().join("Config.json");
        if !config_path.exists() {
            continue;
        }

        let icon_path = entry.path().join("Icon.png");
        let has_icon = icon_path.exists();

        // 读取 Config.json 获取 gameId 和 displayName
        let config_data: Option<serde_json::Value> = std::fs::read_to_string(&config_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok());

        // gameId = LogicName || GamePreset || 文件夹名
        let game_id = config_data.as_ref()
            .and_then(|v| v.get("LogicName").or_else(|| v.get("GamePreset")))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| name.clone());

        let display_name = config_data.as_ref()
            .and_then(|v| v.get("DisplayName").and_then(|d| d.as_str()).map(|s| s.to_string()))
            .unwrap_or_else(|| get_chinese_display_name(&game_id).to_string());

        // 用 gameId 检查 Games 目录下是否已存在
        let already_exists = games_dir.join(&game_id).exists();

        templates.push(GameTemplateInfo {
            name,
            game_id,
            display_name,
            icon_path: if has_icon {
                icon_path.to_string_lossy().to_string()
            } else {
                String::new()
            },
            has_icon,
            already_exists,
        });
    }

    templates.sort_by(|a, b| a.name.cmp(&b.name));
    info!("扫描到 {} 个游戏配置模板", templates.len());
    Ok(templates)
}

/// 导入游戏配置模板到 Games 目录
#[tauri::command]
pub fn import_game_template(
    app: tauri::AppHandle,
    template_name: String,
    overwrite: bool,
) -> Result<(), String> {
    let templates_dir = get_templates_dir();
    let template_dir = templates_dir.join(&template_name);
    if !template_dir.exists() {
        return Err(format!("模板不存在: {}", template_name));
    }

    // 从 Config.json 读取 LogicName 作为 Games 目录下的实际文件夹名
    let config_path = template_dir.join("Config.json");
    let game_id = std::fs::read_to_string(&config_path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("LogicName").or_else(|| v.get("GamePreset")).and_then(|x| x.as_str()).map(|s| s.to_string()))
        .unwrap_or_else(|| template_name.clone());

    let games_dir = get_games_dir(&app)?;
    let target_dir = games_dir.join(&game_id);

    if target_dir.exists() && !overwrite {
        return Err(format!("游戏已存在: {}", game_id));
    }

    if target_dir.exists() {
        std::fs::remove_dir_all(&target_dir)
            .map_err(|e| format!("删除旧游戏目录失败: {}", e))?;
    }

    // 递归复制模板目录
    copy_dir_recursive(&template_dir, &target_dir)?;

    info!("已导入游戏配置模板: {} ({}) -> {}", template_name, game_id, target_dir.display());
    Ok(())
}

/// 递归复制目录
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dst)
        .map_err(|e| format!("创建目录失败: {}", e))?;

    for entry in std::fs::read_dir(src).map_err(|e| format!("读取目录失败: {}", e))?.flatten() {
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)
                .map_err(|e| format!("复制文件失败: {}", e))?;
        }
    }
    Ok(())
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
