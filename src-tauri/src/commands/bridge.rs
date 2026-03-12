use crate::events::{emit_game_lifecycle, GameLifecycleEvent};
use crate::utils::migoto_layout::{
    ensure_required_start_args, importer_behavior, locked_importer_legacy_defaults,
    normalize_importer_name, resolve_migoto_path_state,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{debug, error, info, warn};

pub const BRIDGE_CONFIG_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BridgeJsonObject(pub Map<String, Value>);

impl BridgeJsonObject {
    fn from_value(value: Option<&Value>, context: &str) -> Self {
        match value {
            Some(Value::Object(map)) => Self(map.clone()),
            Some(Value::Null) | None => Self::default(),
            Some(other) => {
                warn!(
                    "bridge config field '{}' expects JSON object, got {:?}; falling back to empty object",
                    context,
                    other
                );
                Self::default()
            }
        }
    }

    #[cfg(test)]
    fn as_value(&self) -> Value {
        Value::Object(self.0.clone())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum BridgeStartArgsInput {
    String(String),
    Array(Vec<String>),
}

#[derive(Debug, Clone, Default)]
struct BridgeSourceConfig {
    start_args: Option<BridgeStartArgsInput>,
    process_start_method: Option<String>,
    process_priority: Option<String>,
    process_timeout: Option<u32>,
    use_hook: Option<bool>,
    use_dll_drop: bool,
    enforce_rendering: Option<bool>,
    enable_hunting: bool,
    dump_shaders: bool,
    mute_warnings: Option<bool>,
    calls_logging: bool,
    debug_logging: bool,
    unsafe_mode: bool,
    xxmi_dll_init_delay: Option<u32>,
    d3dx_ini: Option<BridgeJsonObject>,
    xxmi_public_key: Option<String>,
    extra_libraries_enabled: bool,
    extra_libraries_paths: Option<String>,
    custom_launch_enabled: bool,
    custom_launch_cmd: Option<String>,
    custom_launch_inject_mode: Option<String>,
    pre_launch_enabled: bool,
    pre_launch_cmd: Option<String>,
    pre_launch_wait: Option<bool>,
    post_load_enabled: bool,
    post_load_cmd: Option<String>,
    post_load_wait: Option<bool>,
    jadeite_enabled: bool,
    jadeite_path: Option<String>,
    migoto_path: Option<String>,
    importer_folder: Option<String>,
    mod_folder: Option<String>,
    shader_fixes_folder: Option<String>,
    d3dx_ini_path: Option<String>,
    enabled: Option<bool>,
}

impl BridgeSourceConfig {
    fn from_value(value: Option<&Value>) -> Self {
        let Some(root) = value else {
            return Self::default();
        };
        if !root.is_object() {
            warn!(
                "bridge source config expects JSON object, got {:?}; falling back to defaults",
                root
            );
            return Self::default();
        }

        Self {
            start_args: bridge_typed_field(root, "start_args"),
            process_start_method: bridge_string_field(root, "process_start_method"),
            process_priority: bridge_string_field(root, "process_priority"),
            process_timeout: bridge_typed_field(root, "process_timeout"),
            use_hook: bridge_typed_field(root, "use_hook"),
            use_dll_drop: bridge_typed_field(root, "use_dll_drop").unwrap_or(false),
            enforce_rendering: bridge_typed_field(root, "enforce_rendering"),
            enable_hunting: bridge_typed_field(root, "enable_hunting").unwrap_or(false),
            dump_shaders: bridge_typed_field(root, "dump_shaders").unwrap_or(false),
            mute_warnings: bridge_typed_field(root, "mute_warnings"),
            calls_logging: bridge_typed_field(root, "calls_logging").unwrap_or(false),
            debug_logging: bridge_typed_field(root, "debug_logging").unwrap_or(false),
            unsafe_mode: bridge_typed_field(root, "unsafe_mode").unwrap_or(false),
            xxmi_dll_init_delay: bridge_typed_field(root, "xxmi_dll_init_delay"),
            d3dx_ini: bridge_typed_field(root, "d3dx_ini"),
            xxmi_public_key: bridge_string_field(root, "xxmi_public_key"),
            extra_libraries_enabled: bridge_typed_field(root, "extra_libraries_enabled")
                .unwrap_or(false),
            extra_libraries_paths: bridge_string_field(root, "extra_libraries_paths"),
            custom_launch_enabled: bridge_typed_field(root, "custom_launch_enabled")
                .unwrap_or(false),
            custom_launch_cmd: bridge_string_field(root, "custom_launch_cmd"),
            custom_launch_inject_mode: bridge_string_field(root, "custom_launch_inject_mode"),
            pre_launch_enabled: bridge_typed_field(root, "pre_launch_enabled").unwrap_or(false),
            pre_launch_cmd: bridge_string_field(root, "pre_launch_cmd"),
            pre_launch_wait: bridge_typed_field(root, "pre_launch_wait"),
            post_load_enabled: bridge_typed_field(root, "post_load_enabled").unwrap_or(false),
            post_load_cmd: bridge_string_field(root, "post_load_cmd"),
            post_load_wait: bridge_typed_field(root, "post_load_wait"),
            jadeite_enabled: bridge_typed_field(root, "jadeite_enabled").unwrap_or(false),
            jadeite_path: bridge_string_field(root, "jadeite_path"),
            migoto_path: bridge_string_field(root, "migoto_path"),
            importer_folder: bridge_string_field(root, "importer_folder"),
            mod_folder: bridge_string_field(root, "mod_folder"),
            shader_fixes_folder: bridge_string_field(root, "shader_fixes_folder"),
            d3dx_ini_path: bridge_string_field(root, "d3dx_ini_path"),
            enabled: bridge_typed_field(root, "enabled"),
        }
    }

    fn start_args_vec(&self) -> Vec<String> {
        match &self.start_args {
            Some(BridgeStartArgsInput::String(raw)) => raw
                .split_whitespace()
                .filter(|arg| !arg.is_empty())
                .map(|arg| arg.to_string())
                .collect(),
            Some(BridgeStartArgsInput::Array(args)) => args.clone(),
            None => Vec::new(),
        }
    }

    fn path_state_value(&self) -> Option<Value> {
        let mut map = Map::new();

        insert_string_override(&mut map, "migoto_path", self.migoto_path.as_deref());
        insert_string_override(&mut map, "importer_folder", self.importer_folder.as_deref());
        insert_string_override(&mut map, "mod_folder", self.mod_folder.as_deref());
        insert_string_override(
            &mut map,
            "shader_fixes_folder",
            self.shader_fixes_folder.as_deref(),
        );
        insert_string_override(&mut map, "d3dx_ini_path", self.d3dx_ini_path.as_deref());

        if let Some(enabled) = self.enabled {
            map.insert("enabled".to_string(), Value::Bool(enabled));
        }

        if map.is_empty() {
            None
        } else {
            Some(Value::Object(map))
        }
    }
}

/// Bridge configuration that gets serialized to bridge-config.json.
/// The bridge executable reads this file — all data is provided by the frontend,
/// nothing is hardcoded in the C++ bridge binary.
///
/// Each game has its own independent importer_folder, packages_folder, etc.
/// Modifying one game's config never affects another.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    pub importer: String,
    pub paths: BridgePaths,
    pub game: BridgeGameConfig,
    pub migoto: BridgeMigotoConfig,
    #[serde(flatten)]
    pub game_specific: BTreeMap<String, BridgeJsonObject>,
    pub d3dx_ini: BridgeJsonObject,
    pub signatures: BridgeSignatures,
    pub extra_libraries: BridgeExtraLibraries,
    pub custom_launch: BridgeCustomLaunch,
    pub pre_launch: BridgeShellCommand,
    pub post_load: BridgeShellCommand,
    pub jadeite: BridgeJadeite,
}

impl BridgeConfig {
    fn validate(&self) -> Result<(), String> {
        if self.schema_version != BRIDGE_CONFIG_SCHEMA_VERSION {
            return Err(format!(
                "Unsupported bridge config schema version: {}",
                self.schema_version
            ));
        }

        validate_non_empty_field("importer", &self.importer)?;
        validate_non_empty_field("paths.app_root", &self.paths.app_root)?;
        validate_non_empty_field("paths.importer_folder", &self.paths.importer_folder)?;
        validate_non_empty_field("paths.packages_folder", &self.paths.packages_folder)?;
        validate_non_empty_field("paths.game_folder", &self.paths.game_folder)?;
        validate_non_empty_field("paths.game_exe", &self.paths.game_exe)?;
        validate_non_empty_field("paths.cache_folder", &self.paths.cache_folder)?;
        validate_non_empty_field("paths.mod_folder", &self.paths.mod_folder)?;
        validate_non_empty_field("paths.shader_fixes_folder", &self.paths.shader_fixes_folder)?;
        validate_non_empty_field("paths.d3dx_ini", &self.paths.d3dx_ini)?;
        validate_non_empty_field("game.start_exe", &self.game.start_exe)?;
        validate_non_empty_field("game.work_dir", &self.game.work_dir)?;
        validate_non_empty_field("game.process_name", &self.game.process_name)?;
        validate_non_empty_field("game.process_start_method", &self.game.process_start_method)?;
        validate_non_empty_field("game.process_priority", &self.game.process_priority)?;

        if self.game.process_timeout == 0 {
            return Err("game.process_timeout must be greater than 0".to_string());
        }

        if self.custom_launch.enabled {
            validate_non_empty_field("custom_launch.cmd", &self.custom_launch.cmd)?;
            validate_non_empty_field("custom_launch.inject_mode", &self.custom_launch.inject_mode)?;
        }
        if self.pre_launch.enabled {
            validate_non_empty_field("pre_launch.cmd", &self.pre_launch.cmd)?;
        }
        if self.post_load.enabled {
            validate_non_empty_field("post_load.cmd", &self.post_load.cmd)?;
        }
        if self.jadeite.enabled {
            validate_non_empty_field("jadeite.exe_path", &self.jadeite.exe_path)?;
        }

        Ok(())
    }
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
    pub use_dll_drop: bool,
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
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeMessageType {
    Status,
    Progress,
    Warning,
    Error,
    InjectResult,
    Log,
    Done,
    #[serde(other)]
    Unknown,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct BridgeMessage {
    #[serde(rename = "type")]
    pub msg_type: BridgeMessageType,
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

#[derive(Debug)]
pub(crate) struct BridgeLaunchContext<'a> {
    pub(crate) app: &'a tauri::AppHandle,
    pub(crate) game_name: &'a str,
    pub(crate) app_root: &'a Path,
    pub(crate) proton_program: &'a Path,
    pub(crate) proton_args_prefix: &'a [String],
    pub(crate) env: &'a HashMap<String, String>,
    pub(crate) working_dir: &'a Path,
}

#[derive(Debug)]
struct BridgeLaunchPaths {
    bridge_exe: PathBuf,
    config_path: PathBuf,
    bridge_wine_path: String,
    config_wine_path: String,
}

#[derive(Debug, Default)]
struct BridgeRunState {
    game_pid: u32,
}

enum BridgeStreamControl {
    Continue,
    Complete,
    Fail(String),
}

struct BridgeEventDispatcher<'a> {
    app: &'a tauri::AppHandle,
    game_name: &'a str,
}

/// Convert a Linux path to a Windows Z: drive path for use inside Proton.
pub fn linux_to_wine_path(linux_path: &str) -> String {
    format!("Z:{}", linux_path.replace('/', "\\"))
}

/// Get the path to the bridge executable.
/// The bridge is deployed at: <app_root>/Windows/ssmt4-bridge.exe
#[allow(dead_code)]
pub fn get_bridge_exe_path(app_root: &Path) -> PathBuf {
    app_root.join("Windows").join("ssmt4-bridge.exe")
}

fn validate_non_empty_field(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{} must not be empty", label))
    } else {
        Ok(())
    }
}

fn bridge_typed_field<T>(root: &Value, key: &str) -> Option<T>
where
    T: DeserializeOwned,
{
    let value = root.get(key)?;
    match serde_json::from_value::<T>(value.clone()) {
        Ok(parsed) => Some(parsed),
        Err(error) => {
            warn!(
                "bridge source field '{}' failed to deserialize as {}: {}; ignoring override",
                key,
                std::any::type_name::<T>(),
                error
            );
            None
        }
    }
}

fn bridge_string_field(root: &Value, key: &str) -> Option<String> {
    bridge_typed_field::<String>(root, key)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn insert_string_override(map: &mut Map<String, Value>, key: &str, value: Option<&str>) {
    if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
        map.insert(key.to_string(), Value::String(value.to_string()));
    }
}

fn importer_default_d3dx_ini(importer_name: &str) -> BridgeJsonObject {
    let (texture_hash, track_texture_updates) =
        match normalize_importer_name(importer_name).as_str() {
            "WWMI" => (1, 1),
            _ => (0, 0),
        };

    BridgeJsonObject::from_value(
        Some(&json!({
            "core": {
                "Loader": { "loader": "XXMI Launcher.exe" }
            },
            "enforce_rendering": {
                "Rendering": {
                    "texture_hash": texture_hash,
                    "track_texture_updates": track_texture_updates
                }
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
        })),
        "importer_default_d3dx_ini",
    )
}

fn heal_locked_importer_legacy_u32(
    locked_importer: bool,
    raw_value: Option<u64>,
    importer_default: u32,
    legacy_default: u32,
) -> u32 {
    match raw_value {
        Some(value)
            if locked_importer
                && value == legacy_default as u64
                && importer_default != legacy_default =>
        {
            importer_default
        }
        Some(value) => value as u32,
        None => importer_default,
    }
}

fn heal_locked_importer_legacy_bool(
    locked_importer: bool,
    raw_value: Option<bool>,
    importer_default: bool,
    legacy_default: bool,
) -> bool {
    match raw_value {
        Some(value)
            if locked_importer && value == legacy_default && importer_default != legacy_default =>
        {
            importer_default
        }
        Some(value) => value,
        None => importer_default,
    }
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
    let importer_name = normalize_importer_name(importer_name);
    let importer_behavior = importer_behavior(&importer_name);
    let locked_defaults = locked_importer_legacy_defaults();

    // Extract game-specific settings from the config JSON, or use defaults
    // Settings are stored at config.other.migoto by the frontend
    let gs_value = game_config_json
        .and_then(|c| c.pointer("/other/migoto"))
        .or_else(|| game_config_json.and_then(|c| c.get("migoto")))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let gs = BridgeSourceConfig::from_value(Some(&gs_value));
    let path_state_input = gs.path_state_value();

    let path_state = resolve_migoto_path_state(
        &importer_name,
        path_state_input.as_ref(),
        PathBuf::from(format!("{}/3Dmigoto-data", app_root_str)),
    );
    let migoto_data_linux = path_state.migoto_path.to_string_lossy().into_owned();
    let importer_folder_linux = path_state.importer_folder.to_string_lossy().into_owned();

    let packages_folder_linux = app_root
        .join("3Dmigoto-data")
        .join("Packages")
        .join("XXMI")
        .to_string_lossy()
        .into_owned();
    let cache_folder_linux = format!("{}/Cache", app_root_str);

    let mut start_args = gs.start_args_vec();
    ensure_required_start_args(&mut start_args, &importer_name);

    let game_specific_section = BridgeJsonObject::from_value(
        game_config_json.and_then(|c| c.get(importer_name.to_ascii_lowercase().as_str())),
        "game_specific",
    );

    let mut game_specific_map = BTreeMap::new();
    game_specific_map.insert(importer_name.to_ascii_lowercase(), game_specific_section);

    let mod_folder_linux = path_state.mod_folder.to_string_lossy().into_owned();
    let shader_fixes_folder_linux = path_state
        .shader_fixes_folder
        .to_string_lossy()
        .into_owned();
    let d3dx_ini_linux = path_state.d3dx_ini_path.to_string_lossy().into_owned();

    info!(
        "bridge paths: migoto_data={}, importer={}, mods={}, shaders={}, d3dx_ini={}",
        migoto_data_linux,
        importer_folder_linux,
        mod_folder_linux,
        shader_fixes_folder_linux,
        d3dx_ini_linux
    );

    BridgeConfig {
        schema_version: BRIDGE_CONFIG_SCHEMA_VERSION,
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
            start_args,
            work_dir: linux_to_wine_path(game_folder_linux),
            process_name: game_exe_name.to_string(),
            process_start_method: gs
                .process_start_method
                .clone()
                .unwrap_or_else(|| "Native".to_string()),
            process_priority: gs
                .process_priority
                .clone()
                .unwrap_or_else(|| "Normal".to_string()),
            process_timeout: heal_locked_importer_legacy_u32(
                importer_behavior.injection_locked,
                gs.process_timeout.map(u64::from),
                importer_behavior.default_process_timeout,
                locked_defaults.process_timeout,
            ),
        },
        migoto: BridgeMigotoConfig {
            use_hook: if importer_behavior.injection_locked {
                importer_behavior.default_use_hook
            } else {
                gs.use_hook.unwrap_or(importer_behavior.default_use_hook)
            },
            use_dll_drop: gs.use_dll_drop,
            enforce_rendering: heal_locked_importer_legacy_bool(
                importer_behavior.injection_locked,
                gs.enforce_rendering,
                importer_behavior.default_enforce_rendering,
                locked_defaults.enforce_rendering,
            ),
            enable_hunting: gs.enable_hunting,
            dump_shaders: gs.dump_shaders,
            mute_warnings: gs.mute_warnings.unwrap_or(true),
            calls_logging: gs.calls_logging,
            debug_logging: gs.debug_logging,
            unsafe_mode: gs.unsafe_mode,
            xxmi_dll_init_delay: heal_locked_importer_legacy_u32(
                importer_behavior.injection_locked,
                gs.xxmi_dll_init_delay.map(u64::from),
                importer_behavior.default_xxmi_dll_init_delay,
                locked_defaults.xxmi_dll_init_delay,
            ),
        },
        game_specific: game_specific_map,
        d3dx_ini: gs
            .d3dx_ini
            .clone()
            .unwrap_or_else(|| importer_default_d3dx_ini(&importer_name)),
        signatures: BridgeSignatures {
            xxmi_public_key: gs.xxmi_public_key.clone().unwrap_or_default(),
            deployed_migoto_signatures: HashMap::new(),
        },
        extra_libraries: BridgeExtraLibraries {
            enabled: gs.extra_libraries_enabled,
            paths: gs
                .extra_libraries_paths
                .as_deref()
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
            enabled: gs.custom_launch_enabled,
            cmd: gs.custom_launch_cmd.clone().unwrap_or_default(),
            inject_mode: gs
                .custom_launch_inject_mode
                .clone()
                .unwrap_or_else(|| "Hook".to_string()),
        },
        pre_launch: BridgeShellCommand {
            enabled: gs.pre_launch_enabled,
            cmd: gs.pre_launch_cmd.clone().unwrap_or_default(),
            wait: gs.pre_launch_wait.unwrap_or(true),
        },
        post_load: BridgeShellCommand {
            enabled: gs.post_load_enabled,
            cmd: gs.post_load_cmd.clone().unwrap_or_default(),
            wait: gs.post_load_wait.unwrap_or(true),
        },
        jadeite: {
            let jadeite_enabled = gs.jadeite_enabled;
            let jadeite_path_linux = gs.jadeite_path.clone().unwrap_or_default();
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
pub fn write_bridge_config(config: &BridgeConfig, app_root: &Path) -> Result<PathBuf, String> {
    config.validate()?;

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
#[allow(dead_code)]
pub(crate) async fn run_bridge(
    bridge_config: &BridgeConfig,
    context: BridgeLaunchContext<'_>,
) -> Result<u32, String> {
    let launch_paths = prepare_bridge_launch_paths(bridge_config, context.app_root)?;
    let mut child = spawn_bridge_process(&context, &launch_paths)?;
    let stdout = child
        .stdout
        .take()
        .ok_or("Failed to capture bridge stdout")?;

    if let Some(stderr) = child.stderr.take() {
        spawn_bridge_stderr_pipe(context.game_name, stderr);
    }

    let dispatcher = BridgeEventDispatcher {
        app: context.app,
        game_name: context.game_name,
    };
    let stdout_result = read_bridge_stdout(&dispatcher, stdout).await;
    let wait_result = wait_for_bridge_exit(&mut child).await;

    match stdout_result {
        Ok(state) => {
            wait_result?;
            Ok(state.game_pid)
        }
        Err(err) => {
            let _ = wait_result;
            Err(err)
        }
    }
}

fn prepare_bridge_launch_paths(
    bridge_config: &BridgeConfig,
    app_root: &Path,
) -> Result<BridgeLaunchPaths, String> {
    let config_path = write_bridge_config(bridge_config, app_root)?;
    let bridge_exe = get_bridge_exe_path(app_root);

    if !bridge_exe.exists() {
        return Err(format!(
            "Bridge executable not found: {}. Please ensure ssmt4-bridge.exe is built and deployed.",
            bridge_exe.display()
        ));
    }

    Ok(BridgeLaunchPaths {
        bridge_wine_path: linux_to_wine_path(&bridge_exe.to_string_lossy()),
        config_wine_path: linux_to_wine_path(&config_path.to_string_lossy()),
        bridge_exe,
        config_path,
    })
}

fn spawn_bridge_process(
    context: &BridgeLaunchContext<'_>,
    launch_paths: &BridgeLaunchPaths,
) -> Result<tokio::process::Child, String> {
    let mut cmd = build_bridge_command(context, launch_paths);
    info!(
        "Launching bridge: {} {:?} {} --config {}",
        context.proton_program.display(),
        context.proton_args_prefix,
        launch_paths.bridge_wine_path,
        launch_paths.config_wine_path
    );

    cmd.spawn().map_err(|e| {
        format!(
            "Failed to start bridge process: {}. Proton={}, Bridge={}, Config={}",
            e,
            context.proton_program.display(),
            launch_paths.bridge_exe.display(),
            launch_paths.config_path.display()
        )
    })
}

fn build_bridge_command(
    context: &BridgeLaunchContext<'_>,
    launch_paths: &BridgeLaunchPaths,
) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(context.proton_program);

    for arg in context.proton_args_prefix {
        cmd.arg(arg);
    }

    cmd.arg(&launch_paths.bridge_wine_path);
    cmd.arg("--config");
    cmd.arg(&launch_paths.config_wine_path);
    cmd.envs(context.env);

    if context.working_dir.exists() {
        cmd.current_dir(context.working_dir);
    }

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd
}

fn spawn_bridge_stderr_pipe(game_name: &str, stderr: tokio::process::ChildStderr) {
    let game_name = game_name.to_string();
    tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            debug!("[bridge stderr] {}", line);
            crate::commands::game_log::append_game_log_line(
                &game_name,
                "DEBUG",
                "bridge-stderr",
                &line,
            );
        }
    });
}

async fn read_bridge_stdout(
    dispatcher: &BridgeEventDispatcher<'_>,
    stdout: tokio::process::ChildStdout,
) -> Result<BridgeRunState, String> {
    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();
    let mut state = BridgeRunState::default();

    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        debug!("[bridge] {}", line);
        crate::commands::game_log::append_game_log_line(
            dispatcher.game_name,
            "DEBUG",
            "bridge",
            &line,
        );

        match serde_json::from_str::<BridgeMessage>(&line) {
            Ok(msg) => match dispatch_bridge_message(dispatcher, msg, &mut state) {
                BridgeStreamControl::Continue => {}
                BridgeStreamControl::Complete => break,
                BridgeStreamControl::Fail(err) => return Err(err),
            },
            Err(err) => {
                debug!("[bridge parse] {}", err);
                debug!("[bridge raw] {}", line);
            }
        }
    }

    Ok(state)
}

fn dispatch_bridge_message(
    dispatcher: &BridgeEventDispatcher<'_>,
    msg: BridgeMessage,
    state: &mut BridgeRunState,
) -> BridgeStreamControl {
    match msg.msg_type {
        BridgeMessageType::Status => {
            info!("[bridge] {}", msg.message);
            emit_bridge_event(
                dispatcher,
                GameLifecycleEvent::BridgeStatus {
                    game: dispatcher.game_name.to_string(),
                    message: msg.message,
                },
            );
            BridgeStreamControl::Continue
        }
        BridgeMessageType::Progress => {
            emit_bridge_event(
                dispatcher,
                GameLifecycleEvent::BridgeProgress {
                    game: dispatcher.game_name.to_string(),
                    stage: msg.stage,
                    current: msg.current,
                    total: msg.total,
                },
            );
            BridgeStreamControl::Continue
        }
        BridgeMessageType::Warning => {
            warn!("[bridge] {}", msg.message);
            crate::commands::game_log::append_game_log_line(
                dispatcher.game_name,
                "WARN",
                "bridge",
                &msg.message,
            );
            BridgeStreamControl::Continue
        }
        BridgeMessageType::Error => {
            error!("[bridge] {} - {}", msg.code, msg.message);
            emit_bridge_event(
                dispatcher,
                GameLifecycleEvent::BridgeError {
                    game: dispatcher.game_name.to_string(),
                    code: msg.code.clone(),
                    message: msg.message.clone(),
                },
            );
            BridgeStreamControl::Fail(format!("Bridge error [{}]: {}", msg.code, msg.message))
        }
        BridgeMessageType::InjectResult => {
            state.game_pid = msg.pid;
            info!(
                "[bridge] Injection {}: method={}, pid={}",
                if msg.success { "succeeded" } else { "failed" },
                msg.method,
                msg.pid
            );
            BridgeStreamControl::Continue
        }
        BridgeMessageType::Log => {
            let level = match msg.level.as_str() {
                "error" => "ERROR",
                "warn" => "WARN",
                "info" => "INFO",
                _ => "DEBUG",
            };
            crate::commands::game_log::append_game_log_line(
                dispatcher.game_name,
                level,
                "bridge",
                &msg.message,
            );
            BridgeStreamControl::Continue
        }
        BridgeMessageType::Done => {
            if msg.success {
                info!("[bridge] Completed successfully");
            } else {
                warn!("[bridge] Completed with failure");
            }
            BridgeStreamControl::Complete
        }
        BridgeMessageType::Unknown => {
            debug!("[bridge] Unknown message type");
            BridgeStreamControl::Continue
        }
    }
}

fn emit_bridge_event(dispatcher: &BridgeEventDispatcher<'_>, payload: GameLifecycleEvent) {
    emit_game_lifecycle(dispatcher.app, &payload);
}

async fn wait_for_bridge_exit(child: &mut tokio::process::Child) -> Result<(), String> {
    match child.wait().await {
        Ok(status) => {
            if status.success() {
                info!("Bridge process exited successfully");
                Ok(())
            } else {
                let code = status.code().unwrap_or(-1);
                Err(format!("Bridge process exited with code {}", code))
            }
        }
        Err(e) => Err(format!("Failed to wait for bridge process: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn build_test_bridge_config(importer_name: &str, migoto: Option<Value>) -> BridgeConfig {
        let game_config_json = migoto.map(|migoto| {
            json!({
                "other": {
                    "migoto": migoto
                }
            })
        });

        build_bridge_config(
            importer_name,
            Path::new("/tmp/ssmt4-test"),
            "/games/test",
            "game.exe",
            game_config_json.as_ref(),
        )
    }

    #[test]
    fn efmi_defaults_match_upstream() {
        let config = build_test_bridge_config("EFMI", None);

        assert_eq!(config.game.start_args, vec!["-force-d3d11"]);
        assert_eq!(config.game.process_timeout, 60);
        assert!(!config.migoto.use_hook);
        assert_eq!(config.migoto.xxmi_dll_init_delay, 0);
        assert_eq!(config.migoto.enforce_rendering, false);
        assert_eq!(
            config
                .d3dx_ini
                .as_value()
                .pointer("/enforce_rendering/Rendering/texture_hash")
                .and_then(|value| value.as_i64()),
            Some(0)
        );
        assert_eq!(
            config
                .d3dx_ini
                .as_value()
                .pointer("/enforce_rendering/Rendering/track_texture_updates")
                .and_then(|value| value.as_i64()),
            Some(0)
        );
    }

    #[test]
    fn efmi_heals_legacy_global_defaults() {
        let config = build_test_bridge_config(
            "EFMI",
            Some(json!({
                "process_timeout": 30,
                "enforce_rendering": true,
                "xxmi_dll_init_delay": 500
            })),
        );

        assert_eq!(config.game.process_timeout, 60);
        assert_eq!(config.migoto.enforce_rendering, false);
        assert_eq!(config.migoto.xxmi_dll_init_delay, 0);
    }

    #[test]
    fn efmi_keeps_explicit_non_legacy_overrides() {
        let config = build_test_bridge_config(
            "EFMI",
            Some(json!({
                "process_timeout": 75,
                "enforce_rendering": false,
                "xxmi_dll_init_delay": 250
            })),
        );

        assert_eq!(config.game.process_timeout, 75);
        assert_eq!(config.migoto.enforce_rendering, false);
        assert_eq!(config.migoto.xxmi_dll_init_delay, 250);
    }

    #[test]
    fn wwmi_defaults_match_upstream() {
        let config = build_test_bridge_config("WWMI", None);

        assert_eq!(config.game.start_args, vec!["-dx11"]);
        assert_eq!(config.game.process_timeout, 30);
        assert!(config.migoto.use_hook);
        assert_eq!(config.migoto.xxmi_dll_init_delay, 500);
        assert_eq!(config.migoto.enforce_rendering, true);
        assert_eq!(
            config
                .d3dx_ini
                .as_value()
                .pointer("/enforce_rendering/Rendering/texture_hash")
                .and_then(|value| value.as_i64()),
            Some(1)
        );
        assert_eq!(
            config
                .d3dx_ini
                .as_value()
                .pointer("/enforce_rendering/Rendering/track_texture_updates")
                .and_then(|value| value.as_i64()),
            Some(1)
        );
    }

    #[test]
    fn required_start_args_are_appended_without_duplicates() {
        let config = build_test_bridge_config(
            "EFMI",
            Some(json!({
                "start_args": "-windowed -force-d3d11"
            })),
        );

        assert_eq!(
            config.game.start_args,
            vec!["-windowed".to_string(), "-force-d3d11".to_string()]
        );
    }

    #[test]
    fn invalid_game_specific_section_falls_back_to_empty_object() {
        let config = build_bridge_config(
            "WWMI",
            Path::new("/tmp/ssmt4-test"),
            "/games/test",
            "game.exe",
            Some(&json!({
                "wwmi": "invalid-section"
            })),
        );

        assert_eq!(
            config
                .game_specific
                .get("wwmi")
                .map(|section| section.0.is_empty()),
            Some(true)
        );
    }

    #[test]
    fn invalid_d3dx_ini_override_falls_back_to_importer_default() {
        let config = build_test_bridge_config(
            "WWMI",
            Some(json!({
                "d3dx_ini": "invalid"
            })),
        );

        assert_eq!(
            config
                .d3dx_ini
                .as_value()
                .pointer("/enforce_rendering/Rendering/texture_hash")
                .and_then(|value| value.as_i64()),
            Some(1)
        );
    }

    #[test]
    fn bridge_config_serialization_keeps_flat_game_specific_shape() {
        let config = build_test_bridge_config("WWMI", None);
        let serialized = serde_json::to_value(&config).expect("serialize bridge config");

        assert_eq!(
            serialized.get("schemaVersion").and_then(Value::as_u64),
            Some(1)
        );
        assert!(serialized.get("wwmi").is_some());
        assert!(serialized
            .get("d3dx_ini")
            .and_then(|value| value.as_object())
            .is_some());
        assert!(serialized.get("game_specific").is_none());
    }

    #[test]
    fn bridge_config_validation_rejects_enabled_commands_without_cmd() {
        let mut config = build_test_bridge_config("WWMI", None);
        config.pre_launch.enabled = true;
        config.pre_launch.cmd.clear();

        let error = config.validate().expect_err("validation should fail");
        assert!(error.contains("pre_launch.cmd"));
    }
}
