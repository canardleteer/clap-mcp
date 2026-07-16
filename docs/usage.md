# Usage patterns

> Guide for CLI authors adding clap-mcp. See [README](../README.md) to get started.

[← Documentation index](../README.md#documentation)

These are the three most common ways to add MCP to a Rust CLI. Each pattern
keeps normal CLI behavior unless you pass **`--mcp`** (stdio) or
**`--mcp-http`** ([`http`](http.md) feature). See
[CLI compatibility](../README.md#cli-compatibility) for opt-in rules and
subcommand behavior.

Add `clap-mcp` with the default `derive` feature:

```toml
[dependencies]
clap-mcp = "0.1.0-rc.1"
clap = "4"
```

For derive paths, `use clap_mcp::ClapMcp` so you can write `#[derive(ClapMcp)]`.
Runnable binaries: [examples/servers](../examples/servers/).

## Derive with attributes (recommended)

Use when your CLI can run **in-process** for MCP tool calls — typically a
service-shaped CLI with a shared `run` function and no fragile global state.
Declare execution config with `#[clap_mcp(...)]`; `parse_or_serve_mcp` picks it
up automatically.

```rust
use clap::Parser;
use clap_mcp::ClapMcp;

#[derive(Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe)]
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

Next steps: [`parallel_safe`, `share_runtime`, panics, async tools](execution-safety.md);
[`run` return types and structured output](tool-output.md).

## Derive (minimal)

Use when you want MCP with **no extra attributes** — each tool call spawns a
fresh subprocess of your binary (safest default). Swap `Cli::parse()` for
`Cli::parse_or_serve_mcp()` and add `#[clap_mcp_output_from = "run"]`.

```rust
use clap::Parser;
use clap_mcp::ClapMcp;

#[derive(Parser, ClapMcp)]
#[clap_mcp_output_from = "run"]
#[command(name = "myapp")]
enum Cli {
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

Next steps: [Execution safety — subprocess vs in-process](execution-safety.md);
[Security — subprocess trust model](security.md).

## Imperative (existing clap CLI)

Use when you already build a `clap::Command` by hand. Replace `get_matches()`
with [`get_matches_or_serve_mcp`](https://docs.rs/clap-mcp/latest/clap_mcp/fn.get_matches_or_serve_mcp.html).
When `--mcp` is absent, behavior is unchanged.

```rust
use clap::Command;

fn main() {
    let cmd = Command::new("myapp")
        .subcommand(Command::new("hello").about("Say hello"));

    let matches = clap_mcp::get_matches_or_serve_mcp(cmd);
    if let Some((_, sub)) = matches.subcommand() {
        match sub.name() {
            "hello" => println!("Hello!"),
            _ => {}
        }
    }
}
```

Next steps: [Execution safety — runtime config and schema metadata](execution-safety.md);
[`ServeMcpBuilder` for async embedders](execution-safety.md#async-embedders).

## Setup then serve (embedder)

Use when your application must **parse argv and run setup before MCP starts** —
for example loading `--config`, initializing shared state, or branching on a
custom `serve` subcommand instead of clap-mcp's builtin `--mcp` flag.

Do **not** use [`parse_or_serve_mcp`](https://docs.rs/clap-mcp/latest/clap_mcp/trait.ParseOrServeMcp.html)
or [`get_matches_or_serve_mcp`](https://docs.rs/clap-mcp/latest/clap_mcp/fn.get_matches_or_serve_mcp.html)
for this path. Those entrypoints inspect argv for MCP flags before your normal
parse runs, which is right for vanilla CLIs but wrong when globals or config
must be applied first.

Pattern:

1. Parse with `Cli::parse()` (or your imperative `get_matches()`).
2. Run setup (read config, init logging, and similar).
3. On your MCP branch, call
   [`ServeMcpBuilder::for_cli`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ServeMcpBuilder.html#method.for_cli)
   (derive CLI) or [`ServeMcpBuilder::new`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ServeMcpBuilder.html#method.new)
   (hand-built schema), then `.serve().await` or `.serve_blocking()`.

You do not need [`command_with_mcp_flag`](https://docs.rs/clap-mcp/latest/clap_mcp/fn.command_with_mcp_flag.html)
unless you want clap-mcp's builtin `--mcp` on the CLI.

```rust
use clap::{Parser, Subcommand};
use clap_mcp::{ClapMcp, McpListen, ServeMcpBuilder};
use std::path::PathBuf;

#[derive(Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe)]
#[clap_mcp_output_from = "run_app"]
#[command(name = "myapp", subcommand_required = true)]
struct App {
    #[arg(long, global = true)]
    config: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, ClapMcp)]
#[clap_mcp(schema_only)]
enum Commands {
    /// Start MCP (embedder entry; not an MCP tool).
    #[clap_mcp(skip)]
    Serve,

    Ping,
}

fn run_app(app: App) -> String {
    match app.command {
        Commands::Serve => unreachable!("handled in main"),
        Commands::Ping => "pong".to_string(),
    }
}

fn main() -> Result<(), clap_mcp::ClapMcpError> {
    let app = App::parse();
    load_config(&app.config)?;

    match app.command {
        Commands::Serve => {
            ServeMcpBuilder::for_cli::<App>(McpListen::Stdio).serve_blocking()?;
        }
        _ => println!("{}", run_app(app)),
    }
    Ok(())
}

fn load_config(_path: &PathBuf) -> Result<(), clap_mcp::ClapMcpError> {
    Ok(())
}
```

For async embedders, replace `.serve_blocking()` with `.serve().await` under
`#[tokio::main]`. Pass logging or custom resources via
[`ServeMcpBuilder::serve_options`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ServeMcpBuilder.html#method.serve_options)
instead of [`parse_or_serve_mcp_with`](https://docs.rs/clap-mcp/latest/clap_mcp/fn.parse_or_serve_mcp_with.html).
Stateful CLIs can use
[`ServeMcpBuilder::for_cli_with_state`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ServeMcpBuilder.html#method.for_cli_with_state)
after `parse` and setup; see [Stateful MCP tools](stateful-tools.md).

Runnable demos: **setup_then_serve**, **async_embedder_serve**, and
**placeholder_server** in [examples/README.md](../examples/README.md).

### Custom stdio transport

By default, [`McpListen::Stdio`](https://docs.rs/clap-mcp/latest/clap_mcp/enum.McpListen.html)
uses process stdin/stdout. Embedders that multiplex MCP over an existing
JSON-RPC channel (socket pair, pipe, or in-process duplex) can pass a custom
async read/write pair:

```rust
use clap_mcp::{McpListen, ServeMcpBuilder};
use tokio::io::{AsyncRead, AsyncWrite};

async fn serve_on_channel<R, W>(read: R, write: W) -> Result<(), clap_mcp::ClapMcpError>
where
    R: AsyncRead + Send + Unpin + 'static,
    W: AsyncWrite + Send + Unpin + 'static,
{
    ServeMcpBuilder::for_cli::<App>(McpListen::Stdio)
        .stdio_io(read, write)
        .serve()
        .await
}
```

[`ServeMcpBuilder::stdio_io`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ServeMcpBuilder.html#method.stdio_io)
is only valid with `McpListen::Stdio` (not HTTP). The lower-level
[`serve_mcp`](https://docs.rs/clap-mcp/latest/clap_mcp/fn.serve_mcp.html) free
functions always use process stdio.

## Preserve CLI parse

`parse_or_serve_mcp` and `parse_or_serve_mcp_with` use clap-mcp's argv
preflight and MCP entry detection before your normal `Parser` path runs. That
is required for `--mcp`, `--mcp-http`, and `--export-skills`, but the error
text on invalid **human CLI** argv can differ from `Cli::parse()` alone (clap's
native Usage formatting), especially when `FromArgMatches` adds custom validation.

When native CLI error UX matters, use a preserve-cli entrypoint. It runs
`Cli::parse()` on normal argv and clap-mcp's augmented path only when argv
(before `--`) contains a builtin MCP, HTTP, or export-skills flag:

```rust
use clap::Parser;
use clap_mcp::{ClapMcp, ParseOrServeMcp};

#[derive(Parser, ClapMcp)]
#[clap_mcp_output_from = "run"]
#[command(name = "myapp")]
enum Cli {
    Ping,
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::Ping => "pong".to_string(),
    }
}

fn main() {
    let cli = Cli::parse_or_serve_mcp_preserve_cli();
    println!("{}", run(cli));
}
```

Derive: [`ParseOrServeMcp::parse_or_serve_mcp_preserve_cli`],
[`parse_or_serve_mcp_preserve_cli_with`]. Imperative:
[`get_matches_preserve_cli_or_serve_mcp`]. Detection only:
[`argv_contains_clap_mcp_flags`], [`argv_before_end_of_opts`].

> [!NOTE]
> Integrator policy: see [Execution safety — Integrator policy](execution-safety.md#integrator-policy).

Runnable demo: **preserve_cli_parse** in [examples/README.md](../examples/README.md).

## Split parse (manual)

If you need custom branching (for example renamed builtin flags combined with
other argv logic), detect clap-mcp entry with [`argv_contains_clap_mcp_flags`]
and [`ClapMcpConfig::builtin_flags`] from your derive config:

```rust
use clap::Parser;
use clap_mcp::{ClapMcp, ClapMcpConfigProvider, ParseOrServeMcp};

#[derive(Parser, ClapMcp)]
#[clap_mcp_output_from = "run"]
#[command(name = "myapp")]
enum Cli {
    Ping,
}

fn run(cmd: Cli) -> String {
    match cmd {
        Cli::Ping => "pong".to_string(),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flags = Cli::clap_mcp_config().builtin_flags;
    if clap_mcp::argv_contains_clap_mcp_flags(&args, &flags) {
        let cli = Cli::parse_or_serve_mcp();
        println!("{}", run(cli));
    } else {
        let cli = Cli::parse();
        println!("{}", run(cli));
    }
}
```

Imperative CLIs can call
[`get_matches_preserve_cli_or_serve_mcp`](https://docs.rs/clap-mcp/latest/clap_mcp/fn.get_matches_preserve_cli_or_serve_mcp.html)
instead of hand-rolling the branch.

## Struct root with subcommand

See [Supported CLI shapes](supported-cli-shapes.md) for a pattern matrix and
example binaries.

Common for CLIs that use a struct root and `#[command(subcommand)]`. Derive
`ClapMcp` on **both** the root struct and the subcommand enum; keep required
subcommands — do not switch to `Option<Commands>` only for MCP.

When `run` must see **global root flags** or other root fields, put
`#[clap_mcp_output_from = "run_cli"]` on the struct and use
`#[clap_mcp(schema_only)]` on nested subcommand enums. Otherwise put
`#[clap_mcp_output_from = "run"]` on the subcommand enum and delegate from the
struct (default).

Full pattern and compilable example:
[Execution safety — Dual derive](execution-safety.md#dual-derive--root-and-subcommand).
Runnable binaries: **struct_subcommand_required** (subcommand `run`),
**struct_subcommand_globals** (struct `run` with globals), **flat_struct_root**
(single wide tool, no subcommand), **flatten_skip** (skip flatten + serialize_topic),
**flatten_subcommand_skip_flat** / **flatten_subcommand_skip_nested** (skip flattened
`Subcommand`) in [examples/README.md](../examples/README.md).

## Related guides

| Topic | Guide |
| --- | --- |
| Setup then serve (embedder) | [This guide — Setup then serve](#setup-then-serve-embedder) |
| Passthrough (`--`), renaming `--mcp` | [Execution safety — CLI compatibility details](execution-safety.md#cli-compatibility-details) |
| Skip/requires, dual derive, async | [Execution safety](execution-safety.md) |
| Stateful session tools | [Stateful MCP tools](stateful-tools.md) |
| MCP task-augmented `tools/call` | [MCP tasks support](mcp-tasks.md) |
| Logging to MCP clients | [Logging](logging.md) |
| Streamable HTTP | [HTTP](http.md) |
