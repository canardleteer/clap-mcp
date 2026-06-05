//! Elicitation demo: tool `confirm-echo` calls `peer.elicit` when `elicitation_enabled` is set.
//!
//! Run server: `cargo run -p clap-mcp-examples --bin elicitation_confirm --features elicitation -- --mcp`
//! Use a client that implements `ClientHandler::create_elicitation` (see integration test).

use clap::Parser;
use clap_mcp::{ClapMcp, ClapMcpConfigProvider, ClapMcpServeOptions};

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run"]
#[command(name = "elicitation-confirm-example", subcommand_required = false)]
enum Cli {
    /// Tool intercepted for elicitation demo (see clap-mcp server when elicitation_enabled).
    ConfirmEcho {
        #[arg(long)]
        message: Option<String>,
    },
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::ConfirmEcho { message } => message.unwrap_or_else(|| "hello".into()),
    }
}

fn main() {
    let serve_options = ClapMcpServeOptions {
        elicitation_enabled: true,
        ..Default::default()
    };
    let cli = clap_mcp::parse_or_serve_mcp_with_config_and_options::<Cli>(
        Cli::clap_mcp_config(),
        serve_options,
    );
    println!("{}", run(cli));
}
