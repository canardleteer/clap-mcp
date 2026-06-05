#![allow(unused_variables)]

use clap::{Parser, Subcommand};
use clap_mcp::{ClapMcp, ClapMcpToolExecutorWithState};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct State {
    hits: u32,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe = true)]
#[clap_mcp_output_from_with_state = "run"]
#[clap_mcp_state_type = "Mutex<State>"]
#[command(name = "output-from-with-state-pass", subcommand_required = true)]
enum Cli {
    Bump,
}

fn run(cmd: Cli, state: &Arc<Mutex<State>>) -> String {
    let _ = cmd;
    let mut guard = state.lock().expect("state mutex");
    guard.hits += 1;
    format!("hits={}", guard.hits)
}

fn main() {
    let state = Arc::new(Mutex::new(State::default()));
    let out = Cli::Bump
        .execute_for_mcp_with_state(&state)
        .expect("stateful tool should run");
    assert!(matches!(out, clap_mcp::ClapMcpToolOutput::Text(_)));
    assert_eq!(state.lock().expect("state mutex").hits, 1);
}
