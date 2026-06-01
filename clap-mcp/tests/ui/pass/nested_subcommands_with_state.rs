use clap::{Parser, Subcommand};
use clap_mcp::{ClapMcp, ClapMcpToolExecutorWithState};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
struct State {
    counter: usize,
}

#[derive(Debug, Parser, ClapMcp)]
#[command(name = "nested-subcommands-state-pass", subcommand_required = true)]
struct Cli {
    #[command(subcommand)]
    command: TopLevel,
}

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp_output_from_with_state = "run_top_level"]
#[clap_mcp_state_type = "Mutex<State>"]
enum TopLevel {
    Parent {
        #[command(subcommand)]
        command: ChildCommand,
    },
}

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp_output_from_with_state = "run_child"]
#[clap_mcp_state_type = "Mutex<State>"]
enum ChildCommand {
    Leaf {
        #[arg(long)]
        value: String,
    },
}

fn run_top_level(cmd: TopLevel, state: &Arc<Mutex<State>>) -> String {
    match cmd {
        TopLevel::Parent { command } => run_child(command, state),
    }
}

fn run_child(cmd: ChildCommand, state: &Arc<Mutex<State>>) -> String {
    let mut state = state.lock().expect("state lock should succeed");
    state.counter += 1;
    match cmd {
        ChildCommand::Leaf { value } => format!("leaf={value}:{}", state.counter),
    }
}

fn main() {
    let cli = Cli {
        command: TopLevel::Parent {
            command: ChildCommand::Leaf {
                value: "ok".to_string(),
            },
        },
    };
    let state = Arc::new(Mutex::new(State { counter: 0 }));
    let first = cli
        .execute_for_mcp_with_state(&state)
        .expect("first root struct call should execute for MCP with state");
    let second = Cli {
        command: TopLevel::Parent {
            command: ChildCommand::Leaf {
                value: "ok".to_string(),
            },
        },
    }
    .execute_for_mcp_with_state(&state)
    .expect("second root struct call should execute for MCP with state");
    assert_eq!(first.into_string(), "leaf=ok:1");
    assert_eq!(second.into_string(), "leaf=ok:2");
}
