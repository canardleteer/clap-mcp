mod passthrough_common;

use clap::Parser;
use clap_mcp::{ClapMcp, ParseOrServeMcp};
use passthrough_common::{Command, run_interactive};

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe = false, parallel_safe = false)]
#[command(name = "passthrough-args-subprocess", subcommand_required = true)]
struct App {
    #[command(subcommand)]
    command: Command,
}

fn main() {
    let app = App::parse_or_serve_mcp();
    run_interactive(app.command);
}
