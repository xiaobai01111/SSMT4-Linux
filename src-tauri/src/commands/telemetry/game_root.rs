use std::path::{Path, PathBuf};

use crate::configs::database as db;
use crate::configs::game_presets;

pub(super) fn normalize_game_root(game_preset: &str, game_path: Option<&str>) -> Option<PathBuf> {
    let raw = game_path?.trim();
    if raw.is_empty() {
        return None;
    }

    let path = PathBuf::from(raw);
    let mut candidate = if path.is_file() {
        path.parent().map(|parent| parent.to_path_buf())
    } else if path.extension().is_some() {
        path.parent().map(|parent| parent.to_path_buf())
    } else {
        Some(path)
    }?;

    if let Some(default_folder) = game_presets::get_preset(game_preset)
        .map(|preset| preset.default_folder.trim())
        .filter(|value| !value.is_empty())
    {
        if let Some(inferred) = infer_game_root_with_default_folder(&candidate, default_folder) {
            candidate = inferred;
        }
    }

    Some(candidate)
}

fn infer_game_root_with_default_folder(candidate: &Path, default_folder: &str) -> Option<PathBuf> {
    let target = default_folder
        .split(['/', '\\'])
        .filter(|segment| !segment.trim().is_empty())
        .next_back()?
        .trim();
    if target.is_empty() {
        return None;
    }

    for ancestor in candidate.ancestors() {
        let Some(name) = ancestor.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.eq_ignore_ascii_case(target) {
            return Some(ancestor.to_path_buf());
        }
    }

    None
}

fn resolve_game_root_from_saved_config(game_preset: &str) -> Option<PathBuf> {
    let content = db::get_game_config(game_preset)?;
    let data = serde_json::from_str::<serde_json::Value>(&content).ok()?;
    let folder_candidate = data
        .pointer("/other/gameFolder")
        .and_then(|value| value.as_str())
        .or_else(|| {
            data.pointer("/other/GameFolder")
                .and_then(|value| value.as_str())
        });
    if let Some(root) = normalize_game_root(game_preset, folder_candidate) {
        return Some(root);
    }

    let path_candidate = data
        .pointer("/other/gamePath")
        .and_then(|value| value.as_str())
        .or_else(|| {
            data.pointer("/other/GamePath")
                .and_then(|value| value.as_str())
        });
    normalize_game_root(game_preset, path_candidate)
}

pub(super) fn resolve_game_root(game_preset: &str, game_path: Option<&str>) -> Option<PathBuf> {
    normalize_game_root(game_preset, game_path)
        .or_else(|| resolve_game_root_from_saved_config(game_preset))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::infer_game_root_with_default_folder;

    #[test]
    fn infer_root_with_default_folder_from_nested_exe_dir() {
        let candidate = PathBuf::from(
            "/home/user/Games/WutheringWaves/Wuthering Waves Game/Client/Binaries/Win64",
        );
        let inferred = infer_game_root_with_default_folder(&candidate, "Wuthering Waves Game");
        assert_eq!(
            inferred,
            Some(PathBuf::from(
                "/home/user/Games/WutheringWaves/Wuthering Waves Game"
            ))
        );
    }

    #[test]
    fn infer_root_with_default_folder_keeps_none_when_unmatched() {
        let candidate = PathBuf::from("/home/user/Games/StarRail");
        let inferred = infer_game_root_with_default_folder(&candidate, "Wuthering Waves Game");
        assert_eq!(inferred, None);
    }
}
