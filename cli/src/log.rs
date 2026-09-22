use std::path::PathBuf;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer as _, Registry,
};

pub fn init() -> WorkerGuard {
    let log_dir = log_dir();
    std::fs::create_dir_all(&log_dir).expect("failed to create log directory");

    let file_appender = tracing_appender::rolling::daily(&log_dir, "doze.log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = fmt::layer()
        .with_writer(file_writer)
        .with_ansi(false)
        .with_filter(EnvFilter::new("debug"));

    let terminal_layer = fmt::layer()
        .with_writer(std::io::stderr)
        .without_time()
        .with_target(false)
        .with_filter(
            EnvFilter::try_from_env("DOZE_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        );

    Registry::default()
        .with(file_layer)
        .with(terminal_layer)
        .init();

    guard
}

fn log_dir() -> PathBuf {
    PathBuf::from(".doze").join("logs")
}
