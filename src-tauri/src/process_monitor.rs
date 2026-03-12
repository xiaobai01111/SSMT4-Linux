use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::{LazyLock, Mutex};

use tracing::info;

#[derive(Debug, Clone)]
pub struct GameProcess {
    pub pid: u32,
    pub exe_name: String,
    pub launch_exe_path: String,
    pub root_start_ticks: Option<u64>,
}

#[derive(Debug, Default)]
struct ProcessMonitorState {
    running_games: HashMap<String, GameProcess>,
    launching_games: HashSet<String>,
    game_write_scopes: HashSet<String>,
}

static PROCESS_MONITOR_STATE: LazyLock<Mutex<ProcessMonitorState>> =
    LazyLock::new(|| Mutex::new(ProcessMonitorState::default()));

#[derive(Debug, Clone)]
struct ProcessMatchCriteria {
    expected_exe_name: String,
    expected_launch_path: Option<String>,
    root_pid: Option<u32>,
    min_start_ticks: Option<u64>,
}

impl ProcessMatchCriteria {
    fn from_registered_process(process: &GameProcess) -> Self {
        Self {
            expected_exe_name: process.exe_name.clone(),
            expected_launch_path: normalize_process_target_path(&process.launch_exe_path),
            root_pid: (process.pid != 0).then_some(process.pid),
            min_start_ticks: process.root_start_ticks,
        }
    }
}

#[derive(Debug, Clone)]
struct ProcessSnapshotEntry {
    pid: u32,
    ppid: u32,
    start_ticks: Option<u64>,
    exe_path: Option<String>,
    comm_name: Option<String>,
    cmd_args: Vec<String>,
}

#[derive(Debug)]
pub struct LaunchGuard {
    game_name: String,
}

impl Drop for LaunchGuard {
    fn drop(&mut self) {
        if let Ok(mut state) = PROCESS_MONITOR_STATE.lock() {
            state.launching_games.remove(&self.game_name);
            info!("已释放游戏启动锁: {}", self.game_name);
        }
    }
}

#[derive(Debug)]
pub struct GameWriteGuard {
    scope_key: String,
}

impl Drop for GameWriteGuard {
    fn drop(&mut self) {
        if let Ok(mut state) = PROCESS_MONITOR_STATE.lock() {
            state.game_write_scopes.remove(&self.scope_key);
            info!("已释放游戏写操作锁: {}", self.scope_key);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DerivedRegionScope {
    BizPrefix(String),
    LauncherApiKnown(&'static str),
    LauncherApiFingerprint(String),
    RegionHint(String),
    Default,
}

impl DerivedRegionScope {
    fn into_scope_key(self) -> String {
        match self {
            Self::BizPrefix(scope)
            | Self::LauncherApiFingerprint(scope)
            | Self::RegionHint(scope) => scope,
            Self::LauncherApiKnown(scope) => scope.to_string(),
            Self::Default => "default".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedLauncherApi {
    normalized: String,
    scheme: String,
    host: String,
    path: String,
    query: HashMap<String, String>,
}

fn normalize_scope_value(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .map(|raw| raw.to_ascii_lowercase())
}

fn fingerprint_launcher_api(normalized_api: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    normalized_api.hash(&mut hasher);
    format!("source_{:x}", hasher.finish())
}

fn parse_launcher_api(raw: &str) -> Option<NormalizedLauncherApi> {
    let normalized = raw.trim();
    if normalized.is_empty() {
        return None;
    }

    let parsed = reqwest::Url::parse(normalized).ok()?;
    let query = parsed
        .query_pairs()
        .map(|(key, value)| {
            (
                key.trim().to_ascii_lowercase(),
                value.trim().to_ascii_lowercase(),
            )
        })
        .collect();

    Some(NormalizedLauncherApi {
        normalized: normalized.to_ascii_lowercase(),
        scheme: parsed.scheme().trim().to_ascii_lowercase(),
        host: parsed
            .host_str()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase(),
        path: parsed.path().trim().to_ascii_lowercase(),
        query,
    })
}

fn derive_scope_from_launcher_api(api_raw: &str) -> DerivedRegionScope {
    let normalized = api_raw.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return DerivedRegionScope::Default;
    }

    let parsed = parse_launcher_api(api_raw);
    if let Some(api) = parsed {
        if api.scheme == "snowbreak" {
            return DerivedRegionScope::LauncherApiKnown("global");
        }

        if api.host.contains("mihoyo.com")
            || api.host.contains("hypergryph.com")
            || api.host.contains("prod-cn-")
            || api.path.contains("prod-cn-")
        {
            return DerivedRegionScope::LauncherApiKnown("cn");
        }

        if api.host.contains("hoyoverse.com")
            || api.host.contains("gryphline.com")
            || api.host.contains("prod-alicdn-")
            || api.path.contains("prod-alicdn-")
        {
            return DerivedRegionScope::LauncherApiKnown("global");
        }

        if matches!(
            api.query.get("channel").map(String::as_str),
            Some("1") | Some("6")
        ) {
            return DerivedRegionScope::LauncherApiKnown(
                if api.query.get("channel").map(String::as_str) == Some("1") {
                    "cn"
                } else {
                    "global"
                },
            );
        }

        if matches!(
            api.query.get("sub_channel").map(String::as_str),
            Some("1") | Some("6")
        ) {
            return DerivedRegionScope::LauncherApiKnown(
                if api.query.get("sub_channel").map(String::as_str) == Some("1") {
                    "cn"
                } else {
                    "global"
                },
            );
        }

        return DerivedRegionScope::LauncherApiFingerprint(fingerprint_launcher_api(
            &api.normalized,
        ));
    }

    if normalized.starts_with("snowbreak://") {
        return DerivedRegionScope::LauncherApiKnown("global");
    }

    DerivedRegionScope::LauncherApiFingerprint(fingerprint_launcher_api(&normalized))
}

fn read_running_game(game_name: &str) -> Option<GameProcess> {
    PROCESS_MONITOR_STATE
        .lock()
        .ok()
        .and_then(|state| state.running_games.get(game_name).cloned())
}

fn snapshot_running_games() -> Vec<(String, GameProcess)> {
    PROCESS_MONITOR_STATE
        .lock()
        .map(|state| {
            state
                .running_games
                .iter()
                .map(|(game_name, process)| (game_name.clone(), process.clone()))
                .collect()
        })
        .unwrap_or_default()
}

pub fn acquire_launch_guard(game_name: &str) -> Result<LaunchGuard, String> {
    let mut state = PROCESS_MONITOR_STATE
        .lock()
        .map_err(|_| "获取游戏启动锁失败".to_string())?;
    if state.launching_games.contains(game_name) {
        return Err("游戏正在启动中，请勿重复点击".to_string());
    }
    state.launching_games.insert(game_name.to_string());
    info!("已加锁游戏启动流程: {}", game_name);
    Ok(LaunchGuard {
        game_name: game_name.to_string(),
    })
}

pub fn acquire_game_write_guard(
    game_root: &Path,
    region: &str,
    operation: &str,
) -> Result<GameWriteGuard, String> {
    let scope_key = make_game_write_scope_key(game_root, region);
    let mut state = PROCESS_MONITOR_STATE
        .lock()
        .map_err(|_| "获取游戏写操作锁失败".to_string())?;

    if state.game_write_scopes.contains(&scope_key) {
        return Err(format!(
            "当前目录与区服已有写操作进行中，请稍后重试（scope: {}）",
            scope_key
        ));
    }

    state.game_write_scopes.insert(scope_key.clone());
    info!("已加锁游戏写操作: op={}, scope={}", operation, scope_key);
    Ok(GameWriteGuard { scope_key })
}

pub fn make_game_write_scope_key(game_root: &Path, region: &str) -> String {
    let normalized_root = std::fs::canonicalize(game_root)
        .unwrap_or_else(|_| game_root.to_path_buf())
        .to_string_lossy()
        .to_string();
    let normalized_region = region.trim().to_ascii_lowercase();
    let region_key = if normalized_region.is_empty() {
        "default"
    } else {
        normalized_region.as_str()
    };
    format!("{}::{}", normalized_root, region_key)
}

/// 统一生成“游戏写锁 region scope”：
/// - 优先使用 biz_prefix（HoYoverse 等同 API 多区服场景）
/// - 其次使用 launcher_api（显式 host/query 规则 + 稳定指纹）
/// - 最后回退到 region_hint / default
pub fn derive_region_scope(
    launcher_api: Option<&str>,
    biz_prefix: Option<&str>,
    region_hint: Option<&str>,
) -> String {
    if let Some(scope) = normalize_scope_value(biz_prefix) {
        return DerivedRegionScope::BizPrefix(scope).into_scope_key();
    }

    if let Some(api_raw) = launcher_api
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return derive_scope_from_launcher_api(api_raw).into_scope_key();
    }

    if let Some(scope) = normalize_scope_value(region_hint) {
        return DerivedRegionScope::RegionHint(scope).into_scope_key();
    }

    DerivedRegionScope::Default.into_scope_key()
}

pub async fn is_game_running(game_name: &str) -> bool {
    let Some(process) = read_running_game(game_name) else {
        return false;
    };
    has_related_game_processes(&process).await
}

pub async fn register_game_process(game_name: String, pid: u32, exe_path: String) {
    let exe_name = normalize_process_target_name(&exe_path);
    let root_start_ticks = read_process_start_ticks(pid);
    if let Ok(mut state) = PROCESS_MONITOR_STATE.lock() {
        state.running_games.insert(
            game_name.clone(),
            GameProcess {
                pid,
                exe_name,
                launch_exe_path: exe_path.clone(),
                root_start_ticks,
            },
        );
        info!(
            "已注册游戏进程: {} (PID: {}, EXE: {}, start_ticks={:?})",
            game_name, pid, exe_path, root_start_ticks
        );
    }
}

pub async fn unregister_game_process(game_name: &str) {
    if let Ok(mut state) = PROCESS_MONITOR_STATE.lock() {
        if state.running_games.remove(game_name).is_some() {
            info!("已注销游戏进程: {}", game_name);
        }
    }
}

#[cfg(test)]
pub async fn find_game_processes(game_exe_name: &str, launch_exe_path: Option<&str>) -> Vec<u32> {
    let criteria = ProcessMatchCriteria {
        expected_exe_name: normalize_process_target_name(game_exe_name),
        expected_launch_path: normalize_process_target_path(launch_exe_path.unwrap_or_default()),
        root_pid: None,
        min_start_ticks: None,
    };

    tokio::task::spawn_blocking(move || scan_matching_processes(&criteria))
        .await
        .unwrap_or_default()
}

pub async fn find_related_game_processes(
    game_exe_name: &str,
    launch_exe_path: Option<&str>,
    root_pid: u32,
    root_start_ticks: Option<u64>,
) -> Vec<u32> {
    let criteria = ProcessMatchCriteria {
        expected_exe_name: normalize_process_target_name(game_exe_name),
        expected_launch_path: normalize_process_target_path(launch_exe_path.unwrap_or_default()),
        root_pid: (root_pid != 0).then_some(root_pid),
        min_start_ticks: root_start_ticks,
    };

    tokio::task::spawn_blocking(move || scan_matching_processes(&criteria))
        .await
        .unwrap_or_default()
}

async fn has_related_game_processes(process: &GameProcess) -> bool {
    let criteria = ProcessMatchCriteria::from_registered_process(process);
    !tokio::task::spawn_blocking(move || scan_matching_processes(&criteria))
        .await
        .unwrap_or_default()
        .is_empty()
}

pub fn process_start_ticks(pid: u32) -> Option<u64> {
    read_process_start_ticks(pid)
}

fn normalize_process_target_name(raw: &str) -> String {
    raw.rsplit(['/', '\\'])
        .next()
        .unwrap_or(raw)
        .trim()
        .to_ascii_lowercase()
}

fn normalize_process_target_path(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed.replace('\\', "/").to_ascii_lowercase())
}

fn normalize_process_token(raw: &str) -> String {
    raw.trim().replace('\\', "/").to_ascii_lowercase()
}

fn process_arg_basename(arg: &str) -> String {
    arg.rsplit(['/', '\\'])
        .next()
        .unwrap_or(arg)
        .trim()
        .to_ascii_lowercase()
}

fn process_arg_matches_target(
    arg: &str,
    expected_exe_name: &str,
    expected_launch_path: Option<&str>,
) -> bool {
    let normalized_arg = normalize_process_token(arg);
    if normalized_arg.is_empty() {
        return false;
    }

    if let Some(expected_launch_path) = expected_launch_path {
        if normalized_arg == expected_launch_path {
            return true;
        }
    }

    if !normalized_arg.ends_with(".exe") {
        return false;
    }

    process_arg_basename(&normalized_arg) == expected_exe_name
}

fn read_process_stat(pid: u32) -> Option<(u32, u64)> {
    let stat =
        std::fs::read_to_string(Path::new("/proc").join(pid.to_string()).join("stat")).ok()?;
    let after_comm = stat.rsplit_once(") ")?.1;
    let fields: Vec<&str> = after_comm.split_whitespace().collect();
    let ppid = fields.get(1)?.parse::<u32>().ok()?;
    let start_ticks = fields.get(19)?.parse::<u64>().ok()?;
    Some((ppid, start_ticks))
}

fn read_process_start_ticks(pid: u32) -> Option<u64> {
    if pid == 0 {
        return None;
    }
    read_process_stat(pid).map(|(_, start_ticks)| start_ticks)
}

fn snapshot_process_entry(pid: u32) -> Option<ProcessSnapshotEntry> {
    let proc_root = Path::new("/proc").join(pid.to_string());
    let (ppid, start_ticks) = read_process_stat(pid)?;
    let exe_path = std::fs::read_link(proc_root.join("exe"))
        .ok()
        .map(|path| normalize_process_token(&path.to_string_lossy()));
    let comm_name = std::fs::read_to_string(proc_root.join("comm"))
        .ok()
        .map(|comm| normalize_process_target_name(&comm));
    let cmd_args = std::fs::read(proc_root.join("cmdline"))
        .ok()
        .map(|cmdline| {
            cmdline
                .split(|byte| *byte == 0)
                .filter_map(|segment| std::str::from_utf8(segment).ok())
                .map(normalize_process_token)
                .filter(|segment| !segment.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Some(ProcessSnapshotEntry {
        pid,
        ppid,
        start_ticks: Some(start_ticks),
        exe_path,
        comm_name,
        cmd_args,
    })
}

fn snapshot_processes() -> HashMap<u32, ProcessSnapshotEntry> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return HashMap::new();
    };

    entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter_map(|name| name.parse::<u32>().ok())
        .filter_map(|pid| snapshot_process_entry(pid).map(|entry| (pid, entry)))
        .collect()
}

fn is_descendant_process(
    pid: u32,
    root_pid: u32,
    processes: &HashMap<u32, ProcessSnapshotEntry>,
) -> bool {
    if pid == root_pid {
        return true;
    }

    let mut current = pid;
    let mut guard = 0usize;
    while guard < 256 {
        let Some(entry) = processes.get(&current) else {
            return false;
        };
        if entry.ppid == root_pid {
            return true;
        }
        if entry.ppid == 0 || entry.ppid == current {
            return false;
        }
        current = entry.ppid;
        guard += 1;
    }
    false
}

fn process_entry_matches_target(
    entry: &ProcessSnapshotEntry,
    criteria: &ProcessMatchCriteria,
    processes: &HashMap<u32, ProcessSnapshotEntry>,
) -> bool {
    if criteria.expected_exe_name.trim().is_empty() {
        return false;
    }

    let descendant_match = criteria
        .root_pid
        .map(|root_pid| is_descendant_process(entry.pid, root_pid, processes))
        .unwrap_or(false);
    let fresh_enough = criteria
        .min_start_ticks
        .map(|min| entry.start_ticks.is_some_and(|start| start >= min))
        .unwrap_or(false);

    let path_match = criteria
        .expected_launch_path
        .as_deref()
        .is_some_and(|expected| {
            entry.exe_path.as_deref() == Some(expected)
                || entry.cmd_args.iter().any(|arg| {
                    process_arg_matches_target(arg, &criteria.expected_exe_name, Some(expected))
                })
        });

    if path_match {
        return descendant_match || fresh_enough || criteria.root_pid.is_none();
    }

    let name_match = entry
        .exe_path
        .as_deref()
        .map(normalize_process_target_name)
        .is_some_and(|name| name == criteria.expected_exe_name)
        || entry
            .comm_name
            .as_deref()
            .is_some_and(|name| name == criteria.expected_exe_name)
        || entry
            .cmd_args
            .iter()
            .any(|arg| process_arg_matches_target(arg, &criteria.expected_exe_name, None));

    if !name_match {
        return false;
    }

    descendant_match || fresh_enough || criteria.root_pid.is_none()
}

fn scan_matching_processes(criteria: &ProcessMatchCriteria) -> Vec<u32> {
    if criteria.expected_exe_name.trim().is_empty() {
        return Vec::new();
    }

    let processes = snapshot_processes();
    processes
        .iter()
        .filter_map(|(pid, entry)| {
            process_entry_matches_target(entry, criteria, &processes).then_some(*pid)
        })
        .collect()
}

pub async fn cleanup_stale_processes() {
    let tracked_games = snapshot_running_games();
    if tracked_games.is_empty() {
        return;
    }

    let mut stale_games = Vec::new();
    for (game_name, process) in tracked_games {
        if !has_related_game_processes(&process).await {
            stale_games.push(game_name);
        }
    }

    if stale_games.is_empty() {
        return;
    }

    if let Ok(mut state) = PROCESS_MONITOR_STATE.lock() {
        for game_name in stale_games {
            if state.running_games.remove(&game_name).is_some() {
                info!("清理已结束的游戏进程记录: {}", game_name);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        acquire_game_write_guard, acquire_launch_guard, cleanup_stale_processes,
        derive_region_scope, derive_scope_from_launcher_api, find_game_processes,
        is_descendant_process, make_game_write_scope_key, normalize_scope_value,
        parse_launcher_api, process_arg_matches_target, process_entry_matches_target,
        read_running_game, register_game_process, unregister_game_process, DerivedRegionScope,
        ProcessMatchCriteria, ProcessSnapshotEntry,
    };
    use once_cell::sync::Lazy;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_GUARD: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    #[test]
    fn derive_region_scope_prefers_biz_prefix() {
        let scope = derive_region_scope(
            Some("https://launcher.hoyoverse.com/game"),
            Some("hkrpg_global"),
            Some("cn"),
        );
        assert_eq!(scope, "hkrpg_global");
    }

    #[test]
    fn derive_region_scope_maps_known_hosts() {
        let cn = derive_region_scope(
            Some("https://launcher.hypergryph.com/api/launcher/get_latest_launcher"),
            None,
            None,
        );
        assert_eq!(cn, "cn");

        let global = derive_region_scope(
            Some("https://launcher.gryphline.com/api/launcher/get_latest_launcher"),
            None,
            None,
        );
        assert_eq!(global, "global");
    }

    #[test]
    fn derive_region_scope_maps_known_channel_query_values() {
        let cn = derive_region_scope(
            Some("https://example.com/api/launcher/get_latest_launcher?channel=1&sub_channel=1"),
            None,
            None,
        );
        assert_eq!(cn, "cn");

        let global = derive_region_scope(
            Some("https://example.com/api/launcher/get_latest_launcher?channel=6&sub_channel=6"),
            None,
            None,
        );
        assert_eq!(global, "global");
    }

    #[test]
    fn derive_region_scope_uses_region_hint_when_sources_missing() {
        let scope = derive_region_scope(None, None, Some("Global"));
        assert_eq!(scope, "global");
    }

    #[test]
    fn derive_region_scope_fingerprints_unknown_launcher_api() {
        let scope = derive_region_scope(
            Some("https://launcher.example.com/builds/latest"),
            None,
            None,
        );
        assert!(scope.starts_with("source_"));
    }

    #[test]
    fn derive_region_scope_supports_custom_snowbreak_scheme() {
        let scope = derive_region_scope(Some("snowbreak://launcher"), None, None);
        assert_eq!(scope, "global");
    }

    #[test]
    fn derive_region_scope_falls_back_to_default_when_inputs_are_blank() {
        let scope = derive_region_scope(Some("   "), Some("  "), Some("\n\t"));
        assert_eq!(scope, "default");
    }

    #[test]
    fn launcher_scope_parser_returns_fingerprint_for_unmapped_input() {
        let derived = derive_scope_from_launcher_api("https://launcher.example.com/builds/latest");
        match derived {
            DerivedRegionScope::LauncherApiFingerprint(scope) => {
                assert!(scope.starts_with("source_"));
            }
            other => panic!("unexpected scope: {:?}", other),
        }
    }

    #[test]
    fn normalize_scope_value_trims_lowercases_and_drops_empty() {
        assert_eq!(
            normalize_scope_value(Some("  HKRPG_Global  ")),
            Some("hkrpg_global".to_string())
        );
        assert_eq!(normalize_scope_value(Some("   ")), None);
        assert_eq!(normalize_scope_value(None), None);
    }

    #[test]
    fn parse_launcher_api_normalizes_host_path_and_query_pairs() {
        let parsed =
            parse_launcher_api("https://Launcher.Example.com/API/Game?Channel=6&Sub_Channel=1")
                .unwrap();

        assert_eq!(parsed.scheme, "https");
        assert_eq!(parsed.host, "launcher.example.com");
        assert_eq!(parsed.path, "/api/game");
        assert_eq!(parsed.query.get("channel").map(String::as_str), Some("6"));
        assert_eq!(
            parsed.query.get("sub_channel").map(String::as_str),
            Some("1")
        );
    }

    #[test]
    fn make_game_write_scope_key_normalizes_region() {
        let unique = format!(
            "ssmt4-process-monitor-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&root).unwrap();

        let scope = make_game_write_scope_key(&root, "  Global  ");
        assert!(scope.ends_with("::global"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn launch_and_write_guards_block_reentry_until_drop() {
        let unique = format!(
            "ssmt4-process-guard-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let game_name = format!("Game-{}", unique);
        let root = std::env::temp_dir().join(&unique);
        std::fs::create_dir_all(&root).unwrap();

        let launch_guard = acquire_launch_guard(&game_name).unwrap();
        assert!(acquire_launch_guard(&game_name).is_err());
        drop(launch_guard);
        assert!(acquire_launch_guard(&game_name).is_ok());

        let write_guard = acquire_game_write_guard(&root, "Global", "test-op").unwrap();
        assert!(acquire_game_write_guard(&root, "global", "test-op").is_err());
        drop(write_guard);
        assert!(acquire_game_write_guard(&root, "global", "test-op").is_ok());

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn cleanup_stale_processes_removes_dead_registered_games() {
        let _guard = TEST_GUARD.lock().unwrap();
        let game_name = format!(
            "DeadGame-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );

        unregister_game_process(&game_name).await;
        register_game_process(game_name.clone(), 0, "/tmp/Game.exe".to_string()).await;
        assert_eq!(
            read_running_game(&game_name).map(|process| process.pid),
            Some(0)
        );

        cleanup_stale_processes().await;
        assert_eq!(
            read_running_game(&game_name).map(|process| process.pid),
            None
        );
    }

    #[tokio::test]
    async fn find_game_processes_returns_empty_for_unknown_executable() {
        let pids = find_game_processes("ssmt4-non-existent-process-for-test.exe", None).await;
        assert!(pids.is_empty());
    }

    #[test]
    fn process_arg_matching_uses_exact_executable_token_not_substring() {
        assert!(process_arg_matches_target(
            "/games/Wuthering Waves Game/Wuthering Waves.exe",
            "wuthering waves.exe",
            Some("/games/wuthering waves game/wuthering waves.exe"),
        ));
        assert!(process_arg_matches_target(
            "Z:\\games\\Wuthering Waves Game\\Wuthering Waves.exe",
            "wuthering waves.exe",
            None,
        ));
        assert!(!process_arg_matches_target(
            "/tmp/logs/wuthering waves.exe.log",
            "wuthering waves.exe",
            None,
        ));
        assert!(!process_arg_matches_target(
            "--title=Wuthering Waves.exe helper",
            "wuthering waves.exe",
            None,
        ));
    }

    #[test]
    fn descendant_process_matching_accepts_name_match_inside_launch_tree() {
        let root_pid = 1000;
        let child_pid = 1001;
        let mut processes = HashMap::new();
        processes.insert(
            root_pid,
            ProcessSnapshotEntry {
                pid: root_pid,
                ppid: 1,
                start_ticks: Some(500),
                exe_path: Some("/usr/bin/umu-run".to_string()),
                comm_name: Some("umu-run".to_string()),
                cmd_args: vec!["/usr/bin/umu-run".to_string()],
            },
        );
        processes.insert(
            child_pid,
            ProcessSnapshotEntry {
                pid: child_pid,
                ppid: root_pid,
                start_ticks: Some(510),
                exe_path: Some("/games/helper.exe".to_string()),
                comm_name: Some("game.exe".to_string()),
                cmd_args: vec!["/games/helper.exe".to_string()],
            },
        );

        let criteria = ProcessMatchCriteria {
            expected_exe_name: "game.exe".to_string(),
            expected_launch_path: Some("/games/game.exe".to_string()),
            root_pid: Some(root_pid),
            min_start_ticks: Some(500),
        };

        assert!(is_descendant_process(child_pid, root_pid, &processes));
        assert!(process_entry_matches_target(
            processes.get(&child_pid).unwrap(),
            &criteria,
            &processes
        ));
    }

    #[test]
    fn stale_same_name_process_is_rejected_without_tree_or_time_match() {
        let root_pid = 2000;
        let stale_pid = 2001;
        let mut processes = HashMap::new();
        processes.insert(
            root_pid,
            ProcessSnapshotEntry {
                pid: root_pid,
                ppid: 1,
                start_ticks: Some(500),
                exe_path: Some("/usr/bin/umu-run".to_string()),
                comm_name: Some("umu-run".to_string()),
                cmd_args: vec!["/usr/bin/umu-run".to_string()],
            },
        );
        processes.insert(
            stale_pid,
            ProcessSnapshotEntry {
                pid: stale_pid,
                ppid: 1,
                start_ticks: Some(400),
                exe_path: Some("/games/game.exe".to_string()),
                comm_name: Some("game.exe".to_string()),
                cmd_args: vec!["/games/game.exe".to_string()],
            },
        );

        let criteria = ProcessMatchCriteria {
            expected_exe_name: "game.exe".to_string(),
            expected_launch_path: Some("/games/game.exe".to_string()),
            root_pid: Some(root_pid),
            min_start_ticks: Some(500),
        };

        assert!(!process_entry_matches_target(
            processes.get(&stale_pid).unwrap(),
            &criteria,
            &processes
        ));
    }
}
