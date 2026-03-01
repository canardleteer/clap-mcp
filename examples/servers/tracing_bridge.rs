//! Example CLI with MCP logging (tracing) integration.
//!
//! Run: `cargo run -p clap-mcp-examples --bin tracing_bridge --features tracing -- echo "hello"`
//! Run: `cargo run -p clap-mcp-examples --bin tracing_bridge --features tracing -- --mcp`
//!
//! When run with --mcp, tracing events are forwarded to the MCP client.

use clap::Parser;
use clap_mcp::ClapMcp;

#[cfg(feature = "tracing")]
use tracing::info;

#[cfg(feature = "tracing")]
use clap_mcp::ClapMcpConfigProvider;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run"]
#[command(
    name = "tracing-bridge-example",
    about = "CLI with MCP logging (tracing)",
    subcommand_required = false
)]
enum Cli {
    /// Echo with tracing.
    Echo {
        /// The string to echo.
        s: String,
    },
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::Echo { s } => format!("Echo: {s}"),
    }
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
        ..Default::default()
    };

    let cli = clap_mcp::parse_or_serve_mcp_with_config_and_options::<Cli>(
        Cli::clap_mcp_config(),
        serve_options,
    );

    match cli {
        Cli::Echo { s } => {
            info!("Echoing: {}", s);
            println!("Echo: {}", s);
        }
    }
}

#[cfg(not(feature = "tracing"))]
fn main() {
    eprintln!("This example requires the 'tracing' feature. Run with:");
    eprintln!("  cargo run -p clap-mcp-examples --bin tracing_bridge --features tracing -- --mcp");
    std::process::exit(1);
}
