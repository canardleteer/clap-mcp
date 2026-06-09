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
clap-mcp = "0.0.4"
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
| Passthrough (`--`), renaming `--mcp` | [Execution safety — CLI compatibility details](execution-safety.md#cli-compatibility-details) |
| Skip/requires, dual derive, async | [Execution safety](execution-safety.md) |
| Stateful session tools | [Stateful MCP tools](stateful-tools.md) |
| MCP task-augmented `tools/call` | [MCP tasks support](mcp-tasks.md) |
| Logging to MCP clients | [Logging](logging.md) |
| Streamable HTTP | [HTTP](http.md) |
