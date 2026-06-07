//! Preserve native clap parse errors for invalid human CLI argv.
//!
//! `parse_or_serve_mcp_preserve_cli` runs `Cli::parse()` when argv has no clap-mcp
//! entry flag, so missing required flags produce clap's usual Usage output. MCP
//! entry (`--mcp`) still uses the augmented path.
//!
//! Run:
//!   cargo run -p clap-mcp-examples --bin preserve_cli_parse -- greet
//!   cargo run -p clap-mcp-examples --bin preserve_cli_parse -- greet --name Ada
//!   cargo run -p clap-mcp-examples --bin preserve_cli_parse -- greet   # clap Usage error
//!   cargo run -p clap-mcp-examples --bin preserve_cli_parse -- --mcp

use clap::Parser;
use clap_mcp::{ClapMcp, ParseOrServeMcp};

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run"]
#[command(
    name = "preserve-cli-parse",
    about = "Preserve native clap parse errors (see docs/usage.md#preserve-cli-parse)"
)]
enum Cli {
    Greet {
        #[arg(long)]
        name: String,
    },
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::Greet { name } => format!("Hello, {name}!"),
    }
}

fn main() {
    let cli = Cli::parse_or_serve_mcp_preserve_cli();
    println!("{}", run(cli));
}
