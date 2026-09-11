//! Example CLI with structured output via `#[clap_mcp_output_from]` and `AsStructured`.
//!
//! Per-tool `output_type` advertises `outputSchema` for `add` (requires the
//! `output-schema` feature). `echo` stays text-only with no schema.
//!
//! Run: `cargo run -p clap-mcp-examples --bin structured -- --mcp`

use clap::Parser;
use clap_mcp::{AsStructured, ClapMcp, ClapMcpToolOutput, IntoClapMcpResult, ParseOrServeMcp};
use serde::Serialize;

#[derive(Debug, Serialize, schemars::JsonSchema)]
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
    #[clap_mcp(output_type = "AddResult")]
    Add {
        /// First operand.
        #[arg(long)]
        a: i32,
        /// Second operand.
        #[arg(long)]
        b: i32,
    },
    /// Plain text echo (no `outputSchema`).
    Echo {
        #[arg(long)]
        message: String,
    },
}

fn run(cmd: Cli) -> ClapMcpToolOutput {
    match cmd {
        Cli::Add { a, b } => AsStructured(AddResult {
            sum: a + b,
            operands: vec![a, b],
        })
        .into_tool_result()
        .expect("AddResult should serialize"),
        Cli::Echo { message } => ClapMcpToolOutput::Text(message),
    }
}

fn main() {
    let cli = Cli::parse_or_serve_mcp();
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
        Cli::Echo { message } => println!("{message}"),
    }
}
