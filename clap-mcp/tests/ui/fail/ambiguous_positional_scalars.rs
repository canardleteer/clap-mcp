//! Two bare positional scalar fields must not compile for MCP derive targets.

use clap::Parser;
use clap_mcp::ClapMcp;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp_output_from = "run"]
enum Cli {
    Edit {
        task_id: String,
        state: String,
    },
}

fn run(_cmd: Cli) -> &'static str {
    "ok"
}

fn main() {
    let _ = run(Cli::Edit {
        task_id: "a".into(),
        state: "b".into(),
    });
}
