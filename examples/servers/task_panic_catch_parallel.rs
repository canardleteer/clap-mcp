//! Task-augmented MCP server with `catch_in_process_panics = true` and
//! `parallel_safe = true` for integration tests.

#![allow(unreachable_code)]

use clap::Parser;
use clap_mcp::ClapMcp;

#[cfg(feature = "tracing")]
use clap_mcp::ClapMcpConfigProvider;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(
    reinvocation_safe,
    parallel_safe = true,
    catch_in_process_panics,
    task_augmented_tools
)]
#[clap_mcp_output_from = "run"]
#[command(
    name = "task-panic-catch-parallel",
    about = "Task-augmented tools with panic catching (parallel_safe)",
    subcommand_required = false
)]
enum Cli {
    #[clap_mcp(task)]
    Sleep {
        #[arg(long, default_value_t = 10)]
        ms: u64,
    },
    #[clap_mcp(task)]
    PanicDemo,
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::Sleep { ms } => {
            #[cfg(feature = "tracing")]
            {
                clap_mcp::run_async_tool(&Cli::clap_mcp_config(), move || async move {
                    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
                    format!("slept {ms}ms")
                })
                .unwrap_or_else(|e| format!("async error: {e}"))
            }
            #[cfg(not(feature = "tracing"))]
            {
                let _ = ms;
                "tracing feature required".to_string()
            }
        }
        Cli::PanicDemo => panic!("demo panic in parallel task-augmented tool"),
    }
}

#[cfg(feature = "tracing")]
fn main() {
    let cli = clap_mcp::parse_or_serve_mcp_with::<Cli>(clap_mcp::ClapMcpRunOptions::from_config(
        Cli::clap_mcp_config(),
    ));
    match cli {
        Cli::Sleep { .. } | Cli::PanicDemo => println!("{}", run(cli)),
    }
}

#[cfg(not(feature = "tracing"))]
fn main() {
    eprintln!("This example requires the 'tracing' feature.");
    std::process::exit(1);
}
