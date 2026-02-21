use crate::configs::database as db;
use crate::process_monitor;
use crate::wine::{detector, prefix};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tauri::Emitter;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tracing::{error, info, warn};

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

#[tauri::command]
pub async fn start_game(
    app: tauri::AppHandle,
    game_name: String,
    game_exe_path: String,
    wine_version_id: String,
) -> Result<String, String> {
    start_game_internal(app, game_name, game_exe_path, wine_version_id, None).await
}

#[tauri::command]
pub async fn launch_game(
    app: tauri::AppHandle,
    game_name: String,
    region: Option<String>,
) -> Result<String, String> {
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

    start_game_internal(app, game_name, game_exe_path, wine_version_id, region).await
}

async fn start_game_internal(
    app: tauri::AppHandle,
    game_name: String,
    game_exe_path: String,
    wine_version_id: String,
    region_override: Option<String>,
) -> Result<String, String> {
    let game_name = crate::configs::game_identity::to_canonical_or_keep(&game_name);
    let _launch_guard = process_monitor::acquire_launch_guard(&game_name)?;

    // 检查游戏是否已在运行
    if process_monitor::is_game_running(&game_name).await {
        warn!("游戏 {} 已在运行，拒绝重复启动", game_name);
        return Err("游戏已在运行中，请勿重复启动".to_string());
    }

    // 清理已结束的进程记录
    process_monitor::cleanup_stale_processes().await;

    let game_exe = PathBuf::from(&game_exe_path);
    if !game_exe.exists() {
        return Err(format!("Game executable not found: {}", game_exe_path));
    }

    if !is_tos_risk_acknowledged() {
        return Err(
            "未完成风险确认，禁止启动。请先在首次向导完成风险确认后再启动游戏。".to_string(),
        );
    }

    let game_config_data = load_game_config_json(&game_name);
    let game_preset = resolve_game_preset_with_data(&game_name, game_config_data.as_ref());
    let preset_meta = crate::configs::game_presets::get_preset(&game_preset);
    let launch_launcher_api = resolve_launch_launcher_api(game_config_data.as_ref());
    let launch_biz_prefix = resolve_launch_biz_prefix(
        game_config_data.as_ref(),
        preset_meta,
        launch_launcher_api.as_deref(),
    );
    let launch_region = resolve_launch_region(
        game_config_data.as_ref(),
        preset_meta,
        region_override.as_deref(),
    );
    let write_scope_region = process_monitor::derive_region_scope(
        launch_launcher_api.as_deref(),
        launch_biz_prefix.as_deref(),
        Some(&launch_region),
    );
    info!(
        "启动目标: game={}, preset={}, region={}, write_scope_region={}",
        game_name, game_preset, launch_region, write_scope_region
    );
    let launch_exe = resolve_preferred_launch_exe(&game_preset, &game_exe);
    let game_root = infer_game_root_from_exe(&launch_exe)
        .ok_or_else(|| format!("无法从可执行文件推断游戏目录: {}", game_exe_path))?;
    let game_root_str = game_root.to_string_lossy().to_string();
    let write_guard =
        process_monitor::acquire_game_write_guard(&game_root, &write_scope_region, "launch")?;

    let mut protection_status = crate::commands::telemetry::check_game_protection_status_internal(
        &game_preset,
        Some(&game_root_str),
    )?;
    let mut protection_required = protection_status
        .get("enforceAtLaunch")
        .and_then(|v| v.as_bool())
        .or_else(|| protection_status.get("supported").and_then(|v| v.as_bool()))
        .unwrap_or(false);
    let mut protection_enabled = protection_status
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    if !protection_required && game_preset != game_name {
        let fallback_status = crate::commands::telemetry::check_game_protection_status_internal(
            &game_name,
            Some(&game_root_str),
        )?;
        let fallback_required = fallback_status
            .get("enforceAtLaunch")
            .and_then(|v| v.as_bool())
            .or_else(|| fallback_status.get("supported").and_then(|v| v.as_bool()))
            .unwrap_or(false);
        if fallback_required {
            info!(
                "防护判定已从 preset={} 回退到 game_name={}",
                game_preset, game_name
            );
            protection_status = fallback_status;
            protection_required = fallback_required;
            protection_enabled = protection_status
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
        }
    }
    if let Some(channel) = protection_status.get("channel") {
        let mode = channel
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("n/a");
        let current = channel
            .get("currentValue")
            .and_then(|v| v.as_i64())
            .map(|v| v.to_string())
            .unwrap_or_else(|| "n/a".to_string());
        let expected = channel
            .get("expectedValue")
            .and_then(|v| v.as_i64())
            .map(|v| v.to_string())
            .unwrap_or_else(|| "n/a".to_string());
        let enforcement = channel
            .get("launchEnforcement")
            .and_then(|v| v.as_str())
            .unwrap_or("n/a");
        info!(
            "Channel mode={}, current={}, expected={}, enforcement={}",
            mode, current, expected, enforcement
        );
    }
    if !protection_required {
        let blocked_domains: Vec<String> = protection_status
            .pointer("/telemetry/blocked")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !blocked_domains.is_empty() {
            warn!(
                "检测到当前游戏已屏蔽域名（{}）。该游戏防护非必需，此设置可能导致联网异常，建议恢复防护后重试",
                blocked_domains.join(", ")
            );
        }
    }

    if protection_required && !protection_enabled {
        let missing_items = protection_status
            .get("missing")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join("；")
            })
            .unwrap_or_default();

        let detail = if missing_items.is_empty() {
            String::new()
        } else {
            format!(" 详情：{}", missing_items)
        };

        return Err(format!(
            "未启用应用防护，已阻止启动。请先在“下载/安装游戏”中应用安全防护。{}",
            detail
        ));
    }

    // Load prefix config（不存在则自动创建）
    let prefix_dir = prefix::get_prefix_dir(&game_name);
    info!("prefix 路径: {}", prefix_dir.display());
    let prefix_config = match prefix::load_prefix_config(&game_name) {
        Ok(cfg) => {
            info!(
                "已加载 prefix 配置: steam_deck_compat={}, steamos_compat={}, use_umu_run={}, custom_env={:?}, use_pressure_vessel={}",
                cfg.proton_settings.steam_deck_compat,
                cfg.proton_settings.steamos_compat,
                cfg.proton_settings.use_umu_run,
                cfg.proton_settings.custom_env,
                cfg.proton_settings.use_pressure_vessel,
            );
            cfg
        }
        Err(e) => {
            warn!(
                "加载 prefix 配置失败 ({}), 将创建默认配置——用户设置可能丢失!",
                e
            );
            use crate::configs::wine_config::PrefixConfig;
            let cfg = PrefixConfig {
                wine_version_id: wine_version_id.clone(),
                ..Default::default()
            };
            prefix::create_prefix(&game_name, &cfg)?;
            info!("自动创建了 prefix: {}", prefix_dir.display());
            cfg
        }
    };
    let pfx_dir = prefix::get_prefix_pfx_dir(&game_name);

    // 确保 prefix 中有 CJK 字体（解决中文乱码）
    prefix::ensure_cjk_fonts(&game_name);

    // Find the selected wine/proton version
    let versions = detector::scan_all_versions(&[]);
    let wine_version = versions
        .iter()
        .find(|v| v.id == wine_version_id)
        .ok_or_else(|| format!("Wine version '{}' not found", wine_version_id))?;

    let proton_path = &wine_version.path;
    let settings = &prefix_config.proton_settings;

    // Build environment variables
    let mut env: HashMap<String, String> = HashMap::new();

    // Core Proton env
    env.insert(
        "STEAM_COMPAT_DATA_PATH".to_string(),
        prefix_dir.to_string_lossy().to_string(),
    );
    env.insert(
        "WINEPREFIX".to_string(),
        pfx_dir.to_string_lossy().to_string(),
    );
    env.insert(
        "STEAM_COMPAT_INSTALL_PATH".to_string(),
        game_root.to_string_lossy().to_string(),
    );
    // STEAM_COMPAT_TOOL_PATHS：Proton/protonfixes 需要此变量定位自身目录
    env.insert(
        "STEAM_COMPAT_TOOL_PATHS".to_string(),
        proton_path
            .parent()
            .unwrap_or(proton_path)
            .to_string_lossy()
            .to_string(),
    );

    if let Some(steam_root) = detector::get_steam_root_path() {
        env.insert(
            "STEAM_COMPAT_CLIENT_INSTALL_PATH".to_string(),
            steam_root.to_string_lossy().to_string(),
        );
    }

    // Steam App ID（优先用户配置；为空时尝试由预设推断，便于启用 Proton 兼容分支）
    let mut app_id = settings.steam_app_id.trim().to_string();
    if app_id.is_empty() || app_id == "0" {
        if let Some(from_preset) = preset_meta
            .and_then(|p| p.umu_game_id.as_deref())
            .and_then(|id| id.strip_prefix("umu-"))
            .filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()))
        {
            app_id = from_preset.to_string();
        } else {
            app_id = "0".to_string();
        }
    }
    env.insert("SteamAppId".to_string(), app_id.clone());
    env.insert("SteamGameId".to_string(), app_id.clone());
    if app_id != "0" {
        env.insert("STEAMAPPID".to_string(), app_id.clone());
        env.insert("STEAM_COMPAT_APP_ID".to_string(), app_id);
    }
    env.insert(
        "STEAM_PROTON_PATH".to_string(),
        proton_path.to_string_lossy().to_string(),
    );

    // Proton feature flags
    if settings.proton_media_use_gst {
        env.insert("PROTON_MEDIA_USE_GST".to_string(), "1".to_string());
    }
    if settings.proton_enable_wayland {
        env.insert("PROTON_ENABLE_WAYLAND".to_string(), "1".to_string());
    }
    if settings.proton_no_d3d12 {
        env.insert("PROTON_NO_D3D12".to_string(), "1".to_string());
    }
    if settings.mangohud {
        env.insert("MANGOHUD".to_string(), "1".to_string());
    }
    if settings.steam_deck_compat {
        env.insert("SteamDeck".to_string(), "1".to_string());
        // 兼容不同脚本/游戏对大小写的读取差异
        env.insert("steamdeck".to_string(), "1".to_string());
        env.insert("STEAM_DECK".to_string(), "1".to_string());
        env.insert("STEAMDECK".to_string(), "1".to_string());
    }
    if settings.steamos_compat {
        env.insert("SteamOS".to_string(), "1".to_string());
        env.insert("STEAMOS".to_string(), "1".to_string());
        env.insert("steamos".to_string(), "1".to_string());
    }

    apply_preset_env_defaults(preset_meta, &mut env);

    // Per-prefix env overrides (e.g. WINEDLLOVERRIDES)
    for (key, value) in &prefix_config.env_overrides {
        env.insert(key.clone(), value.clone());
    }

    // Custom env from proton_settings
    for (key, value) in &settings.custom_env {
        info!("注入自定义环境变量: {}={}", key, value);
        env.insert(key.clone(), value.clone());
    }

    if env.get("PROTON_NO_ESYNC").is_some_and(|v| v.trim() == "1") {
        warn!("检测到 PROTON_NO_ESYNC=1：该设置可能导致部分游戏稳定性或联网异常，建议关闭后重试");
    }

    // 打印最终的关键环境变量
    info!("环境变量汇总: SteamDeck={}, steamdeck={}, SteamOS={}, SteamAppId={}, WINEPREFIX={}, custom_env_count={}",
        env.get("SteamDeck").unwrap_or(&"(未设置)".to_string()),
        env.get("steamdeck").unwrap_or(&"(未设置)".to_string()),
        env.get("SteamOS").unwrap_or(&"(未设置)".to_string()),
        env.get("SteamAppId").unwrap_or(&"(未设置)".to_string()),
        env.get("WINEPREFIX").unwrap_or(&"(未设置)".to_string()),
        settings.custom_env.len(),
    );

    // GPU 选择和语言设置（从游戏配置 other 中读取）
    if let Some(config_data) = game_config_data.as_ref() {
        // GPU 选择
        if let Some(gpu_index) = config_data
            .pointer("/other/gpuIndex")
            .and_then(|v| v.as_i64())
        {
            if gpu_index >= 0 {
                let gpus = crate::wine::display::enumerate_gpus();
                if let Some(gpu) = gpus.iter().find(|g| g.index == gpu_index as usize) {
                    if gpu.driver == "nvidia" {
                        // OpenGL PRIME offload
                        env.insert("__NV_PRIME_RENDER_OFFLOAD".to_string(), "1".to_string());
                        env.insert(
                            "__NV_PRIME_RENDER_OFFLOAD_PROVIDER".to_string(),
                            format!("NVIDIA-G{}", gpu.index),
                        );
                        env.insert(
                            "__GLX_VENDOR_LIBRARY_NAME".to_string(),
                            "nvidia".to_string(),
                        );
                        env.insert(
                            "__VK_LAYER_NV_optimus".to_string(),
                            "NVIDIA_only".to_string(),
                        );
                        // Vulkan: 优先选择 NVIDIA（不排除其他 ICD，避免 pressure-vessel 内失败）
                        env.insert(
                            "VK_LOADER_DRIVERS_SELECT".to_string(),
                            "nvidia*".to_string(),
                        );
                        // DXVK/VKD3D: 按 GPU 名称过滤，确保选对设备
                        env.insert("DXVK_FILTER_DEVICE_NAME".to_string(), "NVIDIA".to_string());
                        info!(
                            "GPU 选择: NVIDIA GPU {} ({}) [Vulkan+OpenGL]",
                            gpu.index, gpu.name
                        );
                    } else {
                        env.insert("DRI_PRIME".to_string(), gpu.index.to_string());
                        info!("GPU 选择: DRI_PRIME={} ({})", gpu.index, gpu.name);
                    }
                } else {
                    // GPU 索引对应设备未找到，直接用 DRI_PRIME 兜底
                    env.insert("DRI_PRIME".to_string(), gpu_index.to_string());
                    info!("GPU 选择: DRI_PRIME={} (设备未枚举到，兜底)", gpu_index);
                }
            }
        }

        // 语言设置
        if let Some(lang) = config_data
            .pointer("/other/gameLang")
            .and_then(|v| v.as_str())
        {
            if !lang.is_empty() {
                env.insert("LANG".to_string(), format!("{}.UTF-8", lang));
                env.insert("LC_ALL".to_string(), format!("{}.UTF-8", lang));
                info!("语言设置: LANG={}.UTF-8", lang);
            }
        }
    }

    // 检测 jadeite 补丁（HoYoverse 游戏反作弊包装器）
    let is_hoyoverse = matches!(
        game_preset.as_str(),
        "GenshinImpact" | "HonkaiStarRail" | "ZenlessZoneZero"
    );
    let jadeite_exe = if is_hoyoverse {
        // 使用与 install_jadeite 相同的 resolve_patch_dir（从配置读取 gameFolder）
        super::jadeite::resolve_patch_dir(&game_name)
            .ok()
            .map(|d| d.join("jadeite.exe"))
            .filter(|p| p.exists())
    } else {
        None
    };

    // 实际要运行的可执行文件
    // 优先级：jadeite > 游戏 exe
    let (run_exe, extra_args) = if let Some(ref jade) = jadeite_exe {
        info!("使用 jadeite 反作弊补丁: {}", jade.display());
        let win_game_path = format!("Z:{}", launch_exe.to_string_lossy().replace('/', "\\"));
        (jade.clone(), vec![win_game_path, "--".to_string()])
    } else {
        if is_hoyoverse {
            warn!("未找到 jadeite.exe，HoYoverse 游戏可能因反作弊而无法启动");
        }
        (launch_exe.clone(), vec![])
    };

    let force_direct_proton = preset_meta.map(|p| p.force_direct_proton).unwrap_or(false);
    let effective_use_pressure_vessel = if preset_meta
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
        working_dir: launch_exe
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        prefix_path: prefix_dir.to_string_lossy().to_string(),
        proton_path: proton_path.to_string_lossy().to_string(),
        runtime_flags: LaunchRuntimeFlags {
            sandbox_enabled: settings.sandbox_enabled,
            sandbox_isolate_home: settings.sandbox_isolate_home,
            force_direct_proton,
            use_pressure_vessel: effective_use_pressure_vessel,
            region: launch_region.clone(),
        },
    };

    apply_launch_profile_chain(
        &mut launch_profile,
        game_config_data.as_ref(),
        &launch_region,
    )?;

    if preset_meta
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
        launch_profile.runtime_flags.region = launch_region.clone();
    }

    let command_spec = resolve_launch_command(
        &game_preset,
        settings,
        preset_meta,
        proton_path,
        &run_exe,
        &extra_args,
        &mut launch_profile,
    )?;

    let runner_name = command_spec.runner.as_str().to_string();
    info!(
        "最终启动配置: runner={}, sandbox={}, pressureVessel={}, workingDir={}",
        runner_name,
        launch_profile.runtime_flags.sandbox_enabled,
        launch_profile.runtime_flags.use_pressure_vessel,
        launch_profile.working_dir
    );

    let mut cmd = if launch_profile.runtime_flags.sandbox_enabled && !command_spec.use_umu_runtime {
        info!(
            "Launching with bwrap sandbox (isolate_home={})",
            launch_profile.runtime_flags.sandbox_isolate_home
        );
        build_bwrap_command(
            &command_spec.program,
            &command_spec.args,
            &launch_exe,
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

    if !launch_profile.working_dir.trim().is_empty() {
        let wd = PathBuf::from(launch_profile.working_dir.trim());
        if wd.exists() {
            cmd.current_dir(wd);
        } else {
            warn!(
                "LaunchProfile workingDir 不存在，回退到 exe 目录: {}",
                launch_profile.working_dir
            );
            if let Some(game_dir) = launch_exe.parent() {
                cmd.current_dir(game_dir);
            }
        }
    } else if let Some(game_dir) = launch_exe.parent() {
        cmd.current_dir(game_dir);
    }

    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to launch game: {}", e))?;

    let pid = child.id().unwrap_or(0);
    let launched_at = chrono::Utc::now().to_rfc3339();
    info!("Game launched with PID {}", pid);

    process_monitor::register_game_process(game_name.clone(), pid, game_exe_path.clone()).await;

    app.emit(
        "game-lifecycle",
        serde_json::json!({
            "event": "started",
            "game": game_name,
            "pid": pid,
            "runner": runner_name,
            "region": launch_profile.runtime_flags.region,
            "launchedAt": launched_at
        }),
    )
    .ok();

    if let Some(stdout) = child.stdout.take() {
        spawn_launch_log_pipe(
            app.clone(),
            game_name.clone(),
            launch_profile.runtime_flags.region.clone(),
            runner_name.clone(),
            "stdout",
            stdout,
        );
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_launch_log_pipe(
            app.clone(),
            game_name.clone(),
            launch_profile.runtime_flags.region.clone(),
            runner_name.clone(),
            "stderr",
            stderr,
        );
    }

    let app_clone = app.clone();
    let game_name_for_monitor = game_name.clone();
    let region_for_monitor = launch_profile.runtime_flags.region.clone();
    let runner_for_monitor = runner_name.clone();
    let launched_at_for_monitor = launched_at.clone();
    let exe_name = launch_exe
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    tokio::spawn(async move {
        let _write_guard = write_guard;
        let mut exit_code: Option<i32> = None;
        let mut signal: Option<i32> = None;
        let crashed: bool;

        match child.wait().await {
            Ok(status) => {
                exit_code = status.code();
                signal = exit_status_signal(&status);
                crashed = exit_code.map(|v| v != 0).unwrap_or(false) || signal.is_some();
                info!("Direct child process exited with status: {}", status);
            }
            Err(e) => {
                crashed = true;
                error!("Failed to wait for child process: {}", e);
            }
        }

        info!("检查游戏 {} 的子进程是否仍在运行...", game_name_for_monitor);
        let mut check_count = 0;
        let max_checks = 30;

        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            let processes = process_monitor::find_game_processes(&exe_name).await;

            if processes.is_empty() {
                info!("游戏 {} 的所有进程已退出", game_name_for_monitor);
                break;
            }

            check_count += 1;
            if check_count >= max_checks {
                info!("游戏 {} 进程检查超时，假定已退出", game_name_for_monitor);
                break;
            }

            if check_count % 5 == 0 {
                info!(
                    "游戏 {} 仍有 {} 个进程在运行",
                    game_name_for_monitor,
                    processes.len()
                );
            }
        }

        process_monitor::unregister_game_process(&game_name_for_monitor).await;

        app_clone
            .emit(
                "game-lifecycle",
                serde_json::json!({
                    "event": "exited",
                    "game": game_name_for_monitor,
                    "pid": pid,
                    "runner": runner_for_monitor,
                    "region": region_for_monitor,
                    "launchedAt": launched_at_for_monitor,
                    "finishedAt": chrono::Utc::now().to_rfc3339(),
                    "exitCode": exit_code,
                    "signal": signal,
                    "crashed": crashed
                }),
            )
            .ok();
    });

    Ok(format!("Game launched (PID: {})", pid))
}

fn apply_preset_env_defaults(
    preset: Option<&crate::configs::game_presets::GamePreset>,
    env: &mut HashMap<String, String>,
) {
    let Some(preset) = preset else {
        return;
    };
    for (key, value) in &preset.env_defaults {
        if !env.contains_key(key) {
            env.insert(key.clone(), value.clone());
        }
    }
}

fn find_umu_run_binary() -> Option<PathBuf> {
    which::which("umu-run").ok()
}

fn apply_umu_env_defaults(
    game_preset: &str,
    proton_path: &Path,
    settings: &crate::configs::wine_config::ProtonSettings,
    preset: Option<&crate::configs::game_presets::GamePreset>,
    env: &mut HashMap<String, String>,
) {
    let proton_dir = proton_path
        .parent()
        .unwrap_or(proton_path)
        .to_string_lossy()
        .to_string();
    env.insert("PROTONPATH".to_string(), proton_dir);

    if !env.contains_key("GAMEID") {
        let game_id = preset
            .and_then(|p| p.umu_game_id.clone())
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| {
                if settings.steam_app_id != "0" && !settings.steam_app_id.trim().is_empty() {
                    format!("umu-{}", settings.steam_app_id.trim())
                } else {
                    format!("nonsteam-{}", game_preset.to_lowercase())
                }
            });
        env.insert("GAMEID".to_string(), game_id);
    }
    if !env.contains_key("UMU_ID") {
        if let Some(game_id) = env.get("GAMEID").cloned() {
            env.insert("UMU_ID".to_string(), game_id);
        }
    }

    if !env.contains_key("STORE") {
        if let Some(store) = preset
            .and_then(|p| p.umu_store.as_ref())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            env.insert("STORE".to_string(), store.to_string());
        }
    }

    if env
        .get("SteamAppId")
        .is_none_or(|v| v.trim().is_empty() || v.trim() == "0")
    {
        let maybe_numeric_id = env.get("GAMEID").and_then(|game_id| {
            game_id
                .strip_prefix("umu-")
                .filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()))
                .map(|id| id.to_string())
        });
        if let Some(numeric_id) = maybe_numeric_id {
            env.insert("SteamAppId".to_string(), numeric_id.clone());
            env.insert("SteamGameId".to_string(), numeric_id);
        }
    }
    if env
        .get("STEAM_COMPAT_APP_ID")
        .is_none_or(|v| v.trim().is_empty() || v.trim() == "0")
    {
        let maybe_numeric_id = env
            .get("UMU_ID")
            .and_then(|id| id.strip_prefix("umu-"))
            .or_else(|| env.get("GAMEID").and_then(|id| id.strip_prefix("umu-")))
            .filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()))
            .map(|id| id.to_string());
        if let Some(numeric_id) = maybe_numeric_id {
            env.insert("STEAM_COMPAT_APP_ID".to_string(), numeric_id);
        }
    }

    info!(
        "umu-run env: PROTONPATH={}, GAMEID={}, UMU_ID={}, STORE={}, UMU_USE_STEAM={}, SteamAppId={}, STEAM_COMPAT_APP_ID={}",
        env.get("PROTONPATH").cloned().unwrap_or_default(),
        env.get("GAMEID").cloned().unwrap_or_default(),
        env.get("UMU_ID").cloned().unwrap_or_default(),
        env.get("STORE").cloned().unwrap_or_default(),
        env.get("UMU_USE_STEAM").cloned().unwrap_or_default(),
        env.get("SteamAppId").cloned().unwrap_or_default(),
        env.get("STEAM_COMPAT_APP_ID").cloned().unwrap_or_default(),
    );
}

fn apply_launch_profile_chain(
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
        let Some(raw) = db::get_setting(key) else {
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

fn extract_launch_profile_patch(
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

fn apply_launch_profile_patch(profile: &mut LaunchProfile, patch: LaunchProfilePatch) {
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

fn parse_launch_runner(raw: &str) -> Option<LaunchRunner> {
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
            if let Some(umu_run) = find_umu_run_binary() {
                apply_umu_env_defaults(
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
                build_direct_proton_command_spec_with_args(&proton_path, run_exe, &merged_args)
            }
        }
        LaunchRunner::Wine => resolve_wine_command(&proton_path, run_exe, &merged_args)?,
        LaunchRunner::Proton => {
            let cmd = build_proton_base_command(
                launch_profile.runtime_flags.use_pressure_vessel,
                &proton_path,
                run_exe,
                &merged_args,
            );
            if launch_profile.runtime_flags.use_pressure_vessel && cmd.0 != proton_path {
                runner = LaunchRunner::PressureVessel;
            } else if launch_profile.runtime_flags.use_pressure_vessel {
                // pressure-vessel 请求已回退到直连 Proton
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

fn spawn_launch_log_pipe<R>(
    app: tauri::AppHandle,
    game_name: String,
    region: String,
    runner: String,
    stream: &'static str,
    pipe: R,
) where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut reader = BufReader::new(pipe);
        let mut buf = Vec::with_capacity(4096);
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf).await {
                Ok(0) => break,
                Ok(_) => {
                    while matches!(buf.last(), Some(b'\n' | b'\r')) {
                        buf.pop();
                    }
                    if buf.is_empty() {
                        continue;
                    }
                    // 使用 lossy 解码，避免非 UTF-8 导致停止读取管道。
                    let line = String::from_utf8_lossy(&buf).to_string();
                    app.emit(
                        "game-launch-log",
                        serde_json::json!({
                            "game": game_name,
                            "region": region,
                            "runner": runner,
                            "stream": stream,
                            "line": line
                        }),
                    )
                    .ok();
                }
                Err(err) => {
                    warn!("读取 {} 输出失败: {}", stream, err);
                    break;
                }
            }
        }
    });
}

fn normalize_non_empty(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn build_proton_base_command(
    use_pressure_vessel: bool,
    proton_path: &Path,
    run_exe: &Path,
    extra_args: &[String],
) -> (PathBuf, Vec<String>) {
    if use_pressure_vessel {
        if let Some(runtime_dir) = detector::find_steam_linux_runtime() {
            let entry_point = runtime_dir.join("_v2-entry-point");
            info!(
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

    build_direct_proton_command_spec_with_args(proton_path, run_exe, extra_args)
}

fn build_direct_proton_command_spec_with_args(
    proton_path: &Path,
    run_exe: &Path,
    extra_args: &[String],
) -> (PathBuf, Vec<String>) {
    info!(
        "Launching with direct proton: {} waitforexitandrun {} {:?}",
        proton_path.display(),
        run_exe.display(),
        extra_args
    );
    let mut args = vec![
        "waitforexitandrun".to_string(),
        run_exe.to_string_lossy().to_string(),
    ];
    args.extend_from_slice(extra_args);
    (proton_path.to_path_buf(), args)
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

fn is_tos_risk_acknowledged() -> bool {
    db::get_setting("tos_risk_acknowledged")
        .map(|v| {
            let normalized = v.trim().to_ascii_lowercase();
            normalized == "true" || normalized == "1" || normalized == "yes"
        })
        .unwrap_or(false)
}

fn read_non_empty_string(v: Option<&Value>) -> Option<String> {
    v.and_then(|x| x.as_str()).and_then(normalize_non_empty)
}

fn load_game_config_json(game_name: &str) -> Option<Value> {
    let canonical = crate::configs::game_identity::to_canonical_or_keep(game_name);
    let content = db::get_game_config(&canonical)?;
    match serde_json::from_str::<Value>(&content) {
        Ok(value) => Some(value),
        Err(err) => {
            warn!("解析游戏配置失败 ({}): {}", canonical, err);
            None
        }
    }
}

fn resolve_game_preset_with_data(game_name: &str, data: Option<&Value>) -> String {
    let canonical = crate::configs::game_identity::to_canonical_or_keep(game_name);
    let candidate = data
        .and_then(extract_game_preset_from_config)
        .unwrap_or_else(|| resolve_game_preset(&canonical));
    if crate::configs::game_presets::get_preset(&candidate).is_some() {
        candidate
    } else {
        canonical
    }
}

fn resolve_launch_launcher_api(game_config_data: Option<&Value>) -> Option<String> {
    game_config_data.and_then(|data| {
        read_non_empty_string(
            data.pointer("/other/launcherApi")
                .or_else(|| data.pointer("/launcherApi")),
        )
    })
}

fn resolve_launch_biz_prefix(
    game_config_data: Option<&Value>,
    preset_meta: Option<&crate::configs::game_presets::GamePreset>,
    launcher_api: Option<&str>,
) -> Option<String> {
    if let Some(biz_prefix) = game_config_data.and_then(|data| {
        read_non_empty_string(
            data.pointer("/other/bizPrefix")
                .or_else(|| data.pointer("/bizPrefix")),
        )
    }) {
        return Some(biz_prefix);
    }

    let api = launcher_api
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let preset = preset_meta?;
    let server = preset
        .download_servers
        .iter()
        .find(|server| server.launcher_api.trim() == api)?;
    normalize_non_empty(&server.biz_prefix)
}

fn resolve_launch_region(
    game_config_data: Option<&Value>,
    preset_meta: Option<&crate::configs::game_presets::GamePreset>,
    region_override: Option<&str>,
) -> String {
    if let Some(region) = region_override.and_then(normalize_non_empty) {
        return region;
    }

    if let Some(region) = game_config_data.and_then(|data| {
        read_non_empty_string(
            data.pointer("/other/launchRegion")
                .or_else(|| data.pointer("/launchRegion")),
        )
    }) {
        return region;
    }

    let launcher_api = resolve_launch_launcher_api(game_config_data);
    if let Some(api) = launcher_api.as_deref() {
        if let Some(preset) = preset_meta {
            if let Some(server) = preset
                .download_servers
                .iter()
                .find(|s| s.launcher_api.trim() == api.trim())
            {
                if let Some(id) = normalize_non_empty(&server.id) {
                    return id;
                }
                if let Some(label) = normalize_non_empty(&server.label) {
                    return label;
                }
            }
        }
    }

    if let Some(default_id) = preset_meta
        .and_then(|p| p.download_servers.first())
        .and_then(|s| normalize_non_empty(&s.id))
    {
        return default_id;
    }

    "default".to_string()
}

fn resolve_game_preset(game_name: &str) -> String {
    let game_name = crate::configs::game_identity::to_canonical_or_keep(game_name);
    let Some(content) = db::get_game_config(&game_name) else {
        return game_name.to_string();
    };

    let Ok(data) = serde_json::from_str::<Value>(&content) else {
        return game_name.to_string();
    };

    let candidate = extract_game_preset_from_config(&data).unwrap_or_else(|| game_name.clone());
    if crate::configs::game_presets::get_preset(&candidate).is_some() {
        candidate
    } else {
        game_name
    }
}

fn extract_game_preset_from_config(data: &Value) -> Option<String> {
    data.pointer("/basic/gamePreset")
        .or_else(|| data.pointer("/basic/GamePreset"))
        .or_else(|| data.get("GamePreset"))
        .or_else(|| data.get("LogicName"))
        .or_else(|| data.get("gamePreset"))
        .and_then(|v| v.as_str())
        .map(crate::configs::game_identity::to_canonical_or_keep)
        .filter(|s| !s.is_empty())
}

fn infer_game_root_from_exe(game_exe: &Path) -> Option<PathBuf> {
    game_exe.parent().map(|p| p.to_path_buf())
}

fn resolve_preferred_launch_exe(game_preset: &str, game_exe: &Path) -> PathBuf {
    if game_preset == "WutheringWaves" {
        let file_name = game_exe
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if file_name.eq_ignore_ascii_case("Client-Win64-Shipping.exe") {
            if let Some(win64_dir) = game_exe.parent() {
                if let Some(binaries_dir) = win64_dir.parent() {
                    if let Some(client_dir) = binaries_dir.parent() {
                        if let Some(game_root) = client_dir.parent() {
                            let wrapper = game_root.join("Wuthering Waves.exe");
                            if wrapper.exists() {
                                info!(
                                    "WutheringWaves 启动可执行已切换为包装器: {}",
                                    wrapper.display()
                                );
                                return wrapper;
                            }
                        }
                    }
                }
            }
        }
    }
    game_exe.to_path_buf()
}

#[cfg(unix)]
fn exit_status_signal(status: &std::process::ExitStatus) -> Option<i32> {
    status.signal()
}

#[cfg(not(unix))]
fn exit_status_signal(_status: &std::process::ExitStatus) -> Option<i32> {
    None
}
