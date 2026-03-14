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
    chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S%.3f")
        .to_string()
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
    push_line(
        &mut session,
        "INFO",
        "session",
        &format!("日志会话已创建: {}", canonical),
    );
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

    ensure_game_log_session(&canonical);

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

#[cfg(test)]
mod tests {
    use super::{
        append_game_log_line, clear_game_log_session, ensure_game_log_session, format_log_line,
        read_game_log_snapshot,
    };
    use once_cell::sync::Lazy;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_GUARD: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    fn unique_game_name(label: &str) -> String {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        format!("GameLogTest{label}{nonce}")
    }

    #[test]
    fn read_game_log_snapshot_returns_inactive_message_without_session() {
        let _guard = TEST_GUARD.lock().unwrap();
        clear_game_log_session();

        let snapshot = read_game_log_snapshot(None);
        assert!(!snapshot.active);
        assert_eq!(snapshot.line_count, 0);
        assert!(snapshot.content.contains("日志会话未开启"));
    }

    #[test]
    fn game_log_session_appends_only_matching_game_lines() {
        let _guard = TEST_GUARD.lock().unwrap();
        clear_game_log_session();

        let game = unique_game_name("Primary");
        ensure_game_log_session(&game);
        append_game_log_line(&game, "INFO", "launcher", "hello");
        append_game_log_line("OtherGame", "WARN", "launcher", "ignored");

        let snapshot = read_game_log_snapshot(Some(10));
        assert!(snapshot.active);
        assert_eq!(snapshot.game_name, game);
        assert_eq!(snapshot.line_count, 2);
        assert!(snapshot.content.contains("日志会话已创建"));
        assert!(snapshot.content.contains("[INFO] [launcher] hello"));
        assert!(!snapshot.content.contains("ignored"));

        clear_game_log_session();
    }

    #[test]
    fn ensure_game_log_session_reuses_matching_session_and_snapshot_is_clamped() {
        let _guard = TEST_GUARD.lock().unwrap();
        clear_game_log_session();

        let game = unique_game_name("Clamp");
        ensure_game_log_session(&game);
        append_game_log_line(&game, "INFO", "bridge", "line-1");
        append_game_log_line(&game, "INFO", "bridge", "line-2");

        let snapshot = read_game_log_snapshot(Some(1));
        assert!(snapshot.content.contains("line-2"));
        assert!(!snapshot.content.contains("line-1"));

        clear_game_log_session();
    }

    #[test]
    fn ensure_game_log_session_switches_to_new_game_and_drops_old_lines() {
        let _guard = TEST_GUARD.lock().unwrap();
        clear_game_log_session();

        let game_a = unique_game_name("SwitchA");
        let game_b = unique_game_name("SwitchB");

        ensure_game_log_session(&game_a);
        append_game_log_line(&game_a, "INFO", "launcher", "from-a");
        ensure_game_log_session(&game_b);
        append_game_log_line(&game_b, "INFO", "launcher", "from-b");

        let snapshot = read_game_log_snapshot(Some(10));
        assert!(snapshot.active);
        assert_eq!(snapshot.game_name, game_b);
        assert!(snapshot.content.contains("from-b"));
        assert!(!snapshot.content.contains("from-a"));

        clear_game_log_session();
    }

    #[test]
    fn format_log_line_escapes_newlines_in_message() {
        let line = format_log_line("WARN", "bridge", "line1\nline2");
        assert!(line.contains("[WARN] [bridge] line1\\nline2"));
    }
}
