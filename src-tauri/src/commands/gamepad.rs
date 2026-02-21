use chrono::Utc;
use gilrs::{Axis, Button, EventType, Gamepad, Gilrs};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::JoinHandle;
use tauri::Emitter;
use tracing::instrument;

const SETTING_DEFAULT_GAMEPAD_ID: &str = "gamepad.default.stable_id";
const SETTING_DEFAULT_GAMEPAD_PLAYER_INDEX: &str = "gamepad.default.player_index";
const GAMEPAD_EVENT_TOPIC: &str = "gamepad-event";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamepadInfo {
    pub runtime_id: String,
    pub stable_id: String,
    pub name: String,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub is_connected: bool,
    pub power: String,
    pub mapping_source: String,
    pub button_count: usize,
    pub axis_count: usize,
    pub xinput_like: bool,
    pub is_default: bool,
    pub player_index: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamepadSelection {
    pub default_stable_id: Option<String>,
    pub default_player_index: Option<u8>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamepadMonitorEvent {
    pub event_type: String,
    pub runtime_id: String,
    pub stable_id: String,
    pub name: String,
    pub connected: bool,
    pub button: Option<String>,
    pub axis: Option<String>,
    pub value: Option<f32>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamepadDiagnostics {
    pub distro: Option<String>,
    pub kernel: Option<String>,
    pub session_type: Option<String>,
    pub desktop: Option<String>,
    pub wayland_display: Option<String>,
    pub x11_display: Option<String>,
    pub gamepads: Vec<GamepadInfo>,
    pub selection: GamepadSelection,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamepadPrefixOverrideStatus {
    pub game_id: String,
    pub prefix_path: String,
    pub user_reg_exists: bool,
    pub xinput_overrides: Vec<String>,
    pub dinput_overrides: Vec<String>,
    pub hid_overrides: Vec<String>,
    pub risky_overrides: Vec<String>,
}

struct MonitorHandle {
    stop: Arc<AtomicBool>,
    join: JoinHandle<()>,
}

static GAMEPAD_MONITOR: Lazy<Mutex<Option<MonitorHandle>>> = Lazy::new(|| Mutex::new(None));

#[tauri::command]
#[instrument(level = "info", skip_all, fields(cmd = "list_gamepads"), err)]
pub fn list_gamepads() -> Result<Vec<GamepadInfo>, String> {
    let mut gilrs = Gilrs::new().map_err(|e| format!("初始化 gilrs 失败: {}", e))?;
    // 拉一轮事件，确保连接状态是最新。
    while gilrs.next_event().is_some() {}
    Ok(collect_gamepads(&gilrs))
}

#[tauri::command]
#[instrument(level = "info", skip_all, fields(cmd = "get_gamepad_selection"), err)]
pub fn get_gamepad_selection() -> Result<GamepadSelection, String> {
    Ok(load_selection())
}

#[tauri::command]
#[instrument(level = "info", skip_all, fields(cmd = "set_gamepad_selection"), err)]
pub fn set_gamepad_selection(
    default_stable_id: Option<String>,
    default_player_index: Option<u8>,
) -> Result<(), String> {
    let normalized_id = default_stable_id
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(idx) = default_player_index {
        if idx == 0 || idx > 16 {
            return Err("玩家编号必须在 1-16 之间".to_string());
        }
    }

    crate::configs::database::set_setting(
        SETTING_DEFAULT_GAMEPAD_ID,
        normalized_id.as_deref().unwrap_or(""),
    );
    crate::configs::database::set_setting(
        SETTING_DEFAULT_GAMEPAD_PLAYER_INDEX,
        &default_player_index
            .map(|v| v.to_string())
            .unwrap_or_default(),
    );

    crate::log_info!(
        event: "gamepad.selection_saved",
        "手柄选择已保存: default={:?}, player={:?}",
        normalized_id,
        default_player_index
    );
    Ok(())
}

#[tauri::command]
#[instrument(level = "info", skip_all, fields(cmd = "get_gamepad_diagnostics"), err)]
pub fn get_gamepad_diagnostics() -> Result<GamepadDiagnostics, String> {
    let mut gilrs = Gilrs::new().map_err(|e| format!("初始化 gilrs 失败: {}", e))?;
    while gilrs.next_event().is_some() {}
    let pads = collect_gamepads(&gilrs);

    let mut notes = vec![
        "若游戏内无手柄输入，优先尝试 Proton runner，并避免在前缀中覆盖 xinput*.dll。".to_string(),
        "若启用 bwrap 沙盒后无输入，建议先关闭沙盒验证是否与 /dev/input 访问有关。".to_string(),
        "蓝牙手柄断开重连后可能变化 runtime_id，建议按 stable_id 重新选择默认手柄。".to_string(),
    ];
    if pads.is_empty() {
        notes.push("当前未检测到任何手柄设备。".to_string());
    }
    if pads.iter().any(|p| !p.xinput_like) {
        notes.push("存在非标准映射设备，部分老游戏可能仅识别 DirectInput/RawInput。".to_string());
    }

    Ok(GamepadDiagnostics {
        distro: read_os_release_pretty_name(),
        kernel: read_uname("-r"),
        session_type: std::env::var("XDG_SESSION_TYPE").ok(),
        desktop: std::env::var("XDG_CURRENT_DESKTOP")
            .ok()
            .or_else(|| std::env::var("DESKTOP_SESSION").ok()),
        wayland_display: std::env::var("WAYLAND_DISPLAY").ok(),
        x11_display: std::env::var("DISPLAY").ok(),
        gamepads: pads,
        selection: load_selection(),
        notes,
    })
}

#[tauri::command]
#[instrument(
    level = "info",
    skip_all,
    fields(cmd = "check_gamepad_prefix_overrides"),
    err
)]
pub fn check_gamepad_prefix_overrides(
    game_id: String,
) -> Result<GamepadPrefixOverrideStatus, String> {
    let canonical_game_id = crate::configs::game_identity::to_canonical_or_keep(&game_id);
    let pfx_dir = crate::wine::prefix::get_prefix_pfx_dir(&canonical_game_id);
    let user_reg = pfx_dir.join("user.reg");

    if !user_reg.exists() {
        return Ok(GamepadPrefixOverrideStatus {
            game_id: canonical_game_id,
            prefix_path: pfx_dir.to_string_lossy().to_string(),
            user_reg_exists: false,
            xinput_overrides: Vec::new(),
            dinput_overrides: Vec::new(),
            hid_overrides: Vec::new(),
            risky_overrides: Vec::new(),
        });
    }

    let content =
        std::fs::read_to_string(&user_reg).map_err(|e| format!("读取 user.reg 失败: {}", e))?;

    let mut in_dll_overrides = false;
    let mut xinput_overrides = Vec::new();
    let mut dinput_overrides = Vec::new();
    let mut hid_overrides = Vec::new();
    let mut risky_overrides = Vec::new();

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_dll_overrides = line.eq_ignore_ascii_case("[Software\\\\Wine\\\\DllOverrides]")
                || line.eq_ignore_ascii_case("[Software\\Wine\\DllOverrides]");
            continue;
        }
        if !in_dll_overrides || !line.starts_with('"') {
            continue;
        }

        let Some((key, value, record)) = parse_override_line(line) else {
            continue;
        };

        if key.starts_with("xinput") {
            xinput_overrides.push(record.clone());
        } else if key.starts_with("dinput") || key.starts_with("dinput8") {
            dinput_overrides.push(record.clone());
        } else if key.starts_with("hid") || key.starts_with("rawinput") {
            hid_overrides.push(record.clone());
        }

        if is_input_override_key(&key) && value.contains("native") {
            risky_overrides.push(record);
        }
    }

    Ok(GamepadPrefixOverrideStatus {
        game_id: canonical_game_id,
        prefix_path: pfx_dir.to_string_lossy().to_string(),
        user_reg_exists: true,
        xinput_overrides,
        dinput_overrides,
        hid_overrides,
        risky_overrides,
    })
}

#[tauri::command]
#[instrument(
    level = "info",
    skip_all,
    fields(cmd = "repair_gamepad_prefix_overrides"),
    err
)]
pub fn repair_gamepad_prefix_overrides(
    game_id: String,
) -> Result<GamepadPrefixOverrideStatus, String> {
    let canonical_game_id = crate::configs::game_identity::to_canonical_or_keep(&game_id);
    let pfx_dir = crate::wine::prefix::get_prefix_pfx_dir(&canonical_game_id);
    let user_reg = pfx_dir.join("user.reg");

    if !user_reg.exists() {
        return check_gamepad_prefix_overrides(canonical_game_id);
    }

    let content =
        std::fs::read_to_string(&user_reg).map_err(|e| format!("读取 user.reg 失败: {}", e))?;
    let had_trailing_newline = content.ends_with('\n');

    let mut in_dll_overrides = false;
    let mut removed_entries = Vec::new();
    let mut kept_lines = Vec::new();

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_dll_overrides = line.eq_ignore_ascii_case("[Software\\\\Wine\\\\DllOverrides]")
                || line.eq_ignore_ascii_case("[Software\\Wine\\DllOverrides]");
            kept_lines.push(raw_line.to_string());
            continue;
        }

        if in_dll_overrides && line.starts_with('"') {
            if let Some((key, _value, record)) = parse_override_line(line) {
                if is_input_override_key(&key) {
                    removed_entries.push(record);
                    continue;
                }
            }
        }

        kept_lines.push(raw_line.to_string());
    }

    if !removed_entries.is_empty() {
        let mut rewritten = kept_lines.join("\n");
        if had_trailing_newline {
            rewritten.push('\n');
        }
        std::fs::write(&user_reg, rewritten).map_err(|e| format!("写入 user.reg 失败: {}", e))?;
    }

    crate::log_info!(
        event: "gamepad.prefix_override_repaired",
        "已修复手柄输入 DLL 覆盖: game_id={}, removed={}",
        canonical_game_id,
        removed_entries.len()
    );
    check_gamepad_prefix_overrides(canonical_game_id)
}

#[tauri::command]
#[instrument(level = "info", skip_all, fields(cmd = "start_gamepad_monitor"), err)]
pub fn start_gamepad_monitor(app: tauri::AppHandle) -> Result<(), String> {
    let mut guard = GAMEPAD_MONITOR.lock().map_err(|e| e.to_string())?;
    if guard.is_some() {
        return Ok(());
    }

    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);
    let join = std::thread::Builder::new()
        .name("ssmt4-gamepad-monitor".to_string())
        .spawn(move || run_monitor_loop(app, stop_flag))
        .map_err(|e| format!("启动手柄监控线程失败: {}", e))?;

    *guard = Some(MonitorHandle { stop, join });
    crate::log_info!(event: "gamepad.monitor_started", "手柄热插拔监控已启动");
    Ok(())
}

#[tauri::command]
#[instrument(level = "info", skip_all, fields(cmd = "stop_gamepad_monitor"), err)]
pub fn stop_gamepad_monitor() -> Result<(), String> {
    let handle = {
        let mut guard = GAMEPAD_MONITOR.lock().map_err(|e| e.to_string())?;
        guard.take()
    };

    if let Some(monitor) = handle {
        monitor.stop.store(true, Ordering::Relaxed);
        if let Err(e) = monitor.join.join() {
            crate::log_warn!(event: "gamepad.monitor_join_failed", "手柄监控线程回收失败: {:?}", e);
        }
        crate::log_info!(event: "gamepad.monitor_stopped", "手柄热插拔监控已停止");
    }
    Ok(())
}

fn run_monitor_loop(app: tauri::AppHandle, stop: Arc<AtomicBool>) {
    let mut gilrs = match Gilrs::new() {
        Ok(g) => g,
        Err(e) => {
            let _ = app.emit(
                GAMEPAD_EVENT_TOPIC,
                &GamepadMonitorEvent {
                    event_type: "monitor_error".to_string(),
                    runtime_id: String::new(),
                    stable_id: String::new(),
                    name: String::new(),
                    connected: false,
                    button: None,
                    axis: None,
                    value: None,
                    timestamp: Utc::now().to_rfc3339(),
                },
            );
            crate::log_error!(event: "gamepad.monitor_init_failed", "初始化手柄监控失败: {}", e);
            return;
        }
    };

    while !stop.load(Ordering::Relaxed) {
        while let Some(ev) = gilrs.next_event() {
            let gamepad = gilrs.gamepad(ev.id);
            let event = match ev.event {
                EventType::Connected => Some(GamepadMonitorEvent {
                    event_type: "connected".to_string(),
                    runtime_id: format!("{:?}", ev.id),
                    stable_id: stable_id(&gamepad),
                    name: gamepad.name().to_string(),
                    connected: true,
                    button: None,
                    axis: None,
                    value: None,
                    timestamp: Utc::now().to_rfc3339(),
                }),
                EventType::Disconnected => Some(GamepadMonitorEvent {
                    event_type: "disconnected".to_string(),
                    runtime_id: format!("{:?}", ev.id),
                    stable_id: stable_id(&gamepad),
                    name: gamepad.name().to_string(),
                    connected: false,
                    button: None,
                    axis: None,
                    value: None,
                    timestamp: Utc::now().to_rfc3339(),
                }),
                EventType::ButtonPressed(btn, _) => Some(GamepadMonitorEvent {
                    event_type: "button_pressed".to_string(),
                    runtime_id: format!("{:?}", ev.id),
                    stable_id: stable_id(&gamepad),
                    name: gamepad.name().to_string(),
                    connected: gamepad.is_connected(),
                    button: Some(format!("{:?}", btn)),
                    axis: None,
                    value: Some(1.0),
                    timestamp: Utc::now().to_rfc3339(),
                }),
                EventType::ButtonReleased(btn, _) => Some(GamepadMonitorEvent {
                    event_type: "button_released".to_string(),
                    runtime_id: format!("{:?}", ev.id),
                    stable_id: stable_id(&gamepad),
                    name: gamepad.name().to_string(),
                    connected: gamepad.is_connected(),
                    button: Some(format!("{:?}", btn)),
                    axis: None,
                    value: Some(0.0),
                    timestamp: Utc::now().to_rfc3339(),
                }),
                EventType::AxisChanged(axis, value, _) => {
                    // 过滤微小抖动，降低事件噪声。
                    if value.abs() < 0.08 {
                        None
                    } else {
                        Some(GamepadMonitorEvent {
                            event_type: "axis_changed".to_string(),
                            runtime_id: format!("{:?}", ev.id),
                            stable_id: stable_id(&gamepad),
                            name: gamepad.name().to_string(),
                            connected: gamepad.is_connected(),
                            button: None,
                            axis: Some(format!("{:?}", axis)),
                            value: Some(value),
                            timestamp: Utc::now().to_rfc3339(),
                        })
                    }
                }
                _ => None,
            };

            if let Some(payload) = event {
                let _ = app.emit(GAMEPAD_EVENT_TOPIC, &payload);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
}

fn collect_gamepads(gilrs: &Gilrs) -> Vec<GamepadInfo> {
    let selection = load_selection();
    let mut list = Vec::new();

    for (id, gp) in gilrs.gamepads() {
        if !gp.is_connected() {
            continue;
        }
        let sid = stable_id(&gp);
        let is_default = selection
            .default_stable_id
            .as_ref()
            .is_some_and(|saved| saved == &sid);

        let mapping_source = format!("{:?}", gp.mapping_source());
        let xinput_like = !mapping_source.eq_ignore_ascii_case("none");
        list.push(GamepadInfo {
            runtime_id: format!("{:?}", id),
            stable_id: sid,
            name: gp.name().to_string(),
            vendor_id: gp.vendor_id(),
            product_id: gp.product_id(),
            is_connected: gp.is_connected(),
            power: format!("{:?}", gp.power_info()),
            mapping_source,
            button_count: known_button_count(&gp),
            axis_count: known_axis_count(&gp),
            xinput_like,
            is_default,
            player_index: if is_default {
                selection.default_player_index
            } else {
                None
            },
        });
    }

    list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    if !list.iter().any(|p| p.is_default) && !list.is_empty() {
        // 如果未命中保存项，默认使用第一个，便于 UI 直接展示。
        list[0].is_default = true;
        list[0].player_index = selection.default_player_index.or(Some(1));
    }

    list
}

fn stable_id(gamepad: &Gamepad<'_>) -> String {
    let vendor = gamepad
        .vendor_id()
        .map(|v| format!("{:04x}", v))
        .unwrap_or_else(|| "0000".to_string());
    let product = gamepad
        .product_id()
        .map(|v| format!("{:04x}", v))
        .unwrap_or_else(|| "0000".to_string());
    let uuid_hex = gamepad
        .uuid()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    let name = gamepad
        .name()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>();
    format!("{}:{}:{}:{}", vendor, product, uuid_hex, name)
}

fn known_button_count(gamepad: &Gamepad<'_>) -> usize {
    let mut count = 0usize;
    for btn in [
        Button::South,
        Button::East,
        Button::North,
        Button::West,
        Button::C,
        Button::Z,
        Button::LeftTrigger,
        Button::LeftTrigger2,
        Button::RightTrigger,
        Button::RightTrigger2,
        Button::Select,
        Button::Start,
        Button::Mode,
        Button::LeftThumb,
        Button::RightThumb,
        Button::DPadUp,
        Button::DPadDown,
        Button::DPadLeft,
        Button::DPadRight,
    ] {
        if gamepad.button_data(btn).is_some() {
            count += 1;
        }
    }
    count
}

fn known_axis_count(gamepad: &Gamepad<'_>) -> usize {
    let mut count = 0usize;
    for axis in [
        Axis::LeftStickX,
        Axis::LeftStickY,
        Axis::LeftZ,
        Axis::RightStickX,
        Axis::RightStickY,
        Axis::RightZ,
        Axis::DPadX,
        Axis::DPadY,
    ] {
        if gamepad.axis_data(axis).is_some() {
            count += 1;
        }
    }
    count
}

fn load_selection() -> GamepadSelection {
    let default_stable_id = crate::configs::database::get_setting(SETTING_DEFAULT_GAMEPAD_ID)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let default_player_index =
        crate::configs::database::get_setting(SETTING_DEFAULT_GAMEPAD_PLAYER_INDEX)
            .and_then(|s| s.trim().parse::<u8>().ok())
            .filter(|v| *v >= 1 && *v <= 16);

    GamepadSelection {
        default_stable_id,
        default_player_index,
    }
}

fn parse_override_line(line: &str) -> Option<(String, String, String)> {
    let eq_idx = line.find('=')?;
    let key_raw = line[..eq_idx].trim();
    let value_raw = line[eq_idx + 1..].trim();
    let key = key_raw.trim_matches('"').to_ascii_lowercase();
    let value = value_raw.trim_matches('"').to_ascii_lowercase();
    let record = format!("{}={}", key, value);
    Some((key, value, record))
}

fn is_input_override_key(key: &str) -> bool {
    key.starts_with("xinput")
        || key.starts_with("dinput")
        || key.starts_with("dinput8")
        || key.starts_with("hid")
        || key.starts_with("rawinput")
}

fn read_uname(flag: &str) -> Option<String> {
    let out = std::process::Command::new("uname")
        .arg(flag)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

fn read_os_release_pretty_name() -> Option<String> {
    let content = std::fs::read_to_string("/etc/os-release").ok()?;
    for line in content.lines() {
        if let Some(value) = line.strip_prefix("PRETTY_NAME=") {
            return Some(value.trim_matches('"').to_string());
        }
    }
    None
}
