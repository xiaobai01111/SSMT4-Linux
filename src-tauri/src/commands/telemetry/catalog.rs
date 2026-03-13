use crate::configs::{game_identity, game_presets};

pub(super) fn canonical_preset(value: &str) -> String {
    game_identity::to_canonical_or_keep(value)
}

pub(super) fn telemetry_servers(game_preset: &str) -> Vec<String> {
    let game_preset = canonical_preset(game_preset);
    game_presets::get_preset(&game_preset)
        .map(|preset| preset.telemetry_servers.clone())
        .unwrap_or_default()
}

pub(super) fn telemetry_dlls(game_preset: &str) -> Vec<String> {
    let game_preset = canonical_preset(game_preset);
    game_presets::get_preset(&game_preset)
        .map(|preset| preset.telemetry_dlls.clone())
        .unwrap_or_default()
}

pub(super) fn channel_protection_config(
    game_preset: &str,
) -> Option<game_presets::ChannelProtectionConfig> {
    let game_preset = canonical_preset(game_preset);
    game_presets::get_preset(&game_preset).and_then(|preset| preset.channel_protection.clone())
}

pub(super) fn require_protection_before_launch(game_preset: &str) -> bool {
    let game_preset = canonical_preset(game_preset);
    game_presets::get_preset(&game_preset)
        .map(|preset| preset.require_protection_before_launch)
        .unwrap_or(true)
}

pub(super) fn protection_category(game_preset: &str) -> &'static str {
    match canonical_preset(game_preset).as_str() {
        "HonkaiStarRail" | "ZenlessZoneZero" => "HoYoverse",
        "WutheringWaves" => "Kuro Games",
        "SnowbreakContainmentZone" => "Seasun",
        _ => "Unknown",
    }
}
