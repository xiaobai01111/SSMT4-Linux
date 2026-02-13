mod commands;
mod configs;
mod downloader;
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
        .manage(Mutex::new(commands::mod_manager::ModWatcher::default()))
        .setup(|_app| {
            // Ensure config directories exist
            let config_dir = configs::app_config::get_app_config_dir();
            let data_dir = configs::app_config::get_app_data_dir();
            let cache_dir = configs::app_config::get_app_cache_dir();
            let games_dir = utils::file_manager::get_global_games_dir();
            let prefixes_dir = utils::file_manager::get_prefixes_dir();

            for dir in [&config_dir, &data_dir, &cache_dir, &games_dir, &prefixes_dir] {
                utils::file_manager::ensure_dir(dir).ok();
            }

            tracing::info!("Config dir: {}", config_dir.display());
            tracing::info!("Data dir: {}", data_dir.display());
            tracing::info!("Cache dir: {}", cache_dir.display());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Common
            commands::common::greet,
            commands::common::get_resource_path,
            commands::common::ensure_directory,
            commands::common::open_in_explorer,
            // Settings
            commands::settings::load_settings,
            commands::settings::save_settings,
            // Process
            commands::process::run_resource_executable,
            // Game scanner
            commands::game_scanner::scan_games,
            commands::game_scanner::set_game_visibility,
            // Game config
            commands::game_config::load_game_config,
            commands::game_config::save_game_config,
            commands::game_config::create_new_config,
            commands::game_config::delete_game_config_folder,
            commands::game_config::set_game_icon,
            commands::game_config::set_game_background,
            commands::game_config::update_game_background,
            commands::game_config::get_3dmigoto_latest_release,
            commands::game_config::install_3dmigoto_update,
            // Game launcher (pressure-vessel + Wine/Proton)
            commands::game_launcher::start_game,
            commands::game_launcher::check_3dmigoto_integrity,
            commands::game_launcher::toggle_symlink,
            // Mod manager
            commands::mod_manager::scan_mods,
            commands::mod_manager::toggle_mod,
            commands::mod_manager::watch_mods,
            commands::mod_manager::unwatch_mods,
            commands::mod_manager::create_mod_group,
            commands::mod_manager::rename_mod_group,
            commands::mod_manager::delete_mod_group,
            commands::mod_manager::set_mod_group_icon,
            commands::mod_manager::delete_mod,
            commands::mod_manager::move_mod_to_group,
            commands::mod_manager::open_game_mods_folder,
            commands::mod_manager::open_mod_group_folder,
            commands::mod_manager::preview_mod_archive,
            commands::mod_manager::install_mod_archive,
            // Wine manager
            commands::wine_manager::scan_wine_versions,
            commands::wine_manager::get_game_wine_config,
            commands::wine_manager::set_game_wine_config,
            commands::wine_manager::create_prefix,
            commands::wine_manager::delete_prefix,
            commands::wine_manager::get_prefix_info,
            commands::wine_manager::install_dxvk,
            commands::wine_manager::uninstall_dxvk,
            commands::wine_manager::install_vkd3d,
            commands::wine_manager::check_vulkan,
            commands::wine_manager::install_runtime,
            commands::wine_manager::list_available_runtimes,
            commands::wine_manager::get_installed_runtimes,
            commands::wine_manager::get_display_info,
            commands::wine_manager::get_recent_logs,
            commands::wine_manager::open_log_folder,
            commands::wine_manager::list_prefix_templates,
            commands::wine_manager::save_prefix_template,
            // Game downloader
            commands::game_downloader::get_launcher_info,
            commands::game_downloader::get_game_state,
            commands::game_downloader::download_game,
            commands::game_downloader::update_game,
            commands::game_downloader::update_game_patch,
            commands::game_downloader::verify_game_files,
            commands::game_downloader::cancel_download,
            commands::game_downloader::get_local_version,
            commands::game_downloader::get_game_launcher_api,
            commands::game_downloader::get_default_game_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
