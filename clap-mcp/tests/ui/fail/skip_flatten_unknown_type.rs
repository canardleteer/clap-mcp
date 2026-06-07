//! External flattened types cannot be classified for #[clap_mcp(skip)].

use clap::{CommandFactory, Parser};
use clap_mcp::ClapMcp;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run_skip_flatten_unknown"]
#[command(name = "skip-flatten-unknown")]
struct Cli {
    #[command(flatten)]
    #[clap_mcp(skip)]
    opaque: std::ffi::OsString,
    #[arg(long)]
    kept: Option<String>,
}

fn run_skip_flatten_unknown(cmd: Cli) -> String {
    format!("{cmd:?}")
}

fn main() {
    let _ = Cli::command();
}
