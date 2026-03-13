use std::process::Stdio;
use std::{future::Future, pin::Pin};
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

fn is_zero_mapping_for_server(line: &str, server: &str) -> bool {
    let line = line.trim();
    !line.starts_with('#') && line == format!("0.0.0.0 {}", server)
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

fn append_managed_telemetry_hosts_entries(
    hosts_content: &str,
    game_preset: &str,
    servers: &[String],
) -> String {
    let mut content = hosts_content.to_string();
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }

    let block = build_managed_telemetry_hosts_block(game_preset, servers);
    if content.is_empty() {
        content.push_str(block.trim_start_matches('\n'));
    } else {
        content.push_str(&block);
    }
    content
}

pub(super) fn partition_blocked_servers(
    hosts_content: &str,
    servers: &[String],
) -> (Vec<String>, Vec<String>) {
    let mut blocked = Vec::new();
    let mut unblocked = Vec::new();

    for server in servers {
        let is_blocked = hosts_content
            .lines()
            .any(|line| is_zero_mapping_for_server(line, server));
        if is_blocked {
            blocked.push(server.clone());
        } else {
            unblocked.push(server.clone());
        }
    }

    (blocked, unblocked)
}

pub(super) fn evaluate_telemetry_protection(
    servers: &[String],
) -> (bool, Vec<String>, Vec<String>) {
    if servers.is_empty() {
        return (false, Vec::new(), Vec::new());
    }

    let hosts_content = std::fs::read_to_string("/etc/hosts").unwrap_or_default();
    let (blocked, unblocked) = partition_blocked_servers(&hosts_content, servers);
    (true, blocked, unblocked)
}

type HostsIoFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

trait HostsFileAccess {
    fn read<'a>(&'a self) -> HostsIoFuture<'a, Result<String, String>>;
    fn overwrite<'a>(&'a self, content: &'a str) -> HostsIoFuture<'a, Result<(), String>>;
}

struct SystemHostsFileAccess;

impl HostsFileAccess for SystemHostsFileAccess {
    fn read<'a>(&'a self) -> HostsIoFuture<'a, Result<String, String>> {
        Box::pin(async {
            tokio::fs::read_to_string("/etc/hosts")
                .await
                .map_err(|error| format!("读取 /etc/hosts 失败: {}", error))
        })
    }

    fn overwrite<'a>(&'a self, content: &'a str) -> HostsIoFuture<'a, Result<(), String>> {
        Box::pin(async move {
            match tokio::fs::write("/etc/hosts", content).await {
                Ok(()) => Ok(()),
                Err(direct_error) => {
                    let pkexec_result = overwrite_hosts_file_with_pkexec(content).await;
                    match pkexec_result {
                        Ok(()) => Ok(()),
                        Err(pkexec_error) => Err(format!(
                            "直接写入 /etc/hosts 失败: {}；{}",
                            direct_error, pkexec_error
                        )),
                    }
                }
            }
        })
    }
}

pub(super) async fn ensure_managed_telemetry_hosts_blocked(
    game_preset: &str,
    servers: &[String],
) -> Result<Vec<String>, String> {
    ensure_managed_telemetry_hosts_blocked_with_access(game_preset, servers, &SystemHostsFileAccess)
        .await
}

async fn ensure_managed_telemetry_hosts_blocked_with_access<A: HostsFileAccess + Sync>(
    game_preset: &str,
    servers: &[String],
    access: &A,
) -> Result<Vec<String>, String> {
    if servers.is_empty() {
        return Ok(Vec::new());
    }

    let hosts_content = access.read().await.unwrap_or_default();
    let (_, unblocked) = partition_blocked_servers(&hosts_content, servers);
    if unblocked.is_empty() {
        return Ok(Vec::new());
    }

    let new_content =
        append_managed_telemetry_hosts_entries(&hosts_content, game_preset, &unblocked);
    access.overwrite(&new_content).await?;
    Ok(unblocked)
}

pub(super) async fn restore_managed_telemetry_hosts_entries(
    game_preset: &str,
    servers: &[String],
) -> Result<usize, String> {
    restore_managed_telemetry_hosts_entries_with_access(game_preset, servers, &SystemHostsFileAccess)
        .await
}

async fn restore_managed_telemetry_hosts_entries_with_access<A: HostsFileAccess + Sync>(
    game_preset: &str,
    servers: &[String],
    access: &A,
) -> Result<usize, String> {
    if servers.is_empty() {
        return Ok(0);
    }

    let hosts_content = access.read().await?;
    let (new_content, removed_count) =
        remove_managed_telemetry_hosts_entries(&hosts_content, game_preset, servers);
    if removed_count == 0 {
        return Ok(0);
    }

    access.overwrite(&new_content).await?;
    Ok(removed_count)
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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::{
        build_managed_telemetry_hosts_block, partition_blocked_servers,
        remove_managed_telemetry_hosts_entries, restore_managed_telemetry_hosts_entries_with_access,
        ensure_managed_telemetry_hosts_blocked_with_access,
    };

    #[derive(Clone)]
    struct MemoryHostsFileAccess {
        content: Arc<Mutex<String>>,
        writes: Arc<Mutex<Vec<String>>>,
    }

    impl MemoryHostsFileAccess {
        fn new(content: &str) -> Self {
            Self {
                content: Arc::new(Mutex::new(content.to_string())),
                writes: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl super::HostsFileAccess for MemoryHostsFileAccess {
        fn read<'a>(&'a self) -> super::HostsIoFuture<'a, Result<String, String>> {
            let content = self.content.lock().expect("content lock").clone();
            Box::pin(async move { Ok(content) })
        }

        fn overwrite<'a>(&'a self, content: &'a str) -> super::HostsIoFuture<'a, Result<(), String>> {
            let content = content.to_string();
            let this = self.clone();
            Box::pin(async move {
                this.content
                    .lock()
                    .expect("content lock")
                    .clone_from(&content);
                this.writes.lock().expect("writes lock").push(content);
                Ok(())
            })
        }
    }

    #[test]
    fn managed_telemetry_hosts_block_uses_start_and_end_markers() {
        let block = build_managed_telemetry_hosts_block(
            "WutheringWaves",
            &["telemetry.example.com".to_string()],
        );
        assert!(block.contains("# SSMT4 遥测屏蔽 START - WutheringWaves"));
        assert!(block.contains("0.0.0.0 telemetry.example.com"));
        assert!(block.contains("# SSMT4 遥测屏蔽 END - WutheringWaves"));
    }

    #[test]
    fn restore_telemetry_only_removes_managed_block_entries() {
        let hosts = "\
127.0.0.1 localhost
# admin rule
0.0.0.0 telemetry.example.com
# SSMT4 遥测屏蔽 START - WutheringWaves
0.0.0.0 telemetry.example.com
0.0.0.0 metrics.example.com
# SSMT4 遥测屏蔽 END - WutheringWaves
";
        let servers = vec![
            "telemetry.example.com".to_string(),
            "metrics.example.com".to_string(),
        ];

        let (content, removed) =
            remove_managed_telemetry_hosts_entries(hosts, "WutheringWaves", &servers);

        assert_eq!(removed, 4);
        assert!(content.contains("127.0.0.1 localhost"));
        assert!(content.contains("# admin rule"));
        assert!(content.contains("0.0.0.0 telemetry.example.com"));
        assert!(!content.contains("# SSMT4 遥测屏蔽 START - WutheringWaves"));
        assert!(!content.contains("0.0.0.0 metrics.example.com"));
    }

    #[test]
    fn restore_telemetry_supports_legacy_marker_without_touching_manual_rules() {
        let hosts = "\
127.0.0.1 localhost
# SSMT4 遥测屏蔽 - WutheringWaves
0.0.0.0 telemetry.example.com
0.0.0.0 metrics.example.com
# manual rule kept
0.0.0.0 telemetry.example.com
";
        let servers = vec![
            "telemetry.example.com".to_string(),
            "metrics.example.com".to_string(),
        ];

        let (content, removed) =
            remove_managed_telemetry_hosts_entries(hosts, "WutheringWaves", &servers);

        assert_eq!(removed, 3);
        assert!(content.contains("# manual rule kept"));
        assert_eq!(content.matches("0.0.0.0 telemetry.example.com").count(), 1);
        assert!(!content.contains("# SSMT4 遥测屏蔽 - WutheringWaves"));
    }

    #[tokio::test]
    async fn ensure_telemetry_hosts_blocked_uses_injected_access() {
        let access = MemoryHostsFileAccess::new("127.0.0.1 localhost\n");
        let servers = vec![
            "telemetry.example.com".to_string(),
            "metrics.example.com".to_string(),
        ];

        let newly_blocked =
            ensure_managed_telemetry_hosts_blocked_with_access("WutheringWaves", &servers, &access)
                .await
                .expect("ensure hosts");

        assert_eq!(newly_blocked, servers);
        let writes = access.writes.lock().expect("writes lock");
        assert_eq!(writes.len(), 1);
        assert!(writes[0].contains("# SSMT4 遥测屏蔽 START - WutheringWaves"));
    }

    #[tokio::test]
    async fn restore_telemetry_hosts_entries_uses_injected_access() {
        let access = MemoryHostsFileAccess::new(
            "\
127.0.0.1 localhost
# SSMT4 遥测屏蔽 START - WutheringWaves
0.0.0.0 telemetry.example.com
# SSMT4 遥测屏蔽 END - WutheringWaves
",
        );
        let servers = vec!["telemetry.example.com".to_string()];

        let removed =
            restore_managed_telemetry_hosts_entries_with_access("WutheringWaves", &servers, &access)
                .await
                .expect("restore hosts");

        assert_eq!(removed, 3);
        let final_content = access.content.lock().expect("content lock").clone();
        assert!(!final_content.contains("telemetry.example.com"));
    }

    #[test]
    fn partition_blocked_servers_requires_exact_zero_mapping() {
        let hosts = "\
0.0.0.0 telemetry.example.com
127.0.0.1 metrics.example.com
";
        let servers = vec![
            "telemetry.example.com".to_string(),
            "metrics.example.com".to_string(),
        ];

        let (blocked, unblocked) = partition_blocked_servers(hosts, &servers);
        assert_eq!(blocked, vec!["telemetry.example.com".to_string()]);
        assert_eq!(unblocked, vec!["metrics.example.com".to_string()]);
    }
}
