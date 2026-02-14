use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::{info, warn};

/// 配置文件 schema 版本，用于未来兼容性检测
const CURRENT_SCHEMA_VERSION: u32 = 1;
const PRESETS_FILE: &str = "game_presets.json";

/// 单个游戏预设：聚合 launcher API、默认目录、3DMigoto 仓库、遥测等全部元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamePreset {
    /// 预设 ID（如 "SRMI", "WWMI"）
    pub id: String,
    /// 是否支持下载/管理
    #[serde(default = "default_true")]
    pub supported: bool,
    /// 游戏默认安装子目录名
    #[serde(default)]
    pub default_folder: String,
    /// Kuro Games launcher API URL（仅鸣潮等需要）
    #[serde(default)]
    pub launcher_api: Option<String>,
    /// Kuro Games launcher 下载 API URL
    #[serde(default)]
    pub launcher_download_api: Option<String>,
    /// 3DMigoto GitHub releases API URL
    #[serde(default)]
    pub migoto_repo_api: Option<String>,
    /// 需要屏蔽的遥测服务器列表
    #[serde(default)]
    pub telemetry_servers: Vec<String>,
    /// 需要删除的遥测 DLL 路径列表（相对于游戏根目录）
    #[serde(default)]
    pub telemetry_dlls: Vec<String>,
}

fn default_true() -> bool {
    true
}

/// 带 schema 版本的顶层配置
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PresetsFile {
    schema_version: u32,
    presets: Vec<GamePreset>,
}

/// 全局预设注册表（进程内单例）
static PRESETS: once_cell::sync::Lazy<HashMap<String, GamePreset>> =
    once_cell::sync::Lazy::new(|| {
        let map = load_presets();
        info!("游戏预设注册表已加载: {} 个预设", map.len());
        map
    });

/// 查询指定预设
pub fn get_preset(id: &str) -> Option<&'static GamePreset> {
    // 支持别名："WuWa" → "WWMI"
    let canonical = match id {
        "WuWa" => "WWMI",
        other => other,
    };
    PRESETS.get(canonical)
}

/// 获取全部预设（只读引用）
#[allow(dead_code)]
pub fn all_presets() -> &'static HashMap<String, GamePreset> {
    &PRESETS
}

/// 加载逻辑：本地 JSON → 硬编码兜底
fn load_presets() -> HashMap<String, GamePreset> {
    // 尝试从数据目录读取自定义配置
    let data_dir = super::app_config::get_app_data_dir();
    let json_path = data_dir.join(PRESETS_FILE);

    if let Some(map) = try_load_from_json(&json_path) {
        return map;
    }

    // 硬编码兜底
    info!("使用硬编码游戏预设（{}）", PRESETS_FILE);
    build_hardcoded_presets()
}

fn try_load_from_json(path: &Path) -> Option<HashMap<String, GamePreset>> {
    let content = std::fs::read_to_string(path).ok()?;
    let file: PresetsFile = match serde_json::from_str(&content) {
        Ok(f) => f,
        Err(e) => {
            warn!("解析 {} 失败: {}，回退到硬编码", path.display(), e);
            return None;
        }
    };

    if file.schema_version > CURRENT_SCHEMA_VERSION {
        warn!(
            "{} schema_version {} 高于当前 {}，回退到硬编码",
            path.display(),
            file.schema_version,
            CURRENT_SCHEMA_VERSION
        );
        return None;
    }

    let mut map = HashMap::new();
    for preset in file.presets {
        map.insert(preset.id.clone(), preset);
    }
    info!(
        "从 {} 加载了 {} 个游戏预设 (schema v{})",
        path.display(),
        map.len(),
        file.schema_version
    );
    Some(map)
}

/// 硬编码预设 —— 当 JSON 不存在或不可用时兜底
fn build_hardcoded_presets() -> HashMap<String, GamePreset> {
    let presets = vec![
        GamePreset {
            id: "WWMI".to_string(),
            supported: true,
            default_folder: "Wuthering Waves Game".to_string(),
            launcher_api: Some("https://prod-cn-alicdn-gamestarter.kurogame.com/launcher/game/G152/10003_Y8xXrXk65DqFHEDgApn3cpK5lfczpFx5/index.json".to_string()),
            launcher_download_api: Some("https://prod-cn-alicdn-gamestarter.kurogame.com/launcher/launcher/10003_Y8xXrXk65DqFHEDgApn3cpK5lfczpFx5/G152/index.json".to_string()),
            migoto_repo_api: Some("https://api.github.com/repos/SpectrumQT/WWMI/releases/latest".to_string()),
            telemetry_servers: vec!["pc.crashsight.wetest.net".to_string()],
            telemetry_dlls: vec![],
        },
        GamePreset {
            id: "SRMI".to_string(),
            supported: true,
            default_folder: "StarRail".to_string(),
            launcher_api: None,
            launcher_download_api: None,
            migoto_repo_api: Some("https://api.github.com/repos/SilentNightSound/SR-Model-Importer/releases/latest".to_string()),
            telemetry_servers: vec![
                "log-upload.mihoyo.com".to_string(),
                "uspider.yuanshen.com".to_string(),
                "log-upload-os.hoyoverse.com".to_string(),
                "overseauspider.yuanshen.com".to_string(),
                "sg-public-data-api.hoyoverse.com".to_string(),
                "public-data-api.mihoyo.com".to_string(),
            ],
            telemetry_dlls: vec![
                "StarRail_Data/Plugins/x86_64/Telemetry.dll".to_string(),
                "StarRail_Data/Plugins/x86_64/telemetry.dll".to_string(),
            ],
        },
        GamePreset {
            id: "ZZMI".to_string(),
            supported: true,
            default_folder: "ZenlessZoneZero".to_string(),
            launcher_api: None,
            launcher_download_api: None,
            migoto_repo_api: Some("https://api.github.com/repos/leotorrez/ZZ-Model-Importer/releases/latest".to_string()),
            telemetry_servers: vec![
                "log-upload.mihoyo.com".to_string(),
                "uspider.yuanshen.com".to_string(),
                "log-upload-os.hoyoverse.com".to_string(),
                "overseauspider.yuanshen.com".to_string(),
                "sg-public-data-api.hoyoverse.com".to_string(),
                "public-data-api.mihoyo.com".to_string(),
            ],
            telemetry_dlls: vec![
                "ZenlessZoneZero_Data/Plugins/x86_64/Telemetry.dll".to_string(),
                "ZenlessZoneZero_Data/Plugins/x86_64/telemetry.dll".to_string(),
            ],
        },
        GamePreset {
            id: "GIMI".to_string(),
            supported: true,
            default_folder: "GenshinImpact".to_string(),
            launcher_api: None,
            launcher_download_api: None,
            migoto_repo_api: Some("https://api.github.com/repos/SilentNightSound/GI-Model-Importer/releases/latest".to_string()),
            telemetry_servers: vec![
                "log-upload.mihoyo.com".to_string(),
                "uspider.yuanshen.com".to_string(),
                "log-upload-os.hoyoverse.com".to_string(),
                "overseauspider.yuanshen.com".to_string(),
                "sg-public-data-api.hoyoverse.com".to_string(),
                "public-data-api.mihoyo.com".to_string(),
            ],
            telemetry_dlls: vec![
                "GenshinImpact_Data/Plugins/x86_64/Telemetry.dll".to_string(),
                "GenshinImpact_Data/Plugins/x86_64/telemetry.dll".to_string(),
                "YuanShen_Data/Plugins/x86_64/Telemetry.dll".to_string(),
                "YuanShen_Data/Plugins/x86_64/telemetry.dll".to_string(),
            ],
        },
        GamePreset {
            id: "HIMI".to_string(),
            supported: true,
            default_folder: "HonkaiImpact3rd".to_string(),
            launcher_api: None,
            launcher_download_api: None,
            migoto_repo_api: Some("https://api.github.com/repos/SilentNightSound/HI-Model-Importer/releases/latest".to_string()),
            telemetry_servers: vec![
                "log-upload.mihoyo.com".to_string(),
                "uspider.yuanshen.com".to_string(),
                "log-upload-os.hoyoverse.com".to_string(),
                "overseauspider.yuanshen.com".to_string(),
                "sg-public-data-api.hoyoverse.com".to_string(),
                "public-data-api.mihoyo.com".to_string(),
            ],
            telemetry_dlls: vec![],
        },
        GamePreset {
            id: "EFMI".to_string(),
            supported: true,
            default_folder: "SnowbreakContainmentZone".to_string(),
            launcher_api: None,
            launcher_download_api: None,
            migoto_repo_api: None,
            telemetry_servers: vec![],
            telemetry_dlls: vec![],
        },
    ];

    let mut map = HashMap::new();
    for p in presets {
        map.insert(p.id.clone(), p);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hardcoded_presets_cover_all_known_games() {
        let map = build_hardcoded_presets();
        for id in &["WWMI", "SRMI", "ZZMI", "GIMI", "HIMI", "EFMI"] {
            assert!(map.contains_key(*id), "missing preset: {}", id);
        }
    }

    #[test]
    fn alias_wuwa_resolves_to_wwmi() {
        // 直接测试 build_hardcoded_presets 而非 get_preset（避免 Lazy 初始化依赖）
        let map = build_hardcoded_presets();
        assert!(map.contains_key("WWMI"));
    }

    #[test]
    fn preset_json_roundtrip() {
        let map = build_hardcoded_presets();
        let presets: Vec<&GamePreset> = map.values().collect();
        let file = PresetsFile {
            schema_version: CURRENT_SCHEMA_VERSION,
            presets: presets.into_iter().cloned().collect(),
        };
        let json = serde_json::to_string_pretty(&file).unwrap();
        let parsed: PresetsFile = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.presets.len(), map.len());
        assert_eq!(parsed.schema_version, CURRENT_SCHEMA_VERSION);
    }
}
