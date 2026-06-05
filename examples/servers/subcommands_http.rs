mod subcommands_common;

use clap_mcp::ParseOrServeMcp;
use subcommands_common::{Cli, run_cli_interactive};

/// Streamable HTTP MCP example (`--mcp-http`, requires `http` feature).
///
/// Try:
/// - `cargo run -p clap-mcp-examples --bin subcommands_http --features http -- --mcp-http 127.0.0.1:8080`
fn main() {
    let cli = Cli::parse_or_serve_mcp();
    run_cli_interactive(cli);
}
