//! Example: opt-in in-process panic catching (`catch_in_process_panics = true`).
//!
//! With `catch_in_process_panics` enabled, panics in tool code are caught and returned
//! as an MCP error instead of crashing the server. This example has a `panic-demo` tool
//! that panics for demonstration.
//!
//! **Warning:** After a caught panic, the process may no longer be reinvocation_safe;
//! consider restarting the MCP server for reliability.

#![allow(unreachable_code)] // panic! in #[clap_mcp_output] makes generated code unreachable

use clap::Parser;
use clap_mcp::ClapMcp;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false, catch_in_process_panics)]
#[command(
    name = "panic-catch-opt-in",
    about = "Example: in-process panic caught and returned as MCP error",
    subcommand_required = true
)]
enum Cli {
    /// Succeeds and prints a message.
    Succeed,
    /// Panics when invoked (caught and returned as error when catch_in_process_panics is true).
    #[clap_mcp_output = "panic!(\"demo panic for catch_in_process_panics example\")"]
    PanicDemo,
}

fn main() {
    let cli = clap_mcp::parse_or_serve_mcp_attr::<Cli>();

    match cli {
        Cli::Succeed => {
            println!("ok");
        }
        Cli::PanicDemo => {
            panic!("demo panic for catch_in_process_panics example");
        }
    }
}
