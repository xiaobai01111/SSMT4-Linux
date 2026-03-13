use super::*;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

fn migoto_section<'a>(config: Option<&'a Value>) -> Option<&'a Value> {
    config
        .and_then(|value| value.pointer("/other/migoto"))
        .or_else(|| config.and_then(|value| value.get("migoto")))
}

fn migoto_bool_flag(migoto: Option<&Value>, key: &str) -> bool {
    migoto
        .and_then(|value| value.get(key))
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn migoto_string_flag(migoto: Option<&Value>, key: &str) -> Option<String> {
    migoto
        .and_then(|value| value.get(key))
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn has_active_migoto_entries(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };

    entries.flatten().any(|entry| {
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        !name.is_empty() && !name.starts_with('.') && !name.ends_with(".disabled")
    })
}

fn should_run_migoto_bridge(
    game_preset: &str,
    game_config_data: Option<&Value>,
    app_data_dir: &Path,
) -> bool {
    let migoto = migoto_section(game_config_data);
    let path_state = crate::utils::migoto_layout::resolve_migoto_path_state_for_game(
        game_preset,
        game_config_data.unwrap_or(&Value::Null),
        app_data_dir.join("3Dmigoto-data"),
    );

    let has_mod_entries = has_active_migoto_entries(&path_state.mod_folder);
    let has_shader_fixes = has_active_migoto_entries(&path_state.shader_fixes_folder);
    let has_debug_features = [
        "enable_hunting",
        "dump_shaders",
        "calls_logging",
        "debug_logging",
    ]
    .iter()
    .any(|key| migoto_bool_flag(migoto, key));
    let has_extra_libraries = migoto_bool_flag(migoto, "extra_libraries_enabled")
        && migoto_string_flag(migoto, "extra_libraries_paths").is_some();
    let has_custom_launch = migoto_bool_flag(migoto, "custom_launch_enabled")
        && migoto_string_flag(migoto, "custom_launch_cmd").is_some();

    has_mod_entries
        || has_shader_fixes
        || has_debug_features
        || has_extra_libraries
        || has_custom_launch
}

fn should_force_umu_runner_with_availability(
    game_preset: &str,
    proton_path: &Path,
    current_runner: LaunchRunner,
    force_direct_proton: bool,
    umu_available: bool,
) -> bool {
    if force_direct_proton || !umu_available || current_runner == LaunchRunner::Wine {
        return false;
    }
    if !game_preset.eq_ignore_ascii_case("ArknightsEndfield") {
        return false;
    }

    proton_path
        .to_string_lossy()
        .to_ascii_lowercase()
        .contains("proton")
}

fn should_force_umu_runner(
    game_preset: &str,
    proton_path: &Path,
    current_runner: LaunchRunner,
    force_direct_proton: bool,
) -> bool {
    should_force_umu_runner_with_availability(
        game_preset,
        proton_path,
        current_runner,
        force_direct_proton,
        super::runtime_env::find_umu_run_binary().is_some(),
    )
}

pub(super) fn resolve_run_target(
    game_name: &str,
    target: &ResolvedLaunchTarget,
    runtime_config: &crate::configs::app_config::AppConfig,
    env: &mut HashMap<String, String>,
) -> Result<ResolvedRunTarget, String> {
    let is_hoyoverse = matches!(
        target.game_preset.as_str(),
        "HonkaiStarRail" | "ZenlessZoneZero"
    );
    let jadeite_exe = if is_hoyoverse {
        super::super::jadeite::resolve_patch_dir(game_name)
            .ok()
            .map(|d| d.join("jadeite.exe"))
            .filter(|p| p.exists())
    } else {
        None
    };

    let migoto_globally_enabled = runtime_config.migoto_enabled;
    let migoto_supported =
        crate::configs::game_presets::supports_migoto(&target.game_preset);
    let migoto_requested = target
        .game_config_data
        .as_ref()
        .and_then(|c| c.pointer("/other/migoto/enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let app_data_dir = crate::configs::app_config::get_app_data_dir();
    let migoto_enabled = migoto_globally_enabled && migoto_supported && migoto_requested;
    let migoto_runtime_required = migoto_enabled
        && should_run_migoto_bridge(&target.game_preset, target.game_config_data.as_ref(), &app_data_dir);
    let configured_migoto_importer = target
        .game_config_data
        .as_ref()
        .and_then(|c| c.pointer("/other/migoto/importer"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let migoto_importer =
        super::resolve_preferred_migoto_importer(&target.game_preset, configured_migoto_importer);

    if migoto_requested && !migoto_globally_enabled {
        warn!("3DMigoto 已被全局禁用，本次启动将跳过相关配置和注入");
        super::append_game_log(
            game_name,
            "INFO",
            "launcher",
            "3DMigoto is globally disabled; skipping migoto bridge and injection",
        );
    }
    if migoto_requested && !migoto_supported {
        warn!(
            "当前游戏暂不支持 3DMigoto / Mod 注入链，已忽略该配置: preset={}",
            target.game_preset
        );
        super::append_game_log(
            game_name,
            "WARN",
            "launcher",
            format!(
                "3DMigoto is not supported for preset {}; skipping migoto bridge and injection",
                target.game_preset
            ),
        );
    }
    if migoto_enabled && !migoto_runtime_required {
        info!(
            "3DMigoto 已启用，但未检测到有效 Mods/ShaderFixes 或高级调试功能，跳过 bridge 注入链: preset={}",
            target.game_preset
        );
        super::append_game_log(
            game_name,
            "INFO",
            "bridge",
            format!(
                "3DMigoto enabled for preset {}, but no active Mods/ShaderFixes or advanced bridge features were detected; skipping bridge injection",
                target.game_preset
            ),
        );
    }
    if migoto_enabled && !configured_migoto_importer.eq_ignore_ascii_case(&migoto_importer) {
        warn!(
            "3DMigoto importer 已按游戏预设校正: preset={}, configured={}, effective={}",
            target.game_preset, configured_migoto_importer, migoto_importer
        );
        super::append_game_log(
            game_name,
            "WARN",
            "launcher",
            format!(
                "migoto importer overridden by preset: preset={}, configured={}, effective={}",
                target.game_preset, configured_migoto_importer, migoto_importer
            ),
        );
    }

    let (run_exe, extra_args) = if migoto_runtime_required {
        let bridge_exe = target
            .game_config_data
            .as_ref()
            .and_then(|c| c.pointer("/other/migoto/bridge_exe_path"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| app_data_dir.join("Windows").join("ssmt4-bridge.exe"));
        if !bridge_exe.exists() {
            return Err(format!(
                "3DMigoto 已启用但桥接程序未找到: {}。请在设置中配置正确的 Bridge 可执行文件路径，或构建 ssmt4-bridge.exe。",
                bridge_exe.display()
            ));
        }

        let game_folder_linux = target.game_root.to_string_lossy().to_string();
        let game_exe_name = target
            .launch_exe
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("game.exe")
            .to_string();

        let mut bridge_config = super::super::bridge::build_bridge_config(
            &migoto_importer,
            &app_data_dir,
            &game_folder_linux,
            &game_exe_name,
            target.game_config_data.as_ref(),
        );

        if target.game_preset == "ArknightsEndfield" {
            if let Some(chain) = super::find_endfield_launcher_chain(&target.launch_exe) {
                let game_root = chain.endfield_exe.parent().ok_or_else(|| {
                    format!("无法推断终末地主程序目录: {}", chain.endfield_exe.display())
                })?;
                let target_exe_name = chain
                    .endfield_exe
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Endfield.exe")
                    .to_string();

                bridge_config.paths.game_folder =
                    super::super::bridge::linux_to_wine_path(&game_root.to_string_lossy());
                bridge_config.paths.game_exe = target_exe_name.clone();
                bridge_config.game.start_exe = target_exe_name.clone();
                bridge_config.game.work_dir =
                    super::super::bridge::linux_to_wine_path(&game_root.to_string_lossy());
                bridge_config.game.process_name = target_exe_name.clone();

                info!(
                    "ArknightsEndfield EFMI 对齐原版 XXMI: start_exe={}, launcher={}",
                    chain.endfield_exe.display(),
                    chain.launcher_exe.display()
                );
                super::append_game_log(
                    game_name,
                    "INFO",
                    "bridge",
                    format!(
                        "Endfield EFMI aligned to original XXMI flow: start_exe={}, process_name={}, start_args={:?}, launcher={}",
                        chain.endfield_exe.display(),
                        target_exe_name,
                        bridge_config.game.start_args,
                        chain.launcher_exe.display()
                    ),
                );
            }
        }

        if let Some(ref jade) = jadeite_exe {
            let jade_wine = super::super::bridge::linux_to_wine_path(&jade.to_string_lossy());
            bridge_config.jadeite = super::super::bridge::BridgeJadeite {
                enabled: true,
                exe_path: jade_wine,
            };
            info!(
                "3DMigoto + Jadeite: hook 注入模式，通过 {} 启动游戏",
                jade.display()
            );
        }

        let config_path = super::super::bridge::write_bridge_config(&bridge_config, &app_data_dir)?;
        let config_wine_path =
            super::super::bridge::linux_to_wine_path(&config_path.to_string_lossy());

        if !env.contains_key("DXVK_ASYNC") {
            env.insert("DXVK_ASYNC".to_string(), "1".to_string());
            info!("3DMigoto: 自动设置 DXVK_ASYNC=1 (异步着色器编译)");
        }

        info!(
            "3DMigoto 启用: importer={}, bridge={}, config={}",
            migoto_importer,
            bridge_exe.display(),
            config_path.display()
        );
        super::append_game_log(
            game_name,
            "INFO",
            "bridge",
            format!(
                "3DMigoto enabled: importer={}, bridge_exe={}, config={}",
                migoto_importer,
                bridge_exe.display(),
                config_path.display()
            ),
        );

        (bridge_exe, vec!["--config".to_string(), config_wine_path])
    } else if let Some(ref jade) = jadeite_exe {
        info!("使用 jadeite 反作弊补丁: {}", jade.display());
        let win_game_path = format!(
            "Z:{}",
            target.launch_exe.to_string_lossy().replace('/', "\\")
        );
        (jade.clone(), vec![win_game_path, "--".to_string()])
    } else {
        if is_hoyoverse {
            warn!("未找到 jadeite.exe，HoYoverse 游戏可能因反作弊而无法启动");
        }
        (target.launch_exe.clone(), vec![])
    };

    let runner_exe_path = run_exe.to_string_lossy().to_string();
    info!(
        "启动可执行文件: 配置路径={}, 识别主程序={}, 实际执行器={}",
        target.configured_exe_path, target.launch_exe_path, runner_exe_path
    );
    super::append_game_log(
        game_name,
        "INFO",
        "session",
        format!(
            "target executable: configured={}, launch_exe={}, runner_exe={}",
            target.configured_exe_path, target.launch_exe_path, runner_exe_path
        ),
    );
    if !extra_args.is_empty() {
        info!("启动附加参数: {:?}", extra_args);
        super::append_game_log(
            game_name,
            "INFO",
            "session",
            format!("extra args: {:?}", extra_args),
        );
    }

    Ok(ResolvedRunTarget {
        run_exe,
        runner_exe_path,
        extra_args,
        used_bridge: migoto_runtime_required,
    })
}

pub(super) fn prepare_launch_command(
    game_name: &str,
    target: &ResolvedLaunchTarget,
    runtime: &PrefixRuntimeContext,
    env: HashMap<String, String>,
    run_target: &ResolvedRunTarget,
) -> Result<PreparedLaunchCommand, String> {
    let settings = &runtime.settings;
    let force_direct_proton = target
        .preset_meta
        .map(|p| p.force_direct_proton)
        .unwrap_or(false);
    let effective_use_pressure_vessel = if target
        .preset_meta
        .map(|p| p.force_disable_pressure_vessel)
        .unwrap_or(false)
    {
        if settings.use_pressure_vessel {
            warn!("当前预设要求禁用 pressure-vessel，已忽略该设置");
        }
        false
    } else {
        settings.use_pressure_vessel
    };

    let default_runner = if force_direct_proton {
        LaunchRunner::Proton
    } else if settings.use_umu_run {
        LaunchRunner::UmuRun
    } else if effective_use_pressure_vessel {
        LaunchRunner::PressureVessel
    } else {
        LaunchRunner::Proton
    };

    let mut launch_profile = LaunchProfile {
        runner: default_runner,
        env,
        args: Vec::new(),
        working_dir: target
            .launch_exe
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        prefix_path: runtime.prefix_dir.to_string_lossy().to_string(),
        proton_path: runtime.proton_path.to_string_lossy().to_string(),
        runtime_flags: LaunchRuntimeFlags {
            sandbox_enabled: settings.sandbox_enabled,
            sandbox_isolate_home: settings.sandbox_isolate_home,
            force_direct_proton,
            use_pressure_vessel: effective_use_pressure_vessel,
            region: target.launch_region.clone(),
        },
    };

    apply_launch_profile_chain(
        &mut launch_profile,
        target.game_config_data.as_ref(),
        &target.launch_region,
    )?;

    let effective_proton_path = normalize_non_empty(&launch_profile.proton_path)
        .map(PathBuf::from)
        .unwrap_or_else(|| runtime.proton_path.clone());
    if should_force_umu_runner(
        &target.game_preset,
        &effective_proton_path,
        launch_profile.runner.clone(),
        launch_profile.runtime_flags.force_direct_proton,
    ) {
        if launch_profile.runner != LaunchRunner::UmuRun {
            info!(
                "ArknightsEndfield 运行时覆盖: 强制使用 umu-run 以对齐 Lutris Proton 启动链"
            );
            super::append_game_log(
                game_name,
                "INFO",
                "runtime",
                "runtime override: forcing umu-run for ArknightsEndfield to align with Lutris Proton launch path",
            );
        }
        launch_profile.runner = LaunchRunner::UmuRun;
        launch_profile.runtime_flags.use_pressure_vessel = false;
    }

    if target
        .preset_meta
        .map(|p| p.force_disable_pressure_vessel)
        .unwrap_or(false)
        && launch_profile.runtime_flags.use_pressure_vessel
    {
        warn!("当前预设要求禁用 pressure-vessel，已覆盖 LaunchProfile 设置");
        launch_profile.runtime_flags.use_pressure_vessel = false;
        if launch_profile.runner == LaunchRunner::PressureVessel {
            launch_profile.runner = LaunchRunner::Proton;
        }
    }

    if launch_profile.runtime_flags.force_direct_proton
        && launch_profile.runner == LaunchRunner::UmuRun
    {
        warn!("LaunchProfile 配置为 forceDirectProton，runner 已从 umu_run 回退为 proton");
        launch_profile.runner = if launch_profile.runtime_flags.use_pressure_vessel {
            LaunchRunner::PressureVessel
        } else {
            LaunchRunner::Proton
        };
    }

    if launch_profile.runtime_flags.region.trim().is_empty() {
        launch_profile.runtime_flags.region = target.launch_region.clone();
    }

    super::append_game_log(
        game_name,
        "DEBUG",
        "host-env",
        "---- host environment begin ----",
    );
    super::append_host_env_snapshot(game_name);
    super::append_game_log(
        game_name,
        "DEBUG",
        "host-env",
        "---- host environment end ----",
    );

    let command_spec = resolve_launch_command(
        &target.game_preset,
        settings,
        target.preset_meta,
        &runtime.proton_path,
        &run_target.run_exe,
        &run_target.extra_args,
        &mut launch_profile,
    )?;

    let runner_name = command_spec.runner.as_str().to_string();
    let command_program_path = command_spec.program.to_string_lossy().to_string();
    super::append_game_log(
        game_name,
        "DEBUG",
        "launch-env",
        "---- launch environment begin ----",
    );
    super::append_sorted_env_snapshot(game_name, "launch-env", &launch_profile.env);
    super::append_game_log(
        game_name,
        "DEBUG",
        "launch-env",
        "---- launch environment end ----",
    );
    super::append_game_log(
        game_name,
        "DEBUG",
        "launcher",
        format!(
            "runner={}, program={}, args={:?}, use_umu_runtime={}, effective_prefix_dir={}",
            runner_name,
            super::sanitize_path_for_log(&command_spec.program),
            command_spec
                .args
                .iter()
                .map(|arg| super::sanitize_arg_for_log(arg))
                .collect::<Vec<_>>(),
            command_spec.use_umu_runtime,
            super::sanitize_path_for_log(&command_spec.effective_prefix_dir)
        ),
    );
    debug!(
        "启动命令解析: runner={}, program={}, args={:?}, use_umu_runtime={}, effective_prefix_dir={}",
        runner_name,
        command_program_path,
        command_spec.args,
        command_spec.use_umu_runtime,
        command_spec.effective_prefix_dir.display()
    );
    info!(
        "最终启动配置: runner={}, sandbox={}, pressureVessel={}, workingDir={}, commandProgram={}",
        runner_name,
        launch_profile.runtime_flags.sandbox_enabled,
        launch_profile.runtime_flags.use_pressure_vessel,
        launch_profile.working_dir,
        command_program_path
    );
    super::append_game_log(
        game_name,
        "INFO",
        "session",
        format!(
            "launch command: {} {}",
            command_program_path,
            command_spec.args.join(" ")
        ),
    );

    let required_env_keys = [
        "WINEPREFIX",
        "STEAM_COMPAT_DATA_PATH",
        "STEAM_COMPAT_INSTALL_PATH",
        "STEAM_COMPAT_TOOL_PATHS",
        "STEAM_PROTON_PATH",
    ];
    let missing_env_keys: Vec<&str> = required_env_keys
        .iter()
        .copied()
        .filter(|k| {
            launch_profile
                .env
                .get(*k)
                .map(|v| v.trim().is_empty())
                .unwrap_or(true)
        })
        .collect();
    if missing_env_keys.is_empty() {
        debug!("启动环境变量检查通过: required={:?}", required_env_keys);
        super::append_game_log(
            game_name,
            "DEBUG",
            "launcher",
            format!("required env ok: {:?}", required_env_keys),
        );
    } else {
        warn!(
            "启动环境变量缺失，可能导致启动异常: missing={:?}",
            missing_env_keys
        );
        super::append_game_log(
            game_name,
            "WARN",
            "launcher",
            format!("missing required env: {:?}", missing_env_keys),
        );
    }

    Ok(PreparedLaunchCommand {
        launch_profile,
        command_spec,
        runner_name,
        command_program_path,
        runner_exe_path: run_target.runner_exe_path.clone(),
        used_bridge: run_target.used_bridge,
    })
}

pub(super) fn apply_launch_profile_chain(
    launch_profile: &mut LaunchProfile,
    game_config_data: Option<&Value>,
    region: &str,
) -> Result<(), String> {
    if let Some(global_patch) = load_global_launch_profile_patch() {
        apply_launch_profile_patch(launch_profile, global_patch);
    }

    if let Some(config) = game_config_data {
        if let Some(base_patch) = extract_launch_profile_patch(config, None) {
            apply_launch_profile_patch(launch_profile, base_patch);
        }
        if let Some(region_patch) = extract_launch_profile_patch(config, Some(region)) {
            apply_launch_profile_patch(launch_profile, region_patch);
        }
    }

    Ok(())
}

fn load_global_launch_profile_patch() -> Option<LaunchProfilePatch> {
    for key in [
        "launch_profile_default",
        "launchProfileDefault",
        "launch_profile_global",
    ] {
        let Some(raw) = db::read_setting_value(key) else {
            continue;
        };
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<LaunchProfilePatch>(trimmed) {
            Ok(patch) => return Some(patch),
            Err(err) => warn!("解析全局 LaunchProfile 失败 ({}): {}", key, err),
        }
    }
    None
}

pub(super) fn extract_launch_profile_patch(
    config: &Value,
    region: Option<&str>,
) -> Option<LaunchProfilePatch> {
    let mut candidates = Vec::new();

    match region.and_then(normalize_non_empty) {
        Some(region_key) => {
            if let Some(v) =
                lookup_region_profile(config.pointer("/other/launchProfiles"), &region_key)
            {
                candidates.push(v);
            }
            if let Some(v) = lookup_region_profile(config.pointer("/launchProfiles"), &region_key) {
                candidates.push(v);
            }
        }
        None => {
            if let Some(v) = config.pointer("/other/launchProfile") {
                candidates.push(v);
            }
            if let Some(v) = config.pointer("/launchProfile") {
                candidates.push(v);
            }
            if let Some(v) =
                lookup_region_profile(config.pointer("/other/launchProfiles"), "default")
            {
                candidates.push(v);
            }
            if let Some(v) = lookup_region_profile(config.pointer("/launchProfiles"), "default") {
                candidates.push(v);
            }
        }
    }

    for candidate in candidates {
        match serde_json::from_value::<LaunchProfilePatch>(candidate.clone()) {
            Ok(patch) => return Some(patch),
            Err(err) => warn!("解析 LaunchProfile Patch 失败: {}", err),
        }
    }

    None
}

fn lookup_region_profile<'a>(profiles: Option<&'a Value>, region: &str) -> Option<&'a Value> {
    let map = profiles?.as_object()?;
    let normalized = region.trim();
    if normalized.is_empty() {
        return None;
    }

    let mut keys = Vec::new();
    keys.push(normalized.to_string());
    keys.push(normalized.to_ascii_lowercase());
    keys.push(normalized.to_ascii_uppercase());

    let underscore = normalized.replace('-', "_");
    if underscore != normalized {
        keys.push(underscore.clone());
        keys.push(underscore.to_ascii_lowercase());
        keys.push(underscore.to_ascii_uppercase());
    }

    for key in keys {
        if let Some(value) = map.get(&key) {
            return Some(value);
        }
    }

    None
}

pub(super) fn apply_launch_profile_patch(profile: &mut LaunchProfile, patch: LaunchProfilePatch) {
    if let Some(runner) = patch.runner.as_deref().and_then(parse_launch_runner) {
        profile.runner = runner;
    }

    if let Some(env_patch) = patch.env {
        for (key, value) in env_patch {
            if !key.trim().is_empty() {
                profile.env.insert(key, value);
            }
        }
    }

    if let Some(args) = patch.args {
        profile.args = args
            .into_iter()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .collect();
    }

    if let Some(working_dir) = patch.working_dir.and_then(|v| normalize_non_empty(&v)) {
        profile.working_dir = working_dir;
    }
    if let Some(prefix_path) = patch.prefix_path.and_then(|v| normalize_non_empty(&v)) {
        profile.prefix_path = prefix_path;
    }
    if let Some(proton_path) = patch.proton_path.and_then(|v| normalize_non_empty(&v)) {
        profile.proton_path = proton_path;
    }

    if let Some(sandbox_enabled) = patch.runtime_flags.sandbox_enabled {
        profile.runtime_flags.sandbox_enabled = sandbox_enabled;
    }
    if let Some(sandbox_isolate_home) = patch.runtime_flags.sandbox_isolate_home {
        profile.runtime_flags.sandbox_isolate_home = sandbox_isolate_home;
    }
    if let Some(force_direct_proton) = patch.runtime_flags.force_direct_proton {
        profile.runtime_flags.force_direct_proton = force_direct_proton;
    }
    if let Some(use_pressure_vessel) = patch.runtime_flags.use_pressure_vessel {
        profile.runtime_flags.use_pressure_vessel = use_pressure_vessel;
    }
}

pub(super) fn parse_launch_runner(raw: &str) -> Option<LaunchRunner> {
    let normalized = raw.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "umu_run" | "umu" => Some(LaunchRunner::UmuRun),
        "proton" | "direct_proton" => Some(LaunchRunner::Proton),
        "wine" => Some(LaunchRunner::Wine),
        "pressure_vessel" | "steam_runtime" => Some(LaunchRunner::PressureVessel),
        _ => None,
    }
}

fn resolve_launch_command(
    game_preset: &str,
    settings: &crate::configs::wine_config::ProtonSettings,
    preset_meta: Option<&crate::configs::game_presets::GamePreset>,
    default_proton_path: &Path,
    run_exe: &Path,
    extra_args: &[String],
    launch_profile: &mut LaunchProfile,
) -> Result<LaunchCommandSpec, String> {
    let proton_path = normalize_non_empty(&launch_profile.proton_path)
        .map(PathBuf::from)
        .unwrap_or_else(|| default_proton_path.to_path_buf());
    let effective_prefix_dir = normalize_non_empty(&launch_profile.prefix_path)
        .map(PathBuf::from)
        .unwrap_or_else(|| prefix::get_prefix_dir(game_preset));

    let mut merged_args = extra_args.to_vec();
    merged_args.extend_from_slice(&launch_profile.args);

    if launch_profile.runtime_flags.force_direct_proton
        && launch_profile.runner == LaunchRunner::UmuRun
    {
        launch_profile.runner = if launch_profile.runtime_flags.use_pressure_vessel {
            LaunchRunner::PressureVessel
        } else {
            LaunchRunner::Proton
        };
    }

    let mut runner = launch_profile.runner.clone();
    let mut use_umu_runtime = false;
    let (program, args) = match runner {
        LaunchRunner::UmuRun => {
            if let Some(umu_run) = super::runtime_env::find_umu_run_binary() {
                super::runtime_env::apply_umu_env_defaults(
                    game_preset,
                    &proton_path,
                    settings,
                    preset_meta,
                    &mut launch_profile.env,
                );
                use_umu_runtime = true;
                let mut args = vec![run_exe.to_string_lossy().to_string()];
                args.extend(merged_args.clone());
                (umu_run, args)
            } else {
                warn!("已启用 umu-run，但系统未找到 umu-run，回退到 Proton 启动链");
                runner = if launch_profile.runtime_flags.use_pressure_vessel {
                    LaunchRunner::PressureVessel
                } else {
                    LaunchRunner::Proton
                };
                let cmd = build_proton_base_command(
                    game_preset,
                    launch_profile.runtime_flags.use_pressure_vessel,
                    &proton_path,
                    run_exe,
                    &merged_args,
                );
                if launch_profile.runtime_flags.use_pressure_vessel && cmd.0 == proton_path {
                    runner = LaunchRunner::Proton;
                    launch_profile.runtime_flags.use_pressure_vessel = false;
                }
                cmd
            }
        }
        LaunchRunner::PressureVessel => {
            if let Some(cmd) = build_pressure_vessel_command(&proton_path, run_exe, &merged_args) {
                (cmd.0, cmd.1)
            } else {
                warn!("未找到 Steam Linux Runtime，pressure-vessel runner 回退到直连 Proton");
                runner = LaunchRunner::Proton;
                launch_profile.runtime_flags.use_pressure_vessel = false;
                build_direct_proton_command_spec_with_args(
                    game_preset,
                    &proton_path,
                    run_exe,
                    &merged_args,
                )
            }
        }
        LaunchRunner::Wine => resolve_wine_command(&proton_path, run_exe, &merged_args)?,
        LaunchRunner::Proton => {
            let cmd = build_proton_base_command(
                game_preset,
                launch_profile.runtime_flags.use_pressure_vessel,
                &proton_path,
                run_exe,
                &merged_args,
            );
            if launch_profile.runtime_flags.use_pressure_vessel && cmd.0 != proton_path {
                runner = LaunchRunner::PressureVessel;
            } else if launch_profile.runtime_flags.use_pressure_vessel {
                launch_profile.runtime_flags.use_pressure_vessel = false;
            }
            cmd
        }
    };

    Ok(LaunchCommandSpec {
        runner,
        program,
        args,
        use_umu_runtime,
        effective_prefix_dir,
    })
}

fn resolve_wine_command(
    selected_path: &Path,
    run_exe: &Path,
    extra_args: &[String],
) -> Result<(PathBuf, Vec<String>), String> {
    let wine_binary = resolve_wine_binary(selected_path)
        .ok_or_else(|| format!("无法找到 Wine 可执行文件: {}", selected_path.display()))?;
    let mut args = vec![run_exe.to_string_lossy().to_string()];
    args.extend_from_slice(extra_args);
    Ok((wine_binary, args))
}

fn resolve_wine_binary(selected_path: &Path) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if selected_path.is_file() {
        candidates.push(selected_path.to_path_buf());
    }
    if let Some(parent) = selected_path.parent() {
        candidates.push(parent.join("bin/wine64"));
        candidates.push(parent.join("bin/wine"));
        candidates.push(parent.join("files/bin/wine64"));
        candidates.push(parent.join("files/bin/wine"));
    }
    candidates.push(PathBuf::from("/usr/bin/wine64"));
    candidates.push(PathBuf::from("/usr/bin/wine"));
    if let Ok(path) = which::which("wine64") {
        candidates.push(path);
    }
    if let Ok(path) = which::which("wine") {
        candidates.push(path);
    }

    candidates.into_iter().find(|path| path.exists())
}

fn build_pressure_vessel_command(
    proton_path: &Path,
    run_exe: &Path,
    extra_args: &[String],
) -> Option<(PathBuf, Vec<String>)> {
    let runtime_dir = detector::find_steam_linux_runtime()?;
    let entry_point = runtime_dir.join("_v2-entry-point");
    if !entry_point.exists() {
        return None;
    }
    let mut args = vec![
        "--verb=waitforexitandrun".to_string(),
        "--".to_string(),
        proton_path.to_string_lossy().to_string(),
        "waitforexitandrun".to_string(),
        run_exe.to_string_lossy().to_string(),
    ];
    args.extend_from_slice(extra_args);
    Some((entry_point, args))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProtonLaunchVerb {
    Run,
    WaitForExitAndRun,
}

impl ProtonLaunchVerb {
    fn as_proton_arg(self) -> &'static str {
        match self {
            Self::Run => "run",
            Self::WaitForExitAndRun => "waitforexitandrun",
        }
    }
}

fn resolve_direct_proton_launch_verb(game_preset: &str, run_exe: &Path) -> ProtonLaunchVerb {
    if game_preset == "ArknightsEndfield"
        && run_exe
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("Launcher.exe"))
    {
        return ProtonLaunchVerb::Run;
    }

    ProtonLaunchVerb::WaitForExitAndRun
}

pub(super) fn normalize_non_empty(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn build_proton_base_command(
    game_preset: &str,
    use_pressure_vessel: bool,
    proton_path: &Path,
    run_exe: &Path,
    extra_args: &[String],
) -> (PathBuf, Vec<String>) {
    if use_pressure_vessel {
        if let Some(runtime_dir) = detector::find_steam_linux_runtime() {
            let entry_point = runtime_dir.join("_v2-entry-point");
            debug!(
                "Launching with pressure-vessel: {} -> {} -> {}",
                entry_point.display(),
                proton_path.display(),
                run_exe.display()
            );
            let mut args = vec![
                "--verb=waitforexitandrun".to_string(),
                "--".to_string(),
                proton_path.to_string_lossy().to_string(),
                "waitforexitandrun".to_string(),
                run_exe.to_string_lossy().to_string(),
            ];
            args.extend_from_slice(extra_args);
            return (entry_point, args);
        }

        warn!("SteamLinuxRuntime not found, falling back to direct proton launch");
    }

    build_direct_proton_command_spec_with_args(game_preset, proton_path, run_exe, extra_args)
}

fn build_direct_proton_command_spec_with_args(
    game_preset: &str,
    proton_path: &Path,
    run_exe: &Path,
    extra_args: &[String],
) -> (PathBuf, Vec<String>) {
    let verb = resolve_direct_proton_launch_verb(game_preset, run_exe);
    debug!(
        "Launching with direct proton: {} {} {} {:?}",
        proton_path.display(),
        verb.as_proton_arg(),
        run_exe.display(),
        extra_args
    );
    let mut args = vec![
        verb.as_proton_arg().to_string(),
        run_exe.to_string_lossy().to_string(),
    ];
    args.extend_from_slice(extra_args);
    (proton_path.to_path_buf(), args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::bridge::{
        BridgeMigotoConfig,
    };

    #[test]
    fn endfield_launcher_uses_run_verb_for_direct_proton() {
        let verb = resolve_direct_proton_launch_verb(
            "ArknightsEndfield",
            Path::new("/tmp/Hypergryph Launcher/Launcher.exe"),
        );
        assert_eq!(verb, ProtonLaunchVerb::Run);

        let (_program, args) = build_direct_proton_command_spec_with_args(
            "ArknightsEndfield",
            Path::new("/tmp/proton"),
            Path::new("/tmp/Hypergryph Launcher/Launcher.exe"),
            &[],
        );
        assert_eq!(args.first().map(String::as_str), Some("run"));
    }

    #[test]
    fn regular_games_keep_waitforexitandrun_verb() {
        let verb = resolve_direct_proton_launch_verb(
            "HonkaiStarRail",
            Path::new("/tmp/HonkaiStarRail/StarRail.exe"),
        );
        assert_eq!(verb, ProtonLaunchVerb::WaitForExitAndRun);
    }

    #[test]
    fn endfield_keeps_original_efmi_direct_injection_defaults() {
        let migoto = BridgeMigotoConfig {
            use_hook: false,
            use_dll_drop: false,
            enforce_rendering: false,
            enable_hunting: false,
            dump_shaders: false,
            mute_warnings: true,
            calls_logging: false,
            debug_logging: false,
            unsafe_mode: false,
            xxmi_dll_init_delay: 0,
        };

        assert!(!migoto.use_hook);
        assert_eq!(migoto.xxmi_dll_init_delay, 0);
    }

    #[test]
    fn endfield_prefers_umu_runner_when_proton_and_available() {
        assert!(should_force_umu_runner_with_availability(
            "ArknightsEndfield",
            Path::new("/tmp/dwproton-10.0-18/proton"),
            LaunchRunner::Proton,
            false,
            true,
        ));
        assert!(should_force_umu_runner_with_availability(
            "ArknightsEndfield",
            Path::new("/tmp/dwproton-10.0-18/proton"),
            LaunchRunner::UmuRun,
            false,
            true,
        ));
    }

    #[test]
    fn endfield_does_not_force_umu_when_unavailable_or_not_proton() {
        assert!(!should_force_umu_runner_with_availability(
            "ArknightsEndfield",
            Path::new("/tmp/dwproton-10.0-18/proton"),
            LaunchRunner::Proton,
            false,
            false,
        ));
        assert!(!should_force_umu_runner_with_availability(
            "ArknightsEndfield",
            Path::new("/tmp/wine/bin/wine"),
            LaunchRunner::Proton,
            false,
            true,
        ));
        assert!(!should_force_umu_runner_with_availability(
            "ArknightsEndfield",
            Path::new("/tmp/dwproton-10.0-18/proton"),
            LaunchRunner::Wine,
            false,
            true,
        ));
    }
}

pub(super) fn build_launch_process_command(
    launch_profile: &LaunchProfile,
    command_spec: &LaunchCommandSpec,
    launch_exe: &Path,
) -> Result<tokio::process::Command, String> {
    let mut cmd = if launch_profile.runtime_flags.sandbox_enabled && !command_spec.use_umu_runtime {
        info!(
            "Launching with bwrap sandbox (isolate_home={})",
            launch_profile.runtime_flags.sandbox_isolate_home
        );
        build_bwrap_command(
            &command_spec.program,
            &command_spec.args,
            launch_exe,
            &command_spec.effective_prefix_dir,
            launch_profile.runtime_flags.sandbox_isolate_home,
            &launch_profile.env,
        )?
    } else {
        if launch_profile.runtime_flags.sandbox_enabled && command_spec.use_umu_runtime {
            warn!("umu-run 已启用，跳过额外 bwrap 沙盒以避免容器嵌套冲突");
        }
        let mut command = tokio::process::Command::new(&command_spec.program);
        command.args(&command_spec.args);
        command
    };

    cmd.envs(&launch_profile.env);
    Ok(cmd)
}

pub(super) fn apply_launch_working_dir(
    cmd: &mut tokio::process::Command,
    launch_profile: &LaunchProfile,
    launch_exe: &Path,
) {
    if !launch_profile.working_dir.trim().is_empty() {
        let wd = PathBuf::from(launch_profile.working_dir.trim());
        if wd.exists() {
            cmd.current_dir(wd);
            return;
        }

        warn!(
            "LaunchProfile workingDir 不存在，回退到 exe 目录: {}",
            launch_profile.working_dir
        );
    }

    if let Some(game_dir) = launch_exe.parent() {
        cmd.current_dir(game_dir);
    }
}

fn build_bwrap_command(
    base_program: &Path,
    base_args: &[String],
    game_exe: &Path,
    prefix_dir: &Path,
    isolate_home: bool,
    env: &HashMap<String, String>,
) -> Result<tokio::process::Command, String> {
    let bwrap_path = which::which("bwrap")
        .map_err(|_| "Sandbox enabled but 'bwrap' command is not available".to_string())?;

    let mut cmd = tokio::process::Command::new(bwrap_path);
    cmd.arg("--die-with-parent")
        .arg("--new-session")
        .arg("--ro-bind")
        .arg("/")
        .arg("/")
        .arg("--dev")
        .arg("/dev")
        .arg("--proc")
        .arg("/proc")
        .arg("--tmpfs")
        .arg("/tmp")
        .arg("--tmpfs")
        .arg("/var/tmp");

    let mut rw_bound = HashSet::new();
    let mut ro_bound = HashSet::new();
    bind_rw_path(&mut cmd, prefix_dir, &mut rw_bound)?;

    if let Some(game_dir) = game_exe.parent() {
        bind_rw_path(&mut cmd, game_dir, &mut rw_bound)?;
    }

    if isolate_home {
        let sandbox_home = prefix_dir.join("sandbox-home");
        std::fs::create_dir_all(&sandbox_home)
            .map_err(|e| format!("Failed to create sandbox home: {}", e))?;
        bind_rw_path(&mut cmd, &sandbox_home, &mut rw_bound)?;
        cmd.arg("--setenv")
            .arg("HOME")
            .arg(sandbox_home.to_string_lossy().to_string());
    } else if let Ok(home) = std::env::var("HOME") {
        bind_rw_path(&mut cmd, Path::new(&home), &mut rw_bound)?;
    }

    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        bind_rw_path(&mut cmd, Path::new(&runtime_dir), &mut rw_bound)?;
    }

    if let Ok(xauthority) = std::env::var("XAUTHORITY") {
        bind_ro_path(&mut cmd, Path::new(&xauthority), &mut ro_bound)?;
    }

    if let Some(steam_root) = env.get("STEAM_COMPAT_CLIENT_INSTALL_PATH") {
        bind_ro_path(&mut cmd, Path::new(steam_root), &mut ro_bound)?;
    }

    bind_ro_path(&mut cmd, Path::new("/tmp/.X11-unix"), &mut ro_bound)?;

    for key in [
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "XAUTHORITY",
        "XDG_RUNTIME_DIR",
        "PULSE_SERVER",
        "DBUS_SESSION_BUS_ADDRESS",
        "LANG",
        "LC_ALL",
    ] {
        if let Ok(value) = std::env::var(key) {
            cmd.arg("--setenv").arg(key).arg(value);
        }
    }

    cmd.arg("--")
        .arg(base_program.to_string_lossy().to_string())
        .args(base_args);

    Ok(cmd)
}

fn bind_rw_path(
    cmd: &mut tokio::process::Command,
    path: &Path,
    seen: &mut HashSet<PathBuf>,
) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if !seen.insert(canonical.clone()) {
        return Ok(());
    }
    let p = canonical.to_string_lossy().to_string();
    cmd.arg("--bind").arg(&p).arg(&p);
    Ok(())
}

fn bind_ro_path(
    cmd: &mut tokio::process::Command,
    path: &Path,
    seen: &mut HashSet<PathBuf>,
) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if !seen.insert(canonical.clone()) {
        return Ok(());
    }
    let p = canonical.to_string_lossy().to_string();
    cmd.arg("--ro-bind").arg(&p).arg(&p);
    Ok(())
}
