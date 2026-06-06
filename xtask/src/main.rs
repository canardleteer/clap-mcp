mod code_coverage;
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
    /// Run tests with LLVM coverage and emit an HTML report (clap-mcp + macros only).
    CodeCoverageHtml(code_coverage::CodeCoverageHtmlArgs),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Conformance(args) => conformance::run_conformance(args),
        Command::ConformanceServer(args) => conformance::run_conformance_server(args),
        Command::CodeCoverageHtml(args) => code_coverage::run_code_coverage_html(args),
    }
}
