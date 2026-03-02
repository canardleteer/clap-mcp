//! Example: subprocess exit handling (`reinvocation_safe = false`).
//!
//! When a tool runs in a subprocess and exits with a non-zero status, the MCP server
//! returns a tool result with `is_error: true` and a message that includes the exit code
//! (and stderr when non-empty). This example has an `exit-fail` tool that exits with
//! code 1 for demonstration.
//!
//! Run with `--mcp` to start the MCP server, or run a tool directly (e.g. `exit-fail`)
//! to see the process exit non-zero.

use clap::Parser;
use clap_mcp::ClapMcp;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe = false, parallel_safe = false)]
#[command(
    name = "subprocess-exit-handling",
    about = "Example: subprocess non-zero exit returns MCP error",
    subcommand_required = true
)]
enum Cli {
    /// Succeeds and prints a message.
    Succeed,
    /// Exits with code 1 (for testing MCP error handling).
    ExitFail,
}

fn main() {
    let cli = clap_mcp::parse_or_serve_mcp_attr::<Cli>();

    match cli {
        Cli::Succeed => {
            println!("ok");
        }
        Cli::ExitFail => {
            eprintln!("exit-fail: exiting with code 1");
            std::process::exit(1);
        }
    }
}
