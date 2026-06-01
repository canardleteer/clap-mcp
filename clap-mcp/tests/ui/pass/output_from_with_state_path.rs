use clap::Parser;
use clap_mcp::{ClapMcp, ClapMcpToolExecutorWithState};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
struct State {
    counter: usize,
}

mod handlers {
    use super::{Cli, State};
    use std::sync::{Arc, Mutex};

    pub fn run(cli: Cli, state: &Arc<Mutex<State>>) -> String {
        let mut state = state.lock().expect("state lock should succeed");
        state.counter += 1;
        match cli {
            Cli::Double { value } => format!("{}:{}", value * 2, state.counter),
        }
    }
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from_with_state = "handlers::run"]
#[clap_mcp_state_type = "Mutex<State>"]
#[command(name = "output-from-state-path", subcommand_required = true)]
enum Cli {
    Double {
        #[arg(long)]
        value: i32,
    },
}

fn main() {
    let state = Arc::new(Mutex::new(State { counter: 0 }));
    let first = Cli::Double { value: 7 }
        .execute_for_mcp_with_state(&state)
        .expect("first output_from_with_state call should execute");
    let second = Cli::Double { value: 7 }
        .execute_for_mcp_with_state(&state)
        .expect("second output_from_with_state call should execute");
    assert_eq!(first.into_string(), "14:1");
    assert_eq!(second.into_string(), "14:2");
}
