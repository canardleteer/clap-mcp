mod conformance;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask", about = "clap-mcp workspace tasks")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Build server, run pinned conformance harness in Docker (local dev).
    Conformance(conformance::ConformanceArgs),
    /// Start subcommands_http for CI; writes CONFORMANCE_PORT to GITHUB_ENV when set.
    ConformanceServer(conformance::ConformanceServerArgs),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Conformance(args) => conformance::run_conformance(args),
        Command::ConformanceServer(args) => conformance::run_conformance_server(args),
    }
}
