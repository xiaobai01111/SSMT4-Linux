use std::path::{Path, PathBuf};
use tracing::{error, info, warn};

use crate::configs::database as db;
use crate::configs::game_presets;

fn canonical_preset(value: &str) -> String {
    crate::configs::game_identity::to_canonical_or_keep(value)
}

fn get_telemetry_servers(game_preset: &str) -> Vec<String> {
    let game_preset = canonical_preset(game_preset);
    game_presets::get_preset(&game_preset)
        .map(|p| p.telemetry_servers.clone())
        .unwrap_or_default()
}

fn get_telemetry_dlls(game_preset: &str) -> Vec<String> {
    let game_preset = canonical_preset(game_preset);
    game_presets::get_preset(&game_preset)
        .map(|p| p.telemetry_dlls.clone())
        .unwrap_or_default()
}

fn get_channel_protection_config(game_preset: &str) -> Option<game_presets::ChannelProtectionConfig> {
    let game_preset = canonical_preset(game_preset);
    game_presets::get_preset(&game_preset).and_then(|p| p.channel_protection.clone())
}

fn require_protection_before_launch(game_preset: &str) -> bool {
    let game_preset = canonical_preset(game_preset);
    game_presets::get_preset(&game_preset)
        .map(|p| p.require_protection_before_launch)
        .unwrap_or(true)
}

fn normalize_game_root(game_path: Option<&str>) -> Option<PathBuf> {
    let raw = game_path?.trim();
    if raw.is_empty() {
        return None;
    }

    let path = PathBuf::from(raw);
    if path.is_file() {
        return path.parent().map(|p| p.to_path_buf());
    }

    if path.extension().is_some() {
        if let Some(parent) = path.parent() {
            return Some(parent.to_path_buf());
        }
    }

    Some(path)
}

fn resolve_game_root_from_saved_config(game_preset: &str) -> Option<PathBuf> {
    let content = db::get_game_config(game_preset)?;
    let data = serde_json::from_str::<serde_json::Value>(&content).ok()?;
    let candidate = data
        .pointer("/other/gameFolder")
        .and_then(|v| v.as_str())
        .or_else(|| data.pointer("/other/GameFolder").and_then(|v| v.as_str()))?;
    normalize_game_root(Some(candidate))
}

fn resolve_game_root(game_preset: &str, game_path: Option<&str>) -> Option<PathBuf> {
    normalize_game_root(game_path).or_else(|| resolve_game_root_from_saved_config(game_preset))
}

fn evaluate_telemetry_protection(game_preset: &str) -> (bool, Vec<String>, Vec<String>) {
    let servers = get_telemetry_servers(game_preset);
    if servers.is_empty() {
        return (false, Vec::new(), Vec::new());
    }

    let hosts_content = std::fs::read_to_string("/etc/hosts").unwrap_or_default();
    let mut blocked: Vec<String> = Vec::new();
    let mut unblocked: Vec<String> = Vec::new();

    for server in &servers {
        let is_blocked = hosts_content.lines().any(|line| {
            let line = line.trim();
            !line.starts_with('#') && line.contains(server.as_str()) && line.contains("0.0.0.0")
        });

        if is_blocked {
            blocked.push(server.clone());
        } else {
            unblocked.push(server.clone());
        }
    }

    (true, blocked, unblocked)
}

fn evaluate_file_protection(
    game_preset: &str,
    game_root: Option<&Path>,
) -> (bool, Vec<String>, Vec<String>, Option<String>) {
    let dlls = get_telemetry_dlls(game_preset);
    if dlls.is_empty() {
        return (false, Vec::new(), Vec::new(), None);
    }

    let Some(root) = game_root else {
        return (
            true,
            Vec::new(),
            dlls.iter().map(|s| s.to_string()).collect(),
            Some("缺少游戏目录，无法校验遥测 DLL".to_string()),
        );
    };

    let mut removed: Vec<String> = Vec::new();
    let mut existing: Vec<String> = Vec::new();
    for dll in &dlls {
        let full_path = root.join(dll);
        if full_path.exists() {
            existing.push(dll.to_string());
        } else {
            removed.push(dll.to_string());
        }
    }

    (true, removed, existing, None)
}

#[derive(Debug, Clone)]
struct ChannelProtectionState {
    required: bool,
    enabled: bool,
    channel_key: Option<String>,
    current_value: Option<i64>,
    protected_value: Option<i64>,
    config_path: Option<String>,
    error: Option<String>,
    backup_exists: bool,
}

fn parse_i64_json(value: &serde_json::Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|n| i64::try_from(n).ok()))
        .or_else(|| value.as_str().and_then(|s| s.trim().parse::<i64>().ok()))
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
        .map_err(|e| format!("读取渠道配置失败: {} ({})", config_path.display(), e))?;
    let data = serde_json::from_str::<serde_json::Value>(&content)
        .map_err(|e| format!("解析渠道配置失败: {} ({})", config_path.display(), e))?;
    let Some(raw_value) = data.get(channel_key) else {
        return Err(format!("渠道配置缺少字段 {} ({})", channel_key, config_path.display()));
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
        .map_err(|e| format!("读取渠道配置失败: {} ({})", config_path.display(), e))?;
    let mut data = serde_json::from_str::<serde_json::Value>(&content)
        .map_err(|e| format!("解析渠道配置失败: {} ({})", config_path.display(), e))?;
    let Some(obj) = data.as_object_mut() else {
        return Err(format!("渠道配置结构非法: {}", config_path.display()));
    };
    // KRSDKConfig 里的渠道值需要保持字符串形态（例如 "205"）
    obj.insert(channel_key.to_string(), serde_json::Value::String(target.to_string()));
    let output = serde_json::to_string_pretty(&data)
        .map_err(|e| format!("序列化渠道配置失败: {} ({})", config_path.display(), e))?;
    std::fs::write(config_path, output)
        .map_err(|e| format!("写入渠道配置失败: {} ({})", config_path.display(), e))?;
    Ok(())
}

fn evaluate_channel_protection(game_preset: &str, game_root: Option<&Path>) -> ChannelProtectionState {
    let Some(config) = get_channel_protection_config(game_preset) else {
        return ChannelProtectionState {
            required: false,
            enabled: true,
            channel_key: None,
            current_value: None,
            protected_value: None,
            config_path: None,
            error: None,
            backup_exists: false,
        };
    };

    let channel_key = config.channel_key.trim().to_string();
    let protected_value = config.protected_value;
    if channel_key.is_empty() {
        return ChannelProtectionState {
            required: true,
            enabled: false,
            channel_key: None,
            current_value: None,
            protected_value: Some(protected_value),
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
            protected_value: Some(protected_value),
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
            protected_value: Some(protected_value),
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
            protected_value: Some(protected_value),
            config_path: Some(config_path.to_string_lossy().to_string()),
            error: Some("未找到渠道配置文件".to_string()),
            backup_exists: false,
        };
    }

    let backup_key = channel_backup_setting_key(game_preset, &config_path);
    let backup_exists = db::get_setting(&backup_key).is_some();

    match read_channel_value(&config_path, &channel_key) {
        Ok(current_value) => ChannelProtectionState {
            required: true,
            enabled: current_value == protected_value,
            channel_key: Some(channel_key),
            current_value: Some(current_value),
            protected_value: Some(protected_value),
            config_path: Some(config_path.to_string_lossy().to_string()),
            error: None,
            backup_exists,
        },
        Err(err) => ChannelProtectionState {
            required: true,
            enabled: false,
            channel_key: Some(channel_key),
            current_value: None,
            protected_value: Some(protected_value),
            config_path: Some(config_path.to_string_lossy().to_string()),
            error: Some(err),
            backup_exists,
        },
    }
}

pub fn check_game_protection_status_internal(
    game_preset: &str,
    game_path: Option<&str>,
) -> Result<serde_json::Value, String> {
    let game_preset = canonical_preset(game_preset);
    let game_root = resolve_game_root(&game_preset, game_path);

    let (telemetry_required, blocked, unblocked) = evaluate_telemetry_protection(&game_preset);
    let telemetry_all_blocked = !telemetry_required || unblocked.is_empty();

    let (files_required, removed, existing, files_error) =
        evaluate_file_protection(&game_preset, game_root.as_deref());
    let files_all_removed = !files_required || (existing.is_empty() && files_error.is_none());

    let channel_state = evaluate_channel_protection(&game_preset, game_root.as_deref());

    let supported = telemetry_required || files_required || channel_state.required;
    let enforce_at_launch = supported && require_protection_before_launch(&game_preset);

    let mut missing: Vec<String> = Vec::new();
    if telemetry_required && !telemetry_all_blocked {
        missing.push(format!("未屏蔽遥测域名: {}", unblocked.join(", ")));
    }
    if files_required {
        if let Some(err) = files_error.clone() {
            missing.push(err);
        } else if !existing.is_empty() {
            missing.push(format!("未移除遥测文件: {}", existing.join(", ")));
        }
    }
    if channel_state.required {
        if let Some(err) = channel_state.error.as_ref() {
            missing.push(err.clone());
        } else if !channel_state.enabled {
            let key = channel_state
                .channel_key
                .clone()
                .unwrap_or_else(|| "渠道字段".to_string());
            let current = channel_state
                .current_value
                .map(|v| v.to_string())
                .unwrap_or_else(|| "未知".to_string());
            let target = channel_state
                .protected_value
                .map(|v| v.to_string())
                .unwrap_or_else(|| "未知".to_string());
            missing.push(format!("{key} 未设置为 {target}（当前 {current}）"));
        }
    }

    let enabled = !supported || missing.is_empty();

    Ok(serde_json::json!({
        "gamePreset": game_preset,
        "supported": supported,
        "enforceAtLaunch": enforce_at_launch,
        "hasProtections": supported,
        "enabled": enabled,
        "allProtected": enabled,
        "missing": missing,
        "gameRoot": game_root.as_ref().map(|p| p.to_string_lossy().to_string()),
        "telemetry": {
            "required": telemetry_required,
            "allBlocked": telemetry_all_blocked,
            "blocked": blocked,
            "unblocked": unblocked,
            "totalServers": get_telemetry_servers(&game_preset).len(),
        },
        "files": {
            "required": files_required,
            "allRemoved": files_all_removed,
            "removed": removed,
            "existing": existing,
            "totalFiles": get_telemetry_dlls(&game_preset).len(),
            "error": files_error,
        },
        "channel": {
            "required": channel_state.required,
            "enabled": channel_state.enabled,
            "channelKey": channel_state.channel_key,
            "currentValue": channel_state.current_value,
            "protectedValue": channel_state.protected_value,
            "configPath": channel_state.config_path,
            "error": channel_state.error,
            "backupExists": channel_state.backup_exists,
        }
    }))
}

#[tauri::command]
pub fn check_game_protection_status(
    game_preset: String,
    game_path: Option<String>,
) -> Result<serde_json::Value, String> {
    let game_preset = canonical_preset(&game_preset);
    check_game_protection_status_internal(&game_preset, game_path.as_deref())
}

#[tauri::command]
pub fn check_telemetry_status(
    game_preset: String,
    game_path: Option<String>,
) -> Result<serde_json::Value, String> {
    let game_preset = canonical_preset(&game_preset);
    let (supported, blocked, unblocked) = evaluate_telemetry_protection(&game_preset);
    let channel_state = evaluate_channel_protection(
        &game_preset,
        resolve_game_root(&game_preset, game_path.as_deref()).as_deref(),
    );
    if !supported && !channel_state.required {
        return Ok(serde_json::json!({
            "supported": false,
            "message": "该游戏无需防护"
        }));
    }

    Ok(serde_json::json!({
        "supported": true,
        "allBlocked": !supported || unblocked.is_empty(),
        "blocked": blocked,
        "unblocked": unblocked,
        "totalServers": get_telemetry_servers(&game_preset).len(),
        "channel": {
            "required": channel_state.required,
            "enabled": channel_state.enabled,
            "channelKey": channel_state.channel_key,
            "currentValue": channel_state.current_value,
            "protectedValue": channel_state.protected_value,
            "configPath": channel_state.config_path,
            "error": channel_state.error,
            "backupExists": channel_state.backup_exists,
        }
    }))
}

#[tauri::command]
pub async fn disable_telemetry(
    game_preset: String,
    game_path: Option<String>,
) -> Result<serde_json::Value, String> {
    let game_preset = canonical_preset(&game_preset);
    let game_root = resolve_game_root(&game_preset, game_path.as_deref());

    let servers = get_telemetry_servers(&game_preset);
    let telemetry_result = if servers.is_empty() {
        serde_json::json!({
            "supported": false,
            "message": "该游戏无需域名屏蔽"
        })
    } else {
        let hosts_content = std::fs::read_to_string("/etc/hosts").unwrap_or_default();
        let mut to_block: Vec<String> = Vec::new();

        for server in &servers {
            let already_blocked = hosts_content.lines().any(|line| {
                let line = line.trim();
                !line.starts_with('#') && line.contains(server.as_str()) && line.contains("0.0.0.0")
            });
            if !already_blocked {
                to_block.push(server.clone());
            }
        }

        if to_block.is_empty() {
            serde_json::json!({
                "supported": true,
                "message": "所有遥测服务器已屏蔽",
                "newlyBlocked": 0
            })
        } else {
            let mut append_lines = String::new();
            append_lines.push_str(&format!("\n# SSMT4 遥测屏蔽 - {}\n", game_preset));
            for server in &to_block {
                append_lines.push_str(&format!("0.0.0.0 {}\n", server));
            }

            let cmd_str = format!(
                "echo '{}' >> /etc/hosts",
                append_lines.replace('\'', "'\\''")
            );

            let output = tokio::process::Command::new("pkexec")
                .arg("bash")
                .arg("-c")
                .arg(&cmd_str)
                .output()
                .await
                .map_err(|e| format!("执行 pkexec 失败: {}", e))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                error!("[遥测] 写入 /etc/hosts 失败: {}", stderr);
                return Err(format!(
                    "写入 /etc/hosts 失败（需要管理员权限）: {}",
                    stderr
                ));
            }

            serde_json::json!({
                "supported": true,
                "message": format!("已屏蔽 {} 个遥测服务器", to_block.len()),
                "newlyBlocked": to_block.len(),
                "servers": to_block
            })
        }
    };

    let channel_result = if let Some(channel) = get_channel_protection_config(&game_preset) {
        let Some(root) = game_root.as_ref() else {
            return Err("缺少游戏目录，无法应用渠道防护".to_string());
        };
        let rel = channel
            .config_relative_path
            .trim()
            .trim_matches(['/', '\\'])
            .to_string();
        if rel.is_empty() {
            return Err("渠道防护配置缺少 config_relative_path".to_string());
        }
        let config_path = root.join(rel);
        if !config_path.exists() {
            return Err(format!("未找到渠道配置文件: {}", config_path.display()));
        }
        let channel_key = channel.channel_key.trim();
        if channel_key.is_empty() {
            return Err("渠道防护配置缺少 channel_key".to_string());
        }

        let current = read_channel_value(&config_path, channel_key)?;
        let target = channel.protected_value;
        let changed = current != target;
        let backup_key = channel_backup_setting_key(&game_preset, &config_path);
        let mut backup_saved = false;

        if changed {
            db::set_setting(&backup_key, &current.to_string());
            backup_saved = true;
            write_channel_value(&config_path, channel_key, target)?;
            info!(
                "[防护] {} {}: {} -> {} ({})",
                game_preset,
                channel_key,
                current,
                target,
                config_path.display()
            );
        } else {
            info!(
                "[防护] {} {} 已是目标值 {} ({})",
                game_preset,
                channel_key,
                target,
                config_path.display()
            );
        }

        serde_json::json!({
            "supported": true,
            "channelKey": channel_key,
            "configPath": config_path.to_string_lossy().to_string(),
            "previousValue": current,
            "targetValue": target,
            "changed": changed,
            "backupSaved": backup_saved,
        })
    } else {
        serde_json::json!({
            "supported": false,
            "message": "该游戏无需渠道防护"
        })
    };

    Ok(serde_json::json!({
        "success": true,
        "gamePreset": game_preset,
        "telemetry": telemetry_result,
        "channel": channel_result,
    }))
}

#[tauri::command]
pub async fn restore_telemetry(
    game_preset: String,
    game_path: Option<String>,
) -> Result<serde_json::Value, String> {
    let game_preset = canonical_preset(&game_preset);
    let game_root = resolve_game_root(&game_preset, game_path.as_deref());

    let servers = get_telemetry_servers(&game_preset);
    let telemetry_result = if servers.is_empty() {
        serde_json::json!({
            "supported": false,
            "message": "该游戏无域名屏蔽条目"
        })
    } else {
        let hosts_content = std::fs::read_to_string("/etc/hosts")
            .map_err(|e| format!("读取 /etc/hosts 失败: {}", e))?;

        let marker = format!("# SSMT4 遥测屏蔽 - {}", game_preset);
        let mut removed_count = 0usize;
        let mut in_ssmt4_block = false;

        let filtered_lines: Vec<&str> = hosts_content
            .lines()
            .filter(|line| {
                let trimmed = line.trim();

                if trimmed == marker {
                    in_ssmt4_block = true;
                    removed_count += 1;
                    return false;
                }

                if in_ssmt4_block {
                    let is_telemetry_entry = servers
                        .iter()
                        .any(|server| trimmed == format!("0.0.0.0 {}", server));
                    if is_telemetry_entry {
                        removed_count += 1;
                        return false;
                    }
                    if !trimmed.is_empty() {
                        in_ssmt4_block = false;
                    }
                }

                if trimmed.starts_with("0.0.0.0 ") {
                    let is_our_entry = servers
                        .iter()
                        .any(|server| trimmed == format!("0.0.0.0 {}", server));
                    if is_our_entry {
                        removed_count += 1;
                        return false;
                    }
                }

                true
            })
            .collect();

        if removed_count == 0 {
            serde_json::json!({
                "supported": true,
                "message": "未找到需要恢复的屏蔽条目",
                "removedEntries": 0
            })
        } else {
            let new_content = filtered_lines.join("\n") + "\n";
            let tmp_path = "/tmp/ssmt4_hosts_restore.tmp";
            std::fs::write(tmp_path, &new_content)
                .map_err(|e| format!("写入临时文件失败: {}", e))?;

            let output = tokio::process::Command::new("pkexec")
                .arg("bash")
                .arg("-c")
                .arg(format!("cp {} /etc/hosts && rm {}", tmp_path, tmp_path))
                .output()
                .await
                .map_err(|e| format!("执行 pkexec 失败: {}", e))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let _ = std::fs::remove_file(tmp_path);
                error!("[遥测] 恢复 /etc/hosts 失败: {}", stderr);
                return Err(format!(
                    "恢复 /etc/hosts 失败（需要管理员权限）: {}",
                    stderr
                ));
            }

            serde_json::json!({
                "supported": true,
                "message": format!("已恢复 {} 条遥测屏蔽条目", removed_count),
                "removedEntries": removed_count
            })
        }
    };

    let channel_result = if let Some(channel) = get_channel_protection_config(&game_preset) {
        let Some(root) = game_root.as_ref() else {
            return Err("缺少游戏目录，无法恢复渠道防护".to_string());
        };
        let rel = channel
            .config_relative_path
            .trim()
            .trim_matches(['/', '\\'])
            .to_string();
        if rel.is_empty() {
            return Err("渠道防护配置缺少 config_relative_path".to_string());
        }
        let config_path = root.join(rel);
        if !config_path.exists() {
            return Err(format!("未找到渠道配置文件: {}", config_path.display()));
        }
        let channel_key = channel.channel_key.trim();
        if channel_key.is_empty() {
            return Err("渠道防护配置缺少 channel_key".to_string());
        }

        let backup_key = channel_backup_setting_key(&game_preset, &config_path);
        let backup_value = db::get_setting(&backup_key).and_then(|v| v.trim().parse::<i64>().ok());
        let current = read_channel_value(&config_path, channel_key)?;
        if let Some(original) = backup_value {
            if current != original {
                write_channel_value(&config_path, channel_key, original)?;
                info!(
                    "[防护] {} {} 已恢复: {} -> {} ({})",
                    game_preset,
                    channel_key,
                    current,
                    original,
                    config_path.display()
                );
            }

            serde_json::json!({
                "supported": true,
                "restored": true,
                "channelKey": channel_key,
                "configPath": config_path.to_string_lossy().to_string(),
                "currentValue": current,
                "restoredValue": original,
                "changed": current != original,
            })
        } else {
            warn!(
                "[防护] {} {} 无备份值，跳过恢复 ({})",
                game_preset,
                channel_key,
                config_path.display()
            );
            serde_json::json!({
                "supported": true,
                "restored": false,
                "channelKey": channel_key,
                "configPath": config_path.to_string_lossy().to_string(),
                "currentValue": current,
                "reason": "未找到可恢复的原始值，请先应用防护后再恢复",
            })
        }
    } else {
        serde_json::json!({
            "supported": false,
            "message": "该游戏无需渠道防护"
        })
    };

    Ok(serde_json::json!({
        "success": true,
        "gamePreset": game_preset,
        "telemetry": telemetry_result,
        "channel": channel_result
    }))
}

#[tauri::command]
pub fn remove_telemetry_files(
    game_preset: String,
    game_path: String,
) -> Result<serde_json::Value, String> {
    let game_preset = canonical_preset(&game_preset);
    let dlls = get_telemetry_dlls(&game_preset);
    if dlls.is_empty() {
        return Ok(serde_json::json!({
            "supported": false,
            "message": "该游戏无需删除遥测文件"
        }));
    }

    let game_dir = PathBuf::from(&game_path);
    let mut removed: Vec<String> = Vec::new();
    let mut not_found: Vec<String> = Vec::new();

    for dll_path in &dlls {
        let full_path = game_dir.join(dll_path);
        if full_path.exists() {
            match std::fs::remove_file(&full_path) {
                Ok(_) => {
                    info!("[遥测] 已删除: {}", full_path.display());
                    removed.push(dll_path.to_string());
                }
                Err(e) => {
                    warn!("[遥测] 删除失败 {}: {}", full_path.display(), e);
                }
            }
        } else {
            not_found.push(dll_path.to_string());
        }
    }

    Ok(serde_json::json!({
        "supported": true,
        "removed": removed,
        "notFound": not_found
    }))
}

#[tauri::command]
pub async fn apply_game_protection(
    game_preset: String,
    game_path: String,
) -> Result<serde_json::Value, String> {
    let game_preset = canonical_preset(&game_preset);
    let mut results = serde_json::Map::new();

    let telemetry_result = disable_telemetry(game_preset.clone(), Some(game_path.clone())).await?;
    results.insert("telemetry".to_string(), telemetry_result);

    let dll_result = remove_telemetry_files(game_preset.clone(), game_path)?;
    results.insert("telemetryFiles".to_string(), dll_result);

    info!("[防护] 游戏 {} 安全防护已应用", game_preset);

    Ok(serde_json::json!({
        "success": true,
        "gamePreset": game_preset,
        "results": results
    }))
}

#[tauri::command]
pub fn get_game_protection_info(game_preset: String) -> Result<serde_json::Value, String> {
    let game_preset = canonical_preset(&game_preset);
    let servers = get_telemetry_servers(&game_preset);
    let dlls = get_telemetry_dlls(&game_preset);
    let channel = get_channel_protection_config(&game_preset);

    let category = match game_preset.as_str() {
        "GenshinImpact" | "HonkaiStarRail" | "ZenlessZoneZero" | "HonkaiImpact3rd" => {
            "HoYoverse"
        }
        "WutheringWaves" => "Kuro Games",
        "SnowbreakContainmentZone" => "Seasun",
        _ => "Unknown",
    };

    let protections: Vec<serde_json::Value> = {
        let mut p = Vec::new();
        if !servers.is_empty() {
            p.push(serde_json::json!({
                "type": "telemetryBlock",
                "name": "遥测服务器屏蔽",
                "description": format!("屏蔽 {} 个遥测/数据上报服务器", servers.len()),
                "servers": servers,
            }));
        }
        if !dlls.is_empty() {
            p.push(serde_json::json!({
                "type": "telemetryDll",
                "name": "删除遥测 DLL",
                "description": "删除游戏内置的遥测数据收集模块",
                "files": dlls,
            }));
        }
        if let Some(channel_cfg) = channel {
            p.push(serde_json::json!({
                "type": "channelRewrite",
                "name": "渠道参数防护",
                "description": format!("将 {} 设置为 {}", channel_cfg.channel_key, channel_cfg.protected_value),
                "channelKey": channel_cfg.channel_key,
                "targetValue": channel_cfg.protected_value,
                "configRelativePath": channel_cfg.config_relative_path,
            }));
        }
        p
    };

    Ok(serde_json::json!({
        "gamePreset": game_preset,
        "category": category,
        "protections": protections,
        "hasProtections": !protections.is_empty(),
    }))
}
