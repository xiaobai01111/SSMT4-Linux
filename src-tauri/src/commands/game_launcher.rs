use crate::utils::ini_manager;
use crate::wine::{detector, prefix};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{info, warn, error};

#[tauri::command]
pub async fn start_game(
    _app: tauri::AppHandle,
    game_name: String,
    game_exe_path: String,
    wine_version_id: String,
) -> Result<String, String> {
    let game_exe = PathBuf::from(&game_exe_path);
    if !game_exe.exists() {
        return Err(format!("Game executable not found: {}", game_exe_path));
    }

    // Load prefix config
    let prefix_config = prefix::load_prefix_config(&game_name)?;
    let prefix_dir = prefix::get_prefix_dir(&game_name);
    let pfx_dir = prefix::get_prefix_pfx_dir(&game_name);

    // Find the selected wine/proton version
    let versions = detector::scan_all_versions(&[]);
    let wine_version = versions
        .iter()
        .find(|v| v.id == wine_version_id)
        .ok_or_else(|| format!("Wine version '{}' not found", wine_version_id))?;

    let proton_path = &wine_version.path;
    let settings = &prefix_config.proton_settings;

    // Build environment variables
    let mut env: HashMap<String, String> = HashMap::new();

    // Core Proton env
    env.insert(
        "STEAM_COMPAT_DATA_PATH".to_string(),
        prefix_dir.to_string_lossy().to_string(),
    );
    env.insert("WINEPREFIX".to_string(), pfx_dir.to_string_lossy().to_string());

    if let Some(steam_root) = detector::get_steam_root_path() {
        env.insert(
            "STEAM_COMPAT_CLIENT_INSTALL_PATH".to_string(),
            steam_root.to_string_lossy().to_string(),
        );
    }

    // Steam App ID
    if settings.steam_app_id != "0" && !settings.steam_app_id.is_empty() {
        env.insert("SteamAppId".to_string(), settings.steam_app_id.clone());
        env.insert("SteamGameId".to_string(), settings.steam_app_id.clone());
    }

    // Proton feature flags
    if settings.proton_media_use_gst {
        env.insert("PROTON_MEDIA_USE_GST".to_string(), "1".to_string());
    }
    if settings.proton_enable_wayland {
        env.insert("PROTON_ENABLE_WAYLAND".to_string(), "1".to_string());
    }
    if settings.proton_no_d3d12 {
        env.insert("PROTON_NO_D3D12".to_string(), "1".to_string());
    }
    if settings.mangohud {
        env.insert("MANGOHUD".to_string(), "1".to_string());
    }
    if settings.steam_deck_compat {
        env.insert("SteamDeck".to_string(), "1".to_string());
    }

    // Per-prefix env overrides (e.g. WINEDLLOVERRIDES)
    for (key, value) in &prefix_config.env_overrides {
        env.insert(key.clone(), value.clone());
    }

    // Custom env from proton_settings
    for (key, value) in &settings.custom_env {
        env.insert(key.clone(), value.clone());
    }

    // Build command based on pressure-vessel support
    let mut cmd: tokio::process::Command;

    if settings.use_pressure_vessel {
        if let Some(runtime_dir) = detector::find_steam_linux_runtime() {
            let entry_point = runtime_dir.join("_v2-entry-point");
            info!(
                "Launching with pressure-vessel: {} -> {} -> {}",
                entry_point.display(),
                proton_path.display(),
                game_exe.display()
            );
            cmd = tokio::process::Command::new(&entry_point);
            cmd.arg("--verb=waitforexitandrun")
                .arg("--")
                .arg(proton_path)
                .arg("waitforexitandrun")
                .arg(&game_exe);
        } else {
            warn!("SteamLinuxRuntime not found, falling back to direct proton launch");
            cmd = build_direct_proton_command(proton_path, &game_exe);
        }
    } else {
        cmd = build_direct_proton_command(proton_path, &game_exe);
    }

    // Set environment
    cmd.envs(&env);

    // Set working directory to game exe's parent
    if let Some(game_dir) = game_exe.parent() {
        cmd.current_dir(game_dir);
    }

    // Launch
    let child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to launch game: {}", e))?;

    let pid = child.id().unwrap_or(0);
    info!("Game launched with PID {}", pid);

    // Optionally capture output in background for logging
    tokio::spawn(async move {
        match child.wait_with_output().await {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                if !stdout.is_empty() {
                    info!("Game stdout: {}", stdout);
                }
                if !stderr.is_empty() {
                    warn!("Game stderr: {}", stderr);
                }
                info!("Game process exited with status: {}", output.status);
            }
            Err(e) => {
                error!("Failed to wait for game process: {}", e);
            }
        }
    });

    Ok(format!("Game launched (PID: {})", pid))
}

fn build_direct_proton_command(proton_path: &Path, game_exe: &Path) -> tokio::process::Command {
    info!(
        "Launching with direct proton: {} waitforexitandrun {}",
        proton_path.display(),
        game_exe.display()
    );
    let mut cmd = tokio::process::Command::new(proton_path);
    cmd.arg("waitforexitandrun").arg(game_exe);
    cmd
}

#[tauri::command]
pub fn check_3dmigoto_integrity(
    _app: tauri::AppHandle,
    _game_name: &str,
    game_path: &str,
) -> Result<bool, String> {
    let game_dir = PathBuf::from(game_path);
    let d3dx_ini = game_dir.join("d3dx.ini");

    if !d3dx_ini.exists() {
        return Ok(false);
    }

    let d3d11_dll = game_dir.join("d3d11.dll");
    let d3dcompiler = game_dir.join("d3dcompiler_47.dll");

    Ok(d3d11_dll.exists() && d3dcompiler.exists())
}

#[tauri::command]
pub fn toggle_symlink(
    game_path: &str,
    enabled: bool,
) -> Result<bool, String> {
    let game_dir = PathBuf::from(game_path);
    let ini_path = game_dir.join("d3dx.ini");

    if !ini_path.exists() {
        return Err("d3dx.ini not found".to_string());
    }

    let mut ini_data = ini_manager::load_ini(&ini_path)?;

    if enabled {
        ini_manager::set_value(&mut ini_data, "Loader", "target", "d3d11.dll");
    } else {
        ini_manager::remove_value(&mut ini_data, "Loader", "target");
    }

    ini_manager::save_ini(&ini_data, &ini_path)?;
    info!("Toggled symlink for {}: enabled={}", game_path, enabled);
    Ok(enabled)
}
