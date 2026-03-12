use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProtonVariant {
    Official,
    Experimental,
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
            ProtonVariant::Experimental => write!(f, "Proton Experimental"),
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
    pub mangohud: bool,
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
            mangohud: false,
            steam_deck_compat: false,
            steamos_compat: false,
            sandbox_enabled: false,
            sandbox_isolate_home: false,
            dxvk_hud: String::new(),
            dxvk_async: false,
            dxvk_frame_rate: 0,
            disable_gpu_filter: false,
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
