use once_cell::sync::Lazy;
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::Mutex;
use tauri::Manager;

const GAME_LOG_WINDOW_LABEL: &str = "game-log-viewer";
const GAME_LOG_MAX_LINES: usize = 8_000;
const GAME_LOG_READ_CAP: usize = 20_000;

#[derive(Debug, Clone)]
struct GameLogSession {
    game_name: String,
    started_at: String,
    lines: VecDeque<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameLogSnapshot {
    pub active: bool,
    pub game_name: String,
    pub started_at: String,
    pub line_count: usize,
    pub content: String,
}

static GAME_LOG_SESSION: Lazy<Mutex<Option<GameLogSession>>> = Lazy::new(|| Mutex::new(None));

fn now_string() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string()
}

fn canonical_game_name(raw: &str) -> String {
    crate::configs::game_identity::to_canonical_or_keep(raw)
}

fn format_log_line(level: &str, source: &str, message: &str) -> String {
    let normalized = message.replace('\n', "\\n");
    format!("{} [{}] [{}] {}", now_string(), level, source, normalized)
}

fn push_line(session: &mut GameLogSession, level: &str, source: &str, message: &str) {
    if session.lines.len() >= GAME_LOG_MAX_LINES {
        session.lines.pop_front();
    }
    session
        .lines
        .push_back(format_log_line(level, source, message));
}

pub fn start_game_log_session(game_name: &str) {
    let game_name = canonical_game_name(game_name);
    let mut session = GameLogSession {
        game_name: game_name.clone(),
        started_at: now_string(),
        lines: VecDeque::with_capacity(512),
    };
    push_line(&mut session, "INFO", "session", &format!("日志会话已创建: {}", game_name));
    let mut guard = GAME_LOG_SESSION.lock().unwrap();
    *guard = Some(session);
}

pub fn ensure_game_log_session(game_name: &str) {
    let canonical = canonical_game_name(game_name);
    let mut guard = GAME_LOG_SESSION.lock().unwrap();
    if let Some(session) = guard.as_ref() {
        if session.game_name.eq_ignore_ascii_case(&canonical) {
            return;
        }
    }

    let mut session = GameLogSession {
        game_name: canonical.clone(),
        started_at: now_string(),
        lines: VecDeque::with_capacity(512),
    };
    push_line(&mut session, "INFO", "session", &format!("日志会话已创建: {}", canonical));
    *guard = Some(session);
}

pub fn clear_game_log_session() {
    let mut guard = GAME_LOG_SESSION.lock().unwrap();
    *guard = None;
}

pub fn append_game_log_line(game_name: &str, level: &str, source: &str, message: &str) {
    let canonical = canonical_game_name(game_name);
    let mut guard = GAME_LOG_SESSION.lock().unwrap();
    if let Some(session) = guard.as_mut() {
        if session.game_name.eq_ignore_ascii_case(&canonical) {
            push_line(session, level, source, message);
        }
    }
}

#[tauri::command]
pub fn read_game_log_snapshot(max_lines: Option<usize>) -> GameLogSnapshot {
    let max = max_lines.unwrap_or(2_000).clamp(1, GAME_LOG_READ_CAP);
    let guard = GAME_LOG_SESSION.lock().unwrap();
    if let Some(session) = guard.as_ref() {
        let all_lines: Vec<&str> = session.lines.iter().map(|s| s.as_str()).collect();
        let start = all_lines.len().saturating_sub(max);
        return GameLogSnapshot {
            active: true,
            game_name: session.game_name.clone(),
            started_at: session.started_at.clone(),
            line_count: session.lines.len(),
            content: all_lines[start..].join("\n"),
        };
    }

    GameLogSnapshot {
        active: false,
        game_name: String::new(),
        started_at: String::new(),
        line_count: 0,
        content: "日志会话未开启。请在主页快速设置中点击“打开游戏日志窗口”。".to_string(),
    }
}

#[tauri::command]
pub async fn open_game_log_window(app: tauri::AppHandle, game_name: String) -> Result<(), String> {
    let canonical = canonical_game_name(&game_name);
    if canonical.trim().is_empty() || canonical == "Default" {
        return Err("请先选择有效的游戏配置".to_string());
    }

    start_game_log_session(&canonical);

    let window_title = format!("{} - 游戏日志", canonical);
    if let Some(window) = app.get_webview_window(GAME_LOG_WINDOW_LABEL) {
        window
            .set_title(&window_title)
            .map_err(|e| format!("更新日志窗口标题失败: {}", e))?;
        window
            .set_focus()
            .map_err(|e| format!("聚焦日志窗口失败: {}", e))?;
        return Ok(());
    }

    let window = tauri::WebviewWindowBuilder::new(
        &app,
        GAME_LOG_WINDOW_LABEL,
        tauri::WebviewUrl::App("/game-log-viewer".into()),
    )
    .title(&window_title)
    .inner_size(980.0, 660.0)
    .min_inner_size(680.0, 420.0)
    .center()
    .build()
    .map_err(|e| format!("创建游戏日志窗口失败: {}", e))?;

    window.on_window_event(|event| {
        if matches!(
            event,
            tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed
        ) {
            clear_game_log_session();
        }
    });

    Ok(())
}
