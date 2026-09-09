#![allow(unused_assignments, unused_variables)]

use clap::{Parser, Subcommand};
use clap_mcp::ClapMcp;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(skip_root_when_subcommands, leaves_only)]
#[clap_mcp_output_from = "run"]
#[command(name = "leaves-only-pass", subcommand_required = true)]
struct Cli {
    #[command(subcommand)]
    command: Top,
}

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp(schema_only)]
enum Top {
    Parent {
        #[command(subcommand)]
        command: Leaf,
    },
}

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp(schema_only)]
enum Leaf {
    Child {
        #[arg(long)]
        value: String,
    },
}

fn run(cli: Cli) -> String {
    match cli.command {
        Top::Parent { command } => match command {
            Leaf::Child { value } => value,
        },
    }
}

fn main() {
    let _ = Cli::clap_mcp_schema_metadata();
    let _ = run;
}
