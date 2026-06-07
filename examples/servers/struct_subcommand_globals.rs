//! Struct root with global flags and `#[clap_mcp_output_from]` on the root type.
//!
//! Tool execution receives the full parsed `Cli` (globals + subcommand), not only
//! the subcommand enum. See **struct_subcommand_globals** in examples/README.md.

use clap::{Parser, Subcommand};
use clap_mcp::{ClapMcp, ParseOrServeMcp};

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run_cli"]
#[command(name = "struct-subcommand-globals", subcommand_required = true)]
struct Cli {
    #[arg(long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp(schema_only)]
enum Commands {
    Greet {
        #[arg(long)]
        name: Option<String>,
    },
}

fn run_cli(cli: Cli) -> String {
    let who = match &cli.command {
        Commands::Greet { name } => name.as_deref().unwrap_or("world"),
    };
    if cli.verbose {
        format!("verbose: Hello, {who}!")
    } else {
        format!("Hello, {who}!")
    }
}

fn main() {
    let cli = Cli::parse_or_serve_mcp();
    println!("{}", run_cli(cli));
}
