use serde::{Deserialize, Serialize};

pub const GAME_INFO_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeEnvironment {
    #[default]
    Wine,
    Steam,
    Linux,
}

impl RuntimeEnvironment {
    pub fn from_legacy(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "steam" => Self::Steam,
            "linux" => Self::Linux,
            _ => Self::Wine,
        }
    }

    pub fn as_legacy_str(&self) -> &'static str {
        match self {
            Self::Wine => "wine",
            Self::Steam => "steam",
            Self::Linux => "linux",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "PascalCase")]
pub enum BackgroundType {
    #[default]
    Image,
    #[serde(alias = "Video")]
    Video,
}

impl BackgroundType {
    pub fn from_legacy(value: &str) -> Self {
        let _ = value;
        Self::Image
    }

    pub fn as_legacy_str(&self) -> &'static str {
        let _ = self;
        "Image"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct GameInfoMeta {
    pub display_name: String,
    pub game_preset: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct GameInfoRuntime {
    pub runtime_env: RuntimeEnvironment,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct GameInfoAssets {
    pub background_type: BackgroundType,
    pub icon_file: Option<String>,
    pub background_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GameInfoConfigV2 {
    pub schema_version: u32,
    pub game_name: String,
    pub meta: GameInfoMeta,
    pub runtime: GameInfoRuntime,
    pub assets: GameInfoAssets,
    pub read_only: bool,
    pub warning_code: Option<String>,
}

impl Default for GameInfoConfigV2 {
    fn default() -> Self {
        Self {
            schema_version: GAME_INFO_SCHEMA_VERSION,
            game_name: String::new(),
            meta: GameInfoMeta::default(),
            runtime: GameInfoRuntime::default(),
            assets: GameInfoAssets::default(),
            read_only: false,
            warning_code: None,
        }
    }
}

impl GameInfoConfigV2 {
    #[allow(dead_code)]
    pub fn new(game_name: &str) -> Self {
        let canonical = crate::configs::game_identity::to_canonical_or_keep(game_name);
        Self {
            game_name: canonical.clone(),
            meta: GameInfoMeta {
                game_preset: canonical.clone(),
                display_name: canonical,
            },
            ..Default::default()
        }
    }

    pub fn normalized(mut self, game_name: &str) -> Self {
        let canonical = crate::configs::game_identity::to_canonical_or_keep(game_name);
        self.schema_version = GAME_INFO_SCHEMA_VERSION;
        if self.game_name.trim().is_empty() {
            self.game_name = canonical;
        }
        if self.meta.game_preset.trim().is_empty() {
            self.meta.game_preset = self.game_name.clone();
        } else {
            self.meta.game_preset =
                crate::configs::game_identity::to_canonical_or_keep(&self.meta.game_preset);
        }
        if self.meta.display_name.trim().is_empty() {
            self.meta.display_name =
                crate::configs::game_identity::display_name_en_for_key(&self.game_name)
                    .unwrap_or_else(|| self.game_name.clone());
        } else {
            let candidate = self.meta.display_name.trim();
            let should_replace = candidate.eq_ignore_ascii_case(&self.game_name)
                || candidate.eq_ignore_ascii_case(&self.meta.game_preset)
                || crate::configs::game_identity::legacy_aliases_for_canonical(&self.game_name)
                    .iter()
                    .any(|alias| candidate.eq_ignore_ascii_case(alias));
            if should_replace {
                self.meta.display_name =
                    crate::configs::game_identity::display_name_en_for_key(&self.game_name)
                        .unwrap_or_else(|| self.game_name.clone());
            }
        }
        self.assets.background_type = BackgroundType::Image;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct GameInfoMetaPatch {
    pub display_name: Option<String>,
    pub game_preset: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct GameInfoRuntimePatch {
    pub runtime_env: Option<RuntimeEnvironment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct GameInfoAssetsPatch {
    pub background_type: Option<BackgroundType>,
    pub icon_file: Option<String>,
    pub background_file: Option<String>,
}
