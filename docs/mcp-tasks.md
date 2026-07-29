# MCP tasks support

> Guide for CLI authors adding clap-mcp. See [README](../README.md) to get started.

[← Documentation index](../README.md#documentation)

clap-mcp supports MCP tasks for server-side, task-augmented `tools/call` via the
[SEP-2663 tasks extension](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2663).
When the client declares the tasks extension in `initialize`, eligible tools
return a
[`CreateTaskResult`](https://docs.rs/rmcp/latest/rmcp/model/task/struct.CreateTaskResult.html)
immediately; the client polls `tasks/get` until the task completes and reads the
tool result from the completed task payload. See the
[MCP tasks specification](https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/tasks)
and the [2026-07-28 release](https://modelcontextprotocol.io/specification/2026-07-28/).

Requires `reinvocation_safe = true` (in-process execution). Subprocess mode
(`reinvocation_safe = false`) does not support task-augmented calls.

## Task support matrix

Workspace [`rmcp`](https://docs.rs/rmcp) is pinned at **3.x** (root
[`Cargo.toml`](../Cargo.toml)). clap-mcp targets **CLI binaries as MCP servers**;
the matrix below uses MCP method names and initiator → receiver flow.
Implementation detail (derive attrs, examples, transports) is in
[clap-mcp task implementation](#clap-mcp-task-implementation).

| MCP method / concept | Initiator → receiver | clap-mcp | rmcp (workspace) | Upstream |
| --- | --- | --- | --- | --- |
| SEP-2663 tasks extension on client | Client → server | Required for task enqueue (examples declare `ClientCapabilities::enable_tasks`) | **3.0+** | [SEP-2663](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2663) |
| Task-eligible `tools/call` → `CreateTaskResult` (server-directed) | Client → server (your CLI) | Shipped (server) | **3.0+** | No client `task` hint on `CallToolRequestParams` |
| `tasks/get` (status + completed payload) | Client → server | Shipped (server answers) | **3.0+** | Replaces separate `tasks/result` poll from rmcp 2.x |
| `tasks/cancel` | Client → server | Shipped (rmcp handler; no clap-mcp override) | **3.0+** | — |
| `tasks/list` | Client → server | Shipped (rmcp default `list_tasks`) | **3.0+** | — |
| `Tool.execution.taskSupport` in `list_tools` | Server advertises per tool | Not used (SEP-2663 server-directed model) | Removed in **3.0** | Was rmcp 2.x optional/required hints |
| Client `CallToolRequestParams::with_task` / `TaskMetadata` | Client → server | Not used | Removed in **3.0** | Server decides enqueue from extension + tool eligibility |
| `tasks/result` RPC | Client → server | Not used | Removed in **3.0** | Completed tool output lives in `tasks/get` payload |
| `capabilities.tasks` on server | Server capability negotiation | Shipped when `task_augmented_tools` is on | **3.0+** | — |
| Client poll sequence (`tools/call` + `tasks/get`) | MCP client → your CLI server | Examples only (not a clap-mcp crate feature) | **3.0+** | [task_augmented_client](../examples/task_augmented_client.rs) |
| Task-augmented `sampling/createMessage` | Server → client | Not offered | Not in **3.0** | [PR #816](https://github.com/modelcontextprotocol/rust-sdk/pull/816) |
| Task-augmented `elicitation/create` | Server → client | Not offered | Not in **3.0** | Same as #816 |
| `tasks/get` / `tasks/cancel` / `tasks/list` on **client** | Server → client | Not offered (CLI server role) | Not in **3.0** | #816 adds `ClientHandler` receive path |

### clap-mcp task implementation

| Area | Status | Execution constraints | Surface |
| --- | --- | --- | --- |
| Enable server task-augmented `tools/call` | Shipped | [`reinvocation_safe`](execution-safety.md) required (`task_augmented_tools` compile error otherwise) | `#[clap_mcp(task_augmented_tools)]`; optional `#[clap_mcp(task)]` per subcommand |
| Server-directed enqueue (SEP-2663) | Shipped | Client must declare tasks extension; eligible tools return `CreateTaskResult` without a client task hint | `meta.clapMcp.taskAugmented` on eligible tools |
| `TaskSupport` / per-tool `taskSupport` in `list_tools` | Not used | SEP-2663 removed client hints and `Tool.execution.taskSupport` | Derive marks eligibility via `task_augmented_tools` / `#[clap_mcp(task)]` only |
| Subprocess MCP + tasks | Not supported | `reinvocation_safe = false` | Compile error without `reinvocation_safe` |
| stdio transport (`--mcp`) | Shipped | `reinvocation_safe` required | `parse_or_serve_mcp*`; **task_tools_*** examples |
| HTTP transport (`--mcp-http`) | Shipped | `reinvocation_safe` required | `http` feature; **task_tools_http** |
| Concurrent task + plain `tools/call` | Shipped | [`parallel_safe`](execution-safety.md) false serializes all tools; true allows overlap | See probes **task_serial_probe_***, **task_parallel_probe_*** |
| Async task tool bodies | Shipped | [`share_runtime`](execution-safety.md#async-tools-and-share_runtime) false (default) or true; use [`run_async_tool`](https://docs.rs/clap-mcp/latest/clap_mcp/fn.run_async_tool.html) for async `run` | **task_tools_dedicated** vs **task_tools_shared** |
| `meta.taskId` on log notifications during task bodies | Shipped | `reinvocation_safe`; with `share_runtime = true`, call bodies via `run_async_tool` so task id propagates under `block_on` | `ClapMcpServeOptions::log_rx` + `tracing`/`log`; [logging](logging.md#task-augmented-tools-and-metataskid) |
| Panics in task-scheduled work | Shipped | `reinvocation_safe`; opt-in [`catch_in_process_panics`](execution-safety.md#crash-exit-and-panic-behavior) | **task_panic_catch** |
| MCP client calling your task tools | Shipped (examples/tests) | Server side must enable tasks (in-process) | **task_augmented_client**; `clap-mcp/tests/task_augmented_tests.rs` |
| Server→client task routing on `ClientHandler` | Not planned | — | Full MCP client apps use rmcp directly after #816 |

## Enable on your CLI

Use `#[clap_mcp(task_augmented_tools)]` on your CLI root (enum or struct with
nested subcommands). Compile error if combined with `reinvocation_safe = false`.
On a struct root with `#[clap_mcp(schema_only)]` nested enums, the flag on the
root struct applies without requiring root-field skip or requires attrs to force
metadata merge. Optionally mark individual subcommands with `#[clap_mcp(task)]`
so only those tools advertise `meta.clapMcp.taskAugmented` and accept
task-augmented calls. When no variant has `#[clap_mcp(task)]`, every tool is
eligible while `task_augmented_tools` is on.

```rust
use clap::Parser;
use clap_mcp::ClapMcp;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, task_augmented_tools)]
#[clap_mcp_output_from = "run"]
#[command(name = "myapp", subcommand_required = false)]
enum Cli {
    /// Long-running work eligible for task-augmented tools/call.
    #[clap_mcp(task)]
    Sleep {
        #[arg(long, default_value_t = 50)]
        ms: u64,
    },
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::Sleep { ms } => {
            clap_mcp::run_async_tool(&Cli::clap_mcp_config(), move || async move {
                tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
                format!("slept {ms}ms")
            })
            .unwrap_or_else(|e| format!("async error: {e}"))
        }
    }
}

fn main() {
    let cli = Cli::parse_or_serve_mcp();
    println!("{}", run(cli));
}
```

Add `tokio` to `Cargo.toml` when using `tokio::time` (see
[task_tools_dedicated](../examples/servers/task_tools_dedicated.rs)).

Async tool bodies use `clap_mcp::run_async_tool` (see
[Execution safety — Async tools](execution-safety.md#async-tools-and-share_runtime)).
`share_runtime` defaults to `false` (dedicated runtime per call); set
`share_runtime = true` to reuse the MCP server runtime when appropriate.

When `ClapMcpServeOptions::log_rx` is set and you use the `tracing` or `log`
bridges, log notifications emitted during a task-augmented tool body include
`meta.taskId` in notification extensions, matching
`CreateTaskResult.task.task_id` (including when multiple task bodies run
concurrently). With `share_runtime = true`, call async tool bodies through
[`run_async_tool`](https://docs.rs/clap-mcp/latest/clap_mcp/fn.run_async_tool.html)
so clap-mcp can re-install per-task logging context inside `block_on` (see
[Logging — task-augmented tools](logging.md#task-augmented-tools-and-metataskid)).

## Server and client example

Full sources: [task_tools_dedicated](../examples/servers/task_tools_dedicated.rs)
(dedicated async runtime), [task_tools_shared](../examples/servers/task_tools_shared.rs)
(shared MCP runtime), [task_augmented_client](../examples/task_augmented_client.rs)
(rmcp client with polling).

Server (`tracing` feature required for async sleep demo):

```shell
cargo run -p clap-mcp-examples --bin task_tools_dedicated --features tracing -- sleep --ms 80
cargo run -p clap-mcp-examples --bin task_tools_dedicated --features tracing -- --mcp
```

Client (spawns the server, declares the tasks extension, polls `tasks/get` for
completion):

```shell
cargo run -p clap-mcp-examples --bin task_augmented_client --features tracing -- task_tools_dedicated
cargo run -p clap-mcp-examples --bin task_augmented_client --features tracing -- task_tools_shared
```

Additional examples: [task_panic_catch](../examples/servers/task_panic_catch.rs)
(panic-in-task with `catch_in_process_panics`). See
[examples/README.md](../examples/README.md) for integration-test probes
(`task_serial_probe_*`, `task_parallel_probe_*`).

## Stack and migration

Workspace `rmcp` features: `server`, `client`, `macros`, `transport-io`,
`transport-child-process` (see root [`Cargo.toml`](../Cargo.toml)). Broader API
renames and rmcp port notes: [migration-notes.md](migration-notes.md).

Maintainers: when bumping workspace `rmcp`, see
[AGENTS.md — When bumping workspace rmcp](../AGENTS.md#when-bumping-workspace-rmcp).
