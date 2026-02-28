# clap-mcp Examples

This directory contains example CLIs that demonstrate clap-mcp capabilities.

- **`client.rs`** — MCP client that exercises the server examples (easiest way to see everything working)
- **`servers/`** — Example MCP server CLIs (derive, structured, tracing_bridge, log_bridge)

## Testing with the Client Example

The `client` example is the easiest way to see everything working together. It runs each server and exercises its tools:

```bash
# Test derive (default)
cargo run --example client --features tracing -- derive

# Test structured
cargo run --example client --features tracing -- structured

# Test tracing_bridge
cargo run --example client --features tracing -- tracing-bridge

# Test log_bridge (requires both tracing and log features)
cargo run --example client --features tracing,log -- log-bridge
```

## Running Server Examples Directly

Each server example can be run as a normal CLI or as an MCP server over stdio.

### derive

Basic example with text output, structured output, and subprocess execution. No
optional features required.

```bash
# Normal CLI usage
cargo run --example derive -- greet --name Rust
cargo run --example derive -- add 2 3
cargo run --example derive -- sub 10 5

# MCP server mode (exposes tools over stdio)
cargo run --example derive -- --mcp
```

### structured

CLI with structured JSON output via `#[clap_mcp_output_type]`. No optional
features required.

```bash
# Normal CLI usage
cargo run --example structured -- add 7 3

# MCP server mode
cargo run --example structured -- --mcp
```

### tracing_bridge

CLI with `tracing` integration. Requires the `tracing` feature, which enables
`ClapMcpTracingLayer` — a standard `tracing_subscriber::Layer` that forwards
tracing events to MCP clients via `notifications/message`. The layer composes
with any other layers in your subscriber stack (e.g. `tracing-opentelemetry`,
file appenders).

```bash
# Normal CLI usage (requires tracing feature)
cargo run --example tracing_bridge --features tracing -- echo "hello"

# MCP server mode
cargo run --example tracing_bridge --features tracing -- --mcp
```

### log_bridge

CLI with `log` crate integration. Requires the `log` feature, which enables
`ClapMcpLogBridge` — a `log::Log` implementation that forwards `log::info!`,
`log::debug!`, etc. to MCP clients. Note that the `log` crate supports only one
global logger; see the [main README](../README.md#log-feature) for guidance on multiplexing to disk and MCP.

```bash
# Normal CLI usage (requires log feature)
cargo run --example log_bridge --features log -- echo "hello"

# MCP server mode
cargo run --example log_bridge --features log -- --mcp
```

## Example Summary

| Example            | Path                         | Required feature | Demonstrates                                       |
| ------------------ | ---------------------------- | ---------------- | -------------------------------------------------- |
| **derive**         | `servers/derive.rs`          | —                | Text output, structured output, subprocess         |
| **structured**     | `servers/structured.rs`      | —                | Structured output only (`#[clap_mcp_output_type]`) |
| **tracing_bridge** | `servers/tracing_bridge.rs`  | `tracing`        | Tracing integration, MCP log forwarding, prompts   |
| **log_bridge**     | `servers/log_bridge.rs`      | `log`            | `log` crate integration, MCP log forwarding        |
| **client**         | `client.rs`                  | `tracing`        | MCP client that exercises the server examples      |
