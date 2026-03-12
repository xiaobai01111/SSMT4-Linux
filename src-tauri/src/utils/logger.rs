use chrono::Local;
use std::io::{self, Write};
use std::path::Path;
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::{
    fmt::{
        self,
        format::{FmtSpan, Writer},
        writer::MakeWriter,
        FmtContext, FormatEvent, FormatFields,
    },
    layer::SubscriberExt,
    registry::LookupSpan,
    util::SubscriberInitExt,
    EnvFilter, Layer,
};

#[derive(Clone, Copy, Debug, Default)]
struct ChineseCompactFormatter;

#[derive(Clone, Copy, Debug, Default)]
struct TeeMakeWriter;

struct TeeWriter {
    stderr: io::Stderr,
    buffer: Vec<u8>,
}

#[derive(Default)]
struct EventVisitor {
    message: Option<String>,
    fields: Vec<(String, String)>,
}

impl EventVisitor {
    fn push_field(&mut self, field_name: &str, value: String) {
        let value = value.replace('\n', "\\n");
        if field_name == "message" {
            self.message = Some(value);
            return;
        }

        // scope/event 在本项目中常与正文重复，默认隐藏，保留其他关键字段
        if matches!(field_name, "scope" | "event") {
            return;
        }

        self.fields.push((field_name.to_string(), value));
    }
}

impl tracing::field::Visit for EventVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.push_field(field.name(), value.to_string());
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.push_field(field.name(), value.to_string());
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.push_field(field.name(), value.to_string());
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.push_field(field.name(), value.to_string());
    }

    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.push_field(field.name(), value.to_string());
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.push_field(field.name(), format!("{value:?}"));
    }
}

fn level_to_chinese(level: &Level) -> &'static str {
    match *level {
        Level::ERROR => "错误",
        Level::WARN => "警告",
        Level::INFO => "信息",
        Level::DEBUG => "调试",
        Level::TRACE => "跟踪",
    }
}

fn short_target(target: &str) -> String {
    let parts: Vec<&str> = target.split("::").collect();
    if parts.len() <= 3 {
        return target.to_string();
    }
    parts[parts.len() - 3..].join("::")
}

impl<S, N> FormatEvent<S, N> for ChineseCompactFormatter
where
    S: Subscriber + for<'span> LookupSpan<'span>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        _ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        let meta = event.metadata();
        let ts = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let level = level_to_chinese(meta.level());
        let target = short_target(meta.target());

        let mut visitor = EventVisitor::default();
        event.record(&mut visitor);

        if let Some(message) = visitor.message.as_deref() {
            write!(writer, "{ts} [{level}] [{target}] {message}")?;
        } else {
            write!(writer, "{ts} [{level}] [{target}]")?;
        }

        if !visitor.fields.is_empty() {
            write!(writer, " | ")?;
            for (idx, (key, value)) in visitor.fields.iter().enumerate() {
                if idx > 0 {
                    write!(writer, ", ")?;
                }
                write!(writer, "{key}={value}")?;
            }
        }

        writeln!(writer)
    }
}

impl<'a> MakeWriter<'a> for TeeMakeWriter {
    type Writer = TeeWriter;

    fn make_writer(&'a self) -> Self::Writer {
        TeeWriter {
            stderr: io::stderr(),
            buffer: Vec::with_capacity(1_024),
        }
    }
}

impl Write for TeeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stderr.write_all(buf)?;
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stderr.flush()
    }
}

impl Drop for TeeWriter {
    fn drop(&mut self) {
        if self.buffer.is_empty() {
            return;
        }

        let text = String::from_utf8_lossy(&self.buffer);
        for line in text.lines() {
            crate::utils::runtime_log::append_runtime_log_line(line);
        }
    }
}

fn resolve_filter_value<'a>(
    explicit: Option<&'a str>,
    legacy: Option<&'a str>,
    rust_log: Option<&'a str>,
) -> Option<&'a str> {
    explicit
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .or_else(|| legacy.map(str::trim).filter(|v| !v.is_empty()))
        .or_else(|| rust_log.map(str::trim).filter(|v| !v.is_empty()))
}

fn build_env_filter(var_name: &str, default_directive: &str) -> (EnvFilter, String) {
    let explicit = std::env::var(var_name).ok();
    let legacy = std::env::var("SSMT4_LOG_LEVEL").ok();
    let rust_log = std::env::var("RUST_LOG").ok();
    let value = resolve_filter_value(explicit.as_deref(), legacy.as_deref(), rust_log.as_deref());

    match value {
        Some(raw) => match EnvFilter::try_new(raw.trim()) {
            Ok(filter) => (filter, raw.trim().to_string()),
            Err(_) => (
                EnvFilter::new(default_directive),
                default_directive.to_string(),
            ),
        },
        None => (
            EnvFilter::new(default_directive),
            default_directive.to_string(),
        ),
    }
}

fn cleanup_legacy_log_files(log_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(log_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with("ssmt4.log") {
            let _ = std::fs::remove_file(path);
        }
    }
}

pub fn init_logger(log_dir: &Path) {
    // 迁移策略：新版本不再写入持久化日志，启动时清理历史 ssmt4.log* 残留文件。
    if log_dir.exists() {
        cleanup_legacy_log_files(log_dir);
    }

    // 仅保留控制台输出，避免本地磁盘日志在会话结束后残留。
    let (console_filter, console_directive) =
        build_env_filter("SSMT4_LOG_CONSOLE", "info,ssmt4_lib=info");

    let mut console_layer = fmt::layer()
        .event_format(ChineseCompactFormatter)
        .with_writer(TeeMakeWriter)
        .with_ansi(true);
    console_layer.set_span_events(FmtSpan::NONE);

    tracing_subscriber::registry()
        .with(console_layer.with_filter(console_filter))
        .init();

    tracing::info!(
        "日志系统已启动: 持久化=禁用, 控制台级别={}, 历史日志目录={}",
        console_directive,
        log_dir.display(),
    );
}

#[cfg(test)]
mod tests {
    use super::{level_to_chinese, resolve_filter_value, short_target};
    use tracing::Level;

    #[test]
    fn level_to_chinese_maps_all_standard_levels() {
        assert_eq!(level_to_chinese(&Level::ERROR), "错误");
        assert_eq!(level_to_chinese(&Level::WARN), "警告");
        assert_eq!(level_to_chinese(&Level::INFO), "信息");
        assert_eq!(level_to_chinese(&Level::DEBUG), "调试");
        assert_eq!(level_to_chinese(&Level::TRACE), "跟踪");
    }

    #[test]
    fn short_target_keeps_last_three_segments_only() {
        assert_eq!(
            short_target("ssmt4_lib::commands::settings"),
            "ssmt4_lib::commands::settings"
        );
        assert_eq!(
            short_target("crate::ssmt4_lib::commands::settings::loader"),
            "commands::settings::loader"
        );
    }

    #[test]
    fn resolve_filter_value_prefers_specific_env_then_legacy_then_rust_log() {
        assert_eq!(
            resolve_filter_value(Some("warn,ssmt4_lib=debug"), Some("info"), Some("trace")),
            Some("warn,ssmt4_lib=debug")
        );
        assert_eq!(
            resolve_filter_value(Some("   "), Some("info"), Some("trace")),
            Some("info")
        );
        assert_eq!(
            resolve_filter_value(None, None, Some("trace")),
            Some("trace")
        );
        assert_eq!(resolve_filter_value(None, None, None), None);
    }

    #[test]
    fn resolve_filter_value_returns_none_when_no_directive_is_selected() {
        assert_eq!(resolve_filter_value(None, None, None), None);
    }
}
