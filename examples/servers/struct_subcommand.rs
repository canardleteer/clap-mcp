//! Example CLI with struct root and `#[command(subcommand)]`.
//! Demonstrates the **Struct root with subcommand** pattern: derive `ClapMcp` on both
//! root and subcommand; put `#[clap_mcp_output_from = "run"]` on the subcommand enum.
//! See the crate README section "Struct root with subcommand" / "Dual derive (root + subcommand)".

use clap::{Parser, Subcommand};
use clap_mcp::{ClapMcp, ClapMcpToolError, ClapMcpToolOutput};
use serde::Serialize;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[command(
    name = "struct-subcommand-example",
    about = "Struct root with subcommand, optional subcommand support",
    subcommand_required = false
)]
struct Cli {
    #[arg(long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp_output_from = "run"]
enum Commands {
    Greet {
        #[arg(long)]
        name: Option<String>,
    },
    Add {
        #[arg(long)]
        a: i32,
        #[arg(long)]
        b: i32,
    },
    Sub {
        #[arg(long)]
        a: i32,
        #[arg(long)]
        b: i32,
    },
}

#[derive(Debug, Serialize)]
struct SubResult {
    difference: i32,
    minuend: i32,
    subtrahend: i32,
}

#[derive(Debug)]
enum CommandsOutput {
    Text(String),
    Structured(SubResult),
}

impl clap_mcp::IntoClapMcpResult for CommandsOutput {
    fn into_tool_result(self) -> std::result::Result<ClapMcpToolOutput, ClapMcpToolError> {
        match self {
            CommandsOutput::Text(s) => Ok(ClapMcpToolOutput::Text(s)),
            CommandsOutput::Structured(s) => Ok(ClapMcpToolOutput::Structured(
                serde_json::to_value(&s).expect("SubResult must serialize"),
            )),
        }
    }
}

fn run(cmd: Commands) -> CommandsOutput {
    match cmd {
        Commands::Greet { name } => {
            let who = name.as_deref().unwrap_or("world");
            CommandsOutput::Text(format!("Hello, {who}!"))
        }
        Commands::Add { a, b } => CommandsOutput::Text(format!("{}", a + b)),
        Commands::Sub { a, b } => CommandsOutput::Structured(SubResult {
            difference: a - b,
            minuend: a,
            subtrahend: b,
        }),
    }
}

fn main() {
    let cli = clap_mcp::parse_or_serve_mcp_attr::<Cli>();

    match cli.command {
        None => println!("No subcommand (try greet, add, or sub)"),
        Some(Commands::Greet { name }) => {
            println!("Hello, {}!", name.as_deref().unwrap_or("world"));
        }
        Some(Commands::Add { a, b }) => println!("{a} + {b} = {}", a + b),
        Some(Commands::Sub { a, b }) => println!("{a} - {b} = {}", a - b),
    }
}
