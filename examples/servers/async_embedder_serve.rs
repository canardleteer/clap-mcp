//! Example CLI using imperative async embedder serve (`serve_mcp`).
//!
//! Run: `cargo run -p clap-mcp-examples --bin async_embedder_serve --features tracing -- sleep-demo`
//! Run: `cargo run -p clap-mcp-examples --bin async_embedder_serve --features tracing -- --mcp`
//!
//! Demonstrates `share_runtime = true` with [`clap_mcp::serve_mcp`] on the caller's tokio
//! runtime. Same business logic as `async_sleep_shared` (derive + `parse_or_serve_mcp` path);
//! this binary shows the low-level async embedder pattern for `#[tokio::main]` apps.

mod async_sleep_common;

use async_sleep_common::run_sleep_demo;
use clap::{CommandFactory, FromArgMatches, Parser};
use clap_mcp::{AsStructured, ClapMcp, ClapMcpSchemaMetadataProvider};

#[cfg(feature = "tracing")]
use clap_mcp::{
    ClapMcpConfigProvider, McpListen, in_process_tool_handler_for,
    schema_from_command_with_metadata, serve_mcp,
};

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false, share_runtime)]
#[clap_mcp_output_from = "run"]
#[command(
    name = "async-embedder-serve-example",
    about = "Async embedder serve_mcp example: 3 sleep tasks (shared runtime)",
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

    let config = Cli::clap_mcp_config();
    let metadata = Cli::clap_mcp_schema_metadata();
    let base_cmd = Cli::command();
    let schema = schema_from_command_with_metadata(&base_cmd, &metadata);
    let schema_json = serde_json::to_string_pretty(&schema).expect("schema should serialize");

    let serve_options = clap_mcp::ClapMcpServeOptions {
        log_rx: Some(log_rx),
        #[cfg(unix)]
        capture_stdout: false,
        custom_resources: vec![],
        custom_prompts: vec![],
        elicitation_enabled: false,
    };

    if std::env::args().any(|arg| arg == "--mcp") {
        let in_process_handler = in_process_tool_handler_for::<Cli>(schema, false);
        serve_mcp(
            McpListen::Stdio,
            schema_json,
            None,
            config,
            Some(in_process_handler),
            serve_options,
            &metadata,
        )
        .await?;
        return Ok(());
    }

    let matches = base_cmd.get_matches();
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
