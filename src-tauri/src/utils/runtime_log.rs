use once_cell::sync::Lazy;
use std::collections::VecDeque;
use std::sync::Mutex;

const RUNTIME_LOG_MAX_LINES: usize = 12_000;
const RUNTIME_LOG_READ_CAP: usize = 20_000;

static RUNTIME_LOG_LINES: Lazy<Mutex<VecDeque<String>>> =
    Lazy::new(|| Mutex::new(VecDeque::with_capacity(2_048)));

pub fn append_runtime_log_line(line: &str) {
    let line = line.trim_end_matches(['\n', '\r']);
    if line.is_empty() {
        return;
    }

    let mut guard = RUNTIME_LOG_LINES.lock().unwrap();
    if guard.len() >= RUNTIME_LOG_MAX_LINES {
        guard.pop_front();
    }
    guard.push_back(line.to_string());
}

pub fn read_runtime_log_text(max_lines: usize) -> String {
    let max = max_lines.clamp(1, RUNTIME_LOG_READ_CAP);
    let guard = RUNTIME_LOG_LINES.lock().unwrap();
    let all_lines: Vec<&str> = guard.iter().map(|s| s.as_str()).collect();
    let start = all_lines.len().saturating_sub(max);
    all_lines[start..].join("\n")
}
