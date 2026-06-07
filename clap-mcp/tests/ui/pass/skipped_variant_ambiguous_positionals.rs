use clap::Parser;
use clap_mcp::{ClapMcp, ClapMcpToolExecutor};

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp_output_from = "run"]
enum Cli {
    List,
    #[clap_mcp(skip)]
    Upload {
        name: String,
        local_path: String,
    },
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::List => "list".into(),
        Cli::Upload {
            name,
            local_path,
        } => format!("{name}:{local_path}"),
    }
}

fn main() {
    let out = Cli::List.execute_for_mcp().expect("tool should run");
    assert!(matches!(out, clap_mcp::ClapMcpToolOutput::Text(_)));
}
