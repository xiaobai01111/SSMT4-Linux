use crate::commands::bridge_contract_generated::{
    self as bridge_contract, BridgeFieldContract, BridgeFieldKind, BridgeSectionContract,
};
use crate::utils::migoto_layout::{
    ensure_required_start_args, importer_behavior, locked_importer_legacy_defaults,
    normalize_importer_name, resolve_migoto_path_state,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tracing::{info, warn};

mod runtime;

#[allow(dead_code)]
pub(crate) type BridgeLaunchContext<'a> = runtime::BridgeLaunchContext<'a>;

pub const BRIDGE_CONFIG_SCHEMA_VERSION: u32 = 1;

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SharedBridgeGameDefaults {
    process_start_method: String,
    process_priority: String,
    process_timeout: u32,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SharedBridgeMigotoDefaultsSection {
    use_hook: bool,
    use_dll_drop: bool,
    enforce_rendering: bool,
    enable_hunting: bool,
    dump_shaders: bool,
    mute_warnings: bool,
    calls_logging: bool,
    debug_logging: bool,
    unsafe_mode: bool,
    xxmi_dll_init_delay: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SharedBridgeCustomLaunchDefaults {
    enabled: bool,
    inject_mode: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SharedBridgeShellCommandDefaults {
    enabled: bool,
    wait: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SharedBridgeExtraLibrariesDefaults {
    enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SharedBridgeJadeiteDefaults {
    enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SharedBridgeMigotoDefaults {
    #[serde(rename = "schemaVersion")]
    schema_version: u32,
    game: SharedBridgeGameDefaults,
    migoto: SharedBridgeMigotoDefaultsSection,
    custom_launch: SharedBridgeCustomLaunchDefaults,
    shell_command: SharedBridgeShellCommandDefaults,
    extra_libraries: SharedBridgeExtraLibrariesDefaults,
    jadeite: SharedBridgeJadeiteDefaults,
}

fn shared_bridge_migoto_defaults() -> &'static SharedBridgeMigotoDefaults {
    static DEFAULTS: OnceLock<SharedBridgeMigotoDefaults> = OnceLock::new();
    DEFAULTS.get_or_init(|| {
        let defaults = serde_json::from_str::<SharedBridgeMigotoDefaults>(include_str!(
            "../../../src/shared/bridgeMigotoDefaults.json"
        ))
        .expect("bridgeMigotoDefaults.json must remain valid");

        assert_eq!(
            defaults.schema_version, BRIDGE_CONFIG_SCHEMA_VERSION,
            "shared bridge defaults schema version must match Rust bridge schema version"
        );

        defaults
    })
}

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

#[derive(Debug, Clone)]
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

impl Default for BridgeSourceConfig {
    fn default() -> Self {
        let defaults = shared_bridge_migoto_defaults();
        Self {
            start_args: None,
            process_start_method: None,
            process_priority: None,
            process_timeout: None,
            use_hook: None,
            use_dll_drop: defaults.migoto.use_dll_drop,
            enforce_rendering: None,
            enable_hunting: defaults.migoto.enable_hunting,
            dump_shaders: defaults.migoto.dump_shaders,
            mute_warnings: None,
            calls_logging: defaults.migoto.calls_logging,
            debug_logging: defaults.migoto.debug_logging,
            unsafe_mode: defaults.migoto.unsafe_mode,
            xxmi_dll_init_delay: None,
            d3dx_ini: None,
            xxmi_public_key: None,
            extra_libraries_enabled: defaults.extra_libraries.enabled,
            extra_libraries_paths: None,
            custom_launch_enabled: defaults.custom_launch.enabled,
            custom_launch_cmd: None,
            custom_launch_inject_mode: None,
            pre_launch_enabled: defaults.shell_command.enabled,
            pre_launch_cmd: None,
            pre_launch_wait: None,
            post_load_enabled: defaults.shell_command.enabled,
            post_load_cmd: None,
            post_load_wait: None,
            jadeite_enabled: defaults.jadeite.enabled,
            jadeite_path: None,
            migoto_path: None,
            importer_folder: None,
            mod_folder: None,
            shader_fixes_folder: None,
            d3dx_ini_path: None,
            enabled: None,
        }
    }
}

impl BridgeSourceConfig {
    fn from_value(value: Option<&Value>) -> Self {
        let defaults = shared_bridge_migoto_defaults();
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
            start_args: bridge_typed_field(root, bridge_contract::migoto_form::START_ARGS),
            process_start_method: bridge_string_field(
                root,
                bridge_contract::migoto_form::PROCESS_START_METHOD,
            ),
            process_priority: bridge_string_field(
                root,
                bridge_contract::migoto_form::PROCESS_PRIORITY,
            ),
            process_timeout: bridge_typed_field(
                root,
                bridge_contract::migoto_form::PROCESS_TIMEOUT,
            ),
            use_hook: bridge_typed_field(root, bridge_contract::migoto_form::USE_HOOK),
            use_dll_drop: bridge_typed_field(root, "use_dll_drop")
                .unwrap_or(defaults.migoto.use_dll_drop),
            enforce_rendering: bridge_typed_field(
                root,
                bridge_contract::migoto_form::ENFORCE_RENDERING,
            ),
            enable_hunting: bridge_typed_field(root, bridge_contract::migoto_form::ENABLE_HUNTING)
                .unwrap_or(defaults.migoto.enable_hunting),
            dump_shaders: bridge_typed_field(root, bridge_contract::migoto_form::DUMP_SHADERS)
                .unwrap_or(defaults.migoto.dump_shaders),
            mute_warnings: bridge_typed_field(root, bridge_contract::migoto_form::MUTE_WARNINGS),
            calls_logging: bridge_typed_field(root, bridge_contract::migoto_form::CALLS_LOGGING)
                .unwrap_or(defaults.migoto.calls_logging),
            debug_logging: bridge_typed_field(root, bridge_contract::migoto_form::DEBUG_LOGGING)
                .unwrap_or(defaults.migoto.debug_logging),
            unsafe_mode: bridge_typed_field(root, bridge_contract::migoto_form::UNSAFE_MODE)
                .unwrap_or(defaults.migoto.unsafe_mode),
            xxmi_dll_init_delay: bridge_typed_field(
                root,
                bridge_contract::migoto_form::XXMI_DLL_INIT_DELAY,
            ),
            d3dx_ini: bridge_typed_field(root, bridge_contract::sections::D3DX_INI),
            xxmi_public_key: bridge_string_field(root, "xxmi_public_key"),
            extra_libraries_enabled: bridge_typed_field(
                root,
                bridge_contract::migoto_form::EXTRA_LIBRARIES_ENABLED,
            )
            .unwrap_or(defaults.extra_libraries.enabled),
            extra_libraries_paths: bridge_string_field(
                root,
                bridge_contract::migoto_form::EXTRA_LIBRARIES_PATHS,
            ),
            custom_launch_enabled: bridge_typed_field(
                root,
                bridge_contract::migoto_form::CUSTOM_LAUNCH_ENABLED,
            )
            .unwrap_or(defaults.custom_launch.enabled),
            custom_launch_cmd: bridge_string_field(
                root,
                bridge_contract::migoto_form::CUSTOM_LAUNCH_CMD,
            ),
            custom_launch_inject_mode: bridge_string_field(
                root,
                bridge_contract::migoto_form::CUSTOM_LAUNCH_INJECT_MODE,
            ),
            pre_launch_enabled: bridge_typed_field(
                root,
                bridge_contract::migoto_form::PRE_LAUNCH_ENABLED,
            )
            .unwrap_or(defaults.shell_command.enabled),
            pre_launch_cmd: bridge_string_field(root, bridge_contract::migoto_form::PRE_LAUNCH_CMD),
            pre_launch_wait: bridge_typed_field(
                root,
                bridge_contract::migoto_form::PRE_LAUNCH_WAIT,
            ),
            post_load_enabled: bridge_typed_field(
                root,
                bridge_contract::migoto_form::POST_LOAD_ENABLED,
            )
            .unwrap_or(defaults.shell_command.enabled),
            post_load_cmd: bridge_string_field(root, bridge_contract::migoto_form::POST_LOAD_CMD),
            post_load_wait: bridge_typed_field(root, bridge_contract::migoto_form::POST_LOAD_WAIT),
            jadeite_enabled: bridge_typed_field(root, "jadeite_enabled")
                .unwrap_or(defaults.jadeite.enabled),
            jadeite_path: bridge_string_field(root, "jadeite_path"),
            migoto_path: bridge_string_field(root, bridge_contract::migoto_form::MIGOTO_PATH),
            importer_folder: bridge_string_field(
                root,
                bridge_contract::migoto_form::IMPORTER_FOLDER,
            ),
            mod_folder: bridge_string_field(root, bridge_contract::migoto_form::MOD_FOLDER),
            shader_fixes_folder: bridge_string_field(
                root,
                bridge_contract::migoto_form::SHADER_FIXES_FOLDER,
            ),
            d3dx_ini_path: bridge_string_field(root, bridge_contract::migoto_form::D3DX_INI_PATH),
            enabled: bridge_typed_field(root, bridge_contract::migoto_form::ENABLED),
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

        insert_string_override(
            &mut map,
            bridge_contract::migoto_form::MIGOTO_PATH,
            self.migoto_path.as_deref(),
        );
        insert_string_override(
            &mut map,
            bridge_contract::migoto_form::IMPORTER_FOLDER,
            self.importer_folder.as_deref(),
        );
        insert_string_override(
            &mut map,
            bridge_contract::migoto_form::MOD_FOLDER,
            self.mod_folder.as_deref(),
        );
        insert_string_override(
            &mut map,
            bridge_contract::migoto_form::SHADER_FIXES_FOLDER,
            self.shader_fixes_folder.as_deref(),
        );
        insert_string_override(
            &mut map,
            bridge_contract::migoto_form::D3DX_INI_PATH,
            self.d3dx_ini_path.as_deref(),
        );

        if let Some(enabled) = self.enabled {
            map.insert(
                bridge_contract::migoto_form::ENABLED.to_string(),
                Value::Bool(enabled),
            );
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

        let serialized = serde_json::to_value(self)
            .map_err(|error| format!("serialize bridge config: {error}"))?;
        validate_bridge_json_contract(&serialized, bridge_contract::BRIDGE_CONFIG_CONTRACT)?;

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

/// Convert a Linux path to a Windows Z: drive path for use inside Proton.
pub fn linux_to_wine_path(linux_path: &str) -> String {
    format!("Z:{}", linux_path.replace('/', "\\"))
}

fn validate_non_empty_field(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{} must not be empty", label))
    } else {
        Ok(())
    }
}

fn validate_bridge_json_contract(
    root: &Value,
    contract: &[BridgeSectionContract],
) -> Result<(), String> {
    for section_contract in contract {
        let section_value = if let Some(section_name) = section_contract.section {
            root.get(section_name)
                .ok_or_else(|| format!("bridge config missing section '{}'", section_name))?
        } else {
            root
        };
        validate_bridge_section_contract(
            section_contract.section,
            section_value,
            section_contract.fields,
        )?;
    }
    Ok(())
}

fn validate_bridge_section_contract(
    section_name: Option<&str>,
    section_value: &Value,
    fields: &[BridgeFieldContract],
) -> Result<(), String> {
    for field in fields {
        let value = if section_name == Some(field.name) {
            section_value
        } else {
            section_value.get(field.name).ok_or_else(|| {
                format!(
                    "bridge config missing field '{}'",
                    bridge_contract_path(section_name, field.name)
                )
            })?
        };

        if !bridge_contract_kind_matches(value, field.kind) {
            return Err(format!(
                "bridge config field '{}' has invalid type",
                bridge_contract_path(section_name, field.name)
            ));
        }
    }

    Ok(())
}

fn bridge_contract_kind_matches(value: &Value, kind: BridgeFieldKind) -> bool {
    match kind {
        BridgeFieldKind::String => value.is_string(),
        BridgeFieldKind::Boolean => value.is_boolean(),
        BridgeFieldKind::Int => value.as_i64().is_some() || value.as_u64().is_some(),
        BridgeFieldKind::StringArray => value
            .as_array()
            .map(|items| items.iter().all(Value::is_string))
            .unwrap_or(false),
        BridgeFieldKind::Object => value.is_object(),
    }
}

fn bridge_contract_path(section_name: Option<&str>, field_name: &str) -> String {
    match section_name {
        Some(section) if section == field_name => section.to_string(),
        Some(section) => format!("{section}.{field_name}"),
        None => field_name.to_string(),
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
    let shared_defaults = shared_bridge_migoto_defaults();

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
        schema_version: shared_defaults.schema_version,
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
                .unwrap_or_else(|| shared_defaults.game.process_start_method.clone()),
            process_priority: gs
                .process_priority
                .clone()
                .unwrap_or_else(|| shared_defaults.game.process_priority.clone()),
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
            mute_warnings: gs
                .mute_warnings
                .unwrap_or(shared_defaults.migoto.mute_warnings),
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
                .unwrap_or_else(|| shared_defaults.custom_launch.inject_mode.clone()),
        },
        pre_launch: BridgeShellCommand {
            enabled: gs.pre_launch_enabled,
            cmd: gs.pre_launch_cmd.clone().unwrap_or_default(),
            wait: gs
                .pre_launch_wait
                .unwrap_or(shared_defaults.shell_command.wait),
        },
        post_load: BridgeShellCommand {
            enabled: gs.post_load_enabled,
            cmd: gs.post_load_cmd.clone().unwrap_or_default(),
            wait: gs
                .post_load_wait
                .unwrap_or(shared_defaults.shell_command.wait),
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

#[allow(dead_code)]
pub(crate) async fn run_bridge(
    bridge_config: &BridgeConfig,
    context: BridgeLaunchContext<'_>,
) -> Result<u32, String> {
    runtime::run_bridge(bridge_config, context).await
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
