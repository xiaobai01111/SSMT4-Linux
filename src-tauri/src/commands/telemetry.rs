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

fn get_channel_protection_config(
    game_preset: &str,
) -> Option<game_presets::ChannelProtectionConfig> {
    let game_preset = canonical_preset(game_preset);
    game_presets::get_preset(&game_preset).and_then(|p| p.channel_protection.clone())
}

fn require_protection_before_launch(game_preset: &str) -> bool {
    let game_preset = canonical_preset(game_preset);
    game_presets::get_preset(&game_preset)
        .map(|p| p.require_protection_before_launch)
        .unwrap_or(true)
}

const CHANNEL_MODE_INIT: &str = "init";
const CHANNEL_MODE_PROTECTED: &str = "protected";
const CHANNEL_ENFORCEMENT_BLOCK: &str = "block";
const CHANNEL_ENFORCEMENT_WARN: &str = "warn";

fn channel_mode_setting_key(game_preset: &str) -> String {
    format!("protection.channel.mode.{}", game_preset)
}

fn normalize_channel_mode(mode: &str, has_init_value: bool) -> Option<String> {
    let normalized = mode.trim().to_ascii_lowercase();
    match normalized.as_str() {
        CHANNEL_MODE_INIT if has_init_value => Some(CHANNEL_MODE_INIT.to_string()),
        CHANNEL_MODE_PROTECTED => Some(CHANNEL_MODE_PROTECTED.to_string()),
        _ => None,
    }
}

fn normalize_launch_enforcement(raw: Option<&str>) -> String {
    match raw.map(|v| v.trim().to_ascii_lowercase()) {
        Some(value) if value == CHANNEL_ENFORCEMENT_WARN => CHANNEL_ENFORCEMENT_WARN.to_string(),
        _ => CHANNEL_ENFORCEMENT_BLOCK.to_string(),
    }
}

fn default_channel_mode(config: &game_presets::ChannelProtectionConfig) -> String {
    let has_init = config.init_value.is_some();
    if let Some(default_mode) = config
        .default_mode
        .as_deref()
        .and_then(|v| normalize_channel_mode(v, has_init))
    {
        return default_mode;
    }
    if has_init {
        CHANNEL_MODE_INIT.to_string()
    } else {
        CHANNEL_MODE_PROTECTED.to_string()
    }
}

fn resolve_channel_mode(game_preset: &str, config: &game_presets::ChannelProtectionConfig) -> String {
    let has_init = config.init_value.is_some();
    if let Some(saved_mode) = db::get_setting(&channel_mode_setting_key(game_preset))
        .and_then(|v| normalize_channel_mode(&v, has_init))
    {
        return saved_mode;
    }
    default_channel_mode(config)
}

fn expected_channel_value(config: &game_presets::ChannelProtectionConfig, mode: &str) -> i64 {
    if mode == CHANNEL_MODE_INIT {
        config.init_value.unwrap_or(config.protected_value)
    } else {
        config.protected_value
    }
}

fn normalize_game_root(game_preset: &str, game_path: Option<&str>) -> Option<PathBuf> {
    let raw = game_path?.trim();
    if raw.is_empty() {
        return None;
    }

    let path = PathBuf::from(raw);
    let mut candidate = if path.is_file() {
        path.parent().map(|p| p.to_path_buf())
    } else if path.extension().is_some() {
        path.parent().map(|p| p.to_path_buf())
    } else {
        Some(path)
    }?;

    if let Some(default_folder) = game_presets::get_preset(game_preset)
        .map(|p| p.default_folder.trim())
        .filter(|v| !v.is_empty())
    {
        if let Some(inferred) = infer_game_root_with_default_folder(&candidate, default_folder) {
            candidate = inferred;
        }
    }

    Some(candidate)
}

fn infer_game_root_with_default_folder(candidate: &Path, default_folder: &str) -> Option<PathBuf> {
    let target = default_folder
        .split(['/', '\\'])
        .filter(|seg| !seg.trim().is_empty())
        .last()?
        .trim();
    if target.is_empty() {
        return None;
    }

    for ancestor in candidate.ancestors() {
        let Some(name) = ancestor.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.eq_ignore_ascii_case(target) {
            return Some(ancestor.to_path_buf());
        }
    }

    None
}

fn resolve_game_root_from_saved_config(game_preset: &str) -> Option<PathBuf> {
    let content = db::get_game_config(game_preset)?;
    let data = serde_json::from_str::<serde_json::Value>(&content).ok()?;
    let folder_candidate = data
        .pointer("/other/gameFolder")
        .and_then(|v| v.as_str())
        .or_else(|| data.pointer("/other/GameFolder").and_then(|v| v.as_str()));
    if let Some(root) = normalize_game_root(game_preset, folder_candidate) {
        return Some(root);
    }

    let path_candidate = data
        .pointer("/other/gamePath")
        .and_then(|v| v.as_str())
        .or_else(|| data.pointer("/other/GamePath").and_then(|v| v.as_str()));
    normalize_game_root(game_preset, path_candidate)
}

fn resolve_game_root(game_preset: &str, game_path: Option<&str>) -> Option<PathBuf> {
    normalize_game_root(game_preset, game_path)
        .or_else(|| resolve_game_root_from_saved_config(game_preset))
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
    init_value: Option<i64>,
    expected_value: Option<i64>,
    protected_value: Option<i64>,
    mode: Option<String>,
    launch_enforcement: String,
    config_path: Option<String>,
    error: Option<String>,
    backup_exists: bool,
}

fn channel_state_json(state: &ChannelProtectionState) -> serde_json::Value {
    serde_json::json!({
        "required": state.required,
        "enabled": state.enabled,
        "mode": state.mode,
        "launchEnforcement": state.launch_enforcement,
        "channelKey": state.channel_key,
        "currentValue": state.current_value,
        "initValue": state.init_value,
        "expectedValue": state.expected_value,
        "protectedValue": state.protected_value,
        "configPath": state.config_path,
        "error": state.error,
        "backupExists": state.backup_exists,
    })
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
        .map_err(|e| format!("读取渠道配置失败: {} ({})", config_path.display(), e))?;
    let mut data = serde_json::from_str::<serde_json::Value>(&content)
        .map_err(|e| format!("解析渠道配置失败: {} ({})", config_path.display(), e))?;
    let Some(obj) = data.as_object_mut() else {
        return Err(format!("渠道配置结构非法: {}", config_path.display()));
    };
    // KRSDKConfig 里的渠道值需要保持字符串形态（例如 "205"）
    obj.insert(
        channel_key.to_string(),
        serde_json::Value::String(target.to_string()),
    );
    let output = serde_json::to_string_pretty(&data)
        .map_err(|e| format!("序列化渠道配置失败: {} ({})", config_path.display(), e))?;
    std::fs::write(config_path, output)
        .map_err(|e| format!("写入渠道配置失败: {} ({})", config_path.display(), e))?;
    Ok(())
}

fn evaluate_channel_protection(
    game_preset: &str,
    game_root: Option<&Path>,
) -> ChannelProtectionState {
    let Some(config) = get_channel_protection_config(game_preset) else {
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
    let backup_exists = db::get_setting(&backup_key).is_some();

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
        Err(err) => ChannelProtectionState {
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
            error: Some(err),
            backup_exists,
        },
    }
}

fn set_channel_mode_internal(
    game_preset: &str,
    game_root: &Path,
    requested_mode: &str,
) -> Result<ChannelProtectionState, String> {
    let Some(channel) = get_channel_protection_config(game_preset) else {
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
        if db::get_setting(&backup_key).is_none() {
            db::set_setting(&backup_key, &current.to_string());
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

    db::set_setting(&channel_mode_setting_key(game_preset), &mode);

    Ok(evaluate_channel_protection(game_preset, Some(game_root)))
}

#[tauri::command]
pub fn get_channel_protection_status(
    game_preset: String,
    game_path: Option<String>,
) -> Result<serde_json::Value, String> {
    let game_preset = canonical_preset(&game_preset);
    let game_root = resolve_game_root(&game_preset, game_path.as_deref());
    let state = evaluate_channel_protection(&game_preset, game_root.as_deref());
    Ok(serde_json::json!({
        "gamePreset": game_preset,
        "supported": state.required,
        "gameRoot": game_root.map(|p| p.to_string_lossy().to_string()),
        "channel": channel_state_json(&state),
    }))
}

#[tauri::command]
pub fn set_channel_protection_mode(
    game_preset: String,
    mode: String,
    game_path: String,
) -> Result<serde_json::Value, String> {
    let game_preset = canonical_preset(&game_preset);
    let game_root = normalize_game_root(&game_preset, Some(&game_path))
        .ok_or_else(|| "缺少游戏目录，无法切换渠道模式".to_string())?;
    let state = set_channel_mode_internal(&game_preset, &game_root, &mode)?;
    Ok(serde_json::json!({
        "gamePreset": game_preset,
        "supported": state.required,
        "gameRoot": game_root.to_string_lossy().to_string(),
        "channel": channel_state_json(&state),
    }))
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
    let channel_is_blocking =
        channel_state.required && channel_state.launch_enforcement == CHANNEL_ENFORCEMENT_BLOCK;
    let enforce_at_launch = supported
        && require_protection_before_launch(&game_preset)
        && (telemetry_required || files_required || channel_is_blocking);

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
                .expected_value
                .map(|v| v.to_string())
                .unwrap_or_else(|| "未知".to_string());
            let mode = channel_state
                .mode
                .clone()
                .unwrap_or_else(|| CHANNEL_MODE_PROTECTED.to_string());
            let level = if channel_state.launch_enforcement == CHANNEL_ENFORCEMENT_WARN {
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
        "channel": channel_state_json(&channel_state)
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
        "channel": channel_state_json(&channel_state)
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

    let channel_result = if get_channel_protection_config(&game_preset).is_some() {
        let state = evaluate_channel_protection(&game_preset, game_root.as_deref());
        serde_json::json!({
            "supported": true,
            "message": "渠道模式由 set_channel_protection_mode 控制，disable_telemetry 不再改写 KR_ChannelId",
            "state": channel_state_json(&state),
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

    let channel_result = if get_channel_protection_config(&game_preset).is_some() {
        let state = evaluate_channel_protection(&game_preset, game_root.as_deref());
        serde_json::json!({
            "supported": true,
            "message": "渠道模式由 set_channel_protection_mode 控制，restore_telemetry 不再改写 KR_ChannelId",
            "state": channel_state_json(&state),
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
    let game_root = normalize_game_root(&game_preset, Some(&game_path))
        .ok_or_else(|| "缺少游戏目录，无法应用防护".to_string())?;
    let mut results = serde_json::Map::new();

    let telemetry_result = disable_telemetry(game_preset.clone(), Some(game_path.clone())).await?;
    results.insert("telemetry".to_string(), telemetry_result);

    let dll_result = remove_telemetry_files(game_preset.clone(), game_path)?;
    results.insert("telemetryFiles".to_string(), dll_result);

    if let Some(channel_cfg) = get_channel_protection_config(&game_preset) {
        let mode_key = channel_mode_setting_key(&game_preset);
        let preferred_mode = db::get_setting(&mode_key)
            .and_then(|v| normalize_channel_mode(&v, channel_cfg.init_value.is_some()))
            .unwrap_or_else(|| default_channel_mode(&channel_cfg));
        let channel_state = set_channel_mode_internal(&game_preset, &game_root, &preferred_mode)?;
        results.insert(
            "channel".to_string(),
            serde_json::json!({
                "mode": preferred_mode,
                "state": channel_state_json(&channel_state),
            }),
        );
    }

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
        "GenshinImpact" | "HonkaiStarRail" | "ZenlessZoneZero" => "HoYoverse",
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
            let default_mode = default_channel_mode(&channel_cfg);
            let init_value = channel_cfg.init_value;
            p.push(serde_json::json!({
                "type": "channelRewrite",
                "name": "渠道参数防护",
                "description": format!(
                    "渠道模式: init={} / protected={}（默认 {}）",
                    init_value
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    channel_cfg.protected_value,
                    default_mode
                ),
                "channelKey": channel_cfg.channel_key,
                "initValue": init_value,
                "targetValue": channel_cfg.protected_value,
                "defaultMode": default_mode,
                "launchEnforcement": normalize_launch_enforcement(channel_cfg.launch_enforcement.as_deref()),
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
        let cfg = sample_channel_config();
        assert_eq!(default_channel_mode(&cfg), CHANNEL_MODE_INIT);
    }

    #[test]
    fn expected_channel_value_switches_between_init_and_protected() {
        let cfg = sample_channel_config();
        assert_eq!(expected_channel_value(&cfg, CHANNEL_MODE_INIT), 19);
        assert_eq!(expected_channel_value(&cfg, CHANNEL_MODE_PROTECTED), 205);
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

    #[test]
    fn infer_root_with_default_folder_from_nested_exe_dir() {
        let candidate = PathBuf::from(
            "/home/user/Games/WutheringWaves/Wuthering Waves Game/Client/Binaries/Win64",
        );
        let inferred = infer_game_root_with_default_folder(&candidate, "Wuthering Waves Game");
        assert_eq!(
            inferred,
            Some(PathBuf::from(
                "/home/user/Games/WutheringWaves/Wuthering Waves Game"
            ))
        );
    }

    #[test]
    fn infer_root_with_default_folder_keeps_none_when_unmatched() {
        let candidate = PathBuf::from("/home/user/Games/StarRail");
        let inferred = infer_game_root_with_default_folder(&candidate, "Wuthering Waves Game");
        assert_eq!(inferred, None);
    }
}
