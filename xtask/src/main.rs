mod code_coverage;
mod conformance;
mod examples_help;

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
    /// Build server, run pinned conformance harness in Docker (active@2025-11-25 + all@2026-07-28 by default).
    Conformance(conformance::ConformanceArgs),
    /// Stop a running conformance-server and remove pid/log/port artifacts.
    ConformanceStop(conformance::ConformanceStopArgs),
    /// Start clap-mcp-conformance-http for CI/debug (not for ad-hoc local harness runs).
    ConformanceServer(conformance::ConformanceServerArgs),
    /// Run tests with LLVM coverage and emit an HTML report (clap-mcp + macros only).
    CodeCoverageHtml(code_coverage::CodeCoverageHtmlArgs),
    /// List or smoke-test example binaries (`--help` after build).
    ExamplesHelp(examples_help::ExamplesHelpArgs),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Conformance(args) => conformance::run_conformance(args),
        Command::ConformanceStop(args) => conformance::run_conformance_stop(args),
        Command::ConformanceServer(args) => conformance::run_conformance_server(args),
        Command::CodeCoverageHtml(args) => code_coverage::run_code_coverage_html(args),
        Command::ExamplesHelp(args) => examples_help::run_examples_help(args),
    }
}
