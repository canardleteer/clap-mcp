//! Skip a flattened `Subcommand` enum (nested hidden tools).
//!
//! Recursive skip: `build`, `compile`, `link`, and `clean` are excluded from MCP tools
//! when the whole flattened subcommand tree is marked `#[clap_mcp(skip)]`.
//!
//! See also **flatten_subcommand_skip_flat** for a root flag + flat hidden subcommands.
//!
//! Run:
//!   cargo run -p clap-mcp-examples --bin flatten_subcommand_skip_nested -- build compile
//!   cargo run -p clap-mcp-examples --bin flatten_subcommand_skip_nested -- --mcp

mod flatten_subcommand_skip_common;

use clap_mcp::ParseOrServeMcp;
use flatten_subcommand_skip_common::nested::{NestedCli, run_nested};

fn main() {
    let cli = NestedCli::parse_or_serve_mcp();
    println!("{}", run_nested(cli));
}
