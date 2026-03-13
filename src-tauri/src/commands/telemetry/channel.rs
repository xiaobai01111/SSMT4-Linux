use std::path::{Path, PathBuf};

use crate::configs::database as db;
use crate::configs::game_presets;
use tracing::info;

use super::catalog::channel_protection_config;
use super::types::ChannelProtectionPayload;

pub(super) const CHANNEL_MODE_INIT: &str = "init";
pub(super) const CHANNEL_MODE_PROTECTED: &str = "protected";
pub(super) const CHANNEL_ENFORCEMENT_BLOCK: &str = "block";
pub(super) const CHANNEL_ENFORCEMENT_WARN: &str = "warn";

#[derive(Debug, Clone)]
pub(super) struct ChannelProtectionState {
    pub required: bool,
    pub enabled: bool,
    pub channel_key: Option<String>,
    pub current_value: Option<i64>,
    pub init_value: Option<i64>,
    pub expected_value: Option<i64>,
    pub protected_value: Option<i64>,
    pub mode: Option<String>,
    pub launch_enforcement: String,
    pub config_path: Option<String>,
    pub error: Option<String>,
    pub backup_exists: bool,
}

#[derive(Debug, Clone)]
pub(super) struct ChannelProtectionRollback {
    pub config_path: PathBuf,
    pub channel_key: String,
    pub original_value: i64,
    pub restore_mode: String,
}

pub(super) fn channel_state_payload(state: &ChannelProtectionState) -> ChannelProtectionPayload {
    ChannelProtectionPayload {
        required: state.required,
        enabled: state.enabled,
        mode: state.mode.clone(),
        launch_enforcement: state.launch_enforcement.clone(),
        channel_key: state.channel_key.clone(),
        current_value: state.current_value,
        init_value: state.init_value,
        expected_value: state.expected_value,
        protected_value: state.protected_value,
        config_path: state.config_path.clone(),
        error: state.error.clone(),
        backup_exists: state.backup_exists,
    }
}

fn channel_mode_setting_key(game_preset: &str) -> String {
    format!("protection.channel.mode.{}", game_preset)
}

pub(super) fn normalize_channel_mode(mode: &str, has_init_value: bool) -> Option<String> {
    let normalized = mode.trim().to_ascii_lowercase();
    match normalized.as_str() {
        CHANNEL_MODE_INIT if has_init_value => Some(CHANNEL_MODE_INIT.to_string()),
        CHANNEL_MODE_PROTECTED => Some(CHANNEL_MODE_PROTECTED.to_string()),
        _ => None,
    }
}

pub(super) fn normalize_launch_enforcement(raw: Option<&str>) -> String {
    match raw.map(|value| value.trim().to_ascii_lowercase()) {
        Some(value) if value == CHANNEL_ENFORCEMENT_WARN => CHANNEL_ENFORCEMENT_WARN.to_string(),
        _ => CHANNEL_ENFORCEMENT_BLOCK.to_string(),
    }
}

pub(super) fn default_channel_mode(config: &game_presets::ChannelProtectionConfig) -> String {
    let has_init = config.init_value.is_some();
    if let Some(default_mode) = config
        .default_mode
        .as_deref()
        .and_then(|value| normalize_channel_mode(value, has_init))
    {
        return default_mode;
    }
    if has_init {
        CHANNEL_MODE_INIT.to_string()
    } else {
        CHANNEL_MODE_PROTECTED.to_string()
    }
}

fn resolve_channel_mode(
    game_preset: &str,
    config: &game_presets::ChannelProtectionConfig,
) -> String {
    let has_init = config.init_value.is_some();
    if let Some(saved_mode) = db::read_setting_value(&channel_mode_setting_key(game_preset))
        .and_then(|value| normalize_channel_mode(&value, has_init))
    {
        return saved_mode;
    }
    default_channel_mode(config)
}

pub(super) fn expected_channel_value(
    config: &game_presets::ChannelProtectionConfig,
    mode: &str,
) -> i64 {
    if mode == CHANNEL_MODE_INIT {
        config.init_value.unwrap_or(config.protected_value)
    } else {
        config.protected_value
    }
}

fn parse_i64_json(value: &serde_json::Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|number| i64::try_from(number).ok()))
        .or_else(|| {
            value
                .as_str()
                .and_then(|text| text.trim().parse::<i64>().ok())
        })
}

fn channel_backup_setting_key(game_preset: &str, config_path: &Path) -> String {
    let suffix: String = config_path
        .to_string_lossy()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("protection.channel.original.{}.{}", game_preset, suffix)
}

fn read_channel_value(config_path: &Path, channel_key: &str) -> Result<i64, String> {
    let content = std::fs::read_to_string(config_path)
        .map_err(|error| format!("读取渠道配置失败: {} ({})", config_path.display(), error))?;
    let data = serde_json::from_str::<serde_json::Value>(&content)
        .map_err(|error| format!("解析渠道配置失败: {} ({})", config_path.display(), error))?;
    let Some(raw_value) = data.get(channel_key) else {
        return Err(format!(
            "渠道配置缺少字段 {} ({})",
            channel_key,
            config_path.display()
        ));
    };
    parse_i64_json(raw_value).ok_or_else(|| {
        format!(
            "字段 {} 不是可解析的数字 ({})",
            channel_key,
            config_path.display()
        )
    })
}

fn write_channel_value(config_path: &Path, channel_key: &str, target: i64) -> Result<(), String> {
    let content = std::fs::read_to_string(config_path)
        .map_err(|error| format!("读取渠道配置失败: {} ({})", config_path.display(), error))?;
    let mut data = serde_json::from_str::<serde_json::Value>(&content)
        .map_err(|error| format!("解析渠道配置失败: {} ({})", config_path.display(), error))?;
    let Some(obj) = data.as_object_mut() else {
        return Err(format!("渠道配置结构非法: {}", config_path.display()));
    };
    obj.insert(
        channel_key.to_string(),
        serde_json::Value::String(target.to_string()),
    );
    let output = serde_json::to_string_pretty(&data)
        .map_err(|error| format!("序列化渠道配置失败: {} ({})", config_path.display(), error))?;
    std::fs::write(config_path, output)
        .map_err(|error| format!("写入渠道配置失败: {} ({})", config_path.display(), error))?;
    Ok(())
}

pub(super) fn evaluate_channel_protection(
    game_preset: &str,
    game_root: Option<&Path>,
) -> ChannelProtectionState {
    let Some(config) = channel_protection_config(game_preset) else {
        return ChannelProtectionState {
            required: false,
            enabled: true,
            channel_key: None,
            current_value: None,
            init_value: None,
            expected_value: None,
            protected_value: None,
            mode: None,
            launch_enforcement: CHANNEL_ENFORCEMENT_BLOCK.to_string(),
            config_path: None,
            error: None,
            backup_exists: false,
        };
    };

    let channel_key = config.channel_key.trim().to_string();
    let mode = resolve_channel_mode(game_preset, &config);
    let expected_value = expected_channel_value(&config, &mode);
    let launch_enforcement = normalize_launch_enforcement(config.launch_enforcement.as_deref());
    let protected_value = config.protected_value;
    if channel_key.is_empty() {
        return ChannelProtectionState {
            required: true,
            enabled: false,
            channel_key: None,
            current_value: None,
            init_value: config.init_value,
            expected_value: Some(expected_value),
            protected_value: Some(protected_value),
            mode: Some(mode),
            launch_enforcement,
            config_path: None,
            error: Some("渠道防护配置缺少 channel_key".to_string()),
            backup_exists: false,
        };
    }

    let Some(root) = game_root else {
        return ChannelProtectionState {
            required: true,
            enabled: false,
            channel_key: Some(channel_key),
            current_value: None,
            init_value: config.init_value,
            expected_value: Some(expected_value),
            protected_value: Some(protected_value),
            mode: Some(mode),
            launch_enforcement,
            config_path: None,
            error: Some("缺少游戏目录，无法校验渠道防护配置".to_string()),
            backup_exists: false,
        };
    };

    let rel_path = config
        .config_relative_path
        .trim()
        .trim_matches(['/', '\\'])
        .to_string();
    if rel_path.is_empty() {
        return ChannelProtectionState {
            required: true,
            enabled: false,
            channel_key: Some(channel_key),
            current_value: None,
            init_value: config.init_value,
            expected_value: Some(expected_value),
            protected_value: Some(protected_value),
            mode: Some(mode),
            launch_enforcement,
            config_path: None,
            error: Some("渠道防护配置缺少 config_relative_path".to_string()),
            backup_exists: false,
        };
    }

    let config_path = root.join(rel_path);
    if !config_path.exists() {
        return ChannelProtectionState {
            required: true,
            enabled: false,
            channel_key: Some(channel_key),
            current_value: None,
            init_value: config.init_value,
            expected_value: Some(expected_value),
            protected_value: Some(protected_value),
            mode: Some(mode),
            launch_enforcement,
            config_path: Some(config_path.to_string_lossy().to_string()),
            error: Some("未找到渠道配置文件".to_string()),
            backup_exists: false,
        };
    }

    let backup_key = channel_backup_setting_key(game_preset, &config_path);
    let backup_exists = db::read_setting_value(&backup_key).is_some();

    match read_channel_value(&config_path, &channel_key) {
        Ok(current_value) => ChannelProtectionState {
            required: true,
            enabled: current_value == expected_value,
            channel_key: Some(channel_key),
            current_value: Some(current_value),
            init_value: config.init_value,
            expected_value: Some(expected_value),
            protected_value: Some(protected_value),
            mode: Some(mode),
            launch_enforcement,
            config_path: Some(config_path.to_string_lossy().to_string()),
            error: None,
            backup_exists,
        },
        Err(error) => ChannelProtectionState {
            required: true,
            enabled: false,
            channel_key: Some(channel_key),
            current_value: None,
            init_value: config.init_value,
            expected_value: Some(expected_value),
            protected_value: Some(protected_value),
            mode: Some(mode),
            launch_enforcement,
            config_path: Some(config_path.to_string_lossy().to_string()),
            error: Some(error),
            backup_exists,
        },
    }
}

pub(super) fn set_channel_mode_internal(
    game_preset: &str,
    game_root: &Path,
    requested_mode: &str,
) -> Result<ChannelProtectionState, String> {
    let Some(channel) = channel_protection_config(game_preset) else {
        return Err("该游戏未配置渠道防护".to_string());
    };

    let mode = normalize_channel_mode(requested_mode, channel.init_value.is_some())
        .ok_or_else(|| "渠道模式非法，仅支持 init/protected".to_string())?;

    let rel = channel
        .config_relative_path
        .trim()
        .trim_matches(['/', '\\'])
        .to_string();
    if rel.is_empty() {
        return Err("渠道防护配置缺少 config_relative_path".to_string());
    }
    let config_path = game_root.join(rel);
    if !config_path.exists() {
        return Err(format!("未找到渠道配置文件: {}", config_path.display()));
    }
    let channel_key = channel.channel_key.trim();
    if channel_key.is_empty() {
        return Err("渠道防护配置缺少 channel_key".to_string());
    }

    let current = read_channel_value(&config_path, channel_key)?;
    let target = expected_channel_value(&channel, &mode);

    let backup_key = channel_backup_setting_key(game_preset, &config_path);
    if current != target {
        if db::read_setting_value(&backup_key).is_none() {
            db::write_setting_value(&backup_key, &current.to_string());
        }
        write_channel_value(&config_path, channel_key, target)?;
        info!(
            "[防护] {} {}: {} -> {} ({}, mode={})",
            game_preset,
            channel_key,
            current,
            target,
            config_path.display(),
            mode
        );
    } else {
        info!(
            "[防护] {} {} 已是目标值 {} ({}, mode={})",
            game_preset,
            channel_key,
            target,
            config_path.display(),
            mode
        );
    }

    db::write_setting_value(&channel_mode_setting_key(game_preset), &mode);

    Ok(evaluate_channel_protection(game_preset, Some(game_root)))
}

pub(super) fn prepare_channel_protection_rollback(
    game_preset: &str,
    game_root: &Path,
    requested_mode: &str,
) -> Result<Option<ChannelProtectionRollback>, String> {
    let Some(channel) = channel_protection_config(game_preset) else {
        return Ok(None);
    };

    normalize_channel_mode(requested_mode, channel.init_value.is_some())
        .ok_or_else(|| "渠道模式非法，仅支持 init/protected".to_string())?;
    let rel = channel
        .config_relative_path
        .trim()
        .trim_matches(['/', '\\'])
        .to_string();
    if rel.is_empty() {
        return Err("渠道防护配置缺少 config_relative_path".to_string());
    }

    let config_path = game_root.join(rel);
    if !config_path.exists() {
        return Err(format!("未找到渠道配置文件: {}", config_path.display()));
    }

    let channel_key = channel.channel_key.trim();
    if channel_key.is_empty() {
        return Err("渠道防护配置缺少 channel_key".to_string());
    }

    let original_value = read_channel_value(&config_path, channel_key)?;
    let restore_mode = db::read_setting_value(&channel_mode_setting_key(game_preset))
        .and_then(|value| normalize_channel_mode(&value, channel.init_value.is_some()))
        .unwrap_or_else(|| default_channel_mode(&channel));
    Ok(Some(ChannelProtectionRollback {
        config_path,
        channel_key: channel_key.to_string(),
        original_value,
        restore_mode,
    }))
}

impl ChannelProtectionRollback {
    pub(super) fn rollback(&self, game_preset: &str) -> Result<(), String> {
        write_channel_value(&self.config_path, &self.channel_key, self.original_value)?;
        db::write_setting_value(
            &channel_mode_setting_key(game_preset),
            self.restore_mode.as_str(),
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_channel_config() -> game_presets::ChannelProtectionConfig {
        game_presets::ChannelProtectionConfig {
            config_relative_path: "Client/KRSDKConfig.json".to_string(),
            channel_key: "KR_ChannelId".to_string(),
            init_value: Some(19),
            protected_value: 205,
            default_mode: Some("init".to_string()),
            launch_enforcement: Some("warn".to_string()),
        }
    }

    #[test]
    fn channel_mode_normalization_respects_init_availability() {
        assert_eq!(
            normalize_channel_mode("init", true).as_deref(),
            Some(CHANNEL_MODE_INIT)
        );
        assert_eq!(normalize_channel_mode("init", false), None);
        assert_eq!(
            normalize_channel_mode("protected", true).as_deref(),
            Some(CHANNEL_MODE_PROTECTED)
        );
    }

    #[test]
    fn default_channel_mode_prefers_configured_init() {
        let config = sample_channel_config();
        assert_eq!(default_channel_mode(&config), CHANNEL_MODE_INIT);
    }

    #[test]
    fn expected_channel_value_switches_between_init_and_protected() {
        let config = sample_channel_config();
        assert_eq!(expected_channel_value(&config, CHANNEL_MODE_INIT), 19);
        assert_eq!(expected_channel_value(&config, CHANNEL_MODE_PROTECTED), 205);
    }

    #[test]
    fn launch_enforcement_defaults_to_block() {
        assert_eq!(
            normalize_launch_enforcement(Some("warn")),
            CHANNEL_ENFORCEMENT_WARN
        );
        assert_eq!(
            normalize_launch_enforcement(Some("unknown")),
            CHANNEL_ENFORCEMENT_BLOCK
        );
        assert_eq!(
            normalize_launch_enforcement(None),
            CHANNEL_ENFORCEMENT_BLOCK
        );
    }
}
