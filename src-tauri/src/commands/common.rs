use crate::utils::file_manager;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tauri::Manager;
use tracing::{debug, info, warn};

#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! Welcome to SSMT4 Linux.", name)
}

/// 规范化路径并逐文件加入 asset protocol 白名单，确保前端 convertFileSrc 可访问。
/// 返回规范化后的路径字符串。
pub fn allow_asset_file(app: &tauri::AppHandle, path: &Path) -> String {
    let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

    match std::fs::metadata(&resolved) {
        Ok(meta) if meta.is_file() => {
            if let Err(err) = app.asset_protocol_scope().allow_file(&resolved) {
                warn!(
                    "asset protocol 放行失败: path={}, err={}",
                    resolved.display(),
                    err
                );
            }
        }
        Ok(_) => {
            warn!("拒绝放行非文件路径: {}", resolved.display());
        }
        Err(err) => {
            warn!(
                "asset protocol 放行前读取文件元数据失败: path={}, err={}",
                resolved.display(),
                err
            );
        }
    }

    resolved.to_string_lossy().to_string()
}

#[tauri::command]
pub fn get_resource_path(app: tauri::AppHandle, relative: &str) -> Result<String, String> {
    debug!("解析资源路径: relative={}", relative);
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to get resource dir: {}", e))?;

    let mut candidates = collect_resource_candidates(&resource_dir, relative);
    if let Ok(exe) = std::env::current_exe() {
        if let Some(bin_dir) = exe.parent() {
            // 兼容不同生产发行包布局（deb/rpm/pacman/历史版本）
            for base in [
                "../lib/ssmt4/resources",
                "../lib/SSMT4-Linux",
                "../lib/SSMT4-Linux/resources",
            ] {
                candidates.push(bin_dir.join(base).join(relative));
            }

            // 开发版目录仅在 debug 构建时参与查找，避免生产环境误入 Dev 路径。
            #[cfg(debug_assertions)]
            for base in ["../lib/SSMT4-Linux-Dev", "../lib/SSMT4-Linux-Dev/resources"] {
                candidates.push(bin_dir.join(base).join(relative));
            }
        }
    }

    for path in &candidates {
        if path.exists() {
            info!("资源命中: relative={}, path={}", relative, path.display());
            return Ok(allow_asset_file(&app, path));
        }
    }

    warn!(
        "资源缺失: relative={}, searched_locations={}",
        relative,
        candidates.len()
    );
    Err(format!(
        "Resource not found: {} (searched {} locations)",
        relative,
        candidates.len()
    ))
}

pub fn collect_resource_candidates(resource_dir: &Path, relative: &str) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();

    let mut push_unique = |path: PathBuf| {
        let key = path.to_string_lossy().to_string();
        if seen.insert(key) {
            result.push(path);
        }
    };

    // Tauri 官方资源目录布局
    push_unique(resource_dir.join(relative));
    // 兼容某些构建产物将资源放到 resources 子目录
    push_unique(resource_dir.join("resources").join(relative));

    // 开发模式回退：src-tauri/resources/*（仅 debug 构建）
    #[cfg(debug_assertions)]
    push_unique(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join(relative),
    );

    result
}

#[cfg(feature = "devtools")]
#[tauri::command]
pub fn toggle_devtools(window: tauri::WebviewWindow) {
    if window.is_devtools_open() {
        window.close_devtools();
    } else {
        window.open_devtools();
    }
}

#[tauri::command]
pub fn ensure_directory(path: &str) -> Result<(), String> {
    file_manager::ensure_dir(&PathBuf::from(path))
}

#[tauri::command]
pub fn path_exists(path: &str) -> bool {
    Path::new(path).exists()
}

#[tauri::command]
pub fn get_app_data_dir_path() -> String {
    crate::configs::app_config::get_app_data_dir()
        .to_string_lossy()
        .to_string()
}

#[tauri::command]
pub fn open_in_explorer(path: &str) -> Result<(), String> {
    std::process::Command::new("xdg-open")
        .arg(path)
        .spawn()
        .map_err(|e| format!("Failed to open path: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn mark_startup_ready(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(main_window) = app.get_webview_window("main") {
        main_window
            .show()
            .map_err(|e| format!("显示主窗口失败: {}", e))?;
        let _ = main_window.set_focus();
    }

    if let Some(splash_window) = app.get_webview_window("startup-splash") {
        splash_window
            .close()
            .map_err(|e| format!("关闭启动动画窗口失败: {}", e))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        collect_resource_candidates, ensure_directory, get_app_data_dir_path, greet, path_exists,
    };
    use crate::configs::app_config;
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        std::env::temp_dir()
            .join("ssmt4-tests")
            .join(format!("common-{label}-{nonce}"))
    }

    #[test]
    fn greet_returns_expected_message() {
        assert_eq!(greet("xiaobai"), "Hello, xiaobai! Welcome to SSMT4 Linux.");
    }

    #[test]
    fn collect_resource_candidates_covers_known_layouts_without_duplicates() {
        let resource_dir = PathBuf::from("/tmp/ssmt4-resource-root");
        let relative = "docs/home.md";
        let candidates = collect_resource_candidates(&resource_dir, relative);

        assert!(candidates.contains(&resource_dir.join(relative)));
        assert!(candidates.contains(&resource_dir.join("resources").join(relative)));

        #[cfg(debug_assertions)]
        assert!(candidates.contains(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("resources")
                .join(relative)
        ));

        let unique_count = candidates
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect::<HashSet<_>>()
            .len();
        assert_eq!(unique_count, candidates.len());
    }

    #[test]
    fn ensure_directory_and_path_exists_follow_filesystem_state() {
        let dir = unique_temp_dir("ensure-directory");
        let dir_str = dir.to_string_lossy().to_string();

        assert!(!path_exists(&dir_str));
        ensure_directory(&dir_str).expect("create directory");
        assert!(path_exists(&dir_str));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn path_exists_detects_regular_files_too() {
        let dir = unique_temp_dir("path-file");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let file = dir.join("sample.txt");
        std::fs::write(&file, "hello").expect("write temp file");

        assert!(path_exists(&file.to_string_lossy()));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn get_app_data_dir_path_matches_app_config_resolution() {
        assert_eq!(
            get_app_data_dir_path(),
            app_config::get_app_data_dir().to_string_lossy().to_string()
        );
    }
}
