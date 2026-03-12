pub use crate::services::mod_manager::{
    GameModDirectoryState, ManagedModEntry, ModBulkToggleResult,
};

#[tauri::command]
pub fn scan_game_mods(
    app: tauri::AppHandle,
    game_name: &str,
) -> Result<GameModDirectoryState, String> {
    crate::services::mod_manager::scan_game_mods(app, game_name)
}

#[tauri::command]
pub fn set_game_mod_entry_enabled(
    app: tauri::AppHandle,
    game_name: &str,
    relative_name: &str,
    enabled: bool,
) -> Result<ManagedModEntry, String> {
    crate::services::mod_manager::set_game_mod_entry_enabled(app, game_name, relative_name, enabled)
}

#[tauri::command]
pub fn set_all_game_mod_entries_enabled(
    app: tauri::AppHandle,
    game_name: &str,
    enabled: bool,
) -> Result<ModBulkToggleResult, String> {
    crate::services::mod_manager::set_all_game_mod_entries_enabled(app, game_name, enabled)
}

#[cfg(test)]
mod tests {
    use crate::utils::migoto_layout::resolve_migoto_path_state_for_game;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ssmt4-mod-manager-{name}-{nanos}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn uses_base_path_when_it_already_looks_like_importer_folder() {
        let migoto_path = unique_temp_dir("base-importer");
        fs::create_dir_all(migoto_path.join("Mods")).expect("create mods dir");

        let config = json!({
            "other": {
                "migoto": {
                    "migoto_path": migoto_path,
                    "importer": "EFMI"
                }
            }
        });

        let paths = resolve_migoto_path_state_for_game(
            "ArknightsEndfield",
            &config,
            PathBuf::from("unused"),
        );
        assert_eq!(paths.mod_folder, migoto_path.join("Mods"));
        assert_eq!(paths.shader_fixes_folder, migoto_path.join("ShaderFixes"));

        let _ = fs::remove_dir_all(migoto_path);
    }

    #[test]
    fn uses_nested_importer_folder_when_it_exists() {
        let migoto_path = unique_temp_dir("nested-importer");
        let importer_folder = migoto_path.join("EFMI");
        fs::create_dir_all(importer_folder.join("Mods")).expect("create nested mods dir");

        let config = json!({
            "other": {
                "migoto": {
                    "migoto_path": migoto_path,
                    "importer": "EFMI"
                }
            }
        });

        let paths = resolve_migoto_path_state_for_game(
            "ArknightsEndfield",
            &config,
            PathBuf::from("unused"),
        );
        assert_eq!(paths.mod_folder, importer_folder.join("Mods"));
        assert_eq!(
            paths.shader_fixes_folder,
            importer_folder.join("ShaderFixes")
        );

        let _ = fs::remove_dir_all(migoto_path);
    }

    #[test]
    fn falls_back_to_base_path_for_fresh_deploys_without_nested_importer() {
        let migoto_path = unique_temp_dir("fresh-deploy");

        let config = json!({
            "other": {
                "migoto": {
                    "migoto_path": migoto_path,
                    "importer": "EFMI"
                }
            }
        });

        let paths = resolve_migoto_path_state_for_game(
            "ArknightsEndfield",
            &config,
            PathBuf::from("unused"),
        );
        assert_eq!(paths.mod_folder, migoto_path.join("Mods"));
        assert_eq!(paths.shader_fixes_folder, migoto_path.join("ShaderFixes"));

        let _ = fs::remove_dir_all(migoto_path);
    }
}
