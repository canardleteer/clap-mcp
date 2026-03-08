# clap-mcp Examples

This directory contains example CLIs that demonstrate clap-mcp capabilities.

Run all commands from the **workspace root** (the parent of this `examples/` directory). The examples depend on `clap-mcp` via a path dependency.

- **`client.rs`** — MCP client that exercises the server examples (easiest way to see everything working)
- **`servers/`** — Example MCP server CLIs (subcommands, struct_subcommand, optional_commands_and_args, result_output, structured, tracing_bridge, log_bridge, async_sleep, async_sleep_shared, **subprocess_exit_handling**, **panic_catch_opt_in**, **custom_resources_prompts**, **vec_and_flags**)

## Crash / panic behavior

When a tool fails internally, behavior depends on execution mode:

- **Subprocess (`reinvocation_safe = false`):** If the tool process exits with a non-zero status, the server returns a tool result with `is_error: true` and a message that includes the exit code (and stderr when non-empty). See **subprocess_exit_handling**.
- **In-process (`reinvocation_safe = true`):** By default, a panic in tool code crashes the server. With **`catch_in_process_panics = true`** (opt-in), panics are caught and returned as an MCP error; the server stays up. After a caught panic, the process may no longer be reinvocation_safe — consider restarting the server. See **panic_catch_opt_in** and [`ClapMcpConfig::catch_in_process_panics`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapMcpConfig.html#structfield.catch_in_process_panics).

## Testing with the Client Example

The `client` example is the easiest way to see everything working together. It runs each server and exercises its tools:

```bash
# Test subcommands (default)
cargo run -p clap-mcp-examples --bin client -- subcommands

# Test structured
cargo run -p clap-mcp-examples --bin client -- structured

# Test struct_subcommand
cargo run -p clap-mcp-examples --bin client -- struct-subcommand

# Test optional_commands_and_args
cargo run -p clap-mcp-examples --bin client -- optional-commands-and-args

# Test result_output (Result<T, E> with #[clap_mcp_output_from])
cargo run -p clap-mcp-examples --bin client -- result-output

# Test tracing_bridge
cargo run -p clap-mcp-examples --bin client -- tracing-bridge

# Test async_sleep (dedicated thread)
cargo run -p clap-mcp-examples --bin client -- async-sleep

# Test async_sleep_shared (shared runtime)
cargo run -p clap-mcp-examples --bin client -- async-sleep-shared

# Test log_bridge
cargo run -p clap-mcp-examples --bin client -- log-bridge
```

### custom_resources_prompts

Custom MCP resources and prompts, and the `--export-skills` flag. Adds a static resource (`example://readme`) and a static prompt (`example-prompt`) via `ClapMcpServeOptions`. When run with `--mcp`, clients can list/read the extra resource and list/get the prompt. When run with `--export-skills` (or `--export-skills=DIR`), generates [Agent Skills](https://agentskills.io/specification) (SKILL.md) into `.agents/skills/` or the given directory.

```bash
# Normal CLI
cargo run -p clap-mcp-examples --bin custom_resources_prompts -- echo --message "hi"

# MCP server (includes custom resource and prompt)
cargo run -p clap-mcp-examples --bin custom_resources_prompts -- --mcp

# Export agent skills (default: .agents/skills/custom-resources-prompts/)
cargo run -p clap-mcp-examples --bin custom_resources_prompts -- --export-skills

# Export to a specific directory
cargo run -p clap-mcp-examples --bin custom_resources_prompts -- --export-skills=./out
```

### vec_and_flags

Demonstrates **Vec (list)** and **action-based** args in MCP: `--files` and positional `versions` are exposed as arrays, `dry_run` as boolean, and `verbose` as integer (count). Plain text output only.

```bash
# Normal CLI: option list (--files a --files b), positional list (1.0 2.0)
cargo run -p clap-mcp-examples --bin vec_and_flags -- run --files a --files b --files c 1.0 2.0 --dry-run -vv

# MCP server mode (inspect tool schema: files and versions = array, dry_run = boolean, verbose = integer)
cargo run -p clap-mcp-examples --bin vec_and_flags -- --mcp
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

### struct_subcommand

Struct root with `#[command(subcommand)]`, optional subcommand
(`Option<Commands>`), and `#[clap_mcp(...)]` on the struct. Output attributes
live on the subcommand enum variants. Also demonstrates **root-level
`#[clap_mcp(skip)]`**: the `--out` option is available to the CLI but hidden
from MCP tool schemas.

```bash
# Normal CLI usage (no subcommand)
cargo run -p clap-mcp-examples --bin struct_subcommand

# With subcommands
cargo run -p clap-mcp-examples --bin struct_subcommand -- greet --name Rust
cargo run -p clap-mcp-examples --bin struct_subcommand -- add --a 2 --b 3
cargo run -p clap-mcp-examples --bin struct_subcommand -- sub --a 10 --b 5

# MCP server mode
cargo run -p clap-mcp-examples --bin struct_subcommand -- --mcp
```

### optional_commands_and_args

Demonstrates `#[clap_mcp(skip)]` and `#[clap_mcp(requires)]`:
- **skip**: `internal` subcommand is hidden from MCP
- **requires** (argument-level): `read`'s `path` is optional in CLI but required in MCP
- **requires** (variant-level): `process`'s `path` and `input` are required in MCP

```bash
# Normal CLI usage
cargo run -p clap-mcp-examples --bin optional_commands_and_args -- public
cargo run -p clap-mcp-examples --bin optional_commands_and_args -- internal
cargo run -p clap-mcp-examples --bin optional_commands_and_args -- read --path /tmp/foo
cargo run -p clap-mcp-examples --bin optional_commands_and_args -- process --path /tmp --input data

# MCP server mode (only public, read, process are exposed; internal is skipped)
cargo run -p clap-mcp-examples --bin optional_commands_and_args -- --mcp
```

### result_output

Demonstrates `#[clap_mcp_output_from = "run"]` with a fallible `run` that returns
`Result<T, E>`. `Ok(value)` produces normal MCP output; `Err(e)` produces an MCP error
response (`is_error: true`). Implements `IntoClapMcpToolError` for a custom error type
so structured errors are sent as JSON.

```bash
# Normal CLI usage
cargo run -p clap-mcp-examples --bin result_output -- sqrt --n 42
cargo run -p clap-mcp-examples --bin result_output -- sqrt --n -1   # exits with error
cargo run -p clap-mcp-examples --bin result_output -- double --x 21
cargo run -p clap-mcp-examples --bin result_output -- check --x 10
cargo run -p clap-mcp-examples --bin result_output -- check --x -5  # exits with error
cargo run -p clap-mcp-examples --bin result_output -- parse --path /tmp/foo
cargo run -p clap-mcp-examples --bin result_output -- parse --path invalid  # exits with error

# MCP server mode
cargo run -p clap-mcp-examples --bin result_output -- --mcp
```

### structured

CLI with structured JSON output via `#[clap_mcp_output_from = "run"]` and
`AsStructured<T>`. No optional features required.

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

### subprocess_exit_handling

Subprocess execution (`reinvocation_safe = false`) with a tool that exits non-zero.
When the tool process exits with a non-zero status, the MCP server returns a tool
result with `is_error: true` and a message that includes the exit code (and stderr).
Uses **`subcommand_required = true`**; `--mcp` alone is valid and starts the MCP server
(clap-mcp handles `--mcp` before clap's subcommand check).

```bash
# Normal CLI usage
cargo run -p clap-mcp-examples --bin subprocess_exit_handling -- succeed
cargo run -p clap-mcp-examples --bin subprocess_exit_handling -- exit-fail   # exits with code 1

# MCP server mode (calling exit-fail returns is_error: true)
cargo run -p clap-mcp-examples --bin subprocess_exit_handling -- --mcp
```

### panic_catch_opt_in

In-process execution with `catch_in_process_panics = true`. Panics in tool code
are caught and returned as an MCP error instead of crashing the server. After a
caught panic, the process may no longer be reinvocation_safe — consider restarting.
Uses **`subcommand_required = true`**; `--mcp` alone is valid and starts the MCP server.

```bash
# Normal CLI usage
cargo run -p clap-mcp-examples --bin panic_catch_opt_in -- succeed
cargo run -p clap-mcp-examples --bin panic_catch_opt_in -- panic-demo   # panics

# MCP server mode (calling panic-demo returns is_error: true, server stays up)
cargo run -p clap-mcp-examples --bin panic_catch_opt_in -- --mcp
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

| Example            | Path                            | Demonstrates                                                       |
| ------------------ | ------------------------------- | ------------------------------------------------------------------ |
| **subcommands**    | `servers/subcommands.rs`        | Text output, structured output, subprocess                         |
| **struct_subcommand** | `servers/struct_subcommand.rs` | Struct root, `#[command(subcommand)]`, optional subcommand         |
| **optional_commands_and_args** | `servers/optional_commands_and_args.rs` | `#[clap_mcp(skip)]`, `#[clap_mcp(requires)]` (arg and variant-level) |
| **result_output**  | `servers/result_output.rs`      | `#[clap_mcp_output_from]` with `Result<T, E>`, `IntoClapMcpToolError` for structured errors |
| **structured**     | `servers/structured.rs`         | Structured output via `#[clap_mcp_output_from]` and `AsStructured<T>` |
| **tracing_bridge** | `servers/tracing_bridge.rs`  | Tracing integration, MCP log forwarding, prompts   |
| **log_bridge**     | `servers/log_bridge.rs`      | `log` crate integration, MCP log forwarding       |
| **async_sleep**       | `servers/async_sleep.rs`        | Async tokio, 3 sleep tasks, `share_runtime = false` |
| **async_sleep_shared** | `servers/async_sleep_shared.rs` | Same, `share_runtime = true` (shares `async_sleep_common`) |
| **subprocess_exit_handling** | `servers/subprocess_exit_handling.rs` | Subprocess non-zero exit → MCP `is_error: true` |
| **panic_catch_opt_in** | `servers/panic_catch_opt_in.rs` | In-process panic catching (opt-in), server stays up |
| **client**            | `client.rs`                    | MCP client that exercises the server examples      |

## Async tools and share_runtime

When your CLI has async subcommands (e.g. using `tokio::sleep`, `tokio::spawn`), do async
work inside your `run` function and call `clap_mcp::run_async_tool` from there. Configure
`share_runtime` in `#[clap_mcp(...)]`: `false` (default) uses a dedicated thread per call;
`true` shares the MCP server's tokio runtime. See **async_sleep** and **async_sleep_shared**
for full examples.
