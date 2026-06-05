#![allow(unused_assignments, unused_variables)]

use clap::{Parser, Subcommand};
use clap_mcp::{ClapMcp, ClapMcpToolExecutorWithState};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct State {
    value: String,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe = true)]
#[clap_mcp_state_type = "Mutex<State>"]
#[command(name = "nested-subcommands-with-state-pass", subcommand_required = true)]
struct Root {
    #[command(subcommand)]
    command: TopLevel,
}

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp(reinvocation_safe = true)]
#[clap_mcp_output_from_with_state = "run_top_level"]
#[clap_mcp_state_type = "Mutex<State>"]
enum TopLevel {
    Parent {
        #[command(subcommand)]
        command: ChildCommand,
    },
}

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp(reinvocation_safe = true)]
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
    match cmd {
        ChildCommand::Leaf { value } => {
            state.lock().expect("state mutex").value = value.clone();
            format!("leaf={value}")
        }
    }
}

fn main() {
    let state = Arc::new(Mutex::new(State::default()));
    let cli = Root {
        command: TopLevel::Parent {
            command: ChildCommand::Leaf {
                value: "ok".to_string(),
            },
        },
    };
    let result = cli
        .execute_for_mcp_with_state(&state)
        .expect("root struct should execute for MCP with state");
    assert!(matches!(result, clap_mcp::ClapMcpToolOutput::Text(_)));
    assert_eq!(state.lock().expect("state mutex").value, "ok");
}
