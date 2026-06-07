//! Flatten skip, hidden subcommands, and nested serialize_topic in shared Args.
//!
//! Demonstrates:
//! - `#[clap_mcp(skip)]` on flattened `Args` (connection flags CLI-only for MCP)
//! - `#[clap_mcp(skip)]` on subcommand variants (maintenance tools hidden)
//! - `#[clap_mcp(args_metadata)]` + `#[clap_mcp(serialize_topic)]` inside a helper
//! - `#[arg(id = "...")]` aligned with derive metadata (`custom-out`)
//!
//! Skipping a whole flattened `Subcommand` field (`#[command(subcommand)]` +
//! `#[clap_mcp(skip)]`) is demonstrated in **flatten_subcommand_skip_flat** and
//! **flatten_subcommand_skip_nested** (see [examples/README.md](../README.md)).
//!
//! Run:
//!   cargo run -p clap-mcp-examples --bin flatten_skip -- show --id abc
//!   cargo run -p clap-mcp-examples --bin flatten_skip -- flush --custom-out /tmp/out
//!   cargo run -p clap-mcp-examples --bin flatten_skip -- --mcp

use clap::{Args, Parser, Subcommand};
use clap_mcp::{ClapMcp, ParseOrServeMcp};

#[derive(Debug, Args)]
struct ConnectionArgs {
    #[arg(long)]
    host: Option<String>,
    #[arg(long)]
    port: Option<u16>,
}

#[derive(Debug, Args, ClapMcp)]
#[clap_mcp(args_metadata)]
struct FlushArgs {
    #[clap_mcp(serialize_topic)]
    #[arg(id = "custom-out", long = "custom-out")]
    output: Option<String>,
}

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp(schema_only)]
enum Commands {
    Show {
        #[arg(long)]
        id: String,
    },
    #[clap_mcp(serialized = "custom-out")]
    Flush {
        #[command(flatten)]
        args: FlushArgs,
    },
    #[clap_mcp(skip)]
    Reindex,
    #[clap_mcp(skip)]
    Repair,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = true)]
#[clap_mcp_output_from = "run"]
#[command(
    name = "flatten-skip",
    about = "Flatten skip, hidden subcommands, args_metadata serialize_topic",
    subcommand_required = true
)]
struct Cli {
    #[command(flatten)]
    #[clap_mcp(skip)]
    connection: ConnectionArgs,
    #[command(subcommand)]
    command: Commands,
}

fn run(cli: Cli) -> String {
    match cli.command {
        Commands::Show { id } => format!("show:{id}"),
        Commands::Flush { args } => {
            format!("flush:{}", args.output.as_deref().unwrap_or("default"))
        }
        Commands::Reindex => "reindex".into(),
        Commands::Repair => "repair".into(),
    }
}

fn main() {
    let cli = Cli::parse_or_serve_mcp();
    println!("{}", run(cli));
}
