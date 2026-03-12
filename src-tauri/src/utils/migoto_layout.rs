use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigotoImporterBehavior {
    pub default_use_hook: bool,
    pub default_enforce_rendering: bool,
    pub default_process_timeout: u32,
    pub default_xxmi_dll_init_delay: u32,
    pub required_start_args: Vec<String>,
    #[serde(default)]
    pub injection_locked: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockedImporterLegacyDefaults {
    pub enforce_rendering: bool,
    pub process_timeout: u32,
    pub xxmi_dll_init_delay: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MigotoLayoutManifest {
    default_importer: String,
    game_importers: HashMap<String, String>,
    importer_markers: Vec<String>,
    importer_behaviors: HashMap<String, MigotoImporterBehavior>,
    default_importer_behavior: MigotoImporterBehavior,
    locked_importer_legacy_defaults: LockedImporterLegacyDefaults,
}

#[derive(Debug, Clone)]
pub struct MigotoPathState {
    pub importer: String,
    pub migoto_enabled: bool,
    pub migoto_path: PathBuf,
    pub importer_folder: PathBuf,
    pub mod_folder: PathBuf,
    pub shader_fixes_folder: PathBuf,
    pub d3dx_ini_path: PathBuf,
}

static MIGOTO_LAYOUT: LazyLock<MigotoLayoutManifest> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../../src/shared/migoto-layout.json"))
        .expect("invalid shared migoto layout manifest")
});

pub fn trim_value(value: Option<&str>) -> String {
    value.unwrap_or_default().trim().to_string()
}

fn migoto_section<'a>(config: &'a Value) -> Option<&'a Value> {
    config
        .pointer("/other/migoto")
        .or_else(|| config.get("migoto"))
}

pub fn string_to_path(value: Option<&str>) -> Option<PathBuf> {
    let trimmed = trim_value(value);
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

pub fn default_importer() -> &'static str {
    let trimmed = MIGOTO_LAYOUT.default_importer.trim();
    if trimmed.is_empty() {
        "WWMI"
    } else {
        trimmed
    }
}

pub fn normalize_importer_name(importer_name: &str) -> String {
    let normalized = importer_name.trim().to_ascii_uppercase();
    if normalized.is_empty() {
        default_importer().to_string()
    } else {
        normalized
    }
}

pub fn required_importer_for_game(game_name: &str) -> Option<String> {
    MIGOTO_LAYOUT
        .game_importers
        .get(game_name)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn resolve_importer(game_name: &str, configured: Option<&str>) -> String {
    if let Some(mapped) = required_importer_for_game(game_name) {
        return normalize_importer_name(&mapped);
    }

    let normalized = configured.unwrap_or_default().trim().to_ascii_uppercase();
    if normalized.is_empty() {
        default_importer().to_string()
    } else {
        normalized
    }
}

pub fn importer_behavior(importer_name: &str) -> MigotoImporterBehavior {
    MIGOTO_LAYOUT
        .importer_behaviors
        .get(&normalize_importer_name(importer_name))
        .cloned()
        .unwrap_or_else(|| MIGOTO_LAYOUT.default_importer_behavior.clone())
}

pub fn locked_importer_legacy_defaults() -> &'static LockedImporterLegacyDefaults {
    &MIGOTO_LAYOUT.locked_importer_legacy_defaults
}

fn path_basename(path: &Path) -> String {
    path.file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default()
}

pub fn looks_like_migoto_importer_folder(path: &Path) -> bool {
    MIGOTO_LAYOUT
        .importer_markers
        .iter()
        .any(|marker| path.join(marker).exists())
}

pub fn resolve_migoto_importer_folder(
    migoto_data_path: &Path,
    importer_name: &str,
    explicit_importer_folder: Option<&str>,
) -> PathBuf {
    if let Some(explicit) = explicit_importer_folder.filter(|value| !value.trim().is_empty()) {
        return PathBuf::from(explicit);
    }

    let normalized_importer = normalize_importer_name(importer_name);
    if looks_like_migoto_importer_folder(migoto_data_path)
        || path_basename(migoto_data_path).eq_ignore_ascii_case(&normalized_importer)
    {
        return migoto_data_path.to_path_buf();
    }

    let nested_importer_folder = migoto_data_path.join(&normalized_importer);
    if nested_importer_folder.exists() {
        return nested_importer_folder;
    }

    migoto_data_path.to_path_buf()
}

pub fn ensure_required_start_args(start_args: &mut Vec<String>, importer_name: &str) {
    let behavior = importer_behavior(importer_name);
    for required_arg in behavior.required_start_args {
        if start_args
            .iter()
            .any(|arg| arg.eq_ignore_ascii_case(&required_arg))
        {
            continue;
        }
        start_args.push(required_arg);
    }
}

pub fn resolve_migoto_path_state(
    importer_name: &str,
    migoto: Option<&Value>,
    default_migoto_path: PathBuf,
) -> MigotoPathState {
    let importer = normalize_importer_name(importer_name);
    let migoto_path = string_to_path(
        migoto
            .and_then(|value| value.get("migoto_path"))
            .and_then(|value| value.as_str()),
    )
    .unwrap_or(default_migoto_path);

    let importer_folder = resolve_migoto_importer_folder(
        &migoto_path,
        &importer,
        migoto
            .and_then(|value| value.get("importer_folder"))
            .and_then(|value| value.as_str()),
    );

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

    let d3dx_ini_path = string_to_path(
        migoto
            .and_then(|value| value.get("d3dx_ini_path"))
            .and_then(|value| value.as_str()),
    )
    .unwrap_or_else(|| importer_folder.join("d3dx.ini"));

    let migoto_enabled = migoto
        .and_then(|value| value.get("enabled"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false);

    MigotoPathState {
        importer,
        migoto_enabled,
        migoto_path,
        importer_folder,
        mod_folder,
        shader_fixes_folder,
        d3dx_ini_path,
    }
}

pub fn resolve_migoto_path_state_for_game(
    game_name: &str,
    config: &Value,
    default_migoto_path: PathBuf,
) -> MigotoPathState {
    let migoto = migoto_section(config);
    let importer = resolve_importer(
        game_name,
        migoto
            .and_then(|value| value.get("importer"))
            .and_then(|value| value.as_str()),
    );
    resolve_migoto_path_state(&importer, migoto, default_migoto_path)
}

#[cfg(test)]
mod tests {
    use super::{
        ensure_required_start_args, normalize_importer_name, resolve_importer,
        resolve_migoto_importer_folder,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("ssmt4-migoto-layout-{}-{}", name, unique))
    }

    #[test]
    fn resolves_required_importer_for_known_game() {
        assert_eq!(resolve_importer("ArknightsEndfield", Some("wwmi")), "EFMI");
        assert_eq!(normalize_importer_name(""), "WWMI");
    }

    #[test]
    fn appends_required_start_args_without_duplicates() {
        let mut args = vec!["-dx11".to_string(), "-windowed".to_string()];
        ensure_required_start_args(&mut args, "WWMI");
        assert_eq!(args, vec!["-dx11".to_string(), "-windowed".to_string()]);
    }

    #[test]
    fn uses_nested_importer_folder_when_present() {
        let base = unique_temp_dir("nested");
        let nested = base.join("EFMI");
        fs::create_dir_all(nested.join("Mods")).expect("create nested mods");

        let resolved = resolve_migoto_importer_folder(&base, "EFMI", None);
        assert_eq!(resolved, nested);

        let _ = fs::remove_dir_all(base);
    }
}
