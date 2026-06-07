//! Example: clap ArgGroup hints in MCP tool metadata and descriptions.
//!
//! Demonstrates mutually exclusive flags exposed as advisory hints (not JSON Schema
//! `oneOf`). Agents should read `meta.clapMcp.argGroups` on `list_tools`; invalid combinations
//! still fail at clap parse time.
//!
//! Run:
//!   cargo run -p clap-mcp-examples --bin arg_group_hints -- --help
//!   cargo run -p clap-mcp-examples --bin arg_group_hints -- --mcp
//!   cargo run -p clap-mcp-examples --bin arg_group_hints -- search --pattern '*.rs' --exec 'echo hi'

use clap::{Args, Parser};
use clap_mcp::{ClapMcp, ParseOrServeMcp};

#[derive(Debug, Args)]
#[group(id = "execs", required = true, multiple = false)]
struct ExecMode {
    /// Run a single command on each match.
    #[arg(long)]
    exec: Option<String>,

    /// Run a batch command on matches.
    #[arg(long)]
    exec_batch: Option<String>,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = true)]
#[clap_mcp_output_from = "run"]
#[command(
    name = "arg-group-hints-example",
    about = "Example: ArgGroup hints in MCP meta"
)]
enum Cli {
    /// Search with exactly one execution mode from the execs group.
    Search {
        #[command(flatten)]
        mode: ExecMode,
        /// Glob pattern to search.
        #[arg(long)]
        pattern: String,
    },
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::Search { mode, pattern } => {
            let mode = mode
                .exec
                .map(|c| format!("exec={c}"))
                .or_else(|| mode.exec_batch.map(|c| format!("exec_batch={c}")))
                .unwrap_or_else(|| "no-exec".to_string());
            format!("pattern={pattern}, {mode}")
        }
    }
}

fn main() {
    let cli = Cli::parse_or_serve_mcp();
    println!("{}", run(cli));
}
