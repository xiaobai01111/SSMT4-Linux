mod catalog;
mod channel;
mod files;
mod game_root;
mod hosts;
mod status;
mod types;
mod workflow;

use catalog::canonical_preset;
use channel::{channel_state_payload, evaluate_channel_protection, set_channel_mode_internal};
use game_root::{normalize_game_root, resolve_game_root};
use types::{
    ApplyGameProtectionPayload, ChannelProtectionStatusResponse, GameProtectionInfoPayload,
    GameProtectionStatusPayload, RemoveTelemetryFilesPayload, TelemetryStatusPayload,
    TelemetryWorkflowPayload,
};

pub(crate) fn check_game_protection_status_internal(
    game_preset: &str,
    game_path: Option<&str>,
) -> Result<GameProtectionStatusPayload, String> {
    let game_preset = canonical_preset(game_preset);
    status::check_game_protection_status_internal(&game_preset, game_path)
}

#[tauri::command]
pub fn get_channel_protection_status(
    game_preset: String,
    game_path: Option<String>,
) -> Result<ChannelProtectionStatusResponse, String> {
    let game_preset = canonical_preset(&game_preset);
    let game_root = resolve_game_root(&game_preset, game_path.as_deref());
    let state = evaluate_channel_protection(&game_preset, game_root.as_deref());
    Ok(ChannelProtectionStatusResponse {
        game_preset,
        supported: state.required,
        game_root: game_root.map(|path| path.to_string_lossy().to_string()),
        channel: channel_state_payload(&state),
    })
}

#[tauri::command]
pub fn set_channel_protection_mode(
    game_preset: String,
    mode: String,
    game_path: String,
) -> Result<ChannelProtectionStatusResponse, String> {
    let game_preset = canonical_preset(&game_preset);
    let game_root = normalize_game_root(&game_preset, Some(&game_path))
        .ok_or_else(|| "缺少游戏目录，无法切换渠道模式".to_string())?;
    let state = set_channel_mode_internal(&game_preset, &game_root, &mode)?;
    Ok(ChannelProtectionStatusResponse {
        game_preset,
        supported: state.required,
        game_root: Some(game_root.to_string_lossy().to_string()),
        channel: channel_state_payload(&state),
    })
}

#[tauri::command]
pub fn check_game_protection_status(
    game_preset: String,
    game_path: Option<String>,
) -> Result<GameProtectionStatusPayload, String> {
    check_game_protection_status_internal(&game_preset, game_path.as_deref())
}

#[tauri::command]
pub fn check_telemetry_status(
    game_preset: String,
    game_path: Option<String>,
) -> Result<TelemetryStatusPayload, String> {
    let game_preset = canonical_preset(&game_preset);
    Ok(status::check_telemetry_status(
        &game_preset,
        game_path.as_deref(),
    ))
}

#[tauri::command]
pub async fn disable_telemetry(
    game_preset: String,
    game_path: Option<String>,
) -> Result<TelemetryWorkflowPayload, String> {
    let game_preset = canonical_preset(&game_preset);
    workflow::disable_telemetry(&game_preset, game_path.as_deref()).await
}

#[tauri::command]
pub async fn restore_telemetry(
    game_preset: String,
    game_path: Option<String>,
) -> Result<TelemetryWorkflowPayload, String> {
    let game_preset = canonical_preset(&game_preset);
    workflow::restore_telemetry(&game_preset, game_path.as_deref()).await
}

#[tauri::command]
pub fn remove_telemetry_files(
    game_preset: String,
    game_path: String,
) -> Result<RemoveTelemetryFilesPayload, String> {
    let game_preset = canonical_preset(&game_preset);
    workflow::remove_telemetry_files(&game_preset, &game_path)
}

#[tauri::command]
pub async fn apply_game_protection(
    game_preset: String,
    game_path: String,
) -> Result<ApplyGameProtectionPayload, String> {
    let game_preset = canonical_preset(&game_preset);
    workflow::apply_game_protection(&game_preset, &game_path).await
}

#[tauri::command]
pub fn get_game_protection_info(game_preset: String) -> Result<GameProtectionInfoPayload, String> {
    let game_preset = canonical_preset(&game_preset);
    Ok(status::game_protection_info(&game_preset))
}

#[cfg(test)]
mod tests {
    use super::channel::evaluate_channel_protection;
    use super::game_root::resolve_game_root;

    #[test]
    fn telemetry_status_without_support_returns_no_protection_message() {
        let payload = super::status::check_telemetry_status("UnknownGame", None);
        assert!(!payload.supported);
        assert_eq!(payload.message.as_deref(), Some("该游戏无需防护"));
    }

    #[test]
    fn channel_status_without_config_is_disabled_but_supported_false() {
        let state = evaluate_channel_protection(
            "UnknownGame",
            resolve_game_root("UnknownGame", None).as_deref(),
        );
        assert!(!state.required);
        assert!(state.enabled);
    }

    #[test]
    fn protection_info_without_preset_has_no_protections() {
        let payload = super::status::game_protection_info("UnknownGame");
        assert!(!payload.has_protections);
    }

    #[test]
    fn channel_protection_config_lookup_is_optional() {
        assert!(super::catalog::channel_protection_config("UnknownGame").is_none());
    }
}
