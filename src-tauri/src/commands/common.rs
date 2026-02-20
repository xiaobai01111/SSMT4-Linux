use crate::utils::file_manager;
use std::path::PathBuf;
use tauri::Manager;

#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! Welcome to SSMT4-Linux.", name)
}

#[tauri::command]
pub fn get_resource_path(app: tauri::AppHandle, relative: &str) -> Result<String, String> {
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to get resource dir: {}", e))?;
    let resource_path = resource_dir.join(relative);

    // 生产模式：优先 root，再兼容 root/resources（tauri bundle 常见布局）
    if resource_path.exists() {
        return Ok(resource_path.to_string_lossy().to_string());
    }
    let legacy_resource_path = resource_dir.join("resources").join(relative);
    if legacy_resource_path.exists() {
        return Ok(legacy_resource_path.to_string_lossy().to_string());
    }

    // 开发模式回退：resource_dir 指向 target/debug，资源实际在 src-tauri/resources/
    let dev_fallback = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join(relative);
    if dev_fallback.exists() {
        return Ok(dev_fallback.to_string_lossy().to_string());
    }

    // 都找不到，返回原始路径（让调用方处理错误）
    Ok(resource_path.to_string_lossy().to_string())
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
