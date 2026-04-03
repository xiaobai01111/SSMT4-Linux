#[path = "game_launcher/command_builder.rs"]
mod command_builder;
#[path = "game_launcher/launch_monitor.rs"]
mod launch_monitor;
#[path = "game_launcher/log_policy.rs"]
mod log_policy;
#[path = "game_launcher/runtime_env.rs"]
mod runtime_env;
#[path = "game_launcher/target_resolver.rs"]
mod target_resolver;

use crate::configs::app_config::AppConfig;
use crate::configs::database as db;
use crate::events::{emit_game_lifecycle, GameLifecycleEvent};
use crate::process_monitor;
use crate::services::runtime_config;
use crate::wine::{detector, prefix};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum LaunchRunner {
    UmuRun,
    Proton,
    Wine,
    PressureVessel,
}

impl LaunchRunner {
    fn as_str(&self) -> &'static str {
        match self {
            Self::UmuRun => "umu_run",
            Self::Proton => "proton",
            Self::Wine => "wine",
            Self::PressureVessel => "pressure_vessel",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct LaunchRuntimeFlags {
    sandbox_enabled: bool,
    sandbox_isolate_home: bool,
    force_direct_proton: bool,
    use_pressure_vessel: bool,
    region: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LaunchProfile {
    runner: LaunchRunner,
    env: HashMap<String, String>,
    args: Vec<String>,
    working_dir: String,
    prefix_path: String,
    proton_path: String,
    runtime_flags: LaunchRuntimeFlags,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct LaunchRuntimeFlagsPatch {
    sandbox_enabled: Option<bool>,
    sandbox_isolate_home: Option<bool>,
    force_direct_proton: Option<bool>,
    use_pressure_vessel: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct LaunchProfilePatch {
    runner: Option<String>,
    env: Option<HashMap<String, String>>,
    args: Option<Vec<String>>,
    working_dir: Option<String>,
    prefix_path: Option<String>,
    proton_path: Option<String>,
    runtime_flags: LaunchRuntimeFlagsPatch,
}

#[derive(Debug, Clone)]
struct LaunchCommandSpec {
    runner: LaunchRunner,
    program: PathBuf,
    args: Vec<String>,
    use_umu_runtime: bool,
    effective_prefix_dir: PathBuf,
}

fn append_game_log(game_name: &str, level: &str, source: &str, message: impl AsRef<str>) {
    log_policy::append_game_log(game_name, level, source, message);
}

#[cfg(test)]
fn format_env_entry_for_log(key: &str, value: &str) -> Option<String> {
    log_policy::format_env_entry_for_log(key, value)
}

fn sanitize_path_for_log(path: &Path) -> String {
    log_policy::sanitize_path_for_log(path)
}

fn sanitize_arg_for_log(value: &str) -> String {
    log_policy::sanitize_arg_for_log(value)
}

fn append_sorted_env_snapshot(game_name: &str, source: &str, env_map: &HashMap<String, String>) {
    log_policy::append_sorted_env_snapshot(game_name, source, env_map)
}

fn append_host_env_snapshot(game_name: &str) {
    log_policy::append_host_env_snapshot(game_name)
}

#[cfg(test)]
fn detect_external_log_level(stream: &str, line: &str) -> &'static str {
    log_policy::detect_external_log_level(stream, line)
}

#[cfg(test)]
fn detect_external_log_source(stream: &str, line: &str) -> String {
    log_policy::detect_external_log_source(stream, line)
}

#[tauri::command]
pub async fn start_game(
    app: tauri::AppHandle,
    settings: tauri::State<'_, Mutex<AppConfig>>,
    game_name: String,
    game_exe_path: String,
    wine_version_id: String,
) -> Result<String, String> {
    let runtime_config = runtime_config::state_snapshot(&settings)?;
    start_game_internal(
        app,
        runtime_config,
        game_name,
        game_exe_path,
        wine_version_id,
        None,
    )
    .await
}

#[tauri::command]
pub async fn launch_game(
    app: tauri::AppHandle,
    settings: tauri::State<'_, Mutex<AppConfig>>,
    game_name: String,
    region: Option<String>,
) -> Result<String, String> {
    let runtime_config = runtime_config::state_snapshot(&settings)?;
    let game_name = crate::configs::game_identity::to_canonical_or_keep(&game_name);
    let content =
        db::get_game_config(&game_name).ok_or_else(|| format!("未找到游戏配置: {}", game_name))?;
    let config_data: Value =
        serde_json::from_str(&content).map_err(|e| format!("解析游戏配置失败: {}", e))?;

    let game_exe_path = read_non_empty_string(
        config_data
            .pointer("/other/gamePath")
            .or_else(|| config_data.pointer("/gamePath")),
    )
    .ok_or_else(|| "未配置游戏可执行文件路径".to_string())?;

    let mut wine_version_id = read_non_empty_string(
        config_data
            .pointer("/other/wineVersionId")
            .or_else(|| config_data.pointer("/wineVersionId")),
    )
    .unwrap_or_default();

    if wine_version_id.is_empty() {
        if let Ok(prefix_cfg) = prefix::load_prefix_config(&game_name) {
            wine_version_id = prefix_cfg.wine_version_id;
        }
    }
    if wine_version_id.trim().is_empty() {
        return Err("未配置 Wine/Proton 版本".to_string());
    }

    start_game_internal(
        app,
        runtime_config,
        game_name,
        game_exe_path,
        wine_version_id,
        region,
    )
    .await
}

async fn start_game_internal(
    app: tauri::AppHandle,
    runtime_config: AppConfig,
    game_name: String,
    game_exe_path: String,
    wine_version_id: String,
    region_override: Option<String>,
) -> Result<String, String> {
    let request = LaunchRequest {
        app,
        runtime_config,
        game_name: crate::configs::game_identity::to_canonical_or_keep(&game_name),
        game_exe_path,
        wine_version_id,
        region_override,
    };

    log_launch_request(&request);
    let (_launch_guard, game_exe) = preflight_launch_request(&request).await?;
    let launch_plan = build_launch_plan(&request, &game_exe)?;

    spawn_monitored_launch(
        request.app,
        request.game_name,
        launch_plan.resolved_target,
        launch_plan.prepared_command,
    )
    .await
}

fn log_launch_request(request: &LaunchRequest) {
    append_game_log(
        &request.game_name,
        "INFO",
        "session",
        format!(
            "launch request: game={}, game_exe_path={}, wine_version_id={}, region_override={:?}",
            request.game_name,
            request.game_exe_path,
            request.wine_version_id,
            request.region_override
        ),
    );
    debug!(
        "启动请求参数: game={}, game_exe_path={}, wine_version_id={}, region_override={:?}",
        request.game_name, request.game_exe_path, request.wine_version_id, request.region_override
    );
}

async fn preflight_launch_request(
    request: &LaunchRequest,
) -> Result<(process_monitor::LaunchGuard, PathBuf), String> {
    let launch_guard = process_monitor::acquire_launch_guard(&request.game_name)?;

    if process_monitor::is_game_running(&request.game_name).await {
        warn!("游戏 {} 已在运行，拒绝重复启动", request.game_name);
        append_game_log(
            &request.game_name,
            "WARN",
            "session",
            "detected running instance; launch rejected",
        );
        return Err("游戏已在运行中，请勿重复启动".to_string());
    }

    process_monitor::cleanup_stale_processes().await;

    let game_exe = PathBuf::from(&request.game_exe_path);
    if !game_exe.exists() {
        append_game_log(
            &request.game_name,
            "ERROR",
            "session",
            format!("configured executable not found: {}", request.game_exe_path),
        );
        return Err(format!(
            "Game executable not found: {}",
            request.game_exe_path
        ));
    }

    if !request.runtime_config.tos_risk_acknowledged {
        append_game_log(
            &request.game_name,
            "ERROR",
            "session",
            "launch blocked because risk acknowledgement is missing",
        );
        return Err(
            "未完成风险确认，禁止启动。请先在首次向导完成风险确认后再启动游戏。".to_string(),
        );
    }

    Ok((launch_guard, game_exe))
}

fn build_launch_plan(request: &LaunchRequest, game_exe: &Path) -> Result<LaunchPlan, String> {
    let resolved_target = resolve_launch_target(
        &request.game_name,
        game_exe,
        request.region_override.as_deref(),
    )?;
    enforce_launch_protection(&request.game_name, &resolved_target)?;
    let runtime_context =
        load_prefix_runtime_context(&request.game_name, &request.wine_version_id)?;
    let mut env = build_launch_environment(&resolved_target, &runtime_context);
    let run_target = resolve_run_target(
        &request.game_name,
        &resolved_target,
        &request.runtime_config,
        &mut env,
    )?;
    let prepared_command = prepare_launch_command(
        &request.game_name,
        &resolved_target,
        &runtime_context,
        env,
        &run_target,
    )?;

    Ok(LaunchPlan {
        resolved_target,
        prepared_command,
    })
}

#[derive(Debug)]
struct ResolvedLaunchTarget {
    game_config_data: Option<Value>,
    game_preset: String,
    preset_meta: Option<&'static crate::configs::game_presets::GamePreset>,
    launch_region: String,
    configured_exe_path: String,
    launch_exe: PathBuf,
    launch_exe_path: String,
    game_root: PathBuf,
    game_root_str: String,
    write_guard: process_monitor::GameWriteGuard,
}

#[derive(Debug)]
struct PrefixRuntimeContext {
    prefix_dir: PathBuf,
    pfx_dir: PathBuf,
    compat_root_dir: PathBuf,
    use_proton_compat_env: bool,
    prefix_config: crate::configs::wine_config::PrefixConfig,
    settings: crate::configs::wine_config::ProtonSettings,
    proton_path: PathBuf,
}

#[derive(Debug)]
struct ResolvedRunTarget {
    run_exe: PathBuf,
    runner_exe_path: String,
    extra_args: Vec<String>,
    used_bridge: bool,
    managed_migoto_logs: Option<crate::utils::migoto_logs::ManagedMigotoLogPaths>,
}

#[derive(Debug)]
struct PreparedLaunchCommand {
    launch_profile: LaunchProfile,
    command_spec: LaunchCommandSpec,
    runner_name: String,
    command_program_path: String,
    runner_exe_path: String,
    used_bridge: bool,
    managed_migoto_logs: Option<crate::utils::migoto_logs::ManagedMigotoLogPaths>,
}

#[derive(Debug)]
struct LaunchRequest {
    app: tauri::AppHandle,
    runtime_config: AppConfig,
    game_name: String,
    game_exe_path: String,
    wine_version_id: String,
    region_override: Option<String>,
}

#[derive(Debug)]
struct LaunchPlan {
    resolved_target: ResolvedLaunchTarget,
    prepared_command: PreparedLaunchCommand,
}

#[derive(Debug)]
struct StartedLaunch {
    pid: u32,
    root_start_ticks: Option<u64>,
    launched_at: String,
    exe_name: String,
    launch_exe_path: String,
}

struct LaunchStartedContext<'a> {
    app: &'a tauri::AppHandle,
    game_name: &'a str,
    pid: u32,
    command_args: &'a [String],
    configured_exe_path: &'a str,
    launch_exe_path: &'a str,
    runner_name: &'a str,
    runner_exe_path: &'a str,
    command_program_path: &'a str,
    launch_exe: &'a Path,
    region: &'a str,
}

#[derive(Debug)]
struct LaunchMonitorContext {
    app: tauri::AppHandle,
    game_name: String,
    region: String,
    runner_name: String,
    used_bridge: bool,
    launched_at: String,
    pid: u32,
    root_start_ticks: Option<u64>,
    exe_name: String,
    launch_exe_path: String,
    managed_migoto_logs: Option<crate::utils::migoto_logs::ManagedMigotoLogPaths>,
}

#[derive(Debug)]
struct LaunchExitObservation {
    exit_code: Option<i32>,
    signal: Option<i32>,
    crashed: bool,
}

#[derive(Debug)]
struct LaunchMonitorOutcome {
    observation: LaunchExitObservation,
    monitor_timed_out: bool,
}

fn resolve_launch_target(
    game_name: &str,
    game_exe: &Path,
    region_override: Option<&str>,
) -> Result<ResolvedLaunchTarget, String> {
    target_resolver::resolve_launch_target(game_name, game_exe, region_override)
}

fn enforce_launch_protection(game_name: &str, target: &ResolvedLaunchTarget) -> Result<(), String> {
    target_resolver::enforce_launch_protection(game_name, target)
}

fn load_prefix_runtime_context(
    game_name: &str,
    wine_version_id: &str,
) -> Result<PrefixRuntimeContext, String> {
    runtime_env::load_prefix_runtime_context(game_name, wine_version_id)
}

fn build_launch_environment(
    target: &ResolvedLaunchTarget,
    runtime: &PrefixRuntimeContext,
) -> HashMap<String, String> {
    runtime_env::build_launch_environment(target, runtime)
}

fn resolve_run_target(
    game_name: &str,
    target: &ResolvedLaunchTarget,
    runtime_config: &AppConfig,
    env: &mut HashMap<String, String>,
) -> Result<ResolvedRunTarget, String> {
    command_builder::resolve_run_target(game_name, target, runtime_config, env)
}

fn prepare_launch_command(
    game_name: &str,
    target: &ResolvedLaunchTarget,
    runtime: &PrefixRuntimeContext,
    env: HashMap<String, String>,
    run_target: &ResolvedRunTarget,
) -> Result<PreparedLaunchCommand, String> {
    command_builder::prepare_launch_command(game_name, target, runtime, env, run_target)
}

async fn spawn_monitored_launch(
    app: tauri::AppHandle,
    game_name: String,
    target: ResolvedLaunchTarget,
    prepared: PreparedLaunchCommand,
) -> Result<String, String> {
    launch_monitor::spawn_monitored_launch(app, game_name, target, prepared).await
}

fn build_launch_process_command(
    launch_profile: &LaunchProfile,
    command_spec: &LaunchCommandSpec,
    launch_exe: &Path,
) -> Result<tokio::process::Command, String> {
    command_builder::build_launch_process_command(launch_profile, command_spec, launch_exe)
}

fn apply_launch_working_dir(
    cmd: &mut tokio::process::Command,
    launch_profile: &LaunchProfile,
    launch_exe: &Path,
) {
    command_builder::apply_launch_working_dir(cmd, launch_profile, launch_exe)
}

fn attach_launch_log_pipes(child: &mut tokio::process::Child, game_name: &str) {
    log_policy::attach_launch_log_pipes(child, game_name)
}

fn append_bridge_exit_log(game_name: &str, crashed: bool) -> bool {
    log_policy::append_bridge_exit_log(game_name, crashed)
}

#[cfg(test)]
fn extract_launch_profile_patch(
    config: &Value,
    region: Option<&str>,
) -> Option<LaunchProfilePatch> {
    command_builder::extract_launch_profile_patch(config, region)
}

#[cfg(test)]
fn apply_launch_profile_patch(profile: &mut LaunchProfile, patch: LaunchProfilePatch) {
    command_builder::apply_launch_profile_patch(profile, patch)
}

#[cfg(test)]
fn parse_launch_runner(raw: &str) -> Option<LaunchRunner> {
    command_builder::parse_launch_runner(raw)
}

fn read_non_empty_string(v: Option<&Value>) -> Option<String> {
    target_resolver::read_non_empty_string(v)
}

#[cfg(test)]
fn resolve_launch_biz_prefix(
    game_config_data: Option<&Value>,
    preset_meta: Option<&crate::configs::game_presets::GamePreset>,
    launcher_api: Option<&str>,
) -> Option<String> {
    target_resolver::resolve_launch_biz_prefix(game_config_data, preset_meta, launcher_api)
}

#[cfg(test)]
fn resolve_launch_region(
    game_config_data: Option<&Value>,
    preset_meta: Option<&crate::configs::game_presets::GamePreset>,
    region_override: Option<&str>,
) -> String {
    target_resolver::resolve_launch_region(game_config_data, preset_meta, region_override)
}

#[derive(Debug, Clone)]
struct EndfieldLaunchChain {
    launcher_exe: PathBuf,
    endfield_exe: PathBuf,
}

fn find_endfield_launcher_chain(game_exe: &Path) -> Option<EndfieldLaunchChain> {
    target_resolver::find_endfield_launcher_chain(game_exe).map(|chain| EndfieldLaunchChain {
        launcher_exe: chain.launcher_exe,
        endfield_exe: chain.endfield_exe,
    })
}

#[cfg(test)]
fn resolve_preferred_launch_exe(game_preset: &str, game_exe: &Path) -> PathBuf {
    target_resolver::resolve_preferred_launch_exe(game_preset, game_exe)
}

#[cfg(test)]
fn normalize_endfield_launcher_start_args(start_args: &mut Vec<String>) {
    target_resolver::normalize_endfield_launcher_start_args(start_args)
}

fn resolve_preferred_migoto_importer(game_preset: &str, configured_importer: &str) -> String {
    target_resolver::resolve_preferred_migoto_importer(game_preset, configured_importer)
}

#[cfg(unix)]
fn exit_status_signal(status: &std::process::ExitStatus) -> Option<i32> {
    status.signal()
}

#[cfg(not(unix))]
fn exit_status_signal(_status: &std::process::ExitStatus) -> Option<i32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configs::game_presets::{DownloadMode, GamePreset, PresetDownloadServer};
    use serde_json::json;
    use std::collections::HashMap;

    fn sample_game_preset() -> GamePreset {
        GamePreset {
            id: "SampleGame".to_string(),
            legacy_ids: Vec::new(),
            display_name_en: "Sample Game".to_string(),
            supported: true,
            migoto_supported: true,
            require_protection_before_launch: true,
            default_folder: "SampleGame".to_string(),
            launcher_api: None,
            launcher_download_api: None,
            download_servers: vec![
                PresetDownloadServer {
                    id: "global".to_string(),
                    label: "Global".to_string(),
                    launcher_api: "https://launcher.example.com/global".to_string(),
                    biz_prefix: "sample_global".to_string(),
                    api_config: None,
                },
                PresetDownloadServer {
                    id: "cn".to_string(),
                    label: "CN".to_string(),
                    launcher_api: "https://launcher.example.com/cn".to_string(),
                    biz_prefix: "sample_cn".to_string(),
                    api_config: None,
                },
            ],
            download_mode: DownloadMode::FullGame,
            audio_languages: Vec::new(),
            telemetry_servers: Vec::new(),
            telemetry_dlls: Vec::new(),
            channel_protection: None,
            env_defaults: HashMap::new(),
            default_umu_run: false,
            umu_game_id: None,
            umu_store: None,
            force_direct_proton: false,
            force_disable_pressure_vessel: false,
            enable_network_log_by_default: false,
        }
    }

    #[test]
    fn detect_external_log_level_classifies_known_runtime_patterns() {
        assert_eq!(
            detect_external_log_level(
                "stderr",
                "GStreamer-WARNING **: Failed to load plugin /usr/lib/gstreamer-1.0/libgstbad.so: ELFCLASS32"
            ),
            "INFO"
        );
        assert_eq!(
            detect_external_log_level("stderr", "wine: fatal: unimplemented function"),
            "ERROR"
        );
        assert_eq!(detect_external_log_level("stdout", "debug: hello"), "DEBUG");
    }

    #[test]
    fn detect_external_log_source_prefers_structured_prefixes() {
        assert_eq!(
            detect_external_log_source("stderr", "[Bridge] launch completed"),
            "Bridge"
        );
        assert_eq!(
            detect_external_log_source("stdout", "ProtonFixes[123] Using global defaults"),
            "ProtonFixes"
        );
        assert_eq!(
            detect_external_log_source("stdout", "wine: created the configuration directory"),
            "wine"
        );
    }

    #[test]
    fn parse_launch_runner_supports_aliases() {
        assert_eq!(parse_launch_runner("umu"), Some(LaunchRunner::UmuRun));
        assert_eq!(
            parse_launch_runner("direct-proton"),
            Some(LaunchRunner::Proton)
        );
        assert_eq!(
            parse_launch_runner("steam_runtime"),
            Some(LaunchRunner::PressureVessel)
        );
        assert_eq!(parse_launch_runner("unknown"), None);
    }

    #[test]
    fn apply_launch_profile_patch_overrides_runner_env_and_runtime_flags() {
        let mut profile = LaunchProfile {
            runner: LaunchRunner::UmuRun,
            env: HashMap::from([("EXISTING".to_string(), "1".to_string())]),
            args: vec!["--old".to_string()],
            working_dir: "/old".to_string(),
            prefix_path: "/prefix-old".to_string(),
            proton_path: "/proton-old".to_string(),
            runtime_flags: LaunchRuntimeFlags::default(),
        };
        let patch = LaunchProfilePatch {
            runner: Some("wine".to_string()),
            env: Some(HashMap::from([
                ("NEW_ENV".to_string(), "2".to_string()),
                ("   ".to_string(), "ignored".to_string()),
            ])),
            args: Some(vec!["  --new ".to_string(), "   ".to_string()]),
            working_dir: Some(" /work ".to_string()),
            prefix_path: Some(" /prefix-new ".to_string()),
            proton_path: Some(" /proton-new ".to_string()),
            runtime_flags: LaunchRuntimeFlagsPatch {
                sandbox_enabled: Some(true),
                sandbox_isolate_home: Some(true),
                force_direct_proton: Some(true),
                use_pressure_vessel: Some(true),
            },
        };

        apply_launch_profile_patch(&mut profile, patch);

        assert_eq!(profile.runner, LaunchRunner::Wine);
        assert_eq!(profile.env.get("EXISTING").map(String::as_str), Some("1"));
        assert_eq!(profile.env.get("NEW_ENV").map(String::as_str), Some("2"));
        assert_eq!(profile.args, vec!["--new".to_string()]);
        assert_eq!(profile.working_dir, "/work");
        assert_eq!(profile.prefix_path, "/prefix-new");
        assert_eq!(profile.proton_path, "/proton-new");
        assert!(profile.runtime_flags.sandbox_enabled);
        assert!(profile.runtime_flags.sandbox_isolate_home);
        assert!(profile.runtime_flags.force_direct_proton);
        assert!(profile.runtime_flags.use_pressure_vessel);
    }

    #[test]
    fn resolve_launch_biz_prefix_prefers_config_then_matches_preset_server() {
        let preset = sample_game_preset();
        let config = json!({
            "other": {
                "bizPrefix": "configured_biz"
            }
        });

        assert_eq!(
            resolve_launch_biz_prefix(
                Some(&config),
                Some(&preset),
                Some("https://launcher.example.com/cn")
            ),
            Some("configured_biz".to_string())
        );

        let no_config = json!({});
        assert_eq!(
            resolve_launch_biz_prefix(
                Some(&no_config),
                Some(&preset),
                Some("https://launcher.example.com/cn")
            ),
            Some("sample_cn".to_string())
        );
    }

    #[test]
    fn resolve_launch_region_uses_override_then_config_then_server_then_default() {
        let preset = sample_game_preset();
        let config = json!({
            "other": {
                "launcherApi": "https://launcher.example.com/cn",
                "launchRegion": "configured-region"
            }
        });

        assert_eq!(
            resolve_launch_region(Some(&config), Some(&preset), Some("override-region")),
            "override-region".to_string()
        );
        assert_eq!(
            resolve_launch_region(Some(&config), Some(&preset), None),
            "configured-region".to_string()
        );

        let no_region = json!({
            "other": {
                "launcherApi": "https://launcher.example.com/cn"
            }
        });
        assert_eq!(
            resolve_launch_region(Some(&no_region), Some(&preset), None),
            "cn".to_string()
        );

        let no_api = json!({});
        assert_eq!(
            resolve_launch_region(Some(&no_api), Some(&preset), None),
            "global".to_string()
        );
    }

    #[test]
    fn extract_launch_profile_patch_supports_case_and_hyphenated_region_keys() {
        let config = json!({
            "other": {
                "launchProfiles": {
                    "zh_CN": {
                        "runner": "wine",
                        "args": ["--from-region"]
                    }
                }
            }
        });

        let patch = extract_launch_profile_patch(&config, Some("zh-CN")).unwrap();

        assert_eq!(patch.runner.as_deref(), Some("wine"));
        assert_eq!(patch.args, Some(vec!["--from-region".to_string()]));
    }

    #[test]
    fn resolve_preferred_migoto_importer_respects_locked_game_mapping() {
        assert_eq!(
            resolve_preferred_migoto_importer("ArknightsEndfield", "WWMI"),
            "EFMI"
        );
        assert_eq!(
            resolve_preferred_migoto_importer("WutheringWaves", "EFMI"),
            "WWMI"
        );
        assert_eq!(
            resolve_preferred_migoto_importer("UnknownGame", "  efmi "),
            "EFMI"
        );
    }

    #[test]
    fn resolve_game_preset_with_data_ignores_mismatched_known_preset() {
        let config = json!({
            "basic": {
                "gamePreset": "ArknightsEndfield"
            }
        });

        assert_eq!(
            target_resolver::resolve_game_preset_with_data("HonkaiStarRail", Some(&config)),
            "HonkaiStarRail"
        );
    }

    #[test]
    fn resolve_game_preset_with_data_keeps_matching_preset() {
        let config = json!({
            "basic": {
                "gamePreset": "HonkaiStarRail"
            }
        });

        assert_eq!(
            target_resolver::resolve_game_preset_with_data("HonkaiStarRail", Some(&config)),
            "HonkaiStarRail"
        );
    }

    #[test]
    fn resolve_preferred_launch_exe_prefers_endfield_launcher_chain() {
        let unique = format!(
            "ssmt4-endfield-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        let launcher_root = root.join("Program Files/Hypergryph Launcher");
        let launcher_exe = launcher_root.join("Launcher.exe");
        let endfield_exe = launcher_root.join("games/EndField Game/Endfield.exe");

        std::fs::create_dir_all(endfield_exe.parent().unwrap()).unwrap();
        std::fs::write(&launcher_exe, []).unwrap();
        std::fs::write(&endfield_exe, []).unwrap();

        let resolved = resolve_preferred_launch_exe("ArknightsEndfield", &launcher_exe);
        assert_eq!(resolved, launcher_exe);

        let resolved_from_game = resolve_preferred_launch_exe("ArknightsEndfield", &endfield_exe);
        assert_eq!(resolved_from_game, launcher_exe);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn normalize_endfield_launcher_start_args_prefers_launcher_dx11_flag() {
        let mut start_args = vec![
            "-windowed".to_string(),
            "-force-d3d11".to_string(),
            "-dx11".to_string(),
        ];

        normalize_endfield_launcher_start_args(&mut start_args);

        assert_eq!(
            start_args,
            vec!["-windowed".to_string(), "-dx11".to_string()]
        );
    }

    #[test]
    fn format_env_entry_for_log_allows_safe_keys_and_redacts_paths() {
        assert_eq!(
            format_env_entry_for_log("LANG", "zh_CN.UTF-8").as_deref(),
            Some("LANG=zh_CN.UTF-8")
        );
        assert_eq!(
            format_env_entry_for_log("WINEPREFIX", "/home/user/.local/share/ssmt4/prefix")
                .as_deref(),
            Some("WINEPREFIX=<path>")
        );
    }

    #[test]
    fn format_env_entry_for_log_drops_unapproved_or_sensitive_keys() {
        assert_eq!(format_env_entry_for_log("CUSTOM_API_KEY", "secret"), None);
        assert_eq!(
            format_env_entry_for_log("HTTPS_PROXY", "http://127.0.0.1:7890"),
            None
        );
    }

    #[test]
    fn sanitize_arg_for_log_redacts_sensitive_values_and_paths() {
        assert_eq!(sanitize_arg_for_log("--token=abcdef"), "<redacted>");
        assert_eq!(
            sanitize_arg_for_log("/home/user/Games/WutheringWaves"),
            "<path>"
        );
        assert_eq!(sanitize_arg_for_log("--windowed"), "--windowed");
    }
}
