//! Flat struct root: one MCP tool with a wide inputSchema.
//!
//! When the derive root is a struct with no `#[command(subcommand)]`, clap-mcp
//! exposes a single tool whose schema includes every non-skipped root field and
//! flattened `Args` group. Prefer subcommands when you want smaller per-tool
//! schemas. See [Supported CLI shapes — Flat struct tradeoff](../../docs/supported-cli-shapes.md#flat-struct-tradeoff).
//!
//! Also demos `hide_default` (derive) and `override_default` (serve options).
//!
//! Run:
//!   cargo run -p clap-mcp-examples --bin flat_struct_root -- --verbose --target prod --email a@b.c
//!   cargo run -p clap-mcp-examples --bin flat_struct_root -- --mcp

use clap::{Args, Parser};
use clap_mcp::{ClapMcp, ClapMcpConfigProvider, ClapMcpServeOptions};

#[derive(Debug, Args)]
struct ProfileArgs {
    #[arg(long)]
    email: Option<String>,
    #[arg(long)]
    region: Option<String>,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false, hide_default = "config_dir")]
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
    /// Host path default is hidden from MCP `inputSchema`.
    #[arg(long, default_value = "/var/lib/flat-struct")]
    config_dir: String,
    /// Native CLI default is `dev`; MCP advertises `staging` via `override_default`.
    #[arg(long, default_value = "dev")]
    env: String,
}

fn run(cli: Cli) -> String {
    let mut parts = vec![
        format!("target={}", cli.target),
        format!("config_dir={}", cli.config_dir),
        format!("env={}", cli.env),
    ];
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
    let serve = ClapMcpServeOptions::default().with_override_default(
        "*",
        "env",
        serde_json::json!("staging"),
    );

    let cli = clap_mcp::parse_or_serve_mcp_with::<Cli>(clap_mcp::ClapMcpRunOptions {
        config: Cli::clap_mcp_config(),
        serve,
    });
    println!("{}", run(cli));
}
