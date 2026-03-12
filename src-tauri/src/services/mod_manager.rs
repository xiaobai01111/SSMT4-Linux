use crate::commands::game_config;
use crate::configs::app_config;
use crate::utils::file_manager::safe_join;
use crate::utils::migoto_layout::resolve_migoto_path_state_for_game;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

const DISABLED_SUFFIX: &str = ".disabled";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedModEntry {
    pub relative_name: String,
    pub display_name: String,
    pub path: String,
    pub enabled: bool,
    pub entry_type: String,
    pub size_bytes: u64,
    pub modified_unix: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameModDirectoryState {
    pub game_name: String,
    pub importer: String,
    pub migoto_enabled: bool,
    pub mod_folder: String,
    pub mod_folder_exists: bool,
    pub shader_fixes_folder: String,
    pub shader_fixes_folder_exists: bool,
    pub entries: Vec<ManagedModEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModBulkToggleResult {
    pub changed: usize,
    pub skipped: Vec<String>,
}

fn canonical_key(input: &str) -> String {
    crate::configs::game_identity::to_canonical_or_keep(input)
}

fn metadata_modified_unix(metadata: &fs::Metadata) -> Option<u64> {
    metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_secs())
}

fn calculate_entry_size(path: &Path) -> u64 {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return 0;
    };

    if metadata.file_type().is_file() {
        return metadata.len();
    }

    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return 0;
    }

    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };

    entries
        .flatten()
        .map(|entry| calculate_entry_size(&entry.path()))
        .sum()
}

fn build_managed_entry(path: &Path) -> Result<ManagedModEntry, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|e| format!("读取 Mod 条目元数据失败 {}: {}", path.display(), e))?;
    let relative_name = path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .ok_or_else(|| format!("无效的 Mod 条目名称: {}", path.display()))?;
    let enabled = !relative_name.ends_with(DISABLED_SUFFIX);
    let display_name = if enabled {
        relative_name.clone()
    } else {
        relative_name
            .strip_suffix(DISABLED_SUFFIX)
            .unwrap_or(&relative_name)
            .to_string()
    };

    Ok(ManagedModEntry {
        display_name,
        enabled,
        entry_type: if metadata.file_type().is_dir() {
            "directory".to_string()
        } else {
            "file".to_string()
        },
        size_bytes: calculate_entry_size(path),
        modified_unix: metadata_modified_unix(&metadata),
        path: path.to_string_lossy().to_string(),
        relative_name,
    })
}

fn scan_mod_entries(mod_folder: &Path) -> Result<Vec<ManagedModEntry>, String> {
    if !mod_folder.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(mod_folder)
        .map_err(|e| format!("读取 Mod 目录失败 {}: {}", mod_folder.display(), e))?;

    let mut result = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取 Mod 目录条目失败: {}", e))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.is_empty() || name.starts_with('.') {
            continue;
        }
        result.push(build_managed_entry(&path)?);
    }

    result.sort_by(|a, b| {
        b.enabled.cmp(&a.enabled).then_with(|| {
            a.display_name
                .to_lowercase()
                .cmp(&b.display_name.to_lowercase())
        })
    });

    Ok(result)
}

fn load_migoto_state(
    app: tauri::AppHandle,
    game_name: &str,
) -> Result<GameModDirectoryState, String> {
    let canonical = canonical_key(game_name);
    let config = game_config::load_game_config(app, &canonical)?;
    let paths = resolve_migoto_path_state_for_game(
        &canonical,
        &config,
        app_config::get_app_data_dir().join("3Dmigoto-data"),
    );
    let mod_folder_exists = paths.mod_folder.exists();
    let shader_fixes_folder_exists = paths.shader_fixes_folder.exists();
    let entries = scan_mod_entries(&paths.mod_folder)?;

    Ok(GameModDirectoryState {
        game_name: canonical,
        importer: paths.importer,
        migoto_enabled: paths.migoto_enabled,
        mod_folder: paths.mod_folder.to_string_lossy().to_string(),
        mod_folder_exists,
        shader_fixes_folder: paths.shader_fixes_folder.to_string_lossy().to_string(),
        shader_fixes_folder_exists,
        entries,
    })
}

fn toggle_entry_enabled(
    mod_folder: &Path,
    relative_name: &str,
    enabled: bool,
) -> Result<ManagedModEntry, String> {
    let source = safe_join(mod_folder, relative_name)?;
    if !source.exists() {
        return Err(format!("目标 Mod 条目不存在: {}", source.display()));
    }

    let currently_enabled = !relative_name.ends_with(DISABLED_SUFFIX);
    if currently_enabled == enabled {
        return build_managed_entry(&source);
    }

    let target_name = if enabled {
        relative_name
            .strip_suffix(DISABLED_SUFFIX)
            .ok_or_else(|| format!("无法启用未标记为禁用的条目: {}", relative_name))?
            .to_string()
    } else {
        format!("{}{}", relative_name, DISABLED_SUFFIX)
    };

    let target = safe_join(mod_folder, &target_name)?;
    if target.exists() {
        return Err(format!(
            "目标名称已存在，无法切换状态: {}",
            target.file_name().unwrap_or_default().to_string_lossy()
        ));
    }

    fs::rename(&source, &target).map_err(|e| {
        format!(
            "切换 Mod 状态失败 {} -> {}: {}",
            source.display(),
            target.display(),
            e
        )
    })?;

    build_managed_entry(&target)
}

pub fn scan_game_mods(
    app: tauri::AppHandle,
    game_name: &str,
) -> Result<GameModDirectoryState, String> {
    load_migoto_state(app, game_name)
}

pub fn set_game_mod_entry_enabled(
    app: tauri::AppHandle,
    game_name: &str,
    relative_name: &str,
    enabled: bool,
) -> Result<ManagedModEntry, String> {
    let state = load_migoto_state(app, game_name)?;
    toggle_entry_enabled(Path::new(&state.mod_folder), relative_name, enabled)
}

pub fn set_all_game_mod_entries_enabled(
    app: tauri::AppHandle,
    game_name: &str,
    enabled: bool,
) -> Result<ModBulkToggleResult, String> {
    let state = load_migoto_state(app, game_name)?;
    let mod_folder = Path::new(&state.mod_folder);
    let mut changed = 0usize;
    let mut skipped = Vec::new();

    for entry in state.entries {
        if entry.enabled == enabled {
            continue;
        }
        match toggle_entry_enabled(mod_folder, &entry.relative_name, enabled) {
            Ok(_) => changed += 1,
            Err(_) => skipped.push(entry.relative_name),
        }
    }

    Ok(ModBulkToggleResult { changed, skipped })
}
