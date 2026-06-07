//! Flat struct root: one MCP tool with a wide inputSchema.
//!
//! When the derive root is a struct with no `#[command(subcommand)]`, clap-mcp
//! exposes a single tool whose schema includes every non-skipped root field and
//! flattened `Args` group. Prefer subcommands when you want smaller per-tool
//! schemas. See [Supported CLI shapes — Flat struct tradeoff](../../docs/supported-cli-shapes.md#flat-struct-tradeoff).
//!
//! Run:
//!   cargo run -p clap-mcp-examples --bin flat_struct_root -- --verbose --target prod --email a@b.c
//!   cargo run -p clap-mcp-examples --bin flat_struct_root -- --mcp

use clap::{Args, Parser};
use clap_mcp::{ClapMcp, ParseOrServeMcp};

#[derive(Debug, Args)]
struct ProfileArgs {
    #[arg(long)]
    email: Option<String>,
    #[arg(long)]
    region: Option<String>,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run"]
#[command(
    name = "flat-struct-root",
    about = "Flat struct root — single MCP tool, wide inputSchema"
)]
struct Cli {
    #[arg(long)]
    verbose: bool,
    #[command(flatten)]
    profile: ProfileArgs,
    #[arg(long)]
    target: String,
}

fn run(cli: Cli) -> String {
    let mut parts = vec![format!("target={}", cli.target)];
    if cli.verbose {
        parts.push("verbose".into());
    }
    if let Some(email) = &cli.profile.email {
        parts.push(format!("email={email}"));
    }
    if let Some(region) = &cli.profile.region {
        parts.push(format!("region={region}"));
    }
    parts.join(" ")
}

fn main() {
    let cli = Cli::parse_or_serve_mcp();
    println!("{}", run(cli));
}
