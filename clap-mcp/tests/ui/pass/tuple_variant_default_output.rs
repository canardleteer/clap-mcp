use clap::Parser;
use clap_mcp::{ClapMcp, ClapMcpToolExecutor};

#[derive(Debug, Parser, ClapMcp)]
#[command(name = "struct-default-output")]
struct Cli {
    #[arg(long)]
    value: i32,
}

fn main() {
    let pair = Cli { value: 2 }
        .execute_for_mcp()
        .expect("plain struct should execute for MCP");
    assert!(matches!(pair, clap_mcp::ClapMcpToolOutput::Text(_)));
}
