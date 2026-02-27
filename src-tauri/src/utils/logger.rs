use chrono::Local;
use std::path::Path;
use tracing::{Event, Level, Subscriber};
use tracing_appender::rolling;
use tracing_subscriber::{
    fmt::{
        self,
        format::{FmtSpan, Writer},
        FormatEvent, FormatFields, FmtContext,
    },
    layer::SubscriberExt,
    registry::LookupSpan,
    util::SubscriberInitExt,
    EnvFilter,
    Layer,
};

#[derive(Clone, Copy, Debug, Default)]
struct ChineseCompactFormatter;

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

fn build_env_filter(var_name: &str, default_directive: &str) -> (EnvFilter, String) {
    let value = std::env::var(var_name)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| std::env::var("SSMT4_LOG_LEVEL").ok().filter(|v| !v.trim().is_empty()))
        .or_else(|| std::env::var("RUST_LOG").ok().filter(|v| !v.trim().is_empty()));

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

pub fn init_logger(log_dir: &Path) {
    std::fs::create_dir_all(log_dir).ok();

    let file_appender = rolling::daily(log_dir, "ssmt4.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Leak the guard so it lives for the entire program
    Box::leak(Box::new(_guard));

    // 级别策略：
    // - 控制台默认 INFO，便于快速定位关键流程
    // - 文件默认全局 INFO，但保留 ssmt4_lib=DEBUG，避免第三方网络库日志洪泛
    let (console_filter, console_directive) = build_env_filter("SSMT4_LOG_CONSOLE", "info");
    let (file_filter, file_directive) = build_env_filter(
        "SSMT4_LOG_FILE",
        "info,ssmt4_lib=debug,reqwest=warn,hyper=warn,h2=warn,rustls=warn",
    );

    let mut file_layer = fmt::layer()
        .event_format(ChineseCompactFormatter)
        .with_writer(non_blocking)
        .with_ansi(false);
    file_layer.set_span_events(FmtSpan::NONE);

    let mut console_layer = fmt::layer()
        .event_format(ChineseCompactFormatter)
        .with_writer(std::io::stderr)
        .with_ansi(true);
    console_layer.set_span_events(FmtSpan::NONE);

    tracing_subscriber::registry()
        .with(file_layer.with_filter(file_filter))
        .with(console_layer.with_filter(console_filter))
        .init();

    tracing::info!(
        "日志系统已启动: 目录={}, 控制台级别={}, 文件级别={}",
        log_dir.display(),
        console_directive,
        file_directive
    );
}
