# clap-mcp

> **Enrich your Rust CLI with MCP Capabilities**

[![crates.io](https://img.shields.io/crates/v/clap-mcp.svg)](https://crates.io/crates/clap-mcp)
[![docs.rs](https://docs.rs/clap-mcp/badge.svg)](https://docs.rs/clap-mcp)
[![crates.io (clap-mcp-macros)](https://img.shields.io/crates/v/clap-mcp-macros.svg)](https://crates.io/crates/clap-mcp-macros)
[![docs.rs (clap-mcp-macros)](https://docs.rs/clap-mcp-macros/badge.svg)](https://docs.rs/clap-mcp-macros)

## Usage

You can take a look at the examples, but this is a **VERY** early draft. See
[examples/README.md](examples/README.md) for detailed instructions on running them.

## Development

Run all tests (including feature-gated logging tests):

```bash
cargo test --all-features
```

## Design

Compared to a Command Line Interface, I'm not a huge fan of the [Model Context
Protocol](https://modelcontextprotocol.io/docs/getting-started/intro), but my
feelings don't represent real world usage patterns. I feel MCP would do better
with gRPC and Protobuf as it's "transport." All that being said, I'm not bitter
about it, so I'm just letting a model do the development work and deal with it's
own self-generated mess.

**The intent is generally:**

- Make it easy to add a MCP server to current Rust CLIs that use `clap`.
- Have it work well enough and provide enough guardrails to cover the 95% case.
- If there is structured information available from the CLI as an outcome, we
  should provide a way to express it naturally via MCP.
- Provide a way to express structured logging information (if available) as part
  of the response if requested.

Overall, the more you design your CLI around a service pattern, the more
naturally this crate will behave as an MCP server, and modern CLIs often do
that. At the same time, we shouldn't force CLIs that don't do that, out of the
ecosystem.

## Quick start

Add `clap-mcp` and `clap-mcp-macros` to your `Cargo.toml`:

```toml
[dependencies]
clap-mcp = "0.0.1-rc.5"
clap-mcp-macros = "0.0.1-rc.5"
```

For derive usage, `use clap_mcp_macros::ClapMcp` for brevity so you can write `#[derive(ClapMcp)]`.

### Imperative (existing clap CLI)

If you already have a `clap::Command`-based CLI, you can add MCP support in one
line. When `--mcp` is not passed, your CLI works exactly as before:

```rust
use clap::Command;

fn main() {
    let cmd = Command::new("myapp")
        .subcommand(Command::new("hello").about("Say hello"));

    let matches = clap_mcp::get_matches_or_serve_mcp(cmd);
    // If we reach here, --mcp was not passed — normal CLI execution continues.
}
```

### Derive (minimal)

With `#[derive(ClapMcp)]`, each subcommand is automatically exposed as an MCP
tool. This uses default config (subprocess execution, serialized tool calls):

```rust
use clap::Parser;
use clap_mcp_macros::ClapMcp;

#[derive(Debug, Parser, ClapMcp)]
#[command(name = "myapp")]
enum Cli {
    /// Say hello.
    #[clap_mcp_output = "format!(\"Hello, {}!\", clap_mcp::opt_str(&name, \"world\"))"]
    Greet {
        #[arg(long)]
        name: Option<String>,
    },
}

fn main() {
    let cli = clap_mcp::parse_or_serve_mcp::<Cli>();
    // Normal CLI logic here...
}
```

### Derive with attributes (recommended)

Use `#[clap_mcp(...)]` to declare execution safety, and
`parse_or_serve_mcp_attr` to pick up that config automatically:

```rust
use clap_mcp_macros::ClapMcp;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(parallel_safe = false, reinvocation_safe)]
#[command(name = "myapp")]
enum Cli {
    #[clap_mcp_output = "format!(\"{}\", a + b)"]
    Add { a: i32, b: i32 },
}

let cli = clap_mcp::parse_or_serve_mcp_attr::<Cli>();
```

## Feature flags

| Flag | Enables |
| --- | --- |
| `tracing` | `ClapMcpTracingLayer` — a `tracing_subscriber::Layer` that forwards tracing events to MCP clients via `notifications/message`. |
| `log` | `ClapMcpLogBridge` — a `log::Log` implementation that forwards `log` crate messages to MCP clients. |
| `output-schema` | `schemars`-based JSON schema generation for structured tool output. Enables [`output_schema_for_type`], [`output_schema_one_of!`], and `#[clap_mcp_output_type]` / `#[clap_mcp_output_one_of]` to set each tool's `output_schema` for MCP clients. |

Enable features in `Cargo.toml`:

```toml
[dependencies]
clap-mcp = { version = "0.0.1-rc.5", features = ["tracing"] }
```

## Execution safety configuration

CLIs differ in how safely they can be invoked over MCP. Two flags control this:

- **`reinvocation_safe`** (default: `false`): Controls whether tool calls spawn
  a fresh subprocess of your binary (`false`) or run in-process via
  `ClapMcpToolExecutor` (`true`). The name refers to whether the CLI's internal
  state can survive repeated invocations without a process restart. Most CLIs
  that don't hold mutable global state can set this to `true`.

- **`parallel_safe`** (default: `false`): Controls whether tool calls are
  serialized behind a tokio `Mutex` (`false`) or dispatched concurrently
  (`true`). Set to `true` only if your CLI logic is safe to run concurrently.

- **`share_runtime`** (default: `false`): When `reinvocation_safe` is true,
  controls how async tool execution runs. See [Async tools and share_runtime](#async-tools-and-share_runtime) below.

### Attribute-based config (recommended)

Use `#[derive(ClapMcp)]` and `#[clap_mcp(...)]` on your CLI type:

```rust
use clap_mcp_macros::ClapMcp;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(parallel_safe = false, reinvocation_safe)]
#[command(...)]
enum Cli {
    #[clap_mcp_output = "format!(\"{}\", a + b)"]
    Add { a: i32, b: i32 },
    // ...
}

let cli = clap_mcp::parse_or_serve_mcp_attr::<Cli>();
```

### Schema metadata: skip and requires

Use `#[clap_mcp(skip)]` to exclude subcommands or arguments from MCP exposure.
Use `#[clap_mcp(requires)]` or `#[clap_mcp(requires = "arg_name")]` to make an optional
argument required in the MCP tool schema (useful for positional args that may trigger
stdin behavior when omitted). When the client omits a required arg, a clear error is returned.

**Argument-level** (on each field):
```rust
#[derive(Parser, ClapMcp)]
enum Cli {
    #[clap_mcp_output = "path.clone()"]
    Read {
        #[clap_mcp(requires)]  // MCP schema makes path required
        #[arg(long)]
        path: Option<String>,
    },
}
```

**Variant-level** (prefer when declaring multiple required args):
```rust
#[derive(Parser, ClapMcp)]
enum Cli {
    #[clap_mcp(requires = "path, input")]  // both become required in MCP
    #[clap_mcp_output = "format!(\"{:?}\", (path, input))"]
    Process {
        #[arg(long)] path: Option<String>,
        #[arg(long)] input: Option<String>,
    },
}
```

**Skip:**
```rust
#[derive(Parser, ClapMcp)]
enum Cli {
    #[clap_mcp_output_literal = "ok"]
    Public,
    #[clap_mcp(skip)]
    #[clap_mcp_output_literal = "hidden"]
    Internal,
}
```

**Imperative:** Use `schema_from_command_with_metadata` and `get_matches_or_serve_mcp_with_config_and_metadata` with `ClapMcpSchemaMetadata`:

```rust
let mut metadata = ClapMcpSchemaMetadata::default();
metadata.skip_commands.push("internal".into());
metadata.requires_args.insert("read".into(), vec!["path".into()]);
let schema = schema_from_command_with_metadata(&cmd, &metadata);
```

When the client omits a required argument, the tool returns a clear error:
`"Missing required argument(s): path. The MCP tool schema marks these as required."`

### Runtime config

Use `ClapMcpConfig` with `parse_or_serve_mcp_with_config` or `get_matches_or_serve_mcp_with_config`:

```rust
clap_mcp::parse_or_serve_mcp_with_config::<Cli>(clap_mcp::ClapMcpConfig {
    reinvocation_safe: true,   // in-process execution
    parallel_safe: false,      // serialize tool calls (default)
    ..Default::default()
})
```

Tools include `meta.clapMcp` with these hints for clients.

### Async tools and share_runtime

When your CLI has async subcommands (e.g. `tokio::sleep`, `tokio::spawn`), use
`clap_mcp::run_async_tool` in `#[clap_mcp_output]` and set `share_runtime` in
`#[clap_mcp(...)]`:

| `share_runtime` | Behavior | When to use |
|-----------------|----------|-------------|
| `false` (default) | Dedicated thread with its own tokio runtime per tool call. No nesting. | **Recommended.** Use unless you need deep integration. |
| `true` | Shares the MCP server's tokio runtime. Requires `reinvocation_safe`; uses multi-thread runtime. | Advanced: share runtime state, spawn long-lived tasks, or integrate with other async code. |

**Non-shared (default):**

```rust
#[clap_mcp(reinvocation_safe, parallel_safe = false, share_runtime = false)]
enum Cli {
    #[clap_mcp_output_json = "clap_mcp::run_async_tool(&Cli::clap_mcp_config(), || my_async_fn())"]
    SleepDemo,
}
```

**Shared runtime:**

```rust
#[clap_mcp(reinvocation_safe, parallel_safe = false, share_runtime)]
enum Cli {
    #[clap_mcp_output_json = "clap_mcp::run_async_tool(&Cli::clap_mcp_config(), || my_async_fn())"]
    SleepDemo,
}
```

`share_runtime` only applies when `reinvocation_safe` is true. When tools run
in subprocesses (`reinvocation_safe = false`), `share_runtime` is ignored.

## Security

The MCP server does **not** trust the client for tool or argument discovery. Every
tool call is validated against the schema before any execution (in-process or
subprocess). Unknown tools and unknown argument names are rejected immediately with
an error; execution proceeds only for schema-defined tools and arguments.

When `reinvocation_safe` is `false` (the default), each tool call spawns a fresh
subprocess of your binary. Consider the following:

**Shell injection is not a concern.** Arguments are passed via `std::process::Command::arg()`
directly to the executable as `argv` — no shell is invoked, so metacharacters
(`;`, `|`, `$()`, etc.) are not interpreted.

**Unknown tools and arguments are rejected.** The server validates every tool name and
argument name against the schema before execution. Invalid requests fail with
`CallToolError::unknown_tool` or `CallToolError::invalid_arguments`; no subprocess
is spawned and no in-process handler is invoked for invalid calls.

**Argument values come from the MCP client.** The schema constrains which argument
names are accepted, but values are passed through unvalidated. If your CLI uses those
values unsafely (e.g., in file paths, system calls, or other sensitive operations),
a malicious or compromised MCP client could exploit that. Ensure your CLI validates
and sanitizes all inputs.

**Environment and working directory are inherited.** The subprocess inherits the
full environment and CWD of the MCP server. Sensitive env vars (API keys, tokens)
are visible to every subprocess; relative paths resolve against the server's CWD.

**Resource usage.** Each tool call spawns a new process. With `parallel_safe = true`,
many concurrent calls can create many processes. There are no timeouts or resource
limits on subprocess execution.

## Tool output attributes

When using `#[derive(ClapMcp)]`, you control how each subcommand's output is
returned to MCP clients. You can either use a **single output function** (recommended)
or **per-variant output attributes**.

### `#[clap_mcp_output_from = "run"]` — single output function (recommended)

Put **one function** in charge of all tool output. The macro generates
`execute_for_mcp` by calling `run(self)` and converting the return value.
Use the same `run` in `main` so CLI and MCP share the same logic.

**Supported return types for `run`:**

- `String` or `&str` → text output
- [`AsStructured`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.AsStructured.html)`<T>` where `T: Serialize` → structured JSON output
- A type that implements [`IntoClapMcpResult`](https://docs.rs/clap-mcp/latest/clap_mcp/trait.IntoClapMcpResult.html) (e.g. a custom enum for mixed text/structured)
- `Option<O>` → `None` becomes empty text; `Some(o)` → `o.into_tool_result()`
- `Result<O, E>` → `Ok(o)` → output; `Err(e)` → MCP error. `E` must implement [`IntoClapMcpToolError`](https://docs.rs/clap-mcp/latest/clap_mcp/trait.IntoClapMcpToolError.html) (e.g. `String`, or your type for structured errors)

**Example:**

```rust
use clap::Parser;
use clap_mcp::{ClapMcp, AsStructured};

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run"]
#[command(name = "myapp", subcommand_required = false)]
enum Cli {
    Greet { #[arg(long)] name: Option<String> },
    Add { a: i32, b: i32 },
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::Greet { name } => format!("Hello, {}!", name.as_deref().unwrap_or("world")),
        Cli::Add { a, b } => format!("{}", a + b),
    }
}

fn main() {
    let cli = clap_mcp::parse_or_serve_mcp_attr::<Cli>();
    // Same logic: run(cli) for CLI, run(self) for MCP
    println!("{}", run(cli));
}
```

When `#[clap_mcp_output_from]` is set, **per-variant** `#[clap_mcp_output]` /
`#[clap_mcp_output_json]` / etc. are **not** used for execution.

### Per-variant output (when not using `output_from`)

When you do **not** set `#[clap_mcp_output_from]`, use these attributes on each variant:

### `#[clap_mcp_output = "expr"]`

A **Rust expression written as a string literal**. The macro destructures the
variant's fields, so field names are directly in scope:

```rust
enum Cli {
    #[clap_mcp_output = "format!(\"Hello, {}!\", clap_mcp::opt_str(&name, \"world\"))"]
    Greet {
        #[arg(long)]
        name: Option<String>,   // `name` is in scope inside the expression
    },
}
```

Variants without `#[clap_mcp_output]` default to the variant name in kebab-case
for unit variants, or `format!("{:?}", self)` for struct variants.

### `#[clap_mcp_output_json = "expr"]`

Single attribute for **structured JSON** output. The expression must evaluate to
a type that implements `Serialize`:

```rust
#[derive(Serialize)]
struct SubResult { difference: i32 }

enum Cli {
    #[clap_mcp_output_json = "SubResult { difference: a - b }"]
    Sub { a: i32, b: i32 },
}
```

The MCP response will include both a `content` text block (pretty-printed JSON)
and a `structuredContent` object.

### `#[clap_mcp_output_literal = "string"]`

Shorthand for constant string output. Generates `"string".to_string()`:

```rust
#[clap_mcp_output_literal = "done"]
Public,
```

### `ClapMcpServeOptions::capture_stdout`

When `true` and running in-process, captures stdout written during tool execution
and merges it with Text output. Only has effect when `reinvocation_safe = true`
(in-process execution). **Unix only** — the field is not present on Windows, so
code that sets `capture_stdout` will not compile on Windows. Subprocess mode
already captures stdout via `Command::output()`.

### `clap_mcp::opt_str` — helper for optional args

Use in `#[clap_mcp_output]` to avoid `as_deref().unwrap_or("default")` boilerplate:

```rust
#[clap_mcp_output = "format!(\"Hello, {}!\", clap_mcp::opt_str(&name, \"world\"))"]
Greet { #[arg(long)] name: Option<String> },
```

### `#[clap_mcp_output_result]` — fallible output

When the expression returns `Result<T, E>`, add `#[clap_mcp_output_result]`:

- `Ok(value)` → normal MCP output (Text or Structured)
- `Err(e)` → MCP error response (`is_error: true`) with the error message

```rust
enum Cli {
    #[clap_mcp_output_result]
    #[clap_mcp_output = "if n >= 0 { Ok(format!(\"sqrt ~{}\", n)) } else { Err(format!(\"negative: {}\", n)) }"]
    Sqrt { #[arg(long)] n: i32 },
}
```

### `#[clap_mcp_error_type = "TypeName"]` — structured errors

When `E: Serialize`, add `#[clap_mcp_error_type = "TypeName"]` to include the
serialized error in the MCP response's `structuredContent`:

```rust
#[derive(Serialize)]
struct MyError { code: i32, msg: String }

enum Cli {
    #[clap_mcp_output_result]
    #[clap_mcp_error_type = "MyError"]
    #[clap_mcp_output = "if x > 0 { Ok(\"ok\".into()) } else { Err(MyError { code: -1, msg: \"invalid\".into() }) }"]
    Check { #[arg(long)] x: i32 },
}
```

## Output schema (oneOf) for MCP tool discovery

With the **`output-schema`** feature enabled, you can attach a JSON schema to each tool's
`outputSchema` field so MCP clients know the shape of the tool's output.

### `#[clap_mcp_output_type = "TypeName"]`

Use when your tool output is a **single type** (e.g. an enum or struct). The type must
implement [`schemars::JsonSchema`](https://docs.rs/schemars/latest/schemars/trait.JsonSchema.html).
For enums, schemars typically produces a `oneOf` schema.

```rust
// Requires: features = ["output-schema"], and schemars + JsonSchema on the type
#[derive(Serialize, schemars::JsonSchema)]
struct SubcommandResult { result: String }

#[derive(Parser, ClapMcp)]
#[clap_mcp_output_from = "run"]
#[clap_mcp_output_type = "SubcommandResult"]
enum Cli { ... }
```

### `#[clap_mcp_output_one_of = "T1, T2, T3"]`

Use when you want to list **multiple types** explicitly for a `oneOf` schema without
defining a wrapper enum. Each type must implement `schemars::JsonSchema`.

```rust
#[derive(Serialize, schemars::JsonSchema)]
struct AddResult { sum: i32 }
#[derive(Serialize, schemars::JsonSchema)]
struct SubResult { difference: i32 }

#[derive(Parser, ClapMcp)]
#[clap_mcp_output_one_of = "AddResult, SubResult"]
enum Cli { ... }
```

When either attribute is set, [`ClapMcpSchemaMetadata::output_schema`] is populated
(by the derive) and [`tools_from_schema_with_config_and_metadata`] attaches it to
each tool. The high-level serve path (`parse_or_serve_mcp_attr`, etc.) uses metadata
automatically, so tools get `output_schema` when you use the derive and these attributes.

## Logging and observability

clap-mcp can forward application log messages to MCP clients as
`notifications/message`. Two feature-gated paths are available depending on
your logging ecosystem.

### `tracing` feature

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
```

**Current limitations:**

- Only the `message` field of each tracing event is forwarded. Other structured
  fields (e.g. `tracing::info!(count = 42, "done")` — `count` is dropped) are
  not yet included.
- Span lifecycle events (`on_new_span`, `on_enter`, `on_close`) are not
  captured.

### `log` feature

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
```

**Trade-off:** The `log` crate supports exactly **one global logger**. Installing
`ClapMcpLogBridge` replaces any existing logger (e.g. `env_logger`,
`simplelog`). If you need to log to both disk and MCP simultaneously, you'll
need a multiplexing wrapper — either a custom `Log` impl that fans out to
multiple sinks, or a crate like
[`multi_log`](https://crates.io/crates/multi_log).
