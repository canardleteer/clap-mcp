# MCP tasks support

> Guide for CLI authors adding clap-mcp. See [README](../README.md) to get started.

[← Documentation index](../README.md#documentation)

clap-mcp supports MCP tasks for server-side, task-augmented `tools/call`.
Clients send `tools/call` with `task` metadata, receive a
[`CreateTaskResult`](https://docs.rs/rmcp/latest/rmcp/model/task/struct.CreateTaskResult.html)
immediately, and poll `tasks/get` / `tasks/result` for completion. See the
[MCP tasks specification](https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/tasks).

Requires `reinvocation_safe = true` (in-process execution). Subprocess mode
(`reinvocation_safe = false`) does not support task-augmented calls.

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

## Supported matrix (this release)

| Configuration | Task-augmented `tools/call` |
| --- | --- |
| `reinvocation_safe = true` | Supported |
| `share_runtime = false` or `true` | Supported |
| `parallel_safe = false` or `true` | Supported |
| `catch_in_process_panics = true` | Supported (panics map to task error payloads; server stays up) |
| `reinvocation_safe = false` (subprocess) | Not supported |

When `parallel_safe = false`, task-augmented and plain `tools/call` share one
serialization queue. When `parallel_safe = true`, tool bodies may overlap;
logging uses per-task context so `meta.taskId` stays correct.

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

Client (spawns the server, sends `task: Some(...)`, polls `tasks/get` and
`tasks/result`):

```shell
cargo run -p clap-mcp-examples --bin task_augmented_client --features tracing -- task_tools_dedicated
cargo run -p clap-mcp-examples --bin task_augmented_client --features tracing -- task_tools_shared
```

Additional examples: [task_panic_catch](../examples/servers/task_panic_catch.rs)
(panic-in-task with `catch_in_process_panics`). See
[examples/README.md](../examples/README.md) for integration-test probes
(`task_serial_probe_*`, `task_parallel_probe_*`).

## Stack and migration

Pinned stack (review on bump): workspace [`rmcp`](https://docs.rs/rmcp) 1.7.x
(see root `Cargo.toml`; features `server`, `client`, `macros`, `transport-io`,
`transport-child-process`). Protocol 2025-11-25 (tasks). Migration notes:
[migration-notes.md](migration-notes.md).

## Client-side task routing (not yet supported)

> [!NOTE]
> Server-side task-augmented `tools/call` is shipped in clap-mcp on rmcp 1.7.x.
> That is distinct from client-side task routing, where the MCP server polls the
> client via `tasks/*` on `ClientHandler`. Client-side routing is not in rmcp
> 1.7.0; track [rust-sdk PR #816](https://github.com/modelcontextprotocol/rust-sdk/pull/816).

When #816 lands in a released rmcp version:

* Implement `ClientHandler::{list_tasks, get_task_info, get_task_result,
  delete_task}` in test/client utilities
* Add an example pairing task-augmented server requests with async client
  completion
* Document `capabilities.tasks` negotiation on the client
