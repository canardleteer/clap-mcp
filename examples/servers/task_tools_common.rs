//! Shared helpers for task-augmented tool examples.

/// Sleeps asynchronously and returns a short message (for task-augmented `tools/call` demos).
#[cfg(feature = "tracing")]
#[allow(dead_code)]
pub async fn sleep_ms(ms: u64) -> String {
    tracing::info!(ms, "task sleep start");
    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
    format!("slept {}ms", ms)
}

/// Tracing subscriber + MCP log forwarding for `--mcp` (required for logging meta tests).
#[cfg(feature = "tracing")]
pub fn serve_options_with_logging() -> clap_mcp::ClapMcpServeOptions {
    use clap_mcp::logging::{ClapMcpTracingLayer, log_channel};
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let (log_tx, log_rx) = log_channel(64);
    let layer = ClapMcpTracingLayer::new(log_tx);
    // Filter before the MCP layer: without this, rmcp TRACE floods the bounded
    // channel and `try_send` drops tool-body events (including `meta.taskId`
    // under concurrent shared-runtime probes on Windows CI).
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,rmcp=warn,tokio=warn"));
    tracing_subscriber::registry()
        .with(filter)
        .with(layer)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    clap_mcp::ClapMcpServeOptions {
        log_rx: Some(log_rx),
        ..Default::default()
    }
}
