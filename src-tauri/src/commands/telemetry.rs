use std::path::{Path, PathBuf};
use tracing::{error, info, warn};

use crate::configs::game_presets;

// ============================================================
// 遥测数据查询（配置驱动，从 game_presets 注册表获取）
// ============================================================

/// 根据游戏 preset 返回对应的遥测服务器列表
fn get_telemetry_servers(game_preset: &str) -> Vec<String> {
    game_presets::get_preset(game_preset)
        .map(|p| p.telemetry_servers.clone())
        .unwrap_or_default()
}

/// 根据游戏 preset 返回需要删除的遥测 DLL 路径
fn get_telemetry_dlls(game_preset: &str) -> Vec<String> {
    game_presets::get_preset(game_preset)
        .map(|p| p.telemetry_dlls.clone())
        .unwrap_or_default()
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

    // 允许传入尚不存在的 exe 路径：按“有扩展名即文件”推断其父目录
    if path.extension().is_some() {
        if let Some(parent) = path.parent() {
            return Some(parent.to_path_buf());
        }
    }

    Some(path)
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

pub fn check_game_protection_status_internal(
    game_preset: &str,
    game_path: Option<&str>,
) -> Result<serde_json::Value, String> {
    let game_root = normalize_game_root(game_path);

    let (telemetry_required, blocked, unblocked) = evaluate_telemetry_protection(game_preset);
    let telemetry_all_blocked = !telemetry_required || unblocked.is_empty();

    let (files_required, removed, existing, files_error) =
        evaluate_file_protection(game_preset, game_root.as_deref());
    let files_all_removed = !files_required || (existing.is_empty() && files_error.is_none());

    let supported = telemetry_required || files_required;

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

    let enabled = !supported || missing.is_empty();

    Ok(serde_json::json!({
        "gamePreset": game_preset,
        "supported": supported,
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
            "totalServers": get_telemetry_servers(game_preset).len(),
        },
        "files": {
            "required": files_required,
            "allRemoved": files_all_removed,
            "removed": removed,
            "existing": existing,
            "totalFiles": get_telemetry_dlls(game_preset).len(),
            "error": files_error,
        }
    }))
}

#[tauri::command]
pub fn check_game_protection_status(
    game_preset: String,
    game_path: Option<String>,
) -> Result<serde_json::Value, String> {
    check_game_protection_status_internal(&game_preset, game_path.as_deref())
}

// ============================================================
// 检查遥测屏蔽状态
// ============================================================

#[tauri::command]
pub fn check_telemetry_status(game_preset: String) -> Result<serde_json::Value, String> {
    let (supported, blocked, unblocked) = evaluate_telemetry_protection(&game_preset);
    if !supported {
        return Ok(serde_json::json!({
            "supported": false,
            "message": "该游戏无需遥测屏蔽"
        }));
    }

    let all_blocked = unblocked.is_empty();

    Ok(serde_json::json!({
        "supported": true,
        "allBlocked": all_blocked,
        "blocked": blocked,
        "unblocked": unblocked,
        "totalServers": get_telemetry_servers(&game_preset).len()
    }))
}

// ============================================================
// 屏蔽遥测服务器（写入 /etc/hosts）
// ============================================================

#[tauri::command]
pub async fn disable_telemetry(game_preset: String) -> Result<serde_json::Value, String> {
    let servers = get_telemetry_servers(&game_preset);
    if servers.is_empty() {
        return Ok(serde_json::json!({
            "success": true,
            "message": "该游戏无需遥测屏蔽"
        }));
    }

    // 读取当前 /etc/hosts 判断哪些还未屏蔽
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
        info!("[遥测] 所有遥测服务器已屏蔽");
        return Ok(serde_json::json!({
            "success": true,
            "message": "所有遥测服务器已屏蔽",
            "newlyBlocked": 0
        }));
    }

    // 构建要追加到 /etc/hosts 的内容
    let mut append_lines = String::new();
    append_lines.push_str(&format!("\n# SSMT4 遥测屏蔽 - {}\n", game_preset));
    for server in &to_block {
        append_lines.push_str(&format!("0.0.0.0 {}\n", server));
    }

    // 使用 pkexec 以 root 权限写入 /etc/hosts
    let cmd_str = format!(
        "echo '{}' >> /etc/hosts",
        append_lines.replace('\'', "'\\''")
    );

    info!(
        "[遥测] 屏蔽 {} 个遥测服务器: {:?}",
        to_block.len(),
        to_block
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

    info!("[遥测] 成功屏蔽 {} 个遥测服务器", to_block.len());

    Ok(serde_json::json!({
        "success": true,
        "message": format!("已屏蔽 {} 个遥测服务器", to_block.len()),
        "newlyBlocked": to_block.len(),
        "servers": to_block
    }))
}

// ============================================================
// 恢复遥测（从 /etc/hosts 移除 SSMT4 写入的屏蔽条目）
// ============================================================

#[tauri::command]
pub async fn restore_telemetry(game_preset: String) -> Result<serde_json::Value, String> {
    let servers = get_telemetry_servers(&game_preset);
    if servers.is_empty() {
        return Ok(serde_json::json!({
            "success": true,
            "message": "该游戏无遥测屏蔽条目"
        }));
    }

    let hosts_content = std::fs::read_to_string("/etc/hosts")
        .map_err(|e| format!("读取 /etc/hosts 失败: {}", e))?;

    let marker = format!("# SSMT4 遥测屏蔽 - {}", game_preset);
    let mut removed_count = 0usize;
    let mut in_ssmt4_block = false;

    // 逐行过滤：移除 SSMT4 标记行 + 该标记块中属于本游戏的 0.0.0.0 条目
    let filtered_lines: Vec<&str> = hosts_content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();

            // 遇到 SSMT4 标记行，进入该游戏的屏蔽块
            if trimmed == marker {
                in_ssmt4_block = true;
                removed_count += 1;
                return false; // 移除标记行
            }

            // 在屏蔽块内，检查是否是该游戏的遥测条目
            if in_ssmt4_block {
                let is_telemetry_entry = servers
                    .iter()
                    .any(|server| trimmed == format!("0.0.0.0 {}", server));
                if is_telemetry_entry {
                    removed_count += 1;
                    return false; // 移除该遥测条目
                }
                // 遇到非遥测条目（空行或其他内容），退出块
                if !trimmed.is_empty() {
                    in_ssmt4_block = false;
                }
            }

            // 额外安全检查：即使不在块内，也移除散落的 SSMT4 遥测条目
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
        info!("[遥测] {} 无需恢复，hosts 中未找到屏蔽条目", game_preset);
        return Ok(serde_json::json!({
            "success": true,
            "message": "未找到需要恢复的屏蔽条目",
            "removedEntries": 0
        }));
    }

    // 重建 hosts 内容，清理多余空行
    let new_content = filtered_lines.join("\n") + "\n";

    // 使用 pkexec 写回 /etc/hosts
    // 先写到临时文件再移动，避免截断风险
    let tmp_path = "/tmp/ssmt4_hosts_restore.tmp";
    std::fs::write(tmp_path, &new_content).map_err(|e| format!("写入临时文件失败: {}", e))?;

    let output = tokio::process::Command::new("pkexec")
        .arg("bash")
        .arg("-c")
        .arg(format!("cp {} /etc/hosts && rm {}", tmp_path, tmp_path))
        .output()
        .await
        .map_err(|e| format!("执行 pkexec 失败: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // 清理临时文件
        let _ = std::fs::remove_file(tmp_path);
        error!("[遥测] 恢复 /etc/hosts 失败: {}", stderr);
        return Err(format!(
            "恢复 /etc/hosts 失败（需要管理员权限）: {}",
            stderr
        ));
    }

    info!(
        "[遥测] 已从 /etc/hosts 移除 {} 条 {} 遥测屏蔽",
        removed_count, game_preset
    );

    Ok(serde_json::json!({
        "success": true,
        "message": format!("已恢复 {} 条遥测屏蔽条目", removed_count),
        "removedEntries": removed_count
    }))
}

// ============================================================
// 删除遥测 DLL（HoYoverse 游戏专用）
// ============================================================

#[tauri::command]
pub fn remove_telemetry_files(
    game_preset: String,
    game_path: String,
) -> Result<serde_json::Value, String> {
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

// ============================================================
// 一键安全防护（遥测屏蔽 + 删除 DLL）
// ============================================================

#[tauri::command]
pub async fn apply_game_protection(
    game_preset: String,
    game_path: String,
) -> Result<serde_json::Value, String> {
    let mut results = serde_json::Map::new();

    // 1. 屏蔽遥测服务器
    let telemetry_result = disable_telemetry(game_preset.clone()).await?;
    results.insert("telemetry".to_string(), telemetry_result);

    // 2. 删除遥测 DLL
    let dll_result = remove_telemetry_files(game_preset.clone(), game_path)?;
    results.insert("telemetryFiles".to_string(), dll_result);

    info!("[防护] 游戏 {} 安全防护已应用", game_preset);

    Ok(serde_json::json!({
        "success": true,
        "gamePreset": game_preset,
        "results": results
    }))
}

// ============================================================
// 获取游戏防护信息（前端用于显示）
// ============================================================

#[tauri::command]
pub fn get_game_protection_info(game_preset: String) -> Result<serde_json::Value, String> {
    let servers = get_telemetry_servers(&game_preset);
    let dlls = get_telemetry_dlls(&game_preset);

    let category = match game_preset.as_str() {
        "GIMI" | "SRMI" | "ZZMI" | "HIMI" => "HoYoverse",
        "WWMI" | "WuWa" => "Kuro Games",
        "EFMI" => "Seasun",
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
        p
    };

    Ok(serde_json::json!({
        "gamePreset": game_preset,
        "category": category,
        "protections": protections,
        "hasProtections": !protections.is_empty(),
    }))
}
