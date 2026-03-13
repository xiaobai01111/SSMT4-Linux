use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::utils::file_manager::safe_join;

use super::catalog::telemetry_dlls;

#[derive(Debug, Clone)]
struct StagedTelemetryFile {
    relative_path: String,
    original_path: PathBuf,
    backup_path: PathBuf,
}

#[derive(Debug, Default)]
pub(super) struct StagedTelemetryFiles {
    backup_root: Option<PathBuf>,
    staged: Vec<StagedTelemetryFile>,
    pub not_found: Vec<String>,
}

pub(super) fn evaluate_file_protection(
    game_preset: &str,
    game_root: Option<&Path>,
) -> (bool, Vec<String>, Vec<String>, Option<String>) {
    let dlls = telemetry_dlls(game_preset);
    if dlls.is_empty() {
        return (false, Vec::new(), Vec::new(), None);
    }

    let Some(root) = game_root else {
        return (
            true,
            Vec::new(),
            dlls.iter().map(|value| value.to_string()).collect(),
            Some("缺少游戏目录，无法校验遥测 DLL".to_string()),
        );
    };

    let mut removed = Vec::new();
    let mut existing = Vec::new();
    for dll in &dlls {
        let full_path = root.join(dll);
        if full_path.exists() {
            existing.push(dll.to_string());
        } else {
            removed.push(dll.to_string());
        }
    }

    (true, removed, existing, None)
}

fn stage_telemetry_backup_root(game_root: &Path) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    game_root.join(".ssmt4-protection").join(format!(
        "telemetry-stage-{}-{}",
        std::process::id(),
        nonce
    ))
}

impl StagedTelemetryFiles {
    pub(super) fn stage(game_preset: &str, game_root: &Path) -> Result<Self, String> {
        let dlls = telemetry_dlls(game_preset);
        if dlls.is_empty() {
            return Ok(Self::default());
        }

        let backup_root = stage_telemetry_backup_root(game_root);
        let mut staged = Self {
            backup_root: Some(backup_root.clone()),
            staged: Vec::new(),
            not_found: Vec::new(),
        };

        for dll_path in &dlls {
            let normalized = dll_path.trim().trim_matches(['/', '\\']);
            if normalized.is_empty() {
                staged.not_found.push(dll_path.to_string());
                continue;
            }

            let original_path = safe_join(game_root, normalized)
                .map_err(|error| format!("遥测文件路径非法 {}: {}", dll_path, error))?;
            if !original_path.exists() {
                staged.not_found.push(dll_path.to_string());
                continue;
            }

            let backup_path = safe_join(&backup_root, normalized)
                .map_err(|error| format!("遥测备份路径非法 {}: {}", dll_path, error))?;
            if let Some(parent) = backup_path.parent() {
                std::fs::create_dir_all(parent).map_err(|error| {
                    format!("创建遥测备份目录失败 {}: {}", parent.display(), error)
                })?;
            }

            if let Err(error) = std::fs::rename(&original_path, &backup_path) {
                let rollback_error = staged.rollback().err();
                let mut message = format!(
                    "暂存遥测文件失败 {} -> {}: {}",
                    original_path.display(),
                    backup_path.display(),
                    error
                );
                if let Some(rollback_error) = rollback_error {
                    message.push_str(&format!("；回滚失败: {}", rollback_error));
                }
                return Err(message);
            }

            staged.staged.push(StagedTelemetryFile {
                relative_path: dll_path.to_string(),
                original_path,
                backup_path,
            });
        }

        if staged.staged.is_empty() {
            let _ = staged.cleanup_backup_root();
            staged.backup_root = None;
        }

        Ok(staged)
    }

    pub(super) fn removed(&self) -> Vec<String> {
        self.staged
            .iter()
            .map(|entry| entry.relative_path.clone())
            .collect()
    }

    fn cleanup_backup_root(&mut self) -> Result<(), String> {
        let Some(root) = self.backup_root.as_ref() else {
            return Ok(());
        };
        if !root.exists() {
            self.backup_root = None;
            return Ok(());
        }
        std::fs::remove_dir_all(root)
            .map_err(|error| format!("清理遥测备份目录失败 {}: {}", root.display(), error))?;
        self.backup_root = None;
        Ok(())
    }

    pub(super) fn commit(&mut self) -> Result<(), String> {
        self.cleanup_backup_root()
    }

    pub(super) fn rollback(&mut self) -> Result<(), String> {
        let mut errors = Vec::new();

        for entry in self.staged.iter().rev() {
            if !entry.backup_path.exists() {
                continue;
            }

            if let Some(parent) = entry.original_path.parent() {
                if let Err(error) = std::fs::create_dir_all(parent) {
                    errors.push(format!("创建回滚目录失败 {}: {}", parent.display(), error));
                    continue;
                }
            }

            if let Err(error) = std::fs::rename(&entry.backup_path, &entry.original_path) {
                errors.push(format!(
                    "恢复遥测文件失败 {} -> {}: {}",
                    entry.backup_path.display(),
                    entry.original_path.display(),
                    error
                ));
            }
        }

        if let Err(error) = self.cleanup_backup_root() {
            errors.push(error);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("；"))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn sample_telemetry_dll_fixture() -> (String, Vec<String>) {
        for preset in [
            "WutheringWaves",
            "HonkaiStarRail",
            "ZenlessZoneZero",
            "SnowbreakContainmentZone",
        ] {
            let dlls = telemetry_dlls(preset);
            if !dlls.is_empty() {
                return (preset.to_string(), dlls);
            }
        }
        panic!("expected at least one preset with telemetry dlls");
    }

    fn create_temp_test_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ssmt4-telemetry-{name}-{}-{}",
            std::process::id(),
            nonce
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn staged_telemetry_files_rollback_restores_original_files() {
        let (preset, dlls) = sample_telemetry_dll_fixture();
        let game_root = create_temp_test_dir("rollback");

        for dll in &dlls {
            let path = safe_join(&game_root, dll).unwrap();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&path, b"telemetry").unwrap();
        }

        let mut staged = StagedTelemetryFiles::stage(&preset, &game_root).unwrap();
        assert_eq!(staged.removed().len(), dlls.len());
        for dll in &dlls {
            let path = safe_join(&game_root, dll).unwrap();
            assert!(
                !path.exists(),
                "staged file should be moved out: {}",
                path.display()
            );
        }

        staged.rollback().unwrap();

        for dll in &dlls {
            let path = safe_join(&game_root, dll).unwrap();
            assert!(
                path.exists(),
                "rolled back file should exist: {}",
                path.display()
            );
        }

        let _ = std::fs::remove_dir_all(&game_root);
    }

    #[test]
    fn staged_telemetry_files_commit_keeps_files_removed() {
        let (preset, dlls) = sample_telemetry_dll_fixture();
        let game_root = create_temp_test_dir("commit");

        for dll in &dlls {
            let path = safe_join(&game_root, dll).unwrap();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&path, b"telemetry").unwrap();
        }

        let mut staged = StagedTelemetryFiles::stage(&preset, &game_root).unwrap();
        let backup_root = staged.backup_root.clone();
        staged.commit().unwrap();

        for dll in &dlls {
            let path = safe_join(&game_root, dll).unwrap();
            assert!(
                !path.exists(),
                "committed file should stay removed: {}",
                path.display()
            );
        }
        if let Some(backup_root) = backup_root {
            assert!(
                !backup_root.exists(),
                "backup root should be cleaned: {}",
                backup_root.display()
            );
        }

        let _ = std::fs::remove_dir_all(&game_root);
    }
}
