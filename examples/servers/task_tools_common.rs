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
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let (log_tx, log_rx) = log_channel(32);
    let layer = ClapMcpTracingLayer::new(log_tx);
    tracing_subscriber::registry()
        .with(layer)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    clap_mcp::ClapMcpServeOptions {
        log_rx: Some(log_rx),
        #[cfg(unix)]
        capture_stdout: false,
        custom_resources: vec![],
        custom_resource_templates: vec![],
        custom_prompts: vec![],
    }
}
