//! Task-augmented MCP server for **serialization probe** tests (`share_runtime = false`).
//!
//! Used only from integration tests; set `CLAP_MCP_SERIAL_PROBE` to a JSONL file path.

mod serial_probe_common;

use clap::Parser;
use clap_mcp::ClapMcp;

#[cfg(feature = "tracing")]
use clap_mcp::ClapMcpConfigProvider;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(
    reinvocation_safe,
    parallel_safe = false,
    share_runtime = false,
    task_augmented_tools
)]
#[clap_mcp_output_from = "run"]
#[command(
    name = "task-serial-probe-dedicated",
    about = "Task tools + serial probe (dedicated runtime)",
    subcommand_required = false
)]
enum Cli {
    #[clap_mcp(task)]
    Sleep {
        #[arg(long)]
        ms: u64,
        #[arg(long)]
        label: String,
        /// Probe metadata only: `"plain"` or `"task"` (mirrors MCP call style).
        #[arg(long, default_value = "plain")]
        call: String,
    },
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::Sleep { ms, label, call } => {
            #[cfg(feature = "tracing")]
            {
                clap_mcp::run_async_tool(&Cli::clap_mcp_config(), move || async move {
                    serial_probe_common::sleep_with_probe(&label, &call, ms).await
                })
                .unwrap_or_else(|e| format!("async error: {e}"))
            }
            #[cfg(not(feature = "tracing"))]
            {
                let _ = (ms, label, call);
                "tracing feature required".to_string()
            }
        }
    }
}

#[cfg(feature = "tracing")]
fn main() {
    let cli = clap_mcp::parse_or_serve_mcp_with_config::<Cli>(Cli::clap_mcp_config());
    match cli {
        Cli::Sleep { .. } => println!("{}", run(cli)),
    }
}

#[cfg(not(feature = "tracing"))]
fn main() {
    eprintln!("This example requires the 'tracing' feature.");
    std::process::exit(1);
}
