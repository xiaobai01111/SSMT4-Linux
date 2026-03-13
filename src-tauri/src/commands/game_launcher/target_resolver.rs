use super::command_builder::normalize_non_empty;
use super::*;

pub(super) fn resolve_launch_target(
    game_name: &str,
    game_exe: &Path,
    region_override: Option<&str>,
) -> Result<ResolvedLaunchTarget, String> {
    let game_config_data = load_game_config_json(game_name);
    let game_preset = resolve_game_preset_with_data(game_name, game_config_data.as_ref());
    let preset_meta = crate::configs::game_presets::get_preset(&game_preset);
    let launch_launcher_api = resolve_launch_launcher_api(game_config_data.as_ref());
    let launch_biz_prefix = resolve_launch_biz_prefix(
        game_config_data.as_ref(),
        preset_meta,
        launch_launcher_api.as_deref(),
    );
    let launch_region =
        resolve_launch_region(game_config_data.as_ref(), preset_meta, region_override);
    let write_scope_region = process_monitor::derive_region_scope(
        launch_launcher_api.as_deref(),
        launch_biz_prefix.as_deref(),
        Some(&launch_region),
    );
    info!(
        "启动目标: game={}, preset={}, region={}, write_scope_region={}",
        game_name, game_preset, launch_region, write_scope_region
    );
    super::append_game_log(
        game_name,
        "INFO",
        "session",
        format!(
            "launch target: preset={}, region={}, write_scope_region={}, launcher_api={}, biz_prefix={}",
            game_preset,
            launch_region,
            write_scope_region,
            launch_launcher_api.as_deref().unwrap_or(""),
            launch_biz_prefix.as_deref().unwrap_or("")
        ),
    );

    let launch_exe = resolve_preferred_launch_exe(&game_preset, game_exe);
    let game_root = infer_game_root_from_exe(&launch_exe).ok_or_else(|| {
        format!(
            "无法从可执行文件推断游戏目录: {}",
            game_exe.to_string_lossy()
        )
    })?;
    let game_root_str = game_root.to_string_lossy().to_string();
    let write_guard =
        process_monitor::acquire_game_write_guard(&game_root, &write_scope_region, "launch")?;

    Ok(ResolvedLaunchTarget {
        game_config_data,
        game_preset,
        preset_meta,
        launch_region,
        configured_exe_path: game_exe.to_string_lossy().to_string(),
        launch_exe_path: launch_exe.to_string_lossy().to_string(),
        game_root,
        game_root_str,
        launch_exe,
        write_guard,
    })
}

pub(super) fn enforce_launch_protection(
    game_name: &str,
    target: &ResolvedLaunchTarget,
) -> Result<(), String> {
    let mut protection_status = crate::commands::telemetry::check_game_protection_status_internal(
        &target.game_preset,
        Some(&target.game_root_str),
    )?;
    let mut protection_required =
        protection_status.enforce_at_launch || protection_status.supported;
    let mut protection_enabled = protection_status.enabled;

    if !protection_required && target.game_preset != game_name {
        let fallback_status = crate::commands::telemetry::check_game_protection_status_internal(
            game_name,
            Some(&target.game_root_str),
        )?;
        let fallback_required = fallback_status.enforce_at_launch || fallback_status.supported;
        if fallback_required {
            info!(
                "防护判定已从 preset={} 回退到 game_name={}",
                target.game_preset, game_name
            );
            protection_status = fallback_status;
            protection_required = fallback_required;
            protection_enabled = protection_status.enabled;
        }
    }

    let channel = &protection_status.channel;
    let mode = channel.mode.as_deref().unwrap_or("n/a");
    let current = channel
        .current_value
        .map(|v| v.to_string())
        .unwrap_or_else(|| "n/a".to_string());
    let expected = channel
        .expected_value
        .map(|v| v.to_string())
        .unwrap_or_else(|| "n/a".to_string());
    let enforcement = channel.launch_enforcement.as_str();
    info!(
        "Channel mode={}, current={}, expected={}, enforcement={}",
        mode, current, expected, enforcement
    );

    if !protection_required {
        let blocked_domains = protection_status.telemetry.blocked.clone();
        if !blocked_domains.is_empty() {
            warn!(
                "检测到当前游戏已屏蔽域名（{}）。该游戏防护非必需，此设置可能导致联网异常，建议恢复防护后重试",
                blocked_domains.join(", ")
            );
        }
    }

    if protection_required && !protection_enabled {
        let missing_items = protection_status.missing.join("；");

        let detail = if missing_items.is_empty() {
            String::new()
        } else {
            format!(" 详情：{}", missing_items)
        };

        super::append_game_log(
            game_name,
            "ERROR",
            "session",
            format!(
                "protection check failed: enforce=true, enabled=false, missing={}",
                missing_items
            ),
        );
        return Err(format!(
            "未启用应用防护，已阻止启动。请先在“下载/安装游戏”中应用安全防护。{}",
            detail
        ));
    }

    Ok(())
}

pub(super) fn read_non_empty_string(v: Option<&Value>) -> Option<String> {
    v.and_then(|x| x.as_str()).and_then(normalize_non_empty)
}

pub(super) fn load_game_config_json(game_name: &str) -> Option<Value> {
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

pub(super) fn resolve_game_preset_with_data(game_name: &str, data: Option<&Value>) -> String {
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

pub(super) fn resolve_launch_launcher_api(game_config_data: Option<&Value>) -> Option<String> {
    game_config_data.and_then(|data| {
        read_non_empty_string(
            data.pointer("/other/launcherApi")
                .or_else(|| data.pointer("/launcherApi")),
        )
    })
}

pub(super) fn resolve_launch_biz_prefix(
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

pub(super) fn resolve_launch_region(
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

pub(super) fn resolve_game_preset(game_name: &str) -> String {
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

pub(super) fn extract_game_preset_from_config(data: &Value) -> Option<String> {
    data.pointer("/basic/gamePreset")
        .or_else(|| data.pointer("/basic/GamePreset"))
        .or_else(|| data.get("GamePreset"))
        .or_else(|| data.get("LogicName"))
        .or_else(|| data.get("gamePreset"))
        .and_then(|v| v.as_str())
        .map(crate::configs::game_identity::to_canonical_or_keep)
        .filter(|s| !s.is_empty())
}

pub(super) fn infer_game_root_from_exe(game_exe: &Path) -> Option<PathBuf> {
    game_exe.parent().map(|p| p.to_path_buf())
}

#[derive(Debug, Clone)]
pub(super) struct EndfieldLaunchChain {
    pub launcher_exe: PathBuf,
    pub endfield_exe: PathBuf,
}

pub(super) fn find_endfield_launcher_chain(game_exe: &Path) -> Option<EndfieldLaunchChain> {
    let mut search_roots = Vec::new();
    if let Some(parent) = game_exe.parent() {
        search_roots.push(parent.to_path_buf());
    } else {
        search_roots.push(game_exe.to_path_buf());
    }

    for root in search_roots {
        for ancestor in root.ancestors() {
            let launcher_exe = ancestor.join("Launcher.exe");
            let endfield_exe = ancestor.join("games/EndField Game/Endfield.exe");
            if launcher_exe.exists() && endfield_exe.exists() {
                return Some(EndfieldLaunchChain {
                    launcher_exe,
                    endfield_exe,
                });
            }
        }
    }

    None
}

pub(super) fn resolve_preferred_launch_exe(game_preset: &str, game_exe: &Path) -> PathBuf {
    if game_preset == "WutheringWaves" {
        let file_name = game_exe
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if file_name.eq_ignore_ascii_case("Wuthering Waves.exe") {
            if let Some(game_root) = game_exe.parent() {
                let shipping = game_root.join("Client/Binaries/Win64/Client-Win64-Shipping.exe");
                if shipping.exists() {
                    info!(
                        "WutheringWaves 启动可执行已切换为主程序: {}",
                        shipping.display()
                    );
                    return shipping;
                }
            }
        }
    }

    if game_preset == "ArknightsEndfield" {
        if let Some(chain) = find_endfield_launcher_chain(game_exe) {
            if chain.launcher_exe != game_exe {
                info!(
                    "ArknightsEndfield 启动已切换为官方启动器: {} (目标进程: {})",
                    chain.launcher_exe.display(),
                    chain.endfield_exe.display()
                );
            }
            return chain.launcher_exe;
        }
    }

    game_exe.to_path_buf()
}

pub(super) fn normalize_endfield_launcher_start_args(start_args: &mut Vec<String>) {
    let mut normalized = Vec::with_capacity(start_args.len() + 1);
    let mut saw_dx11 = false;

    for arg in start_args.iter() {
        if arg.eq_ignore_ascii_case("-force-d3d11") {
            continue;
        }
        if arg.eq_ignore_ascii_case("-dx11") {
            if !saw_dx11 {
                normalized.push("-dx11".to_string());
                saw_dx11 = true;
            }
            continue;
        }
        normalized.push(arg.clone());
    }

    if !saw_dx11 {
        normalized.push("-dx11".to_string());
    }

    *start_args = normalized;
}

pub(super) fn resolve_preferred_migoto_importer(
    game_preset: &str,
    configured_importer: &str,
) -> String {
    match game_preset {
        "WutheringWaves" => "WWMI".to_string(),
        "ZenlessZoneZero" => "ZZMI".to_string(),
        "HonkaiStarRail" => "SRMI".to_string(),
        "GenshinImpact" | "Genshin" => "GIMI".to_string(),
        "HonkaiImpact3rd" | "Honkai3rd" => "HIMI".to_string(),
        "ArknightsEndfield" => "EFMI".to_string(),
        _ => {
            let normalized = configured_importer.trim();
            if normalized.is_empty() {
                "WWMI".to_string()
            } else {
                normalized.to_ascii_uppercase()
            }
        }
    }
}
