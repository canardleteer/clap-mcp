# Execution safety configuration

> Guide for CLI authors adding clap-mcp. See [README](../README.md) to get started.
> Integration patterns: [Usage patterns](usage.md).

[← Documentation index](../README.md#documentation)

For a CLI layout reference (flat enum, struct root, nesting, skip), see
[Supported CLI shapes](supported-cli-shapes.md).

CLIs differ in how safely they can be invoked over MCP. Three layers control
concurrency and process reuse:

* **`reinvocation_safe`** (default: `false`): Controls whether tool calls spawn
  a fresh subprocess of your binary (`false`) or run in-process via
  `ClapMcpToolExecutor` (`true`). The name refers to whether the CLI's internal
  state can survive repeated invocations without a process restart. Most CLIs
  that don't hold mutable global state can set this to `true`.

* **`parallel_safe`** (default: `false`): When `false`, **every** MCP tool call
  (and task body) is serialized behind one global tokio `Mutex`. That is the
  safe umbrella when you are unsure, but it over-serializes CLIs where only a
  few subcommands need mutual exclusion. When `true`, unmarked tools may overlap;
  combine with [topical serialization](#topical-serialization) to lock only the
  subcommands (or arg topics) that need it.

* **`share_runtime`** (default: `false`): When `reinvocation_safe` is true,
  controls how async tool execution runs. See
  [Async tools](#async-tools-and-share_runtime) below.

For shared in-process session state, see [Stateful MCP tools](stateful-tools.md).

## CLI compatibility details

These rules preserve normal CLI behavior when MCP is added. See also
[README — CLI compatibility](../README.md#cli-compatibility).

### Shell `--` vs MCP passthrough

| Path | How passthrough works |
| --- | --- |
| **Direct CLI (shell)** | clap handles `--` natively; tokens after the first `--` are not parsed as flags. |
| **MCP `tools/call`** | No `--` is inserted. Pass trailing tokens as a JSON **array** on a `Vec<String>` field (often with `#[arg(last = true, allow_hyphen_values = true)]`) or as an explicit `--long` list. |

clap-mcp's pre-clap argv checks (MCP stdio, HTTP, export-skills) inspect only
tokens **before** the first standalone `"--"`. So `myapp run -- --mcp` does not
start MCP. `--mcp` is passthrough to a child.

`build_tool_argv` rebuilds named JSON into argv for tool execution. For trailing
multi-value positionals (`num_args(1..)` / cargo-style `last` vecs), it inserts
`--` before the trailing tokens. For hyphen-prefixed tokens, prefer an explicit
`#[arg(long)] args: Vec<String>` or `allow_hyphen_values = true` on the trailing
field. Examples: **passthrough_args**, **vec_and_flags** in
[examples/README.md](../examples/README.md).

### Renaming clap-mcp builtin flags

If your app already uses `--mcp` for something else, rename clap-mcp's stdio,
HTTP, and export-skills flags via derive attributes or
[`ClapMcpBuiltinFlags`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapMcpBuiltinFlags.html):

| Builtin | Default long | Derive attr | Stable clap arg id |
| --- | --- | --- | --- |
| stdio MCP | `--mcp` | `mcp_flag = "…"` | `CLAP_MCP_STDIO_FLAG_ID` |
| HTTP MCP | `--mcp-http` | `mcp_http_flag = "…"` | `CLAP_MCP_HTTP_FLAG_ID` |
| export skills | `--export-skills` | `export_skills_flag = "…"` | `CLAP_MCP_EXPORT_SKILLS_FLAG_ID` |

Imperative helpers: `command_with_mcp_flag_with_flags(cmd, &flags)` (and
export-skills / HTTP variants). Example: **custom_mcp_flags** in
[examples/README.md](../examples/README.md).

## Subprocess defaults (minimal derive)

Default config: subprocess execution (`reinvocation_safe = false`), serialized
tool calls (`parallel_safe = false`). No `#[clap_mcp(...)]` needed.

Compilable example: [Usage — Derive (minimal)](usage.md#derive-minimal).

## Attribute-based config (recommended)

Use `#[derive(ClapMcp)]` and `#[clap_mcp(...)]` on your CLI type for in-process
tool execution.

Compilable example: [Usage — Derive with attributes](usage.md#derive-with-attributes-recommended).

> [!NOTE]
> With `reinvocation_safe`, tool calls run in-process. `parallel_safe = false`
> serializes **all** tools (heavy hammer). Prefer `parallel_safe = true` plus
> `#[clap_mcp(serialized)]` on the few subcommands that need exclusion when your
> CLI is mostly concurrent-safe.

## Topical serialization

When most of your CLI is safe to run concurrently but a few subcommands need
mutual exclusion, set `parallel_safe = true` globally and mark only those
subcommands with `#[clap_mcp(serialized)]` or `#[clap_mcp(serialized = "arg")]`.
That sculpts the global umbrella into per-tool (or per-arg) topics instead of
forcing everything through one lock.

| Declaration | Lock topic |
| --- | --- |
| `#[clap_mcp(serialized)]` on a variant | All invocations of that tool serialize together |
| `#[clap_mcp(serialized = "output")]` | Invocations with the same MCP `output` value serialize; different values may overlap |
| `#[clap_mcp(serialized = "region, bucket")]` | Composite topic from all listed arg ids (sorted when building the key) |

**Specificity:** less specific declaration or invocation → wider lock. For
arg-scoped serialization, if a listed arg is omitted or null in the MCP request,
clap-mcp falls back to the tool-wide topic for that tool (same as `serialized`
without args).

Topical serialization applies only when `parallel_safe = true`. When
`parallel_safe = false`, the global mutex still serializes all tools (topical
metadata is ignored).

```rust
#[derive(Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = true)]
#[clap_mcp_output_from = "run"]
enum Cli {
    Search { #[arg(long)] query: String },

    /// All flush invocations serialize with each other.
    #[clap_mcp(serialized)]
    FlushAll,

    /// Only flushes targeting the same output path serialize together.
    #[clap_mcp(serialized = "output")]
    Flush {
        #[clap_mcp(serialize_topic)]
        #[arg(long)]
        output: String,
    },
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::Search { query } => format!("search: {query}"),
        Cli::FlushAll => "flushed".into(),
        Cli::Flush { output } => format!("flushed to {output}"),
    }
}
```

Imperative metadata: set [`ClapMcpSchemaMetadata::serialize_tools`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapMcpSchemaMetadata.html#structfield.serialize_tools)
with [`ClapMcpSerializeScope::Tool`](https://docs.rs/clap-mcp/latest/clap_mcp/enum.ClapMcpSerializeScope.html)
or `ClapMcpSerializeScope::Args(vec![...])`. Marked tools expose
`meta.clapMcp.serialized`, `serializeScope`, optional `serializeArgs`, and
`serializeTopicArgs` on `list_tools`.

### Default topic keys (no Rust traits required)

Arg-scoped serialization uses **canonical MCP JSON** for lock keys. Your Rust arg
types do not need `Hash`, `Eq`, or any clap-mcp trait. clap-mcp builds keys from
the raw `tools/call` argument map before clap parses your enum. That keeps the
common path declarative and subprocess-friendly.

Canonicalization is best-effort for MCP dispatch, not a guarantee of equivalence
after parsing (for example `"1"` vs `1` may differ). Prefer consistent MCP client
types, or use typed topics below when parsed identity matters.

### Optional typed topics (`Hash` / `Eq` / serde)

When you **want** parsed-type semantics and accept the extra complexity, mark the
field with `#[clap_mcp(serialize_topic)]` (the arg must appear in
`serialized = "..."` on the same variant). clap-mcp calls
[`ClapMcpSerializeTopic::serialize_topic_segment`] for that arg before falling
back to JSON canonicalization.

Helpers (opt-in per type):

```rust
use clap_mcp::{ClapMcpSerializeTopic, impl_serialize_topic_hash_eq, impl_serialize_topic_serde_eq};
use serde::{Deserialize, Serialize};

#[derive(Hash, Eq, PartialEq, Deserialize)]
struct ShardId(u32);
impl_serialize_topic_hash_eq!(ShardId);

#[derive(Serialize, Deserialize, Eq, PartialEq)]
struct OutputPath(String);
impl_serialize_topic_serde_eq!(OutputPath);
```

`String` and common scalars already implement `ClapMcpSerializeTopic` via
`impl_serialize_topic_serde_eq!`. Imperative servers can set
[`ClapMcpSchemaMetadata::serialize_topic_args`] with function pointers directly.

### Documented guidance (recommended use)

* Use **arg-scoped** serialization on identity-like args: paths, resource names,
  tenant ids, shard keys.
* Use **tool-wide** `serialized` when the whole subcommand touches shared mutable
  state and per-arg splitting is not worth the complexity.
* Do **not** use arg-scoped serialization on large blobs, nested config objects,
  or values whose JSON form may not match your CLI's parsed equality. Prefer
  tool-wide `serialized` or lock inside `run()`.
* Pair with `#[clap_mcp(requires = "output")]` when omitting a scoped arg is
  invalid for your CLI anyway.

### Complexity balance

clap-mcp deliberately splits the problem into three levels so the derive surface
stays small:

| Level | Mechanism | Complexity | When |
| --- | --- | --- | --- |
| Global | `parallel_safe = false` | Lowest config | Unsure; serialize everything |
| Topical (JSON) | `parallel_safe = true` + `serialized` / `serialized = "arg"` | Low | ~95% case; identity-like args |
| Typed topic | `serialize_topic` + `ClapMcpSerializeTopic` | Medium | Parsed `Hash`/`Eq`/serde identity |
| In-process locks | `Mutex` in `run()` / [stateful tools](stateful-tools.md) | Highest | Cross-tool groups, custom rules |

The attribute layer is **documented guidance**, not a validated concurrency
framework. JSON topical keys do not promise post-parse equality. Typed topics
extend that when you implement the trait (or use the macros) and accept parse +
key maintenance. Anything beyond that belongs in your tool code, not more
clap-mcp attributes.

> [!TIP]
> Topical serialization covers the common case: mark a few subcommands, optionally
> keyed by a simple arg. Use `serialize_topic` when parsed-type identity matters.
> If you need cross-command lock groups or ordering, lock inside your tool code
> instead of extending the derive surface. See [Stateful MCP tools](stateful-tools.md).

Runnable examples: **topical_serialization** (author demo) and **topical_serial_probe**
(integration probe) in [examples/README.md](../examples/README.md).

## Schema metadata (skip and requires)

Use `#[clap_mcp(skip)]` to exclude subcommands or arguments from MCP exposure.
Use `#[clap_mcp(requires)]` or `#[clap_mcp(requires = "arg_name")]` to make an
optional argument required in the MCP tool schema (useful for positional args
that may trigger stdin behavior when omitted). When the client omits a required
arg, clap-mcp returns a clear error.

For **optional positional arguments** that might read from stdin when omitted,
prefer an
explicit `#[clap_mcp(requires)]` or `#[clap_mcp(skip)]` so MCP behavior is
intentional.

**Multiple positional scalars:** MCP clients send **named** JSON arguments, but
clap-mcp rebuilds **positional-only** argv for subprocess and in-process tool
calls. Two or more bare scalar positionals on the same variant (non-`Vec`) are
rejected at **compile time** — use `#[arg(long)]` on each field or
`#[clap_mcp(skip)]` on the variant (skipped variants are exempt from this
guard; you do not need to reshape fields MCP never exposes). See the
**stateful_counter** and **vec_and_flags** examples for safe patterns.

**Argument-level** (on each field; excerpt):

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

**Variant-level** (one or more args; excerpt):

```rust
#[derive(Parser, ClapMcp)]
#[clap_mcp_output_from = "run"]
enum Cli {
    #[clap_mcp(requires = "versions")]
    Sort { versions: Option<String> },

    #[clap_mcp(requires = "path, input")]
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
use clap::Parser;
use clap_mcp::ClapMcp;

#[derive(Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe)]
#[clap_mcp_output_from = "run"]
#[command(name = "myapp")]
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

fn main() {
    let cli = Cli::parse_or_serve_mcp();
    println!("{}", run(cli));
}
```

You can also use `#[clap_mcp(skip)]` on **root struct fields** so options like
output format are hidden from MCP (they remain available to the CLI). Excerpt:

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
`ClapMcpSchemaMetadata`. Excerpt:

```rust
let mut metadata = ClapMcpSchemaMetadata::default();
metadata.skip_commands.push("internal".into());
metadata.requires_args.insert("read".into(), vec!["path".into()]);
let schema = schema_from_command_with_metadata(&cmd, &metadata);
```

When the client omits a required argument, the tool returns a clear error:
`"Missing required argument(s): path. The MCP tool schema marks these as
required."`

### `hide` vs `#[clap_mcp(skip)]`

clap's `#[command(hide = true)]` affects help text only. MCP tool exposure is
controlled by `#[clap_mcp(skip)]`. Hidden subcommands still appear in
`tools/list` unless you skip them explicitly. `skip` and clap `hide` are
different dimensions and are not correlated in clap-mcp: tying them would
complicate the API and silently change MCP tool lists for embedders who use
`hide` for operator UX without intending an agent policy change. To omit a
command from MCP, use `#[clap_mcp(skip)]` explicitly.

### Optional args that trigger interactive fallback

| CLI behavior when arg omitted | MCP attribute |
| --- | --- |
| Reads stdin, opens a prompt, or picks a "last used" default | `#[clap_mcp(requires)]` or `#[clap_mcp(requires = "arg")]` |
| Should not be callable without the arg | `#[clap_mcp(requires)]` |
| Interactive-only; no JSON-safe default | `#[clap_mcp(skip)]` on the subcommand |

`Option<T>` with `#[arg(long)]` is common for human-friendly CLIs. MCP clients
send JSON; omitting the key is not the same as omitting a flag in the shell.

### Struct root with globals

When the derive root is a struct with `#[clap_mcp_output_from]` and nested
`#[clap_mcp(schema_only)]` enums, tool execution receives the full parsed root
(including global flags). Leaf MCP tool `inputSchema` values also include
ancestor `#[arg(global)]` properties so clients can pass globals in
`tools/call` JSON. See [supported CLI shapes](supported-cli-shapes.md) and the
`struct_subcommand_globals` example.

### Nested subcommand metadata

The derive deep-merges `ClapMcpSchemaMetadata` from nested `#[command(subcommand)]`
enum fields into each ancestor. Skips, requires, task markers, and topical
serialization attrs on inner enums propagate to the root schema without a manual
merge function. For imperative servers, use
[`ClapMcpSchemaMetadata::merge_from`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapMcpSchemaMetadata.html#method.merge_from)
the same way.

## Dual derive — root and subcommand

When your CLI has a **struct root** with `#[command(subcommand)]`, derive
`ClapMcp` on **both** the root struct and the subcommand enum:

* **`#[clap_mcp(...)]`** on the **root struct** — execution config for
  `Root::parse_or_serve_mcp()` (`Root::clap_mcp_config()`)
* **`#[clap_mcp_output_from = "run"]`** on the **subcommand enum** — tool bodies
  when the subcommand enum alone is enough (no root-only fields in `run`)
* **`#[clap_mcp_output_from = "run_cli"]`** on the **root struct** — when `run`
  must see global flags or other root fields; pass the full parsed root type.
  Mark nested subcommand enums with `#[clap_mcp(schema_only)]` when they only
  contribute schema metadata. See **struct_subcommand_globals** in
  [examples/README.md](../examples/README.md).

By default the root delegates MCP tool execution to the subcommand's `run`.
With `#[clap_mcp_output_from]` on the struct, `run` receives the full root.
Same `Add`
tool as [Usage — Derive with attributes](usage.md#derive-with-attributes-recommended),
with a required subcommand field:

```rust
use clap::{Parser, Subcommand};
use clap_mcp::ClapMcp;

#[derive(Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe)]
#[command(name = "myapp", subcommand_required = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, ClapMcp)]
#[clap_mcp_output_from = "run"]
enum Commands {
    Add {
        #[arg(long)]
        a: i32,
        #[arg(long)]
        b: i32,
    },
}

fn run(cmd: Commands) -> String {
    match cmd {
        Commands::Add { a, b } => (a + b).to_string(),
    }
}

fn main() {
    let cli = Cli::parse_or_serve_mcp();
    println!("{}", run(cli.command));
}
```

Keep **`subcommand_required = true`** and a required `Commands` field when that
is your CLI today — `myapp --mcp` works without switching to `Option`. Bare
`myapp` still fails with clap's missing-subcommand error; `myapp add --a 1 --b 2`
and `myapp --mcp` both work. See
[CLI compatibility](../README.md#cli-compatibility) and
**struct_subcommand_required** in [examples/README.md](../examples/README.md).

**MCP tool list:** The tool list includes the root command and all subcommands.
If your CLI has `subcommand_required = true`, the root command still appears as
a tool but has no subcommand in the MCP invocation model and is rarely used by
clients; the meaningful tools are the subcommands (e.g. `add`, `greet`).
To exclude the root from the tool list when it has subcommands, set
[`ClapMcpSchemaMetadata::skip_root_command_when_subcommands`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapMcpSchemaMetadata.html#structfield.skip_root_command_when_subcommands)
to `true` via the derive with `#[clap_mcp(skip_root_when_subcommands)]` on the
root struct, or imperatively (e.g. implement `ClapMcpSchemaMetadataProvider` for
the root and set the field, or build metadata manually).

**Nested enums (schema only):** When a struct root or ancestor enum owns tool
execution (manual `ClapMcpToolExecutor` or `#[clap_mcp_output_from]` on the
executor type), intermediate subcommand enums can use `#[clap_mcp(schema_only)]`
instead of a dead `#[clap_mcp_output_from]` stub. The derive emits
`ClapMcpSchemaMetadataProvider` only; skip, requires, task, and serialization
attrs still apply and merge into ancestor metadata. See **nested_subcommands** in
[examples/README.md](../examples/README.md).

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

## Interactive and session commands

MCP `tools/call` is request/response. It does not provide a bidirectional TTY,
SSH session, or long-lived pipe. Subcommands that attach to a terminal, open an
interactive REPL, or replace the process (`exec`, `process::exit`) are poor MCP
tools.

Mark them with `#[clap_mcp(skip)]` and document the shell invocation for
operators. Examples: `connect`, `ssh`, `shell`, `completions`, blocking
forwarders that never return.

## Crash, exit, and panic behavior

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
* **In-process `std::process::exit` or `exec`:** The MCP server process
  terminates. `catch_in_process_panics` only intercepts unwinding panics, not
  process replacement or explicit exit.
* **TTY / session tool paths:** Interactive attach, blocking `--wait`, or
  replace-process flows have the same in-process hazard when
  `reinvocation_safe` is true.

| Hazard | In-process effect | Typical mitigation |
| --- | --- | --- |
| `process::exit` / non-zero exit in tool code | MCP server process ends | `#[clap_mcp(skip)]`, refactor to `Result`, or subprocess mode |
| `exec` / replace-process | MCP server replaced or ends | `#[clap_mcp(skip)]` or subprocess-only |
| Blocking interactive / TTY session | Hangs or kills server context | `#[clap_mcp(skip)]`; document shell invocation |
| Unwinding panic | Server crash (default) or caught error (`catch_in_process_panics`) | Opt-in `catch_in_process_panics`; restart after corruption |

> [!WARNING]
> `catch_in_process_panics` does **not** intercept `std::process::exit` or
> `exec`. In-process tool code that terminates the process kills the MCP server.
> Skip those subcommands, run them in subprocess mode only, or refactor them to
> return `Result` instead of exiting.

## Arg groups

clap `ArgGroup` rules (exactly one of several flags) are enforced at argv parse
time. MCP tool JSON Schema lists arguments independently; clap-mcp does not emit
JSON Schema shapes derived from ArgGroup graphs. That would require brittle
introspection of clap internals and would not reliably cover all group semantics
across clap versions. Invalid combinations fail when clap parses the rebuilt
argv. Document exclusivity in tool descriptions when agents need hints.

## Cross-tool serialization

Topical `#[clap_mcp(serialized)]` locks are keyed per tool name (and optional
args). Tools that must exclude each other but are not the same MCP tool name
(for example `forward start` and `forward stop`) need coordination in your
code: a shared `Mutex` in `run()`, [stateful tools](stateful-tools.md), or
`parallel_safe = false` when conservative serialization is acceptable. clap-mcp
does not provide a cross-tool lock-group attribute.

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
