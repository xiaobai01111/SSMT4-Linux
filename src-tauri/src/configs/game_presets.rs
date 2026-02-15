use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

/// 单个游戏预设：聚合 launcher API、默认目录、3DMigoto 仓库、遥测等全部元数据。
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
pub struct GamePreset {
    /// Canonical 预设 ID（如 "HonkaiStarRail", "WutheringWaves"）
    pub id: String,
    /// Legacy 别名（如 "SRMI", "WWMI", "WuWa"）
    #[serde(default)]
    pub legacy_ids: Vec<String>,
    /// 英文显示名
    #[serde(default)]
    pub display_name_en: String,
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
    /// 下载服务器配置（可多服）；为空时回退 launcher_api 单通道
    #[serde(default)]
    pub download_servers: Vec<PresetDownloadServer>,
    /// 可选语音包（主要用于 HoYoverse）
    #[serde(default)]
    pub audio_languages: Vec<PresetAudioLanguage>,
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
    fn alias_wuwa_resolves_to_canonical() {
        let map = load_presets_from_db();
        assert!(map.contains_key("WutheringWaves"));
        assert_eq!(
            crate::configs::game_identity::normalize_game_key_or_alias("WuWa"),
            Some("WutheringWaves".to_string())
        );
    }

    #[test]
    fn preset_json_roundtrip() {
        let map = load_presets_from_db();
        let first = map.values().next().expect("preset table should not be empty");
        let json = serde_json::to_string(first).unwrap();
        let parsed: GamePreset = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, first.id);
    }

    #[test]
    fn normalize_legacy_default_folder() {
        assert_eq!(
            normalize_default_folder("WutheringWaves", "WWMI"),
            "WutheringWaves".to_string()
        );
        assert_eq!(
            normalize_default_folder("HonkaiStarRail", "SRMI/StarRail"),
            "HonkaiStarRail/StarRail".to_string()
        );
    }
}
