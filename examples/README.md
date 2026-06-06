# clap-mcp Examples

This directory contains example CLIs that demonstrate clap-mcp capabilities.

Run all commands from the **workspace root** (the parent of this `examples/`
directory). The examples depend on `clap-mcp` via a path dependency.

## 0.0.4 API (derive path)

Since **0.0.4-rc.1**, examples use the slim derive entrypoints and
[`ServeMcpBuilder`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ServeMcpBuilder.html)
for imperative embedders:

* **`ParseOrServeMcp::parse_or_serve_mcp()`** — default serve options; import
  `clap_mcp::ParseOrServeMcp`.
* **`parse_or_serve_mcp_with(ClapMcpRunOptions { config, serve })`** — custom
  logging, resources, or HTTP-related serve options.
* **`ServeMcpBuilder::for_cli::<T>(listen)`** — derive CLI pre-filled; call
  `.serve().await` or `.serve_blocking()` (see `async_embedder_serve`,
  `placeholder_server`, `invalid_executable_server`).
* **`ServeMcpBuilder::for_cli_with_state::<T, S>(listen, state)`** — like
  `for_cli`, but captures shared state for stateful derive CLIs (see
  `stateful_counter`).
* **`ServeMcpBuilder::new()`** — hand-built schema/config for imperative embedders.
* **`serve_mcp` / `serve_mcp_blocking`** — lower-level 7-arg equivalents (delegate to builder).
* **`tools_from_schema_with_metadata`** — build MCP tools from schema +
  [`ClapMcpSchemaMetadata`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapMcpSchemaMetadata.html)
  (including `#[clap_mcp(task_augmented_tools)]`).

Removed: `parse_or_serve_mcp_with_config*`, freestanding `tools_from_schema*`,
public `ClapMcpServer` / `build_clap_mcp_server`. See
[API slim notes](../docs/rmcp-migration-notes.md#004-rc1-api-slim-post-rmcp-port).

* **`client.rs`** — MCP client that exercises the server examples (easiest way
  to see everything working)
* **`task_augmented_client.rs`** — Minimal client for MCP task-augmented
  `tools/call` (requires `--features tracing`; use with `task_tools_dedicated`
  or `task_tools_shared`)
* **`servers/`** — Example MCP server CLIs (subcommands, struct_subcommand,
  **struct_subcommand_required**, optional_commands_and_args, result_output, structured, tracing_bridge,
  log_bridge, async_sleep, async_sleep_shared, **async_embedder_serve**,
  **task_tools_dedicated**,
  **task_tools_shared**, **subprocess_exit_handling**, **panic_catch_opt_in**,
  **custom_resources_prompts**, **vec_and_flags**, **passthrough_args**,
  **passthrough_args_subprocess**, **custom_mcp_flags**, **stateful_counter**)

## Async embedders

Two patterns for async MCP servers with `share_runtime = true`:

| Example | Pattern |
|---------|---------|
| **async_sleep_shared** | Derive path: `parse_or_serve_mcp_with` from sync `main` |
| **async_embedder_serve** | Imperative path: `ServeMcpBuilder::for_cli` + `.serve().await` from `#[tokio::main]` |

See [README Async embedders](../README.md#async-embedders) for runtime selection details.

## Crash / panic behavior

When a tool fails internally, behavior depends on execution mode:

* **Subprocess (`reinvocation_safe = false`):** If the tool process exits with a
  non-zero status, the server returns a tool result with `is_error: true` and a
  message that includes the exit code (and stderr when non-empty). See
  **subprocess_exit_handling**.
* **In-process (`reinvocation_safe = true`):** By default, a panic in tool code
  crashes the server. With **`catch_in_process_panics = true`** (opt-in), panics
  are caught and returned as an MCP error; the server stays up. After a caught
  panic, the process may no longer be reinvocation_safe — consider restarting
  the server. See **panic_catch_opt_in** and
  [`ClapMcpConfig::catch_in_process_panics`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapMcpConfig.html#structfield.catch_in_process_panics).

## Testing with the Client Example

The `client` example is the easiest way to see everything working together. It
runs each server and exercises its tools:

```bash
# Test subcommands (default)
cargo run -p clap-mcp-examples --bin client -- subcommands

# Test structured
cargo run -p clap-mcp-examples --bin client -- structured

# Test struct_subcommand (optional subcommand demo)
cargo run -p clap-mcp-examples --bin client -- struct-subcommand

# Test struct_subcommand_required (recommended struct-root migration)
cargo run -p clap-mcp-examples --bin client -- struct-subcommand-required

# Test optional_commands_and_args
cargo run -p clap-mcp-examples --bin client -- optional-commands-and-args

# Test result_output (Result<T, E> with #[clap_mcp_output_from])
cargo run -p clap-mcp-examples --bin client -- result-output

# Test tracing_bridge
cargo run -p clap-mcp-examples --bin client -- tracing-bridge

# Test async_sleep (dedicated thread)
cargo run -p clap-mcp-examples --bin client -- async-sleep

# Test async_sleep_shared (shared runtime, derive path)
cargo run -p clap-mcp-examples --bin client -- async-sleep-shared

# Test async_embedder_serve (shared runtime, imperative ServeMcpBuilder)
cargo run -p clap-mcp-examples --bin client -- async-embedder-serve

# Test log_bridge
cargo run -p clap-mcp-examples --bin client -- log-bridge
```

### custom_resources_prompts

Custom MCP resources and prompts, and the `--export-skills` flag. Adds a static
resource (`example://readme`) and a static prompt (`example-prompt`) via
`ClapMcpServeOptions`. When run with `--mcp`, clients can list/read the extra
resource and list/get the prompt. When run with `--export-skills` (or
`--export-skills=DIR`), generates
[Agent Skills](https://agentskills.io/specification) (SKILL.md) into
`.agents/skills/` or the given directory.

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

Demonstrates **Vec (list)** and **action-based** args in MCP: `--files` and
positional `versions` are exposed as arrays, `dry_run` as boolean, and `verbose`
as integer (count). Plain text output only.

```bash
# Normal CLI: option list (--files a --files b), positional list (1.0 2.0)
cargo run -p clap-mcp-examples --bin vec_and_flags -- run --files a --files b --files c 1.0 2.0 --dry-run -vv

# MCP server mode (inspect tool schema: files and versions = array, dry_run = boolean, verbose = integer)
cargo run -p clap-mcp-examples --bin vec_and_flags -- --mcp
```

### passthrough_args / passthrough_args_subprocess

Demonstrates **trailing passthrough** for shell and MCP:

| Pattern | clap | MCP |
|---|---|---|
| `exec` | `#[arg(last = true, allow_hyphen_values = true)] command: Vec<String>` | Pass `command` as JSON array (hyphen tokens OK) |
| `forward` | `#[arg(long)] args: Vec<String>` | Pass `args` as JSON array |
| `run` | `#[clap_mcp(skip)]` on internal-only field | `internal` hidden from tool schema |

**passthrough_args** uses in-process execution (`reinvocation_safe = true`);
**passthrough_args_subprocess** uses subprocess reinvocation. Cross-link:
**vec_and_flags** for list/flag schema shapes.

```bash
# Shell: cargo-style trailing args after --
cargo run -p clap-mcp-examples --bin passthrough_args -- exec --dry-run -- echo hello

# Shell: --mcp after -- is passthrough, not MCP server startup
cargo run -p clap-mcp-examples --bin passthrough_args -- exec -- --mcp

# MCP server
cargo run -p clap-mcp-examples --bin passthrough_args -- --mcp
```

### custom_mcp_flags

When your CLI already has `--mcp` for an unrelated purpose, rename clap-mcp's
stdio flag (e.g. `--modelcontextprotocol`) via `#[clap_mcp(mcp_flag = "modelcontextprotocol")]`.
User `--mcp` runs normal CLI; `--modelcontextprotocol` starts the MCP server.

```bash
cargo run -p clap-mcp-examples --bin custom_mcp_flags -- --mcp
cargo run -p clap-mcp-examples --bin custom_mcp_flags -- --modelcontextprotocol
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

### struct_subcommand_required

Struct root with **required** subcommand (`command: Commands`,
`subcommand_required = true`) — the typical migration path when your CLI already
required a subcommand before clap-mcp. Bare invocation still fails with clap's
missing-subcommand error; normal subcommands and `myapp --mcp` work unchanged.

```bash
# Bare invocation fails (same as pre-clap-mcp clap)
cargo run -p clap-mcp-examples --bin struct_subcommand_required
# exit code != 0

# Normal CLI usage
cargo run -p clap-mcp-examples --bin struct_subcommand_required -- greet --name Rust
cargo run -p clap-mcp-examples --bin struct_subcommand_required -- add --a 2 --b 3

# MCP server mode
cargo run -p clap-mcp-examples --bin struct_subcommand_required -- --mcp
```

### struct_subcommand

Struct root with **optional** subcommand (`Option<Commands>`,
`subcommand_required = false`) — demonstrates optional subcommands at the clap
level only; **not** the recommended MCP migration path. See
**struct_subcommand_required** above for zero-regression migration.

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

* **skip**: `internal` subcommand is hidden from MCP
* **requires** (argument-level): `read`'s `path` is optional in CLI but required
  in MCP
* **requires** (variant-level): `process`'s `path` and `input` are required in
  MCP

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

Demonstrates `#[clap_mcp_output_from = "run"]` with a fallible `run` that
returns
`Result<T, E>`. `Ok(value)` produces normal MCP output; `Err(e)` produces an MCP
error
response (`is_error: true`). Implements `IntoClapMcpToolError` for a custom
error type
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
3 concurrent sleep tasks and returns structured JSON. Uses `share_runtime =
false`.
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

### task_tools_dedicated / task_tools_shared

MCP **task-augmented** `tools/call` with `#[clap_mcp(task_augmented_tools)]` and
`#[clap_mcp(task)]` on the async sleep subcommand. **task_tools_dedicated** uses
`share_runtime = false` (dedicated async runtime per call);
**task_tools_shared** uses `share_runtime = true`. Requires `--features
tracing`.

Use **task_augmented_client** to run an end-to-end client
(`CallToolRequestParams` with `task: Some(...)`, poll `tasks/get`, then
`tasks/result`):

```bash
cargo run -p clap-mcp-examples --bin task_tools_dedicated --features tracing -- sleep --ms 80
cargo run -p clap-mcp-examples --bin task_tools_dedicated --features tracing -- --mcp

cargo run -p clap-mcp-examples --bin task_augmented_client --features tracing -- task_tools_dedicated
cargo run -p clap-mcp-examples --bin task_augmented_client --features tracing -- task_tools_shared
```

### subprocess_exit_handling

Subprocess execution (`reinvocation_safe = false`) with a tool that exits
non-zero.
When the tool process exits with a non-zero status, the MCP server returns a
tool
result with `is_error: true` and a message that includes the exit code (and
stderr).
Uses **`subcommand_required = true`**; `--mcp` alone is valid and starts the MCP
server
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
caught panic, the process may no longer be reinvocation_safe — consider
restarting.
Uses **`subcommand_required = true`**; `--mcp` alone is valid and starts the MCP
server.

```bash
# Normal CLI usage
cargo run -p clap-mcp-examples --bin panic_catch_opt_in -- succeed
cargo run -p clap-mcp-examples --bin panic_catch_opt_in -- panic-demo   # panics

# MCP server mode (calling panic-demo returns is_error: true, server stays up)
cargo run -p clap-mcp-examples --bin panic_catch_opt_in -- --mcp
```

### task_panic_catch

Task-augmented `tools/call` with `catch_in_process_panics = true`. A panicking
task tool returns an error payload on `tasks/result` (`is_error: true`); the
server stays up. Pair with **task_augmented_client** or integration tests.

```bash
cargo run -p clap-mcp-examples --bin task_panic_catch --features tracing -- sleep --ms 50
cargo run -p clap-mcp-examples --bin task_panic_catch --features tracing -- --mcp
```

### task_parallel_probe_dedicated / task_parallel_probe_shared

Integration-test servers for **concurrent** task-augmented execution
(`parallel_safe = true`). Set `CLAP_MCP_SERIAL_PROBE` to a JSONL path to record
`body_start` / `body_end` probe events (same format as `task_serial_probe_*`).
Not intended for interactive demos.

```bash
cargo run -p clap-mcp-examples --bin task_parallel_probe_dedicated --features tracing -- --mcp
```

### log_bridge

CLI with `log` crate integration. Uses `ClapMcpLogBridge` — a `log::Log`
implementation that forwards `log::info!`, `log::debug!`, etc. to MCP clients.
Note that the `log` crate supports only one global logger; see the
[main README](../README.md#log-feature) for guidance on multiplexing to disk
and MCP.

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
| **struct_subcommand_required** | `servers/struct_subcommand_required.rs` | Required subcommand struct root (recommended migration) |
| **struct_subcommand** | `servers/struct_subcommand.rs` | Optional subcommand struct root (clap demo only)         |
| **optional_commands_and_args** | `servers/optional_commands_and_args.rs` | `#[clap_mcp(skip)]`, `#[clap_mcp(requires)]` (arg and variant-level) |
| **passthrough_args** | `servers/passthrough_args.rs` | Trailing `Vec` passthrough (in-process); see `passthrough_common.rs` |
| **passthrough_args_subprocess** | `servers/passthrough_args_subprocess.rs` | Same patterns, subprocess reinvocation |
| **custom_mcp_flags** | `servers/custom_mcp_flags.rs` | Renamed stdio flag when `--mcp` is already taken |
| **vec_and_flags** | `servers/vec_and_flags.rs` | Vec/list and flag/count args in MCP schema |
| **result_output**  | `servers/result_output.rs`      | `#[clap_mcp_output_from]` with `Result<T, E>`, `IntoClapMcpToolError` for structured errors |
| **structured**     | `servers/structured.rs`         | Structured output via `#[clap_mcp_output_from]` and `AsStructured<T>` |
| **tracing_bridge** | `servers/tracing_bridge.rs`  | Tracing integration, MCP log forwarding, prompts   |
| **log_bridge**     | `servers/log_bridge.rs`      | `log` crate integration, MCP log forwarding       |
| **async_sleep**       | `servers/async_sleep.rs`        | Async tokio, 3 sleep tasks, `share_runtime = false` |
| **async_sleep_shared** | `servers/async_sleep_shared.rs` | Same, `share_runtime = true` (shares `async_sleep_common`) |
| **task_tools_dedicated** | `servers/task_tools_dedicated.rs` | Task-augmented `tools/call`, `share_runtime = false` |
| **task_tools_shared** | `servers/task_tools_shared.rs` | Task-augmented `tools/call`, `share_runtime = true` |
| **task_serial_probe_dedicated** | `servers/task_serial_probe_dedicated.rs` | Serialized task probe (`parallel_safe = false`, dedicated runtime) |
| **task_serial_probe_shared** | `servers/task_serial_probe_shared.rs` | Serialized task probe (`parallel_safe = false`, shared runtime) |
| **task_parallel_probe_dedicated** | `servers/task_parallel_probe_dedicated.rs` | Concurrent task probe (`parallel_safe = true`, dedicated runtime) |
| **task_parallel_probe_shared** | `servers/task_parallel_probe_shared.rs` | Concurrent task probe (`parallel_safe = true`, shared runtime) |
| **task_panic_catch** | `servers/task_panic_catch.rs` | Task-augmented panic catching (`catch_in_process_panics`) |
| **task_augmented_client** | `task_augmented_client.rs` | rmcp client + task polling |
| **subprocess_exit_handling** | `servers/subprocess_exit_handling.rs` | Subprocess non-zero exit → MCP `is_error: true` |
| **panic_catch_opt_in** | `servers/panic_catch_opt_in.rs` | In-process panic catching (opt-in), server stays up |
| **client**            | `client.rs`                    | MCP client that exercises the server examples      |

## Async tools and share_runtime

When your CLI has async subcommands (e.g. using `tokio::sleep`, `tokio::spawn`),
do async
work inside your `run` function and call `clap_mcp::run_async_tool` from there.
Configure
`share_runtime` in `#[clap_mcp(...)]`: `false` (default) uses a dedicated thread
per call;
`true` shares the MCP server's tokio runtime. See **async_sleep** and
**async_sleep_shared**
for full examples.
