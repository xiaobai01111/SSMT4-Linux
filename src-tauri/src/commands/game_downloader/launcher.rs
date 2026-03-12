use serde_json::Value;
use std::path::{Path, PathBuf};

fn read_non_empty_string(v: Option<&Value>) -> Option<String> {
    v.and_then(|x| x.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}

fn build_launcher_api_from_config(game_preset: &str, config: &Value) -> Option<Value> {
    let root = config.as_object();
    let other = config.get("other").and_then(|v| v.as_object());

    let launcher_api = read_non_empty_string(
        other
            .and_then(|m| m.get("launcherApi"))
            .or_else(|| root.and_then(|m| m.get("launcherApi"))),
    );

    let launcher_download_api = read_non_empty_string(
        other
            .and_then(|m| m.get("launcherDownloadApi"))
            .or_else(|| root.and_then(|m| m.get("launcherDownloadApi"))),
    );

    let mut servers = Vec::new();
    let mut has_explicit_servers = false;
    if let Some(server_list) = other
        .and_then(|m| m.get("downloadServers"))
        .or_else(|| root.and_then(|m| m.get("downloadServers")))
        .and_then(|v| v.as_array())
    {
        has_explicit_servers = true;
        for (idx, item) in server_list.iter().enumerate() {
            let Some(obj) = item.as_object() else {
                continue;
            };
            let api = read_non_empty_string(obj.get("launcherApi"));
            let Some(api) = api else {
                continue;
            };
            let id = read_non_empty_string(obj.get("id")).unwrap_or_else(|| {
                if idx == 0 {
                    "custom".to_string()
                } else {
                    format!("custom-{}", idx + 1)
                }
            });
            let label =
                read_non_empty_string(obj.get("label")).unwrap_or_else(|| "自定义".to_string());
            let biz_prefix = read_non_empty_string(obj.get("bizPrefix")).unwrap_or_default();
            servers.push(serde_json::json!({
                "id": id,
                "label": label,
                "launcherApi": api,
                "bizPrefix": biz_prefix,
            }));
        }
    }

    if has_explicit_servers && servers.is_empty() {
        if let Some(api) = launcher_api.as_ref() {
            servers.push(serde_json::json!({
                "id": "custom",
                "label": "自定义",
                "launcherApi": api,
                "bizPrefix": "",
            }));
        }
    }

    if has_explicit_servers && servers.is_empty() {
        has_explicit_servers = false;
    }

    let has_default_folder_override = other
        .and_then(|m| m.get("defaultFolder"))
        .or_else(|| root.and_then(|m| m.get("defaultFolder")))
        .is_some();
    let has_audio_languages_override = other
        .and_then(|m| m.get("audioLanguages"))
        .or_else(|| root.and_then(|m| m.get("audioLanguages")))
        .is_some();
    let default_folder = read_non_empty_string(
        other
            .and_then(|m| m.get("defaultFolder"))
            .or_else(|| root.and_then(|m| m.get("defaultFolder"))),
    )
    .unwrap_or_else(|| game_preset.to_string());
    let download_mode = read_non_empty_string(
        other
            .and_then(|m| m.get("downloadMode"))
            .or_else(|| root.and_then(|m| m.get("downloadMode"))),
    );
    if !has_explicit_servers
        && launcher_api.is_none()
        && launcher_download_api.is_none()
        && download_mode.is_none()
        && !has_default_folder_override
        && !has_audio_languages_override
    {
        return None;
    }

    let mut result = serde_json::json!({
        "supported": true,
        "defaultFolder": default_folder,
        "audioLanguages": other
            .and_then(|m| m.get("audioLanguages"))
            .or_else(|| root.and_then(|m| m.get("audioLanguages")))
            .cloned()
            .unwrap_or_else(|| serde_json::json!([])),
    });
    if let Some(api) = launcher_api {
        result["launcherApi"] = Value::String(api);
    }
    if let Some(api) = launcher_download_api {
        result["launcherDownloadApi"] = Value::String(api);
    }
    if let Some(mode) = download_mode {
        result["downloadMode"] = Value::String(mode);
    }
    if has_explicit_servers {
        result["servers"] = Value::Array(servers);
    }

    Some(result)
}

fn read_launcher_api_override_from_game_config(game_preset: &str) -> Option<Value> {
    let config_json = crate::configs::database::get_game_config(game_preset)?;
    let config: Value = serde_json::from_str(&config_json).ok()?;
    build_launcher_api_from_config(game_preset, &config)
}

pub(crate) fn get_game_launcher_api(game_preset: String) -> Result<Value, String> {
    use crate::configs::game_presets;

    let game_preset = crate::configs::game_identity::to_canonical_or_keep(&game_preset);
    let override_obj = read_launcher_api_override_from_game_config(&game_preset);

    let Some(preset) = game_presets::get_preset(&game_preset) else {
        return Ok(override_obj.unwrap_or_else(|| serde_json::json!({ "supported": false })));
    };

    let mut obj = serde_json::json!({
        "supported": preset.supported,
        "defaultFolder": preset.default_folder,
        "servers": preset.download_servers,
        "audioLanguages": preset.audio_languages,
        "downloadMode": preset.download_mode,
    });

    if let Some(ref api) = preset.launcher_api {
        obj["launcherApi"] = Value::String(api.clone());
    }
    if let Some(ref api) = preset.launcher_download_api {
        obj["launcherDownloadApi"] = Value::String(api.clone());
    }

    if let Some(override_value) = override_obj {
        if let Some(override_map) = override_value.as_object() {
            for key in [
                "supported",
                "defaultFolder",
                "servers",
                "audioLanguages",
                "launcherApi",
                "launcherDownloadApi",
                "downloadMode",
            ] {
                if let Some(value) = override_map.get(key) {
                    obj[key] = value.clone();
                }
            }
        }
    }

    Ok(obj)
}

pub(crate) fn get_default_game_folder(game_name: String) -> Result<String, String> {
    let game_name = crate::configs::game_identity::to_canonical_or_keep(&game_name);
    let game_dir = crate::utils::file_manager::get_global_games_dir().join(&game_name);
    Ok(game_dir.to_string_lossy().to_string())
}

pub(crate) fn resolve_downloaded_game_executable(
    game_name: String,
    game_folder: String,
    launcher_api: Option<String>,
) -> Result<Option<String>, String> {
    let _ = launcher_api;
    let game_preset = crate::configs::game_identity::to_canonical_or_keep(&game_name);
    let folder = game_folder.trim();
    if folder.is_empty() {
        return Ok(None);
    }
    let root = PathBuf::from(folder);
    if !root.exists() || !root.is_dir() {
        return Ok(None);
    }
    if !supports_auto_exe_detection(&game_preset) {
        return Ok(None);
    }

    if let Some(path) = resolve_known_game_executable(&game_preset, &root) {
        return Ok(Some(path.to_string_lossy().to_string()));
    }

    Ok(resolve_best_executable_by_scan(&game_preset, &root)
        .map(|path| path.to_string_lossy().to_string()))
}

fn supports_auto_exe_detection(game_preset: &str) -> bool {
    matches!(
        game_preset,
        "HonkaiStarRail" | "ZenlessZoneZero" | "WutheringWaves" | "SnowbreakContainmentZone"
    )
}

fn resolve_known_game_executable(game_preset: &str, game_root: &Path) -> Option<PathBuf> {
    let mut candidates: Vec<String> = Vec::new();
    match game_preset {
        "HonkaiStarRail" => candidates.push("StarRail.exe".to_string()),
        "ZenlessZoneZero" => candidates.push("ZenlessZoneZero.exe".to_string()),
        "WutheringWaves" => {
            candidates.push(
                "Wuthering Waves Game/Client/Binaries/Win64/Client-Win64-Shipping.exe".to_string(),
            );
            candidates.push("Client/Binaries/Win64/Client-Win64-Shipping.exe".to_string());
        }
        "SnowbreakContainmentZone" => {
            candidates.push("Snowbreak.exe".to_string());
            candidates.push("X6Game.exe".to_string());
            candidates.push("X6Game/Binaries/Win64/X6Game-Win64-Shipping.exe".to_string());
            candidates.push("Game/Binaries/Win64/Game-Win64-Shipping.exe".to_string());
        }
        _ => {}
    }

    for rel in candidates {
        let path = game_root.join(rel);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

fn resolve_best_executable_by_scan(game_preset: &str, game_root: &Path) -> Option<PathBuf> {
    let candidates = collect_exe_files(game_root, 7, 20_000);
    let mut best: Option<(i32, usize, PathBuf)> = None;

    for path in candidates {
        let score = score_executable_candidate(game_preset, game_root, &path);
        if score < 10 {
            continue;
        }
        let depth = relative_depth(game_root, &path);
        match &best {
            None => best = Some((score, depth, path)),
            Some((best_score, best_depth, _)) => {
                if score > *best_score || (score == *best_score && depth < *best_depth) {
                    best = Some((score, depth, path));
                }
            }
        }
    }

    best.map(|(_, _, path)| path)
}

fn collect_exe_files(root: &Path, max_depth: usize, max_entries: usize) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let mut stack: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];
    let mut visited: usize = 0;

    while let Some((dir, depth)) = stack.pop() {
        if visited >= max_entries {
            break;
        }
        let Ok(read_dir) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in read_dir.flatten() {
            if visited >= max_entries {
                break;
            }
            visited += 1;
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if file_type.is_dir() {
                if depth < max_depth {
                    stack.push((path, depth + 1));
                }
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            if path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("exe"))
                .unwrap_or(false)
            {
                result.push(path);
            }
        }
    }

    result
}

fn relative_depth(root: &Path, path: &Path) -> usize {
    path.strip_prefix(root)
        .ok()
        .map(|value| value.components().count())
        .unwrap_or(usize::MAX)
}

fn score_executable_candidate(game_preset: &str, root: &Path, path: &Path) -> i32 {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let rel = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();

    let blocked_tokens = [
        "launcher",
        "uninstall",
        "unins",
        "updater",
        "repair",
        "crashreport",
        "crashpad",
        "cef",
        "elevate",
        "easyanticheat",
        "eac",
        "vc_redist",
        "dxsetup",
    ];
    if blocked_tokens
        .iter()
        .any(|token| file_name.contains(token) || rel.contains(token))
    {
        return -1000;
    }

    let mut score = 0;

    if file_name.ends_with("-win64-shipping.exe") {
        score += 55;
    }
    if rel.contains("/binaries/win64/") {
        score += 25;
    }
    if rel.contains("/engine/") || rel.contains("/thirdparty/") {
        score -= 25;
    }
    if relative_depth(root, path) <= 2 {
        score += 10;
    }

    let keywords: &[&str] = match game_preset {
        "WutheringWaves" => &["wuthering", "client-win64-shipping"],
        "SnowbreakContainmentZone" => &["snowbreak", "x6game", "shipping"],
        "HonkaiStarRail" => &["starrail"],
        "ZenlessZoneZero" => &["zenlesszonezero", "zenless"],
        _ => &[],
    };
    for kw in keywords {
        if rel.contains(kw) || file_name.contains(kw) {
            score += 30;
        }
    }

    if let Ok(meta) = std::fs::metadata(path) {
        let size = meta.len();
        if size < 300 * 1024 {
            score -= 20;
        }
        if size >= 20 * 1024 * 1024 {
            score += 8;
        }
        if size >= 80 * 1024 * 1024 {
            score += 8;
        }
    }

    score
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        std::env::temp_dir()
            .join("ssmt4-tests")
            .join(format!("launcher-{label}-{nonce}"))
    }

    #[test]
    fn read_non_empty_string_trims_and_filters_empty() {
        assert_eq!(
            read_non_empty_string(Some(&json!("  launcher  "))),
            Some("launcher".to_string())
        );
        assert_eq!(read_non_empty_string(Some(&json!("   "))), None);
        assert_eq!(read_non_empty_string(None), None);
    }

    #[test]
    fn build_launcher_api_from_config_returns_none_without_overrides() {
        let config = json!({
            "other": {}
        });

        let result = build_launcher_api_from_config("WutheringWaves", &config);
        assert_eq!(result, None);
    }

    #[test]
    fn build_launcher_api_from_config_falls_back_to_custom_server_when_explicit_servers_invalid() {
        let config = json!({
            "other": {
                "launcherApi": "https://example.com/launcher",
                "downloadServers": [
                    { "id": "broken", "label": "Broken" }
                ]
            }
        });

        let result =
            build_launcher_api_from_config("WutheringWaves", &config).expect("override expected");
        let servers = result
            .get("servers")
            .and_then(|value| value.as_array())
            .expect("servers should exist");
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0]["id"], json!("custom"));
        assert_eq!(servers[0]["label"], json!("自定义"));
        assert_eq!(
            servers[0]["launcherApi"],
            json!("https://example.com/launcher")
        );
    }

    #[test]
    fn build_launcher_api_from_config_auto_assigns_server_ids() {
        let config = json!({
            "other": {
                "downloadServers": [
                    {
                        "launcherApi": " https://mirror-a.example.com/api ",
                        "label": "镜像A",
                        "bizPrefix": "  hkrpg_global "
                    },
                    {
                        "launcherApi": "https://mirror-b.example.com/api"
                    }
                ]
            }
        });

        let result =
            build_launcher_api_from_config("HonkaiStarRail", &config).expect("override expected");
        let servers = result
            .get("servers")
            .and_then(|value| value.as_array())
            .expect("servers should exist");
        assert_eq!(servers.len(), 2);
        assert_eq!(servers[0]["id"], json!("custom"));
        assert_eq!(servers[0]["label"], json!("镜像A"));
        assert_eq!(servers[0]["bizPrefix"], json!("hkrpg_global"));
        assert_eq!(servers[1]["id"], json!("custom-2"));
        assert_eq!(servers[1]["label"], json!("自定义"));
        assert_eq!(servers[1]["bizPrefix"], json!(""));
    }

    #[test]
    fn supports_auto_exe_detection_whitelist_is_explicit() {
        assert!(supports_auto_exe_detection("WutheringWaves"));
        assert!(supports_auto_exe_detection("SnowbreakContainmentZone"));
        assert!(!supports_auto_exe_detection("UnknownGame"));
    }

    #[test]
    fn collect_exe_files_respects_depth_and_extension() {
        let root = unique_temp_dir("collect-exe");
        std::fs::create_dir_all(root.join("sub/deep")).expect("create nested dirs");
        std::fs::write(root.join("game.exe"), b"root").expect("write root exe");
        std::fs::write(root.join("sub/launcher.EXE"), b"sub").expect("write sub exe");
        std::fs::write(root.join("sub/deep/too_deep.exe"), b"deep").expect("write deep exe");
        std::fs::write(root.join("sub/readme.txt"), b"txt").expect("write non exe");

        let found = collect_exe_files(&root, 1, 100);
        let as_strings: Vec<String> = found
            .iter()
            .map(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default()
                    .to_string()
            })
            .collect();

        assert!(as_strings.contains(&"game.exe".to_string()));
        assert!(as_strings.contains(&"launcher.EXE".to_string()));
        assert!(!as_strings.contains(&"too_deep.exe".to_string()));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn relative_depth_returns_max_for_non_descendant_path() {
        let root = PathBuf::from("/tmp/ssmt4-root");
        let outside = PathBuf::from("/opt/game.exe");
        assert_eq!(relative_depth(&root, &outside), usize::MAX);
    }

    #[test]
    fn score_executable_candidate_rejects_blocked_launcher_tokens() {
        let root = PathBuf::from("/tmp/ssmt4-root");
        let blocked = root.join("Launcher.exe");
        assert_eq!(
            score_executable_candidate("WutheringWaves", &root, &blocked),
            -1000
        );
    }
}
