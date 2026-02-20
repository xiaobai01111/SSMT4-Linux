use std::path::Path;
use std::process::Command;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init_logger(log_dir: &Path) {
    std::fs::create_dir_all(log_dir).ok();

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let file_layer = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("ssmt4-linux")
        .filename_suffix("log")
        .build(log_dir)
        .ok()
        .map(|file_appender| {
            let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
            // Keep the non-blocking worker alive for the entire process lifetime.
            Box::leak(Box::new(guard));
            fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .with_target(true)
                .with_thread_ids(false)
        });
    let file_logging_enabled = file_layer.is_some();

    tracing_subscriber::registry()
        .with(env_filter)
        .with(file_layer)
        .with(
            fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(true)
                .with_target(true),
        )
        .init();

    if file_logging_enabled {
        tracing::info!("Logger initialized, log_dir={}", log_dir.display());
    } else {
        tracing::warn!(
            "File logger unavailable, using stderr only (log_dir={})",
            log_dir.display()
        );
    }
}

pub fn log_startup_context(args: &[String]) {
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|e| format!("<unknown: {}>", e));
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|e| format!("<unknown: {}>", e));
    let rust_log = std::env::var("RUST_LOG").unwrap_or_else(|_| "<unset>".to_string());

    tracing::info!("Startup executable: {}", exe);
    tracing::info!("Startup current dir: {}", cwd);
    tracing::info!("Startup args: {}", args.join(" "));
    tracing::info!("RUST_LOG: {}", rust_log);

    for key in [
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "XDG_SESSION_TYPE",
        "XDG_CURRENT_DESKTOP",
        "XDG_RUNTIME_DIR",
        "DESKTOP_SESSION",
        "LANG",
        "LC_ALL",
        "WINEPREFIX",
        "STEAM_COMPAT_DATA_PATH",
    ] {
        let value = std::env::var(key).unwrap_or_else(|_| "<unset>".to_string());
        tracing::info!("env {}={}", key, value);
    }
}

pub fn log_runtime_dependency_diagnostics() {
    tracing::info!("Runtime diagnostics start");

    for binary in [
        "wine",
        "wine64",
        "winetricks",
        "umu-run",
        "pressure-vessel-wrap",
        "proton",
        "vulkaninfo",
        "ldd",
        "xdg-open",
    ] {
        match which::which(binary) {
            Ok(path) => tracing::info!("Dependency found: {} -> {}", binary, path.display()),
            Err(_) => tracing::warn!("Dependency missing in PATH: {}", binary),
        }
    }

    log_command_brief("wine --version", "wine", &["--version"]);
    log_command_brief("winetricks --version", "winetricks", &["--version"]);
    log_command_brief("umu-run --version", "umu-run", &["--version"]);
    log_command_brief("vulkaninfo --summary", "vulkaninfo", &["--summary"]);

    if let Ok(exe) = std::env::current_exe() {
        if which::which("ldd").is_ok() {
            match Command::new("ldd").arg(&exe).output() {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let mut missing_count = 0usize;

                    for line in stdout.lines() {
                        if line.contains("not found") {
                            missing_count += 1;
                            tracing::warn!("ldd unresolved: {}", line.trim());
                        }
                    }
                    if !stderr.trim().is_empty() {
                        tracing::warn!("ldd stderr: {}", stderr.trim());
                    }
                    if missing_count == 0 {
                        tracing::info!("ldd check passed: no unresolved shared libraries");
                    }
                }
                Err(e) => tracing::warn!("Failed to run ldd: {}", e),
            }
        }
    }

    tracing::info!("Runtime diagnostics end");
}

fn log_command_brief(label: &str, cmd: &str, args: &[&str]) {
    if which::which(cmd).is_err() {
        return;
    }
    match Command::new(cmd).args(args).output() {
        Ok(output) => {
            let code = output.status.code().unwrap_or(-1);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let first = stdout
                .lines()
                .find(|line| !line.trim().is_empty())
                .or_else(|| stderr.lines().find(|line| !line.trim().is_empty()))
                .unwrap_or("<no output>");
            tracing::info!("{} => exit={} output={}", label, code, first.trim());
        }
        Err(e) => tracing::warn!("{} => failed to execute: {}", label, e),
    }
}
