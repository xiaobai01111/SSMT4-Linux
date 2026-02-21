use crate::configs::database as db;
use crate::configs::game_config_adapter;
use crate::configs::game_config_v2::{
    GameInfoAssetsPatch, GameInfoConfigV2, GameInfoMetaPatch, GameInfoRuntimePatch,
};
use crate::utils::file_manager::safe_join;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Manager;
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct GameConfig {
    #[serde(flatten)]
    pub data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetCatalogItem {
    pub id: String,
    pub label: String,
    pub display_name_en: String,
    pub legacy_ids: Vec<String>,
    pub default_folder: String,
    pub supported_download: bool,
    pub supported_protection: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateNameResult {
    pub valid: bool,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrateResult {
    pub success: bool,
    pub migrated: bool,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameKeyMigrationStatus {
    pub needed: bool,
    pub done: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameKeyMigrationPreview {
    pub needed: bool,
    pub db_renames: Vec<RenamePair>,
    pub game_dir_renames: Vec<RenamePair>,
    pub prefix_dir_renames: Vec<RenamePair>,
    pub config_files_to_update: usize,
    pub conflicts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameKeyMigrationResult {
    pub success: bool,
    pub migrated: bool,
    pub message: String,
    pub backup_dir: Option<String>,
    pub conflicts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenamePair {
    pub from: String,
    pub to: String,
}

const GAME_KEY_MIGRATION_META_KEY: &str = "game_key_migration_v1_done";

fn canonical_key(input: &str) -> String {
    crate::configs::game_identity::to_canonical_or_keep(input)
}

fn normalize_known_identity_fields(data: &mut Value) {
    fn normalize_leaf(node: &mut Value) {
        if let Some(value) = node.as_str() {
            let canonical = crate::configs::game_identity::to_canonical_or_keep(value);
            if canonical != value {
                *node = Value::String(canonical);
            }
        }
    }

    if let Some(root) = data.as_object_mut() {
        if let Some(v) = root.get_mut("LogicName") {
            normalize_leaf(v);
        }
        if let Some(v) = root.get_mut("GamePreset") {
            normalize_leaf(v);
        }
        if let Some(v) = root.get_mut("GameTypeName") {
            normalize_leaf(v);
        }
    }
    if let Some(v) = data.pointer_mut("/basic/gamePreset") {
        normalize_leaf(v);
    }
    if let Some(v) = data.pointer_mut("/meta/gamePreset") {
        normalize_leaf(v);
    }
    if let Some(v) = data.pointer_mut("/basic/GamePreset") {
        normalize_leaf(v);
    }
}

fn rewrite_legacy_paths_in_value(data: &mut Value, renames: &[(String, String)]) {
    match data {
        Value::String(value) => {
            let mut updated = value.clone();
            for (from, to) in renames {
                let patterns = [
                    format!("/Games/{}/", from),
                    format!("\\Games\\{}\\", from),
                    format!("/prefixes/{}/", from),
                    format!("\\prefixes\\{}\\", from),
                ];
                let replacements = [
                    format!("/Games/{}/", to),
                    format!("\\Games\\{}\\", to),
                    format!("/prefixes/{}/", to),
                    format!("\\prefixes\\{}\\", to),
                ];
                for (idx, pattern) in patterns.iter().enumerate() {
                    if updated.contains(pattern) {
                        updated = updated.replace(pattern, &replacements[idx]);
                    }
                }
            }
            *value = updated;
        }
        Value::Array(items) => {
            for item in items {
                rewrite_legacy_paths_in_value(item, renames);
            }
        }
        Value::Object(map) => {
            for value in map.values_mut() {
                rewrite_legacy_paths_in_value(value, renames);
            }
        }
        _ => {}
    }
}

fn identity_legacy_pairs() -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for identity in crate::configs::game_identity::all_identities() {
        for alias in &identity.legacy_aliases {
            if !alias.eq_ignore_ascii_case(&identity.canonical_key) {
                pairs.push((alias.clone(), identity.canonical_key.clone()));
            }
        }
    }
    pairs
}

#[tauri::command]
pub fn load_game_config(app: tauri::AppHandle, game_name: &str) -> Result<Value, String> {
    let game_name = canonical_key(game_name);
    // 优先从 SQLite 读取
    if let Some(json_str) = db::get_game_config(&game_name) {
        let mut parsed: Value =
            serde_json::from_str(&json_str).map_err(|e| format!("解析游戏配置失败: {}", e))?;
        normalize_known_identity_fields(&mut parsed);
        return Ok(parsed);
    }

    // 回退到文件系统（兼容资源目录中的 Config.json）
    let config_path = get_game_config_path(&app, &game_name)?;
    if !config_path.exists() {
        return Err(format!("Config not found for game: {}", game_name));
    }
    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config: {}", e))?;
    let mut val: Value =
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse config: {}", e))?;
    normalize_known_identity_fields(&mut val);

    // 迁移到 SQLite
    let normalized_content = serde_json::to_string_pretty(&val)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    db::set_game_config(&game_name, &normalized_content);
    info!("从文件迁移游戏配置到 SQLite: {}", game_name);
    Ok(val)
}

#[tauri::command]
pub fn save_game_config(
    _app: tauri::AppHandle,
    game_name: &str,
    mut config: Value,
) -> Result<(), String> {
    let game_name = canonical_key(game_name);
    normalize_known_identity_fields(&mut config);
    let content = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    db::set_game_config(&game_name, &content);
    info!("Saved config for game: {}", game_name);
    Ok(())
}

fn parse_v2_config(json_str: &str, game_name: &str) -> Result<GameInfoConfigV2, String> {
    serde_json::from_str::<GameInfoConfigV2>(json_str)
        .map(|cfg| cfg.normalized(game_name))
        .map_err(|e| format!("解析 V2 配置失败: {}", e))
}

fn save_v2_and_legacy(game_name: &str, config_v2: &GameInfoConfigV2) -> Result<(), String> {
    let game_name = canonical_key(game_name);
    let mut normalized = config_v2.clone().normalized(&game_name);
    normalized.game_name = game_name.clone();
    normalized.meta.game_preset = canonical_key(&normalized.meta.game_preset);
    let v2_json = serde_json::to_string_pretty(&normalized)
        .map_err(|e| format!("序列化 V2 配置失败: {}", e))?;
    db::set_game_config_v2(&game_name, normalized.schema_version, &v2_json);

    let legacy_seed = db::get_game_config(&game_name)
        .and_then(|content| serde_json::from_str::<Value>(&content).ok());
    let legacy = game_config_adapter::v2_to_legacy(&normalized, legacy_seed.as_ref());
    let legacy_json = serde_json::to_string_pretty(&legacy)
        .map_err(|e| format!("序列化 legacy 投影失败: {}", e))?;
    db::set_game_config(&game_name, &legacy_json);
    Ok(())
}

fn migrate_legacy_to_v2_internal(
    app: &tauri::AppHandle,
    game_name: &str,
    persist: bool,
) -> Result<(GameInfoConfigV2, MigrateResult), String> {
    let canonical = canonical_key(game_name);
    let legacy = load_game_config(app.clone(), &canonical)?;
    let mut config_v2 = game_config_adapter::legacy_to_v2(&canonical, &legacy);
    config_v2.meta.game_preset = canonical_key(&config_v2.meta.game_preset);
    if persist {
        save_v2_and_legacy(&canonical, &config_v2)?;
    }
    Ok((
        config_v2,
        MigrateResult {
            success: true,
            migrated: true,
            code: "MIGRATED".to_string(),
            message: "legacy 配置已迁移到 V2".to_string(),
        },
    ))
}

fn ensure_v2_config(app: &tauri::AppHandle, game_name: &str) -> Result<GameInfoConfigV2, String> {
    let canonical = canonical_key(game_name);
    if let Some(v2_json) = db::get_game_config_v2(&canonical) {
        return parse_v2_config(&v2_json, &canonical);
    }
    migrate_legacy_to_v2_internal(app, &canonical, true).map(|(cfg, _)| cfg)
}

fn update_v2_config<F>(app: &tauri::AppHandle, game_name: &str, updater: F) -> Result<(), String>
where
    F: FnOnce(&mut GameInfoConfigV2) -> Result<(), String>,
{
    let canonical = canonical_key(game_name);
    let mut config = ensure_v2_config(app, &canonical)?;
    if config.read_only {
        return Err("当前配置处于只读降级模式，无法保存".to_string());
    }
    updater(&mut config)?;
    save_v2_and_legacy(&canonical, &config)
}

fn collect_existing_game_names() -> BTreeSet<String> {
    let mut names: BTreeSet<String> = db::list_game_names().into_iter().collect();
    if let Ok(games_dir) = get_user_games_dir() {
        if let Ok(entries) = std::fs::read_dir(games_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    names.insert(canonical_key(&entry.file_name().to_string_lossy()));
                }
            }
        }
    }
    names
}

fn validate_game_config_name_internal(
    name: &str,
    current_game_name: Option<&str>,
) -> ValidateNameResult {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return ValidateNameResult {
            valid: false,
            code: "EMPTY".to_string(),
            message: "配置名称不能为空".to_string(),
        };
    }
    if trimmed.len() > 64 {
        return ValidateNameResult {
            valid: false,
            code: "TOO_LONG".to_string(),
            message: "配置名称长度不能超过 64".to_string(),
        };
    }
    let invalid_chars = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
    if trimmed.chars().any(|ch| invalid_chars.contains(&ch)) {
        return ValidateNameResult {
            valid: false,
            code: "INVALID_CHAR".to_string(),
            message: "配置名称包含非法字符".to_string(),
        };
    }
    if trimmed.starts_with('.') {
        return ValidateNameResult {
            valid: false,
            code: "INVALID_DOT_PREFIX".to_string(),
            message: "配置名称不能以 . 开头".to_string(),
        };
    }

    let current = canonical_key(current_game_name.unwrap_or_default());
    let candidate = canonical_key(trimmed);
    if !candidate.eq_ignore_ascii_case(&current) {
        let existing = collect_existing_game_names();
        if existing
            .iter()
            .any(|item| item.eq_ignore_ascii_case(&candidate))
        {
            return ValidateNameResult {
                valid: false,
                code: "DUPLICATE".to_string(),
                message: "配置名称已存在".to_string(),
            };
        }
    }

    ValidateNameResult {
        valid: true,
        code: "OK".to_string(),
        message: "名称可用".to_string(),
    }
}

#[tauri::command]
pub fn list_game_presets_for_info() -> Result<Vec<PresetCatalogItem>, String> {
    let mut list: Vec<PresetCatalogItem> = crate::configs::game_presets::all_presets()
        .values()
        .map(|preset| PresetCatalogItem {
            id: preset.id.clone(),
            label: preset.display_name_en.clone(),
            display_name_en: preset.display_name_en.clone(),
            legacy_ids: preset.legacy_ids.clone(),
            default_folder: preset.default_folder.clone(),
            supported_download: preset.supported,
            supported_protection: !preset.telemetry_servers.is_empty()
                || !preset.telemetry_dlls.is_empty()
                || preset.channel_protection.is_some(),
        })
        .collect();
    list.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(list)
}

#[tauri::command]
pub fn load_game_info_v2(
    app: tauri::AppHandle,
    game_name: String,
) -> Result<GameInfoConfigV2, String> {
    let game_name = canonical_key(&game_name);
    if let Some(v2_json) = db::get_game_config_v2(&game_name) {
        return parse_v2_config(&v2_json, &game_name);
    }

    match migrate_legacy_to_v2_internal(&app, &game_name, true) {
        Ok((config, _)) => Ok(config),
        Err(migrate_error) => {
            let legacy_result = load_game_config(app, &game_name);
            match legacy_result {
                Ok(legacy) => {
                    let mut fallback = game_config_adapter::legacy_to_v2(&game_name, &legacy);
                    fallback.read_only = true;
                    fallback.warning_code = Some("MIGRATE_FAILED_READONLY".to_string());
                    warn!(
                        "迁移 {} 到 V2 失败，已降级为 legacy 只读模式: {}",
                        game_name, migrate_error
                    );
                    Ok(fallback)
                }
                Err(load_error) => Err(format!(
                    "加载 V2 配置失败，且 legacy 兜底也失败。migrate_error={}, legacy_error={}",
                    migrate_error, load_error
                )),
            }
        }
    }
}

#[tauri::command]
pub fn save_game_info_meta(
    app: tauri::AppHandle,
    game_name: String,
    patch: GameInfoMetaPatch,
) -> Result<(), String> {
    update_v2_config(&app, &game_name, |config| {
        if let Some(display_name) = patch.display_name.as_ref() {
            if !display_name.trim().is_empty() {
                config.meta.display_name = display_name.trim().to_string();
            }
        }
        if let Some(game_preset) = patch.game_preset.as_ref() {
            let trimmed = game_preset.trim();
            if !trimmed.is_empty() {
                config.meta.game_preset = canonical_key(trimmed);
            }
        }
        Ok(())
    })
}

#[tauri::command]
pub fn save_game_info_runtime(
    app: tauri::AppHandle,
    game_name: String,
    patch: GameInfoRuntimePatch,
) -> Result<(), String> {
    update_v2_config(&app, &game_name, |config| {
        if let Some(runtime_env) = patch.runtime_env.as_ref() {
            config.runtime.runtime_env = runtime_env.clone();
        }
        Ok(())
    })
}

#[tauri::command]
pub fn save_game_info_assets(
    app: tauri::AppHandle,
    game_name: String,
    patch: GameInfoAssetsPatch,
) -> Result<(), String> {
    update_v2_config(&app, &game_name, |config| {
        let _ = patch.background_type.as_ref();
        config.assets.background_type = crate::configs::game_config_v2::BackgroundType::Image;
        if let Some(icon_file) = patch.icon_file.as_ref() {
            let trimmed = icon_file.trim();
            config.assets.icon_file = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            };
        }
        if let Some(background_file) = patch.background_file.as_ref() {
            let trimmed = background_file.trim();
            config.assets.background_file = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            };
        }
        Ok(())
    })
}

#[tauri::command]
pub fn validate_game_config_name(
    name: String,
    current_game_name: Option<String>,
) -> Result<ValidateNameResult, String> {
    Ok(validate_game_config_name_internal(
        &name,
        current_game_name.as_deref(),
    ))
}

#[tauri::command]
pub fn migrate_game_config_to_v2(
    app: tauri::AppHandle,
    game_name: String,
) -> Result<MigrateResult, String> {
    let game_name = canonical_key(&game_name);
    if db::get_game_config_v2(&game_name).is_some() {
        return Ok(MigrateResult {
            success: true,
            migrated: false,
            code: "ALREADY_V2".to_string(),
            message: "已存在 V2 配置".to_string(),
        });
    }

    match migrate_legacy_to_v2_internal(&app, &game_name, true) {
        Ok((_config, result)) => Ok(result),
        Err(error) => Ok(MigrateResult {
            success: false,
            migrated: false,
            code: "MIGRATE_FAILED".to_string(),
            message: error,
        }),
    }
}

fn config_requires_identity_update(data: &Value) -> bool {
    let candidates = [
        data.get("LogicName").and_then(Value::as_str),
        data.get("GamePreset").and_then(Value::as_str),
        data.get("GameTypeName").and_then(Value::as_str),
        data.pointer("/basic/gamePreset").and_then(Value::as_str),
        data.pointer("/meta/gamePreset").and_then(Value::as_str),
    ];
    candidates.into_iter().flatten().any(|value| {
        crate::configs::game_identity::normalize_game_key_or_alias(value)
            .map(|canonical| !canonical.eq_ignore_ascii_case(value))
            .unwrap_or(false)
    })
}

fn build_game_key_migration_preview(
    app: &tauri::AppHandle,
) -> Result<GameKeyMigrationPreview, String> {
    use std::collections::BTreeSet;

    let games_dir = get_user_games_dir()?;
    let prefixes_dir = crate::utils::file_manager::get_prefixes_dir();
    let db_keys: BTreeSet<String> = db::list_game_names_raw()
        .into_iter()
        .chain(db::list_game_names_v2_raw())
        .collect();

    let mut db_renames = Vec::new();
    let mut game_dir_renames = Vec::new();
    let mut prefix_dir_renames = Vec::new();
    let mut conflicts = Vec::new();

    for (from, to) in identity_legacy_pairs() {
        if from.eq_ignore_ascii_case(&to) {
            continue;
        }

        let has_db_from = db_keys.iter().any(|item| item.eq_ignore_ascii_case(&from));
        let has_db_to = db_keys.iter().any(|item| item.eq_ignore_ascii_case(&to));
        if has_db_from {
            db_renames.push(RenamePair {
                from: from.clone(),
                to: to.clone(),
            });
        }
        if has_db_from && has_db_to {
            conflicts.push(format!("数据库键冲突: {} 与 {} 同时存在", from, to));
        }

        let from_game_dir = safe_join(&games_dir, &from)?;
        let to_game_dir = safe_join(&games_dir, &to)?;
        if from_game_dir.exists() {
            game_dir_renames.push(RenamePair {
                from: from.clone(),
                to: to.clone(),
            });
            if to_game_dir.exists() {
                conflicts.push(format!(
                    "游戏目录冲突: {} 与 {} 同时存在",
                    from_game_dir.display(),
                    to_game_dir.display()
                ));
            }
        }

        let from_prefix_dir = safe_join(&prefixes_dir, &from)?;
        let to_prefix_dir = safe_join(&prefixes_dir, &to)?;
        if from_prefix_dir.exists() {
            prefix_dir_renames.push(RenamePair {
                from: from.clone(),
                to: to.clone(),
            });
            if to_prefix_dir.exists() {
                conflicts.push(format!(
                    "Prefix 目录冲突: {} 与 {} 同时存在",
                    from_prefix_dir.display(),
                    to_prefix_dir.display()
                ));
            }
        }
    }

    let mut config_files_to_update = 0usize;
    if let Ok(entries) = fs::read_dir(&games_dir) {
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let config_path = entry.path().join("Config.json");
            if !config_path.exists() {
                continue;
            }
            if let Ok(content) = fs::read_to_string(&config_path) {
                if let Ok(data) = serde_json::from_str::<Value>(&content) {
                    if config_requires_identity_update(&data) {
                        config_files_to_update += 1;
                    }
                }
            }
        }
    }

    let needed = !db_renames.is_empty()
        || !game_dir_renames.is_empty()
        || !prefix_dir_renames.is_empty()
        || config_files_to_update > 0;

    let _ = app;
    Ok(GameKeyMigrationPreview {
        needed,
        db_renames,
        game_dir_renames,
        prefix_dir_renames,
        config_files_to_update,
        conflicts,
    })
}

fn rollback_dir_renames(base: &std::path::Path, applied: &[RenamePair]) {
    for pair in applied.iter().rev() {
        let from = safe_join(base, &pair.to).ok();
        let to = safe_join(base, &pair.from).ok();
        if let (Some(from), Some(to)) = (from, to) {
            if from.exists() && !to.exists() {
                let _ = fs::rename(&from, &to);
            }
        }
    }
}

fn backup_dir_path() -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    crate::configs::app_config::get_app_cache_dir()
        .join("migration_backups")
        .join(format!("game_key_migration_{}", ts))
}

#[tauri::command]
pub fn preview_game_key_migration(
    app: tauri::AppHandle,
) -> Result<GameKeyMigrationPreview, String> {
    build_game_key_migration_preview(&app)
}

#[tauri::command]
pub fn get_game_key_migration_status(
    app: tauri::AppHandle,
) -> Result<GameKeyMigrationStatus, String> {
    let done = db::get_migration_meta(GAME_KEY_MIGRATION_META_KEY)
        .map(|v| v == "true")
        .unwrap_or(false);
    let preview = build_game_key_migration_preview(&app)?;
    let reason = if done {
        "DONE".to_string()
    } else if preview.needed {
        "NEEDED".to_string()
    } else {
        "NO_CHANGES".to_string()
    };
    Ok(GameKeyMigrationStatus {
        needed: preview.needed,
        done,
        reason,
    })
}

#[tauri::command]
pub fn execute_game_key_migration(app: tauri::AppHandle) -> Result<GameKeyMigrationResult, String> {
    let preview = build_game_key_migration_preview(&app)?;
    if !preview.conflicts.is_empty() {
        return Ok(GameKeyMigrationResult {
            success: false,
            migrated: false,
            message: "存在冲突，迁移已阻止".to_string(),
            backup_dir: None,
            conflicts: preview.conflicts,
        });
    }

    if !preview.needed {
        db::set_migration_meta(GAME_KEY_MIGRATION_META_KEY, "true");
        return Ok(GameKeyMigrationResult {
            success: true,
            migrated: false,
            message: "无需迁移".to_string(),
            backup_dir: None,
            conflicts: Vec::new(),
        });
    }

    let games_dir = get_user_games_dir()?;
    let prefixes_dir = crate::utils::file_manager::get_prefixes_dir();
    let backup_dir = backup_dir_path();
    crate::utils::file_manager::ensure_dir(&backup_dir)?;

    let mut applied_game_renames = Vec::new();
    let mut applied_prefix_renames = Vec::new();
    let mut backed_up_files: Vec<(PathBuf, PathBuf)> = Vec::new();
    let rename_pairs: Vec<(String, String)> = preview
        .db_renames
        .iter()
        .map(|pair| (pair.from.clone(), pair.to.clone()))
        .collect();

    for pair in &preview.game_dir_renames {
        let from = safe_join(&games_dir, &pair.from)?;
        let to = safe_join(&games_dir, &pair.to)?;
        if from.exists() {
            fs::rename(&from, &to)
                .map_err(|e| format!("重命名目录失败 {} -> {}: {}", pair.from, pair.to, e))?;
            applied_game_renames.push(pair.clone());
        }
    }

    for pair in &preview.prefix_dir_renames {
        let from = safe_join(&prefixes_dir, &pair.from)?;
        let to = safe_join(&prefixes_dir, &pair.to)?;
        if from.exists() {
            if let Some(parent) = to.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Err(e) = fs::rename(&from, &to) {
                rollback_dir_renames(&games_dir, &applied_game_renames);
                rollback_dir_renames(&prefixes_dir, &applied_prefix_renames);
                return Err(format!(
                    "重命名 Prefix 目录失败 {} -> {}: {}",
                    pair.from, pair.to, e
                ));
            }
            applied_prefix_renames.push(pair.clone());
        }
    }

    // Update user Config.json files with backup.
    if let Ok(entries) = fs::read_dir(&games_dir) {
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let config_path = entry.path().join("Config.json");
            if !config_path.exists() {
                continue;
            }
            let Ok(content) = fs::read_to_string(&config_path) else {
                continue;
            };
            let Ok(mut data) = serde_json::from_str::<Value>(&content) else {
                continue;
            };
            if !config_requires_identity_update(&data) {
                continue;
            }

            let backup_target = backup_dir.join(format!(
                "{}_Config.json",
                entry.file_name().to_string_lossy().replace('/', "_")
            ));
            fs::copy(&config_path, &backup_target)
                .map_err(|e| format!("备份配置文件失败 {}: {}", config_path.display(), e))?;
            backed_up_files.push((config_path.clone(), backup_target));

            normalize_known_identity_fields(&mut data);
            rewrite_legacy_paths_in_value(&mut data, &rename_pairs);
            let serialized = serde_json::to_string_pretty(&data)
                .map_err(|e| format!("序列化配置失败 {}: {}", config_path.display(), e))?;
            if let Err(e) = fs::write(&config_path, serialized) {
                for (target, backup) in backed_up_files.iter().rev() {
                    let _ = fs::copy(backup, target);
                }
                rollback_dir_renames(&games_dir, &applied_game_renames);
                rollback_dir_renames(&prefixes_dir, &applied_prefix_renames);
                return Err(format!("写入配置失败 {}: {}", config_path.display(), e));
            }
        }
    }

    let hidden_path = games_dir.join("hidden_games.json");
    if hidden_path.exists() {
        if let Ok(content) = fs::read_to_string(&hidden_path) {
            if let Ok(hidden) = serde_json::from_str::<Vec<String>>(&content) {
                let normalized: Vec<String> = hidden
                    .into_iter()
                    .map(|item| canonical_key(&item))
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect();
                let backup_target = backup_dir.join("hidden_games.json");
                fs::copy(&hidden_path, &backup_target).map_err(|e| {
                    format!(
                        "备份 hidden_games.json 失败 {}: {}",
                        hidden_path.display(),
                        e
                    )
                })?;
                backed_up_files.push((hidden_path.clone(), backup_target));
                let serialized = serde_json::to_string_pretty(&normalized)
                    .map_err(|e| format!("序列化 hidden_games.json 失败: {}", e))?;
                fs::write(&hidden_path, serialized)
                    .map_err(|e| format!("写入 hidden_games.json 失败: {}", e))?;
            }
        }
    }

    if let Err(error) = db::rename_game_keys(&rename_pairs) {
        for (target, backup) in backed_up_files.iter().rev() {
            let _ = fs::copy(backup, target);
        }
        rollback_dir_renames(&games_dir, &applied_game_renames);
        rollback_dir_renames(&prefixes_dir, &applied_prefix_renames);
        return Err(error);
    }

    // Normalize stored JSON payloads after key rename.
    for key in db::list_game_names_raw() {
        if let Some(content) = db::get_game_config_exact(&key) {
            if let Ok(mut value) = serde_json::from_str::<Value>(&content) {
                normalize_known_identity_fields(&mut value);
                rewrite_legacy_paths_in_value(&mut value, &rename_pairs);
                if let Ok(serialized) = serde_json::to_string_pretty(&value) {
                    db::set_game_config_exact(&key, &serialized);
                }
            }
        }
    }

    for key in db::list_game_names_v2_raw() {
        if let Some(content) = db::get_game_config_v2_exact(&key) {
            if let Ok(mut value) = serde_json::from_str::<Value>(&content) {
                normalize_known_identity_fields(&mut value);
                rewrite_legacy_paths_in_value(&mut value, &rename_pairs);
                let schema_version = value
                    .get("schemaVersion")
                    .and_then(Value::as_u64)
                    .unwrap_or(2) as u32;
                if let Ok(serialized) = serde_json::to_string_pretty(&value) {
                    db::set_game_config_v2_exact(&key, schema_version, &serialized);
                }
            }
        }
    }

    for (legacy, canonical) in identity_legacy_pairs() {
        db::set_game_key_alias(&legacy, &canonical);
    }
    db::set_migration_meta(GAME_KEY_MIGRATION_META_KEY, "true");

    Ok(GameKeyMigrationResult {
        success: true,
        migrated: true,
        message: "游戏主键迁移完成".to_string(),
        backup_dir: Some(backup_dir.to_string_lossy().to_string()),
        conflicts: Vec::new(),
    })
}

#[tauri::command]
pub fn create_new_config(
    app: tauri::AppHandle,
    new_name: &str,
    config: Option<Value>,
) -> Result<(), String> {
    let new_name = canonical_key(new_name);
    let game_dir = get_writable_game_dir(&app, &new_name)?;
    crate::utils::file_manager::ensure_dir(&game_dir)?;

    let config_path = game_dir.join("Config.json");
    let mut final_config = config.unwrap_or_else(|| {
        serde_json::json!({
            "name": &new_name,
            "LogicName": &new_name,
            "GamePreset": &new_name,
            "GameTypeName": &new_name,
            "gamePath": "",
            "launchArgs": "",
            "workingDir": "",
        })
    });
    normalize_known_identity_fields(&mut final_config);

    let content = serde_json::to_string_pretty(&final_config)
        .map_err(|e| format!("Failed to serialize: {}", e))?;
    std::fs::write(&config_path, &content)
        .map_err(|e| format!("Failed to create config: {}", e))?;
    db::set_game_config(&new_name, &content);
    let v2 = game_config_adapter::legacy_to_v2(&new_name, &final_config);
    let _ = save_v2_and_legacy(&new_name, &v2);

    info!("Created new config for game: {}", new_name);
    Ok(())
}

#[tauri::command]
pub fn delete_game_config_folder(app: tauri::AppHandle, game_name: &str) -> Result<(), String> {
    let game_name = canonical_key(game_name);
    let user_games_dir = get_user_games_dir()?;
    if let Some(game_dir) = find_game_dir_by_logic_name(&user_games_dir, &game_name) {
        if game_dir.exists() {
            std::fs::remove_dir_all(&game_dir)
                .map_err(|e| format!("Failed to delete game folder: {}", e))?;
            info!("Deleted config folder for game: {}", game_name);
        }
    }
    // 同时清理 SQLite 中的游戏配置
    crate::configs::database::delete_game_config(&game_name);
    crate::configs::database::delete_game_config_v2(&game_name);
    // 从 hidden_games.json 中移除
    let _ = super::game_scanner::set_game_visibility(app, game_name.clone(), false);
    Ok(())
}

#[tauri::command]
pub fn reset_game_background(app: tauri::AppHandle, game_name: &str) -> Result<(), String> {
    let game_name = canonical_key(game_name);
    let game_dir = get_writable_game_dir(&app, &game_name)?;
    let bg_extensions = ["png", "jpg", "jpeg", "webp", "mp4", "webm", "ogg", "mov"];
    for ext in &bg_extensions {
        let path = game_dir.join(format!("Background.{}", ext));
        if path.exists() {
            std::fs::remove_file(&path).ok();
            info!("Removed custom background: {}", path.display());
        }
    }
    if let Ok(mut v2) = ensure_v2_config(&app, &game_name) {
        v2.assets.background_file = None;
        v2.assets.background_type = crate::configs::game_config_v2::BackgroundType::Image;
        let _ = save_v2_and_legacy(&game_name, &v2);
    }
    Ok(())
}

#[tauri::command]
pub fn reset_game_icon(app: tauri::AppHandle, game_name: &str) -> Result<(), String> {
    let game_name = canonical_key(game_name);
    let game_dir = get_writable_game_dir(&app, &game_name)?;
    let icon_candidates = [
        "Icon.png",
        "icon.png",
        "Icon.jpg",
        "icon.jpg",
        "Icon.jpeg",
        "icon.jpeg",
    ];
    for name in &icon_candidates {
        let path = game_dir.join(name);
        if path.exists() {
            std::fs::remove_file(&path).ok();
            info!("Removed custom icon: {}", path.display());
        }
    }
    if let Ok(mut v2) = ensure_v2_config(&app, &game_name) {
        v2.assets.icon_file = None;
        let _ = save_v2_and_legacy(&game_name, &v2);
    }
    Ok(())
}

#[tauri::command]
pub fn set_game_icon(
    app: tauri::AppHandle,
    game_name: &str,
    file_path: &str,
) -> Result<String, String> {
    let game_name = canonical_key(game_name);
    let game_dir = get_writable_game_dir(&app, &game_name)?;
    let dest = game_dir.join("Icon.png");
    std::fs::copy(file_path, &dest).map_err(|e| format!("Failed to copy icon: {}", e))?;
    if let Ok(mut v2) = ensure_v2_config(&app, &game_name) {
        v2.assets.icon_file = Some(dest.to_string_lossy().to_string());
        let _ = save_v2_and_legacy(&game_name, &v2);
    }
    Ok(dest.to_string_lossy().to_string())
}

#[tauri::command]
pub fn set_game_background(
    app: tauri::AppHandle,
    game_name: &str,
    file_path: &str,
    bg_type: Option<String>,
) -> Result<String, String> {
    let game_name = canonical_key(game_name);
    let game_dir = get_writable_game_dir(&app, &game_name)?;
    let ext = std::path::Path::new(file_path)
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let dest = game_dir.join(format!("Background.{}", ext));
    std::fs::copy(file_path, &dest).map_err(|e| format!("Failed to copy background: {}", e))?;

    // 同步更新 Config.json 中的 backgroundType
    {
        let _ = bg_type;
        let bt = "Image";
        let mut config = load_game_config(app.clone(), &game_name).unwrap_or(serde_json::json!({}));
        if let Some(basic) = config.get_mut("basic") {
            basic
                .as_object_mut()
                .map(|obj| obj.insert("backgroundType".to_string(), serde_json::json!(bt)));
        } else {
            config.as_object_mut().map(|obj| {
                obj.insert(
                    "basic".to_string(),
                    serde_json::json!({"backgroundType": bt}),
                )
            });
        }
        let content = serde_json::to_string_pretty(&config).unwrap_or_default();
        db::set_game_config(&game_name, &content);
        info!("Updated backgroundType to {} for {}", bt, game_name);

        if let Ok(mut v2) = ensure_v2_config(&app, &game_name) {
            v2.assets.background_type = crate::configs::game_config_v2::BackgroundType::Image;
            v2.assets.background_file = Some(dest.to_string_lossy().to_string());
            let _ = save_v2_and_legacy(&game_name, &v2);
        }
    }

    Ok(dest.to_string_lossy().to_string())
}

#[tauri::command]
pub fn update_game_background(
    app: tauri::AppHandle,
    game_name: &str,
    game_preset: &str,
    bg_type: Option<String>,
) -> Result<String, String> {
    let game_name = canonical_key(game_name);
    let game_preset = canonical_key(game_preset);
    let _ = bg_type;
    // 从预设游戏的资源目录中查找默认背景
    let preset_dir = get_game_dir(&app, &game_preset)?;
    let bg_extensions = ["png", "jpg", "jpeg", "webp", "mp4", "webm"];
    for ext in &bg_extensions {
        let src = preset_dir.join(format!("Background.{}", ext));
        if src.exists() {
            let dest_dir = get_writable_game_dir(&app, &game_name)?;
            let dest = dest_dir.join(format!("Background.{}", ext));
            std::fs::copy(&src, &dest).map_err(|e| format!("Failed to copy background: {}", e))?;
            return Ok(dest.to_string_lossy().to_string());
        }
    }
    Err(format!(
        "No default background found for preset: {}",
        game_preset
    ))
}

fn get_user_games_dir() -> Result<PathBuf, String> {
    let games_dir = crate::utils::file_manager::get_global_games_dir();
    crate::utils::file_manager::ensure_dir(&games_dir)?;
    Ok(games_dir)
}

fn get_resource_games_dirs(app: &tauri::AppHandle) -> Result<Vec<PathBuf>, String> {
    if let Ok(resource_dir) = app.path().resource_dir() {
        crate::utils::data_parameters::set_resource_dir(resource_dir);
    }
    Ok(crate::utils::data_parameters::resolve_games_dirs())
}

fn find_game_dir_by_logic_name(games_dir: &std::path::Path, game_name: &str) -> Option<PathBuf> {
    let target = canonical_key(game_name);
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
        let direct = safe_join(games_dir, candidate).ok()?;
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

            if let Ok(content) = std::fs::read_to_string(&config_path) {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                    let logic_name = data
                        .get("LogicName")
                        .or_else(|| data.get("GamePreset"))
                        .and_then(|v| v.as_str());
                    if logic_name.map(canonical_key).as_deref() == Some(target.as_str()) {
                        return Some(entry.path());
                    }
                }
            }
        }
    }

    None
}

fn find_game_dir_in_candidates(candidates: &[PathBuf], game_name: &str) -> Option<PathBuf> {
    for games_dir in candidates {
        if let Some(found) = find_game_dir_by_logic_name(games_dir, game_name) {
            return Some(found);
        }
    }
    None
}

fn get_game_dir(app: &tauri::AppHandle, game_name: &str) -> Result<PathBuf, String> {
    let game_name = canonical_key(game_name);
    let user_games_dir = get_user_games_dir()?;
    if let Some(found) = find_game_dir_by_logic_name(&user_games_dir, &game_name) {
        return Ok(found);
    }

    let resource_dirs = get_resource_games_dirs(app)?;
    if let Some(found) = find_game_dir_in_candidates(&resource_dirs, &game_name) {
        return Ok(found);
    }

    safe_join(&user_games_dir, &game_name)
}

fn get_writable_game_dir(app: &tauri::AppHandle, game_name: &str) -> Result<PathBuf, String> {
    let game_name = canonical_key(game_name);
    let user_games_dir = get_user_games_dir()?;

    if let Some(found) = find_game_dir_by_logic_name(&user_games_dir, &game_name) {
        return Ok(found);
    }

    let resource_dirs = get_resource_games_dirs(app)?;
    if let Some(src_dir) = find_game_dir_in_candidates(&resource_dirs, &game_name) {
        let folder_name = src_dir
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| game_name.clone());
        let dst_dir = user_games_dir.join(folder_name);

        if !dst_dir.exists() {
            crate::utils::file_manager::copy_dir_recursive(&src_dir, &dst_dir)?;
            info!(
                "Copied game resources to writable dir: {} -> {}",
                src_dir.display(),
                dst_dir.display()
            );
        }

        return Ok(dst_dir);
    }

    let dst_dir = safe_join(&user_games_dir, &game_name)?;
    crate::utils::file_manager::ensure_dir(&dst_dir)?;
    Ok(dst_dir)
}

fn get_game_config_path(app: &tauri::AppHandle, game_name: &str) -> Result<PathBuf, String> {
    Ok(get_game_dir(app, game_name)?.join("Config.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_name_rejects_empty() {
        let result = validate_game_config_name_internal("", None);
        assert!(!result.valid);
        assert_eq!(result.code, "EMPTY");
    }

    #[test]
    fn validate_name_rejects_invalid_characters() {
        let result = validate_game_config_name_internal("bad/name", None);
        assert!(!result.valid);
        assert_eq!(result.code, "INVALID_CHAR");
    }

    #[test]
    fn validate_name_accepts_normal_name() {
        let result =
            validate_game_config_name_internal("ZenlessZoneZero-Test", Some("ZenlessZoneZero"));
        assert!(result.valid);
        assert_eq!(result.code, "OK");
    }
}
