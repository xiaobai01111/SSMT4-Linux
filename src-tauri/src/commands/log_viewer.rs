use crate::utils::file_manager;
use tauri::Manager;

/// 获取日志目录路径
#[tauri::command]
pub fn get_log_dir() -> String {
    file_manager::get_logs_dir().to_string_lossy().to_string()
}

/// 读取当天日志文件内容（最多返回尾部 max_lines 行）
#[tauri::command]
pub async fn read_log_file(max_lines: Option<usize>) -> Result<String, String> {
    let log_dir = file_manager::get_logs_dir();
    let max = max_lines.unwrap_or(500);

    // 查找最新的日志文件（按修改时间排序）
    let mut entries: Vec<_> = std::fs::read_dir(&log_dir)
        .map_err(|e| format!("读取日志目录失败: {}", e))?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .file_name()
                .map(|n| n.to_string_lossy().starts_with("ssmt4.log"))
                .unwrap_or(false)
        })
        .collect();

    entries.sort_by_key(|e| {
        e.metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });

    let latest = entries
        .last()
        .ok_or("未找到日志文件")?
        .path();

    let content =
        std::fs::read_to_string(&latest).map_err(|e| format!("读取日志文件失败: {}", e))?;

    // 只返回尾部 max 行
    let lines: Vec<&str> = content.lines().collect();
    let start = if lines.len() > max { lines.len() - max } else { 0 };
    Ok(lines[start..].join("\n"))
}

/// 打开独立的日志查看器窗口
#[tauri::command]
pub async fn open_log_window(app: tauri::AppHandle) -> Result<(), String> {
    // 如果窗口已存在，聚焦
    if let Some(window) = app.get_webview_window("log-viewer") {
        window.set_focus().map_err(|e| format!("聚焦窗口失败: {}", e))?;
        return Ok(());
    }

    // 创建新窗口
    tauri::WebviewWindowBuilder::new(
        &app,
        "log-viewer",
        tauri::WebviewUrl::App("/log-viewer".into()),
    )
    .title("SSMT4 日志查看器")
    .inner_size(900.0, 600.0)
    .min_inner_size(600.0, 400.0)
    .center()
    .build()
    .map_err(|e| format!("创建日志窗口失败: {}", e))?;

    Ok(())
}
