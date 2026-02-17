use std::collections::{HashMap, HashSet};
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
