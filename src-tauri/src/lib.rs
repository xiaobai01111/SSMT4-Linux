mod bootstrap;
mod commands;
mod commands_registry;
mod configs;
mod downloader;
<<<<<<< HEAD
mod process_monitor;
=======
>>>>>>> d458e2327e8b8895ae6f9c250c450772d6a0d6b1
mod utils;
mod wine;

use configs::app_config::AppConfig;
use std::sync::Mutex;
use tauri::Manager;

pub fn run() {
    // Initialize logger
    let log_dir = utils::file_manager::get_logs_dir();
    utils::logger::init_logger(&log_dir);

    tracing::info!("SSMT4 Linux starting...");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _: Result<(), _> = window.set_focus();
            }
        }))
        .manage(Mutex::new(AppConfig::default()))
        .setup(bootstrap::setup)
        .invoke_handler(commands_registry::handler())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
