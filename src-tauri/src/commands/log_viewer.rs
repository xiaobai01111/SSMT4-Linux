use tauri::Manager;

/// 获取日志目录路径
#[tauri::command]
pub fn get_log_dir() -> String {
    "in-memory://runtime-log-session".to_string()
}

/// 读取内存中的运行日志（最多返回尾部 max_lines 行）
#[tauri::command]
pub async fn read_log_file(max_lines: Option<usize>) -> Result<String, String> {
    let max = max_lines.unwrap_or(500);
    let content = crate::utils::runtime_log::read_runtime_log_text(max);
    if content.is_empty() {
        return Ok("当前会话暂无运行日志。".to_string());
    }
    Ok(content)
}

/// 打开独立的日志查看器窗口
#[tauri::command]
pub async fn open_log_window(app: tauri::AppHandle) -> Result<(), String> {
    // 如果窗口已存在，聚焦
    if let Some(window) = app.get_webview_window("log-viewer") {
        window
            .set_focus()
            .map_err(|e| format!("聚焦窗口失败: {}", e))?;
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
