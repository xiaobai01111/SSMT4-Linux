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

#[cfg(test)]
pub fn clear_runtime_log_for_test() {
    RUNTIME_LOG_LINES.lock().unwrap().clear();
}

#[cfg(test)]
mod tests {
    use super::{append_runtime_log_line, clear_runtime_log_for_test, read_runtime_log_text};
    use once_cell::sync::Lazy;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_GUARD: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    fn unique_tag(label: &str) -> String {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        format!("runtime-log-{label}-{nonce}")
    }

    #[test]
    fn append_runtime_log_line_ignores_empty_lines_and_preserves_order() {
        let _guard = TEST_GUARD.lock().unwrap();
        clear_runtime_log_for_test();
        let line1 = unique_tag("line1");
        let line2 = unique_tag("line2");

        append_runtime_log_line("");
        append_runtime_log_line(&format!("{line1}\n"));
        append_runtime_log_line(&format!("{line2}\r\n"));

        let snapshot = read_runtime_log_text(2);
        assert!(snapshot.ends_with(&format!("{line1}\n{line2}")));
    }

    #[test]
    fn read_runtime_log_text_clamps_requested_line_count() {
        let _guard = TEST_GUARD.lock().unwrap();
        clear_runtime_log_for_test();
        let line = unique_tag("clamp");
        append_runtime_log_line(&line);

        let snapshot = read_runtime_log_text(0);
        assert!(snapshot.ends_with(&line));
        assert!(!snapshot.is_empty());
    }
}
