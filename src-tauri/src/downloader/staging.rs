use crate::utils::file_manager::safe_join_remote;
use std::path::{Path, PathBuf};
use tracing::{error, info, warn};

/// 将 staging 目录中的文件树合并到目标目录。
///
/// 合并策略：
/// 1) 先把目标同名文件移动到 rollback 目录；
/// 2) 再把 staging 文件原子移动到目标（跨分区回退到 copy+remove）；
/// 3) 任意一步失败会尽力回滚已写入内容。
pub async fn merge_staging_tree_atomically(
    staging_root: &Path,
    target_root: &Path,
    op_tag: &str,
) -> Result<(), String> {
    if !staging_root.exists() {
        return Ok(());
    }

    let staged_files = collect_staged_files(staging_root)?;
    if staged_files.is_empty() {
        return Ok(());
    }

    let session = format!(
        "{}-{}",
        sanitize_tag(op_tag),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    let rollback_root = target_root.join(".ssmt4_rollback").join(session);
    tokio::fs::create_dir_all(&rollback_root)
        .await
        .map_err(|e| format!("创建 rollback 目录失败: {}", e))?;

    let mut moved_into_target: Vec<PathBuf> = Vec::new();
    let mut backups: Vec<(PathBuf, PathBuf)> = Vec::new();

    for staged_path in staged_files {
        let relative = staged_path
            .strip_prefix(staging_root)
            .map_err(|e| format!("staging 路径解析失败: {}", e))?;
        let rel_str = relative.to_string_lossy();
        let dest = safe_join_remote(target_root, &rel_str)?;

        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("创建目标目录失败 {}: {}", parent.display(), e))?;
        }

        if path_is_file_or_symlink(&dest).await {
            let backup_path = rollback_root.join(relative);
            if let Some(parent) = backup_path.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| format!("创建回滚目录失败 {}: {}", parent.display(), e))?;
            }
            if let Err(err) = move_file_or_copy(&dest, &backup_path).await {
                rollback_target_changes(&moved_into_target, &backups).await;
                tokio::fs::remove_dir_all(&rollback_root).await.ok();
                return Err(format!(
                    "备份现有文件失败 {} -> {}: {}",
                    dest.display(),
                    backup_path.display(),
                    err
                ));
            }
            backups.push((dest.clone(), backup_path));
        }

        if let Err(err) = move_file_or_copy(&staged_path, &dest).await {
            error!(
                "staging 合并失败，开始回滚: {} -> {}, err={}",
                staged_path.display(),
                dest.display(),
                err
            );
            rollback_target_changes(&moved_into_target, &backups).await;
            tokio::fs::remove_dir_all(&rollback_root).await.ok();
            return Err(format!("写入目标文件失败 {}: {}", dest.display(), err));
        }

        moved_into_target.push(dest);
    }

    tokio::fs::remove_dir_all(&rollback_root).await.ok();
    info!(
        "staging 合并完成: {} files -> {}",
        moved_into_target.len(),
        target_root.display()
    );
    Ok(())
}

fn collect_staged_files(staging_root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files: Vec<PathBuf> = walkdir::WalkDir::new(staging_root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .collect();
    files.sort();
    for file in &files {
        if file
            .strip_prefix(staging_root)
            .ok()
            .is_none_or(|p| p.as_os_str().is_empty())
        {
            return Err(format!("非法 staging 文件路径: {}", file.display()));
        }
    }
    Ok(files)
}

fn sanitize_tag(tag: &str) -> String {
    let normalized: String = tag
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if normalized.is_empty() {
        "staging".to_string()
    } else {
        normalized
    }
}

async fn path_is_file_or_symlink(path: &Path) -> bool {
    match tokio::fs::symlink_metadata(path).await {
        Ok(meta) => meta.is_file() || meta.file_type().is_symlink(),
        Err(_) => false,
    }
}

async fn move_file_or_copy(src: &Path, dst: &Path) -> Result<(), String> {
    if let Some(parent) = dst.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("创建目录失败 {}: {}", parent.display(), e))?;
    }

    if let Ok(meta) = tokio::fs::symlink_metadata(dst).await {
        if meta.is_file() || meta.file_type().is_symlink() {
            tokio::fs::remove_file(dst)
                .await
                .map_err(|e| format!("删除旧文件失败 {}: {}", dst.display(), e))?;
        }
    }

    match tokio::fs::rename(src, dst).await {
        Ok(_) => Ok(()),
        Err(rename_err) => {
            tokio::fs::copy(src, dst)
                .await
                .map_err(|copy_err| {
                    format!(
                        "rename/copy 都失败 (rename: {}, copy: {})",
                        rename_err, copy_err
                    )
                })?;
            tokio::fs::remove_file(src)
                .await
                .map_err(|e| format!("删除源文件失败 {}: {}", src.display(), e))
        }
    }
}

async fn rollback_target_changes(
    moved_into_target: &[PathBuf],
    backups: &[(PathBuf, PathBuf)],
) {
    for installed in moved_into_target.iter().rev() {
        tokio::fs::remove_file(installed).await.ok();
    }

    for (dest, backup) in backups.iter().rev() {
        if !path_is_file_or_symlink(backup).await {
            continue;
        }
        if let Err(err) = move_file_or_copy(backup, dest).await {
            warn!(
                "回滚恢复失败 {} -> {}: {}",
                backup.display(),
                dest.display(),
                err
            );
        }
    }
}
