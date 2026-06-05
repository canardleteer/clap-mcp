//! Stateful derive without reinvocation_safe must not compile.

use clap::Parser;
use clap_mcp::ClapMcp;
use std::sync::Mutex;

#[derive(Default)]
struct State {}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe = false)]
#[clap_mcp_output_from_with_state = "run"]
#[clap_mcp_state_type = "Mutex<State>"]
enum Cli {
    Foo,
}

fn run(_cmd: Cli, _state: &Mutex<State>) -> &'static str {
    "ok"
}

fn main() {
    let _ = run(Cli::Foo, &Mutex::new(State::default()));
}
