use crate::utils::file_manager::{copy_dir_recursive, ensure_dir, get_global_games_dir, safe_join};
use serde_json::Value;
use std::path::{Path, PathBuf};
use tauri::Manager;
use tracing::info;

pub(crate) fn get_user_games_dir() -> Result<PathBuf, String> {
    let games_dir = get_global_games_dir();
    ensure_dir(&games_dir)?;
    Ok(games_dir)
}

pub(crate) fn get_resource_games_dirs(app: &tauri::AppHandle) -> Result<Vec<PathBuf>, String> {
    if let Ok(resource_dir) = app.path().resource_dir() {
        crate::utils::data_parameters::set_resource_dir(resource_dir);
    }
    Ok(crate::utils::data_parameters::resolve_games_dirs())
}

pub(crate) fn extract_game_id_from_config(config: &Value) -> Option<String> {
    config
        .get("LogicName")
        .or_else(|| config.get("GamePreset"))
        .and_then(Value::as_str)
        .map(crate::configs::game_identity::to_canonical_or_keep)
        .filter(|value| !value.trim().is_empty())
}

pub(crate) fn find_game_dir_by_logic_name(games_dir: &Path, game_name: &str) -> Option<PathBuf> {
    let target = crate::configs::game_identity::to_canonical_or_keep(game_name);
    let mut direct_candidates = vec![target.clone()];
    let raw = game_name.trim().to_string();
    if !raw.is_empty() && !raw.eq_ignore_ascii_case(&target) {
        direct_candidates.push(raw);
    }
    for alias in crate::configs::game_identity::legacy_aliases_for_canonical(&target) {
        if !direct_candidates
            .iter()
            .any(|item| item.eq_ignore_ascii_case(&alias))
        {
            direct_candidates.push(alias.clone());
        }
    }

    for candidate in &direct_candidates {
        let Ok(direct) = safe_join(games_dir, candidate) else {
            continue;
        };
        if direct.exists() {
            return Some(direct);
        }
    }

    if let Ok(entries) = std::fs::read_dir(games_dir) {
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let config_path = entry.path().join("Config.json");
            if !config_path.exists() {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&config_path) else {
                continue;
            };
            let Ok(data) = serde_json::from_str::<Value>(&content) else {
                continue;
            };
            if extract_game_id_from_config(&data).as_deref() == Some(target.as_str()) {
                return Some(entry.path());
            }
        }
    }

    None
}

pub(crate) fn find_game_dir_in_candidates(
    candidates: &[PathBuf],
    game_name: &str,
) -> Option<PathBuf> {
    for games_dir in candidates {
        if let Some(found) = find_game_dir_by_logic_name(games_dir, game_name) {
            return Some(found);
        }
    }
    None
}

pub(crate) fn resolve_game_dir_from_roots(
    user_games_dir: &Path,
    resource_dirs: &[PathBuf],
    game_name: &str,
) -> Result<PathBuf, String> {
    let game_name = crate::configs::game_identity::to_canonical_or_keep(game_name);
    if let Some(found) = find_game_dir_by_logic_name(user_games_dir, &game_name) {
        return Ok(found);
    }

    if let Some(found) = find_game_dir_in_candidates(resource_dirs, &game_name) {
        return Ok(found);
    }

    safe_join(user_games_dir, &game_name)
}

pub(crate) fn ensure_writable_game_dir_from_roots(
    user_games_dir: &Path,
    resource_dirs: &[PathBuf],
    game_name: &str,
) -> Result<PathBuf, String> {
    let game_name = crate::configs::game_identity::to_canonical_or_keep(game_name);

    if let Some(found) = find_game_dir_by_logic_name(user_games_dir, &game_name) {
        return Ok(found);
    }

    if let Some(src_dir) = find_game_dir_in_candidates(resource_dirs, &game_name) {
        let folder_name = src_dir
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| game_name.clone());
        let dst_dir = user_games_dir.join(folder_name);

        if !dst_dir.exists() {
            copy_dir_recursive(&src_dir, &dst_dir)?;
            info!(
                "Copied game resources to writable dir: {} -> {}",
                src_dir.display(),
                dst_dir.display()
            );
        }

        return Ok(dst_dir);
    }

    let dst_dir = safe_join(user_games_dir, &game_name)?;
    ensure_dir(&dst_dir)?;
    Ok(dst_dir)
}

pub(crate) fn get_game_dir(app: &tauri::AppHandle, game_name: &str) -> Result<PathBuf, String> {
    let user_games_dir = get_user_games_dir()?;
    let resource_dirs = get_resource_games_dirs(app)?;
    resolve_game_dir_from_roots(&user_games_dir, &resource_dirs, game_name)
}

pub(crate) fn get_writable_game_dir(
    app: &tauri::AppHandle,
    game_name: &str,
) -> Result<PathBuf, String> {
    let user_games_dir = get_user_games_dir()?;
    let resource_dirs = get_resource_games_dirs(app)?;
    ensure_writable_game_dir_from_roots(&user_games_dir, &resource_dirs, game_name)
}

pub(crate) fn get_game_config_path(
    app: &tauri::AppHandle,
    game_name: &str,
) -> Result<PathBuf, String> {
    Ok(get_game_dir(app, game_name)?.join("Config.json"))
}

#[cfg(test)]
mod tests {
    use super::{
        ensure_writable_game_dir_from_roots, extract_game_id_from_config,
        find_game_dir_by_logic_name, resolve_game_dir_from_roots,
    };
    use serde_json::json;
    use std::path::PathBuf;

    fn make_temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "ssmt4_game_dirs_test_{label}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ))
    }

    #[test]
    fn extract_game_id_from_config_prefers_logic_name_and_normalizes() {
        let config = json!({
            "LogicName": " ZenlessZoneZero ",
            "GamePreset": "WutheringWaves"
        });

        assert_eq!(
            extract_game_id_from_config(&config),
            Some("ZenlessZoneZero".to_string())
        );
    }

    #[test]
    fn find_game_dir_by_logic_name_matches_direct_and_config_entries() {
        let root = make_temp_root("find");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("WutheringWaves")).expect("direct");
        std::fs::create_dir_all(root.join("CustomFolder")).expect("custom");
        std::fs::write(
            root.join("CustomFolder").join("Config.json"),
            r#"{"LogicName":"HonkaiStarRail"}"#,
        )
        .expect("config");

        let direct = find_game_dir_by_logic_name(&root, "WutheringWaves").expect("direct");
        let by_config = find_game_dir_by_logic_name(&root, "HonkaiStarRail").expect("config match");

        assert_eq!(
            direct.file_name().and_then(|v| v.to_str()),
            Some("WutheringWaves")
        );
        assert_eq!(
            by_config.file_name().and_then(|v| v.to_str()),
            Some("CustomFolder")
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn resolve_game_dir_from_roots_prefers_user_dir_over_resource_dir() {
        let root = make_temp_root("resolve");
        let user_root = root.join("user");
        let resource_root = root.join("resource");
        std::fs::create_dir_all(user_root.join("WutheringWaves")).expect("user");
        std::fs::create_dir_all(resource_root.join("WutheringWaves")).expect("resource");

        let resolved = resolve_game_dir_from_roots(
            &user_root,
            std::slice::from_ref(&resource_root),
            "WutheringWaves",
        )
        .expect("resolve");

        assert_eq!(resolved, user_root.join("WutheringWaves"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn ensure_writable_game_dir_from_roots_copies_resource_dir_once() {
        let root = make_temp_root("writable");
        let user_root = root.join("user");
        let resource_root = root.join("resource");
        std::fs::create_dir_all(resource_root.join("ZenlessZoneZero")).expect("resource");
        std::fs::write(
            resource_root.join("ZenlessZoneZero").join("Config.json"),
            r#"{"LogicName":"ZenlessZoneZero"}"#,
        )
        .expect("config");

        let writable = ensure_writable_game_dir_from_roots(
            &user_root,
            std::slice::from_ref(&resource_root),
            "ZenlessZoneZero",
        )
        .expect("writable");

        assert_eq!(writable, user_root.join("ZenlessZoneZero"));
        assert!(writable.join("Config.json").exists());

        let rewritten = ensure_writable_game_dir_from_roots(
            &user_root,
            std::slice::from_ref(&resource_root),
            "ZenlessZoneZero",
        )
        .expect("existing writable");
        assert_eq!(rewritten, writable);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn ensure_writable_game_dir_from_roots_creates_empty_user_dir_when_missing() {
        let root = make_temp_root("create");
        let user_root = root.join("user");

        let writable =
            ensure_writable_game_dir_from_roots(&user_root, &[], "Snowbreak").expect("writable");

        assert_eq!(writable, user_root.join("Snowbreak"));
        assert!(writable.exists());

        let _ = std::fs::remove_dir_all(&root);
    }
}
