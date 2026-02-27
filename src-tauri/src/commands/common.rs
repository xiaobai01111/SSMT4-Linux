use crate::utils::file_manager;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tauri::Manager;
use tracing::{debug, info, warn};

#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! Welcome to SSMT4 Linux.", name)
}

/// 规范化路径并动态加入 asset protocol 白名单，确保前端 convertFileSrc 可访问。
/// 返回规范化后的路径字符串。
pub fn allow_asset_file(app: &tauri::AppHandle, path: &Path) -> String {
    let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let _ = app.asset_protocol_scope().allow_file(&resolved);
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
pub fn open_in_explorer(path: &str) -> Result<(), String> {
    std::process::Command::new("xdg-open")
        .arg(path)
        .spawn()
        .map_err(|e| format!("Failed to open path: {}", e))?;
    Ok(())
}
