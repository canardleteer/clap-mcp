use clap::Parser;
use clap_mcp::ClapMcp;

#[derive(Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe = false, parallel_safe = false)]
#[clap_mcp(leaves_only = "yes")]
#[clap_mcp_output_from = "run"]
#[command(name = "test-leaves-only-non-bool")]
enum Cli {
    Ping,
}

fn run(_cmd: Cli) -> String {
    "ok".into()
}

fn main() {}
