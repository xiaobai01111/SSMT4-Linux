use crate::configs::wine_config::{PrefixConfig, PrefixTemplate};
use crate::utils::file_manager;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// 从数据库的游戏配置中解析游戏根目录（gameFolder 的父目录）
fn resolve_game_root_from_db(game_id: &str) -> Option<PathBuf> {
    let config_json = crate::configs::database::get_game_config(game_id)?;
    let data: serde_json::Value = serde_json::from_str(&config_json).ok()?;
    let game_folder = data.pointer("/other/gameFolder")?.as_str()?;
    let game_folder_path = PathBuf::from(game_folder);
    // gameFolder 是游戏文件目录（如 .../SRMI/StarRail），取其父目录作为游戏根目录
    game_folder_path.parent().map(|p| p.to_path_buf())
}

pub fn get_prefix_dir(game_id: &str) -> PathBuf {
    // 优先使用游戏安装目录下的 prefix/
    if let Some(game_root) = resolve_game_root_from_db(game_id) {
        return game_root.join("prefix");
    }
    // 回退到旧位置 (~/.config/ssmt4/prefixes/{game_id})
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
        if path.extension().is_some_and(|e| e == "json") {
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

#[allow(dead_code)]
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

/// 确保 prefix 中有 CJK 字体（将系统字体目录链接到 prefix 的 Fonts 目录）
pub fn ensure_cjk_fonts(game_id: &str) {
    let pfx_dir = get_prefix_pfx_dir(game_id);
    let fonts_dir = pfx_dir.join("drive_c").join("windows").join("Fonts");

    if !fonts_dir.exists() {
        // prefix 还没被 Proton 初始化过，跳过
        return;
    }

    // 常见系统字体目录
    let system_font_dirs = [
        "/usr/share/fonts",
        "/usr/local/share/fonts",
    ];

    // 查找系统中的 CJK 字体文件
    let cjk_patterns = ["noto", "cjk", "wqy", "wenquanyi", "droid", "source-han", "sarasa"];

    let home = std::env::var("HOME").unwrap_or_default();
    let user_fonts = PathBuf::from(&home).join(".local").join("share").join("fonts");
    let mut search_dirs: Vec<PathBuf> = system_font_dirs.iter().map(PathBuf::from).collect();
    if user_fonts.exists() {
        search_dirs.push(user_fonts);
    }

    let mut linked = 0u32;
    for search_dir in &search_dirs {
        if !search_dir.exists() {
            continue;
        }
        for entry in walkdir::WalkDir::new(search_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if !(name.ends_with(".ttf") || name.ends_with(".ttc") || name.ends_with(".otf")) {
                continue;
            }
            let is_cjk = cjk_patterns.iter().any(|p| name.contains(p));
            if !is_cjk {
                continue;
            }
            let target = fonts_dir.join(entry.file_name());
            if target.exists() {
                continue;
            }
            if let Err(e) = std::os::unix::fs::symlink(entry.path(), &target) {
                warn!("字体链接失败: {} -> {}: {}", entry.path().display(), target.display(), e);
            } else {
                linked += 1;
            }
        }
    }
    if linked > 0 {
        info!("已链接 {} 个 CJK 字体到 prefix: {}", linked, fonts_dir.display());
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrefixInfo {
    pub game_id: String,
    pub exists: bool,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub config: Option<PrefixConfig>,
}
