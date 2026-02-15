use crate::configs::app_config;
use std::path::{Component, Path, PathBuf};

/// 安全路径拼接：将不可信的相对路径拼接到根目录，拒绝路径穿越。
/// 适用于所有来自用户输入、远程清单、配置文件等不可信源的路径拼接。
pub fn safe_join(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let rel = Path::new(relative);
    // 拒绝绝对路径
    if rel.is_absolute() {
        return Err(format!("路径安全校验失败：拒绝绝对路径 '{}'", relative));
    }
    // 拒绝 .. 组件
    for component in rel.components() {
        if matches!(component, Component::ParentDir) {
            return Err(format!("路径安全校验失败：拒绝包含 '..' 的路径 '{}'", relative));
        }
    }
    let joined = root.join(rel);
    // canonicalize 二次校验（路径已存在时）
    let canon_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    if let Ok(canon) = joined.canonicalize() {
        if !canon.starts_with(&canon_root) {
            return Err(format!("路径安全校验失败：'{}' 不在目标目录内", canon.display()));
        }
    }
    Ok(joined)
}

/// 兼容别名（逐步迁移后可移除）
pub fn safe_join_remote(root: &Path, relative: &str) -> Result<PathBuf, String> {
    safe_join(root, relative)
}

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
    app_config::get_app_data_dir().join("tools")
}

pub fn get_3dmigoto_dir() -> PathBuf {
    app_config::get_app_data_dir().join("3Dmigoto")
}

pub fn get_templates_dir() -> PathBuf {
    app_config::get_app_config_dir()
        .join("prefixes")
        .join("_templates")
}

pub fn ensure_dir(path: &Path) -> Result<(), String> {
    if !path.exists() {
        std::fs::create_dir_all(path)
            .map_err(|e| format!("Failed to create directory {}: {}", path.display(), e))?;
    }
    Ok(())
}

/// 获取 XDG 默认数据目录（~/.local/share/ssmt4）
pub fn get_default_xdg_data_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(xdg).join("ssmt4")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("ssmt4")
    } else {
        PathBuf::from("/tmp/ssmt4/data")
    }
}

/// 将 ~/.local/share/ssmt4 符号链接到自定义数据目录
/// 这样 WebKitGTK 和应用的所有数据都写入自定义目录
pub fn setup_data_dir_symlink(custom_dir: &Path) -> Result<(), String> {
    let default_dir = get_default_xdg_data_dir();

    // 边界路径安全校验：禁止自引用和嵌套配置
    // 注意：不能依赖 canonicalize，因为 custom_dir 可能尚不存在，
    // default_dir 可能是符号链接。用原始路径做结构化比较。
    let raw_custom = custom_dir.to_path_buf();
    let raw_default = default_dir.clone();
    if raw_custom == raw_default {
        return Err(format!(
            "自定义数据目录不能与默认目录相同: {}",
            custom_dir.display()
        ));
    }
    if raw_custom.starts_with(&raw_default) {
        return Err(format!(
            "自定义数据目录不能是默认目录的子目录: {} 位于 {} 内",
            custom_dir.display(),
            default_dir.display()
        ));
    }
    if raw_default.starts_with(&raw_custom) {
        return Err(format!(
            "自定义数据目录不能是默认目录的父目录: {} 包含 {}",
            custom_dir.display(),
            default_dir.display()
        ));
    }

    // 确保自定义目录存在
    ensure_dir(custom_dir)?;

    // 如果默认路径已经是指向正确位置的符号链接，无需操作
    if default_dir.is_symlink() {
        if let Ok(target) = std::fs::read_link(&default_dir) {
            if target == custom_dir {
                tracing::info!(
                    "符号链接已存在: {} -> {}",
                    default_dir.display(),
                    custom_dir.display()
                );
                return Ok(());
            }
        }
        // 符号链接指向错误位置，删除
        std::fs::remove_file(&default_dir).map_err(|e| format!("删除旧符号链接失败: {}", e))?;
    } else if default_dir.exists() {
        if !default_dir.is_dir() {
            return Err(format!(
                "默认数据路径不是目录，无法迁移: {}",
                default_dir.display()
            ));
        }
        // 默认路径是真实目录，迁移内容后删除
        tracing::info!("迁移 {} -> {}", default_dir.display(), custom_dir.display());
        migrate_dir_contents(&default_dir, custom_dir)?;
        std::fs::remove_dir_all(&default_dir).map_err(|e| format!("删除旧数据目录失败: {}", e))?;
    }

    // 确保父目录存在
    if let Some(parent) = default_dir.parent() {
        ensure_dir(parent)?;
    }

    // 创建符号链接
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(custom_dir, &default_dir).map_err(|e| {
            format!(
                "创建符号链接失败: {} -> {}: {}",
                default_dir.display(),
                custom_dir.display(),
                e
            )
        })?;
    }

    tracing::info!(
        "符号链接已创建: {} -> {}",
        default_dir.display(),
        custom_dir.display()
    );
    Ok(())
}

/// 移除数据目录符号链接（恢复默认行为）
pub fn remove_data_dir_symlink() {
    let default_dir = get_default_xdg_data_dir();
    if default_dir.is_symlink() {
        std::fs::remove_file(&default_dir).ok();
        tracing::info!("已移除符号链接: {}", default_dir.display());
    }
}

fn migrate_dir_contents(src_root: &Path, dst_root: &Path) -> Result<(), String> {
    let entries = std::fs::read_dir(src_root)
        .map_err(|e| format!("读取旧数据目录失败 {}: {}", src_root.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("读取旧数据目录条目失败: {}", e))?;
        let src = entry.path();
        let dst = dst_root.join(entry.file_name());

        if dst.exists() {
            continue;
        }

        if src.is_dir() {
            copy_dir_recursive(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst).map_err(|e| {
                format!("迁移文件失败 {} -> {}: {}", src.display(), dst.display(), e)
            })?;
        }
    }

    Ok(())
}

pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    ensure_dir(dst)?;
    let entries = std::fs::read_dir(src)
        .map_err(|e| format!("Failed to read directory {}: {}", src.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // ---- safe_join_remote 测试 ----

    #[test]
    fn safe_join_remote_normal_path() {
        let root = Path::new("/tmp/game");
        let result = safe_join_remote(root, "data/file.pak");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), root.join("data/file.pak"));
    }

    #[test]
    fn safe_join_remote_rejects_absolute_path() {
        let root = Path::new("/tmp/game");
        let result = safe_join_remote(root, "/etc/passwd");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("绝对路径"));
    }

    #[test]
    fn safe_join_remote_rejects_parent_traversal() {
        let root = Path::new("/tmp/game");
        let result = safe_join_remote(root, "../../../etc/passwd");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains(".."));
    }

    #[test]
    fn safe_join_remote_rejects_hidden_traversal() {
        let root = Path::new("/tmp/game");
        let result = safe_join_remote(root, "data/../../etc/passwd");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains(".."));
    }

    #[test]
    fn safe_join_remote_allows_current_dir() {
        let root = Path::new("/tmp/game");
        let result = safe_join_remote(root, "./data/file.pak");
        assert!(result.is_ok());
    }

    #[test]
    fn safe_join_remote_allows_deep_path() {
        let root = Path::new("/tmp/game");
        let result = safe_join_remote(root, "a/b/c/d/e/file.pak");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), root.join("a/b/c/d/e/file.pak"));
    }

    // ---- setup_data_dir_symlink 边界校验测试 ----

    #[test]
    fn symlink_rejects_same_path() {
        let default_dir = get_default_xdg_data_dir();
        let result = setup_data_dir_symlink(&default_dir);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("相同"));
    }

    #[test]
    fn symlink_rejects_child_path() {
        let default_dir = get_default_xdg_data_dir();
        let child = default_dir.join("subdir");
        let result = setup_data_dir_symlink(&child);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("子目录"));
    }
}
