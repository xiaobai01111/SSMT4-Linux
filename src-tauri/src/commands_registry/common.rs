use crate::commands;

const COMMANDS: &[&str] = &[
    "greet",
    "get_resource_path",
    "ensure_directory",
    "path_exists",
    "get_app_data_dir_path",
    "open_in_explorer",
    "mark_startup_ready",
    #[cfg(feature = "devtools")]
    "toggle_devtools",
];

pub fn matches(command: &str) -> bool {
    COMMANDS.contains(&command)
}

pub fn handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        commands::common::greet,
        commands::common::get_resource_path,
        commands::common::ensure_directory,
        commands::common::path_exists,
        commands::common::get_app_data_dir_path,
        commands::common::open_in_explorer,
        commands::common::mark_startup_ready,
        #[cfg(feature = "devtools")]
        commands::common::toggle_devtools,
    ]
}
