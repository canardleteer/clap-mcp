//! Example CLI demonstrating topical serialization with `parallel_safe = true`.
//!
//! Most tools run concurrently; `flush` subcommands opt into per-tool or per-output locks.
//! The `flush` command uses `#[clap_mcp(serialize_topic)]` so lock keys follow parsed `String`
//! equality (via [`ClapMcpSerializeTopic`]) rather than raw JSON text alone.

use clap::Parser;
use clap_mcp::{ClapMcp, ParseOrServeMcp};

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = true)]
#[clap_mcp_output_from = "run"]
#[command(
    name = "topical-serialization",
    about = "Topical serialization demo (parallel_safe + serialized subcommands)",
    subcommand_required = false
)]
enum Cli {
    /// Read-only; safe to overlap under parallel_safe.
    Search {
        #[arg(long)]
        query: String,
    },
    /// All flush invocations share one lock topic.
    #[clap_mcp(serialized)]
    FlushAll,

    /// Only concurrent flushes to the same output path serialize together.
    #[clap_mcp(serialized = "output")]
    Flush {
        #[clap_mcp(serialize_topic)]
        #[arg(long)]
        output: String,
    },
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::Search { query } => format!("search: {query}"),
        Cli::FlushAll => "flushed all".into(),
        Cli::Flush { output } => format!("flushed to {output}"),
    }
}

fn main() {
    let cli = Cli::parse_or_serve_mcp();
    println!("{}", run(cli));
}
