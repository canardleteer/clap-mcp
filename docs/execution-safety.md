# Execution safety configuration

> Embedder guide for clap-mcp. See [README](../README.md) for getting started.

[← Documentation index](../README.md#documentation)

CLIs differ in how safely they can be invoked over MCP. Two flags control this:

* **`reinvocation_safe`** (default: `false`): Controls whether tool calls spawn
  a fresh subprocess of your binary (`false`) or run in-process via
  `ClapMcpToolExecutor` (`true`). The name refers to whether the CLI's internal
  state can survive repeated invocations without a process restart. Most CLIs
  that don't hold mutable global state can set this to `true`.

* **`parallel_safe`** (default: `false`): Controls whether tool calls are
  serialized behind a tokio `Mutex` (`false`) or dispatched concurrently
  (`true`). Set to `true` only if your CLI logic is safe to run concurrently.

* **`share_runtime`** (default: `false`): When `reinvocation_safe` is true,
  controls how async tool execution runs. See
  [Async tools](#async-tools-and-share_runtime) below.

For shared in-process session state, see [Stateful MCP tools](stateful-tools.md).

## Attribute-based config (recommended)

Use `#[derive(ClapMcp)]` and `#[clap_mcp(...)]` on your CLI type:

```rust
use clap::Parser;
use clap_mcp::ClapMcp;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run"]
#[command(...)]
enum Cli {
    Add {
        #[arg(long)]
        a: i32,
        #[arg(long)]
        b: i32,
    },
    // ...
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::Add { a, b } => (a + b).to_string(),
        // ...
    }
}

let cli = Cli::parse_or_serve_mcp();
```

## Schema metadata: skip and requires

Use `#[clap_mcp(skip)]` to exclude subcommands or arguments from MCP exposure.
Use `#[clap_mcp(requires)]` or `#[clap_mcp(requires = "arg_name")]` to make an
optional
argument required in the MCP tool schema (useful for positional args that may
trigger
stdin behavior when omitted). When the client omits a required arg, a clear
error is returned.

For **optional positional arguments** that might read from stdin when omitted,
prefer an
explicit `#[clap_mcp(requires)]` or `#[clap_mcp(skip)]` so MCP behavior is
intentional.

**Multiple positional scalars:** MCP clients send **named** JSON arguments, but
clap-mcp rebuilds **positional-only** argv for subprocess and in-process tool
calls. Two or more bare scalar positionals on the same variant (non-`Vec`) are
rejected at **compile time** — use `#[arg(long)]` on each field or
`#[clap_mcp(skip)]`. See the **stateful_counter** and **vec_and_flags** examples
for safe patterns.

**Argument-level** (on each field):

```rust
#[derive(Parser, ClapMcp)]
#[clap_mcp_output_from = "run"]
enum Cli {
    Read {
        #[clap_mcp(requires)]  // MCP schema makes path required
        #[arg(long)]
        path: Option<String>,
    },
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::Read { path } => path.unwrap_or_default(),
    }
}
```

**Variant-level** (one or more args; use a single name or comma-separated list —
the MCP schema marks each as required):

```rust
#[derive(Parser, ClapMcp)]
#[clap_mcp_output_from = "run"]
enum Cli {
    // Single optional positional made required in MCP
    #[clap_mcp(requires = "versions")]
    Sort { versions: Option<String> },

    // Multiple optional args
    #[clap_mcp(requires = "path, input")]  // both become required in MCP
    Process {
        #[arg(long)] path: Option<String>,
        #[arg(long)] input: Option<String>,
    },
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::Sort { versions } => format!("{versions:?}"),
        Cli::Process { path, input } => format!("{:?}", (path, input)),
    }
}
```

**Skip:** (subcommands or variant-level) use `#[clap_mcp(skip)]` so a variant is
hidden from MCP; pair with `#[clap_mcp_output_from = "run"]` and a single `run`
for the exposed variants.

```rust
#[derive(Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run"]
enum Cli {
    Public,
    #[clap_mcp(skip)]
    Internal,
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::Public => "ok".to_string(),
        Cli::Internal => "hidden".to_string(),
    }
}
```

You can also use `#[clap_mcp(skip)]` on **root struct fields** so options like
output format are hidden from MCP (they remain available to the CLI):

```rust
#[derive(Parser, ClapMcp)]
#[command(name = "myapp")]
struct Args {
    #[clap_mcp(skip)]
    #[arg(long)]
    out: Option<String>,
    #[command(subcommand)]
    command: Option<Commands>,
}
```

**Imperative:** Use `schema_from_command_with_metadata` and
`get_matches_or_serve_mcp_with_config_and_metadata` with
`ClapMcpSchemaMetadata`:

```rust
let mut metadata = ClapMcpSchemaMetadata::default();
metadata.skip_commands.push("internal".into());
metadata.requires_args.insert("read".into(), vec!["path".into()]);
let schema = schema_from_command_with_metadata(&cmd, &metadata);
```

When the client omits a required argument, the tool returns a clear error:
`"Missing required argument(s): path. The MCP tool schema marks these as
required."`

## Dual derive (root + subcommand)

When you use a **struct root** with `#[command(subcommand)]`, derive `ClapMcp` on
**both** the root struct and the subcommand enum. Put `#[clap_mcp_output_from =
"run"]` and execution config (`#[clap_mcp(...)]`) on the **subcommand** enum only.
The root's derive provides schema metadata and delegates tool execution to the
subcommand's executor. In `main`, parse with `Root::parse_or_serve_mcp()` then
dispatch (`run(cli.command)` when the subcommand field is required, or `match
cli.command` when it is `Option<Commands>`).

Keep **`subcommand_required = true`** and a required `Commands` field when that
is your CLI today — `myapp --mcp` works without switching to `Option`. See
[CLI compatibility](../README.md#cli-compatibility) and **struct_subcommand_required** in
[examples/README.md](../examples/README.md).

**MCP tool list:** The tool list includes the root command and all subcommands.
If your CLI has `subcommand_required = true`, the root command still appears as
a tool but has no subcommand in the MCP invocation model and is rarely used by
clients; the meaningful tools are the subcommands (e.g. explain, compare, sort).
To exclude the root from the tool list when it has subcommands, set
[`ClapMcpSchemaMetadata::skip_root_command_when_subcommands`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapMcpSchemaMetadata.html#structfield.skip_root_command_when_subcommands)
to `true` via the derive with `#[clap_mcp(skip_root_when_subcommands)]` on the
root struct, or imperatively (e.g. implement `ClapMcpSchemaMetadataProvider` for
the root and set the field, or build metadata manually).

## Runtime config

Use `ClapMcpConfig` with `parse_or_serve_mcp_with` or
`get_matches_or_serve_mcp_with_config`:

```rust
clap_mcp::parse_or_serve_mcp_with::<Cli>(clap_mcp::ClapMcpRunOptions {
    config: clap_mcp::ClapMcpConfig {
        reinvocation_safe: true,
        parallel_safe: false,
        ..Default::default()
    },
    serve: Default::default(),
})
```

Tools include `meta.clapMcp` with these hints for clients.

## Async embedders

When your application already uses `#[tokio::main]`, use [`ServeMcpBuilder`]
to run MCP on the **caller's tokio runtime**. Use
[`ServeMcpBuilder::serve_blocking`] from a synchronous `fn main()` (it creates
an internal runtime).

| Entry | Who owns the runtime | Multi-thread when |
|-------|----------------------|-------------------|
| [`ServeMcpBuilder::serve`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ServeMcpBuilder.html#method.serve)().await | Caller's `#[tokio::main]` | Required when [`ClapMcpConfig::needs_multi_thread_runtime`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapMcpConfig.html#method.needs_multi_thread_runtime) is true (`reinvocation_safe` + `share_runtime` or `parallel_safe`) |
| [`ServeMcpBuilder::serve_blocking`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ServeMcpBuilder.html#method.serve_blocking) | clap-mcp (internal runtime) | Handled automatically |
| [`serve_mcp`](https://docs.rs/clap-mcp/latest/clap_mcp/fn.serve_mcp.html) / [`serve_mcp_blocking`](https://docs.rs/clap-mcp/latest/clap_mcp/fn.serve_mcp_blocking.html) | Same as builder | Same as builder (lower-level 7-arg equivalents) |
| `parse_or_serve_mcp*` | Same as blocking | Same as blocking |

```rust
use clap_mcp::{ServeMcpBuilder, McpListen, ClapMcpServeOptions};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), clap_mcp::ClapMcpError> {
    ServeMcpBuilder::for_cli::<Cli>(McpListen::Stdio)
        .serve_options(ClapMcpServeOptions::default())
        .serve()
        .await
}

// Stateful derive CLI:
// ServeMcpBuilder::for_cli_with_state::<Cli, S>(McpListen::Stdio, state).serve().await
```

See [async_embedder_serve](../examples/servers/async_embedder_serve.rs) (imperative
[`ServeMcpBuilder`]) vs [async_sleep_shared](../examples/servers/async_sleep_shared.rs)
(derive + `parse_or_serve_mcp_with`). For HTTP, use `McpListen::Http(addr)` —
see [http.md](http.md).

If [`ClapMcpConfig::needs_multi_thread_runtime`] is true and you call
[`ServeMcpBuilder::serve`] on a `current_thread` runtime, you get
[`ClapMcpError::RequiresMultiThreadRuntime`]; use
`#[tokio::main(flavor = "multi_thread")]` or [`ServeMcpBuilder::serve_blocking`].

## Crash and panic behavior

* **Subprocess (`reinvocation_safe` = false):** If the tool process exits with a
  non-zero status, the server returns a tool result with `is_error: true` and a
  message that includes the exit code (and stderr when non-empty).
* **In-process (`reinvocation_safe` = true), `catch_in_process_panics` = false
  (default):** Any panic in tool code (including from `run_async_tool`) crashes
  the server.
* **In-process, `catch_in_process_panics` = true:** Panics are caught and
  returned as an MCP error; the server stays up. After a caught panic, the
  process may no longer be reinvocation_safe (global state may be corrupted) —
  consider restarting the server. See
  [`ClapMcpConfig::catch_in_process_panics`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapMcpConfig.html#structfield.catch_in_process_panics)
  and the **panic_catch_opt_in** and **subprocess_exit_handling** examples in
  [examples/README.md](../examples/README.md).

## Async tools and share_runtime

When your CLI has async subcommands (e.g. `tokio::sleep`, `tokio::spawn`), do
async work inside your `run` function (e.g. call `clap_mcp::run_async_tool` or
use a runtime handle). This is separate from [Async embedders](#async-embedders)
above: `share_runtime` controls whether **tool bodies** reuse the MCP server's
tokio runtime, not whether MCP itself runs on your app's runtime.
Set `share_runtime` in `#[clap_mcp(...)]` to share the MCP server's tokio
runtime:

| `share_runtime` | Behavior | When to use |
|-----------------|----------|-------------|
| `false` (default) | Dedicated thread with its own tokio runtime per tool call. No nesting. | **Recommended.** Use unless you need deep integration. |
| `true` | Shares the MCP server's tokio runtime. Requires `reinvocation_safe`; uses multi-thread runtime. | Advanced: share runtime state, spawn long-lived tasks, or integrate with other async code. |

**Non-shared (default):** do async work inside your `run` function and call
`clap_mcp::run_async_tool` from there:

```rust
fn run(cmd: Cli) -> AsStructured<SleepResult> {
    match cmd {
        Cli::SleepDemo => AsStructured(
            clap_mcp::run_async_tool(&Cli::clap_mcp_config(), run_sleep_demo).expect("async tool failed"),
        ),
    }
}
```

**Shared runtime:** same pattern; set `share_runtime = true` in
`#[clap_mcp(...)]`.

When MCP task-augmented `tools/call` and logging are both enabled,
`run_async_tool` re-establishes per-task logging context inside `block_on`
(because tokio task-local from the MCP task body does not always propagate into
the nested future under concurrent load). Always route async tool bodies through
`run_async_tool` rather than calling `Handle::block_on` directly. See
[Logging — task-augmented tools](logging.md#task-augmented-tools-and-metataskid).

`share_runtime` only applies when `reinvocation_safe` is true. When tools run
in subprocesses (`reinvocation_safe = false`), `share_runtime` is ignored.

For MCP task-augmented `tools/call`, see [MCP tasks support](mcp-tasks.md).
