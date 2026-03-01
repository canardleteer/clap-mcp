# clap-mcp Examples

This directory contains example CLIs that demonstrate clap-mcp capabilities.

Run all commands from the **workspace root** (the parent of this `examples/` directory). The examples depend on `clap-mcp` via a path dependency.

- **`client.rs`** — MCP client that exercises the server examples (easiest way to see everything working)
- **`servers/`** — Example MCP server CLIs (subcommands, structured, tracing_bridge, log_bridge, async_sleep, async_sleep_shared)

## Testing with the Client Example

The `client` example is the easiest way to see everything working together. It runs each server and exercises its tools:

```bash
# Test subcommands (default)
cargo run -p clap-mcp-examples --bin client -- subcommands

# Test structured
cargo run -p clap-mcp-examples --bin client -- structured

# Test tracing_bridge
cargo run -p clap-mcp-examples --bin client -- tracing-bridge

# Test async_sleep (dedicated thread)
cargo run -p clap-mcp-examples --bin client -- async-sleep

# Test async_sleep_shared (shared runtime)
cargo run -p clap-mcp-examples --bin client -- async-sleep-shared

# Test log_bridge
cargo run -p clap-mcp-examples --bin client -- log-bridge
```

## Running Server Examples Directly

Each server example can be run as a normal CLI or as an MCP server over stdio.

### subcommands

Basic example with text output, structured output, and subprocess execution. No
optional features required.

```bash
# Normal CLI usage
cargo run -p clap-mcp-examples --bin subcommands -- greet --name Rust
cargo run -p clap-mcp-examples --bin subcommands -- add 2 3
cargo run -p clap-mcp-examples --bin subcommands -- sub 10 5

# MCP server mode (exposes tools over stdio)
cargo run -p clap-mcp-examples --bin subcommands -- --mcp
```

### structured

CLI with structured JSON output via `#[clap_mcp_output_type]`. No optional
features required.

```bash
# Normal CLI usage
cargo run -p clap-mcp-examples --bin structured -- add 7 3

# MCP server mode
cargo run -p clap-mcp-examples --bin structured -- --mcp
```

### tracing_bridge

CLI with `tracing` integration. Uses `ClapMcpTracingLayer` — a standard
`tracing_subscriber::Layer` that forwards tracing events to MCP clients via
`notifications/message`. The layer composes with any other layers in your
subscriber stack (e.g. `tracing-opentelemetry`, file appenders).

```bash
# Normal CLI usage
cargo run -p clap-mcp-examples --bin tracing_bridge -- echo "hello"

# MCP server mode
cargo run -p clap-mcp-examples --bin tracing_bridge -- --mcp
```

### async_sleep

CLI with tokio async runtime (dedicated thread). Single subcommand that awaits
3 concurrent sleep tasks and returns structured JSON. Uses `share_runtime = false`.
Shares business logic with async_sleep_shared via `async_sleep_common` module.

```bash
# Normal CLI usage
cargo run -p clap-mcp-examples --bin async_sleep -- sleep-demo

# MCP server mode
cargo run -p clap-mcp-examples --bin async_sleep -- --mcp
```

### async_sleep_shared

Same as async_sleep but with `share_runtime = true` — uses the MCP server's
tokio runtime instead of a dedicated thread. Shares the `async_sleep_common`
module for the sleep logic.

```bash
# Normal CLI usage
cargo run -p clap-mcp-examples --bin async_sleep_shared -- sleep-demo

# MCP server mode
cargo run -p clap-mcp-examples --bin async_sleep_shared -- --mcp
```

### log_bridge

CLI with `log` crate integration. Uses `ClapMcpLogBridge` — a `log::Log`
implementation that forwards `log::info!`, `log::debug!`, etc. to MCP clients.
Note that the `log` crate supports only one global logger; see the [main
README](../README.md#log-feature) for guidance on multiplexing to disk and MCP.

```bash
# Normal CLI usage
cargo run -p clap-mcp-examples --bin log_bridge -- echo "hello"

# MCP server mode
cargo run -p clap-mcp-examples --bin log_bridge -- --mcp
```

## Example Summary

| Example            | Path                         | Demonstrates                                       |
| ------------------ | ---------------------------- | -------------------------------------------------- |
| **subcommands**    | `servers/subcommands.rs`     | Text output, structured output, subprocess         |
| **structured**     | `servers/structured.rs`      | Structured output only (`#[clap_mcp_output_type]`) |
| **tracing_bridge** | `servers/tracing_bridge.rs`  | Tracing integration, MCP log forwarding, prompts   |
| **log_bridge**     | `servers/log_bridge.rs`      | `log` crate integration, MCP log forwarding       |
| **async_sleep**       | `servers/async_sleep.rs`        | Async tokio, 3 sleep tasks, `share_runtime = false` |
| **async_sleep_shared** | `servers/async_sleep_shared.rs` | Same, `share_runtime = true` (shares `async_sleep_common`) |
| **client**            | `client.rs`                    | MCP client that exercises the server examples      |

## Async tools and share_runtime

When your CLI has async subcommands (e.g. using `tokio::sleep`, `tokio::spawn`),
use `clap_mcp::run_async_tool` in `#[clap_mcp_output]` and configure
`share_runtime` in `#[clap_mcp(...)]`:

| `share_runtime` | Behavior | When to use |
|-----------------|----------|-------------|
| `false` (default) | Dedicated thread with its own tokio runtime per tool call. No nesting, no special setup. | **Recommended.** Use unless you need deep integration. |
| `true` | Shares the MCP server's tokio runtime. Requires `reinvocation_safe` and uses a multi-thread runtime. | Advanced: when you need to share runtime state, spawn tasks that outlive the tool call, or integrate with other async code. |

**Non-shared (default):**

```rust
#[clap_mcp(reinvocation_safe, parallel_safe = false, share_runtime = false)]
enum Cli {
    #[clap_mcp_output_type = "SleepResult"]
    #[clap_mcp_output = "clap_mcp::run_async_tool(&Cli::clap_mcp_config(), || run_sleep_demo())"]
    SleepDemo,
}
```

**Shared runtime:**

```rust
#[clap_mcp(reinvocation_safe, parallel_safe = false, share_runtime)]
enum Cli {
    #[clap_mcp_output_type = "SleepResult"]
    #[clap_mcp_output = "clap_mcp::run_async_tool(&Cli::clap_mcp_config(), || run_sleep_demo())"]
    SleepDemo,
}
```

`share_runtime` only applies when `reinvocation_safe` is true. When
`reinvocation_safe` is false, tools run in subprocesses and `share_runtime` is
ignored.
