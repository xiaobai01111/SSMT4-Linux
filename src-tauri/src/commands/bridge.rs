use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tauri::Emitter;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{debug, error, info, warn};

/// Bridge configuration that gets serialized to bridge-config.json.
/// The bridge executable reads this file — all data is provided by the frontend,
/// nothing is hardcoded in the C++ bridge binary.
///
/// Each game has its own independent importer_folder, packages_folder, etc.
/// Modifying one game's config never affects another.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    pub importer: String,
    pub paths: BridgePaths,
    pub game: BridgeGameConfig,
    pub migoto: BridgeMigotoConfig,
    #[serde(flatten)]
    pub game_specific: HashMap<String, Value>,
    pub d3dx_ini: Value,
    pub signatures: BridgeSignatures,
    pub extra_libraries: BridgeExtraLibraries,
    pub custom_launch: BridgeCustomLaunch,
    pub pre_launch: BridgeShellCommand,
    pub post_load: BridgeShellCommand,
    pub jadeite: BridgeJadeite,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgePaths {
    pub app_root: String,
    pub importer_folder: String,
    pub packages_folder: String,
    pub game_folder: String,
    pub game_exe: String,
    pub cache_folder: String,
    pub mod_folder: String,
    pub shader_fixes_folder: String,
    pub d3dx_ini: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeGameConfig {
    pub start_exe: String,
    pub start_args: Vec<String>,
    pub work_dir: String,
    pub process_name: String,
    pub process_start_method: String,
    pub process_priority: String,
    pub process_timeout: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeMigotoConfig {
    pub use_hook: bool,
    pub enforce_rendering: bool,
    pub enable_hunting: bool,
    pub dump_shaders: bool,
    pub mute_warnings: bool,
    pub calls_logging: bool,
    pub debug_logging: bool,
    pub unsafe_mode: bool,
    pub xxmi_dll_init_delay: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeSignatures {
    pub xxmi_public_key: String,
    pub deployed_migoto_signatures: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeExtraLibraries {
    pub enabled: bool,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeCustomLaunch {
    pub enabled: bool,
    pub cmd: String,
    pub inject_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeShellCommand {
    pub enabled: bool,
    pub cmd: String,
    pub wait: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeJadeite {
    pub enabled: bool,
    pub exe_path: String,
}

/// Message received from bridge stdout (one JSON object per line).
#[derive(Debug, Clone, Deserialize)]
pub struct BridgeMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub stage: String,
    #[serde(default)]
    pub current: u32,
    #[serde(default)]
    pub total: u32,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub pid: u32,
    #[serde(default)]
    pub level: String,
}

/// Convert a Linux path to a Windows Z: drive path for use inside Proton.
pub fn linux_to_wine_path(linux_path: &str) -> String {
    format!("Z:{}", linux_path.replace('/', "\\"))
}

/// Get the path to the bridge executable.
/// The bridge is deployed at: <app_root>/Windows/ssmt4-bridge.exe
pub fn get_bridge_exe_path(app_root: &Path) -> PathBuf {
    app_root.join("Windows").join("ssmt4-bridge.exe")
}

/// Generate the bridge-config.json file content from game configuration.
///
/// All paths are converted to Windows format (Z:\...) because the bridge
/// runs inside the Proton container and sees Windows paths.
///
/// `game_config_json` is the raw game config JSON from the database/frontend.
/// `importer_name` identifies which 3DMigoto importer to use (e.g. "WWMI").
/// `app_root` is the Linux-side SSMT4-Linux root directory.
pub fn build_bridge_config(
    importer_name: &str,
    app_root: &Path,
    game_folder_linux: &str,
    game_exe_name: &str,
    game_config_json: Option<&Value>,
) -> BridgeConfig {
    let app_root_str = app_root.to_string_lossy();

    // Extract game-specific settings from the config JSON, or use defaults
    // Settings are stored at config.other.migoto by the frontend
    let gs = game_config_json
        .and_then(|c| c.pointer("/other/migoto"))
        .or_else(|| game_config_json.and_then(|c| c.get("migoto")))
        .cloned()
        .unwrap_or_else(|| json!({}));

    // 3Dmigoto-data 路径：优先使用用户自定义路径，否则使用默认
    let migoto_data_linux = gs
        .get("migoto_path")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{}/3Dmigoto-data", app_root_str));

    // Per-game isolation: each importer has its own folder
    let importer_folder_linux = gs
        .get("importer_folder")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{}/{}", migoto_data_linux, importer_name));

    let packages_folder_linux = format!("{}/Packages/XXMI", migoto_data_linux);
    let cache_folder_linux = format!("{}/Cache", app_root_str);

    let game_specific_section = game_config_json
        .and_then(|c| c.get(importer_name.to_ascii_lowercase().as_str()))
        .cloned()
        .unwrap_or_else(|| json!({}));

    let mut game_specific_map: HashMap<String, Value> = HashMap::new();
    game_specific_map.insert(
        importer_name.to_ascii_lowercase(),
        game_specific_section,
    );

    // Mod 文件夹 & ShaderFixes 文件夹：优先用户自定义
    let mod_folder_linux = gs
        .get("mod_folder")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{}/Mods", importer_folder_linux));

    let shader_fixes_folder_linux = gs
        .get("shader_fixes_folder")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{}/ShaderFixes", importer_folder_linux));

    let d3dx_ini_linux = gs
        .get("d3dx_ini_path")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{}/d3dx.ini", importer_folder_linux));

    info!(
        "bridge paths: migoto_data={}, importer={}, mods={}, shaders={}, d3dx_ini={}",
        migoto_data_linux, importer_folder_linux, mod_folder_linux,
        shader_fixes_folder_linux, d3dx_ini_linux
    );

    BridgeConfig {
        importer: importer_name.to_string(),
        paths: BridgePaths {
            app_root: linux_to_wine_path(&migoto_data_linux),
            importer_folder: linux_to_wine_path(&importer_folder_linux),
            packages_folder: linux_to_wine_path(&packages_folder_linux),
            game_folder: linux_to_wine_path(game_folder_linux),
            game_exe: game_exe_name.to_string(),
            cache_folder: linux_to_wine_path(&cache_folder_linux),
            mod_folder: linux_to_wine_path(&mod_folder_linux),
            shader_fixes_folder: linux_to_wine_path(&shader_fixes_folder_linux),
            d3dx_ini: linux_to_wine_path(&d3dx_ini_linux),
        },
        game: BridgeGameConfig {
            start_exe: game_exe_name.to_string(),
            start_args: gs
                .get("start_args")
                .and_then(|v| {
                    // 支持字符串（空格分隔）和 JSON 数组两种格式
                    if let Some(s) = v.as_str() {
                        Some(
                            s.split_whitespace()
                                .filter(|a| !a.is_empty())
                                .map(|a| a.to_string())
                                .collect::<Vec<String>>(),
                        )
                    } else {
                        serde_json::from_value(v.clone()).ok()
                    }
                })
                .unwrap_or_default(),
            work_dir: linux_to_wine_path(game_folder_linux),
            process_name: game_exe_name.to_string(),
            process_start_method: gs
                .get("process_start_method")
                .and_then(|v| v.as_str())
                .unwrap_or("Native")
                .to_string(),
            process_priority: gs
                .get("process_priority")
                .and_then(|v| v.as_str())
                .unwrap_or("Normal")
                .to_string(),
            process_timeout: gs
                .get("process_timeout")
                .and_then(|v| v.as_u64())
                .unwrap_or(30) as u32,
        },
        migoto: BridgeMigotoConfig {
            use_hook: gs.get("use_hook").and_then(|v| v.as_bool()).unwrap_or(true),
            enforce_rendering: gs
                .get("enforce_rendering")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            enable_hunting: gs
                .get("enable_hunting")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            dump_shaders: gs
                .get("dump_shaders")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            mute_warnings: gs
                .get("mute_warnings")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            calls_logging: gs
                .get("calls_logging")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            debug_logging: gs
                .get("debug_logging")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            unsafe_mode: gs
                .get("unsafe_mode")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            xxmi_dll_init_delay: gs
                .get("xxmi_dll_init_delay")
                .and_then(|v| v.as_u64())
                .unwrap_or(500) as u32,
        },
        game_specific: game_specific_map,
        d3dx_ini: gs.get("d3dx_ini").cloned().unwrap_or_else(|| {
            json!({
                "core": {
                    "Loader": { "loader": "XXMI Launcher.exe" }
                },
                "enforce_rendering": {
                    "Rendering": { "texture_hash": 1, "track_texture_updates": 1 }
                },
                "calls_logging": {
                    "Logging": { "calls": { "on": 1, "off": 0 } }
                },
                "debug_logging": {
                    "Logging": { "debug": { "on": 1, "off": 0 } }
                },
                "mute_warnings": {
                    "Logging": { "show_warnings": { "on": 0, "off": 1 } }
                },
                "enable_hunting": {
                    "Hunting": { "hunting": { "on": 2, "off": 0 } }
                },
                "dump_shaders": {
                    "Hunting": { "marking_actions": { "on": "clipboard hlsl asm regex", "off": "clipboard" } }
                }
            })
        }),
        signatures: BridgeSignatures {
            xxmi_public_key: gs
                .get("xxmi_public_key")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            deployed_migoto_signatures: HashMap::new(),
        },
        extra_libraries: BridgeExtraLibraries {
            enabled: gs
                .get("extra_libraries_enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            paths: gs
                .get("extra_libraries_paths")
                .and_then(|v| v.as_str())
                .map(|s| {
                    s.lines()
                        .map(|l| l.trim())
                        .filter(|l| !l.is_empty())
                        .map(|l| linux_to_wine_path(l))
                        .collect()
                })
                .unwrap_or_default(),
        },
        custom_launch: BridgeCustomLaunch {
            enabled: gs
                .get("custom_launch_enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            cmd: gs
                .get("custom_launch_cmd")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            inject_mode: gs
                .get("custom_launch_inject_mode")
                .and_then(|v| v.as_str())
                .unwrap_or("Hook")
                .to_string(),
        },
        pre_launch: BridgeShellCommand {
            enabled: gs
                .get("pre_launch_enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            cmd: gs
                .get("pre_launch_cmd")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            wait: gs
                .get("pre_launch_wait")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
        },
        post_load: BridgeShellCommand {
            enabled: gs
                .get("post_load_enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            cmd: gs
                .get("post_load_cmd")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            wait: gs
                .get("post_load_wait")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
        },
        jadeite: {
            let jadeite_enabled = gs
                .get("jadeite_enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let jadeite_path_linux = gs
                .get("jadeite_path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            BridgeJadeite {
                enabled: jadeite_enabled && !jadeite_path_linux.is_empty(),
                exe_path: if jadeite_path_linux.is_empty() {
                    String::new()
                } else {
                    linux_to_wine_path(&jadeite_path_linux)
                },
            }
        },
    }
}

/// Write the bridge config to a temp file and return the Linux path.
pub fn write_bridge_config(
    config: &BridgeConfig,
    app_root: &Path,
) -> Result<PathBuf, String> {
    let config_dir = app_root.join("Cache").join("bridge");
    std::fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create bridge config dir: {}", e))?;

    let config_path = config_dir.join("bridge-config.json");
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize bridge config: {}", e))?;

    std::fs::write(&config_path, json)
        .map_err(|e| format!("Failed to write bridge config: {}", e))?;

    info!("Bridge config written to: {}", config_path.display());
    Ok(config_path)
}

/// Run the bridge executable inside the same Proton container as the game.
///
/// This function:
/// 1. Writes bridge-config.json
/// 2. Launches ssmt4-bridge.exe using the same Proton runner + env + prefix
/// 3. Reads stdout JSON lines and emits events to the frontend
/// 4. Returns Ok on success, Err on failure
///
/// The bridge MUST run in the same container as the game because:
/// - DLL injection requires shared process address space
/// - Windows API calls (EnumWindows, CreateToolhelp32Snapshot) need the same session
/// - File paths inside the container are relative to the Wine prefix
pub async fn run_bridge(
    app: &tauri::AppHandle,
    game_name: &str,
    bridge_config: &BridgeConfig,
    app_root: &Path,
    proton_program: &Path,
    proton_args_prefix: &[String],
    env: &HashMap<String, String>,
    working_dir: &Path,
) -> Result<u32, String> {
    let config_path = write_bridge_config(bridge_config, app_root)?;
    let bridge_exe = get_bridge_exe_path(app_root);

    if !bridge_exe.exists() {
        return Err(format!(
            "Bridge executable not found: {}. Please ensure ssmt4-bridge.exe is built and deployed.",
            bridge_exe.display()
        ));
    }

    // Build command: same Proton runner as the game, but running bridge.exe instead
    let config_wine_path = linux_to_wine_path(&config_path.to_string_lossy());
    let bridge_wine_path = linux_to_wine_path(&bridge_exe.to_string_lossy());

    let mut cmd = tokio::process::Command::new(proton_program);

    // Add Proton prefix args (e.g. "run" for direct proton, or umu-run args)
    for arg in proton_args_prefix {
        cmd.arg(arg);
    }

    // The actual exe to run inside the container
    cmd.arg(&bridge_wine_path);
    cmd.arg("--config");
    cmd.arg(&config_wine_path);

    // Same environment as the game — critical for same container
    cmd.envs(env);

    if working_dir.exists() {
        cmd.current_dir(working_dir);
    }

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    info!(
        "Launching bridge: {} {:?} {} --config {}",
        proton_program.display(),
        proton_args_prefix,
        bridge_wine_path,
        config_wine_path
    );

    let mut child = cmd.spawn().map_err(|e| {
        format!(
            "Failed to start bridge process: {}. Proton={}, Bridge={}",
            e,
            proton_program.display(),
            bridge_exe.display()
        )
    })?;

    let stdout = child
        .stdout
        .take()
        .ok_or("Failed to capture bridge stdout")?;

    // Pipe stderr to game log
    if let Some(stderr) = child.stderr.take() {
        let gn = game_name.to_string();
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                debug!("[bridge stderr] {}", line);
                crate::commands::game_log::append_game_log_line(
                    &gn, "DEBUG", "bridge-stderr", &line,
                );
            }
        });
    }

    // Read stdout JSON lines from bridge
    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();
    let mut game_pid: u32 = 0;

    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        debug!("[bridge] {}", line);
        crate::commands::game_log::append_game_log_line(
            game_name, "DEBUG", "bridge", &line,
        );

        match serde_json::from_str::<BridgeMessage>(&line) {
            Ok(msg) => {
                match msg.msg_type.as_str() {
                    "status" => {
                        info!("[bridge] {}", msg.message);
                        let _ = app.emit(
                            "game-lifecycle",
                            json!({
                                "event": "bridge-status",
                                "game": game_name,
                                "message": msg.message
                            }),
                        );
                    }
                    "progress" => {
                        let _ = app.emit(
                            "game-lifecycle",
                            json!({
                                "event": "bridge-progress",
                                "game": game_name,
                                "stage": msg.stage,
                                "current": msg.current,
                                "total": msg.total
                            }),
                        );
                    }
                    "warning" => {
                        warn!("[bridge] {}", msg.message);
                        crate::commands::game_log::append_game_log_line(
                            game_name, "WARN", "bridge", &msg.message,
                        );
                    }
                    "error" => {
                        error!("[bridge] {} - {}", msg.code, msg.message);
                        let _ = app.emit(
                            "game-lifecycle",
                            json!({
                                "event": "bridge-error",
                                "game": game_name,
                                "code": msg.code,
                                "message": msg.message
                            }),
                        );
                        // Wait for process to finish then return error
                        let _ = child.wait().await;
                        return Err(format!("Bridge error [{}]: {}", msg.code, msg.message));
                    }
                    "inject_result" => {
                        game_pid = msg.pid;
                        info!(
                            "[bridge] Injection {}: method={}, pid={}",
                            if msg.success { "succeeded" } else { "failed" },
                            msg.method,
                            msg.pid
                        );
                    }
                    "log" => {
                        let level = match msg.level.as_str() {
                            "error" => "ERROR",
                            "warn" => "WARN",
                            "info" => "INFO",
                            _ => "DEBUG",
                        };
                        crate::commands::game_log::append_game_log_line(
                            game_name, level, "bridge", &msg.message,
                        );
                    }
                    "done" => {
                        if msg.success {
                            info!("[bridge] Completed successfully");
                        } else {
                            warn!("[bridge] Completed with failure");
                        }
                        break;
                    }
                    _ => {
                        debug!("[bridge] Unknown message type: {}", msg.msg_type);
                    }
                }
            }
            Err(e) => {
                // Non-JSON line from bridge (e.g. Wine debug output)
                debug!("[bridge raw] {}", line);
            }
        }
    }

    // Wait for bridge process to exit
    match child.wait().await {
        Ok(status) => {
            if status.success() {
                info!("Bridge process exited successfully");
                Ok(game_pid)
            } else {
                let code = status.code().unwrap_or(-1);
                Err(format!("Bridge process exited with code {}", code))
            }
        }
        Err(e) => Err(format!("Failed to wait for bridge process: {}", e)),
    }
}
