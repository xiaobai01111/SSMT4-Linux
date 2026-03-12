use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tracing::{debug, info};

use crate::utils::file_manager::{copy_dir_recursive, remove_dir_if_exists};
use crate::utils::game_dirs::{
    extract_game_id_from_config, find_game_dir_by_logic_name, get_resource_games_dirs,
    get_user_games_dir,
};

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

fn is_readable_file(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|m| m.is_file())
        .unwrap_or(false)
        && std::fs::File::open(path).is_ok()
}

fn get_default_display_name(game_key: &str) -> String {
    crate::configs::game_identity::display_name_en_for_key(game_key)
        .unwrap_or_else(|| game_key.to_string())
}

fn is_infrastructure_dir_name(folder_name: &str) -> bool {
    let normalized = folder_name.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return true;
    }
    if normalized.starts_with('.') {
        return true;
    }
    matches!(
        normalized.as_str(),
        "prefix"
            | "prefixes"
            | "tools"
            | "tool"
            | "cache"
            | "caches"
            | "logs"
            | "log"
            | "tmp"
            | "temp"
            | "data-parameters"
            | "gametemplates"
            | "_templates"
            | "downloads"
    )
}

fn should_include_game_dir(folder_name: &str, fs_config: Option<&serde_json::Value>) -> bool {
    if is_infrastructure_dir_name(folder_name) {
        return false;
    }

    // 有 Config 且可提取游戏标识：一律视为有效游戏目录（含自定义游戏）
    if let Some(config) = fs_config {
        if extract_game_id_from_config(config).is_some() {
            return true;
        }
    }

    // 无 Config 时，仅识别为已知游戏预设（避免把 prefix 等运行目录当游戏）
    let canonical = crate::configs::game_identity::to_canonical_or_keep(folder_name);
    crate::configs::game_presets::get_preset(&canonical).is_some()
}

#[tauri::command]
pub fn scan_games(app: tauri::AppHandle) -> Result<Vec<GameInfo>, String> {
    let user_games_dir = get_user_games_dir()?;
    let hidden_games = load_hidden_games(&user_games_dir);

    let mut scan_dirs = vec![user_games_dir.clone()];
    for resource_games_dir in get_resource_games_dirs(&app)? {
        if resource_games_dir != user_games_dir && !scan_dirs.contains(&resource_games_dir) {
            scan_dirs.push(resource_games_dir);
        }
    }

    // game_id -> (用户目录路径, 资源目录路径)
    let mut game_user_paths: HashMap<String, PathBuf> = HashMap::new();
    let mut game_resource_paths: HashMap<String, PathBuf> = HashMap::new();
    let mut user_game_ids: HashSet<String> = HashSet::new();
    let mut resource_game_ids: HashSet<String> = HashSet::new();
    let mut skipped_dir_count: usize = 0;

    for (dir_idx, games_dir) in scan_dirs.iter().enumerate() {
        if !games_dir.exists() {
            continue;
        }

        let mut entries: Vec<_> = std::fs::read_dir(games_dir)
            .map_err(|e| format!("Failed to read games dir: {}", e))?
            .filter_map(|e| e.ok())
            .collect();
        // 稳定排序，避免 read_dir 无序导致偶发不一致
        entries.sort_by_key(|e| {
            e.file_name()
                .to_string_lossy()
                .to_string()
                .to_ascii_lowercase()
        });

        for entry in entries {
            if !entry.path().is_dir() {
                continue;
            }

            let folder_name = entry.file_name().to_string_lossy().to_string();
            let game_path = entry.path();
            let config_path = game_path.join("Config.json");

            // 从文件系统 Config.json 读取 LogicName 作为游戏标识
            let fs_config: Option<serde_json::Value> = if config_path.exists() {
                std::fs::read_to_string(&config_path)
                    .ok()
                    .and_then(|s| serde_json::from_str(&s).ok())
            } else {
                None
            };

            if !should_include_game_dir(&folder_name, fs_config.as_ref()) {
                skipped_dir_count += 1;
                debug!(
                    "跳过非游戏目录: 目录={}, 来源={}",
                    folder_name,
                    if dir_idx == 0 {
                        "用户目录"
                    } else {
                        "资源目录"
                    }
                );
                continue;
            }

            // game_id = LogicName || GamePreset || 文件夹名
            let game_id = fs_config
                .as_ref()
                .and_then(extract_game_id_from_config)
                .unwrap_or_else(|| {
                    crate::configs::game_identity::to_canonical_or_keep(&folder_name)
                });

            if dir_idx == 0 {
                // 用户目录
                user_game_ids.insert(game_id.clone());
                game_user_paths.entry(game_id).or_insert(game_path);
            } else {
                // 资源目录
                resource_game_ids.insert(game_id.clone());
                game_resource_paths.entry(game_id).or_insert(game_path);
            }
        }
    }

    // 显示“全部支持的游戏”：
    // - 优先使用资源目录（data-linux，旧名 Data-parameters）作为支持列表
    // - 当资源目录不可用时，回退到用户目录
    let final_game_ids: HashSet<String> = if !resource_game_ids.is_empty() {
        resource_game_ids.clone()
    } else {
        user_game_ids.clone()
    };

    // 合并路径：用户目录优先（用于覆盖图标/背景/配置），资源目录回退
    let mut game_paths: HashMap<String, PathBuf> = HashMap::new();
    for game_id in &final_game_ids {
        if let Some(path) = game_user_paths.get(game_id) {
            game_paths.insert(game_id.clone(), path.clone());
        } else if let Some(path) = game_resource_paths.get(game_id) {
            game_paths.insert(game_id.clone(), path.clone());
        }
    }

    let mut games = Vec::new();
    for (game_id, game_path) in game_paths {
        let is_hidden = hidden_games.contains(&game_id);
        let resource_path = game_resource_paths.get(&game_id);

        // Icon: 用户目录优先，资源目录回退
        let icon_path = {
            let user_icon = game_path.join("Icon.png");
            if is_readable_file(&user_icon) {
                user_icon
            } else if let Some(rp) = resource_path {
                let res_icon = rp.join("Icon.png");
                if is_readable_file(&res_icon) {
                    res_icon
                } else {
                    user_icon
                }
            } else {
                user_icon
            }
        };

        // Config: 用户目录优先，资源目录回退
        let config_path = {
            let user_cfg = game_path.join("Config.json");
            if is_readable_file(&user_cfg) {
                user_cfg
            } else if let Some(rp) = resource_path {
                let res_cfg = rp.join("Config.json");
                if is_readable_file(&res_cfg) {
                    res_cfg
                } else {
                    user_cfg
                }
            } else {
                user_cfg
            }
        };

        // 从文件系统读取用于回退和首轮迁移
        let fs_config: Option<serde_json::Value> = if config_path.exists() {
            std::fs::read_to_string(&config_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
        } else {
            None
        };

        // 优先从 SQLite 读取（用户修改保存在此），回退到文件系统 Config.json
        let mut bg_type = "Image".to_string();
        let mut display_name = String::new();
        let config_data: Option<serde_json::Value> =
            crate::configs::database::get_game_config(&game_id)
                .and_then(|s| serde_json::from_str(&s).ok())
                .or_else(|| fs_config.clone());

        if let Some(data) = &config_data {
            if let Some(bt) = data
                .get("basic")
                .and_then(|b| b.get("backgroundType"))
                .and_then(|v| v.as_str())
            {
                bg_type = bt.to_string();
            }
            if let Some(dn) = data.get("DisplayName").and_then(|v| v.as_str()) {
                display_name = dn.to_string();
            }
        }
        // 回退：使用内置中文名映射
        if display_name.is_empty() {
            display_name = get_default_display_name(&game_id);
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
        let bg_path = find_background_image(&game_path).unwrap_or_default();

        // Find background video (mp4, webm)
        let bg_video_path = find_background_video(&game_path);

        // 动态放行资源路径，确保前端 convertFileSrc 可访问
        let icon_str = if is_readable_file(&icon_path) {
            crate::commands::common::allow_asset_file(&app, &icon_path)
        } else {
            String::new()
        };
        let bg_str = if !bg_path.is_empty() {
            crate::commands::common::allow_asset_file(&app, Path::new(&bg_path))
        } else {
            String::new()
        };
        let bg_video_str = bg_video_path
            .as_deref()
            .map(|p| crate::commands::common::allow_asset_file(&app, Path::new(p)));

        games.push(GameInfo {
            name: game_id.clone(),
            display_name,
            icon_path: icon_str,
            bg_path: bg_str,
            bg_video_path: bg_video_str,
            bg_type,
            show_sidebar: !is_hidden,
        });
    }

    games.sort_by(|a, b| a.name.cmp(&b.name));

    // 同步 SQLite：仅清理已不存在的游戏配置
    let db_names = crate::configs::database::list_game_names();
    for db_name in &db_names {
        if !final_game_ids.contains(db_name.as_str()) {
            crate::configs::database::delete_game_config(db_name);
            crate::configs::database::delete_game_config_v2(db_name);
            info!("已清理过期游戏配置: {}", db_name);
        }
    }

    let installed_count = final_game_ids
        .iter()
        .filter(|id| game_user_paths.contains_key(*id))
        .count();
    let resource_fallback_count = final_game_ids
        .iter()
        .filter(|id| game_resource_paths.contains_key(*id))
        .count();
    let resource_only_template_count = final_game_ids
        .iter()
        .filter(|id| !game_user_paths.contains_key(*id))
        .count();
    info!(
        "游戏扫描完成: 支持总数={}, 已安装={}, 资源回退匹配={}, 仅资源模板={}, 忽略目录={}, 返回={}",
        final_game_ids.len(),
        installed_count,
        resource_fallback_count,
        resource_only_template_count,
        skipped_dir_count,
        games.len()
    );
    Ok(games)
}

fn find_background_image(game_dir: &Path) -> Option<String> {
    for ext in &["png", "jpg", "jpeg", "webp"] {
        let path = game_dir.join(format!("Background.{}", ext));
        if is_readable_file(&path) {
            return Some(path.to_string_lossy().to_string());
        }
    }
    None
}

fn find_background_video(game_dir: &Path) -> Option<String> {
    for ext in &["mp4", "webm"] {
        let path = game_dir.join(format!("Background.{}", ext));
        if is_readable_file(&path) {
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
    let _ = app;
    let game_name = crate::configs::game_identity::to_canonical_or_keep(&game_name);
    let games_dir = get_user_games_dir()?;
    let hidden_path = games_dir.join("hidden_games.json");

    let mut hidden_games = load_hidden_games(&games_dir);

    if hidden {
        if !hidden_games
            .iter()
            .any(|item| item.eq_ignore_ascii_case(&game_name))
        {
            hidden_games.push(game_name);
        }
    } else {
        hidden_games.retain(|g| !g.eq_ignore_ascii_case(&game_name));
    }

    let content = serde_json::to_string_pretty(&hidden_games)
        .map_err(|e| format!("Failed to serialize: {}", e))?;
    std::fs::write(&hidden_path, content)
        .map_err(|e| format!("Failed to write hidden games: {}", e))?;

    Ok(())
}

// ============================================================
// 游戏配置模板（独立文件夹，不随软件打包）
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameTemplateInfo {
    /// 模板文件夹名（英文游戏名，如 WutheringWaves）
    pub name: String,
    /// 游戏代码名（从 Config.json 的 LogicName 读取，如 WutheringWaves）
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

    let games_dir = get_user_games_dir()?;

    let entries =
        std::fs::read_dir(&templates_dir).map_err(|e| format!("读取模板目录失败: {}", e))?;

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
        let has_icon = is_readable_file(&icon_path);

        // 读取 Config.json 获取 gameId 和 displayName
        let config_data: Option<serde_json::Value> = std::fs::read_to_string(&config_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok());

        // gameId = LogicName || GamePreset || 文件夹名
        let game_id = config_data
            .as_ref()
            .and_then(extract_game_id_from_config)
            .unwrap_or_else(|| crate::configs::game_identity::to_canonical_or_keep(&name));

        let display_name = config_data
            .as_ref()
            .and_then(|v| {
                v.get("DisplayName")
                    .and_then(|d| d.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| get_default_display_name(&game_id));

        // 用 gameId 检查 Games 目录下是否已存在
        let already_exists = find_game_dir_by_logic_name(&games_dir, &game_id).is_some();

        templates.push(GameTemplateInfo {
            name,
            game_id,
            display_name,
            icon_path: if has_icon {
                crate::commands::common::allow_asset_file(&app, &icon_path)
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
fn resolve_import_game_template_paths(
    templates_dir: &Path,
    games_dir: &Path,
    template_name: &str,
) -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let template_dir = crate::utils::file_manager::safe_join(templates_dir, template_name)?;
    let config_path = crate::utils::file_manager::safe_join(&template_dir, "Config.json")?;
    let target_dir = crate::utils::file_manager::safe_join(games_dir, template_name)?;
    Ok((template_dir, config_path, target_dir))
}

#[tauri::command]
pub fn import_game_template(
    app: tauri::AppHandle,
    template_name: String,
    overwrite: bool,
) -> Result<(), String> {
    let templates_dir = get_templates_dir();
    let _ = app;
    let games_dir = get_user_games_dir()?;
    let (template_dir, config_path, target_dir) =
        resolve_import_game_template_paths(&templates_dir, &games_dir, &template_name)?;
    if !template_dir.exists() {
        return Err(format!("模板不存在: {}", template_name));
    }

    // 从 Config.json 读取 LogicName
    let game_id = std::fs::read_to_string(&config_path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| extract_game_id_from_config(&v))
        .unwrap_or_else(|| crate::configs::game_identity::to_canonical_or_keep(&template_name));

    // 查找已存在的同 LogicName 游戏文件夹（可能是英文名或旧代码名）
    let existing_dir = find_game_dir_by_logic_name(&games_dir, &game_id);

    if existing_dir.is_some() && !overwrite {
        return Err(format!("游戏已存在: {}", game_id));
    }

    // 覆盖时删除旧目录
    if let Some(ref old_dir) = existing_dir {
        remove_dir_if_exists(old_dir, "删除旧游戏目录失败")?;
    }

    // 递归复制模板目录
    copy_dir_recursive(&template_dir, &target_dir)?;

    info!(
        "已导入游戏配置模板: {} ({}) -> {}",
        template_name,
        game_id,
        target_dir.display()
    );
    Ok(())
}

fn load_hidden_games(games_dir: &Path) -> Vec<String> {
    use std::collections::BTreeSet;
    let hidden_path = games_dir.join("hidden_games.json");
    if hidden_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&hidden_path) {
            if let Ok(games) = serde_json::from_str::<Vec<String>>(&content) {
                let mut result = BTreeSet::new();
                for game in games {
                    result.insert(crate::configs::game_identity::to_canonical_or_keep(&game));
                }
                return result.into_iter().collect();
            }
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::{
        extract_game_id_from_config, find_background_image, find_background_video,
        find_game_dir_by_logic_name, is_infrastructure_dir_name, load_hidden_games,
        resolve_import_game_template_paths, should_include_game_dir,
    };
    use crate::utils::file_manager::copy_dir_recursive;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        std::env::temp_dir()
            .join("ssmt4-tests")
            .join(format!("game-scanner-{label}-{nonce}"))
    }

    #[test]
    fn infrastructure_dirs_are_filtered() {
        assert!(is_infrastructure_dir_name("prefix"));
        assert!(is_infrastructure_dir_name("Tools"));
        assert!(is_infrastructure_dir_name(".cache"));
        assert!(!is_infrastructure_dir_name("WutheringWaves"));
    }

    #[test]
    fn non_infrastructure_dir_without_config_is_not_included_by_default() {
        assert!(!should_include_game_dir("random-folder-2", None));
    }

    #[test]
    fn known_game_dir_without_config_is_still_included() {
        assert!(should_include_game_dir("WutheringWaves", None));
        assert!(should_include_game_dir("ZenlessZoneZero", None));
    }

    #[test]
    fn unknown_dir_without_config_is_not_included() {
        assert!(!should_include_game_dir("random-folder", None));
    }

    #[test]
    fn config_with_logic_name_is_included_even_for_custom_game() {
        let cfg = serde_json::json!({
            "LogicName": "MyCustomGame"
        });
        assert!(should_include_game_dir("random-folder", Some(&cfg)));
    }

    #[test]
    fn infrastructure_dir_is_never_included_even_with_valid_config() {
        let cfg = serde_json::json!({
            "LogicName": "WutheringWaves"
        });
        assert!(!should_include_game_dir("prefix", Some(&cfg)));
    }

    #[test]
    fn extract_game_id_from_config_prefers_logic_name_and_normalizes_aliases() {
        let logic_name_cfg = serde_json::json!({
            "LogicName": "  ZenlessZoneZero  ",
            "GamePreset": "WutheringWaves"
        });
        assert_eq!(
            extract_game_id_from_config(&logic_name_cfg),
            Some("ZenlessZoneZero".to_string())
        );

        let preset_only_cfg = serde_json::json!({
            "GamePreset": " HonkaiStarRail "
        });
        assert_eq!(
            extract_game_id_from_config(&preset_only_cfg),
            Some("HonkaiStarRail".to_string())
        );
    }

    #[test]
    fn extract_game_id_from_config_drops_blank_values() {
        let blank_cfg = serde_json::json!({
            "LogicName": "   ",
            "GamePreset": ""
        });

        assert_eq!(extract_game_id_from_config(&blank_cfg), None);
    }

    #[test]
    fn background_helpers_pick_supported_image_and_video_files() {
        let root = unique_temp_dir("backgrounds");
        std::fs::create_dir_all(&root).expect("create temp game dir");
        std::fs::write(root.join("Background.webp"), "img").expect("write background image");
        std::fs::write(root.join("Background.webm"), "vid").expect("write background video");

        let image = find_background_image(&root).expect("find background image");
        let video = find_background_video(&root).expect("find background video");

        assert!(image.ends_with("Background.webp"));
        assert!(video.ends_with("Background.webm"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn background_helpers_return_none_when_assets_are_missing() {
        let root = unique_temp_dir("backgrounds-missing");
        std::fs::create_dir_all(&root).expect("create temp game dir");

        assert_eq!(find_background_image(&root), None);
        assert_eq!(find_background_video(&root), None);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn find_game_dir_by_logic_name_matches_folder_and_config_aliases() {
        let root = unique_temp_dir("logic-name");
        std::fs::create_dir_all(root.join("WutheringWaves")).expect("create direct dir");
        std::fs::create_dir_all(root.join("CustomFolder")).expect("create config dir");
        std::fs::write(
            root.join("CustomFolder").join("Config.json"),
            r#"{"LogicName":"HonkaiStarRail"}"#,
        )
        .expect("write config");

        let direct = find_game_dir_by_logic_name(&root, "WutheringWaves").expect("direct match");
        let by_config =
            find_game_dir_by_logic_name(&root, "HonkaiStarRail").expect("config-based match");

        assert_eq!(direct, root.join("WutheringWaves"));
        assert_eq!(by_config, root.join("CustomFolder"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn load_hidden_games_normalizes_and_deduplicates_entries() {
        let root = unique_temp_dir("hidden-games");
        std::fs::create_dir_all(&root).expect("create temp dir");
        std::fs::write(
            root.join("hidden_games.json"),
            r#"[" MyGame ","MyGame","WutheringWaves","WutheringWaves"]"#,
        )
        .expect("write hidden games");

        let hidden = load_hidden_games(&root);

        assert_eq!(
            hidden,
            vec!["MyGame".to_string(), "WutheringWaves".to_string()]
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn load_hidden_games_returns_empty_for_invalid_json() {
        let root = unique_temp_dir("hidden-games-invalid");
        std::fs::create_dir_all(&root).expect("create temp dir");
        std::fs::write(root.join("hidden_games.json"), "{not valid json")
            .expect("write invalid hidden games");

        let hidden = load_hidden_games(&root);

        assert!(hidden.is_empty());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn load_hidden_games_returns_empty_when_file_is_missing() {
        let root = unique_temp_dir("hidden-games-missing");
        std::fs::create_dir_all(&root).expect("create temp dir");

        let hidden = load_hidden_games(&root);
        assert!(hidden.is_empty());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn copy_dir_recursive_copies_nested_files() {
        let src = unique_temp_dir("copy-src");
        let dst = unique_temp_dir("copy-dst");
        std::fs::create_dir_all(src.join("nested")).expect("create nested src dir");
        std::fs::write(src.join("Config.json"), "{}").expect("write root file");
        std::fs::write(src.join("nested").join("Icon.png"), "icon").expect("write nested file");

        copy_dir_recursive(&src, &dst).expect("copy directory tree");

        assert!(dst.join("Config.json").is_file());
        assert_eq!(
            std::fs::read_to_string(dst.join("nested").join("Icon.png")).expect("read copied file"),
            "icon"
        );

        let _ = std::fs::remove_dir_all(src);
        let _ = std::fs::remove_dir_all(dst);
    }

    #[test]
    fn resolve_import_game_template_paths_rejects_path_traversal() {
        let templates_dir = unique_temp_dir("template-root");
        let games_dir = unique_temp_dir("games-root");

        let err = resolve_import_game_template_paths(&templates_dir, &games_dir, "../escape")
            .expect_err("path traversal should fail");
        assert!(err.contains("路径安全校验失败"));

        let _ = std::fs::remove_dir_all(templates_dir);
        let _ = std::fs::remove_dir_all(games_dir);
    }

    #[test]
    fn resolve_import_game_template_paths_stays_within_template_and_games_roots() {
        let templates_dir = unique_temp_dir("template-root-safe");
        let games_dir = unique_temp_dir("games-root-safe");

        let (template_dir, config_path, target_dir) =
            resolve_import_game_template_paths(&templates_dir, &games_dir, "WutheringWaves")
                .expect("safe template path");

        assert_eq!(template_dir, templates_dir.join("WutheringWaves"));
        assert_eq!(
            config_path,
            templates_dir.join("WutheringWaves").join("Config.json")
        );
        assert_eq!(target_dir, games_dir.join("WutheringWaves"));

        let _ = std::fs::remove_dir_all(templates_dir);
        let _ = std::fs::remove_dir_all(games_dir);
    }
}
