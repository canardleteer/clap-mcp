mod subcommands_common;

use clap_mcp::ParseOrServeMcp;
use subcommands_common::{Cli, run_cli_interactive};

/// Stdio MCP example (`--mcp`).
///
/// Try:
/// - `cargo run -p clap-mcp-examples --bin subcommands -- --help`
/// - `cargo run -p clap-mcp-examples --bin subcommands -- --mcp`
fn main() {
    let cli = Cli::parse_or_serve_mcp();
    run_cli_interactive(cli);
}
