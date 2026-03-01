//! Example CLI with structured output via #[clap_mcp_output_json].
//!
//! Run: `cargo run -p clap-mcp-examples --bin structured -- --mcp`

use clap::Parser;
use clap_mcp::ClapMcp;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct AddResult {
    sum: i32,
    operands: Vec<i32>,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[command(
    name = "structured-example",
    about = "CLI with structured MCP output",
    subcommand_required = false
)]
enum Cli {
    /// Add numbers with structured JSON output.
    #[clap_mcp_output_json = "AddResult { sum: a + b, operands: vec![a, b] }"]
    Add {
        /// First operand.
        a: i32,
        /// Second operand.
        b: i32,
    },
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
