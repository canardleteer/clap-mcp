//! MCP server for topical serialization integration tests (`parallel_safe = true`).
//!
//! Set `CLAP_MCP_SERIAL_PROBE` to a JSONL path to record probe events.

mod serial_probe_common;

use clap::Parser;
use clap_mcp::ClapMcp;

#[cfg(feature = "tracing")]
use clap_mcp::ClapMcpConfigProvider;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = true, share_runtime = false)]
#[clap_mcp_output_from = "run"]
#[command(
    name = "topical-serial-probe",
    about = "Topical serialization probe (parallel_safe + serialized tools)",
    subcommand_required = false
)]
enum Cli {
    Search {
        #[arg(long)]
        ms: u64,
        #[arg(long)]
        label: String,
    },
    #[clap_mcp(serialized = "output")]
    Flush {
        #[clap_mcp(serialize_topic)]
        #[arg(long)]
        output: String,
        #[arg(long)]
        ms: u64,
        #[arg(long)]
        label: String,
    },
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::Search { ms, label } => run_probe(&label, "search", ms),
        Cli::Flush { output, ms, label } => {
            let call = format!("flush:{output}");
            run_probe(&label, &call, ms)
        }
    }
}

#[cfg(feature = "tracing")]
fn run_probe(label: &str, call: &str, ms: u64) -> String {
    clap_mcp::run_async_tool(&Cli::clap_mcp_config(), move || async move {
        serial_probe_common::sleep_with_probe(label, call, ms).await
    })
    .unwrap_or_else(|e| format!("async error: {e}"))
}

#[cfg(not(feature = "tracing"))]
fn run_probe(_label: &str, _call: &str, _ms: u64) -> String {
    "tracing feature required".to_string()
}

#[cfg(feature = "tracing")]
fn main() {
    let cli = clap_mcp::parse_or_serve_mcp_with::<Cli>(clap_mcp::ClapMcpRunOptions::from_config(
        Cli::clap_mcp_config(),
    ));
    match cli {
        Cli::Search { .. } | Cli::Flush { .. } => println!("{}", run(cli)),
    }
}

#[cfg(not(feature = "tracing"))]
fn main() {
    eprintln!("This example requires the 'tracing' feature.");
    std::process::exit(1);
}
