mod subcommands_common;

use subcommands_common::{Cli, run_cli_interactive};

/// Stdio MCP example (`--mcp`).
///
/// Try:
/// - `cargo run -p clap-mcp-examples --bin subcommands -- --help`
/// - `cargo run -p clap-mcp-examples --bin subcommands -- --mcp`
fn main() {
    let cli = clap_mcp::parse_or_serve_mcp_attr::<Cli>();
    run_cli_interactive(cli);
}
