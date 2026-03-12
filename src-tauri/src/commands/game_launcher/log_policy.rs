use super::*;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};

pub(super) fn append_game_log(
    game_name: &str,
    level: &str,
    source: &str,
    message: impl AsRef<str>,
) {
    crate::commands::game_log::append_game_log_line(game_name, level, source, message.as_ref());
}

fn is_allowed_env_key_for_log(key: &str) -> bool {
    let upper = key.trim().to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "DISPLAY"
            | "WAYLAND_DISPLAY"
            | "XDG_SESSION_TYPE"
            | "XDG_CURRENT_DESKTOP"
            | "DESKTOP_SESSION"
            | "LANG"
            | "LC_ALL"
            | "LC_MESSAGES"
            | "GDK_BACKEND"
            | "SDL_VIDEODRIVER"
            | "QT_QPA_PLATFORM"
            | "WINEDLLOVERRIDES"
            | "WINEARCH"
            | "WINEDEBUG"
            | "WINEESYNC"
            | "WINEFSYNC"
            | "WINE_FULLSCREEN_FSR"
            | "PROTON_VERB"
            | "UMU_NO_RUNTIME"
            | "UMU_RUNTIME_UPDATE"
            | "MANGOHUD"
            | "ENABLE_GAMESCOPE_WSI"
            | "DISABLE_GAMESCOPE_WSI"
    ) || [
        "WINEPREFIX",
        "PROTON",
        "STEAM_COMPAT_",
        "DXVK_",
        "VKD3D_",
        "MESA_",
        "RADV_",
        "AMD_VULKAN_",
        "__GL_",
        "__NV_",
        "GALLIUM_",
        "GAMESCOPE",
        "SDL_",
    ]
    .iter()
    .any(|prefix| upper.starts_with(prefix))
}

fn is_sensitive_env_key(key: &str) -> bool {
    let upper = key.trim().to_ascii_uppercase();
    [
        "TOKEN",
        "SECRET",
        "PASSWORD",
        "PASSWD",
        "COOKIE",
        "SESSION",
        "AUTH",
        "CREDENTIAL",
        "PRIVATE",
        "SSH_",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "NO_PROXY",
    ]
    .iter()
    .any(|needle| upper.contains(needle))
}

fn is_path_like_key(key: &str) -> bool {
    let upper = key.trim().to_ascii_uppercase();
    upper.contains("PATH") || upper == "WINEPREFIX" || upper == "PROTONPATH"
}

fn looks_like_path(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }
    trimmed.starts_with('/')
        || trimmed.starts_with("~/")
        || trimmed.starts_with("./")
        || trimmed.starts_with("../")
        || trimmed.chars().nth(1).map(|ch| ch == ':').unwrap_or(false)
            && trimmed
                .chars()
                .next()
                .map(|ch| ch.is_ascii_alphabetic())
                .unwrap_or(false)
}

fn sanitize_env_value_for_log(key: &str, value: &str) -> String {
    if is_sensitive_env_key(key) {
        return "<redacted>".to_string();
    }
    if is_path_like_key(key) || looks_like_path(value) {
        return "<path>".to_string();
    }
    if value.len() > 160 {
        let mut truncated = value.chars().take(157).collect::<String>();
        truncated.push_str("...");
        return truncated;
    }
    value.to_string()
}

pub(super) fn format_env_entry_for_log(key: &str, value: &str) -> Option<String> {
    if !is_allowed_env_key_for_log(key) {
        return None;
    }
    Some(format!(
        "{}={}",
        key,
        sanitize_env_value_for_log(key, value)
    ))
}

pub(super) fn sanitize_path_for_log(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| format!("<path:{}>", name))
        .unwrap_or_else(|| "<path>".to_string())
}

fn looks_sensitive_arg(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "token=",
        "password=",
        "secret=",
        "authorization:",
        "bearer ",
        "--token",
        "--password",
        "--secret",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

pub(super) fn sanitize_arg_for_log(value: &str) -> String {
    if looks_sensitive_arg(value) {
        return "<redacted>".to_string();
    }
    if looks_like_path(value) {
        return "<path>".to_string();
    }
    if value.len() > 160 {
        let mut truncated = value.chars().take(157).collect::<String>();
        truncated.push_str("...");
        return truncated;
    }
    value.to_string()
}

pub(super) fn append_sorted_env_snapshot(
    game_name: &str,
    source: &str,
    env_map: &HashMap<String, String>,
) {
    let mut entries: Vec<(&String, &String)> = env_map.iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));
    let mut logged = 0usize;
    let mut suppressed = 0usize;
    for (key, value) in entries {
        if let Some(entry) = format_env_entry_for_log(key, value) {
            logged += 1;
            append_game_log(game_name, "DEBUG", source, entry);
        } else {
            suppressed += 1;
        }
    }
    append_game_log(
        game_name,
        "DEBUG",
        source,
        format!(
            "env snapshot summary: logged={}, suppressed={}",
            logged, suppressed
        ),
    );
}

pub(super) fn append_host_env_snapshot(game_name: &str) {
    let mut entries: Vec<(String, String)> = std::env::vars().collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let mut logged = 0usize;
    let mut suppressed = 0usize;
    for (key, value) in entries {
        if let Some(entry) = format_env_entry_for_log(&key, &value) {
            logged += 1;
            append_game_log(game_name, "DEBUG", "host-env", entry);
        } else {
            suppressed += 1;
        }
    }
    append_game_log(
        game_name,
        "DEBUG",
        "host-env",
        format!(
            "env snapshot summary: logged={}, suppressed={}",
            logged, suppressed
        ),
    );
}

fn is_optional_gstreamer_plugin_warning(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("gstreamer-warning")
        && lower.contains("gstreamer-1.0")
        && (lower.contains("elfclass32")
            || lower.contains("错误的 elf 类")
            || lower.contains("cannot open shared object file")
            || lower.contains("无法打开共享目标文件"))
}

fn is_non_fatal_input_key_warning(line: &str) -> bool {
    line.starts_with("Couldn't get key from code:")
}

pub(super) fn detect_external_log_level(stream: &str, line: &str) -> &'static str {
    if is_optional_gstreamer_plugin_warning(line) || is_non_fatal_input_key_warning(line) {
        return "INFO";
    }

    let normalized = line.to_ascii_lowercase();
    if normalized.contains(" fatal:") || normalized.contains("panic") {
        return "ERROR";
    }
    if normalized.contains(" error:") || normalized.starts_with("error:") {
        return "ERROR";
    }
    if normalized.contains(" warn:") || normalized.contains(" warning:") {
        return "WARN";
    }
    if normalized.contains(" info:") {
        return "INFO";
    }
    if normalized.contains(" debug:") || normalized.contains(" trace:") {
        return "DEBUG";
    }
    if normalized.contains("unimplemented function") && normalized.contains("aborting") {
        return "ERROR";
    }
    if stream == "stderr" {
        "WARN"
    } else {
        "DEBUG"
    }
}

pub(super) fn detect_external_log_source(stream: &str, line: &str) -> String {
    if is_optional_gstreamer_plugin_warning(line) {
        return "GStreamer".to_string();
    }
    if is_non_fatal_input_key_warning(line) {
        return "input-map".to_string();
    }
    if let Some(rest) = line.strip_prefix('[') {
        if let Some((source, _)) = rest.split_once(']') {
            return source.trim().to_string();
        }
    }
    if line.starts_with("ProtonFixes[") {
        return "ProtonFixes".to_string();
    }
    if line.starts_with("Proton:") {
        return "Proton".to_string();
    }
    if line.starts_with("wine:") {
        return "wine".to_string();
    }
    stream.to_string()
}

fn append_external_runtime_hints(game_name: &str, line: &str) {
    if !line.contains("ProtonFixes") {
        let lower = line.to_ascii_lowercase();
        if lower.contains("unimplemented function")
            && lower.contains("ntoskrnl.exe.psgetprocessexitstatus")
            && lower.contains("aborting")
        {
            append_game_log(
                game_name,
                "ERROR",
                "wine-health",
                "检测到 Wine 致命错误：ntoskrnl.exe.PsGetProcessExitStatus 未实现，进程已中止",
            );
        }
        return;
    }

    if line.contains("No global protonfix found") {
        append_game_log(
            game_name,
            "WARN",
            "protonfixes",
            "ProtonFixes 未匹配到全局规则（No global protonfix found）",
        );
    } else if line.contains("Using global defaults") {
        append_game_log(
            game_name,
            "INFO",
            "protonfixes",
            "ProtonFixes 使用全局默认规则（Using global defaults）",
        );
    } else if line.contains("All checks successful") {
        append_game_log(
            game_name,
            "INFO",
            "protonfixes",
            "ProtonFixes 预检查通过（All checks successful）",
        );
    }
}

pub(super) fn attach_launch_log_pipes(child: &mut tokio::process::Child, game_name: &str) {
    if let Some(stdout) = child.stdout.take() {
        spawn_launch_log_pipe(game_name.to_string(), "stdout", stdout);
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_launch_log_pipe(game_name.to_string(), "stderr", stderr);
    }
}

pub(super) fn append_bridge_exit_log(game_name: &str, crashed: bool) {
    let bridge_log = crate::configs::app_config::get_app_data_dir()
        .join("Cache")
        .join("bridge")
        .join("bridge-output.log");
    if !bridge_log.exists() {
        return;
    }

    if let Ok(content) = std::fs::read_to_string(&bridge_log) {
        let level = if crashed { "WARN" } else { "INFO" };
        for line in content.lines().filter(|l| !l.trim().is_empty()) {
            info!("[bridge-log] {}", line);
            append_game_log(game_name, level, "bridge-log", line);
        }
    }
}

fn spawn_launch_log_pipe<R>(game_name: String, stream: &'static str, pipe: R)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut reader = BufReader::new(pipe);
        let mut buf = Vec::with_capacity(4096);
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf).await {
                Ok(0) => break,
                Ok(_) => {
                    while matches!(buf.last(), Some(b'\n' | b'\r')) {
                        buf.pop();
                    }
                    if buf.is_empty() {
                        continue;
                    }
                    let line = String::from_utf8_lossy(&buf).to_string();
                    let level = detect_external_log_level(stream, &line);
                    let source = detect_external_log_source(stream, &line);
                    append_game_log(&game_name, level, &source, &line);
                    append_external_runtime_hints(&game_name, &line);
                    match level {
                        "ERROR" => error!("[{}] [{}] {}", game_name, source, line),
                        "WARN" => warn!("[{}] [{}] {}", game_name, source, line),
                        "INFO" => info!("[{}] [{}] {}", game_name, source, line),
                        _ => debug!("[{}] [{}] {}", game_name, source, line),
                    }
                }
                Err(err) => {
                    warn!("读取 {} 输出失败: {}", stream, err);
                    crate::commands::game_log::append_game_log_line(
                        &game_name,
                        "WARN",
                        "session",
                        &format!("读取 {} 输出失败: {}", stream, err),
                    );
                    break;
                }
            }
        }
    });
}
