use crate::configs::wine_config::{PrefixConfig, PrefixTemplate};
use crate::utils::file_manager;
use std::path::{Path, PathBuf};
use tracing::info;

pub fn get_prefix_dir(game_id: &str) -> PathBuf {
    file_manager::get_prefixes_dir().join(game_id)
}

pub fn get_prefix_pfx_dir(game_id: &str) -> PathBuf {
    get_prefix_dir(game_id).join("pfx")
}

pub fn get_prefix_config_path(game_id: &str) -> PathBuf {
    get_prefix_dir(game_id).join("prefix.json")
}

pub fn create_prefix(game_id: &str, config: &PrefixConfig) -> Result<PathBuf, String> {
    let prefix_dir = get_prefix_dir(game_id);
    let pfx_dir = prefix_dir.join("pfx");

    file_manager::ensure_dir(&prefix_dir)?;
    file_manager::ensure_dir(&pfx_dir)?;

    save_prefix_config(game_id, config)?;

    info!(
        "Created prefix for game {} at {}",
        game_id,
        prefix_dir.display()
    );
    Ok(prefix_dir)
}

pub fn create_prefix_from_template(
    game_id: &str,
    template: &PrefixTemplate,
) -> Result<PathBuf, String> {
    let config = PrefixConfig {
        wine_version_id: String::new(),
        arch: template.arch.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
        dxvk: template.dxvk.clone(),
        vkd3d: template.vkd3d.clone(),
        installed_runtimes: Vec::new(),
        env_overrides: template.env_overrides.clone(),
        template_id: Some(template.id.clone()),
        proton_settings: template.proton_settings.clone(),
    };
    create_prefix(game_id, &config)
}

pub fn delete_prefix(game_id: &str) -> Result<(), String> {
    let prefix_dir = get_prefix_dir(game_id);
    if prefix_dir.exists() {
        std::fs::remove_dir_all(&prefix_dir)
            .map_err(|e| format!("Failed to delete prefix {}: {}", prefix_dir.display(), e))?;
        info!("Deleted prefix for game {}", game_id);
    }
    Ok(())
}

pub fn load_prefix_config(game_id: &str) -> Result<PrefixConfig, String> {
    let config_path = get_prefix_config_path(game_id);
    if !config_path.exists() {
        return Err(format!("Prefix config not found for game {}", game_id));
    }
    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read prefix config: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse prefix config: {}", e))
}

pub fn save_prefix_config(game_id: &str, config: &PrefixConfig) -> Result<(), String> {
    let config_path = get_prefix_config_path(game_id);
    let content = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize prefix config: {}", e))?;
    std::fs::write(&config_path, content)
        .map_err(|e| format!("Failed to write prefix config: {}", e))
}

pub fn prefix_exists(game_id: &str) -> bool {
    get_prefix_dir(game_id).exists()
}

pub fn get_prefix_info(game_id: &str) -> Result<PrefixInfo, String> {
    let prefix_dir = get_prefix_dir(game_id);
    let pfx_dir = prefix_dir.join("pfx");
    let config = load_prefix_config(game_id).ok();

    let size = if pfx_dir.exists() {
        dir_size(&pfx_dir).unwrap_or(0)
    } else {
        0
    };

    Ok(PrefixInfo {
        game_id: game_id.to_string(),
        exists: prefix_dir.exists(),
        path: prefix_dir,
        size_bytes: size,
        config,
    })
}

fn dir_size(path: &Path) -> Result<u64, std::io::Error> {
    let mut total = 0;
    for entry in walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            total += entry.metadata()?.len();
        }
    }
    Ok(total)
}

// Templates

pub fn list_templates() -> Result<Vec<PrefixTemplate>, String> {
    let templates_dir = file_manager::get_templates_dir();
    if !templates_dir.exists() {
        return Ok(Vec::new());
    }

    let mut templates = Vec::new();
    let entries = std::fs::read_dir(&templates_dir)
        .map_err(|e| format!("Failed to read templates dir: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "json") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(template) = serde_json::from_str::<PrefixTemplate>(&content) {
                    templates.push(template);
                }
            }
        }
    }
    Ok(templates)
}

pub fn save_template(template: &PrefixTemplate) -> Result<(), String> {
    let templates_dir = file_manager::get_templates_dir();
    file_manager::ensure_dir(&templates_dir)?;

    let path = templates_dir.join(format!("{}.json", template.id));
    let content = serde_json::to_string_pretty(template)
        .map_err(|e| format!("Failed to serialize template: {}", e))?;
    std::fs::write(&path, content).map_err(|e| format!("Failed to write template: {}", e))
}

pub fn export_template_from_prefix(
    game_id: &str,
    template_id: &str,
    template_name: &str,
) -> Result<PrefixTemplate, String> {
    let config = load_prefix_config(game_id)?;
    let template = PrefixTemplate {
        id: template_id.to_string(),
        name: template_name.to_string(),
        description: format!("Exported from game {}", game_id),
        recommended_variant: crate::configs::wine_config::ProtonVariant::GEProton,
        arch: config.arch,
        dxvk: config.dxvk,
        vkd3d: config.vkd3d,
        required_runtimes: config.installed_runtimes,
        env_overrides: config.env_overrides,
        proton_settings: config.proton_settings,
    };
    save_template(&template)?;
    Ok(template)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrefixInfo {
    pub game_id: String,
    pub exists: bool,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub config: Option<PrefixConfig>,
}
