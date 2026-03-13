use super::catalog::{
    channel_protection_config, protection_category, require_protection_before_launch,
    telemetry_dlls, telemetry_servers,
};
use super::channel::{
    channel_state_payload, default_channel_mode, evaluate_channel_protection,
    normalize_launch_enforcement, CHANNEL_ENFORCEMENT_BLOCK, CHANNEL_MODE_PROTECTED,
};
use super::files::evaluate_file_protection;
use super::game_root::resolve_game_root;
use super::hosts::evaluate_telemetry_protection;
use super::types::{
    FileProtectionPayload, GameProtectionInfoPayload, GameProtectionStatusPayload,
    ProtectionDescriptorPayload, TelemetryProtectionPayload, TelemetryStatusPayload,
};

pub(super) fn check_game_protection_status_internal(
    game_preset: &str,
    game_path: Option<&str>,
) -> Result<GameProtectionStatusPayload, String> {
    let game_root = resolve_game_root(game_preset, game_path);

    let servers = telemetry_servers(game_preset);
    let (telemetry_required, blocked, unblocked) = evaluate_telemetry_protection(&servers);
    let telemetry_all_blocked = !telemetry_required || unblocked.is_empty();

    let (files_required, removed, existing, files_error) =
        evaluate_file_protection(game_preset, game_root.as_deref());
    let files_all_removed = !files_required || (existing.is_empty() && files_error.is_none());

    let channel_state = evaluate_channel_protection(game_preset, game_root.as_deref());

    let supported = telemetry_required || files_required || channel_state.required;
    let channel_is_blocking =
        channel_state.required && channel_state.launch_enforcement == CHANNEL_ENFORCEMENT_BLOCK;
    let enforce_at_launch = supported
        && require_protection_before_launch(game_preset)
        && (telemetry_required || files_required || channel_is_blocking);

    let mut missing = Vec::new();
    if telemetry_required && !telemetry_all_blocked {
        missing.push(format!("未屏蔽遥测域名: {}", unblocked.join(", ")));
    }
    if files_required {
        if let Some(error) = files_error.clone() {
            missing.push(error);
        } else if !existing.is_empty() {
            missing.push(format!("未移除遥测文件: {}", existing.join(", ")));
        }
    }
    if channel_state.required {
        if let Some(error) = channel_state.error.as_ref() {
            missing.push(error.clone());
        } else if !channel_state.enabled {
            let key = channel_state
                .channel_key
                .clone()
                .unwrap_or_else(|| "渠道字段".to_string());
            let current = channel_state
                .current_value
                .map(|value| value.to_string())
                .unwrap_or_else(|| "未知".to_string());
            let target = channel_state
                .expected_value
                .map(|value| value.to_string())
                .unwrap_or_else(|| "未知".to_string());
            let mode = channel_state
                .mode
                .clone()
                .unwrap_or_else(|| CHANNEL_MODE_PROTECTED.to_string());
            let level =
                if channel_state.launch_enforcement == super::channel::CHANNEL_ENFORCEMENT_WARN {
                    "告警"
                } else {
                    "阻断"
                };
            missing.push(format!(
                "{key} 未设置为 {target}（当前 {current}，模式 {mode}，{level}）"
            ));
        }
    }

    let enabled = !supported || missing.is_empty();

    Ok(GameProtectionStatusPayload {
        game_preset: game_preset.to_string(),
        supported,
        enforce_at_launch,
        has_protections: supported,
        enabled,
        all_protected: enabled,
        missing,
        game_root: game_root
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
        telemetry: TelemetryProtectionPayload {
            required: telemetry_required,
            all_blocked: telemetry_all_blocked,
            blocked,
            unblocked,
            total_servers: servers.len(),
        },
        files: FileProtectionPayload {
            required: files_required,
            all_removed: files_all_removed,
            removed,
            existing,
            total_files: telemetry_dlls(game_preset).len(),
            error: files_error,
        },
        channel: channel_state_payload(&channel_state),
    })
}

pub(super) fn check_telemetry_status(
    game_preset: &str,
    game_path: Option<&str>,
) -> TelemetryStatusPayload {
    let servers = telemetry_servers(game_preset);
    let (supported, blocked, unblocked) = evaluate_telemetry_protection(&servers);
    let channel_state = evaluate_channel_protection(
        game_preset,
        resolve_game_root(game_preset, game_path).as_deref(),
    );
    if !supported && !channel_state.required {
        return TelemetryStatusPayload {
            supported: false,
            message: Some("该游戏无需防护".to_string()),
            all_blocked: None,
            blocked: None,
            unblocked: None,
            total_servers: None,
            channel: None,
        };
    }

    TelemetryStatusPayload {
        supported: true,
        message: None,
        all_blocked: Some(!supported || unblocked.is_empty()),
        blocked: Some(blocked),
        unblocked: Some(unblocked),
        total_servers: Some(servers.len()),
        channel: Some(channel_state_payload(&channel_state)),
    }
}

pub(super) fn game_protection_info(game_preset: &str) -> GameProtectionInfoPayload {
    let servers = telemetry_servers(game_preset);
    let dlls = telemetry_dlls(game_preset);
    let channel = channel_protection_config(game_preset);

    let protections = {
        let mut protections = Vec::new();
        if !servers.is_empty() {
            protections.push(ProtectionDescriptorPayload {
                kind: "telemetryBlock".to_string(),
                name: "遥测服务器屏蔽".to_string(),
                description: format!("屏蔽 {} 个遥测/数据上报服务器", servers.len()),
                servers: Some(servers),
                files: None,
                channel_key: None,
                init_value: None,
                target_value: None,
                default_mode: None,
                launch_enforcement: None,
                config_relative_path: None,
            });
        }
        if !dlls.is_empty() {
            protections.push(ProtectionDescriptorPayload {
                kind: "telemetryDll".to_string(),
                name: "删除遥测 DLL".to_string(),
                description: "删除游戏内置的遥测数据收集模块".to_string(),
                servers: None,
                files: Some(dlls),
                channel_key: None,
                init_value: None,
                target_value: None,
                default_mode: None,
                launch_enforcement: None,
                config_relative_path: None,
            });
        }
        if let Some(channel_cfg) = channel {
            let default_mode = default_channel_mode(&channel_cfg);
            let init_value = channel_cfg.init_value;
            protections.push(ProtectionDescriptorPayload {
                kind: "channelRewrite".to_string(),
                name: "渠道参数防护".to_string(),
                description: format!(
                    "渠道模式: init={} / protected={}（默认 {}）",
                    init_value
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    channel_cfg.protected_value,
                    default_mode
                ),
                servers: None,
                files: None,
                channel_key: Some(channel_cfg.channel_key),
                init_value,
                target_value: Some(channel_cfg.protected_value),
                default_mode: Some(default_mode),
                launch_enforcement: Some(normalize_launch_enforcement(
                    channel_cfg.launch_enforcement.as_deref(),
                )),
                config_relative_path: Some(channel_cfg.config_relative_path),
            });
        }
        protections
    };

    GameProtectionInfoPayload {
        game_preset: game_preset.to_string(),
        category: protection_category(game_preset).to_string(),
        has_protections: !protections.is_empty(),
        protections,
    }
}
