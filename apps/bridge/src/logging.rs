//! Journalisation locale avec rotation quotidienne.
//!
//! Règle absolue (§2.2) : ni token, ni credentials dans les logs. Les métadonnées de piste
//! ne sont journalisées qu'au niveau `debug`.

use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

/// Le guard doit rester vivant pendant toute la durée du processus, sinon les écritures
/// asynchrones sont perdues.
pub struct LoggingGuard(#[allow(dead_code)] Option<WorkerGuard>);

pub fn init(log_dir: &Path, level: &str, also_stderr: bool) -> LoggingGuard {
    let filter = EnvFilter::try_new(format!("dezzer_bridge={level},tower_http=warn"))
        .unwrap_or_else(|_| EnvFilter::new("dezzer_bridge=info"));

    let file_layer = match std::fs::create_dir_all(log_dir) {
        Ok(()) => {
            let appender = tracing_appender::rolling::Builder::new()
                .rotation(tracing_appender::rolling::Rotation::DAILY)
                .filename_prefix("dezzer-bridge")
                .filename_suffix("log")
                .max_log_files(5)
                .build(log_dir)
                .ok();
            appender.map(tracing_appender::non_blocking)
        }
        Err(_) => None,
    };

    let (guard, layer) = match file_layer {
        Some((writer, guard)) => (
            Some(guard),
            Some(
                tracing_subscriber::fmt::layer()
                    .with_writer(writer)
                    .with_ansi(false)
                    .with_target(false),
            ),
        ),
        None => (None, None),
    };

    let registry = tracing_subscriber::registry().with(filter).with(layer);

    if also_stderr {
        let _ = registry
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(std::io::stderr)
                    .with_target(false),
            )
            .try_init();
    } else {
        let _ = registry.try_init();
    }

    LoggingGuard(guard)
}
