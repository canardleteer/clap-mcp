//! Example CLI with tokio async runtime (dedicated thread).
//!
//! Run: `cargo run -p clap-mcp-examples --bin async_sleep --features tracing -- sleep-demo`
//! Run: `cargo run -p clap-mcp-examples --bin async_sleep --features tracing -- --mcp`
//!
//! Demonstrates `share_runtime = false` — uses a dedicated thread with its own
//! tokio runtime per tool call. See async_sleep_shared for the shared-runtime variant.

mod async_sleep_common;

use async_sleep_common::run_sleep_demo;
use clap::Parser;
use clap_mcp::{AsStructured, ClapMcp};

#[cfg(feature = "tracing")]
use clap_mcp::ClapMcpConfigProvider;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false, share_runtime = false)]
#[clap_mcp_output_from = "run"]
#[command(
    name = "async-sleep-example",
    about = "CLI with tokio async runtime: 3 sleep tasks (dedicated thread)",
    subcommand_required = false
)]
enum Cli {
    /// Run 3 concurrent sleep tasks and return structured result.
    SleepDemo,
}

fn run(cmd: Cli) -> AsStructured<async_sleep_common::SleepResult> {
    match cmd {
        Cli::SleepDemo => AsStructured(
            clap_mcp::run_async_tool(&Cli::clap_mcp_config(), run_sleep_demo)
                .expect("async tool failed"),
        ),
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
        #[cfg(unix)]
        capture_stdout: false,
        custom_resources: vec![],
        custom_prompts: vec![],
    };

    let cli = clap_mcp::parse_or_serve_mcp_with_config_and_options::<Cli>(
        Cli::clap_mcp_config(),
        serve_options,
    );

    match cli {
        Cli::SleepDemo => {
            let result = run(cli);
            println!("{}", serde_json::to_string_pretty(&result.0).unwrap());
        }
    }
}

#[cfg(not(feature = "tracing"))]
fn main() {
    eprintln!("This example requires the 'tracing' feature. Run with:");
    eprintln!(
        "  cargo run -p clap-mcp-examples --bin async_sleep --features tracing -- sleep-demo"
    );
    std::process::exit(1);
}
