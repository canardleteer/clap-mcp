//! Unrelated `--mcp` user flag vs clap-mcp on `--modelcontextprotocol`.
//!
//! ```bash
//! # User legacy flag (does not start MCP server)
//! custom_mcp_flags --mcp
//!
//! # clap-mcp stdio server
//! custom_mcp_flags --modelcontextprotocol
//! ```

use clap::Parser;
use clap_mcp::{ClapMcp, ParseOrServeMcp};

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(
    reinvocation_safe = true,
    parallel_safe = false,
    mcp_flag = "modelcontextprotocol"
)]
#[clap_mcp_output_from = "run"]
#[command(name = "custom-mcp-flags")]
struct Cli {
    /// Legacy/unrelated MCP toggle (not clap-mcp Model Context Protocol).
    #[arg(long)]
    mcp: bool,
    #[arg(long)]
    message: Option<String>,
}

fn run(cli: Cli) -> String {
    format!(
        "legacy_mcp={} message={}",
        cli.mcp,
        cli.message.as_deref().unwrap_or("none")
    )
}

fn main() {
    let cli = Cli::parse_or_serve_mcp();
    println!("{}", run(cli));
}
