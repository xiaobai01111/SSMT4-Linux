use crate::commands;

const COMMANDS: &[&str] = &["get_log_dir", "read_log_file", "open_log_window"];

pub fn matches(command: &str) -> bool {
    COMMANDS.contains(&command)
}

pub fn handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        commands::log_viewer::get_log_dir,
        commands::log_viewer::read_log_file,
        commands::log_viewer::open_log_window,
    ]
}
