#![allow(unused_assignments, unused_variables)]

use clap::{Parser, Subcommand};
use clap_mcp::{ClapMcp, ClapMcpToolExecutor};

#[derive(Debug, Parser, ClapMcp)]
#[command(name = "nested-subcommands-pass", subcommand_required = true)]
struct Cli {
    #[command(subcommand)]
    command: TopLevel,
}

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp_output_from = "run_top_level"]
enum TopLevel {
    Parent {
        #[command(subcommand)]
        command: ChildCommand,
    },
}

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp_output_from = "run_child"]
enum ChildCommand {
    Leaf {
        #[arg(long)]
        value: String,
    },
}

fn run_top_level(cmd: TopLevel) -> String {
    match cmd {
        TopLevel::Parent { command } => run_child(command),
    }
}

fn run_child(cmd: ChildCommand) -> String {
    match cmd {
        ChildCommand::Leaf { value } => format!("leaf={value}"),
    }
}

fn main() {
    let cli = Cli {
        command: TopLevel::Parent {
            command: ChildCommand::Leaf {
                value: "ok".to_string(),
            },
        },
    };
    let result = cli
        .execute_for_mcp()
        .expect("root struct should execute for MCP");
    assert!(matches!(result, clap_mcp::ClapMcpToolOutput::Text(_)));
}
