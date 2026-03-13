use std::path::PathBuf;

use tracing::{info, warn};

use super::catalog::{channel_protection_config, telemetry_dlls, telemetry_servers};
use super::channel::{
    channel_state_payload, default_channel_mode, evaluate_channel_protection,
    normalize_channel_mode, prepare_channel_protection_rollback, set_channel_mode_internal,
    ChannelProtectionRollback,
};
use super::files::StagedTelemetryFiles;
use super::game_root::{normalize_game_root, resolve_game_root};
use super::hosts::{
    ensure_managed_telemetry_hosts_blocked, restore_managed_telemetry_hosts_entries,
};
use super::types::{
    ApplyChannelProtectionPayload, ApplyGameProtectionPayload, ApplyGameProtectionResultsPayload,
    RemoveTelemetryFilesPayload, TelemetryActionPayload, TelemetryWorkflowPayload,
};

pub(super) async fn disable_telemetry(
    game_preset: &str,
    game_path: Option<&str>,
) -> Result<TelemetryWorkflowPayload, String> {
    let game_root = resolve_game_root(game_preset, game_path);

    let servers = telemetry_servers(game_preset);
    let telemetry_result = if servers.is_empty() {
        TelemetryActionPayload {
            supported: false,
            message: "该游戏无需域名屏蔽".to_string(),
            newly_blocked: None,
            removed_entries: None,
            servers: None,
            state: None,
        }
    } else {
        let newly_blocked = ensure_managed_telemetry_hosts_blocked(game_preset, &servers).await?;
        if newly_blocked.is_empty() {
            TelemetryActionPayload {
                supported: true,
                message: "所有遥测服务器已屏蔽".to_string(),
                newly_blocked: Some(0),
                removed_entries: None,
                servers: None,
                state: None,
            }
        } else {
            TelemetryActionPayload {
                supported: true,
                message: format!("已屏蔽 {} 个遥测服务器", newly_blocked.len()),
                newly_blocked: Some(newly_blocked.len()),
                removed_entries: None,
                servers: Some(newly_blocked),
                state: None,
            }
        }
    };

    let channel_result = if channel_protection_config(game_preset).is_some() {
        let state = evaluate_channel_protection(game_preset, game_root.as_deref());
        TelemetryActionPayload {
            supported: true,
            message: "渠道模式由 set_channel_protection_mode 控制，disable_telemetry 不再改写 KR_ChannelId".to_string(),
            newly_blocked: None,
            removed_entries: None,
            servers: None,
            state: Some(channel_state_payload(&state)),
        }
    } else {
        TelemetryActionPayload {
            supported: false,
            message: "该游戏无需渠道防护".to_string(),
            newly_blocked: None,
            removed_entries: None,
            servers: None,
            state: None,
        }
    };

    Ok(TelemetryWorkflowPayload {
        success: true,
        game_preset: game_preset.to_string(),
        telemetry: telemetry_result,
        channel: channel_result,
    })
}

pub(super) async fn restore_telemetry(
    game_preset: &str,
    game_path: Option<&str>,
) -> Result<TelemetryWorkflowPayload, String> {
    let game_root = resolve_game_root(game_preset, game_path);

    let servers = telemetry_servers(game_preset);
    let telemetry_result = if servers.is_empty() {
        TelemetryActionPayload {
            supported: false,
            message: "该游戏无域名屏蔽条目".to_string(),
            newly_blocked: None,
            removed_entries: None,
            servers: None,
            state: None,
        }
    } else {
        let removed_count = restore_managed_telemetry_hosts_entries(game_preset, &servers).await?;
        if removed_count == 0 {
            TelemetryActionPayload {
                supported: true,
                message: "未找到需要恢复的屏蔽条目".to_string(),
                newly_blocked: None,
                removed_entries: Some(0),
                servers: None,
                state: None,
            }
        } else {
            TelemetryActionPayload {
                supported: true,
                message: format!("已恢复 {} 条遥测屏蔽条目", removed_count),
                newly_blocked: None,
                removed_entries: Some(removed_count),
                servers: None,
                state: None,
            }
        }
    };

    let channel_result = if channel_protection_config(game_preset).is_some() {
        let state = evaluate_channel_protection(game_preset, game_root.as_deref());
        TelemetryActionPayload {
            supported: true,
            message: "渠道模式由 set_channel_protection_mode 控制，restore_telemetry 不再改写 KR_ChannelId".to_string(),
            newly_blocked: None,
            removed_entries: None,
            servers: None,
            state: Some(channel_state_payload(&state)),
        }
    } else {
        TelemetryActionPayload {
            supported: false,
            message: "该游戏无需渠道防护".to_string(),
            newly_blocked: None,
            removed_entries: None,
            servers: None,
            state: None,
        }
    };

    Ok(TelemetryWorkflowPayload {
        success: true,
        game_preset: game_preset.to_string(),
        telemetry: telemetry_result,
        channel: channel_result,
    })
}

pub(super) fn remove_telemetry_files(
    game_preset: &str,
    game_path: &str,
) -> Result<RemoveTelemetryFilesPayload, String> {
    let dlls = telemetry_dlls(game_preset);
    if dlls.is_empty() {
        return Ok(RemoveTelemetryFilesPayload {
            supported: false,
            message: Some("该游戏无需删除遥测文件".to_string()),
            removed: Vec::new(),
            not_found: Vec::new(),
            cleanup_warning: None,
        });
    }

    let game_dir = PathBuf::from(game_path);
    let mut staged = StagedTelemetryFiles::stage(game_preset, &game_dir)?;
    let removed = staged.removed();
    let not_found = staged.not_found.clone();
    let cleanup_warning = staged.commit().err();

    for dll_path in &removed {
        info!("[遥测] 已删除: {}", game_dir.join(dll_path).display());
    }

    Ok(RemoveTelemetryFilesPayload {
        supported: true,
        message: None,
        removed,
        not_found,
        cleanup_warning,
    })
}

pub(super) async fn apply_game_protection(
    game_preset: &str,
    game_path: &str,
) -> Result<ApplyGameProtectionPayload, String> {
    let game_root = normalize_game_root(game_preset, Some(game_path))
        .ok_or_else(|| "缺少游戏目录，无法应用防护".to_string())?;

    let channel_plan: Option<(String, ChannelProtectionRollback)> =
        if let Some(channel_cfg) = channel_protection_config(game_preset) {
            let mode_key = format!("protection.channel.mode.{}", game_preset);
            let preferred_mode = crate::configs::database::read_setting_value(&mode_key)
                .and_then(|value| normalize_channel_mode(&value, channel_cfg.init_value.is_some()))
                .unwrap_or_else(|| default_channel_mode(&channel_cfg));
            let rollback =
                prepare_channel_protection_rollback(game_preset, &game_root, &preferred_mode)?
                    .ok_or_else(|| "渠道防护预检查失败".to_string())?;
            Some((preferred_mode, rollback))
        } else {
            None
        };
    let mut staged_files = StagedTelemetryFiles::stage(game_preset, &game_root)?;

    let telemetry_result = match disable_telemetry(game_preset, Some(game_path)).await {
        Ok(result) => result,
        Err(error) => {
            if let Err(rollback_error) = staged_files.rollback() {
                return Err(format!(
                    "应用遥测域名屏蔽失败: {}；DLL 回滚失败: {}",
                    error, rollback_error
                ));
            }
            return Err(error);
        }
    };

    let dll_result = RemoveTelemetryFilesPayload {
        supported: true,
        message: None,
        removed: staged_files.removed(),
        not_found: staged_files.not_found.clone(),
        cleanup_warning: None,
    };

    let mut channel_result: Option<ApplyChannelProtectionPayload> = None;
    if let Some((preferred_mode, rollback)) = channel_plan {
        let channel_state =
            match set_channel_mode_internal(game_preset, &game_root, &preferred_mode) {
                Ok(state) => state,
                Err(error) => {
                    let mut rollback_errors = Vec::new();
                    if let Err(channel_error) = rollback.rollback(game_preset) {
                        rollback_errors.push(format!("恢复渠道配置失败: {}", channel_error));
                    }
                    if let Err(restore_error) =
                        restore_telemetry(game_preset, Some(game_path)).await
                    {
                        rollback_errors.push(format!("恢复 hosts 失败: {}", restore_error));
                    }
                    if let Err(file_error) = staged_files.rollback() {
                        rollback_errors.push(format!("恢复遥测 DLL 失败: {}", file_error));
                    }

                    if rollback_errors.is_empty() {
                        return Err(error);
                    }
                    return Err(format!(
                        "{}；回滚失败: {}",
                        error,
                        rollback_errors.join("；")
                    ));
                }
            };

        channel_result = Some(ApplyChannelProtectionPayload {
            mode: preferred_mode,
            state: channel_state_payload(&channel_state),
        });
    }

    let mut cleanup_warning = None;
    if let Err(cleanup_error) = staged_files.commit() {
        warn!("[防护] 清理遥测备份目录失败: {}", cleanup_error);
        cleanup_warning = Some(cleanup_error);
    }

    info!("[防护] 游戏 {} 安全防护已应用", game_preset);

    Ok(ApplyGameProtectionPayload {
        success: true,
        game_preset: game_preset.to_string(),
        results: ApplyGameProtectionResultsPayload {
            telemetry: telemetry_result.telemetry,
            telemetry_files: dll_result,
            channel: channel_result,
            cleanup_warning,
        },
    })
}
