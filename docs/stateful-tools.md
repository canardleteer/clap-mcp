# Stateful MCP tools (shared session state)

> Guide for CLI authors adding clap-mcp. See [README](../README.md) to get started.

[← Documentation index](../README.md#documentation)

When `reinvocation_safe` is true, in-process tool calls can share session state
for the MCP server lifetime. The server stores state in an [`Arc`] internally; your
`run` function receives **`&Self::State`** on each call (not `&Arc<…>`).

Setup (see also [`ClapMcpToolExecutorWithState`] rustdoc):

* **Leaf subcommand enum:** `#[clap_mcp_output_from_with_state = "run"]` plus
  `#[clap_mcp_state_type = "Type"]` where `Type` matches the second parameter of
  `run` (e.g. `run(cmd, state: &Mutex<CounterState>)` →
  `#[clap_mcp_state_type = "Mutex<CounterState>"]`).
* **Struct root / intermediate enums:** `#[clap_mcp(stateful)]` — `State` is
  inferred from the subcommand field; do not repeat `state_type`.

Full drop-in source (same pattern as
[stateful_counter](../examples/servers/stateful_counter.rs)):

```rust
use clap::{Parser, Subcommand};
use clap_mcp::{ClapMcp, ParseOrServeMcpWithState};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct CounterState {
    count: u64,
}

#[derive(Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, stateful)]
#[command(name = "stateful-counter", subcommand_required = true)]
struct App {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, ClapMcp)]
#[clap_mcp(reinvocation_safe)]
#[clap_mcp_output_from_with_state = "run"]
#[clap_mcp_state_type = "Mutex<CounterState>"]
enum Command {
    Increment,
    Read,
}

fn run(cmd: Command, state: &Mutex<CounterState>) -> String {
    let mut guard = state.lock().expect("counter mutex");
    match cmd {
        Command::Increment => {
            guard.count += 1;
            format!("count={}", guard.count)
        }
        Command::Read => format!("count={}", guard.count),
    }
}

fn main() {
    let state = Arc::new(Mutex::new(CounterState::default()));
    let app = App::parse_or_serve_mcp_with_state(state.clone());
    println!("{}", run(app.command, state.as_ref()));
}
```

Entrypoints: [`ParseOrServeMcpWithState::parse_or_serve_mcp_with_state`],
[`parse_or_serve_mcp_with_state`], and
[`ServeMcpBuilder::for_cli_with_state`]. Requires `reinvocation_safe`.
Example binary:
[stateful_counter](../examples/servers/stateful_counter.rs) (ported from
[PR #11](https://github.com/canardleteer/clap-mcp/pull/11) by Eddy Stefes / fneddy).

Session state is shared for the MCP server process lifetime, not per client or OS
user. See [Security — In-process execution and shared state](security.md#in-process-execution-and-shared-state)
before exposing a stateful server beyond localhost or a single trusted operator.
