//! Compiles when #[clap_mcp(skip = "id1,id2")] lists explicit clap arg ids.

use clap::{CommandFactory, Parser};
use clap_mcp::ClapMcp;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run_skip_list"]
#[command(name = "skip-arg-list")]
struct Cli {
    #[clap_mcp(skip = "left,right")]
    #[arg(long)]
    left: Option<String>,
    #[arg(long)]
    right: Option<String>,
    #[arg(long)]
    kept: Option<String>,
}

fn run_skip_list(cmd: Cli) -> String {
    format!("{:?}", cmd.kept)
}

fn main() {
    let _ = Cli::command();
}
