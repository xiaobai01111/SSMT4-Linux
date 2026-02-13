use tracing::info;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum DisplayServer {
    X11,
    Wayland,
    XWayland,
    Unknown,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DisplayInfo {
    pub server: DisplayServer,
    pub wayland_compositor: Option<String>,
    pub gpu_driver: Option<String>,
    pub vulkan_available: bool,
    pub vulkan_version: Option<String>,
    pub ime_configured: bool,
    pub gamepad_detected: bool,
}

pub fn detect_display_info() -> DisplayInfo {
    let server = detect_display_server();
    let wayland_compositor = detect_wayland_compositor();
    let gpu_driver = detect_gpu_driver();
    let vulkan = crate::wine::graphics::check_vulkan();
    let ime_configured = check_ime_configured();
    let gamepad_detected = check_gamepad();

    let info = DisplayInfo {
        server,
        wayland_compositor,
        gpu_driver,
        vulkan_available: vulkan.available,
        vulkan_version: vulkan.version,
        ime_configured,
        gamepad_detected,
    };

    info!(
        "Display info: server={:?}, vulkan={}, gamepad={}",
        info.server, info.vulkan_available, info.gamepad_detected
    );
    info
}

fn detect_display_server() -> DisplayServer {
    let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    let wayland_display = std::env::var("WAYLAND_DISPLAY").ok();
    let x_display = std::env::var("DISPLAY").ok();

    match session_type.as_str() {
        "wayland" => {
            if x_display.is_some() {
                DisplayServer::XWayland
            } else {
                DisplayServer::Wayland
            }
        }
        "x11" => DisplayServer::X11,
        _ => {
            if wayland_display.is_some() {
                if x_display.is_some() {
                    DisplayServer::XWayland
                } else {
                    DisplayServer::Wayland
                }
            } else if x_display.is_some() {
                DisplayServer::X11
            } else {
                DisplayServer::Unknown
            }
        }
    }
}

fn detect_wayland_compositor() -> Option<String> {
    if let Ok(desktop) = std::env::var("XDG_CURRENT_DESKTOP") {
        return Some(desktop);
    }
    if let Ok(session) = std::env::var("DESKTOP_SESSION") {
        return Some(session);
    }
    None
}

fn detect_gpu_driver() -> Option<String> {
    let output = std::process::Command::new("lspci")
        .arg("-nnk")
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        let lower = line.to_lowercase();
        if lower.contains("vga") || lower.contains("3d controller") || lower.contains("display") {
            if lower.contains("nvidia") {
                return Some("nvidia".to_string());
            } else if lower.contains("amd") || lower.contains("ati") || lower.contains("radeon") {
                return Some("amdgpu".to_string());
            } else if lower.contains("intel") {
                return Some("intel".to_string());
            }
        }
    }
    None
}

fn check_ime_configured() -> bool {
    std::env::var("XMODIFIERS").is_ok()
        || std::env::var("GTK_IM_MODULE").is_ok()
        || std::env::var("QT_IM_MODULE").is_ok()
}

fn check_gamepad() -> bool {
    let js_path = std::path::Path::new("/dev/input");
    if !js_path.exists() {
        return false;
    }
    if let Ok(entries) = std::fs::read_dir(js_path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("js") {
                return true;
            }
        }
    }
    false
}
