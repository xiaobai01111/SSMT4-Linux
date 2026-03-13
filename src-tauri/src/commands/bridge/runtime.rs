use super::{linux_to_wine_path, write_bridge_config, BridgeConfig};
use crate::commands::game_log::append_game_log_line;
use crate::events::{emit_game_lifecycle, GameLifecycleEvent};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
enum BridgeLogLevel {
    Error,
    Warn,
    Info,
    Debug,
}

#[derive(Debug, Clone)]
enum BridgeMessage {
    Status {
        message: String,
    },
    Progress {
        stage: String,
        current: u32,
        total: u32,
    },
    Warning {
        message: String,
    },
    Error {
        code: String,
        message: String,
    },
    InjectResult {
        method: String,
        success: bool,
        pid: u32,
    },
    Log {
        level: BridgeLogLevel,
        message: String,
    },
    Done {
        success: bool,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BridgeStatusWire {
    #[serde(rename = "type")]
    _msg_type: String,
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BridgeProgressWire {
    #[serde(rename = "type")]
    _msg_type: String,
    stage: String,
    current: u32,
    total: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BridgeWarningWire {
    #[serde(rename = "type")]
    _msg_type: String,
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BridgeErrorWire {
    #[serde(rename = "type")]
    _msg_type: String,
    code: String,
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BridgeInjectResultWire {
    #[serde(rename = "type")]
    _msg_type: String,
    method: String,
    success: bool,
    pid: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BridgeLogWire {
    #[serde(rename = "type")]
    _msg_type: String,
    level: BridgeLogLevel,
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BridgeDoneWire {
    #[serde(rename = "type")]
    _msg_type: String,
    success: bool,
}

fn parse_bridge_message(line: &str) -> Result<BridgeMessage, String> {
    let value = serde_json::from_str::<Value>(line)
        .map_err(|err| format!("invalid bridge JSON: {}", err))?;
    let msg_type = value
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| "bridge message missing string field 'type'".to_string())?;

    match msg_type {
        "status" => serde_json::from_value::<BridgeStatusWire>(value)
            .map(|msg| BridgeMessage::Status {
                message: msg.message,
            })
            .map_err(|err| format!("invalid bridge status message: {}", err)),
        "progress" => serde_json::from_value::<BridgeProgressWire>(value)
            .map(|msg| BridgeMessage::Progress {
                stage: msg.stage,
                current: msg.current,
                total: msg.total,
            })
            .map_err(|err| format!("invalid bridge progress message: {}", err)),
        "warning" => serde_json::from_value::<BridgeWarningWire>(value)
            .map(|msg| BridgeMessage::Warning {
                message: msg.message,
            })
            .map_err(|err| format!("invalid bridge warning message: {}", err)),
        "error" => serde_json::from_value::<BridgeErrorWire>(value)
            .map(|msg| BridgeMessage::Error {
                code: msg.code,
                message: msg.message,
            })
            .map_err(|err| format!("invalid bridge error message: {}", err)),
        "inject_result" => serde_json::from_value::<BridgeInjectResultWire>(value)
            .map(|msg| BridgeMessage::InjectResult {
                method: msg.method,
                success: msg.success,
                pid: msg.pid,
            })
            .map_err(|err| format!("invalid bridge inject_result message: {}", err)),
        "log" => serde_json::from_value::<BridgeLogWire>(value)
            .map(|msg| BridgeMessage::Log {
                level: msg.level,
                message: msg.message,
            })
            .map_err(|err| format!("invalid bridge log message: {}", err)),
        "done" => serde_json::from_value::<BridgeDoneWire>(value)
            .map(|msg| BridgeMessage::Done {
                success: msg.success,
            })
            .map_err(|err| format!("invalid bridge done message: {}", err)),
        other => Err(format!("unsupported bridge message type '{}'", other)),
    }
}

#[derive(Debug)]
pub(crate) struct BridgeLaunchContext<'a> {
    pub(crate) app: &'a tauri::AppHandle,
    pub(crate) game_name: &'a str,
    pub(crate) app_root: &'a Path,
    pub(crate) proton_program: &'a Path,
    pub(crate) proton_args_prefix: &'a [String],
    pub(crate) env: &'a HashMap<String, String>,
    pub(crate) working_dir: &'a Path,
}

#[derive(Debug)]
struct BridgeLaunchPaths {
    bridge_exe: PathBuf,
    config_path: PathBuf,
    bridge_wine_path: String,
    config_wine_path: String,
}

#[derive(Debug, Default)]
struct BridgeRunState {
    game_pid: u32,
}

enum BridgeStreamControl {
    Continue,
    Complete,
    Fail(String),
}

struct BridgeEventDispatcher<'a> {
    app: &'a tauri::AppHandle,
    game_name: &'a str,
}

fn get_bridge_exe_path(app_root: &Path) -> PathBuf {
    app_root.join("Windows").join("ssmt4-bridge.exe")
}

pub(crate) async fn run_bridge(
    bridge_config: &BridgeConfig,
    context: BridgeLaunchContext<'_>,
) -> Result<u32, String> {
    let launch_paths = prepare_bridge_launch_paths(bridge_config, context.app_root)?;
    let mut child = spawn_bridge_process(&context, &launch_paths)?;
    let stdout = child
        .stdout
        .take()
        .ok_or("Failed to capture bridge stdout")?;

    if let Some(stderr) = child.stderr.take() {
        spawn_bridge_stderr_pipe(context.game_name, stderr);
    }

    let dispatcher = BridgeEventDispatcher {
        app: context.app,
        game_name: context.game_name,
    };
    let stdout_result = read_bridge_stdout(&dispatcher, stdout).await;
    let wait_result = wait_for_bridge_exit(&mut child).await;

    match stdout_result {
        Ok(state) => {
            wait_result?;
            Ok(state.game_pid)
        }
        Err(err) => {
            let _ = wait_result;
            Err(err)
        }
    }
}

fn prepare_bridge_launch_paths(
    bridge_config: &BridgeConfig,
    app_root: &Path,
) -> Result<BridgeLaunchPaths, String> {
    let config_path = write_bridge_config(bridge_config, app_root)?;
    let bridge_exe = get_bridge_exe_path(app_root);

    if !bridge_exe.exists() {
        return Err(format!(
            "Bridge executable not found: {}. Please ensure ssmt4-bridge.exe is built and deployed.",
            bridge_exe.display()
        ));
    }

    Ok(BridgeLaunchPaths {
        bridge_wine_path: linux_to_wine_path(&bridge_exe.to_string_lossy()),
        config_wine_path: linux_to_wine_path(&config_path.to_string_lossy()),
        bridge_exe,
        config_path,
    })
}

fn spawn_bridge_process(
    context: &BridgeLaunchContext<'_>,
    launch_paths: &BridgeLaunchPaths,
) -> Result<tokio::process::Child, String> {
    let mut cmd = build_bridge_command(context, launch_paths);
    info!(
        "Launching bridge: {} {:?} {} --config {}",
        context.proton_program.display(),
        context.proton_args_prefix,
        launch_paths.bridge_wine_path,
        launch_paths.config_wine_path
    );

    cmd.spawn().map_err(|e| {
        format!(
            "Failed to start bridge process: {}. Proton={}, Bridge={}, Config={}",
            e,
            context.proton_program.display(),
            launch_paths.bridge_exe.display(),
            launch_paths.config_path.display()
        )
    })
}

fn build_bridge_command(
    context: &BridgeLaunchContext<'_>,
    launch_paths: &BridgeLaunchPaths,
) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(context.proton_program);

    for arg in context.proton_args_prefix {
        cmd.arg(arg);
    }

    cmd.arg(&launch_paths.bridge_wine_path);
    cmd.arg("--config");
    cmd.arg(&launch_paths.config_wine_path);
    cmd.envs(context.env);

    if context.working_dir.exists() {
        cmd.current_dir(context.working_dir);
    }

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd
}

fn spawn_bridge_stderr_pipe(game_name: &str, stderr: tokio::process::ChildStderr) {
    let game_name = game_name.to_string();
    tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            debug!("[bridge stderr] {}", line);
            append_game_log_line(&game_name, "DEBUG", "bridge-stderr", &line);
        }
    });
}

async fn read_bridge_stdout(
    dispatcher: &BridgeEventDispatcher<'_>,
    stdout: tokio::process::ChildStdout,
) -> Result<BridgeRunState, String> {
    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();
    let mut state = BridgeRunState::default();

    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        debug!("[bridge] {}", line);
        append_game_log_line(dispatcher.game_name, "DEBUG", "bridge", &line);

        match parse_bridge_message(&line) {
            Ok(msg) => match dispatch_bridge_message(dispatcher, msg, &mut state) {
                BridgeStreamControl::Continue => {}
                BridgeStreamControl::Complete => break,
                BridgeStreamControl::Fail(err) => return Err(err),
            },
            Err(err) => {
                let error_message = format!("{}; raw={}", err, line);
                error!("[bridge parse] {}", error_message);
                append_game_log_line(
                    dispatcher.game_name,
                    "ERROR",
                    "bridge-protocol",
                    &error_message,
                );
                emit_bridge_event(
                    dispatcher,
                    GameLifecycleEvent::BridgeError {
                        game: dispatcher.game_name.to_string(),
                        code: "BRIDGE_PROTOCOL".to_string(),
                        message: error_message.clone(),
                    },
                );
                return Err(format!("Bridge protocol error: {}", error_message));
            }
        }
    }

    Ok(state)
}

fn dispatch_bridge_message(
    dispatcher: &BridgeEventDispatcher<'_>,
    msg: BridgeMessage,
    state: &mut BridgeRunState,
) -> BridgeStreamControl {
    match msg {
        BridgeMessage::Status { message } => {
            info!("[bridge] {}", message);
            emit_bridge_event(
                dispatcher,
                GameLifecycleEvent::BridgeStatus {
                    game: dispatcher.game_name.to_string(),
                    message,
                },
            );
            BridgeStreamControl::Continue
        }
        BridgeMessage::Progress {
            stage,
            current,
            total,
        } => {
            emit_bridge_event(
                dispatcher,
                GameLifecycleEvent::BridgeProgress {
                    game: dispatcher.game_name.to_string(),
                    stage,
                    current,
                    total,
                },
            );
            BridgeStreamControl::Continue
        }
        BridgeMessage::Warning { message } => {
            warn!("[bridge] {}", message);
            append_game_log_line(dispatcher.game_name, "WARN", "bridge", &message);
            BridgeStreamControl::Continue
        }
        BridgeMessage::Error { code, message } => {
            error!("[bridge] {} - {}", code, message);
            emit_bridge_event(
                dispatcher,
                GameLifecycleEvent::BridgeError {
                    game: dispatcher.game_name.to_string(),
                    code: code.clone(),
                    message: message.clone(),
                },
            );
            BridgeStreamControl::Fail(format!("Bridge error [{}]: {}", code, message))
        }
        BridgeMessage::InjectResult {
            method,
            success,
            pid,
        } => {
            state.game_pid = pid;
            info!(
                "[bridge] Injection {}: method={}, pid={}",
                if success { "succeeded" } else { "failed" },
                method,
                pid
            );
            BridgeStreamControl::Continue
        }
        BridgeMessage::Log { level, message } => {
            let level = match level {
                BridgeLogLevel::Error => "ERROR",
                BridgeLogLevel::Warn => "WARN",
                BridgeLogLevel::Info => "INFO",
                BridgeLogLevel::Debug => "DEBUG",
            };
            append_game_log_line(dispatcher.game_name, level, "bridge", &message);
            BridgeStreamControl::Continue
        }
        BridgeMessage::Done { success } => {
            if success {
                info!("[bridge] Completed successfully");
            } else {
                warn!("[bridge] Completed with failure");
            }
            BridgeStreamControl::Complete
        }
    }
}

fn emit_bridge_event(dispatcher: &BridgeEventDispatcher<'_>, payload: GameLifecycleEvent) {
    emit_game_lifecycle(dispatcher.app, &payload);
}

async fn wait_for_bridge_exit(child: &mut tokio::process::Child) -> Result<(), String> {
    match child.wait().await {
        Ok(status) => {
            if status.success() {
                info!("Bridge process exited successfully");
                Ok(())
            } else {
                let code = status.code().unwrap_or(-1);
                Err(format!("Bridge process exited with code {}", code))
            }
        }
        Err(e) => Err(format!("Failed to wait for bridge process: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_protocol_parses_progress_message_strictly() {
        let message =
            parse_bridge_message(r#"{"type":"progress","stage":"inject","current":2,"total":3}"#)
                .expect("progress message should parse");

        match message {
            BridgeMessage::Progress {
                stage,
                current,
                total,
            } => {
                assert_eq!(stage, "inject");
                assert_eq!(current, 2);
                assert_eq!(total, 3);
            }
            other => panic!("unexpected message: {:?}", other),
        }
    }

    #[test]
    fn bridge_protocol_rejects_progress_without_stage() {
        let error = parse_bridge_message(r#"{"type":"progress","current":2,"total":3}"#)
            .expect_err("missing stage should fail");
        assert!(error.contains("progress"));
        assert!(error.contains("stage"));
    }

    #[test]
    fn bridge_protocol_rejects_unknown_fields_for_error_message() {
        let error =
            parse_bridge_message(r#"{"type":"error","code":"X","message":"boom","extra":"nope"}"#)
                .expect_err("unexpected field should fail");
        assert!(error.contains("error"));
        assert!(error.contains("unknown field"));
    }

    #[test]
    fn bridge_protocol_rejects_unknown_message_type() {
        let error =
            parse_bridge_message(r#"{"type":"mystery","message":"hello"}"#).expect_err("type");
        assert!(error.contains("unsupported bridge message type"));
    }

    #[test]
    fn bridge_protocol_rejects_unknown_log_level() {
        let error =
            parse_bridge_message(r#"{"type":"log","level":"trace","message":"too chatty"}"#)
                .expect_err("unknown log level should fail");
        assert!(error.contains("log"));
        assert!(error.contains("unknown variant"));
    }
}
