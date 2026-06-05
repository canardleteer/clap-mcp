use clap::Parser;
use clap_mcp::{ClapMcp, ClapMcpToolExecutor};

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp_output_from = "run"]
enum Cli {
    Edit {
        #[arg(long)]
        task_id: String,
        #[arg(long)]
        state: String,
    },
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::Edit { task_id, state } => format!("{task_id}:{state}"),
    }
}

fn main() {
    let out = Cli::Edit {
        task_id: "done".into(),
        state: "TASK-0".into(),
    }
    .execute_for_mcp()
    .expect("tool should run");
    assert!(matches!(out, clap_mcp::ClapMcpToolOutput::Text(_)));
}
