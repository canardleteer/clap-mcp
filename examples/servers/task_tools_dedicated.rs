//! MCP task-augmented `tools/call` with a **dedicated async tool runtime** (`share_runtime = false`).
//!
//! Run: `cargo run -p clap-mcp-examples --bin task_tools_dedicated --features tracing -- sleep --ms 100`
//! Run: `cargo run -p clap-mcp-examples --bin task_tools_dedicated --features tracing -- --mcp`
//!
//! Pair with `task_augmented_client` to exercise `task: Some(...)` and poll `tasks/result`.

mod task_tools_common;

#[cfg(feature = "tracing")]
use clap_mcp::ClapMcpConfigProvider;

use clap::Parser;
use clap_mcp::ClapMcp;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(
    reinvocation_safe,
    parallel_safe = false,
    share_runtime = false,
    task_augmented_tools
)]
#[clap_mcp_output_from = "run"]
#[command(
    name = "task-tools-dedicated-example",
    about = "Task-augmented MCP tools (dedicated runtime)",
    subcommand_required = false
)]
enum Cli {
    /// Async sleep (eligible for task-augmented tools/call).
    #[clap_mcp(task)]
    Sleep {
        #[arg(long, default_value_t = 50)]
        ms: u64,
    },
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::Sleep { ms } => {
            #[cfg(feature = "tracing")]
            {
                clap_mcp::run_async_tool(&Cli::clap_mcp_config(), move || async move {
                    task_tools_common::sleep_ms(ms).await
                })
                .unwrap_or_else(|e| format!("async error: {e}"))
            }
            #[cfg(not(feature = "tracing"))]
            {
                let _ = ms;
                "tracing feature required".to_string()
            }
        }
    }
}

#[cfg(feature = "tracing")]
fn main() {
    let serve_options = task_tools_common::serve_options_with_logging();
    let cli = clap_mcp::parse_or_serve_mcp_with::<Cli>(clap_mcp::ClapMcpRunOptions {
        config: Cli::clap_mcp_config(),
        serve: serve_options,
    });
    match cli {
        Cli::Sleep { .. } => println!("{}", run(cli)),
    }
}

#[cfg(not(feature = "tracing"))]
fn main() {
    eprintln!("This example requires the 'tracing' feature.");
    std::process::exit(1);
}
