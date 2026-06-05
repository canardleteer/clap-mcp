//! task_augmented_tools without reinvocation_safe must not compile.

use clap::Parser;
use clap_mcp::ClapMcp;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe = false, task_augmented_tools)]
#[clap_mcp_output_from = "run"]
enum Cli {
    Foo,
}

fn run(_cmd: Cli) -> &'static str {
    "ok"
}

fn main() {
    let _ = run(Cli::Foo);
}
