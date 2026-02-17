use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

/// 单个游戏预设：聚合 launcher API、默认目录、遥测等全部元数据。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetDownloadServer {
    pub id: String,
    pub label: String,
    pub launcher_api: String,
    #[serde(default)]
    pub biz_prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetAudioLanguage {
    pub code: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelProtectionConfig {
    /// 相对于游戏根目录的 KRSDKConfig 路径
    pub config_relative_path: String,
    /// 需要切换的 JSON 字段名（例如 KR_ChannelId）
    pub channel_key: String,
    /// 启用防护时写入的目标值
    pub protected_value: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePreset {
    /// Canonical 预设 ID（如 "HonkaiStarRail", "WutheringWaves"）
    pub id: String,
    /// Legacy 别名（历史兼容）
    #[serde(default)]
    pub legacy_ids: Vec<String>,
    /// 英文显示名
    #[serde(default)]
    pub display_name_en: String,
    /// 是否支持下载/管理
    #[serde(default = "default_true")]
    pub supported: bool,
    /// 启动前是否强制要求“防护已启用”
    #[serde(default = "default_true")]
    pub require_protection_before_launch: bool,
    /// 游戏默认安装子目录名
    #[serde(default)]
    pub default_folder: String,
    /// Kuro Games launcher API URL（仅鸣潮等需要）
    #[serde(default)]
    pub launcher_api: Option<String>,
    /// Kuro Games launcher 下载 API URL
    #[serde(default)]
    pub launcher_download_api: Option<String>,
    /// 下载服务器配置（可多服）；为空时回退 launcher_api 单通道
    #[serde(default)]
    pub download_servers: Vec<PresetDownloadServer>,
    /// 可选语音包（主要用于 HoYoverse）
    #[serde(default)]
    pub audio_languages: Vec<PresetAudioLanguage>,
    /// 需要屏蔽的遥测服务器列表
    #[serde(default)]
    pub telemetry_servers: Vec<String>,
    /// 需要删除的遥测 DLL 路径列表（相对于游戏根目录）
    #[serde(default)]
    pub telemetry_dlls: Vec<String>,
    /// 基于渠道配置文件的防护（例如 KR_ChannelId 切换）
    #[serde(default)]
    pub channel_protection: Option<ChannelProtectionConfig>,
    /// 游戏启动时注入的默认环境变量（可被用户自定义 env 覆盖）
    #[serde(default)]
    pub env_defaults: HashMap<String, String>,
    /// 是否默认启用 umu-run
    #[serde(default)]
    pub default_umu_run: bool,
    /// umu-run 的 GAMEID（例如 "umu-3513350"）
    #[serde(default)]
    pub umu_game_id: Option<String>,
    /// umu-run 的 STORE（例如 "none", "egs"）
    #[serde(default)]
    pub umu_store: Option<String>,
    /// 强制走直连 Proton 启动链（忽略 umu-run）
    #[serde(default)]
    pub force_direct_proton: bool,
    /// 强制禁用 pressure-vessel（更接近直连 Proton 启动行为）
    #[serde(default)]
    pub force_disable_pressure_vessel: bool,
    /// 默认开启网络诊断日志（Proton/Wine 网络相关）
    #[serde(default)]
    pub enable_network_log_by_default: bool,
}

fn default_true() -> bool {
    true
}

/// 全局预设注册表（进程内单例）
static PRESETS: once_cell::sync::Lazy<HashMap<String, GamePreset>> =
    once_cell::sync::Lazy::new(|| {
        let map = load_presets_from_db();
        info!("游戏预设注册表已加载(数据库): {} 个预设", map.len());
        map
    });

/// 查询指定预设（支持 canonical key 或 legacy alias）
pub fn get_preset(id: &str) -> Option<&'static GamePreset> {
    let canonical = crate::configs::game_identity::to_canonical_or_keep(id);
    if let Some(preset) = PRESETS.get(&canonical) {
        return Some(preset);
    }
    let trimmed = id.trim();
    PRESETS.values().find(|preset| {
        preset.id.eq_ignore_ascii_case(trimmed)
            || preset
                .legacy_ids
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(trimmed))
    })
}

/// 获取全部预设（只读引用）
pub fn all_presets() -> &'static HashMap<String, GamePreset> {
    &PRESETS
}

fn normalize_preset(mut preset: GamePreset) -> GamePreset {
    let canonical = crate::configs::game_identity::to_canonical_or_keep(&preset.id);

    if preset.display_name_en.trim().is_empty() {
        preset.display_name_en = crate::configs::game_identity::display_name_en_for_key(&canonical)
            .unwrap_or_else(|| canonical.clone());
    }

    if preset.legacy_ids.is_empty() {
        preset.legacy_ids = crate::configs::game_identity::legacy_aliases_for_canonical(&canonical);
    }

    if preset.download_servers.is_empty() {
        if let Some(api) = preset.launcher_api.clone() {
            preset.download_servers.push(PresetDownloadServer {
                id: "default".to_string(),
                label: "默认".to_string(),
                launcher_api: api,
                biz_prefix: String::new(),
            });
        }
    }

    preset.default_folder = normalize_default_folder(&canonical, &preset.default_folder);
    if let Some(id) = preset.umu_game_id.as_mut() {
        let trimmed = id.trim();
        if trimmed.is_empty() {
            preset.umu_game_id = None;
        } else if trimmed != id {
            *id = trimmed.to_string();
        }
    }
    if let Some(store) = preset.umu_store.as_mut() {
        let trimmed = store.trim();
        if trimmed.is_empty() {
            preset.umu_store = None;
        } else if trimmed != store {
            *store = trimmed.to_string();
        }
    }
    preset.id = canonical;
    preset
}

fn normalize_default_folder(canonical: &str, raw: &str) -> String {
    let trimmed = raw.trim().trim_matches(['/', '\\']);
    if trimmed.is_empty() {
        return canonical.to_string();
    }

    if let Some(mapped) = crate::configs::game_identity::normalize_game_key_or_alias(trimmed) {
        return mapped;
    }

    let mut changed = false;
    let segments: Vec<String> = trimmed
        .split(['/', '\\'])
        .filter(|seg| !seg.trim().is_empty())
        .map(|seg| {
            if let Some(mapped) = crate::configs::game_identity::normalize_game_key_or_alias(seg) {
                if !mapped.eq_ignore_ascii_case(seg) {
                    changed = true;
                }
                mapped
            } else {
                seg.trim().to_string()
            }
        })
        .collect();

    if segments.is_empty() {
        return canonical.to_string();
    }

    if changed {
        return segments.join("/");
    }

    trimmed.to_string()
}

fn load_presets_from_db() -> HashMap<String, GamePreset> {
    let mut map = HashMap::new();

    for (_id, json) in crate::configs::database::list_game_preset_rows() {
        match serde_json::from_str::<GamePreset>(&json) {
            Ok(preset) => {
                let normalized = normalize_preset(preset);
                map.insert(normalized.id.clone(), normalized);
            }
            Err(e) => {
                warn!("解析数据库预设失败: {}", e);
            }
        }
    }

    if map.is_empty() {
        warn!("数据库中未找到任何游戏预设，请检查 game_presets 表数据");
    }

    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_presets_cover_all_known_games() {
        let map = load_presets_from_db();
        for id in &[
            "WutheringWaves",
            "HonkaiStarRail",
            "ZenlessZoneZero",
            "GenshinImpact",
            "HonkaiImpact3rd",
            "SnowbreakContainmentZone",
        ] {
            assert!(map.contains_key(*id), "missing preset: {}", id);
        }
    }

    #[test]
    fn legacy_alias_list_is_optional() {
        let map = load_presets_from_db();
        assert!(map.contains_key("WutheringWaves"));
        let preset = map
            .get("WutheringWaves")
            .expect("WutheringWaves preset should exist");
        assert!(
            !preset
                .legacy_ids
                .iter()
                .any(|alias| alias.ends_with("MI"))
        );
    }

    #[test]
    fn preset_json_roundtrip() {
        let map = load_presets_from_db();
        let first = map
            .values()
            .next()
            .expect("preset table should not be empty");
        let json = serde_json::to_string(first).unwrap();
        let parsed: GamePreset = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, first.id);
    }

    #[test]
    fn normalize_default_folder_keeps_canonical_path() {
        assert_eq!(
            normalize_default_folder("WutheringWaves", "WutheringWaves"),
            "WutheringWaves".to_string()
        );
        assert_eq!(
            normalize_default_folder("HonkaiStarRail", "HonkaiStarRail/StarRail"),
            "HonkaiStarRail/StarRail".to_string()
        );
    }
}
