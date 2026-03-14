use super::*;

pub(super) fn load_prefix_runtime_context(
    game_name: &str,
    wine_version_id: &str,
) -> Result<PrefixRuntimeContext, String> {
    let prefix_dir = prefix::get_prefix_dir(game_name);
    let compat_root_dir = prefix::get_proton_compat_root_dir(game_name);
    info!("prefix 路径: {}", prefix_dir.display());
    if compat_root_dir != prefix_dir {
        warn!(
            "检测到 Proton compat root 与 prefix 根目录不一致，启动时将使用 compat root: prefix={}, compat_root={}",
            prefix_dir.display(),
            compat_root_dir.display()
        );
        super::append_game_log(
            game_name,
            "WARN",
            "runtime",
            format!(
                "detected nested Proton compat root; using compat root {} instead of {}",
                compat_root_dir.display(),
                prefix_dir.display()
            ),
        );
    }
    let mut prefix_config = match prefix::load_prefix_config(game_name) {
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
                wine_version_id: wine_version_id.to_string(),
                ..Default::default()
            };
            prefix::create_prefix(game_name, &cfg)?;
            info!("自动创建了 prefix: {}", prefix_dir.display());
            cfg
        }
    };

    if should_prefer_umu_runtime_for_launch(
        game_name,
        wine_version_id,
        &prefix_config.proton_settings,
    ) {
        prefix_config.proton_settings.use_umu_run = true;
        info!(
            "运行时已为 {} 自动启用 umu-run，以对齐 Proton/Lutris 启动链兼容性",
            game_name
        );
        super::append_game_log(
            game_name,
            "INFO",
            "runtime",
            "runtime override: enabled umu-run for this launch to align with Lutris Proton compatibility",
        );
    }
    let pfx_dir = prefix::get_prefix_pfx_dir(game_name);

    prefix::ensure_cjk_fonts(game_name);

    let dxvk_status = crate::wine::graphics::detect_installed_dxvk(&pfx_dir);
    let vkd3d_status = crate::wine::graphics::detect_installed_vkd3d(&pfx_dir);
    super::append_game_log(
        game_name,
        "INFO",
        "runtime",
        format!(
            "prefix config: dxvk_enabled={}, dxvk_version_hint={:?}, vkd3d_enabled={}, vkd3d_version_hint={:?}, installed_runtimes={:?}",
            prefix_config.dxvk.enabled,
            prefix_config.dxvk.version,
            prefix_config.vkd3d.enabled,
            prefix_config.vkd3d.version,
            prefix_config.installed_runtimes
        ),
    );
    super::append_game_log(
        game_name,
        "INFO",
        "runtime",
        format!(
            "runtime detect: dxvk_installed={}, dxvk_detected_version={:?}, dxvk_dlls={:?}",
            dxvk_status.installed, dxvk_status.version, dxvk_status.dlls_found
        ),
    );
    super::append_game_log(
        game_name,
        "INFO",
        "runtime",
        format!(
            "runtime detect: vkd3d_installed={}, vkd3d_detected_version={:?}, vkd3d_dlls={:?}",
            vkd3d_status.installed, vkd3d_status.version, vkd3d_status.dlls_found
        ),
    );
    debug!(
        "DXVK 启动前检测: installed={}, version={:?}, dlls={:?}, prefix={}",
        dxvk_status.installed,
        dxvk_status.version,
        dxvk_status.dlls_found,
        pfx_dir.display()
    );
    if !dxvk_status.installed {
        warn!(
            "检测到 Prefix 未安装 DXVK: {}。若游戏依赖 DirectX 11/12，可能黑屏或启动失败。可在“游戏设置 -> 运行环境 -> DXVK 管理”安装。",
            pfx_dir.display()
        );
    }

    let versions = detector::scan_all_versions(&[]);
    debug!("运行时扫描结果: total_versions={}", versions.len());
    let wine_version = select_wine_runtime_for_id(&versions, wine_version_id).ok_or_else(|| {
        format!(
            "未找到已配置的 Wine/Proton 版本: {}。请在“游戏设置 -> 运行环境”重新选择。",
            wine_version_id
        )
    })?;
    debug!(
        "运行时匹配成功: id={}, name={}, variant={}, version={}, path={}",
        wine_version.id,
        wine_version.name,
        wine_version.variant,
        wine_version.version,
        wine_version.path.display()
    );

    let proton_path = wine_version.path.clone();
    let use_proton_compat_env = is_proton_runtime_variant(&wine_version.variant)
        || proton_path
            .to_string_lossy()
            .to_ascii_lowercase()
            .contains("proton");
    if use_proton_compat_env {
        debug!(
            "运行时判定为 Proton 兼容环境: variant={}, path={}",
            wine_version.variant,
            proton_path.display()
        );
    }
    if !proton_path.exists() {
        return Err(format!(
            "启动配置错误：所选 Wine/Proton 路径不存在：{}。请在“游戏设置 -> 运行环境”修复。",
            proton_path.display()
        ));
    }

    let settings = prefix_config.proton_settings.clone();
    super::append_game_log(
        game_name,
        "INFO",
        "runtime",
        format!(
            "proton settings: use_umu_run={}, use_pressure_vessel={}, sandbox_enabled={}, sandbox_isolate_home={}, steam_app_id={}, media_gst={}, wayland={}, no_d3d12={}, mangohud={}, steamdeck={}, steamos={}, custom_env_count={}",
            settings.use_umu_run,
            settings.use_pressure_vessel,
            settings.sandbox_enabled,
            settings.sandbox_isolate_home,
            settings.steam_app_id,
            settings.proton_media_use_gst,
            settings.proton_enable_wayland,
            settings.proton_no_d3d12,
            settings.mangohud,
            settings.steam_deck_compat,
            settings.steamos_compat,
            settings.custom_env.len()
        ),
    );

    let steam_runtime = detector::find_steam_linux_runtime();
    super::append_game_log(
        game_name,
        if steam_runtime.is_some() {
            "INFO"
        } else {
            "WARN"
        },
        "container",
        format!(
            "pressure-vessel runtime path: {}",
            steam_runtime
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "(not found)".to_string())
        ),
    );
    if let Ok(home) = std::env::var("HOME") {
        let umu_runtime_dir = PathBuf::from(&home).join(".local/share/umu/steamrt3");
        let umu_lock = PathBuf::from(&home).join(".local/share/umu/umu.lock");
        super::append_game_log(
            game_name,
            "INFO",
            "container",
            format!(
                "umu runtime preflight: runtime_dir_exists={}, runtime_dir={}, lock_exists={}, lock_file={}, update_check=delegated_to_umu_run",
                umu_runtime_dir.exists(),
                umu_runtime_dir.display(),
                umu_lock.exists(),
                umu_lock.display()
            ),
        );
    }

    Ok(PrefixRuntimeContext {
        prefix_dir,
        pfx_dir,
        compat_root_dir,
        use_proton_compat_env,
        prefix_config,
        settings,
        proton_path,
    })
}

fn is_proton_runtime_variant(variant: &crate::configs::wine_config::ProtonVariant) -> bool {
    matches!(
        variant,
        crate::configs::wine_config::ProtonVariant::Official
            | crate::configs::wine_config::ProtonVariant::Experimental
            | crate::configs::wine_config::ProtonVariant::GEProton
            | crate::configs::wine_config::ProtonVariant::DWProton
            | crate::configs::wine_config::ProtonVariant::ProtonTKG
    )
}

fn runtime_path_source_priority(path: &Path) -> u8 {
    let lower = path.to_string_lossy().to_ascii_lowercase();
    if lower.contains("/compatibilitytools.d/") {
        0
    } else if lower.contains("/steamapps/common/proton") {
        1
    } else if lower.contains("/.local/share/ssmt4/proton/") {
        3
    } else {
        2
    }
}

fn select_wine_runtime_for_id<'a>(
    versions: &'a [crate::configs::wine_config::WineVersion],
    wine_version_id: &str,
) -> Option<&'a crate::configs::wine_config::WineVersion> {
    let mut matches: Vec<&crate::configs::wine_config::WineVersion> = versions
        .iter()
        .filter(|v| v.id == wine_version_id)
        .collect();
    if matches.is_empty() {
        return None;
    }
    if matches.len() > 1 {
        let candidates = matches
            .iter()
            .map(|v| format!("{}@{}", v.name, v.path.display()))
            .collect::<Vec<_>>()
            .join(" | ");
        warn!(
            "检测到重复 Wine/Proton ID: {}，将按来源优先级选择。candidates={}",
            wine_version_id, candidates
        );
    }
    matches.sort_by(|a, b| {
        runtime_path_source_priority(&a.path)
            .cmp(&runtime_path_source_priority(&b.path))
            .then_with(|| a.path.to_string_lossy().cmp(&b.path.to_string_lossy()))
    });
    matches.first().copied()
}

fn should_prefer_umu_runtime_for_launch(
    game_name: &str,
    wine_version_id: &str,
    settings: &crate::configs::wine_config::ProtonSettings,
) -> bool {
    should_prefer_umu_runtime_for_launch_with_availability(
        game_name,
        wine_version_id,
        settings,
        find_umu_run_binary().is_some(),
    )
}

fn should_prefer_umu_runtime_for_launch_with_availability(
    game_name: &str,
    wine_version_id: &str,
    settings: &crate::configs::wine_config::ProtonSettings,
    umu_available: bool,
) -> bool {
    if !game_name.eq_ignore_ascii_case("ArknightsEndfield") {
        return false;
    }
    if settings.use_umu_run {
        return false;
    }
    if !umu_available {
        return false;
    }

    let lower = wine_version_id.to_ascii_lowercase();
    lower.contains("proton")
}

const END_FIELD_IME_ENV_KEYS: [&str; 4] = [
    "XMODIFIERS",
    "GTK_IM_MODULE",
    "QT_IM_MODULE",
    "SDL_IM_MODULE",
];

fn inherit_endfield_ime_env_with_provider<F>(
    game_preset: &str,
    env: &mut HashMap<String, String>,
    host_env: F,
) where
    F: Fn(&str) -> Option<String>,
{
    if !game_preset.eq_ignore_ascii_case("ArknightsEndfield") {
        return;
    }

    for key in END_FIELD_IME_ENV_KEYS {
        if env.contains_key(key) {
            continue;
        }

        if let Some(value) = host_env(key)
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
        {
            env.insert(key.to_string(), value);
        }
    }

    if env.contains_key("QT_IM_MODULES") {
        return;
    }
    if let Some(value) = host_env("QT_IM_MODULES")
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
    {
        env.insert("QT_IM_MODULES".to_string(), value);
    }
}

fn inherit_endfield_ime_env(game_preset: &str, env: &mut HashMap<String, String>) {
    inherit_endfield_ime_env_with_provider(game_preset, env, |key| std::env::var(key).ok());
}

pub(super) fn build_launch_environment(
    target: &ResolvedLaunchTarget,
    runtime: &PrefixRuntimeContext,
) -> HashMap<String, String> {
    let settings = &runtime.settings;
    let mut env: HashMap<String, String> = HashMap::new();
    let compat_data_dir = if runtime.use_proton_compat_env {
        &runtime.compat_root_dir
    } else {
        &runtime.prefix_dir
    };
    let wineprefix_dir = if runtime.use_proton_compat_env {
        &runtime.compat_root_dir
    } else {
        &runtime.pfx_dir
    };

    env.insert(
        "STEAM_COMPAT_DATA_PATH".to_string(),
        compat_data_dir.to_string_lossy().to_string(),
    );
    env.insert(
        "WINEPREFIX".to_string(),
        wineprefix_dir.to_string_lossy().to_string(),
    );
    env.insert(
        "STEAM_COMPAT_INSTALL_PATH".to_string(),
        target.game_root.to_string_lossy().to_string(),
    );
    env.insert(
        "STEAM_COMPAT_TOOL_PATHS".to_string(),
        runtime
            .proton_path
            .parent()
            .unwrap_or(&runtime.proton_path)
            .to_string_lossy()
            .to_string(),
    );

    let resolved_steam_app_id = resolve_numeric_steam_app_id(settings, target.preset_meta);
    // GE-Proton may hard-require STEAM_COMPAT_CLIENT_INSTALL_PATH during prefix setup.
    // Keep this available whenever we can resolve Steam root, regardless of app id.
    if let Some(steam_root) = detector::get_steam_root_path() {
        env.insert(
            "STEAM_COMPAT_CLIENT_INSTALL_PATH".to_string(),
            steam_root.to_string_lossy().to_string(),
        );
    }

    apply_steam_app_id_env(&mut env, resolved_steam_app_id.as_deref());
    env.insert(
        "STEAM_PROTON_PATH".to_string(),
        runtime.proton_path.to_string_lossy().to_string(),
    );

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
        env.insert("steamdeck".to_string(), "1".to_string());
        env.insert("STEAM_DECK".to_string(), "1".to_string());
        env.insert("STEAMDECK".to_string(), "1".to_string());
    }
    if settings.steamos_compat {
        env.insert("SteamOS".to_string(), "1".to_string());
        env.insert("STEAMOS".to_string(), "1".to_string());
        env.insert("steamos".to_string(), "1".to_string());
    }

    apply_preset_env_defaults(target.preset_meta, &mut env);

    apply_lutris_like_proton_defaults(target.game_preset.as_str(), &mut env);
    inherit_endfield_ime_env(target.game_preset.as_str(), &mut env);

    for (key, value) in &runtime.prefix_config.env_overrides {
        env.insert(key.clone(), value.clone());
    }

    for (key, value) in &settings.custom_env {
        info!("注入自定义环境变量: {}={}", key, value);
        env.insert(key.clone(), value.clone());
    }

    let fallback_steam_app_id = resolve_numeric_steam_app_id_from_env(&env);
    apply_steam_app_id_env(&mut env, fallback_steam_app_id.as_deref());

    if env.get("PROTON_NO_ESYNC").is_some_and(|v| v.trim() == "1") {
        warn!("检测到 PROTON_NO_ESYNC=1：该设置可能导致部分游戏稳定性或联网异常，建议关闭后重试");
    }

    if target.game_preset.eq_ignore_ascii_case("ArknightsEndfield") {
        info!(
            "Endfield 输入法环境: XMODIFIERS={}, GTK_IM_MODULE={}, QT_IM_MODULE={}, SDL_IM_MODULE={}, QT_IM_MODULES={}",
            env.get("XMODIFIERS").unwrap_or(&"(未设置)".to_string()),
            env.get("GTK_IM_MODULE").unwrap_or(&"(未设置)".to_string()),
            env.get("QT_IM_MODULE").unwrap_or(&"(未设置)".to_string()),
            env.get("SDL_IM_MODULE").unwrap_or(&"(未设置)".to_string()),
            env.get("QT_IM_MODULES").unwrap_or(&"(未设置)".to_string()),
        );
    }

    info!(
        "环境变量汇总: SteamDeck={}, steamdeck={}, SteamOS={}, SteamAppId={}, STEAM_COMPAT_DATA_PATH={}, WINEPREFIX={}, custom_env_count={}",
        env.get("SteamDeck").unwrap_or(&"(未设置)".to_string()),
        env.get("steamdeck").unwrap_or(&"(未设置)".to_string()),
        env.get("SteamOS").unwrap_or(&"(未设置)".to_string()),
        env.get("SteamAppId").unwrap_or(&"(未设置)".to_string()),
        env.get("STEAM_COMPAT_DATA_PATH")
            .unwrap_or(&"(未设置)".to_string()),
        env.get("WINEPREFIX").unwrap_or(&"(未设置)".to_string()),
        settings.custom_env.len(),
    );

    if let Some(config_data) = target.game_config_data.as_ref() {
        if let Some(gpu_index) = config_data
            .pointer("/other/gpuIndex")
            .and_then(|v| v.as_i64())
        {
            if gpu_index >= 0 {
                let gpus = crate::wine::display::enumerate_gpus();
                if let Some(gpu) = gpus.iter().find(|g| g.index == gpu_index as usize) {
                    if gpu.driver == "nvidia" {
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
                        env.insert(
                            "VK_LOADER_DRIVERS_SELECT".to_string(),
                            "nvidia*".to_string(),
                        );
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
                    env.insert("DRI_PRIME".to_string(), gpu_index.to_string());
                    info!("GPU 选择: DRI_PRIME={} (设备未枚举到，兜底)", gpu_index);
                }
            }
        }

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

    env
}

pub(super) fn align_endfield_umu_default_prefix(env: &mut HashMap<String, String>) {
    let gameid = env
        .get("GAMEID")
        .map(|v| v.trim().to_ascii_lowercase())
        .unwrap_or_default();
    let umu_id = env
        .get("UMU_ID")
        .map(|v| v.trim().to_ascii_lowercase())
        .unwrap_or_default();
    let compat_app = env
        .get("STEAM_COMPAT_APP_ID")
        .map(|v| v.trim().to_ascii_lowercase())
        .unwrap_or_default();
    if gameid != "umu-default" && umu_id != "umu-default" && compat_app != "default" {
        return;
    }

    let Ok(home) = std::env::var("HOME") else {
        return;
    };
    let compat_data_path = PathBuf::from(home).join("Games/umu/umu-default");
    if let Err(err) = std::fs::create_dir_all(compat_data_path.join("shadercache")) {
        warn!(
            "创建 Endfield umu-default compat 目录失败 ({}): {}",
            compat_data_path.display(),
            err
        );
        return;
    }
    let compat_data = compat_data_path.to_string_lossy().to_string();
    env.insert("STEAM_COMPAT_DATA_PATH".to_string(), compat_data.clone());
    env.insert("WINEPREFIX".to_string(), compat_data.clone());
    env.insert(
        "STEAM_COMPAT_SHADER_PATH".to_string(),
        PathBuf::from(&compat_data_path)
            .join("shadercache")
            .to_string_lossy()
            .to_string(),
    );
    env.remove("STEAM_COMPAT_CLIENT_INSTALL_PATH");
    info!(
        "Endfield umu-default 启动：兼容前缀对齐到 {}（与 Lutris 默认行为一致）",
        compat_data
    );
}

fn resolve_numeric_steam_app_id(
    settings: &crate::configs::wine_config::ProtonSettings,
    preset: Option<&crate::configs::game_presets::GamePreset>,
) -> Option<String> {
    let configured = settings.steam_app_id.trim();
    if !configured.is_empty() && configured != "0" && configured.chars().all(|c| c.is_ascii_digit())
    {
        return Some(configured.to_string());
    }

    preset
        .and_then(|p| p.umu_game_id.as_deref())
        .and_then(|id| id.strip_prefix("umu-"))
        .filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()))
        .map(|id| id.to_string())
}

fn resolve_numeric_steam_app_id_from_env(env: &HashMap<String, String>) -> Option<String> {
    for key in [
        "SteamAppId",
        "SteamGameId",
        "STEAMAPPID",
        "STEAM_COMPAT_APP_ID",
    ] {
        let Some(value) = env.get(key) else {
            continue;
        };
        let trimmed = value.trim();
        if !trimmed.is_empty() && trimmed != "0" && trimmed.chars().all(|c| c.is_ascii_digit()) {
            return Some(trimmed.to_string());
        }
    }
    None
}

fn apply_steam_app_id_env(env: &mut HashMap<String, String>, app_id: Option<&str>) {
    const STEAM_APP_KEYS: [&str; 4] = [
        "SteamAppId",
        "SteamGameId",
        "STEAMAPPID",
        "STEAM_COMPAT_APP_ID",
    ];

    match app_id {
        Some(id) if !id.trim().is_empty() && id.trim() != "0" => {
            let normalized = id.trim().to_string();
            for key in STEAM_APP_KEYS {
                env.insert(key.to_string(), normalized.clone());
            }
        }
        _ => {
            for key in STEAM_APP_KEYS {
                if env
                    .get(key)
                    .is_some_and(|value| value.trim().is_empty() || value.trim() == "0")
                {
                    env.remove(key);
                }
            }
        }
    }
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

pub(super) fn find_umu_run_binary() -> Option<PathBuf> {
    which::which("umu-run").ok()
}

pub(super) fn apply_umu_env_defaults(
    _game_preset: &str,
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
                    "umu-default".to_string()
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

    if !env.contains_key("UMU_LOG") {
        env.insert("UMU_LOG".to_string(), "1".to_string());
    }
    if !env.contains_key("PROTON_VERB") {
        env.insert("PROTON_VERB".to_string(), "waitforexitandrun".to_string());
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
        } else {
            env.insert("SteamAppId".to_string(), "default".to_string());
            env.insert("SteamGameId".to_string(), "default".to_string());
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
            env.insert("STEAM_COMPAT_APP_ID".to_string(), numeric_id.clone());
            env.insert("STEAMAPPID".to_string(), numeric_id);
        } else {
            env.insert("STEAM_COMPAT_APP_ID".to_string(), "default".to_string());
            env.insert("STEAMAPPID".to_string(), "default".to_string());
        }
    }

    info!(
        "umu-run env: PROTONPATH={}, GAMEID={}, UMU_ID={}, STORE={}, UMU_USE_STEAM={}, SteamAppId={}, STEAM_COMPAT_APP_ID={}, UMU_LOG={}",
        env.get("PROTONPATH").cloned().unwrap_or_default(),
        env.get("GAMEID").cloned().unwrap_or_default(),
        env.get("UMU_ID").cloned().unwrap_or_default(),
        env.get("STORE").cloned().unwrap_or_default(),
        env.get("UMU_USE_STEAM").cloned().unwrap_or_default(),
        env.get("SteamAppId").cloned().unwrap_or_default(),
        env.get("STEAM_COMPAT_APP_ID").cloned().unwrap_or_default(),
        env.get("UMU_LOG").cloned().unwrap_or_default(),
    );
}

fn apply_lutris_like_proton_defaults(game_preset: &str, env: &mut HashMap<String, String>) {
    if !game_preset.eq_ignore_ascii_case("ArknightsEndfield") {
        return;
    }

    let defaults = [
        ("WINEARCH", "win64"),
        ("WINEDEBUG", "-all"),
        ("DXVK_LOG_LEVEL", "error"),
        ("WINEESYNC", "1"),
        ("WINEFSYNC", "1"),
        ("WINE_LARGE_ADDRESS_AWARE", "1"),
        ("DXVK_ENABLE_NVAPI", "1"),
        ("DXVK_NVAPIHACK", "0"),
        ("PROTON_DXVK_D3D8", "1"),
    ];

    for (key, value) in defaults {
        env.entry(key.to_string())
            .or_insert_with(|| value.to_string());
    }

    match env.get("WINEDLLOVERRIDES") {
        Some(existing) if existing.contains("winemenubuilder=") => {}
        Some(existing) if !existing.trim().is_empty() => {
            env.insert(
                "WINEDLLOVERRIDES".to_string(),
                format!("{};winemenubuilder=", existing.trim()),
            );
        }
        _ => {
            env.insert(
                "WINEDLLOVERRIDES".to_string(),
                "winemenubuilder=".to_string(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_settings() -> crate::configs::wine_config::ProtonSettings {
        crate::configs::wine_config::ProtonSettings::default()
    }

    fn preset_with_umu_id(umu_game_id: Option<&str>) -> crate::configs::game_presets::GamePreset {
        crate::configs::game_presets::GamePreset {
            id: "ArknightsEndfield".to_string(),
            legacy_ids: Vec::new(),
            display_name_en: "Arknights: Endfield".to_string(),
            supported: true,
            migoto_supported: true,
            require_protection_before_launch: true,
            default_folder: "ArknightsEndfield".to_string(),
            launcher_api: None,
            launcher_download_api: None,
            download_servers: Vec::new(),
            download_mode: crate::configs::game_presets::DownloadMode::LauncherInstaller,
            audio_languages: Vec::new(),
            telemetry_servers: Vec::new(),
            telemetry_dlls: Vec::new(),
            channel_protection: None,
            env_defaults: HashMap::new(),
            default_umu_run: false,
            umu_game_id: umu_game_id.map(|value| value.to_string()),
            umu_store: None,
            force_direct_proton: false,
            force_disable_pressure_vessel: false,
            enable_network_log_by_default: false,
        }
    }

    #[test]
    fn resolve_numeric_steam_app_id_skips_zero_and_uses_preset_numeric_id() {
        let settings = default_settings();
        let preset = preset_with_umu_id(Some("umu-2452330"));
        assert_eq!(
            resolve_numeric_steam_app_id(&settings, Some(&preset)),
            Some("2452330".to_string())
        );
    }

    #[test]
    fn apply_steam_app_id_env_removes_zero_value_placeholders() {
        let mut env = HashMap::from([
            ("SteamAppId".to_string(), "0".to_string()),
            ("SteamGameId".to_string(), "0".to_string()),
            ("STEAMAPPID".to_string(), "0".to_string()),
            ("STEAM_COMPAT_APP_ID".to_string(), "0".to_string()),
        ]);

        apply_steam_app_id_env(&mut env, None);

        assert!(!env.contains_key("SteamAppId"));
        assert!(!env.contains_key("SteamGameId"));
        assert!(!env.contains_key("STEAMAPPID"));
        assert!(!env.contains_key("STEAM_COMPAT_APP_ID"));
    }

    #[test]
    fn apply_steam_app_id_env_keeps_consistent_numeric_values() {
        let mut env = HashMap::new();

        apply_steam_app_id_env(&mut env, Some("2452330"));

        assert_eq!(env.get("SteamAppId").map(String::as_str), Some("2452330"));
        assert_eq!(env.get("SteamGameId").map(String::as_str), Some("2452330"));
        assert_eq!(env.get("STEAMAPPID").map(String::as_str), Some("2452330"));
        assert_eq!(
            env.get("STEAM_COMPAT_APP_ID").map(String::as_str),
            Some("2452330")
        );
    }

    #[test]
    fn umu_defaults_use_default_placeholders_for_non_numeric_games() {
        let settings = default_settings();
        let mut env = HashMap::new();

        apply_umu_env_defaults(
            "ArknightsEndfield",
            Path::new("/tmp/proton"),
            &settings,
            None,
            &mut env,
        );

        assert_eq!(env.get("GAMEID").map(String::as_str), Some("umu-default"));
        assert_eq!(env.get("UMU_ID").map(String::as_str), Some("umu-default"));
        assert_eq!(env.get("SteamAppId").map(String::as_str), Some("default"));
        assert_eq!(env.get("SteamGameId").map(String::as_str), Some("default"));
        assert_eq!(
            env.get("STEAM_COMPAT_APP_ID").map(String::as_str),
            Some("default")
        );
        assert_eq!(env.get("STEAMAPPID").map(String::as_str), Some("default"));
    }

    #[test]
    fn endfield_prefers_umu_when_proton_build_and_umu_available() {
        let settings = default_settings();
        assert!(should_prefer_umu_runtime_for_launch_with_availability(
            "ArknightsEndfield",
            "dw_proton-dwproton-10.0-18",
            &settings,
            true,
        ));
    }

    #[test]
    fn non_endfield_does_not_force_umu_override() {
        let settings = default_settings();
        assert!(!should_prefer_umu_runtime_for_launch_with_availability(
            "HonkaiStarRail",
            "dw_proton-dwproton-10.0-18",
            &settings,
            true,
        ));
    }

    #[test]
    fn explicit_umu_setting_or_missing_binary_disables_override() {
        let mut settings = default_settings();
        settings.use_umu_run = true;
        assert!(!should_prefer_umu_runtime_for_launch_with_availability(
            "ArknightsEndfield",
            "dw_proton-dwproton-10.0-18",
            &settings,
            true,
        ));

        let settings = default_settings();
        assert!(!should_prefer_umu_runtime_for_launch_with_availability(
            "ArknightsEndfield",
            "dw_proton-dwproton-10.0-18",
            &settings,
            false,
        ));
    }

    #[test]
    fn inherit_endfield_ime_env_only_inherits_host_values() {
        let mut env = HashMap::new();
        let host = HashMap::from([
            ("XMODIFIERS".to_string(), "@im=fcitx5".to_string()),
            ("GTK_IM_MODULE".to_string(), "fcitx".to_string()),
            ("QT_IM_MODULES".to_string(), "wayland;fcitx".to_string()),
        ]);

        inherit_endfield_ime_env_with_provider("ArknightsEndfield", &mut env, |key| {
            host.get(key).cloned()
        });

        assert_eq!(
            env.get("XMODIFIERS").map(String::as_str),
            Some("@im=fcitx5")
        );
        assert_eq!(env.get("GTK_IM_MODULE").map(String::as_str), Some("fcitx"));
        assert_eq!(env.get("QT_IM_MODULE"), None);
        assert_eq!(env.get("SDL_IM_MODULE"), None);
        assert_eq!(
            env.get("QT_IM_MODULES").map(String::as_str),
            Some("wayland;fcitx")
        );
    }

    #[test]
    fn inherit_endfield_ime_env_skips_non_endfield() {
        let mut env = HashMap::new();

        inherit_endfield_ime_env_with_provider("HonkaiStarRail", &mut env, |_key| {
            Some("dummy".to_string())
        });

        assert!(env.is_empty());
    }
}
