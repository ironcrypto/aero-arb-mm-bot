//! Logging setup and configuration

use anyhow::Result;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub struct LoggingGuard {
    pub _guard: tracing_appender::non_blocking::WorkerGuard,
}

pub fn setup_logging() -> Result<Arc<LoggingGuard>> {
    let file_appender = tracing_appender::rolling::hourly("output/logs", "aerodrome-bot.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_thread_ids(false)
                .with_ansi(true)
                .with_level(true)
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking)
                .with_target(true)
                .with_thread_ids(false)
                .with_level(true)
                .with_ansi(false)
                .compact()
        )
        .with(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("info".parse()?)
        )
        .init();
    
    Ok(Arc::new(LoggingGuard { _guard: guard }))
}

pub fn setup_output_directories() -> Result<()> {
    use std::fs;
    
    fs::create_dir_all("output/logs")?;
    fs::create_dir_all("output/opportunities")?;
    fs::create_dir_all("output/reports")?;
    fs::create_dir_all("output/market_making")?;
    fs::create_dir_all("output/executions")?;
    
    Ok(())
}
