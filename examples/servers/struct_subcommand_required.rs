//! Struct root with a **required** subcommand — typical migration path (e.g. sem-tool).
//!
//! Normal CLI behavior is unchanged from pre-clap-mcp clap: bare invocation errors,
//! subcommands work as before. Only `myapp --mcp` adds MCP server mode.
//!
//! See **struct_subcommand_required** in examples/README.md and README "CLI compatibility".

use clap::{Parser, Subcommand};
use clap_mcp::{ClapMcp, ClapMcpToolError, ClapMcpToolOutput, ParseOrServeMcp};
use serde::Serialize;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe)]
#[command(
    name = "struct-subcommand-required-example",
    about = "Struct root with required subcommand (zero CLI regression + --mcp)",
    subcommand_required = true
)]
struct Cli {
    #[arg(long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
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
    let cli = Cli::parse_or_serve_mcp();
    match cli.command {
        Commands::Greet { name } => {
            println!("Hello, {}!", name.as_deref().unwrap_or("world"));
        }
        Commands::Add { a, b } => println!("{a} + {b} = {}", a + b),
        Commands::Sub { a, b } => println!("{a} - {b} = {}", a - b),
    }
}
