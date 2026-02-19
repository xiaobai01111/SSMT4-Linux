use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

#[derive(Debug, Clone)]
pub struct GameProcess {
    pub game_name: String,
    pub pid: u32,
    pub exe_path: String,
}

lazy_static::lazy_static! {
    static ref RUNNING_GAMES: Arc<Mutex<HashMap<String, GameProcess>>> = Arc::new(Mutex::new(HashMap::new()));
    static ref LAUNCHING_GAMES: std::sync::Mutex<HashSet<String>> = std::sync::Mutex::new(HashSet::new());
    static ref GAME_WRITE_SCOPES: std::sync::Mutex<HashSet<String>> = std::sync::Mutex::new(HashSet::new());
}

#[derive(Debug)]
pub struct LaunchGuard {
    game_name: String,
}

impl Drop for LaunchGuard {
    fn drop(&mut self) {
        if let Ok(mut launching) = LAUNCHING_GAMES.lock() {
            launching.remove(&self.game_name);
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
        if let Ok(mut scopes) = GAME_WRITE_SCOPES.lock() {
            scopes.remove(&self.scope_key);
            info!("已释放游戏写操作锁: {}", self.scope_key);
        }
    }
}

pub fn acquire_launch_guard(game_name: &str) -> Result<LaunchGuard, String> {
    let mut launching = LAUNCHING_GAMES
        .lock()
        .map_err(|_| "获取游戏启动锁失败".to_string())?;
    if launching.contains(game_name) {
        return Err("游戏正在启动中，请勿重复点击".to_string());
    }
    launching.insert(game_name.to_string());
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
    let mut scopes = GAME_WRITE_SCOPES
        .lock()
        .map_err(|_| "获取游戏写操作锁失败".to_string())?;

    if scopes.contains(&scope_key) {
        return Err(format!(
            "当前目录与区服已有写操作进行中，请稍后重试（scope: {}）",
            scope_key
        ));
    }

    scopes.insert(scope_key.clone());
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
/// - 其次使用 launcher_api（内置 CN/Global 推导，未知 API 走稳定 hash）
/// - 最后回退到 region_hint / default
pub fn derive_region_scope(
    launcher_api: Option<&str>,
    biz_prefix: Option<&str>,
    region_hint: Option<&str>,
) -> String {
    if let Some(biz) = biz_prefix
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_ascii_lowercase())
    {
        return biz;
    }

    if let Some(api_raw) = launcher_api.map(str::trim).filter(|v| !v.is_empty()) {
        let api = api_raw.to_ascii_lowercase();
        if api.contains("mihoyo.com")
            || api.contains("hypergryph.com")
            || api.contains("prod-cn-")
            || api.contains("channel=1")
            || api.contains("sub_channel=1")
        {
            return "cn".to_string();
        }
        if api.contains("hoyoverse.com")
            || api.contains("gryphline.com")
            || api.contains("prod-alicdn-")
            || api.contains("channel=6")
            || api.contains("sub_channel=6")
        {
            return "global".to_string();
        }
        if api.starts_with("snowbreak://") {
            return "global".to_string();
        }

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        api.hash(&mut hasher);
        return format!("source_{:x}", hasher.finish());
    }

    if let Some(region) = region_hint
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_ascii_lowercase())
    {
        return region;
    }

    "default".to_string()
}

#[cfg(test)]
mod tests {
    use super::derive_region_scope;

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
    fn derive_region_scope_maps_cn_and_global_launcher_api() {
        let cn = derive_region_scope(
            Some(
                "https://launcher.hypergryph.com/api/launcher/get_latest_launcher?channel=1&sub_channel=1",
            ),
            None,
            None,
        );
        assert_eq!(cn, "cn");

        let global = derive_region_scope(
            Some(
                "https://launcher.gryphline.com/api/launcher/get_latest_launcher?channel=6&sub_channel=6",
            ),
            None,
            None,
        );
        assert_eq!(global, "global");
    }

    #[test]
    fn derive_region_scope_falls_back_to_region_hint_when_api_missing() {
        let scope = derive_region_scope(None, None, Some("Global"));
        assert_eq!(scope, "global");
    }
}

pub async fn is_game_running(game_name: &str) -> bool {
    let games = RUNNING_GAMES.lock().await;
    if let Some(proc) = games.get(game_name) {
        if is_process_alive(proc.pid).await {
            return true;
        }
    }
    false
}

pub async fn register_game_process(game_name: String, pid: u32, exe_path: String) {
    let mut games = RUNNING_GAMES.lock().await;
    let proc = GameProcess {
        game_name: game_name.clone(),
        pid,
        exe_path,
    };
    games.insert(game_name.clone(), proc);
    info!("已注册游戏进程: {} (PID: {})", game_name, pid);
}

pub async fn unregister_game_process(game_name: &str) {
    let mut games = RUNNING_GAMES.lock().await;
    if games.remove(game_name).is_some() {
        info!("已注销游戏进程: {}", game_name);
    }
}

pub async fn is_process_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    
    let output = tokio::process::Command::new("ps")
        .arg("-p")
        .arg(pid.to_string())
        .arg("-o")
        .arg("pid=")
        .output()
        .await;
    
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout.trim().parse::<u32>().ok() == Some(pid)
        }
        Err(_) => false,
    }
}

pub async fn find_game_processes(game_exe_name: &str) -> Vec<u32> {
    let exe_name = Path::new(game_exe_name)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(game_exe_name);
    
    let output = tokio::process::Command::new("pgrep")
        .arg("-f")
        .arg(exe_name)
        .output()
        .await;
    
    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout
                .lines()
                .filter_map(|line| line.trim().parse::<u32>().ok())
                .collect()
        }
        _ => Vec::new(),
    }
}

pub async fn cleanup_stale_processes() {
    let mut games = RUNNING_GAMES.lock().await;
    let mut to_remove = Vec::new();
    
    for (game_name, proc) in games.iter() {
        if !is_process_alive(proc.pid).await {
            to_remove.push(game_name.clone());
        }
    }
    
    for game_name in to_remove {
        games.remove(&game_name);
        info!("清理已结束的游戏进程记录: {}", game_name);
    }
}
