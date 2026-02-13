use crate::configs::app_config;
use std::path::{Path, PathBuf};

pub fn get_global_games_dir() -> PathBuf {
    app_config::get_app_data_dir().join("Games")
}

pub fn get_prefixes_dir() -> PathBuf {
    app_config::get_app_config_dir().join("prefixes")
}

pub fn get_logs_dir() -> PathBuf {
    app_config::get_app_config_dir().join("logs")
}

pub fn get_tools_dir() -> PathBuf {
    app_config::get_app_cache_dir().join("tools")
}

pub fn get_templates_dir() -> PathBuf {
    app_config::get_app_config_dir().join("prefixes").join("_templates")
}

pub fn ensure_dir(path: &Path) -> Result<(), String> {
    if !path.exists() {
        std::fs::create_dir_all(path)
            .map_err(|e| format!("Failed to create directory {}: {}", path.display(), e))?;
    }
    Ok(())
}

pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    ensure_dir(dst)?;
    let entries = std::fs::read_dir(src)
        .map_err(|e| format!("Failed to read directory {}: {}", src.display(), e))?;

    for entry in entries {
        let entry =
            entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let file_type = entry
            .file_type()
            .map_err(|e| format!("Failed to get file type: {}", e))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path).map_err(|e| {
                format!(
                    "Failed to copy {} -> {}: {}",
                    src_path.display(),
                    dst_path.display(),
                    e
                )
            })?;
        }
    }
    Ok(())
}
