//! Example CLI with structured output via `#[clap_mcp_output_from]` and `AsStructured`.
//!
//! Run: `cargo run -p clap-mcp-examples --bin structured -- --mcp`

use clap::Parser;
use clap_mcp::{AsStructured, ClapMcp};
use serde::Serialize;

#[derive(Debug, Serialize)]
struct AddResult {
    sum: i32,
    operands: Vec<i32>,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run"]
#[command(
    name = "structured-example",
    about = "CLI with structured MCP output",
    subcommand_required = false
)]
enum Cli {
    /// Add numbers with structured JSON output.
    Add {
        /// First operand.
        a: i32,
        /// Second operand.
        b: i32,
    },
}

fn run(cmd: Cli) -> AsStructured<AddResult> {
    match cmd {
        Cli::Add { a, b } => AsStructured(AddResult {
            sum: a + b,
            operands: vec![a, b],
        }),
    }
}

fn main() {
    let cli = clap_mcp::parse_or_serve_mcp_attr::<Cli>();
    match cli {
        Cli::Add { a, b } => {
            let result = AddResult {
                sum: a + b,
                operands: vec![a, b],
            };
            println!("{} + {} = {}", a, b, result.sum);
            println!(
                "Structured: {}",
                serde_json::to_string_pretty(&result).unwrap()
            );
        }
    }
}
