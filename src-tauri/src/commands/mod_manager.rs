use notify::{Event, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModInfo {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(rename = "relativePath")]
    pub relative_path: String,
    pub enabled: bool,
    pub group: String,
    #[serde(rename = "isDir")]
    pub is_dir: bool,
    #[serde(rename = "previewImages")]
    pub preview_images: Vec<String>,
    pub last_modified: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupInfo {
    pub id: String,
    pub name: String,
    #[serde(rename = "iconPath")]
    pub icon_path: Option<String>,
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModScanResult {
    pub mods: Vec<ModInfo>,
    pub groups: Vec<GroupInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchivePreview {
    pub root_dirs: Vec<String>,
    pub file_count: usize,
    pub has_ini: bool,
    pub format: String,
}

#[derive(Default)]
pub struct ModWatcher {
    pub watcher: Option<notify::RecommendedWatcher>,
}

fn resolve_game_path(app: &AppHandle, game_name: &str) -> Result<PathBuf, String> {
    // 1) 优先从 SQLite 读取（save_game_config 的落盘位置）
    if let Some(content) = crate::configs::database::get_game_config(game_name) {
        if let Ok(data) = serde_json::from_str::<Value>(&content) {
            if let Some(path) = extract_game_path_from_config(&data) {
                return Ok(path);
            }
        }
    }

    // 2) 回退到文件系统 Config.json（兼容旧数据）
    let config_dirs: Vec<PathBuf> = {
        let mut dirs = Vec::new();
        // 用户可写目录优先
        dirs.push(crate::utils::file_manager::get_global_games_dir().join(game_name));
        if let Ok(resource_dir) = app.path().resource_dir() {
            dirs.push(resource_dir.join("resources").join("Games").join(game_name));
        }
        // 开发模式回退
        let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("Games")
            .join(game_name);
        if dev_path.exists() {
            dirs.push(dev_path);
        }
        dirs
    };

    for dir in &config_dirs {
        let config_path = dir.join("Config.json");
        if config_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                if let Ok(data) = serde_json::from_str::<Value>(&content) {
                    if let Some(path) = extract_game_path_from_config(&data) {
                        return Ok(path);
                    }
                }
            }
        }
    }

    // 回退：使用全局游戏目录
    let games_dir = crate::utils::file_manager::get_global_games_dir();
    Ok(games_dir.join(game_name))
}

fn extract_game_path_from_config(data: &Value) -> Option<PathBuf> {
    let candidate = data
        .pointer("/other/gamePath")
        .or_else(|| data.pointer("/other/game_path"))
        .or_else(|| data.get("gamePath"))
        .or_else(|| data.get("game_path"))
        .or_else(|| data.get("TargetPath"))
        .or_else(|| data.get("targetPath"))
        .and_then(|v| v.as_str())?;

    normalize_game_root(candidate)
}

fn normalize_game_root(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let path = PathBuf::from(trimmed);

    if path.is_dir() {
        return Some(path);
    }
    if path.is_file() {
        return path.parent().map(|p| p.to_path_buf());
    }

    // 路径不存在时：若像可执行文件路径，则使用其父目录作为游戏根目录。
    if path.extension().is_some() {
        return path.parent().map(|p| p.to_path_buf()).or(Some(path));
    }

    Some(path)
}

#[tauri::command]
pub fn scan_mods(app: AppHandle, game_name: &str) -> Result<ModScanResult, String> {
    let game_path = resolve_game_path(&app, game_name)?;
    let mods_dir = game_path.join("Mods");
    if !mods_dir.exists() {
        return Ok(ModScanResult {
            mods: Vec::new(),
            groups: Vec::new(),
        });
    }

    let mut mods = Vec::new();
    let mut groups = Vec::new();
    scan_dir_recursive(&mods_dir, &mods_dir, "", &mut mods, &mut groups)?;
    mods.sort_by(|a, b| a.group.cmp(&b.group).then_with(|| a.name.cmp(&b.name)));
    Ok(ModScanResult { mods, groups })
}

fn scan_dir_recursive(
    base_dir: &Path,
    dir: &Path,
    group: &str,
    mods: &mut Vec<ModInfo>,
    groups: &mut Vec<GroupInfo>,
) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("Failed to read mods dir: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();

        let has_ini = std::fs::read_dir(&path)
            .map(|entries| {
                entries
                    .flatten()
                    .any(|e| e.path().extension().map_or(false, |ext| ext == "ini"))
            })
            .unwrap_or(false);

        if has_ini {
            let enabled = !name.starts_with("DISABLED");
            let display_name = if enabled {
                name.clone()
            } else {
                name.trim_start_matches("DISABLED").trim_start().to_string()
            };

            let relative = path
                .strip_prefix(base_dir)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();

            let preview_images = collect_previews(&path);
            let last_modified = std::fs::metadata(&path)
                .and_then(|m| m.modified())
                .map(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                })
                .unwrap_or(0);

            mods.push(ModInfo {
                id: relative.clone(),
                name: display_name,
                path: path.to_string_lossy().to_string(),
                relative_path: relative,
                enabled,
                group: group.to_string(),
                is_dir: true,
                preview_images,
                last_modified,
            });
        } else {
            // Group folder
            let group_id = if group.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", group, name)
            };

            let icon_path = find_group_icon(&path);

            groups.push(GroupInfo {
                id: group_id.clone(),
                name: name.clone(),
                icon_path,
                parent_id: if group.is_empty() {
                    None
                } else {
                    Some(group.to_string())
                },
            });

            scan_dir_recursive(base_dir, &path, &group_id, mods, groups)?;
        }
    }
    Ok(())
}

fn collect_previews(mod_dir: &Path) -> Vec<String> {
    let mut previews = Vec::new();
    let candidates = ["preview.png", "preview.jpg", "Preview.png", "Preview.jpg"];
    for name in &candidates {
        let p = mod_dir.join(name);
        if p.exists() {
            previews.push(p.to_string_lossy().to_string());
        }
    }
    previews
}

fn find_group_icon(group_dir: &Path) -> Option<String> {
    for name in &["icon.png", "Icon.png", "icon.jpg"] {
        let p = group_dir.join(name);
        if p.exists() {
            return Some(p.to_string_lossy().to_string());
        }
    }
    None
}

#[tauri::command]
pub fn toggle_mod(
    app: AppHandle,
    game_name: &str,
    mod_relative_path: &str,
    enable: bool,
) -> Result<String, String> {
    let game_path = resolve_game_path(&app, game_name)?;
    let mod_path = game_path.join("Mods").join(mod_relative_path);

    if !mod_path.exists() {
        return Err(format!("Mod path not found: {}", mod_path.display()));
    }

    let name = mod_path
        .file_name()
        .ok_or("Invalid mod path")?
        .to_string_lossy()
        .to_string();

    let parent = mod_path.parent().ok_or("Invalid mod path")?;

    let new_name = if enable {
        name.trim_start_matches("DISABLED").trim_start().to_string()
    } else {
        if name.starts_with("DISABLED") {
            name.clone()
        } else {
            format!("DISABLED {}", name)
        }
    };

    if new_name == name {
        return Ok(mod_relative_path.to_string());
    }

    let new_path = parent.join(&new_name);
    std::fs::rename(&mod_path, &new_path).map_err(|e| format!("Failed to toggle mod: {}", e))?;

    let new_relative = new_path
        .strip_prefix(game_path.join("Mods"))
        .unwrap_or(&new_path)
        .to_string_lossy()
        .to_string();

    info!("Toggled mod: {} -> {} (enabled={})", name, new_name, enable);
    Ok(new_relative)
}

#[tauri::command]
pub fn watch_mods(
    app: AppHandle,
    game_name: String,
    watcher_state: tauri::State<'_, std::sync::Mutex<ModWatcher>>,
) -> Result<(), String> {
    let game_path = resolve_game_path(&app, &game_name)?;
    let mods_dir = game_path.join("Mods");
    crate::utils::file_manager::ensure_dir(&mods_dir)?;

    let app_clone = app.clone();
    let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(_event) = res {
            app_clone.emit("mods-changed", ()).ok();
            // Keep legacy event name for frontend compatibility.
            app_clone.emit("mod-filesystem-changed", ()).ok();
        }
    })
    .map_err(|e| format!("Failed to create watcher: {}", e))?;

    watcher
        .watch(&mods_dir, RecursiveMode::Recursive)
        .map_err(|e| format!("Failed to watch mods dir: {}", e))?;

    let mut state = watcher_state.lock().map_err(|e| e.to_string())?;
    state.watcher = Some(watcher);

    info!("Watching mods dir: {}", mods_dir.display());
    Ok(())
}

#[tauri::command]
pub fn unwatch_mods(
    watcher_state: tauri::State<'_, std::sync::Mutex<ModWatcher>>,
) -> Result<(), String> {
    let mut state = watcher_state.lock().map_err(|e| e.to_string())?;
    state.watcher = None;
    info!("Stopped watching mods");
    Ok(())
}

#[tauri::command]
pub fn create_mod_group(app: AppHandle, game_name: &str, group_name: &str) -> Result<(), String> {
    let game_path = resolve_game_path(&app, game_name)?;
    let group_dir = game_path.join("Mods").join(group_name);
    crate::utils::file_manager::ensure_dir(&group_dir)?;
    info!("Created mod group: {}", group_name);
    Ok(())
}

#[tauri::command]
pub fn rename_mod_group(
    app: AppHandle,
    game_name: &str,
    old_group: &str,
    new_group: &str,
) -> Result<(), String> {
    let game_path = resolve_game_path(&app, game_name)?;
    let mods_dir = game_path.join("Mods");
    let old_path = mods_dir.join(old_group);
    let new_path = mods_dir.join(new_group);

    if !old_path.exists() {
        return Err(format!("Group not found: {}", old_group));
    }

    std::fs::rename(&old_path, &new_path).map_err(|e| format!("Failed to rename group: {}", e))?;

    info!("Renamed mod group: {} -> {}", old_group, new_group);
    Ok(())
}

#[tauri::command]
pub fn delete_mod_group(app: AppHandle, game_name: &str, group_name: &str) -> Result<(), String> {
    let game_path = resolve_game_path(&app, game_name)?;
    let group_dir = game_path.join("Mods").join(group_name);
    if group_dir.exists() {
        trash::delete(&group_dir).map_err(|e| format!("Failed to delete group: {}", e))?;
        info!("Deleted mod group: {}", group_name);
    }
    Ok(())
}

#[tauri::command]
pub fn set_mod_group_icon(
    app: AppHandle,
    game_name: &str,
    group_path: &str,
    icon_path: &str,
) -> Result<(), String> {
    let game_path = resolve_game_path(&app, game_name)?;
    let group_dir = game_path.join("Mods").join(group_path);
    let dest = group_dir.join("icon.png");
    std::fs::copy(icon_path, &dest).map_err(|e| format!("Failed to copy icon: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn delete_mod(app: AppHandle, game_name: &str, mod_relative_path: &str) -> Result<(), String> {
    let game_path = resolve_game_path(&app, game_name)?;
    let mod_path = game_path.join("Mods").join(mod_relative_path);
    if mod_path.exists() {
        trash::delete(&mod_path).map_err(|e| format!("Failed to delete mod: {}", e))?;
        info!("Deleted mod: {}", mod_relative_path);
    }
    Ok(())
}

#[tauri::command]
pub fn move_mod_to_group(
    app: AppHandle,
    game_name: &str,
    mod_id: &str,
    new_group: &str,
) -> Result<(), String> {
    let game_path = resolve_game_path(&app, game_name)?;
    let mods_dir = game_path.join("Mods");
    let src = mods_dir.join(mod_id);

    let mod_name = src
        .file_name()
        .ok_or("Invalid mod path")?
        .to_string_lossy()
        .to_string();

    let target_dir = if new_group.is_empty() || new_group == "Root" {
        mods_dir.clone()
    } else {
        mods_dir.join(new_group)
    };
    crate::utils::file_manager::ensure_dir(&target_dir)?;

    let dest = target_dir.join(&mod_name);
    std::fs::rename(&src, &dest).map_err(|e| format!("Failed to move mod: {}", e))?;

    info!("Moved mod {} to group {}", mod_name, new_group);
    Ok(())
}

#[tauri::command]
pub fn open_game_mods_folder(app: AppHandle, game_name: &str) -> Result<(), String> {
    let game_path = resolve_game_path(&app, game_name)?;
    let mods_dir = game_path.join("Mods");
    crate::utils::file_manager::ensure_dir(&mods_dir)?;
    std::process::Command::new("xdg-open")
        .arg(mods_dir.to_string_lossy().as_ref())
        .spawn()
        .map_err(|e| format!("Failed to open folder: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn open_mod_group_folder(
    app: AppHandle,
    game_name: &str,
    group_path: &str,
) -> Result<(), String> {
    let game_path = resolve_game_path(&app, game_name)?;
    let group_dir = game_path.join("Mods").join(group_path);
    if !group_dir.exists() {
        return Err("Group folder does not exist".to_string());
    }
    std::process::Command::new("xdg-open")
        .arg(group_dir.to_string_lossy().as_ref())
        .spawn()
        .map_err(|e| format!("Failed to open folder: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn preview_mod_archive(path: &str) -> Result<ArchivePreview, String> {
    let archive_path = PathBuf::from(path);
    let ext = archive_path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();

    match ext.as_str() {
        "zip" => preview_zip(&archive_path).map(|files| build_archive_preview("zip", files)),
        "7z" => preview_7z(&archive_path).map(|files| build_archive_preview("7z", files)),
        _ => Err(format!("Unsupported archive format: {}", ext)),
    }
}

fn build_archive_preview(format: &str, files: Vec<String>) -> ArchivePreview {
    let mut root_dirs = Vec::new();
    let mut seen = HashSet::new();
    let mut has_ini = false;

    for file in &files {
        let normalized = file.replace('\\', "/");
        let trimmed = normalized.trim_start_matches("./").trim_start_matches('/');
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.to_ascii_lowercase().ends_with(".ini") {
            has_ini = true;
        }

        if let Some((root, _)) = trimmed.split_once('/') {
            if !root.is_empty() && root != "." && seen.insert(root.to_string()) {
                root_dirs.push(root.to_string());
            }
        }
    }

    ArchivePreview {
        root_dirs,
        file_count: files.len(),
        has_ini,
        format: format.to_string(),
    }
}

fn preview_zip(path: &Path) -> Result<Vec<String>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("Failed to open zip: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Failed to read zip: {}", e))?;

    let mut names = Vec::new();
    for i in 0..archive.len() {
        let file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry {}: {}", i, e))?;
        if file.is_dir() {
            continue;
        }
        names.push(file.name().to_string());
    }

    Ok(names)
}

fn preview_7z(path: &Path) -> Result<Vec<String>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("Failed to open 7z: {}", e))?;
    let len = file
        .metadata()
        .map_err(|e| format!("Failed to get file metadata: {}", e))?
        .len();
    let mut reader = std::io::BufReader::new(file);
    let archive = sevenz_rust::Archive::read(&mut reader, len, &[])
        .map_err(|e| format!("Failed to read 7z: {}", e))?;

    let names: Vec<String> = archive
        .files
        .iter()
        .filter(|f| f.has_stream())
        .map(|f| f.name().to_string())
        .collect();

    Ok(names)
}

#[tauri::command]
pub fn install_mod_archive(
    app: AppHandle,
    game_name: &str,
    archive_path: &str,
    target_name: &str,
    target_group: &str,
    _password: Option<String>,
) -> Result<(), String> {
    let game_path = resolve_game_path(&app, game_name)?;
    let archive = PathBuf::from(archive_path);
    let ext = archive
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();

    let base_dir = if target_group.is_empty() {
        game_path.join("Mods")
    } else {
        game_path.join("Mods").join(target_group)
    };

    let target_dir = if target_name.is_empty() {
        base_dir.clone()
    } else {
        base_dir.join(target_name)
    };
    crate::utils::file_manager::ensure_dir(&target_dir)?;

    match ext.as_str() {
        "zip" => install_from_zip(&archive, &target_dir)?,
        "7z" => install_from_7z(&archive, &target_dir)?,
        _ => return Err(format!("Unsupported archive format: {}", ext)),
    }

    info!(
        "Installed mod archive {} to {}",
        archive_path,
        target_dir.display()
    );
    Ok(())
}

fn install_from_zip(archive_path: &Path, target_dir: &Path) -> Result<(), String> {
    let file =
        std::fs::File::open(archive_path).map_err(|e| format!("Failed to open zip: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Failed to read zip: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {}", e))?;
        let name = file.name().to_string();
        let dest = target_dir.join(&name);

        if name.ends_with('/') {
            std::fs::create_dir_all(&dest).ok();
        } else {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let mut out = std::fs::File::create(&dest)
                .map_err(|e| format!("Failed to create file: {}", e))?;
            std::io::copy(&mut file, &mut out).map_err(|e| format!("Failed to extract: {}", e))?;
        }
    }
    Ok(())
}

fn install_from_7z(archive_path: &Path, target_dir: &Path) -> Result<(), String> {
    sevenz_rust::decompress_file(archive_path, target_dir)
        .map_err(|e| format!("Failed to extract 7z: {}", e))?;
    Ok(())
}
