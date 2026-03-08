//! Compile-fail: enum without #[clap_mcp_output_from] must produce a clear error.

use clap::Parser;
use clap_mcp::ClapMcp;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[command(name = "missing-output-from")]
enum Cli {
    Foo,
}

fn main() {}
