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
clap-mcp = "0.0.4-rc.1"
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
**struct_subcommand_globals** (struct `run` with globals) in
[examples/README.md](../examples/README.md).

## Related guides

| Topic | Guide |
| --- | --- |
| Passthrough (`--`), renaming `--mcp` | [Execution safety — CLI compatibility details](execution-safety.md#cli-compatibility-details) |
| Skip/requires, dual derive, async | [Execution safety](execution-safety.md) |
| Stateful session tools | [Stateful MCP tools](stateful-tools.md) |
| MCP task-augmented `tools/call` | [MCP tasks support](mcp-tasks.md) |
| Logging to MCP clients | [Logging](logging.md) |
| Streamable HTTP | [HTTP](http.md) |
