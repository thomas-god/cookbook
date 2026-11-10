//! Application logging.
//!
//! Logs are emitted as JSON to stdout so they can be collected by container runtimes and log
//! shippers without additional parsing. The level is controlled with the standard `RUST_LOG`
//! variable and defaults to `info`.

use tracing_subscriber::{EnvFilter, fmt};

const DEFAULT_FILTER: &str = "info";

/// Installs the global tracing subscriber.
///
/// Returns an error if a subscriber has already been installed.
pub fn init() -> anyhow::Result<()> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));

    fmt()
        .json()
        .flatten_event(true)
        .with_current_span(true)
        .with_span_list(true)
        .with_env_filter(filter)
        .with_writer(std::io::stdout)
        .try_init()
        .map_err(|error| anyhow::anyhow!("failed to install the tracing subscriber: {error}"))?;

    Ok(())
}
