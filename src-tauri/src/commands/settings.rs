use crate::configs::app_config::{self, AppConfig};
use std::sync::Mutex;
use tauri::State;

#[tauri::command]
pub fn load_settings(config: State<'_, Mutex<AppConfig>>) -> Result<AppConfig, String> {
    let config_dir = app_config::get_app_config_dir();
    let config_path = config_dir.join("settings.json");

    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read settings: {}", e))?;
        let loaded: AppConfig = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse settings: {}", e))?;
        let mut state = config.lock().map_err(|e| e.to_string())?;
        *state = loaded.clone();
        Ok(loaded)
    } else {
        let state = config.lock().map_err(|e| e.to_string())?;
        Ok(state.clone())
    }
}

#[tauri::command]
pub fn save_settings(
    config: State<'_, Mutex<AppConfig>>,
    settings: AppConfig,
) -> Result<(), String> {
    let config_dir = app_config::get_app_config_dir();
    crate::utils::file_manager::ensure_dir(&config_dir)?;

    let config_path = config_dir.join("settings.json");
    let content = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    std::fs::write(&config_path, content)
        .map_err(|e| format!("Failed to write settings: {}", e))?;

    let mut state = config.lock().map_err(|e| e.to_string())?;
    *state = settings;
    Ok(())
}
