use crate::commands::game_config;
use crate::configs::app_config;
use crate::utils::file_manager::safe_join;
use crate::utils::migoto_layout::resolve_migoto_path_state_for_game;
use serde::{Deserialize, Serialize};
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
    pub migoto_supported: bool,
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

fn entry_display_size(metadata: &fs::Metadata) -> u64 {
    if metadata.file_type().is_file() {
        return metadata.len();
    }

    // Mod 目录在进入页面时如果递归统计总大小，会把整棵目录树同步扫一遍。
    // 在虚拟机和机械盘环境下，这一步很容易把页面入口直接拖死。
    0
}

fn effective_entry_name(relative_name: &str) -> &str {
    relative_name
        .strip_suffix(DISABLED_SUFFIX)
        .unwrap_or(relative_name)
}

fn is_ini_name(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("ini"))
}

fn directory_contains_ini(path: &Path) -> Result<bool, String> {
    let mut pending = vec![PathBuf::from(path)];

    while let Some(dir) = pending.pop() {
        let entries = fs::read_dir(&dir)
            .map_err(|e| format!("读取 Mod 目录失败 {}: {}", dir.display(), e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("读取 Mod 目录条目失败: {}", e))?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.is_empty() || name.starts_with('.') {
                continue;
            }

            let file_type = entry
                .file_type()
                .map_err(|e| format!("读取 Mod 条目类型失败 {}: {}", entry.path().display(), e))?;
            if file_type.is_file() && is_ini_name(&name) {
                return Ok(true);
            }
            if file_type.is_dir() {
                pending.push(entry.path());
            }
        }
    }

    Ok(false)
}

fn is_managed_mod_entry(path: &Path, relative_name: &str) -> Result<bool, String> {
    let effective_name = effective_entry_name(relative_name);
    if effective_name.is_empty() || effective_name.starts_with('.') {
        return Ok(false);
    }

    let metadata = fs::metadata(path)
        .or_else(|_| fs::symlink_metadata(path))
        .map_err(|e| format!("读取 Mod 条目元数据失败 {}: {}", path.display(), e))?;

    if metadata.file_type().is_dir() {
        return directory_contains_ini(path);
    }

    Ok(metadata.file_type().is_file() && is_ini_name(effective_name))
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
        size_bytes: entry_display_size(&metadata),
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
        if !is_managed_mod_entry(&path, &name)? {
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

#[cfg(test)]
mod tests {
    use super::scan_mod_entries;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ssmt4-service-mod-manager-{label}-{nonce}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn scan_mod_entries_only_returns_entries_backed_by_ini_files() {
        let mod_folder = unique_temp_dir("scan");

        fs::write(mod_folder.join("script.py"), "print('x')").expect("write python file");
        fs::write(mod_folder.join("README.txt"), "docs").expect("write txt file");

        fs::create_dir_all(mod_folder.join("Docs")).expect("create docs dir");
        fs::write(mod_folder.join("Docs").join("guide.txt"), "docs").expect("write docs txt");

        fs::write(mod_folder.join("RootMod.ini"), "[TextureOverride]").expect("write root ini");
        fs::write(
            mod_folder.join("RootDisabled.ini.disabled"),
            "[TextureOverride]",
        )
        .expect("write disabled root ini");

        fs::create_dir_all(mod_folder.join("FolderMod").join("nested"))
            .expect("create folder mod dir");
        fs::write(
            mod_folder
                .join("FolderMod")
                .join("nested")
                .join("config.INI"),
            "[TextureOverride]",
        )
        .expect("write folder ini");

        fs::create_dir_all(mod_folder.join("FolderDisabled.disabled").join("scripts"))
            .expect("create disabled folder mod dir");
        fs::write(
            mod_folder
                .join("FolderDisabled.disabled")
                .join("scripts")
                .join("mod.ini"),
            "[TextureOverride]",
        )
        .expect("write disabled folder ini");

        let entries = scan_mod_entries(&mod_folder).expect("scan mod entries");
        let by_name = entries
            .into_iter()
            .map(|entry| (entry.relative_name.clone(), entry))
            .collect::<BTreeMap<_, _>>();

        assert_eq!(by_name.len(), 4);
        assert!(by_name.contains_key("RootMod.ini"));
        assert!(by_name.contains_key("RootDisabled.ini.disabled"));
        assert!(by_name.contains_key("FolderMod"));
        assert!(by_name.contains_key("FolderDisabled.disabled"));
        assert!(!by_name.contains_key("script.py"));
        assert!(!by_name.contains_key("README.txt"));
        assert!(!by_name.contains_key("Docs"));

        assert_eq!(by_name["RootMod.ini"].entry_type, "file");
        assert!(by_name["RootMod.ini"].enabled);
        assert_eq!(by_name["RootDisabled.ini.disabled"].entry_type, "file");
        assert!(!by_name["RootDisabled.ini.disabled"].enabled);
        assert_eq!(by_name["FolderMod"].entry_type, "directory");
        assert!(by_name["FolderMod"].enabled);
        assert_eq!(by_name["FolderDisabled.disabled"].entry_type, "directory");
        assert!(!by_name["FolderDisabled.disabled"].enabled);

        let _ = fs::remove_dir_all(mod_folder);
    }
}

fn load_migoto_state(
    app: tauri::AppHandle,
    game_name: &str,
) -> Result<GameModDirectoryState, String> {
    let canonical = canonical_key(game_name);
    let migoto_supported = crate::configs::game_presets::supports_migoto(&canonical);
    if !migoto_supported {
        return Ok(GameModDirectoryState {
            game_name: canonical,
            importer: String::new(),
            migoto_supported: false,
            migoto_enabled: false,
            mod_folder: String::new(),
            mod_folder_exists: false,
            shader_fixes_folder: String::new(),
            shader_fixes_folder_exists: false,
            entries: Vec::new(),
        });
    }

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
        migoto_supported,
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
