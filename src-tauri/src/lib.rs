mod bootstrap;
mod commands;
mod commands_registry;
mod configs;
mod downloader;
mod process_monitor;
mod utils;
mod wine;

use configs::app_config::AppConfig;
use std::sync::Mutex;
use tauri::Manager;

pub fn run() {
    run_with_args(std::env::args().collect());
}

pub fn run_with_args(args: Vec<String>) {
    if has_flag(&args, "--help") || has_flag(&args, "-h") {
        print_cli_help();
        return;
    }
    apply_cli_log_level_flags(&args);

    // Initialize logger
    let log_dir = utils::file_manager::get_logs_dir();
    utils::logger::init_logger(&log_dir);
    utils::logger::log_startup_context(&args);

    if has_flag(&args, "--diagnose") {
        utils::logger::log_runtime_dependency_diagnostics();
        tracing::info!("--diagnose finished, process exits without starting GUI");
        return;
    }

    tracing::info!("SSMT4-Linux starting...");

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

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

fn apply_cli_log_level_flags(args: &[String]) {
    if std::env::var("RUST_LOG").is_ok() {
        return;
    }

    if let Some(level) = args
        .iter()
        .find_map(|a| a.strip_prefix("--log-level="))
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        std::env::set_var("RUST_LOG", level);
        return;
    }

    if has_flag(args, "--trace") {
        std::env::set_var("RUST_LOG", "trace");
    } else if has_flag(args, "--verbose") {
        std::env::set_var("RUST_LOG", "debug");
    }
}

fn print_cli_help() {
    println!("SSMT4-Linux Launcher");
    println!();
    println!("Usage:");
    println!("  SSMT4-Linux [--verbose|--trace|--log-level=<level>] [--diagnose]");
    println!();
    println!("Options:");
    println!("  --verbose             Enable debug level logs (RUST_LOG=debug)");
    println!("  --trace               Enable trace level logs (RUST_LOG=trace)");
    println!("  --log-level=<level>   Custom log filter (e.g. info,ssmt4_lib=debug)");
    println!("  --diagnose            Print runtime dependency diagnostics and exit");
    println!("  -h, --help            Show this help");
}
