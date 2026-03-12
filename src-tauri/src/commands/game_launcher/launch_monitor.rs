use super::*;

pub(super) async fn spawn_monitored_launch(
    app: tauri::AppHandle,
    game_name: String,
    target: ResolvedLaunchTarget,
    prepared: PreparedLaunchCommand,
) -> Result<String, String> {
    let PreparedLaunchCommand {
        launch_profile,
        command_spec,
        runner_name,
        command_program_path,
        runner_exe_path,
    } = prepared;
    let ResolvedLaunchTarget {
        configured_exe_path,
        launch_exe,
        launch_exe_path,
        launch_region: _,
        write_guard,
        ..
    } = target;

    let mut cmd = super::build_launch_process_command(&launch_profile, &command_spec, &launch_exe)?;
    super::apply_launch_working_dir(&mut cmd, &launch_profile, &launch_exe);

    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            format!(
                "启动失败：无法执行启动命令（runner={}, program={}）：{}。请检查所选运行时是否完整、可执行，并确认前缀目录权限正常。",
                runner_name,
                command_program_path,
                e
            )
        })?;

    let started_launch = register_started_launch(LaunchStartedContext {
        app: &app,
        game_name: &game_name,
        pid: child.id().unwrap_or(0),
        command_args: &command_spec.args,
        configured_exe_path: &configured_exe_path,
        launch_exe_path: &launch_exe_path,
        runner_name: &runner_name,
        runner_exe_path: &runner_exe_path,
        command_program_path: &command_program_path,
        launch_exe: &launch_exe,
        region: &launch_profile.runtime_flags.region,
    })
    .await;
    super::attach_launch_log_pipes(&mut child, &game_name);

    let monitor_context = LaunchMonitorContext {
        app,
        game_name,
        region: launch_profile.runtime_flags.region.clone(),
        runner_name,
        launched_at: started_launch.launched_at.clone(),
        pid: started_launch.pid,
        root_start_ticks: started_launch.root_start_ticks,
        exe_name: started_launch.exe_name,
        launch_exe_path: started_launch.launch_exe_path,
    };

    spawn_launch_exit_monitor(child, write_guard, monitor_context);

    Ok(format!("Game launched (PID: {})", started_launch.pid))
}

async fn register_started_launch(context: LaunchStartedContext<'_>) -> StartedLaunch {
    let launched_at = chrono::Utc::now().to_rfc3339();
    let exe_name = context
        .launch_exe
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();
    let root_start_ticks = process_monitor::process_start_ticks(context.pid);

    info!("Game launched with PID {}", context.pid);
    super::append_game_log(
        context.game_name,
        "INFO",
        "session",
        format!(
            "Started initial process {} from {} {}",
            context.pid,
            context.command_program_path,
            context.command_args.join(" ")
        ),
    );
    super::append_game_log(
        context.game_name,
        "INFO",
        "session",
        "Start monitoring process.",
    );

    process_monitor::register_game_process(
        context.game_name.to_string(),
        context.pid,
        context.launch_exe_path.to_string(),
    )
    .await;

    emit_game_lifecycle(
        context.app,
        &GameLifecycleEvent::Started {
            game: context.game_name.to_string(),
            pid: context.pid,
            runner: context.runner_name.to_string(),
            region: context.region.to_string(),
            launched_at: launched_at.clone(),
            configured_exe: context.configured_exe_path.to_string(),
            launch_exe: context.launch_exe_path.to_string(),
            runner_exe: context.runner_exe_path.to_string(),
            command_program: context.command_program_path.to_string(),
        },
    );

    StartedLaunch {
        pid: context.pid,
        root_start_ticks,
        launched_at,
        exe_name,
        launch_exe_path: context.launch_exe_path.to_string(),
    }
}

fn spawn_launch_exit_monitor(
    mut child: tokio::process::Child,
    write_guard: process_monitor::GameWriteGuard,
    context: LaunchMonitorContext,
) {
    tokio::spawn(async move {
        let _write_guard = write_guard;
        let observation = wait_for_initial_child_exit(&mut child, &context).await;
        super::append_bridge_exit_log(&context.game_name, observation.crashed);
        let monitor_timed_out = wait_for_related_game_process_exit(
            &context.game_name,
            &context.exe_name,
            &context.launch_exe_path,
            context.pid,
            context.root_start_ticks,
        )
        .await;

        process_monitor::unregister_game_process(&context.game_name).await;

        let outcome = LaunchMonitorOutcome {
            observation,
            monitor_timed_out,
        };
        log_launch_monitor_summary(&context.game_name, &outcome);
        emit_launch_exit_event(&context, &outcome);
    });
}

async fn wait_for_initial_child_exit(
    child: &mut tokio::process::Child,
    context: &LaunchMonitorContext,
) -> LaunchExitObservation {
    match child.wait().await {
        Ok(status) => {
            let exit_code = status.code();
            let signal = super::exit_status_signal(&status);
            let crashed = exit_code.map(|v| v != 0).unwrap_or(false) || signal.is_some();
            info!("Direct child process exited with status: {}", status);
            debug!(
                "子进程退出诊断: game={}, runner={}, exit_code={:?}, signal={:?}",
                context.game_name, context.runner_name, exit_code, signal
            );
            super::append_game_log(
                &context.game_name,
                "DEBUG",
                "session",
                format!(
                    "child wait done: exit_code={:?}, signal={:?}",
                    exit_code, signal
                ),
            );

            LaunchExitObservation {
                exit_code,
                signal,
                crashed,
            }
        }
        Err(e) => {
            error!("Failed to wait for child process: {}", e);
            super::append_game_log(
                &context.game_name,
                "ERROR",
                "session",
                format!("failed to wait child process: {}", e),
            );
            LaunchExitObservation {
                exit_code: None,
                signal: None,
                crashed: true,
            }
        }
    }
}

async fn wait_for_related_game_process_exit(
    game_name: &str,
    exe_name: &str,
    launch_exe_path: &str,
    root_pid: u32,
    root_start_ticks: Option<u64>,
) -> bool {
    info!("检查游戏 {} 的子进程是否仍在运行...", game_name);
    let mut check_count = 0;
    let max_checks = 30;
    super::append_game_log(
        game_name,
        "INFO",
        "monitor",
        format!(
            "monitor start: exe_name={}, launch_exe_path={}, max_checks={}, interval=1s",
            exe_name, launch_exe_path, max_checks
        ),
    );

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        let processes = process_monitor::find_related_game_processes(
            exe_name,
            Some(launch_exe_path),
            root_pid,
            root_start_ticks,
        )
        .await;

        if processes.is_empty() {
            info!("游戏 {} 的所有进程已退出", game_name);
            super::append_game_log(
                game_name,
                "INFO",
                "monitor",
                "health check: no related process remains",
            );
            return false;
        }

        check_count += 1;
        if check_count >= max_checks {
            info!("游戏 {} 进程检查超时，假定已退出", game_name);
            super::append_game_log(
                game_name,
                "WARN",
                "monitor",
                format!("health check timeout: still alive pids={:?}", processes),
            );
            return true;
        }

        if check_count % 5 == 0 {
            info!("游戏 {} 仍有 {} 个进程在运行", game_name, processes.len());
            super::append_game_log(
                game_name,
                "DEBUG",
                "monitor",
                format!(
                    "health check #{}: alive_count={}, pids={:?}",
                    check_count,
                    processes.len(),
                    processes
                ),
            );
        }
    }
}

fn log_launch_monitor_summary(game_name: &str, outcome: &LaunchMonitorOutcome) {
    let return_code_text = outcome
        .observation
        .exit_code
        .map(|code| code.to_string())
        .unwrap_or_else(|| "none".to_string());
    let level = if outcome.observation.crashed || outcome.monitor_timed_out {
        "WARN"
    } else {
        "INFO"
    };

    super::append_game_log(game_name, "INFO", "session", "Monitored process exited.");
    super::append_game_log(
        game_name,
        "INFO",
        "session",
        format!(
            "Initial process has exited (return code: {}, signal: {:?})",
            return_code_text, outcome.observation.signal
        ),
    );
    super::append_game_log(game_name, "INFO", "session", "All processes have quit");
    super::append_game_log(
        game_name,
        level,
        "session",
        format!(
            "Exit with return code {} (signal: {:?})",
            return_code_text, outcome.observation.signal
        ),
    );
    super::append_game_log(
        game_name,
        level,
        "session",
        format!(
            "health summary: crashed={}, monitor_timed_out={}, exit_code={:?}, signal={:?}",
            outcome.observation.crashed,
            outcome.monitor_timed_out,
            outcome.observation.exit_code,
            outcome.observation.signal
        ),
    );
}

fn emit_launch_exit_event(context: &LaunchMonitorContext, outcome: &LaunchMonitorOutcome) {
    emit_game_lifecycle(
        &context.app,
        &GameLifecycleEvent::Exited {
            game: context.game_name.clone(),
            pid: context.pid,
            runner: context.runner_name.clone(),
            region: context.region.clone(),
            launched_at: context.launched_at.clone(),
            finished_at: chrono::Utc::now().to_rfc3339(),
            exit_code: outcome.observation.exit_code,
            signal: outcome.observation.signal,
            crashed: outcome.observation.crashed,
        },
    );
}
