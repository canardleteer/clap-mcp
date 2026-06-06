# clap-mcp

> **Enrich your Rust CLI with MCP Capabilities**

[![crates.io](https://img.shields.io/crates/v/clap-mcp.svg)](https://crates.io/crates/clap-mcp)
[![docs.rs](https://docs.rs/clap-mcp/badge.svg)](https://docs.rs/clap-mcp)

## Usage

You can take a look at the examples, but this is a **VERY** early draft. See
[examples/README.md](examples/README.md) for detailed instructions on running
them.

## Design

Compared to a Command Line Interface, I'm not a huge fan of the
[Model Context Protocol](https://modelcontextprotocol.io/docs/getting-started/intro),
but my feelings don't represent real world usage patterns. I feel MCP would do better
with gRPC and Protobuf as it's "transport." All that being said, I'm not bitter
about it, so I'm just letting a model do the development work and deal with it's
own self-generated mess.

**The intent is generally:**

* Make it easy to add a MCP server to current Rust CLIs that use `clap`.
* Have it work well enough and provide enough guardrails to cover the 95% case.
* If there is structured information available from the CLI as an outcome, we
  should provide a way to express it naturally via MCP.
* Provide a way to express structured logging information (if available) as part
  of the response if requested.
* Avoid being opinionated if we don't have to be, accept being as little
  opinionated as possible if the alternative is complicating the primary public
  API.

Overall, the more you design your CLI around a service pattern, the more
naturally this crate will behave as an MCP server, and modern CLIs often do
that. At the same time, we shouldn't force CLIs that don't do that, out of the
ecosystem.

## Quick start

> [!WARNING]
> Clanker generated code, running an auto-release pipeline, without a stable API
> yet.

Add `clap-mcp` to your `Cargo.toml` (the default `derive` feature includes the
macro):

```toml
[dependencies]
clap-mcp = "0.0.4-rc.1"
```

For derive usage, `use clap_mcp::ClapMcp` so you can write `#[derive(ClapMcp)]`.

## CLI compatibility

Adding clap-mcp should not change how your CLI runs unless you explicitly opt
into MCP.

1. **MCP is flag-opt-in only.** A server starts only when the user passes **`--mcp`**
   (stdio) or **`--mcp-http`** ([`http`](docs/http.md) feature). Normal invocations
   never accidentally enter MCP mode.

2. **Non-MCP behavior is unchanged.** Any argv **without** an MCP flag must
   parse and run the same as before you added clap-mcp: same errors, same
   success paths, same subcommand rules. Swap `Cli::parse()` for
   [`ParseOrServeMcp::parse_or_serve_mcp`] (or [`get_matches_or_serve_mcp`]
   imperatively) — **do not** change subcommand types or `subcommand_required`
   unless you already planned to.

3. **`--mcp` does not require `Option<Commands>`.** If your CLI already uses a
   required subcommand (`command: Commands` + `subcommand_required = true`),
   keep it. clap-mcp checks for `--mcp` **before** clap's subcommand validation,
   so `myapp --mcp` works while bare `myapp` still errors exactly as clap did
   before.

| Invocation | Flat enum CLI (no struct subcommand) | Required struct subcommand | Optional struct subcommand (`Option<Commands>`) |
|---|---|---|---|
| Normal args (no MCP flag) | Unchanged | Unchanged | Unchanged |
| Bare root (no subcommand) | N/A or app-defined | **Still clap error** | Parses; `main` handles `None` |
| `--mcp` / `--mcp-http` | MCP server | MCP server | MCP server |

**Do not migrate to `Option<Commands>` solely for MCP** — that changes bare-invocation
behavior for CLIs that previously required a subcommand. Use
**struct_subcommand_required** in [examples/README.md](examples/README.md) as the
typical struct-root migration reference; **struct_subcommand** demonstrates optional
subcommands only.

### Shell `--` vs MCP passthrough

| Path | How passthrough works |
|---|---|
| **Direct CLI (shell)** | clap handles `--` natively; tokens after the first `--` are not parsed as flags. |
| **MCP `tools/call`** | No `--` is inserted. Pass trailing tokens as a JSON **array** on a `Vec<String>` field (often with `#[arg(last = true, allow_hyphen_values = true)]`) or as an explicit `--long` list. |

clap-mcp's pre-clap argv checks (MCP stdio, HTTP, export-skills) inspect only
tokens **before** the first standalone `"--"`. So `myapp run -- --mcp` does
**not** start MCP — `--mcp` is passthrough to a child. Subcommand names in that
prefix are honored the same way.

`build_tool_argv` rebuilds named JSON into argv for tool execution. For trailing
multi-value positionals (`num_args(1..)` / cargo-style `last` vecs), it inserts
`--` before the trailing tokens so clap parses them as values. For
hyphen-prefixed tokens, prefer an explicit `#[arg(long)] args: Vec<String>` or
`allow_hyphen_values = true` on the trailing field. See **passthrough_args** and
**vec_and_flags** in [examples/README.md](examples/README.md).

### Renaming clap-mcp builtin flags

If your app already uses `--mcp` for something else, rename clap-mcp's stdio,
HTTP, and export-skills flags via derive attributes or
[`ClapMcpBuiltinFlags`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapMcpBuiltinFlags.html):

| Builtin | Default long | Derive attr | Stable clap arg id |
|---|---|---|---|
| stdio MCP | `--mcp` | `mcp_flag = "…"` | `CLAP_MCP_STDIO_FLAG_ID` |
| HTTP MCP | `--mcp-http` | `mcp_http_flag = "…"` | `CLAP_MCP_HTTP_FLAG_ID` |
| export skills | `--export-skills` | `export_skills_flag = "…"` | `CLAP_MCP_EXPORT_SKILLS_FLAG_ID` |

clap matches by **stable id** internally; argv uses the configured **long**
name. Existing apps keep the defaults with no code changes. See
**custom_mcp_flags** in [examples/README.md](examples/README.md).

Imperative helpers: `command_with_mcp_flag_with_flags(cmd, &flags)` (and
export-skills / HTTP variants). Default wrappers unchanged.

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
use clap_mcp::ClapMcp;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp_output_from = "run"]
#[command(name = "myapp")]
enum Cli {
    /// Say hello.
    Greet {
        #[arg(long)]
        name: Option<String>,
    },
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::Greet { name } => format!("Hello, {}!", name.as_deref().unwrap_or("world")),
    }
}

fn main() {
    let cli = Cli::parse_or_serve_mcp();
    println!("{}", run(cli));
}
```

### Derive with attributes (recommended)

Use `#[clap_mcp(...)]` to declare execution safety, and
`ParseOrServeMcp::parse_or_serve_mcp` to pick up that config automatically:

```rust
use clap::Parser;
use clap_mcp::ClapMcp;

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[clap_mcp_output_from = "run"]
#[command(name = "myapp")]
enum Cli {
    Add {
        #[arg(long)]
        a: i32,
        #[arg(long)]
        b: i32,
    },
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::Add { a, b } => (a + b).to_string(),
    }
}

fn main() {
    let cli = Cli::parse_or_serve_mcp();
    println!("{}", run(cli));
}
```

### Struct root with subcommand

When your CLI has a **struct root** with `#[command(subcommand)]` and an enum of
commands, derive `ClapMcp` on **both** the root struct and the subcommand enum.
Put `#[clap_mcp_output_from = "run"]` and execution config (`#[clap_mcp(...)]`)
on the **subcommand** enum. In `main`, parse the root then dispatch on the
subcommand.

**Recommended migration (required subcommand, zero CLI regression):** keep
`command: Commands` and `subcommand_required = true`. Only add
`parse_or_serve_mcp()`.

```rust
use clap::{Parser, Subcommand};
use clap_mcp::ClapMcp;

#[derive(Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe = false)]
#[command(subcommand_required = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, ClapMcp)]
#[clap_mcp_output_from = "run"]
enum Commands {
    Explain { version: String },
}

fn run(cmd: Commands) -> String {
    match cmd {
        Commands::Explain { version } => format!("explain {version}"),
    }
}

fn main() {
    let cli = Cli::parse_or_serve_mcp();
    println!("{}", run(cli.command));
}
```

Bare `myapp` still fails with clap's missing-subcommand error; `myapp explain 1.0`
and `myapp --mcp` both work.

**Optional subcommand (only if your CLI already used this):** use
`subcommand_required = false` with `command: Option<Commands>` and handle `None`
in `main`. Do not adopt this pattern only for MCP — see **struct_subcommand**
in [examples/README.md](examples/README.md).

See [Dual derive (root + subcommand)](#dual-derive-root--subcommand) and
**struct_subcommand_required** in [examples/README.md](examples/README.md).

## Feature flags

| Flag | Enables |
| --- | --- |
| `derive` (default) | `#[derive(ClapMcp)]` proc-macro and `ParseOrServeMcp` |
| `tracing` | `ClapMcpTracingLayer` — a `tracing_subscriber::Layer` that forwards tracing events to MCP clients via `notifications/message`. |
| `log` | `ClapMcpLogBridge` — a `log::Log` implementation that forwards `log` crate messages to MCP clients. |
| `output-schema` | `schemars`-based JSON schema generation for structured tool output. Enables [`output_schema_for_type`], [`output_schema_one_of!`], and `#[clap_mcp_output_type]` / `#[clap_mcp_output_one_of]` to set each tool's `output_schema` for MCP clients. |
| `http` | Streamable HTTP MCP server (`--mcp-http`); see [docs/http.md](docs/http.md). |
| `http-oauth` | OAuth client helpers for calling remote MCP servers; see [docs/oauth.md](docs/oauth.md). |
| `elicitation` | Server-side elicitation during tool execution (experimental). |

Enable features in `Cargo.toml`:

```toml
[dependencies]
clap-mcp = { version = "0.0.4-rc.1", features = ["tracing"] }
```

## Custom resources and prompts

In addition to the built-in **`clap://schema`** resource and the optional
**logging guide** prompt, you can expose custom MCP resources and prompts. Add
them to
[`ClapMcpServeOptions`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapMcpServeOptions.html)
and pass that into `parse_or_serve_mcp_with`, [`ServeMcpBuilder`], or the
lower-level [`serve_mcp`] / [`serve_mcp_blocking`] functions.

### Custom resources

Set
[`custom_resources`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapMcpServeOptions.html#structfield.custom_resources)
to a list of
[`CustomResource`](https://docs.rs/clap-mcp/latest/clap_mcp/content/struct.CustomResource.html)
values. Each has:

* **Identity:** `uri`, `name`, optional `title`, `description`, `mime_type`. Use
  a stable URI (e.g. `myapp://config`) so clients can list and read.
* **Content:** Either **static** (`ResourceContent::Static(String)`) or
  **dynamic** (`ResourceContent::Dynamic(Arc<dyn ResourceContentProvider>)`).
  Dynamic content uses the async
  [`ResourceContentProvider::read`](https://docs.rs/clap-mcp/latest/clap_mcp/content/trait.ResourceContentProvider.html#tymethod.read)
  so the handler can await it.

Example (static):

```rust
use clap_mcp::content::{CustomResource, ResourceContent};

let mut opts = clap_mcp::ClapMcpServeOptions::default();
opts.custom_resources.push(CustomResource {
    uri: "myapp://readme".into(),
    name: "readme".into(),
    title: Some("Readme".into()),
    description: Some("Project readme".into()),
    mime_type: Some("text/markdown".into()),
    content: ResourceContent::Static("# Hello\n".into()),
});
```

For dynamic content, implement
[`ResourceContentProvider`](https://docs.rs/clap-mcp/latest/clap_mcp/content/trait.ResourceContentProvider.html)
(async `read(uri)`).

### Custom prompts

Set
[`custom_prompts`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapMcpServeOptions.html#structfield.custom_prompts)
to a list of
[`CustomPrompt`](https://docs.rs/clap-mcp/latest/clap_mcp/content/struct.CustomPrompt.html)
values. Each has:

* **Identity:** `name`, optional `title`, `description`, optional `arguments`
  (MCP prompt argument descriptors).
* **Content:** Either **static** (`PromptContent::Static(Vec<PromptMessage>)`)
  or **dynamic** (`PromptContent::Dynamic(Arc<dyn PromptContentProvider>)`).
  Dynamic uses the async
  [`PromptContentProvider::get`](https://docs.rs/clap-mcp/latest/clap_mcp/content/trait.PromptContentProvider.html#tymethod.get).

The built-in **`clap-mcp-logging-guide`** prompt is only listed when logging is
enabled (`serve_options.log_rx.is_some()`). Custom prompts are always merged
into the list.

### URI and name conventions

Prefer a stable prefix (e.g. `myapp://`) for custom resource URIs so they don’t
clash with the built-in `clap://schema`. Prompt names must be unique; avoid
`clap-mcp-logging-guide` for custom prompts.

## Exporting agent skills

You can generate **[Agent Skills](https://agentskills.io/specification)**
(SKILL.md) from the same tools, resources, and prompts that the MCP server
exposes. This is useful for documenting your CLI for AI agents.

### The `--export-skills` flag

Add the flag with
[`command_with_export_skills_flag`](https://docs.rs/clap-mcp/latest/clap_mcp/fn.command_with_export_skills_flag.html)
or use
[`command_with_mcp_and_export_skills_flags`](https://docs.rs/clap-mcp/latest/clap_mcp/fn.command_with_mcp_and_export_skills_flags.html)
to add both `--mcp` and `--export-skills`:

* **`--export-skills`** — Generate skills into the default directory (see below)
  and exit.
* **`--export-skills=DIR`** — Generate skills into `DIR` (e.g.
  `--export-skills=./out`) and exit.

When both `--mcp` and `--export-skills` are present, **`--export-skills` wins**:
the process exports and exits without starting the MCP server.

### Default output directory

Default directory is **`.agents/skills/`**, where each skill gets a subdirectory
named after the app or tool. Override with `--export-skills=DIR`.

### What gets generated

* One skill per **tool** (from your clap schema), with name/description and
  usage hints.
* A combined **resources-and-prompts** skill when you have custom resources or
  prompts.

Generated files follow the
[Agent Skills specification](https://agentskills.io/specification) (YAML
frontmatter with `name`, `description`, and `allowed-tools`; markdown body with
usage instructions). The `name` field matches the parent directory name as
required by the spec. Each tool skill includes `allowed-tools` listing the MCP
tool it describes; note that this field is still experimental in the spec
with no defined syntax convention. You can also call
[`content::export_skills`](https://docs.rs/clap-mcp/latest/clap_mcp/content/fn.export_skills.html)
programmatically with schema, tools, custom resources, and custom prompts.

## Execution safety configuration

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

### Attribute-based config (recommended)

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

### Schema metadata: skip and requires

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

### Dual derive (root + subcommand)

When you use a **struct root** with `#[command(subcommand)]`, derive `ClapMcp` on
**both** the root struct and the subcommand enum. Put `#[clap_mcp_output_from =
"run"]` and execution config (`#[clap_mcp(...)]`) on the **subcommand** enum only.
The root's derive provides schema metadata and delegates tool execution to the
subcommand's executor. In `main`, parse with `Root::parse_or_serve_mcp()` then
dispatch (`run(cli.command)` when the subcommand field is required, or `match
cli.command` when it is `Option<Commands>`).

Keep **`subcommand_required = true`** and a required `Commands` field when that
is your CLI today — `myapp --mcp` works without switching to `Option`. See
[CLI compatibility](#cli-compatibility) and **struct_subcommand_required** in
[examples/README.md](examples/README.md).

**MCP tool list:** The tool list includes the root command and all subcommands.
If your CLI has `subcommand_required = true`, the root command still appears as
a tool but has no subcommand in the MCP invocation model and is rarely used by
clients; the meaningful tools are the subcommands (e.g. explain, compare, sort).
To exclude the root from the tool list when it has subcommands, set
[`ClapMcpSchemaMetadata::skip_root_command_when_subcommands`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapMcpSchemaMetadata.html#structfield.skip_root_command_when_subcommands)
to `true` via the derive with `#[clap_mcp(skip_root_when_subcommands)]` on the
root struct, or imperatively (e.g. implement `ClapMcpSchemaMetadataProvider` for
the root and set the field, or build metadata manually).

### Stateful MCP tools (shared session state)

When `reinvocation_safe` is true, in-process tool calls can share session state
for the MCP server lifetime. The server stores state in an [`Arc`] internally; your
`run` function receives **`&Self::State`** on each call (not `&Arc<…>`).

Setup (see also [`ClapMcpToolExecutorWithState`] rustdoc):

* **Leaf subcommand enum:** `#[clap_mcp_output_from_with_state = "run"]` plus
  `#[clap_mcp_state_type = "Type"]` where `Type` matches the second parameter of
  `run` (e.g. `run(cmd, state: &Mutex<CounterState>)` →
  `#[clap_mcp_state_type = "Mutex<CounterState>"]`).
* **Struct root / intermediate enums:** `#[clap_mcp(stateful)]` — `State` is
  inferred from the subcommand field; do not repeat `state_type`.

```rust
use clap_mcp::{ClapMcp, ParseOrServeMcpWithState};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct CounterState { count: u64 }

#[derive(Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe = true, stateful)]
struct App {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, ClapMcp)]
#[clap_mcp(reinvocation_safe = true)]
#[clap_mcp_output_from_with_state = "run"]
#[clap_mcp_state_type = "Mutex<CounterState>"]
enum Command { Increment, Read }

fn run(cmd: Command, state: &Mutex<CounterState>) -> String { /* ... */ }

fn main() {
    let state = Arc::new(Mutex::new(CounterState::default()));
    let app = App::parse_or_serve_mcp_with_state(state.clone());
    // normal CLI path when --mcp was not passed
}
```

Entrypoints: [`ParseOrServeMcpWithState::parse_or_serve_mcp_with_state`],
[`parse_or_serve_mcp_with_state`], and
[`ServeMcpBuilder::for_cli_with_state`]. Requires `reinvocation_safe`. Example:
[stateful_counter](examples/servers/stateful_counter.rs) (ported from
[PR #11](https://github.com/canardleteer/clap-mcp/pull/11) by Eddy Stefes / fneddy).

### Runtime config

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

### Async embedders

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

See [async_embedder_serve](examples/servers/async_embedder_serve.rs) (imperative
[`ServeMcpBuilder`]) vs [async_sleep_shared](examples/servers/async_sleep_shared.rs)
(derive + `parse_or_serve_mcp_with`). For HTTP, use `McpListen::Http(addr)` —
see [docs/http.md](docs/http.md).

If [`ClapMcpConfig::needs_multi_thread_runtime`] is true and you call
[`ServeMcpBuilder::serve`] on a `current_thread` runtime, you get
[`ClapMcpError::RequiresMultiThreadRuntime`]; use
`#[tokio::main(flavor = "multi_thread")]` or [`ServeMcpBuilder::serve_blocking`].

### Crash and panic behavior

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
  [examples/README.md](examples/README.md).

### Async tools and share_runtime

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

### MCP task-augmented `tools/call`

[Tasks](https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/tasks)
let a client send `tools/call` with `task` metadata, receive a
[`CreateTaskResult`](https://docs.rs/rmcp/latest/rmcp/model/task/struct.CreateTaskResult.html)
immediately, and poll `tasks/get` / `tasks/result` for completion.

**Supported in clap-mcp (this release):** in-process servers only
(`reinvocation_safe = true`), both `share_runtime = false` and
`share_runtime = true`, serialized (`parallel_safe = false`) and concurrent
(`parallel_safe = true`) tool execution with task-augmented `tools/call`, and
`catch_in_process_panics = true` with task-augmented `tools/call` (panics in
task-scheduled work map to task error payloads; the server stays up). When
`parallel_safe` is false, task-augmented and plain `tools/call` share one
serialization queue; when `parallel_safe` is true, bodies may overlap and
logging uses per-task context so `meta.taskId` stays correct.

* Enable with `#[clap_mcp(task_augmented_tools)]` on the enum (requires
  `reinvocation_safe`; combining with `reinvocation_safe = false` is a **compile
  error** in the derive).
* Optionally mark subcommands with `#[clap_mcp(task)]` so only those tools
  advertise `meta.clapMcp.taskAugmented` and accept task-augmented calls. If no
  variant has `#[clap_mcp(task)]`, every tool is eligible when
  `task_augmented_tools` is on.
* **Not supported:** subprocess (`reinvocation_safe = false`) + tasks;
  supporting that would need non-blocking waits, task lifecycle, and stderr
  correlation—out of scope here.

**Pinned stack (review on bump):** workspace [`rmcp`](https://docs.rs/rmcp)
**1.7.x** (see root `Cargo.toml`; features `server`, `client`, `macros`,
`transport-io`, `transport-child-process`). Protocol **2025-11-25** (tasks).
Migration notes: [docs/rmcp-migration-notes.md](docs/rmcp-migration-notes.md).

**Examples:** [task_tools_dedicated](examples/servers/task_tools_dedicated.rs)
(dedicated async runtime),
[task_tools_shared](examples/servers/task_tools_shared.rs) (shared MCP runtime),
[task_panic_catch](examples/servers/task_panic_catch.rs) (panic-in-task with
`catch_in_process_panics`), and
[task_augmented_client](examples/task_augmented_client.rs) (rmcp client with
task-augmented `tools/call` and polling). Integration tests also use
`task_serial_probe_*` (serialized probes), `task_parallel_probe_*` (concurrent
probes), and `task_panic_catch_parallel` (panic under `parallel_safe = true`).

**Logging during tasks:** When `ClapMcpServeOptions::log_rx` is set and you use
the `tracing` or `log` bridges, log notifications emitted during a
task-augmented tool body include `meta.taskId` in notification extensions,
matching `CreateTaskResult.task.task_id` (including when multiple task bodies
run concurrently).

`share_runtime` only applies when `reinvocation_safe` is true. When tools run
in subprocesses (`reinvocation_safe = false`), `share_runtime` is ignored.

## Security

The MCP server does **not** trust the client for tool or argument discovery.
Every
tool call is validated against the schema before any execution (in-process or
subprocess). Unknown tools and unknown argument names are rejected immediately
with
an error; execution proceeds only for schema-defined tools and arguments.

When `reinvocation_safe` is `false` (the default), each tool call spawns a fresh
subprocess of your binary. Consider the following:

**Shell injection is not a concern.** Arguments are passed via
`std::process::Command::arg()`
directly to the executable as `argv` — no shell is invoked, so metacharacters
(`;`, `|`, `$()`, etc.) are not interpreted.

**Unknown tools and arguments are rejected.** The server validates every tool
name and
argument name against the schema before execution. Invalid requests fail with
`CallToolError::unknown_tool` or `CallToolError::invalid_arguments`; no
subprocess
is spawned and no in-process handler is invoked for invalid calls.

**Argument values come from the MCP client.** The schema constrains which
argument
names are accepted, but values are passed through unvalidated. If your CLI uses
those
values unsafely (e.g., in file paths, system calls, or other sensitive
operations),
a malicious or compromised MCP client could exploit that. Ensure your CLI
validates
and sanitizes all inputs.

**Environment and working directory are inherited.** The subprocess inherits the
full environment and CWD of the MCP server. Sensitive env vars (API keys,
tokens)
are visible to every subprocess; relative paths resolve against the server's
CWD.

**Resource usage.** Each tool call spawns a new process. With `parallel_safe =
true`,
many concurrent calls can create many processes. There are no timeouts or
resource
limits on subprocess execution.

## Tool output attributes

When using `#[derive(ClapMcp)]`, you control how each subcommand's output is
returned to MCP clients. The **idiomatic** approach is a **single output
function**
(`#[clap_mcp_output_from = "run"]`): one `run` implements both CLI and MCP
behavior, so you
avoid duplicating logic. Per-variant attributes are available for edge cases but
are not
the default.

### `#[clap_mcp_output_from = "run"]` — single output function (recommended)

Put **one function** in charge of all tool output. The macro generates
`execute_for_mcp` by calling `run(self)` and converting the return value.
Use the same `run` in `main` so CLI and MCP share the same logic.

**Supported return types for `run`:**

* `String` or `&str` → text output
* [`AsStructured`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.AsStructured.html)`<T>`
  where `T: Serialize` → structured JSON output
* A type that implements
  [`IntoClapMcpResult`](https://docs.rs/clap-mcp/latest/clap_mcp/trait.IntoClapMcpResult.html)
  (e.g. a custom enum for mixed text/structured)
* `Option<O>` → `None` becomes empty text; `Some(o)` → `o.into_tool_result()`
* `Result<O, E>` → `Ok(o)` → output; `Err(e)` → MCP error. `E` must implement
  [`IntoClapMcpToolError`](https://docs.rs/clap-mcp/latest/clap_mcp/trait.IntoClapMcpToolError.html)
  (e.g. `String`, or your type for structured errors)

`Result<AsStructured<T>, E>` is fully supported when you want structured success
payloads and a separate error type;
[`IntoClapMcpResult`](https://docs.rs/clap-mcp/latest/clap_mcp/trait.IntoClapMcpResult.html)
is implemented for `AsStructured<T: Serialize>`.

**Recommended pattern for CLIs with multiple subcommands:** have `run` return
`Result<AsStructured<SubcommandResult>, ApplicationError>` and use
`#[clap_mcp_output_from = "run"]`. Implement
[`IntoClapMcpToolError`](https://docs.rs/clap-mcp/latest/clap_mcp/trait.IntoClapMcpToolError.html)
for your application error type and cover **all** error variants (e.g.
`InvalidArgument`, validation errors, I/O errors) in that single impl so MCP
error responses are consistent across tools.

For `run() -> Result<O, E>`, ensure `E: IntoClapMcpToolError` and the macro will
convert the return value automatically.

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
    Add {
        #[arg(long)]
        a: i32,
        #[arg(long)]
        b: i32,
    },
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::Greet { name } => format!("Hello, {}!", name.as_deref().unwrap_or("world")),
        Cli::Add { a, b } => format!("{}", a + b),
    }
}

fn main() {
    let cli = Cli::parse_or_serve_mcp();
    // Same logic: run(cli) for CLI, run(self) for MCP
    println!("{}", run(cli));
}
```

Tool output is defined **only** via `#[clap_mcp_output_from = "run"]` and a
single `run`
function; there are no per-variant output attributes. Use `run(Cli) -> T` where
`T`
implements `IntoClapMcpResult` (e.g. `String`, `AsStructured<T>`, `Result<O,
E>`).

### `ClapMcpServeOptions::capture_stdout`

When `true` and running in-process, captures stdout written during tool
execution
and merges it with Text output. Only has effect when `reinvocation_safe = true`
(in-process execution). **Unix only** — the field is not present on Windows, so
code that sets `capture_stdout` will not compile on Windows. Subprocess mode
already captures stdout via `Command::output()`.

## Output schema (oneOf) for MCP tool discovery

With the **`output-schema`** feature enabled, you can attach a JSON schema to
each tool's
`outputSchema` field so MCP clients know the shape of the tool's output.

### `#[clap_mcp_output_type = "TypeName"]`

Use when your tool output is a **single type** (e.g. an enum or struct). The
type must
implement
[`schemars::JsonSchema`](https://docs.rs/schemars/latest/schemars/trait.JsonSchema.html).
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

Use when you want to list **multiple types** explicitly for a `oneOf` schema
without
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

When either attribute is set, [`ClapMcpSchemaMetadata::output_schema`] is
populated
(by the derive) and [`tools_from_schema_with_metadata`] attaches it to
each tool. The high-level serve path (`ParseOrServeMcp::parse_or_serve_mcp`,
etc.) uses metadata
automatically, so tools get `output_schema` when you use the derive and these
attributes.

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
// Pass opts to parse_or_serve_mcp_with or ServeMcpBuilder::serve_options
```

**Current limitations:**

* Only the `message` field of each tracing event is forwarded. Other structured
  fields (e.g. `tracing::info!(count = 42, "done")` — `count` is dropped) are
  not yet included.
* Span lifecycle events (`on_new_span`, `on_enter`, `on_close`) are not
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
// Pass opts to parse_or_serve_mcp_with or ServeMcpBuilder::serve_options
```

**Trade-off:** The `log` crate supports exactly **one global logger**.
Installing
`ClapMcpLogBridge` replaces any existing logger (e.g. `env_logger`,
`simplelog`). If you need to log to both disk and MCP simultaneously, you'll
need a multiplexing wrapper — either a custom `Log` impl that fans out to
multiple sinks, or a crate like
[`multi_log`](https://crates.io/crates/multi_log).

## Streamable HTTP (`http` feature)

Enable the optional `http` feature on `clap-mcp` to serve MCP over Streamable
HTTP instead of stdio:

```toml
clap-mcp = { version = "0.0.4-rc.1", features = ["derive", "http"] }
```

Run with `--mcp-http 127.0.0.1:8080`, `--mcp-http` alone with
`CLAP_MCP_HTTP_LISTEN`, or `CLAP_MCP_HTTP_BIND` + `CLAP_MCP_HTTP_PORT`. See
[docs/http.md](docs/http.md). `--mcp` (stdio) and `--mcp-http` are mutually
exclusive.

Example: `cargo run -p clap-mcp-examples --bin subcommands_http --features http
-- --mcp-http 127.0.0.1:8080`

Maintainers: MCP spec conformance against that example — `cargo xtask
conformance` (local Docker) or see
[docs/conformance-baseline.md](docs/conformance-baseline.md).

## Development

Contributors should follow these conventions. AI agents should also read
[AGENTS.md](AGENTS.md) for design priorities and doc touchpoints.

* Format code with `cargo fmt`. CI runs `cargo fmt --all -- --check`.
* Run `cargo clippy --all-targets --all-features -- -D warnings` before
  submitting; CI enforces this.
* Document public API items and add a `// SAFETY:` comment above any `unsafe`
  block explaining invariants.

MCP task support matrix (including limitations) is under
[MCP task-augmented `tools/call`](#mcp-task-augmented-toolscall) above.

Run all tests (including feature-gated logging tests):

```bash
cargo test --all-features
```

### Code coverage

Coverage is measured with
[cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov). Install and run:

```bash
cargo install cargo-llvm-cov
cargo llvm-cov test --workspace --all-features --summary-only
```

For an HTML report (opens in browser):

```bash
cargo llvm-cov test --workspace --all-features --html
```

Coverage focuses on the `clap-mcp` and `clap-mcp-macros` crates; the `examples`
crate is excluded from coverage targets.

Release prep includes building and running all example binaries (CI runs each
with `--help` as a smoke test); see [examples/README.md](examples/README.md).
