//! Compiles when #[clap_mcp(skip)] on a flattened Subcommand enum hides subcommand tools.

use clap::{CommandFactory, Parser, Subcommand};
use clap_mcp::ClapMcp;

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp(schema_only)]
enum HiddenCommands {
    #[command(name = "hidden")]
    Hidden,
    Kept,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run_skip_flatten_subcommands"]
#[command(name = "skip-flatten-subcommands")]
struct Cli {
    #[command(subcommand)]
    #[clap_mcp(skip)]
    commands: HiddenCommands,
}

fn run_skip_flatten_subcommands(cmd: Cli) -> String {
    format!("{cmd:?}")
}

fn main() {
    let _ = Cli::command();
}
