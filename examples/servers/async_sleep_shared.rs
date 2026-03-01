//! Example CLI with tokio async runtime (shared runtime).
//!
//! Run: `cargo run -p clap-mcp-examples --bin async_sleep_shared --features tracing -- sleep-demo`
//! Run: `cargo run -p clap-mcp-examples --bin async_sleep_shared --features tracing -- --mcp`
//!
//! Demonstrates `share_runtime = true` — uses the MCP server's tokio runtime
//! instead of a dedicated thread. Shares the same business logic as async_sleep
//! via the async_sleep_common module.

mod async_sleep_common;

use async_sleep_common::run_sleep_demo;
use clap::Parser;
use clap_mcp::ClapMcp;

#[cfg(feature = "tracing")]
use clap_mcp::ClapMcpConfigProvider;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false, share_runtime)]
#[command(
    name = "async-sleep-shared-example",
    about = "CLI with tokio async runtime: 3 sleep tasks (shared runtime)",
    subcommand_required = false
)]
enum Cli {
    /// Run 3 concurrent sleep tasks and return structured result.
    #[clap_mcp_output_type = "SleepResult"]
    #[clap_mcp_output = "clap_mcp::run_async_tool(&Cli::clap_mcp_config(), || run_sleep_demo())"]
    SleepDemo,
}

#[cfg(feature = "tracing")]
fn main() {
    use clap_mcp::logging::{ClapMcpTracingLayer, log_channel};
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let (log_tx, log_rx) = log_channel(32);
    let layer = ClapMcpTracingLayer::new(log_tx);
    tracing_subscriber::registry()
        .with(layer)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    let serve_options = clap_mcp::ClapMcpServeOptions {
        log_rx: Some(log_rx),
    };

    let cli = clap_mcp::parse_or_serve_mcp_with_config_and_options::<Cli>(
        Cli::clap_mcp_config(),
        serve_options,
    );

    match cli {
        Cli::SleepDemo => {
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime must build")
                .block_on(run_sleep_demo());
            println!("{}", serde_json::to_string_pretty(&result).unwrap());
        }
    }
}

#[cfg(not(feature = "tracing"))]
fn main() {
    eprintln!("This example requires the 'tracing' feature. Run with:");
    eprintln!(
        "  cargo run -p clap-mcp-examples --bin async_sleep_shared --features tracing -- sleep-demo"
    );
    std::process::exit(1);
}
