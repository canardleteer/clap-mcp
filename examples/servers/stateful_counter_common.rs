use clap::{Parser, Subcommand};
use clap_mcp::ClapMcp;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct CounterState {
    pub count: u64,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe = true)]
#[clap_mcp_state_type = "Mutex<CounterState>"]
#[command(name = "stateful-counter", subcommand_required = true)]
pub struct App {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp(reinvocation_safe = true)]
#[clap_mcp_output_from_with_state = "run"]
#[clap_mcp_state_type = "Mutex<CounterState>"]
pub enum Command {
    /// Increment the shared counter and return the new value.
    Increment,
    /// Read the current counter value without changing it.
    Read,
}

pub fn run(cmd: Command, state: &Arc<Mutex<CounterState>>) -> String {
    let mut guard = state.lock().expect("counter mutex");
    match cmd {
        Command::Increment => {
            guard.count += 1;
            format!("count={}", guard.count)
        }
        Command::Read => format!("count={}", guard.count),
    }
}
