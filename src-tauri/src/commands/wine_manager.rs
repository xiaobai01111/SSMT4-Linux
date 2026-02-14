use crate::configs::wine_config::{
    GameWineConfig, PrefixConfig, PrefixTemplate, ProtonSettings, WineVersion,
};
use crate::wine::{detector, display, graphics, prefix, runtime};
use std::sync::Mutex;
use tauri::State;
use tracing::info;

#[tauri::command]
pub fn scan_wine_versions(
    settings: State<'_, Mutex<crate::configs::app_config::AppConfig>>,
) -> Result<Vec<WineVersion>, String> {
    let custom_paths = {
        let s = settings.lock().map_err(|e| e.to_string())?;
        s.custom_search_paths.clone()
    };
    Ok(detector::scan_all_versions(&custom_paths))
}

#[tauri::command]
pub fn get_game_wine_config(game_id: &str) -> Result<GameWineConfig, String> {
    let prefix_config = prefix::load_prefix_config(game_id).ok();
    let prefix_dir = prefix::get_prefix_dir(game_id);

    Ok(GameWineConfig {
        game_id: game_id.to_string(),
        wine_version_id: prefix_config.as_ref().map(|c| c.wine_version_id.clone()),
        prefix_path: if prefix_dir.exists() {
            Some(prefix_dir)
        } else {
            None
        },
        proton_settings: prefix_config.map(|c| c.proton_settings).unwrap_or_default(),
        launcher_api_config: None,
    })
}

#[tauri::command]
pub fn set_game_wine_config(
    game_id: &str,
    wine_version_id: &str,
    proton_settings: ProtonSettings,
) -> Result<(), String> {
    if prefix::prefix_exists(game_id) {
        let mut config = prefix::load_prefix_config(game_id)?;
        config.wine_version_id = wine_version_id.to_string();
        config.proton_settings = proton_settings;
        prefix::save_prefix_config(game_id, &config)?;
    } else {
        let config = PrefixConfig {
            wine_version_id: wine_version_id.to_string(),
            proton_settings,
            ..Default::default()
        };
        prefix::create_prefix(game_id, &config)?;
    }
    info!("Updated wine config for game {}", game_id);
    Ok(())
}

#[tauri::command]
pub fn create_prefix(game_id: &str, template_id: Option<String>) -> Result<String, String> {
    let path = if let Some(tid) = template_id {
        let templates = prefix::list_templates()?;
        let template = templates
            .iter()
            .find(|t| t.id == tid)
            .ok_or_else(|| format!("Template '{}' not found", tid))?;
        prefix::create_prefix_from_template(game_id, template)?
    } else {
        prefix::create_prefix(game_id, &PrefixConfig::default())?
    };
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn delete_prefix(game_id: &str) -> Result<(), String> {
    prefix::delete_prefix(game_id)
}

#[tauri::command]
pub fn get_prefix_info(game_id: &str) -> Result<prefix::PrefixInfo, String> {
    prefix::get_prefix_info(game_id)
}

#[tauri::command]
pub async fn install_dxvk(game_id: &str, version: &str) -> Result<String, String> {
    let pfx_dir = prefix::get_prefix_pfx_dir(game_id);
    graphics::install_dxvk(&pfx_dir, version).await
}

#[tauri::command]
pub fn uninstall_dxvk(game_id: &str) -> Result<String, String> {
    let pfx_dir = prefix::get_prefix_pfx_dir(game_id);
    graphics::uninstall_dxvk(&pfx_dir)
}

#[tauri::command]
pub async fn install_vkd3d(game_id: &str, version: &str) -> Result<String, String> {
    let pfx_dir = prefix::get_prefix_pfx_dir(game_id);
    graphics::install_vkd3d(&pfx_dir, version).await
}

#[tauri::command]
pub fn check_vulkan() -> Result<graphics::VulkanInfo, String> {
    Ok(graphics::check_vulkan())
}

#[tauri::command]
pub async fn install_runtime(game_id: &str, component: &str) -> Result<String, String> {
    let pfx_dir = prefix::get_prefix_pfx_dir(game_id);

    // Find wine executable from prefix config
    let mut config = prefix::load_prefix_config(game_id)?;
    let versions = detector::scan_all_versions(&[]);
    let wine_version = versions
        .iter()
        .find(|v| v.id == config.wine_version_id)
        .ok_or("Wine version not found for this prefix")?;

    let result = runtime::install_runtime(&pfx_dir, &wine_version.path, component).await?;

    if !config.installed_runtimes.iter().any(|r| r == component) {
        config.installed_runtimes.push(component.to_string());
        prefix::save_prefix_config(game_id, &config)?;
    }

    Ok(result)
}

#[tauri::command]
pub fn list_available_runtimes() -> Result<Vec<runtime::RuntimeComponent>, String> {
    Ok(runtime::list_available_runtimes())
}

#[tauri::command]
pub fn get_installed_runtimes(game_id: &str) -> Result<Vec<String>, String> {
    let config = prefix::load_prefix_config(game_id)?;
    Ok(config.installed_runtimes)
}

#[tauri::command]
pub fn get_display_info() -> Result<display::DisplayInfo, String> {
    Ok(display::detect_display_info())
}

#[tauri::command]
pub fn get_recent_logs(lines: Option<usize>) -> Result<Vec<String>, String> {
    let log_dir = crate::utils::file_manager::get_logs_dir();
    let max_lines = lines.unwrap_or(100);

    // Find the most recent log file
    let mut log_files: Vec<_> = std::fs::read_dir(&log_dir)
        .map_err(|e| format!("Failed to read log dir: {}", e))?
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "log"))
        .collect();

    log_files
        .sort_by_key(|e| std::cmp::Reverse(e.metadata().ok().and_then(|m| m.modified().ok())));

    if let Some(latest) = log_files.first() {
        let content = std::fs::read_to_string(latest.path())
            .map_err(|e| format!("Failed to read log: {}", e))?;
        let all_lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
        let start = if all_lines.len() > max_lines {
            all_lines.len() - max_lines
        } else {
            0
        };
        Ok(all_lines[start..].to_vec())
    } else {
        Ok(Vec::new())
    }
}

#[tauri::command]
pub fn open_log_folder() -> Result<(), String> {
    let log_dir = crate::utils::file_manager::get_logs_dir();
    crate::utils::file_manager::ensure_dir(&log_dir)?;
    std::process::Command::new("xdg-open")
        .arg(log_dir.to_string_lossy().as_ref())
        .spawn()
        .map_err(|e| format!("Failed to open log folder: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn list_prefix_templates() -> Result<Vec<PrefixTemplate>, String> {
    prefix::list_templates()
}

#[tauri::command]
pub fn save_prefix_template(template: PrefixTemplate) -> Result<(), String> {
    prefix::save_template(&template)
}

// ============================================================
// Wine/Proton 远程版本管理
// ============================================================

#[tauri::command]
pub async fn fetch_remote_proton(
    settings: State<'_, Mutex<crate::configs::app_config::AppConfig>>,
) -> Result<Vec<detector::RemoteWineVersion>, String> {
    let custom_paths = {
        let s = settings.lock().map_err(|e| e.to_string())?;
        s.custom_search_paths.clone()
    };
    let installed = detector::scan_all_versions(&custom_paths);
    detector::fetch_remote_proton_versions(&installed).await
}

#[tauri::command]
pub async fn download_proton(app: tauri::AppHandle, download_url: String, tag: String, variant: String) -> Result<String, String> {
    detector::download_and_install_proton(&download_url, &tag, &variant, Some(app)).await
}

// ============================================================
// DXVK 版本管理命令
// ============================================================

#[tauri::command]
pub fn scan_local_dxvk() -> Result<Vec<graphics::DxvkLocalVersion>, String> {
    Ok(graphics::scan_local_dxvk_versions())
}

#[tauri::command]
pub fn detect_dxvk_status(game_id: &str) -> Result<graphics::DxvkInstalledStatus, String> {
    let pfx_dir = prefix::get_prefix_pfx_dir(game_id);
    Ok(graphics::detect_installed_dxvk(&pfx_dir))
}

#[tauri::command]
pub async fn fetch_dxvk_versions() -> Result<Vec<graphics::DxvkRemoteVersion>, String> {
    graphics::fetch_dxvk_releases(20).await
}
