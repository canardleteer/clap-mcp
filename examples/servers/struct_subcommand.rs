//! Example CLI with struct root and #[command(subcommand)].
//! Shows ClapMcpConfigProvider on the struct, delegation to subcommand,
//! and optional subcommand support.

use clap::{Parser, Subcommand};
use clap_mcp::ClapMcp;
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
enum Commands {
    #[clap_mcp_output = "format!(\"Hello, {}!\", clap_mcp::opt_str(&name, \"world\"))"]
    Greet {
        #[arg(long)]
        name: Option<String>,
    },
    #[clap_mcp_output = "format!(\"{}\", a + b)"]
    Add {
        #[arg(long)]
        a: i32,
        #[arg(long)]
        b: i32,
    },
    #[clap_mcp_output_json = "SubResult { difference: a - b, minuend: a, subtrahend: b }"]
    Sub {
        #[arg(long)]
        a: i32,
        #[arg(long)]
        b: i32,
    },
}

#[derive(Serialize)]
struct SubResult {
    difference: i32,
    minuend: i32,
    subtrahend: i32,
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
