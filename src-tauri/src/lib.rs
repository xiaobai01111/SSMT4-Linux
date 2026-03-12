mod bootstrap;
mod commands;
mod commands_registry;
mod configs;
mod downloader;
mod events;
mod process_monitor;
mod services;
mod utils;
mod wine;

use configs::app_config::AppConfig;
use std::sync::Mutex;
use tauri::Manager;

pub fn run() {
    // 修复 NVIDIA + Wayland 下 WebKitGTK DMABUF 渲染器导致的闪退/黑屏
    // 参考: https://github.com/nicbarker/clay/issues/145
    if std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }

    // Initialize logger
    let log_dir = utils::file_manager::get_logs_dir();
    utils::logger::init_logger(&log_dir);

    tracing::info!("SSMT4 Linux 启动中...");

    let initial_config = commands::settings::bootstrap_runtime_app_config()
        .unwrap_or_else(|err| panic!("failed to initialize app config: {err}"));

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
        .manage(Mutex::new(initial_config))
        .setup(bootstrap::setup)
        .invoke_handler(commands_registry::handler())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
