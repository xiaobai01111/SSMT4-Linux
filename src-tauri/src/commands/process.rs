/// 允许执行的资源文件白名单（仅文件名，不含路径）
const ALLOWED_RESOURCE_EXECUTABLES: &[&str] = &["Run.exe", "upx.exe"];

fn validate_resource_executable_name(resource_name: &str) -> Result<(), String> {
    // 安全校验：拒绝包含路径分隔符的输入（防止 ../逃逸）
    if resource_name.contains('/') || resource_name.contains('\\') || resource_name.contains("..") {
        return Err(format!("拒绝执行：资源名包含非法字符 '{}'", resource_name));
    }

    // 安全校验：仅允许白名单内的可执行文件
    if !ALLOWED_RESOURCE_EXECUTABLES
        .iter()
        .any(|&allowed| allowed == resource_name)
    {
        return Err(format!(
            "拒绝执行：'{}' 不在允许的资源列表中",
            resource_name
        ));
    }

    Ok(())
}

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

    validate_resource_executable_name(&resource_name)?;

    let resource_path = crate::commands::common::get_resource_path(app.clone(), &resource_name)
        .map(std::path::PathBuf::from)
        .map_err(|e| format!("Resource executable not found: {}", e))?;

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

#[cfg(test)]
mod tests {
    use super::validate_resource_executable_name;

    #[test]
    fn validate_resource_executable_name_allows_whitelisted_binaries_only() {
        assert!(validate_resource_executable_name("Run.exe").is_ok());
        assert!(validate_resource_executable_name("upx.exe").is_ok());

        let err = validate_resource_executable_name("other.exe").expect_err("non-whitelisted file");
        assert!(err.contains("不在允许的资源列表中"));
    }

    #[test]
    fn validate_resource_executable_name_rejects_path_traversal_and_separators() {
        assert!(validate_resource_executable_name("../Run.exe").is_err());
        assert!(validate_resource_executable_name("dir/Run.exe").is_err());
        assert!(validate_resource_executable_name("dir\\Run.exe").is_err());
    }

    #[test]
    fn validate_resource_executable_name_rejects_blank_and_case_mismatched_names() {
        assert!(validate_resource_executable_name("").is_err());
        assert!(validate_resource_executable_name("   ").is_err());
        assert!(validate_resource_executable_name("run.exe").is_err());
    }
}
