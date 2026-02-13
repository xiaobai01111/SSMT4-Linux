use tauri::Manager;

#[tauri::command]
pub fn run_resource_executable(
    app: tauri::AppHandle,
    resource_name: Option<String>,
    filename: Option<String>,
    args: Vec<String>,
) -> Result<String, String> {
    let resource_name = resource_name
        .or(filename)
        .ok_or("Missing resource executable name".to_string())?;

    let resource_path = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to get resource dir: {}", e))?
        .join(&resource_name);

    if !resource_path.exists() {
        return Err(format!(
            "Resource executable not found: {}",
            resource_path.display()
        ));
    }

    // Ensure executable permission
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&resource_path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&resource_path, perms).ok();
        }
    }

    let output = std::process::Command::new(&resource_path)
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to execute {}: {}", resource_name, e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok(stdout)
    } else {
        Err(format!("Process failed: {}{}", stdout, stderr))
    }
}
