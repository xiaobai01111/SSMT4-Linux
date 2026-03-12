use std::process::Stdio;
use tokio::io::AsyncWriteExt;

const TELEMETRY_HOSTS_BLOCK_MARKER_PREFIX: &str = "# SSMT4 遥测屏蔽";

fn telemetry_hosts_block_start_marker(game_preset: &str) -> String {
    format!(
        "{TELEMETRY_HOSTS_BLOCK_MARKER_PREFIX} START - {}",
        game_preset
    )
}

fn telemetry_hosts_block_end_marker(game_preset: &str) -> String {
    format!(
        "{TELEMETRY_HOSTS_BLOCK_MARKER_PREFIX} END - {}",
        game_preset
    )
}

fn legacy_telemetry_hosts_marker(game_preset: &str) -> String {
    format!("{TELEMETRY_HOSTS_BLOCK_MARKER_PREFIX} - {}", game_preset)
}

fn is_managed_telemetry_hosts_entry(line: &str, servers: &[String]) -> bool {
    servers
        .iter()
        .any(|server| line == format!("0.0.0.0 {}", server))
}

pub(super) fn build_managed_telemetry_hosts_block(game_preset: &str, servers: &[String]) -> String {
    let mut block = String::new();
    block.push('\n');
    block.push_str(&telemetry_hosts_block_start_marker(game_preset));
    block.push('\n');
    for server in servers {
        block.push_str(&format!("0.0.0.0 {}\n", server));
    }
    block.push_str(&telemetry_hosts_block_end_marker(game_preset));
    block.push('\n');
    block
}

pub(super) fn remove_managed_telemetry_hosts_entries(
    hosts_content: &str,
    game_preset: &str,
    servers: &[String],
) -> (String, usize) {
    let start_marker = telemetry_hosts_block_start_marker(game_preset);
    let end_marker = telemetry_hosts_block_end_marker(game_preset);
    let legacy_marker = legacy_telemetry_hosts_marker(game_preset);
    let lines: Vec<&str> = hosts_content.lines().collect();
    let mut filtered_lines: Vec<&str> = Vec::with_capacity(lines.len());
    let mut removed_count = 0usize;
    let mut index = 0usize;

    while index < lines.len() {
        let trimmed = lines[index].trim();

        if trimmed == start_marker {
            removed_count += 1;
            index += 1;

            while index < lines.len() {
                let current = lines[index].trim();
                if current == end_marker {
                    removed_count += 1;
                    index += 1;
                    break;
                }
                if current.is_empty() {
                    removed_count += 1;
                    index += 1;
                    continue;
                }
                if is_managed_telemetry_hosts_entry(current, servers) {
                    removed_count += 1;
                    index += 1;
                    continue;
                }
                break;
            }
            continue;
        }

        if trimmed == legacy_marker {
            removed_count += 1;
            index += 1;

            while index < lines.len() {
                let current = lines[index].trim();
                if is_managed_telemetry_hosts_entry(current, servers) {
                    removed_count += 1;
                    index += 1;
                    continue;
                }
                break;
            }
            continue;
        }

        filtered_lines.push(lines[index]);
        index += 1;
    }

    let mut new_content = filtered_lines.join("\n");
    if !new_content.is_empty() || removed_count > 0 || hosts_content.ends_with('\n') {
        new_content.push('\n');
    }

    (new_content, removed_count)
}

pub(super) async fn overwrite_hosts_file_with_pkexec(content: &str) -> Result<(), String> {
    let mut child = tokio::process::Command::new("pkexec")
        .arg("tee")
        .arg("/etc/hosts")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("执行 pkexec 失败: {}", e))?;

    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| "无法获取 pkexec stdin".to_string())?;
        stdin
            .write_all(content.as_bytes())
            .await
            .map_err(|e| format!("写入 /etc/hosts 内容失败: {}", e))?;
        stdin
            .shutdown()
            .await
            .map_err(|e| format!("关闭 pkexec stdin 失败: {}", e))?;
    }

    let output = child
        .wait_with_output()
        .await
        .map_err(|e| format!("等待 pkexec 完成失败: {}", e))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(format!(
        "恢复 /etc/hosts 失败（需要管理员权限）: {}",
        stderr
    ))
}
