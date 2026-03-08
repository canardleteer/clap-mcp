#![allow(unused_assignments, unused_variables)]

use clap::{Parser, Subcommand};
use clap_mcp::ClapMcp;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe = false, parallel_safe = false)]
#[command(name = "nested-subcommands", subcommand_required = true)]
struct Cli {
    #[command(subcommand)]
    command: TopLevel,
}

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp_output_from = "run_top_level"]
enum TopLevel {
    Parent {
        #[command(subcommand)]
        command: ParentCommand,
    },
}

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp_output_from = "run_parent"]
enum ParentCommand {
    Child {
        #[arg(long)]
        value: String,
    },
}

fn run_top_level(cmd: TopLevel) -> String {
    match cmd {
        TopLevel::Parent { command } => run_parent(command),
    }
}

fn run_parent(cmd: ParentCommand) -> String {
    match cmd {
        ParentCommand::Child { value } => format!("child={value}"),
    }
}

fn main() {
    let cli = clap_mcp::parse_or_serve_mcp_attr::<Cli>();
    println!("{}", run_top_level(cli.command));
}
