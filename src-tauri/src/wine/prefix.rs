use crate::configs::wine_config::{PrefixConfig, PrefixTemplate};
use crate::utils::file_manager;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// 从数据库的游戏配置中解析游戏根目录
///
/// 优先从 gameFolder 的父目录推导，回退到 gamePath 向上两级推导。
fn resolve_game_root_from_db(game_id: &str) -> Option<PathBuf> {
    let game_id = crate::configs::game_identity::to_canonical_or_keep(game_id);
    let config_json = crate::configs::database::get_game_config(&game_id)?;
    let data: serde_json::Value = serde_json::from_str(&config_json).ok()?;

    // 优先：gameFolder 的父目录
    if let Some(game_folder) = data.pointer("/other/gameFolder")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        if let Some(parent) = PathBuf::from(game_folder).parent() {
            return Some(parent.to_path_buf());
        }
    }

    // 回退：gamePath 向上两级（exe → 数据子目录 → 游戏根目录）
    if let Some(game_path) = data.pointer("/other/gamePath")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        if let Some(root) = PathBuf::from(game_path).parent().and_then(|p| p.parent()) {
            return Some(root.to_path_buf());
        }
    }

    None
}

fn prefix_candidate_score(path: &Path) -> u32 {
    let mut score = 0u32;
    if path.join("prefix.json").exists() {
        score += 1;
    }
    let pfx = path.join("pfx");
    if pfx.exists() {
        score += 2;
        if pfx.join(".dxvk-version").exists() {
            score += 3;
        }
    }
    score
}

fn resolve_best_legacy_prefix(game_id: &str) -> Option<PathBuf> {
    let canonical = file_manager::get_prefixes_dir().join(game_id);
    let mut best_path: Option<PathBuf> = None;
    let mut best_score = 0u32;

    let mut candidates = vec![canonical.clone()];
    for alias in crate::configs::game_identity::legacy_aliases_for_canonical(game_id) {
        candidates.push(file_manager::get_prefixes_dir().join(alias));
    }

    for candidate in candidates {
        if !candidate.exists() {
            continue;
        }
        let score = prefix_candidate_score(&candidate);
        if score >= best_score {
            best_score = score;
            best_path = Some(candidate);
        }
    }

    best_path
}

pub fn get_prefix_dir(game_id: &str) -> PathBuf {
    let game_id = crate::configs::game_identity::to_canonical_or_keep(game_id);
    // 优先使用游戏安装目录下的 prefix/
    if let Some(game_root) = resolve_game_root_from_db(&game_id) {
        let preferred = game_root.join("prefix");
        if preferred.exists() {
            return preferred;
        }
        if let Some(legacy) = resolve_best_legacy_prefix(&game_id) {
            return legacy;
        }
        return preferred;
    }
    // 回退到旧位置 (~/.config/ssmt4/prefixes/{game_id})
    if let Some(best) = resolve_best_legacy_prefix(&game_id) {
        let canonical_path = file_manager::get_prefixes_dir().join(&game_id);
        if best != canonical_path && !canonical_path.exists() {
            if let Some(parent) = canonical_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if std::fs::rename(&best, &canonical_path).is_ok() {
                return canonical_path;
            }
        }
        return best;
    }
    file_manager::get_prefixes_dir().join(&game_id)
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

    // 写入字体替换注册表，让 Windows 程序能通过标准字体名找到 CJK 字体
    ensure_font_substitutes(&pfx_dir);
}

/// 在 Wine 注册表中添加 CJK 字体替换项（FontSubstitutes + FontLinkDefaultChar）
/// 等效于 winetricks cjkfonts 中的注册表设置
fn ensure_font_substitutes(pfx_dir: &Path) {
    let system_reg = pfx_dir.join("system.reg");
    if !system_reg.exists() {
        return;
    }

    let content = match std::fs::read_to_string(&system_reg) {
        Ok(c) => c,
        Err(_) => return,
    };

    // 检查是否已经有 FontSubstitutes 项（避免重复写入）
    if content.contains("\"MS Shell Dlg\"=\"Noto Sans CJK")
        || content.contains("\"MS Shell Dlg\"=\"WenQuanYi")
    {
        return;
    }

    // 选择系统中可用的 CJK 字体名
    let fonts_dir = pfx_dir.join("drive_c").join("windows").join("Fonts");
    let fallback_font = detect_available_cjk_font(&fonts_dir);
    if fallback_font.is_empty() {
        warn!("未找到可用的 CJK 字体，跳过注册表配置");
        return;
    }
    info!("使用 CJK 字体 '{}' 配置 Wine 注册表替换", fallback_font);

    // 构造注册表补丁
    let reg_patch = format!(
        r#"REGEDIT4

[HKEY_LOCAL_MACHINE\Software\Microsoft\Windows NT\CurrentVersion\FontSubstitutes]
"MS Shell Dlg"="{font}"
"MS Shell Dlg 2"="{font}"
"MS UI Gothic"="{font}"
"SimSun"="{font}"
"NSimSun"="{font}"
"PMingLiU"="{font}"
"MingLiU"="{font}"
"Microsoft YaHei"="{font}"
"Microsoft YaHei UI"="{font}"
"宋体"="{font}"
"新宋体"="{font}"

[HKEY_LOCAL_MACHINE\Software\Microsoft\Windows NT\CurrentVersion\FontLink\SystemLink]
"Tahoma"="{font}"
"Microsoft Sans Serif"="{font}"
"Arial"="{font}"
"Segoe UI"="{font}"
"Lucida Sans Unicode"="{font}"

"#,
        font = fallback_font
    );

    // 写入临时 .reg 文件，用 regedit 导入
    // 直接追加到 system.reg 更可靠（Wine regedit 可能不可用）
    let patch_marker = format!("\n;; SSMT4 CJK FontSubstitutes ({})\n", fallback_font);
    if content.contains("SSMT4 CJK FontSubstitutes") {
        return;
    }

    // 找到 FontSubstitutes 节并追加，或在文件末尾追加
    let substitutes_section = "[Software\\\\Microsoft\\\\Windows NT\\\\CurrentVersion\\\\FontSubstitutes]";
    let mut new_content = content.clone();

    if let Some(pos) = new_content.find(substitutes_section) {
        // 找到该节的末尾（下一个 [ 开头的行或文件末尾）
        let after = &new_content[pos + substitutes_section.len()..];
        let insert_pos = if let Some(next_section) = after.find("\n[") {
            pos + substitutes_section.len() + next_section
        } else {
            new_content.len()
        };

        let entries = format!(
            "\n\"MS Shell Dlg\"=\"{font}\"\n\"MS Shell Dlg 2\"=\"{font}\"\n\"MS UI Gothic\"=\"{font}\"\n\"SimSun\"=\"{font}\"\n\"NSimSun\"=\"{font}\"\n\"Microsoft YaHei\"=\"{font}\"\n\"\u{5b8b}\u{4f53}\"=\"{font}\"\n",
            font = fallback_font
        );
        new_content.insert_str(insert_pos, &entries);
    } else {
        // 没有 FontSubstitutes 节，在文件末尾添加完整节
        new_content.push_str(&patch_marker);
        new_content.push_str(&format!(
            "\n[Software\\\\Microsoft\\\\Windows NT\\\\CurrentVersion\\\\FontSubstitutes]\n\"MS Shell Dlg\"=\"{font}\"\n\"MS Shell Dlg 2\"=\"{font}\"\n\"MS UI Gothic\"=\"{font}\"\n\"SimSun\"=\"{font}\"\n\"NSimSun\"=\"{font}\"\n\"Microsoft YaHei\"=\"{font}\"\n\"\u{5b8b}\u{4f53}\"=\"{font}\"\n",
            font = fallback_font
        ));
    }

    // 添加标记防止重复
    if !new_content.contains("SSMT4 CJK FontSubstitutes") {
        new_content.push_str(&patch_marker);
    }

    if let Err(e) = std::fs::write(&system_reg, &new_content) {
        warn!("写入 Wine 注册表失败: {}", e);
    } else {
        info!("已写入 CJK 字体替换到 {}", system_reg.display());
    }
}

/// 检测 prefix Fonts 目录中可用的 CJK 字体名（返回 Wine 可识别的字体族名）
fn detect_available_cjk_font(fonts_dir: &Path) -> String {
    if !fonts_dir.exists() {
        return String::new();
    }

    // 按优先级检测（文件名 → Wine 字体族名）
    let candidates: &[(&[&str], &str)] = &[
        (&["notosanscjksc", "notosanscjk-regular", "notosanscjk"], "Noto Sans CJK SC"),
        (&["notoserifcjksc"], "Noto Serif CJK SC"),
        (&["sarasa"], "Sarasa Gothic SC"),
        (&["wqy-microhei", "wenquanyimicrohei"], "WenQuanYi Micro Hei"),
        (&["wqy-zenhei", "wenquanyizenhei"], "WenQuanYi Zen Hei"),
        (&["droidsansfallback"], "Droid Sans Fallback"),
        (&["sourcehanssanscn", "sourcehanssans", "source-han-sans"], "Source Han Sans CN"),
    ];

    let entries: Vec<String> = match std::fs::read_dir(fonts_dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_lowercase().replace(['-', '_', ' '], ""))
            .collect(),
        Err(_) => return String::new(),
    };

    for (patterns, font_name) in candidates {
        for pattern in *patterns {
            let pat = pattern.replace(['-', '_', ' '], "");
            if entries.iter().any(|name| name.contains(&pat)) {
                return font_name.to_string();
            }
        }
    }

    String::new()
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrefixInfo {
    pub game_id: String,
    pub exists: bool,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub config: Option<PrefixConfig>,
}
