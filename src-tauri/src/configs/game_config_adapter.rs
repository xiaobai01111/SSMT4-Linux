use super::game_config_v2::{
    BackgroundType, GameInfoConfigV2, RuntimeEnvironment, GAME_INFO_SCHEMA_VERSION,
};
use serde_json::{json, Map, Value};

fn read_non_empty_string_with_pointer(data: &Value, pointers: &[&str]) -> Option<String> {
    pointers.iter().find_map(|path| {
        data.pointer(path)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
    })
}

fn ensure_object<'a>(root: &'a mut Value, key: &str) -> Option<&'a mut Map<String, Value>> {
    if !root.is_object() {
        *root = json!({});
    }
    let root_obj = root.as_object_mut()?;
    if !root_obj.get(key).map(Value::is_object).unwrap_or(false) {
        root_obj.insert(key.to_string(), json!({}));
    }
    root_obj.get_mut(key).and_then(Value::as_object_mut)
}

pub fn legacy_to_v2(game_name: &str, legacy: &Value) -> GameInfoConfigV2 {
    let preset = read_non_empty_string_with_pointer(
        legacy,
        &[
            "/basic/gamePreset",
            "/basic/GamePreset",
            "/GamePreset",
            "/LogicName",
            "/gamePreset",
        ],
    )
    .map(|value| crate::configs::game_identity::to_canonical_or_keep(&value))
    .unwrap_or_else(|| crate::configs::game_identity::to_canonical_or_keep(game_name));

    let runtime_env = read_non_empty_string_with_pointer(
        legacy,
        &["/basic/runtimeEnv", "/runtimeEnv", "/RuntimeEnv"],
    )
    .map(|value| RuntimeEnvironment::from_legacy(&value))
    .unwrap_or_default();

    let background_type = read_non_empty_string_with_pointer(
        legacy,
        &[
            "/assets/backgroundType",
            "/basic/backgroundType",
            "/backgroundType",
        ],
    )
    .map(|value| BackgroundType::from_legacy(&value))
    .unwrap_or_default();

    let display_name =
        read_non_empty_string_with_pointer(legacy, &["/meta/displayName", "/DisplayName"])
            .unwrap_or_else(|| game_name.to_string());

    let icon_file = read_non_empty_string_with_pointer(
        legacy,
        &[
            "/assets/iconFile",
            "/basic/iconFile",
            "/iconFile",
            "/IconFile",
        ],
    );

    let background_file = read_non_empty_string_with_pointer(
        legacy,
        &[
            "/assets/backgroundFile",
            "/basic/backgroundFile",
            "/backgroundFile",
            "/BackgroundFile",
        ],
    );

    GameInfoConfigV2 {
        schema_version: GAME_INFO_SCHEMA_VERSION,
        game_name: crate::configs::game_identity::to_canonical_or_keep(game_name),
        meta: super::game_config_v2::GameInfoMeta {
            display_name,
            game_preset: preset,
        },
        runtime: super::game_config_v2::GameInfoRuntime { runtime_env },
        assets: super::game_config_v2::GameInfoAssets {
            background_type,
            icon_file,
            background_file,
        },
        read_only: false,
        warning_code: None,
    }
    .normalized(game_name)
}

pub fn v2_to_legacy(v2: &GameInfoConfigV2, legacy_seed: Option<&Value>) -> Value {
    let mut result = legacy_seed.cloned().unwrap_or_else(|| json!({}));
    if !result.is_object() {
        result = json!({});
    }

    if let Some(root) = result.as_object_mut() {
        root.insert(
            "DisplayName".to_string(),
            Value::String(v2.meta.display_name.clone()),
        );
        root.insert(
            "GamePreset".to_string(),
            Value::String(v2.meta.game_preset.clone()),
        );
        root.insert("schemaVersion".to_string(), Value::from(v2.schema_version));
        root.entry("other".to_string()).or_insert_with(|| json!({}));
    }

    if let Some(basic) = ensure_object(&mut result, "basic") {
        basic.insert(
            "gamePreset".to_string(),
            Value::String(v2.meta.game_preset.clone()),
        );
        basic.insert(
            "runtimeEnv".to_string(),
            Value::String(v2.runtime.runtime_env.as_legacy_str().to_string()),
        );
        basic.insert(
            "backgroundType".to_string(),
            Value::String(v2.assets.background_type.as_legacy_str().to_string()),
        );
        if let Some(icon) = &v2.assets.icon_file {
            basic.insert("iconFile".to_string(), Value::String(icon.clone()));
        }
        if let Some(background) = &v2.assets.background_file {
            basic.insert(
                "backgroundFile".to_string(),
                Value::String(background.clone()),
            );
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_to_v2_supports_mixed_fields() {
        let legacy = json!({
            "GamePreset": "ZenlessZoneZero",
            "runtimeEnv": "steam",
            "DisplayName": "绝区零",
            "basic": {
                "backgroundType": "Video"
            }
        });
        let v2 = legacy_to_v2("ZenlessZoneZero", &legacy);
        assert_eq!(v2.schema_version, GAME_INFO_SCHEMA_VERSION);
        assert_eq!(v2.meta.game_preset, "ZenlessZoneZero");
        assert_eq!(v2.meta.display_name, "绝区零");
        assert_eq!(v2.runtime.runtime_env, RuntimeEnvironment::Steam);
        assert_eq!(v2.assets.background_type, BackgroundType::Image);
    }

    #[test]
    fn v2_to_legacy_preserves_unknown_fields() {
        let seed = json!({
            "other": {"gpuIndex": 1},
            "customField": "keep-me"
        });
        let mut v2 = GameInfoConfigV2::new("GenshinImpact");
        v2.meta.game_preset = "GenshinImpact".to_string();
        v2.runtime.runtime_env = RuntimeEnvironment::Linux;
        v2.assets.background_type = BackgroundType::Image;

        let projected = v2_to_legacy(&v2, Some(&seed));
        assert_eq!(projected["customField"], "keep-me");
        assert_eq!(projected["other"]["gpuIndex"], 1);
        assert_eq!(projected["basic"]["runtimeEnv"], "linux");
        assert_eq!(projected["basic"]["gamePreset"], "GenshinImpact");
    }
}
