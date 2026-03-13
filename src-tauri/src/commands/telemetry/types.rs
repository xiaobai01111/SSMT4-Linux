use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChannelProtectionPayload {
    pub required: bool,
    pub enabled: bool,
    pub mode: Option<String>,
    pub launch_enforcement: String,
    pub channel_key: Option<String>,
    pub current_value: Option<i64>,
    pub init_value: Option<i64>,
    pub expected_value: Option<i64>,
    pub protected_value: Option<i64>,
    pub config_path: Option<String>,
    pub error: Option<String>,
    pub backup_exists: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TelemetryProtectionPayload {
    pub required: bool,
    pub all_blocked: bool,
    pub blocked: Vec<String>,
    pub unblocked: Vec<String>,
    pub total_servers: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FileProtectionPayload {
    pub required: bool,
    pub all_removed: bool,
    pub removed: Vec<String>,
    pub existing: Vec<String>,
    pub total_files: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameProtectionStatusPayload {
    pub game_preset: String,
    pub supported: bool,
    pub enforce_at_launch: bool,
    pub has_protections: bool,
    pub enabled: bool,
    pub all_protected: bool,
    pub missing: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_root: Option<String>,
    pub telemetry: TelemetryProtectionPayload,
    pub files: FileProtectionPayload,
    pub channel: ChannelProtectionPayload,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TelemetryStatusPayload {
    pub supported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub all_blocked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unblocked: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_servers: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<ChannelProtectionPayload>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtectionDescriptorPayload {
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub servers: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub init_value: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_value: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub launch_enforcement: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_relative_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameProtectionInfoPayload {
    pub game_preset: String,
    pub category: String,
    pub protections: Vec<ProtectionDescriptorPayload>,
    pub has_protections: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChannelProtectionStatusResponse {
    pub game_preset: String,
    pub supported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_root: Option<String>,
    pub channel: ChannelProtectionPayload,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TelemetryActionPayload {
    pub supported: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub newly_blocked: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removed_entries: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub servers: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<ChannelProtectionPayload>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TelemetryWorkflowPayload {
    pub success: bool,
    pub game_preset: String,
    pub telemetry: TelemetryActionPayload,
    pub channel: TelemetryActionPayload,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoveTelemetryFilesPayload {
    pub supported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub removed: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub not_found: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cleanup_warning: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApplyChannelProtectionPayload {
    pub mode: String,
    pub state: ChannelProtectionPayload,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApplyGameProtectionResultsPayload {
    pub telemetry: TelemetryActionPayload,
    pub telemetry_files: RemoveTelemetryFilesPayload,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<ApplyChannelProtectionPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cleanup_warning: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApplyGameProtectionPayload {
    pub success: bool,
    pub game_preset: String,
    pub results: ApplyGameProtectionResultsPayload,
}
