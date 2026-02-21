use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProtonVariant {
    #[serde(alias = "experimental")]
    Official,
    GEProton,
    DWProton,
    ProtonTKG,
    Lutris,
    SystemWine,
    Custom,
}

impl std::fmt::Display for ProtonVariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtonVariant::Official => write!(f, "Proton"),
            ProtonVariant::GEProton => write!(f, "GE-Proton"),
            ProtonVariant::DWProton => write!(f, "DW-Proton"),
            ProtonVariant::ProtonTKG => write!(f, "Proton-TKG"),
            ProtonVariant::Lutris => write!(f, "Lutris Wine"),
            ProtonVariant::SystemWine => write!(f, "System Wine"),
            ProtonVariant::Custom => write!(f, "Custom"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum WineArch {
    Win32,
    Win64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WineVersion {
    pub id: String,
    pub name: String,
    pub variant: ProtonVariant,
    pub path: PathBuf,
    pub version: String,
    pub arch: WineArch,
    pub supports_dxvk: bool,
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DxvkConfig {
    pub enabled: bool,
    pub version: Option<String>,
}

impl Default for DxvkConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            version: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Vkd3dConfig {
    pub enabled: bool,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PrefixConfig {
    pub wine_version_id: String,
    pub arch: WineArch,
    pub created_at: String,
    pub dxvk: DxvkConfig,
    pub vkd3d: Vkd3dConfig,
    pub installed_runtimes: Vec<String>,
    pub env_overrides: HashMap<String, String>,
    pub template_id: Option<String>,
    pub proton_settings: ProtonSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProtonSettings {
    pub steam_app_id: String,
    pub use_umu_run: bool,
    pub use_pressure_vessel: bool,
    pub proton_media_use_gst: bool,
    pub proton_enable_wayland: bool,
    pub proton_no_d3d12: bool,
    /// 启用 DXVK（关闭时回退 WineD3D）
    pub dxvk_enabled: bool,
    /// 启用 VKD3D（关闭时禁用 D3D12）
    pub vkd3d_enabled: bool,
    /// VKD3D 变体（builtin / disabled / vkd3d / vkd3d-proton / vkd3d-proton-ge）
    pub vkd3d_variant: String,
    /// VKD3D 版本（空字符串表示未指定）
    pub vkd3d_version: String,
    pub mangohud: bool,
    /// 启用全屏 FSR（需要游戏使用全屏并在游戏内设置较低分辨率）
    pub fsr_enabled: bool,
    /// FSR 锐化强度（0-5，数值越高锐化越强）
    pub fsr_strength: u32,
    /// DLSS 兼容模式（启用 NVAPI，兼容时生效）
    pub dlss_compat_enabled: bool,
    /// 垂直同步模式：auto / on / off
    pub vsync_mode: String,
    /// 启用 ESYNC
    pub esync_enabled: bool,
    /// 启用 FSYNC
    pub fsync_enabled: bool,
    /// 分辨率缩放百分比（50-100，100 = 原生）
    pub resolution_scale_percent: u32,
    /// 启用 gamemode（需要系统安装 gamemode）
    pub gamemode_enabled: bool,
    /// 自动性能模式切换（需要 powerprofilesctl）
    pub auto_performance_mode: bool,
    /// 启用 CPU 占用限制（需要系统安装 cpulimit）
    pub cpu_limit_enabled: bool,
    /// CPU 占用上限百分比（1-100）
    pub cpu_limit_percent: u32,
    pub steam_deck_compat: bool,
    pub steamos_compat: bool,
    pub sandbox_enabled: bool,
    pub sandbox_isolate_home: bool,
    /// DXVK HUD 显示模式："" = 关闭, "version" / "fps" / "full" / 自定义
    pub dxvk_hud: String,
    /// 启用 DXVK 异步着色器编译
    pub dxvk_async: bool,
    /// DXVK 帧率限制（0 = 不限制）
    pub dxvk_frame_rate: u32,
    /// 禁用 GPU 自动过滤（DXVK_FILTER_DEVICE_NAME）
    pub disable_gpu_filter: bool,
    /// 手柄兼容模式（off / auto / steam_input / background_input）
    pub gamepad_compat_mode: String,
    pub custom_env: HashMap<String, String>,
}

impl Default for ProtonSettings {
    fn default() -> Self {
        Self {
            steam_app_id: "0".to_string(),
            use_umu_run: false,
            use_pressure_vessel: true,
            proton_media_use_gst: false,
            proton_enable_wayland: false,
            proton_no_d3d12: false,
            dxvk_enabled: true,
            vkd3d_enabled: true,
            vkd3d_variant: "builtin".to_string(),
            vkd3d_version: String::new(),
            mangohud: false,
            fsr_enabled: false,
            fsr_strength: 2,
            dlss_compat_enabled: false,
            vsync_mode: "auto".to_string(),
            esync_enabled: true,
            fsync_enabled: true,
            resolution_scale_percent: 100,
            gamemode_enabled: false,
            auto_performance_mode: false,
            cpu_limit_enabled: false,
            cpu_limit_percent: 100,
            steam_deck_compat: false,
            steamos_compat: false,
            sandbox_enabled: false,
            sandbox_isolate_home: false,
            dxvk_hud: String::new(),
            dxvk_async: false,
            dxvk_frame_rate: 0,
            disable_gpu_filter: false,
            gamepad_compat_mode: "off".to_string(),
            custom_env: HashMap::new(),
        }
    }
}

impl Default for PrefixConfig {
    fn default() -> Self {
        Self {
            wine_version_id: String::new(),
            arch: WineArch::Win64,
            created_at: chrono::Utc::now().to_rfc3339(),
            dxvk: DxvkConfig::default(),
            vkd3d: Vkd3dConfig::default(),
            installed_runtimes: Vec::new(),
            env_overrides: HashMap::new(),
            template_id: None,
            proton_settings: ProtonSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefixTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub recommended_variant: ProtonVariant,
    pub arch: WineArch,
    pub dxvk: DxvkConfig,
    pub vkd3d: Vkd3dConfig,
    pub required_runtimes: Vec<String>,
    pub env_overrides: HashMap<String, String>,
    pub proton_settings: ProtonSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameWineConfig {
    pub game_id: String,
    pub wine_version_id: Option<String>,
    pub prefix_path: Option<PathBuf>,
    pub proton_settings: ProtonSettings,
    pub launcher_api_config: Option<LauncherApiConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherApiConfig {
    pub game_id: String,
    pub launcher_api: String,
    pub launcher_download_api: Option<String>,
}
