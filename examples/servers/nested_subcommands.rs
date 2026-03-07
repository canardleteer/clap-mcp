#![allow(unused_assignments, unused_variables)]

use clap::{Parser, Subcommand};
use clap_mcp::ClapMcp;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe = false, parallel_safe = false)]
#[clap_mcp_output_from = "run"]
#[command(name = "nested-subcommands", subcommand_required = true)]
struct Cli {
    #[command(subcommand)]
    command: TopLevel,
}

#[derive(Debug, Subcommand, ClapMcp)]
enum TopLevel {
    Parent {
        #[command(subcommand)]
        command: ParentCommand,
    },
}

#[derive(Debug, Subcommand, ClapMcp)]
enum ParentCommand {
    Child {
        #[arg(long)]
        value: String,
    },
}

fn run(cli: Cli) -> String {
    match cli.command {
        TopLevel::Parent { command } => match command {
            ParentCommand::Child { value } => format!("child={value}"),
        },
    }
}

fn main() {
    let cli = clap_mcp::parse_or_serve_mcp_attr::<Cli>();
    println!("{}", run(cli));
}
