use crate::downloader::progress::DownloadProgress;
use serde::Serialize;
use tauri::Emitter;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ComponentDownloadPhase {
    Downloading,
    Extracting,
    Done,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentDownloadProgressEvent {
    pub component_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub component_name: Option<String>,
    pub phase: ComponentDownloadPhase,
    pub downloaded: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GameDownloadOperation {
    DownloadGame,
    UpdateGame,
    UpdateGamePatch,
    DownloadLauncherInstaller,
    UpdateLauncherInstaller,
    VerifyGame,
    RepairGame,
}

#[derive(Debug, Clone, Serialize)]
pub struct GameDownloadProgressEvent {
    pub task_id: String,
    pub operation: GameDownloadOperation,
    pub phase: String,
    pub total_size: u64,
    pub finished_size: u64,
    pub total_count: usize,
    pub finished_count: usize,
    pub current_file: String,
    pub speed_bps: u64,
    pub eta_seconds: u64,
}

impl GameDownloadProgressEvent {
    pub fn from_progress(
        task_id: &str,
        operation: GameDownloadOperation,
        progress: &DownloadProgress,
    ) -> Self {
        Self {
            task_id: task_id.to_string(),
            operation,
            phase: progress.phase.clone(),
            total_size: progress.total_size,
            finished_size: progress.finished_size,
            total_count: progress.total_count,
            finished_count: progress.finished_count,
            current_file: progress.current_file.clone(),
            speed_bps: progress.speed_bps,
            eta_seconds: progress.eta_seconds,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum GameLifecycleEvent {
    #[serde(rename_all = "camelCase")]
    Started {
        game: String,
        pid: u32,
        runner: String,
        region: String,
        launched_at: String,
        configured_exe: String,
        launch_exe: String,
        runner_exe: String,
        command_program: String,
    },
    #[serde(rename_all = "camelCase")]
    Exited {
        game: String,
        pid: u32,
        runner: String,
        region: String,
        launched_at: String,
        finished_at: String,
        exit_code: Option<i32>,
        signal: Option<i32>,
        crashed: bool,
    },
    #[serde(rename_all = "camelCase")]
    BridgeStatus { game: String, message: String },
    #[serde(rename_all = "camelCase")]
    BridgeProgress {
        game: String,
        stage: String,
        current: u32,
        total: u32,
    },
    #[serde(rename_all = "camelCase")]
    BridgeError {
        game: String,
        code: String,
        message: String,
    },
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameAnticheatWarningEvent {
    pub message: String,
}

pub fn emit_component_download_progress(
    app: &tauri::AppHandle,
    payload: &ComponentDownloadProgressEvent,
) {
    let _ = app.emit("component-download-progress", payload);
}

pub fn emit_game_download_progress(app: &tauri::AppHandle, payload: &GameDownloadProgressEvent) {
    let _ = app.emit("game-download-progress", payload);
}

pub fn emit_game_lifecycle(app: &tauri::AppHandle, payload: &GameLifecycleEvent) {
    let _ = app.emit("game-lifecycle", payload);
}

#[cfg(test)]
mod tests {
    use super::{
        ComponentDownloadPhase, ComponentDownloadProgressEvent, GameAnticheatWarningEvent,
        GameDownloadOperation, GameDownloadProgressEvent, GameLifecycleEvent,
    };
    use crate::downloader::progress::DownloadProgress;
    use serde_json::json;

    #[test]
    fn game_download_progress_event_preserves_progress_fields() {
        let progress = DownloadProgress::phase("download")
            .with_sizes(256, 1024)
            .with_counts(1, 4)
            .with_current_file("pkg_01.zip")
            .with_transfer(512, 3);

        let payload = GameDownloadProgressEvent::from_progress(
            "task-123",
            GameDownloadOperation::DownloadGame,
            &progress,
        );

        assert_eq!(payload.task_id, "task-123");
        assert_eq!(payload.phase, "download");
        assert_eq!(payload.finished_size, 256);
        assert_eq!(payload.total_size, 1024);
        assert_eq!(payload.finished_count, 1);
        assert_eq!(payload.total_count, 4);
        assert_eq!(payload.current_file, "pkg_01.zip");
        assert_eq!(payload.speed_bps, 512);
        assert_eq!(payload.eta_seconds, 3);
    }

    #[test]
    fn component_download_phase_and_optional_name_serialize_with_expected_shape() {
        let payload = ComponentDownloadProgressEvent {
            component_id: "dxvk".to_string(),
            component_name: None,
            phase: ComponentDownloadPhase::Extracting,
            downloaded: 50,
            total: 100,
        };

        let value = serde_json::to_value(&payload).expect("serialize component progress");
        assert_eq!(
            value,
            json!({
                "componentId": "dxvk",
                "phase": "extracting",
                "downloaded": 50,
                "total": 100
            })
        );
    }

    #[test]
    fn game_lifecycle_event_serializes_tagged_event_payload() {
        let payload = GameLifecycleEvent::BridgeProgress {
            game: "Snowbreak".to_string(),
            stage: "inject".to_string(),
            current: 2,
            total: 5,
        };

        let value = serde_json::to_value(&payload).expect("serialize lifecycle event");
        assert_eq!(
            value,
            json!({
                "event": "bridge-progress",
                "game": "Snowbreak",
                "stage": "inject",
                "current": 2,
                "total": 5
            })
        );
    }

    #[test]
    fn game_download_operation_and_warning_payload_serialize_with_expected_shape() {
        let operation =
            serde_json::to_value(GameDownloadOperation::UpdateLauncherInstaller).expect("op");
        assert_eq!(operation, json!("update-launcher-installer"));

        let warning = serde_json::to_value(GameAnticheatWarningEvent {
            message: "risk".to_string(),
        })
        .expect("warning");
        assert_eq!(warning, json!({ "message": "risk" }));
    }

    #[test]
    fn game_lifecycle_bridge_error_serializes_kebab_case_tag() {
        let payload = GameLifecycleEvent::BridgeError {
            game: "WutheringWaves".to_string(),
            code: "inject-failed".to_string(),
            message: "hook error".to_string(),
        };

        let value = serde_json::to_value(&payload).expect("serialize bridge error");
        assert_eq!(
            value,
            json!({
                "event": "bridge-error",
                "game": "WutheringWaves",
                "code": "inject-failed",
                "message": "hook error"
            })
        );
    }
}
