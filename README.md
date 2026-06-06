# clap-mcp

> **Enrich your Rust CLI with MCP Capabilities**

[![crates.io](https://img.shields.io/crates/v/clap-mcp.svg)](https://crates.io/crates/clap-mcp)
[![docs.rs](https://docs.rs/clap-mcp/badge.svg)](https://docs.rs/clap-mcp)

## Usage

This is still a draft, and we're exposing a rapidly evolving specification (MCP)
through a relatively stable one (`clap`). That mismatch in velocity will err
towards instability of the public API surface.

See [examples/README.md](examples/README.md) for detailed examples of usage.

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

> [!WARNING]
> Clanker generated code, running an auto-release pipeline, without a stable API
> yet.

## Crate Features

Add `clap-mcp` to your `Cargo.toml` (the default `derive` feature includes the
macro):

```toml
[dependencies]
clap-mcp = "0.0.4-rc.1"
```

* Opt-in MCP server on existing `clap` CLIs (`--mcp` stdio, `--mcp-http` with
  `http` feature)
* `#[derive(ClapMcp)]` — subcommands exposed as MCP tools with shared `run`
  output
* Execution modes: subprocess (default) or in-process (`reinvocation_safe`);
  see [execution-safety](docs/execution-safety.md)
* Structured tool output and optional JSON `outputSchema`; see
  [tool-output](docs/tool-output.md)
* Logging forwarded to MCP clients (`tracing` / `log` features); see
  [logging](docs/logging.md)
* Custom MCP resources and prompts; see [custom-content](docs/custom-content.md)
* Agent Skills export (`--export-skills`); see
  [export-skills](docs/export-skills.md)
* Stateful in-process session tools; see
  [stateful-tools](docs/stateful-tools.md)
* MCP task-augmented `tools/call`; see [MCP tasks support](docs/mcp-tasks.md)

### Feature Flags

Cargo features and their maturity:

| Maturity | Meaning |
| --- | --- |
| **Shipped** | Supported embedder surface; exercised in CI and examples. |
| **Scaffolding** | Exploratory spike — API and behavior may change; not a conformance or release parity target. |

| Flag | Maturity | Enables |
| --- | --- | --- |
| `derive` (default) | Shipped | `#[derive(ClapMcp)]` proc-macro and `ParseOrServeMcp` |
| `tracing` | Shipped | `ClapMcpTracingLayer` — a `tracing_subscriber::Layer` that forwards tracing events to MCP clients via `notifications/message`. |
| `log` | Shipped | `ClapMcpLogBridge` — a `log::Log` implementation that forwards `log` crate messages to MCP clients. |
| `output-schema` | Shipped | `schemars`-based JSON schema generation for structured tool output. Enables [`output_schema_for_type`], [`output_schema_one_of!`], and `#[clap_mcp_output_type]` / `#[clap_mcp_output_one_of]` to set each tool's `output_schema` for MCP clients. |
| `http` | Shipped | Streamable HTTP MCP server (`--mcp-http`); see [http.md](docs/http.md). |
| `http-oauth` | Scaffolding | OAuth client helpers for calling remote MCP servers; see [oauth.md](docs/oauth.md). |
| `elicitation` | Scaffolding | Server-side elicitation during tool execution (`confirm-echo` intercept only). |

Enable features in `Cargo.toml`:

```toml
[dependencies]
clap-mcp = { version = "0.0.4-rc.1", features = ["tracing"] }
```

## Documentation

Every guide in [`docs/`](docs/) is listed below. See also
[examples/README.md](examples/README.md) for runnable binaries.

### Embedder guides

| Guide | Topics |
| --- | --- |
| [Custom resources and prompts](docs/custom-content.md) | `ClapMcpServeOptions`, static/dynamic content |
| [Exporting agent skills](docs/export-skills.md) | `--export-skills`, SKILL.md generation |
| [Execution safety](docs/execution-safety.md) | `reinvocation_safe`, skip/requires, dual derive, async embedders |
| [MCP tasks support](docs/mcp-tasks.md) | Task-augmented `tools/call`, examples, support matrix |
| [Stateful MCP tools](docs/stateful-tools.md) | Shared session state, `parse_or_serve_mcp_with_state` |
| [Security](docs/security.md) | Schema validation, subprocess model, trust boundaries |
| [Tool output](docs/tool-output.md) | `run` return types, structured output, `output-schema` |
| [Logging](docs/logging.md) | `tracing` / `log` bridges, MCP notifications |
| [Streamable HTTP](docs/http.md) | `--mcp-http`, listen env vars |
| [OAuth (scaffolding)](docs/oauth.md) | Remote MCP client helpers |
| [rmcp migration notes](docs/rmcp-migration-notes.md) | API changes, breaking renames |

### Maintainer notes

| Guide | Topics |
| --- | --- |
| [Conformance baseline](docs/conformance-baseline.md) | `cargo xtask conformance`, baseline YAML |

## CLI compatibility

For derive usage, `use clap_mcp::ClapMcp` so you can write `#[derive(ClapMcp)]`.
The examples below are the fastest path to a working MCP-enabled CLI.

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

Use `#[clap_mcp(...)]` to declare execution safety (see
[Execution safety](docs/execution-safety.md)), and
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

See [Dual derive (root + subcommand)](docs/execution-safety.md#dual-derive-root-subcommand) and
**struct_subcommand_required** in [examples/README.md](examples/README.md).

## Development

Contributors should follow these conventions. AI agents should also read
[AGENTS.md](AGENTS.md) for design priorities and doc touchpoints.

* Format code with `cargo fmt`. CI runs `cargo fmt --all -- --check`.
* Run `cargo clippy --all-targets --all-features -- -D warnings` before
  submitting; CI enforces this.
* Document public API items and add a `// SAFETY:` comment above any `unsafe`
  block explaining invariants.

MCP task support matrix (including limitations) is in
[MCP tasks support](docs/mcp-tasks.md).

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

For an HTML report:

```bash
cargo xtask code-coverage-html
```

Add `--open` to launch the report in a browser when it finishes:

```bash
cargo xtask code-coverage-html --open
```

Coverage focuses on the `clap-mcp` and `clap-mcp-macros` crates; the `examples`
crate is excluded from coverage targets.

Release prep runs example smoke via `cargo xtask examples-help` (builds with
`--all-features`, runs `--help` on each release-validation binary); see
[examples/README.md](examples/README.md).
