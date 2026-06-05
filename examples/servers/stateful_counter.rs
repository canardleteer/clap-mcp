mod stateful_counter_common;

use clap_mcp::ParseOrServeMcpWithState;
use stateful_counter_common::{run, App, CounterState};
use std::sync::{Arc, Mutex};

/// Stateful MCP counter (`--mcp` keeps `Arc<Mutex<CounterState>>` across tool calls).
///
/// Try:
/// - `cargo run -p clap-mcp-examples --bin stateful_counter -- increment`
/// - `cargo run -p clap-mcp-examples --bin stateful_counter -- --mcp`
fn main() {
    let state = Arc::new(Mutex::new(CounterState::default()));
    let app = App::parse_or_serve_mcp_with_state(state.clone());
    println!("{}", run(app.command, &state));
}
