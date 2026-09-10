//! Server instructions, `Implementation` identity, and per-tool annotations.
//!
//! Uses `parse_or_serve_mcp_with` + [`ClapMcpServeOptions`] so `--mcp` works for
//! contract tests. The same knobs exist on
//! [`ServeMcpBuilder`](clap_mcp::ServeMcpBuilder) (see [Usage](../../docs/usage.md)).
//!
//! Run:
//!   cargo run -p clap-mcp-examples --bin server_metadata -- status
//!   cargo run -p clap-mcp-examples --bin server_metadata -- --mcp

use clap::Parser;
use clap_mcp::{ClapMcp, ClapMcpConfigProvider, ClapMcpServeOptions, Implementation};

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = true)]
#[clap_mcp_output_from = "run"]
#[command(
    name = "server-metadata",
    about = "Server instructions, identity, and tool annotations",
    subcommand_required = true
)]
enum Cli {
    /// Read-only health check.
    #[clap_mcp(read_only, idempotent, tool_title = "Service Status")]
    Status,

    /// Destructive reset (demo annotations only).
    #[clap_mcp(destructive, open_world, tool_title = "Reset Service State")]
    Reset {
        #[arg(long)]
        force: bool,
    },
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::Status => "healthy".to_string(),
        Cli::Reset { force } => format!("reset (force={force})"),
    }
}

fn main() {
    let serve = ClapMcpServeOptions::default()
        .with_instructions("Prefer the status tool before calling reset.")
        .with_server_info(
            Implementation::new("server-metadata-example", env!("CARGO_PKG_VERSION"))
                .with_title("Server Metadata Example")
                .with_description("Demonstrates MCP initialize identity and tool annotations"),
        );

    let cli = clap_mcp::parse_or_serve_mcp_with::<Cli>(clap_mcp::ClapMcpRunOptions {
        config: Cli::clap_mcp_config(),
        serve,
    });

    println!("{}", run(cli));
}
