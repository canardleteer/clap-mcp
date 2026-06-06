//! Example CLI using imperative async embedder serve ([`ServeMcpBuilder`]).
//!
//! Run: `cargo run -p clap-mcp-examples --bin async_embedder_serve --features tracing -- sleep-demo`
//! Run: `cargo run -p clap-mcp-examples --bin async_embedder_serve --features tracing -- --mcp`
//!
//! Demonstrates `share_runtime = true` with [`ServeMcpBuilder::serve`] on the caller's tokio
//! runtime. Same business logic as `async_sleep_shared` (derive + `parse_or_serve_mcp` path);
//! this binary shows the low-level async embedder pattern for `#[tokio::main]` apps.

mod async_sleep_common;

use async_sleep_common::run_sleep_demo;
use clap::{CommandFactory, FromArgMatches, Parser};
use clap_mcp::{AsStructured, ClapMcp, ClapMcpConfigProvider, CLAP_MCP_STDIO_FLAG_ID};

#[cfg(feature = "tracing")]
use clap_mcp::{McpListen, ServeMcpBuilder, command_with_mcp_flag};

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false, share_runtime)]
#[clap_mcp_output_from = "run"]
#[command(
    name = "async-embedder-serve-example",
    about = "Async embedder ServeMcpBuilder example: 3 sleep tasks (shared runtime)",
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
#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), clap_mcp::ClapMcpError> {
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
        elicitation_enabled: false,
    };

    let cmd = command_with_mcp_flag(Cli::command());
    let matches = cmd.get_matches();

    if matches.get_flag(CLAP_MCP_STDIO_FLAG_ID) {
        ServeMcpBuilder::for_cli::<Cli>(McpListen::Stdio)
            .serve_options(serve_options)
            .serve()
            .await?;
        return Ok(());
    }

    let cli = Cli::from_arg_matches(&matches).expect("parse cli");
    match cli {
        Cli::SleepDemo => {
            let result = run(cli);
            println!("{}", serde_json::to_string_pretty(&result.0).unwrap());
        }
    }
    Ok(())
}

#[cfg(not(feature = "tracing"))]
fn main() {
    eprintln!("This example requires the 'tracing' feature. Run with:");
    eprintln!(
        "  cargo run -p clap-mcp-examples --bin async_embedder_serve --features tracing -- sleep-demo"
    );
    std::process::exit(1);
}
