use clap::Parser;
use clap_mcp::{ClapMcp, ClapMcpToolError, ClapMcpToolOutput};
use serde::Serialize;

/// Simple example CLI using clap-mcp with the derive API.
///
/// Try:
/// - `cargo run -p clap-mcp-examples --bin subcommands -- --help`
/// - `cargo run -p clap-mcp-examples --bin subcommands -- --mcp`
///
/// This CLI is both parallel_safe and reinvocation_safe, but we configure the harder case
/// (parallel_safe=false, reinvocation_safe=false) to demonstrate subprocess-based execution.
#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(parallel_safe = false, reinvocation_safe = false)]
#[clap_mcp_output_from = "run"]
#[command(
    name = "clap-mcp-derive-example",
    about = "Example CLI that exposes its clap schema and subcommands over MCP stdio via --mcp",
    subcommand_required = false
)]
enum Cli {
    /// Greet someone (or the world) once.
    Greet {
        /// Optional name to greet.
        #[arg(long)]
        name: Option<String>,
    },
    /// Add two integers together.
    Add {
        /// First addend.
        a: i32,
        /// Second addend.
        b: i32,
    },
    /// Subtract the second integer from the first (returns structured output).
    Sub {
        /// Minuend.
        a: i32,
        /// Subtrahend.
        b: i32,
    },
}

#[derive(Debug, Serialize)]
struct SubResult {
    difference: i32,
    minuend: i32,
    subtrahend: i32,
}

/// Single output type for all variants; implements conversion for MCP.
#[derive(Debug)]
enum CliOutput {
    Text(String),
    Structured(SubResult),
}

impl clap_mcp::IntoClapMcpResult for CliOutput {
    fn into_tool_result(self) -> std::result::Result<ClapMcpToolOutput, ClapMcpToolError> {
        match self {
            CliOutput::Text(s) => Ok(ClapMcpToolOutput::Text(s)),
            CliOutput::Structured(s) => Ok(ClapMcpToolOutput::Structured(
                serde_json::to_value(&s).expect("SubResult must serialize"),
            )),
        }
    }
}

fn run(cmd: Cli) -> CliOutput {
    match cmd {
        Cli::Greet { name } => {
            let who = name.as_deref().unwrap_or("world");
            CliOutput::Text(format!("Hello, {who}!"))
        }
        Cli::Add { a, b } => CliOutput::Text(format!("{}", a + b)),
        Cli::Sub { a, b } => CliOutput::Structured(SubResult {
            difference: a - b,
            minuend: a,
            subtrahend: b,
        }),
    }
}

fn main() {
    // Uses config from #[clap_mcp(...)]: parallel_safe=false (serialize), reinvocation_safe=false (subprocess).
    let cli = clap_mcp::parse_or_serve_mcp_attr::<Cli>();

    match cli {
        Cli::Greet { name } => {
            let who = name.as_deref().unwrap_or("world");
            println!("Hello, {who}!");
        }
        Cli::Add { a, b } => {
            println!("{a} + {b} = {}", a + b);
        }
        Cli::Sub { a, b } => {
            println!("{a} - {b} = {}", a - b);
        }
    }
}
