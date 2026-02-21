use std::path::Path;
use tracing_appender::rolling;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

pub fn init_logger(log_dir: &Path) {
    std::fs::create_dir_all(log_dir).ok();

    let file_appender = rolling::daily(log_dir, "ssmt4.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Leak the guard so it lives for the entire program
    Box::leak(Box::new(_guard));

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(
            fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .with_target(true)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true)
                .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE),
        )
        .with(
            fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(true)
                .with_target(true)
                .with_span_events(FmtSpan::CLOSE),
        )
        .init();

    tracing::info!(
        log_dir = %log_dir.display(),
        rust_log = %std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        "Logger initialized"
    );
}
