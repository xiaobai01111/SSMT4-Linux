mod commands;
mod configs;
mod downloader;
mod utils;
mod wine;

use configs::app_config::AppConfig;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use tauri::Manager;

/// 视频流服务器端口（启动后写入，前端读取）
static VIDEO_SERVER_PORT: AtomicU16 = AtomicU16::new(0);

#[tauri::command]
fn get_video_server_port() -> u16 {
    VIDEO_SERVER_PORT.load(Ordering::Relaxed)
}

/// 解析 HTTP Range 请求头，返回 (start, end) 字节范围
fn parse_range_header(range_str: &str, file_len: u64) -> Option<(u64, u64)> {
    // 格式: "bytes=start-end" 或 "bytes=start-" 或 "bytes=-suffix"
    let range = range_str.strip_prefix("bytes=")?;
    let parts: Vec<&str> = range.splitn(2, '-').collect();
    if parts.len() != 2 {
        return None;
    }
    if parts[0].is_empty() {
        // bytes=-500 (最后 500 字节)
        let suffix: u64 = parts[1].parse().ok()?;
        let start = file_len.saturating_sub(suffix);
        Some((start, file_len - 1))
    } else if parts[1].is_empty() {
        // bytes=500- (从 500 到末尾)
        let start: u64 = parts[0].parse().ok()?;
        Some((start, file_len - 1))
    } else {
        let start: u64 = parts[0].parse().ok()?;
        let end: u64 = parts[1].parse().ok()?;
        Some((start, end.min(file_len - 1)))
    }
}

/// 处理视频流 HTTP 连接（支持 keep-alive + Range 请求）
async fn handle_video_connection(stream: tokio::net::TcpStream) {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufReader};

    // 设置 TCP_NODELAY 减少延迟
    let _ = stream.set_nodelay(true);

    let (reader, mut writer) = tokio::io::split(stream);
    let mut reader = BufReader::new(reader);
    let mut header_buf = String::with_capacity(4096);

    // keep-alive 循环：同一连接处理多个请求
    loop {
        header_buf.clear();
        // 读取 HTTP 请求头（以空行结束）
        loop {
            let mut line = String::new();
            match tokio::time::timeout(
                std::time::Duration::from_secs(30),
                reader.read_line(&mut line),
            )
            .await
            {
                Ok(Ok(0)) => return, // 连接关闭
                Ok(Ok(_)) => {
                    let is_end = line == "\r\n" || line == "\n";
                    header_buf.push_str(&line);
                    if is_end {
                        break;
                    }
                }
                _ => return, // 超时或错误
            }
        }

        // 解析请求行: GET /path HTTP/1.1
        let request_line = match header_buf.lines().next() {
            Some(l) => l.to_string(),
            None => return,
        };
        let parts: Vec<&str> = request_line.split_whitespace().collect();
        if parts.len() < 2 {
            return;
        }

        let path = percent_encoding::percent_decode_str(parts[1].trim_start_matches('/'))
            .decode_utf8_lossy()
            .to_string();

        let file_path = std::path::PathBuf::from(&path);
        if !file_path.exists() {
            let resp =
                "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: keep-alive\r\n\r\n";
            if writer.write_all(resp.as_bytes()).await.is_err() {
                return;
            }
            continue;
        }

        let mut file = match tokio::fs::File::open(&file_path).await {
            Ok(f) => f,
            Err(_) => return,
        };
        let file_len = match file.metadata().await {
            Ok(m) => m.len(),
            Err(_) => return,
        };

        // MIME 类型
        let ext = file_path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        let mime = match ext.as_str() {
            "mp4" | "m4v" => "video/mp4",
            "webm" => "video/webm",
            "ogg" | "ogv" => "video/ogg",
            "mov" => "video/quicktime",
            _ => "application/octet-stream",
        };

        // 解析 Range 头
        let range = header_buf
            .lines()
            .find(|l| l.to_lowercase().starts_with("range:"))
            .and_then(|l| l.split_once(':'))
            .map(|(_, v)| v.trim().to_string())
            .and_then(|v| parse_range_header(&v, file_len));

        if let Some((start, end)) = range {
            let nbytes = end - start + 1;
            let header = format!(
                "HTTP/1.1 206 Partial Content\r\n\
                 Content-Type: {mime}\r\n\
                 Content-Length: {nbytes}\r\n\
                 Content-Range: bytes {start}-{end}/{file_len}\r\n\
                 Accept-Ranges: bytes\r\n\
                 Access-Control-Allow-Origin: *\r\n\
                 Connection: keep-alive\r\n\r\n"
            );
            if writer.write_all(header.as_bytes()).await.is_err() {
                return;
            }

            if file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
                return;
            }
            let mut remaining = nbytes;
            let mut chunk = vec![0u8; 262144]; // 256KB 块
            while remaining > 0 {
                let to_read = (remaining as usize).min(chunk.len());
                match file.read(&mut chunk[..to_read]).await {
                    Ok(0) => break,
                    Ok(n) => {
                        if writer.write_all(&chunk[..n]).await.is_err() {
                            return;
                        }
                        remaining -= n as u64;
                    }
                    Err(_) => break,
                }
            }
        } else {
            let header = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Type: {mime}\r\n\
                 Content-Length: {file_len}\r\n\
                 Accept-Ranges: bytes\r\n\
                 Access-Control-Allow-Origin: *\r\n\
                 Connection: keep-alive\r\n\r\n"
            );
            if writer.write_all(header.as_bytes()).await.is_err() {
                return;
            }

            let mut chunk = vec![0u8; 262144]; // 256KB 块
            loop {
                match file.read(&mut chunk).await {
                    Ok(0) => break,
                    Ok(n) => {
                        if writer.write_all(&chunk[..n]).await.is_err() {
                            return;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
        let _ = writer.flush().await;
    }
}

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
            // 启动本地视频流 HTTP 服务器（支持 Range 请求）
            std::thread::spawn(|| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                    let port = listener.local_addr().unwrap().port();
                    VIDEO_SERVER_PORT.store(port, Ordering::Relaxed);
                    tracing::info!("[视频服务器] 启动于 127.0.0.1:{}", port);

                    loop {
                        if let Ok((stream, _)) = listener.accept().await {
                            tokio::spawn(handle_video_connection(stream));
                        }
                    }
                });
            });

            // 1. 从 SQLite 读取 dataDir（优先），回退到 settings.json 兼容旧数据
            let config_dir_boot = configs::app_config::get_app_config_dir();
            utils::file_manager::ensure_dir(&config_dir_boot).ok();

            // 先尝试 SQLite
            let data_dir_value = configs::database::get_setting("data_dir");

            // 回退到 settings.json
            let data_dir_str = data_dir_value.unwrap_or_else(|| {
                let settings_path = config_dir_boot.join("settings.json");
                if settings_path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&settings_path) {
                        if let Ok(cfg) = serde_json::from_str::<AppConfig>(&content) {
                            return cfg.data_dir;
                        }
                    }
                }
                String::new()
            });

            if !data_dir_str.is_empty() {
                configs::app_config::set_custom_data_dir(std::path::PathBuf::from(&data_dir_str));
            }

            // 2. 创建固定目录（config、cache、prefixes — 不受 dataDir 影响）
            let config_dir = configs::app_config::get_app_config_dir();
            let cache_dir = configs::app_config::get_app_cache_dir();
            let prefixes_dir = utils::file_manager::get_prefixes_dir();

            for dir in [&config_dir, &cache_dir, &prefixes_dir] {
                utils::file_manager::ensure_dir(dir).ok();
            }

            // 3. 如果已设置自定义数据目录，创建符号链接 + Games 子目录
            //    ~/.local/share/ssmt4 -> 自定义目录
            if !data_dir_str.is_empty() {
                let custom_dir = std::path::PathBuf::from(&data_dir_str);
                if let Err(e) = utils::file_manager::setup_data_dir_symlink(&custom_dir) {
                    tracing::error!("设置数据目录符号链接失败: {}", e);
                }
                let games_dir = utils::file_manager::get_global_games_dir();
                utils::file_manager::ensure_dir(&games_dir).ok();
            }

            let data_dir = configs::app_config::get_app_data_dir();
            tracing::info!("Config dir: {}", config_dir.display());
            tracing::info!("Data dir: {}", data_dir.display());
            tracing::info!("Cache dir: {}", cache_dir.display());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Video server
            get_video_server_port,
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
            // Game templates
            commands::game_scanner::get_game_templates_dir,
            commands::game_scanner::list_game_templates,
            commands::game_scanner::import_game_template,
            // Game config
            commands::game_config::load_game_config,
            commands::game_config::save_game_config,
            commands::game_config::create_new_config,
            commands::game_config::delete_game_config_folder,
            commands::game_config::set_game_icon,
            commands::game_config::set_game_background,
            commands::game_config::reset_game_background,
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
            // 遥测防护
            commands::telemetry::check_telemetry_status,
            commands::telemetry::check_game_protection_status,
            commands::telemetry::disable_telemetry,
            commands::telemetry::restore_telemetry,
            commands::telemetry::remove_telemetry_files,
            commands::telemetry::apply_game_protection,
            commands::telemetry::get_game_protection_info,
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
