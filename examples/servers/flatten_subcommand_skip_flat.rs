//! Skip a flattened `Subcommand` enum (flat hidden tools).
//!
//! `#[clap_mcp(skip)]` on `#[command(subcommand)]` probes `Subcommand::augment_subcommands`
//! and adds every subcommand name to `skip_commands`. Shell users can still run
//! `hidden-a` / `hidden-b`; MCP exposes only the root `visible` flag on one wide tool.
//!
//! See also **flatten_subcommand_skip_nested** for recursive skip (`build` / `compile` / …).
//!
//! Run:
//!   cargo run -p clap-mcp-examples --bin flatten_subcommand_skip_flat -- hidden-a
//!   cargo run -p clap-mcp-examples --bin flatten_subcommand_skip_flat -- --visible ops
//!   cargo run -p clap-mcp-examples --bin flatten_subcommand_skip_flat -- --mcp

mod flatten_subcommand_skip_common;

use clap_mcp::ParseOrServeMcp;
use flatten_subcommand_skip_common::flat::{FlatCli, run_flat};

fn main() {
    let cli = FlatCli::parse_or_serve_mcp();
    println!("{}", run_flat(cli));
}
