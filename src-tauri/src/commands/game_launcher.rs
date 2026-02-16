use crate::configs::database as db;
use crate::utils::ini_manager;
use crate::wine::{detector, prefix};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tauri::Emitter;
use tracing::{error, info, warn};

#[tauri::command]
pub async fn start_game(
    app: tauri::AppHandle,
    game_name: String,
    game_exe_path: String,
    wine_version_id: String,
) -> Result<String, String> {
    let game_name = crate::configs::game_identity::to_canonical_or_keep(&game_name);
    let game_exe = PathBuf::from(&game_exe_path);
    if !game_exe.exists() {
        return Err(format!("Game executable not found: {}", game_exe_path));
    }

    if !is_tos_risk_acknowledged() {
        return Err(
            "未完成风险确认，禁止启动。请先在首次向导完成风险确认后再启动游戏。".to_string(),
        );
    }

    let game_preset = resolve_game_preset(&game_name);
    let preset_meta = crate::configs::game_presets::get_preset(&game_preset);
    let launch_exe = resolve_preferred_launch_exe(&game_preset, &game_exe);
    let game_root = infer_game_root_from_exe(&launch_exe)
        .ok_or_else(|| format!("无法从可执行文件推断游戏目录: {}", game_exe_path))?;
    let game_root_str = game_root.to_string_lossy().to_string();

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
                "已加载 prefix 配置: steam_deck_compat={}, use_umu_run={}, custom_env={:?}, use_pressure_vessel={}",
                cfg.proton_settings.steam_deck_compat,
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
    if let Some(config_json) = db::get_game_config(&game_name) {
        if let Ok(config_data) = serde_json::from_str::<Value>(&config_json) {
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
    }

    // 检测 jadeite 补丁（HoYoverse 游戏反作弊包装器）
    let is_hoyoverse = matches!(
        game_preset.as_str(),
        "GenshinImpact" | "HonkaiStarRail" | "ZenlessZoneZero" | "HonkaiImpact3rd"
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

    // 实际要运行的可执行文件（有 jadeite 则用 jadeite 包装）
    // 参考 the-honkers-railway-launcher：jadeite.exe 需要 Windows 路径格式（Z:\...）
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
    let mut use_umu_runtime = false;
    let (base_program, base_args) = if force_direct_proton {
        if settings.use_umu_run {
            warn!("当前预设要求强制直连 Proton，已忽略 umu-run 设置");
        }
        build_proton_base_command(
            effective_use_pressure_vessel,
            proton_path,
            &run_exe,
            &extra_args,
        )
    } else if settings.use_umu_run {
        if let Some(umu_run) = find_umu_run_binary() {
            apply_umu_env_defaults(&game_preset, proton_path, settings, preset_meta, &mut env);
            info!(
                "Launching with umu-run: {} -> {} {:?}",
                umu_run.display(),
                run_exe.display(),
                extra_args
            );
            use_umu_runtime = true;
            let mut args = vec![run_exe.to_string_lossy().to_string()];
            args.extend(extra_args.clone());
            (umu_run, args)
        } else {
            warn!("已启用 umu-run，但系统未找到 umu-run，回退到当前 Proton 启动链");
            build_proton_base_command(
                effective_use_pressure_vessel,
                proton_path,
                &run_exe,
                &extra_args,
            )
        }
    } else {
        build_proton_base_command(
            effective_use_pressure_vessel,
            proton_path,
            &run_exe,
            &extra_args,
        )
    };

    let mut cmd = if settings.sandbox_enabled && !use_umu_runtime {
        info!(
            "Launching with bwrap sandbox (isolate_home={})",
            settings.sandbox_isolate_home
        );
        build_bwrap_command(
            &base_program,
            &base_args,
            &game_exe,
            &prefix_dir,
            settings.sandbox_isolate_home,
            &env,
        )?
    } else {
        if settings.sandbox_enabled && use_umu_runtime {
            warn!("umu-run 已启用，跳过额外 bwrap 沙盒以避免容器嵌套冲突");
        }
        let mut command = tokio::process::Command::new(&base_program);
        command.args(&base_args);
        command
    };

    // Set environment
    cmd.envs(&env);

    // Set working directory to game exe's parent
    if let Some(game_dir) = launch_exe.parent() {
        cmd.current_dir(game_dir);
    }

    // Launch — 不捕获 stdout/stderr，避免长时间运行累积内存
    let mut child = cmd
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to launch game: {}", e))?;

    let pid = child.id().unwrap_or(0);
    info!("Game launched with PID {}", pid);

    // 通知前端游戏已启动
    let game_name_clone = game_name.clone();
    app.emit(
        "game-lifecycle",
        serde_json::json!({
            "event": "started",
            "game": game_name_clone,
            "pid": pid
        }),
    )
    .ok();

    // 后台等待进程退出，退出后通知前端（仅 wait，不累积输出）
    let app_clone = app.clone();
    tokio::spawn(async move {
        match child.wait().await {
            Ok(status) => {
                info!("Game process exited with status: {}", status);
            }
            Err(e) => {
                error!("Failed to wait for game process: {}", e);
            }
        }
        // 通知前端游戏已退出
        app_clone
            .emit(
                "game-lifecycle",
                serde_json::json!({
                    "event": "exited",
                    "game": game_name
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

#[tauri::command]
pub fn check_3dmigoto_integrity(
    _app: tauri::AppHandle,
    _game_name: &str,
    game_path: &str,
) -> Result<bool, String> {
    let game_dir = PathBuf::from(game_path);
    let d3dx_ini = game_dir.join("d3dx.ini");

    if !d3dx_ini.exists() {
        return Ok(false);
    }

    let d3d11_dll = game_dir.join("d3d11.dll");
    let d3dcompiler = game_dir.join("d3dcompiler_47.dll");

    Ok(d3d11_dll.exists() && d3dcompiler.exists())
}

#[tauri::command]
pub fn toggle_symlink(game_path: &str, enabled: bool) -> Result<bool, String> {
    let game_dir = PathBuf::from(game_path);
    let ini_path = game_dir.join("d3dx.ini");

    if !ini_path.exists() {
        return Err("d3dx.ini not found".to_string());
    }

    let mut ini_data = ini_manager::load_ini(&ini_path)?;

    if enabled {
        ini_manager::set_value(&mut ini_data, "Loader", "target", "d3d11.dll");
    } else {
        ini_manager::remove_value(&mut ini_data, "Loader", "target");
    }

    ini_manager::save_ini(&ini_data, &ini_path)?;
    info!("Toggled symlink for {}: enabled={}", game_path, enabled);
    Ok(enabled)
}

#[tauri::command]
pub fn get_symlink_status(game_path: &str) -> Result<bool, String> {
    let game_dir = PathBuf::from(game_path);
    let ini_path = game_dir.join("d3dx.ini");

    if !ini_path.exists() {
        return Err("d3dx.ini not found".to_string());
    }

    let ini_data = ini_manager::load_ini(&ini_path)?;
    let enabled = ini_manager::get_value(&ini_data, "Loader", "target")
        .map(|v| v.trim().eq_ignore_ascii_case("d3d11.dll"))
        .unwrap_or(false);

    Ok(enabled)
}
