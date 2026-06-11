# Execution config

Read when choosing `reinvocation_safe`, `parallel_safe`, `share_runtime`, and topical serialization.

Upstream: [execution-safety.md](../../../../docs/execution-safety.md).

## Config fields

| Field | Default | Meaning |
|-------|---------|---------|
| `reinvocation_safe` | `false` | `false` = subprocess per tool; `true` = in-process `ClapMcpToolExecutor` |
| `parallel_safe` | `false` | `false` = global mutex on all tools; `true` = concurrent unless `serialized` |
| `share_runtime` | `false` | When in-process: shared tokio runtime for async tools via `run_async_tool` |
| `catch_in_process_panics` | `false` | Opt-in: panics in tool code become MCP errors instead of crashing the server |
| `task_augmented_tools` | `false` | MCP tasks — only enable when explicitly requested |

## Recommended default for stateless CLIs

```rust
#[cfg_attr(feature = "mcp", clap_mcp(reinvocation_safe, parallel_safe))]
```

Pure read-only tools with no global init hazards: this is the target shipped config.

## Topical serialization (prefer over global mutex)

When one or few subcommands touch shared mutable state (config file, cache dir, index):

```rust
#[cfg_attr(feature = "mcp", clap_mcp(reinvocation_safe, parallel_safe))]
enum Commands {
    Explain { /* read-only */ },

    #[cfg_attr(feature = "mcp", clap_mcp(serialized))]
    RebuildIndex,

    #[cfg_attr(feature = "mcp", clap_mcp(serialized = "output"))]
    WriteReport {
        #[cfg_attr(feature = "mcp", clap_mcp(serialize_topic))]
        #[arg(long)]
        output: PathBuf,
    },
}
```

Arg-scoped topics serialize only invocations sharing the same MCP JSON value for listed args.

## Async tools

When subcommand handlers are `async` or call async libraries:

1. Keep `reinvocation_safe`.
2. Set `share_runtime = true` when nested runtime creation is costly or incorrect.
3. Use `clap_mcp::run_async_tool` inside `run` — do not call `Handle::block_on` directly from sync `run`.

Examples: **async_sleep**, **async_sleep_shared** in [examples/README.md](../../../../examples/README.md).

Async embedders on `#[tokio::main]` can pass socket or duplex halves via
[`ServeMcpBuilder::stdio_io`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ServeMcpBuilder.html#method.stdio_io)
when MCP does not use process stdin/stdout.

## Panic catching

When in-process tools may panic but the MCP server should survive, add `catch_in_process_panics = true` to `#[clap_mcp(...)]`. See [execution-safety.md — Crash and panic behavior](../../../../docs/execution-safety.md#crash-exit-and-panic-behavior). Examples: **panic_catch**, **task_panic_catch**.

## When to stay subprocess

Keep default (no `reinvocation_safe`) when:

- Tool path calls `std::process::exit` or `exec`
- Non-idempotent global init (`tracing_subscriber::init()` without `try_init`, rustls one-shot)
- Hazards are not quickly fixable

Subprocess still benefits from `#[clap_mcp_output_from]` for schema; structured `structuredContent` requires in-process `AsStructured` or JSON on stdout in the child. See [tool-output.md](../../../../docs/tool-output.md).

## Hazard quick reference

| Hazard | Mitigation |
|--------|------------|
| Double tracing init | `try_init()` or init before serve loop |
| Interactive stdin | `#[clap_mcp(skip)]` or `#[clap_mcp(requires)]` |
| TTY-only branches | Headless path must complete on `tools/call` |
| Shared config dir races | Topical `serialized = "path"` — same as two CLI processes |
| `process::exit` in tool | Subprocess mode, or skip variant |

Document shipped execution config in the project README or integration notes when the team needs an audit trail.

## Probe examples

| Binary | Teaches |
|--------|---------|
| `topical_serial_probe` | Topical locks under concurrent calls |
| `topical_serialization` | Author demo for serialized metadata |
| `task_serial_probe_*` | Task + serialization interaction |
| `subprocess_exit_handling` | Exit codes vs server survival |

Listed in [examples/README.md](../../../../examples/README.md). Do not claim probe PASS without running them or project-equivalent `tools/call` trials.
