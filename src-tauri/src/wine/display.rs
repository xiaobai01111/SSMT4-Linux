use tracing::info;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum DisplayServer {
    X11,
    Wayland,
    XWayland,
    Unknown,
}

/// 单个 GPU 设备信息
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GpuDevice {
    /// PCI 地址，如 "01:00.0"
    pub pci_addr: String,
    /// 设备名称，如 "NVIDIA GeForce RTX 4090"
    pub name: String,
    /// 驱动类型: "nvidia", "amdgpu", "intel", "unknown"
    pub driver: String,
    /// DRI_PRIME 索引（用于切换显卡）
    pub index: usize,
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
    pub gpus: Vec<GpuDevice>,
}

pub fn detect_display_info() -> DisplayInfo {
    let server = detect_display_server();
    let wayland_compositor = detect_wayland_compositor();
    let gpu_driver = detect_gpu_driver();
    let vulkan = crate::wine::graphics::check_vulkan();
    let ime_configured = check_ime_configured();
    let gamepad_detected = check_gamepad();
    let gpus = enumerate_gpus();

    let info = DisplayInfo {
        server,
        wayland_compositor,
        gpu_driver,
        vulkan_available: vulkan.available,
        vulkan_version: vulkan.version,
        ime_configured,
        gamepad_detected,
        gpus,
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

/// 枚举系统中的所有 GPU 设备
pub fn enumerate_gpus() -> Vec<GpuDevice> {
    let mut gpus = Vec::new();

    let output = match std::process::Command::new("lspci").arg("-nnk").output() {
        Ok(o) => o,
        Err(_) => return gpus,
    };
    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut current_pci = String::new();
    let mut current_name = String::new();
    let mut is_vga = false;

    for line in stdout.lines() {
        let trimmed = line.trim();

        // 新设备行格式: "01:00.0 VGA compatible controller: NVIDIA Corporation ..."
        if !trimmed.is_empty() && !trimmed.starts_with('\t') && !trimmed.starts_with(' ') {
            // 先保存上一个 VGA 设备
            if is_vga && !current_name.is_empty() {
                let driver = classify_gpu_driver(&current_name);
                gpus.push(GpuDevice {
                    pci_addr: current_pci.clone(),
                    name: current_name.clone(),
                    driver,
                    index: gpus.len(),
                });
            }
            is_vga = false;
            current_name.clear();
            current_pci.clear();

            let lower = trimmed.to_lowercase();
            if lower.contains("vga")
                || lower.contains("3d controller")
                || lower.contains("display controller")
            {
                is_vga = true;
                // 提取 PCI 地址（行首到第一个空格）
                if let Some(space_pos) = trimmed.find(' ') {
                    current_pci = trimmed[..space_pos].to_string();
                    // 提取设备名（冒号后面的部分）
                    if let Some(colon_pos) = trimmed.find(": ") {
                        current_name = trimmed[colon_pos + 2..].to_string();
                        // 去掉 PCI ID 方括号部分
                        if let Some(bracket_pos) = current_name.rfind(" [") {
                            current_name.truncate(bracket_pos);
                        }
                    }
                }
            }
        }
    }
    // 最后一个
    if is_vga && !current_name.is_empty() {
        let driver = classify_gpu_driver(&current_name);
        gpus.push(GpuDevice {
            pci_addr: current_pci,
            name: current_name,
            driver,
            index: gpus.len(),
        });
    }

    info!("Enumerated {} GPU(s)", gpus.len());
    gpus
}

fn classify_gpu_driver(name: &str) -> String {
    let lower = name.to_lowercase();
    if lower.contains("nvidia") {
        "nvidia".to_string()
    } else if lower.contains("amd") || lower.contains("ati") || lower.contains("radeon") {
        "amdgpu".to_string()
    } else if lower.contains("intel") {
        "intel".to_string()
    } else {
        "unknown".to_string()
    }
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
