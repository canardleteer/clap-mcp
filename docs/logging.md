# Logging and observability

> Guide for CLI authors adding clap-mcp. See [README](../README.md) to get started.

[← Documentation index](../README.md#documentation)

clap-mcp forwards application log messages to MCP clients as
`notifications/message`. Two feature-gated paths are available depending on
your logging ecosystem.

## `tracing` feature

Enable with `features = ["tracing"]`. `ClapMcpTracingLayer` is a standard
[`tracing_subscriber::Layer`](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/layer/trait.Layer.html)
and **composes with any other layers** in your subscriber stack — fmt,
`tracing-opentelemetry`, file appenders, etc. Adding it does not interfere with
your existing tracing pipeline:

```rust
use clap_mcp::logging::{log_channel, ClapMcpTracingLayer};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

let (log_tx, log_rx) = log_channel(32);

tracing_subscriber::registry()
    .with(ClapMcpTracingLayer::new(log_tx))
    .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
    // .with(tracing_opentelemetry::layer().with_tracer(tracer))  // works alongside
    .init();

let mut opts = clap_mcp::ClapMcpServeOptions::default();
opts.log_rx = Some(log_rx);
// Pass opts to parse_or_serve_mcp_with or ServeMcpBuilder::serve_options
```

### Current limitations

* Only the `message` field of each tracing event is forwarded. Other structured
  fields (e.g. `tracing::info!(count = 42, "done")` — `count` is dropped) are
  not yet included.
* Span lifecycle events (`on_new_span`, `on_enter`, `on_close`) are not
  captured.

## `log` feature

Enable with `features = ["log"]`. `ClapMcpLogBridge` implements
[`log::Log`](https://docs.rs/log/latest/log/trait.Log.html) and is installed as
the global logger:

```rust
use clap_mcp::logging::{log_channel, ClapMcpLogBridge};

let (log_tx, log_rx) = log_channel(32);
let bridge = ClapMcpLogBridge::new(log_tx);
log::set_logger(Box::leak(Box::new(bridge))).unwrap();
log::set_max_level(log::LevelFilter::Info);

let mut opts = clap_mcp::ClapMcpServeOptions::default();
opts.log_rx = Some(log_rx);
// Pass opts to parse_or_serve_mcp_with or ServeMcpBuilder::serve_options
```

The `log` crate supports exactly one global logger. Installing `ClapMcpLogBridge`
replaces any existing logger (e.g. `env_logger`, `simplelog`). If you need to
log to both disk and MCP simultaneously, use a multiplexing wrapper (either a
custom `Log` impl that fans out to multiple sinks, or a crate like
[`multi_log`](https://crates.io/crates/multi_log)).

## Task-augmented tools and `meta.taskId`

When MCP task-augmented `tools/call` is enabled (`#[clap_mcp(task_augmented_tools)]`)
and `ClapMcpServeOptions::log_rx` is set, forwarded notifications include
`meta.taskId` in notification extensions for logs emitted during the tool body.
The value matches `CreateTaskResult.task.task_id` for that invocation.

clap-mcp tracks the active task id with [`run_with_mcp_task_id`] (task-local and
thread-local). Behavior by runtime mode:

| Mode | How task id reaches logs |
|------|--------------------------|
| `share_runtime = false` (default) | `run_async_tool` copies the id onto the dedicated async-tool thread via [`McpTaskIdGuard`] |
| `share_runtime = true` | `run_async_tool` captures the id before `Handle::block_on` and re-scopes with `run_with_mcp_task_id` inside the nested future |

The shared-runtime re-scope is required on **all platforms**: tokio task-local
from the outer MCP task body does not always propagate into the future polled by
`block_on`, especially when `parallel_safe = true` and multiple task bodies
overlap. Without the re-scope, logs may arrive without `meta.taskId`.

Use [`run_async_tool`] for async tool bodies; do not call `Handle::block_on`
directly from `run`. See [MCP tasks support](mcp-tasks.md) and
[migration notes](migration-notes.md#task-augmented-toolscall-004-rc1).

[`run_with_mcp_task_id`]: https://docs.rs/clap-mcp/latest/clap_mcp/logging/fn.run_with_mcp_task_id.html
[`McpTaskIdGuard`]: https://docs.rs/clap-mcp/latest/clap_mcp/logging/struct.McpTaskIdGuard.html
[`run_async_tool`]: https://docs.rs/clap-mcp/latest/clap_mcp/fn.run_async_tool.html
