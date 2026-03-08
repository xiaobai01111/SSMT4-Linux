use crate::commands::game_config;
use crate::configs::app_config;
use crate::utils::file_manager::safe_join;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
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

#[derive(Debug, Clone)]
struct MigotoPaths {
    importer: String,
    migoto_enabled: bool,
    mod_folder: PathBuf,
    shader_fixes_folder: PathBuf,
}

fn canonical_key(input: &str) -> String {
    crate::configs::game_identity::to_canonical_or_keep(input)
}

fn trim_value(value: Option<&str>) -> String {
    value.unwrap_or_default().trim().to_string()
}

fn game_to_importer(game_name: &str) -> Option<&'static str> {
    match game_name {
        "WutheringWaves" => Some("WWMI"),
        "ZenlessZoneZero" => Some("ZZMI"),
        "HonkaiStarRail" => Some("SRMI"),
        "GenshinImpact" | "Genshin" => Some("GIMI"),
        "HonkaiImpact3rd" | "Honkai3rd" => Some("HIMI"),
        "ArknightsEndfield" => Some("EFMI"),
        _ => None,
    }
}

fn resolve_importer(game_name: &str, configured: Option<&str>) -> String {
    if let Some(mapped) = game_to_importer(game_name) {
        return mapped.to_string();
    }

    let normalized = trim_value(configured).to_ascii_uppercase();
    if normalized.is_empty() {
        "WWMI".to_string()
    } else {
        normalized
    }
}

fn path_basename(path: &Path) -> String {
    path.file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn string_to_path(value: Option<&str>) -> Option<PathBuf> {
    let trimmed = trim_value(value);
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

fn resolve_migoto_paths(game_name: &str, config: &Value) -> MigotoPaths {
    let migoto = config
        .pointer("/other/migoto")
        .or_else(|| config.get("migoto"));

    let importer = resolve_importer(
        game_name,
        migoto
            .and_then(|value| value.get("importer"))
            .and_then(|value| value.as_str()),
    );

    let default_migoto_path = app_config::get_app_data_dir().join("3Dmigoto-data");
    let migoto_path = string_to_path(
        migoto
            .and_then(|value| value.get("migoto_path"))
            .and_then(|value| value.as_str()),
    )
    .unwrap_or(default_migoto_path);

    let importer_folder = string_to_path(
        migoto
            .and_then(|value| value.get("importer_folder"))
            .and_then(|value| value.as_str()),
    )
    .unwrap_or_else(|| {
        if path_basename(&migoto_path).eq_ignore_ascii_case(&importer) {
            migoto_path.clone()
        } else {
            migoto_path.join(&importer)
        }
    });

    let mod_folder = string_to_path(
        migoto
            .and_then(|value| value.get("mod_folder"))
            .and_then(|value| value.as_str()),
    )
    .unwrap_or_else(|| importer_folder.join("Mods"));

    let shader_fixes_folder = string_to_path(
        migoto
            .and_then(|value| value.get("shader_fixes_folder"))
            .and_then(|value| value.as_str()),
    )
    .unwrap_or_else(|| importer_folder.join("ShaderFixes"));

    let migoto_enabled = migoto
        .and_then(|value| value.get("enabled"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false);

    MigotoPaths {
        importer,
        migoto_enabled,
        mod_folder,
        shader_fixes_folder,
    }
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

    if metadata.file_type().is_symlink() {
        return 0;
    }

    if !metadata.file_type().is_dir() {
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
        b.enabled
            .cmp(&a.enabled)
            .then_with(|| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()))
    });

    Ok(result)
}

fn load_migoto_state(app: tauri::AppHandle, game_name: &str) -> Result<GameModDirectoryState, String> {
    let canonical = canonical_key(game_name);
    let config = game_config::load_game_config(app, &canonical)?;
    let paths = resolve_migoto_paths(&canonical, &config);
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

fn toggle_entry_enabled(mod_folder: &Path, relative_name: &str, enabled: bool) -> Result<ManagedModEntry, String> {
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

#[tauri::command]
pub fn scan_game_mods(app: tauri::AppHandle, game_name: &str) -> Result<GameModDirectoryState, String> {
    load_migoto_state(app, game_name)
}

#[tauri::command]
pub fn set_game_mod_entry_enabled(
    app: tauri::AppHandle,
    game_name: &str,
    relative_name: &str,
    enabled: bool,
) -> Result<ManagedModEntry, String> {
    let state = load_migoto_state(app, game_name)?;
    toggle_entry_enabled(Path::new(&state.mod_folder), relative_name, enabled)
}

#[tauri::command]
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
