use crate::utils::file_manager;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tauri::Manager;

#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! Welcome to SSMT4 Linux.", name)
}

#[tauri::command]
pub fn get_resource_path(app: tauri::AppHandle, relative: &str) -> Result<String, String> {
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to get resource dir: {}", e))?;

    let mut candidates = collect_resource_candidates(&resource_dir, relative);
    if let Ok(exe) = std::env::current_exe() {
        if let Some(bin_dir) = exe.parent() {
            // 兼容不同发行包布局（deb/rpm/pacman/历史版本）
            for base in [
                "../lib/ssmt4/resources",
                "../lib/SSMT4-Linux",
                "../lib/SSMT4-Linux/resources",
                "../lib/SSMT4-Linux-Dev",
                "../lib/SSMT4-Linux-Dev/resources",
            ] {
                candidates.push(bin_dir.join(base).join(relative));
            }
        }
    }

    for path in &candidates {
        if path.exists() {
            return Ok(path.to_string_lossy().to_string());
        }
    }

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

    // 开发模式回退：src-tauri/resources/*
    push_unique(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join(relative),
    );

    result
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
