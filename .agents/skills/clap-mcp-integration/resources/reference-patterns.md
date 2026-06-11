# Integration patterns

Read when wiring derives, dispatch, and entrypoints.

## Shared dispatch (idiomatic)

Extract command logic once:

```rust
pub fn run_app() -> Result<ExitCode, Box<dyn Error>> {
    #[cfg(feature = "mcp")]
    let args = Args::parse_or_serve_mcp();
    #[cfg(not(feature = "mcp"))]
    let args = Args::parse();

    let result = execute(args.cmd)?;
    print!("{}", args.out.format_result(&result)?);
    Ok(outcome_from(result))
}

#[cfg(feature = "mcp")]
fn run(cmd: Commands) -> Result<clap_mcp::AsStructured<McpToolOutput>, AppError> {
    Ok(clap_mcp::AsStructured(wrap_for_mcp(execute(cmd)?)))
}

pub fn execute(cmd: Commands) -> Result<SubcommandResult, AppError> {
    match cmd { /* … */ }
}
```

CLI prints formatted output; MCP `run` returns structured JSON via `AsStructured`.

## Preserve CLI parse

When invalid human argv should use native clap Usage formatting:

```rust
#[cfg(feature = "mcp")]
let args = Args::parse_or_serve_mcp_preserve_cli();
```

See [usage.md — Preserve CLI parse](../../../../docs/usage.md#preserve-cli-parse) and **preserve_cli_parse** in [examples/README.md](../../../../examples/README.md).

## Setup then serve

When argv must be fully parsed and setup run before MCP (for example `--config`):

```rust
#[cfg(feature = "mcp")]
fn run_mcp_server() -> Result<(), clap_mcp::ClapMcpError> {
    clap_mcp::ServeMcpBuilder::for_cli::<App>(clap_mcp::McpListen::Stdio)
        .serve_options(serve_options())
        .serve_blocking()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = App::parse();
    load_config(&app.config)?;
    match app.command {
        #[cfg(feature = "mcp")]
        Commands::Serve => run_mcp_server()?,
        cmd => println!("{}", run_app(App { /* fields */, command: cmd })),
    }
    Ok(())
}
```

Do not call `parse_or_serve_mcp*` on this path. See [usage.md — Setup then serve](../../../../docs/usage.md#setup-then-serve-embedder) and **setup_then_serve** in [examples/README.md](../../../../examples/README.md).

## Custom serve options

For logging bridges, stdout capture, or async embedders:

- Vanilla CLI: `parse_or_serve_mcp_with(ClapMcpRunOptions { .. })`.
- Embedder / `#[tokio::main]`: [`ServeMcpBuilder`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ServeMcpBuilder.html) with `.serve_options(...)`.

See [logging.md](../../../../docs/logging.md) and [execution-safety.md — Async embedders](../../../../docs/execution-safety.md#async-embedders).

## Struct root + required subcommand

```rust
#[derive(Parser, Debug)]
#[cfg_attr(feature = "mcp", derive(ClapMcp))]
#[cfg_attr(feature = "mcp", clap_mcp(skip_root_when_subcommands))]
#[command(version, about)]
pub struct Args {
    #[command(subcommand)]
    pub cmd: Commands,

    #[cfg_attr(feature = "mcp", clap_mcp(skip))]
    #[arg(long, short = 'o', value_enum, default_value_t = OutputFormat::Yaml)]
    pub out: OutputFormat,
}

#[derive(Subcommand, Debug, Clone)]
#[cfg_attr(feature = "mcp", derive(ClapMcp))]
#[cfg_attr(feature = "mcp", clap_mcp(reinvocation_safe, parallel_safe))]
#[cfg_attr(feature = "mcp", clap_mcp_output_from = "run")]
#[cfg_attr(feature = "mcp", clap_mcp_output_type = "McpToolOutput")]
pub enum Commands { /* … */ }
```

## Flat enum root

When the CLI is already an enum root, put all attributes on the enum:

```rust
#[derive(Parser, ClapMcp)]
#[clap_mcp(reinvocation_safe, parallel_safe)]
#[clap_mcp_output_from = "run"]
enum Cli { /* variants */ }
```

## Nested subcommands

Derive `ClapMcp` on each level; use `#[clap_mcp(schema_only)]` on intermediate enums when `output_from` lives on the root or leaf executor. clap-mcp merges metadata across nested enums — prefer derive over manual `merge_from` unless compile errors force imperative metadata.

See [supported-cli-shapes.md](../../../../docs/supported-cli-shapes.md) and **nested_subcommands** in [examples/README.md](../../../../examples/README.md).

## `IntoClapMcpToolError`

Implement once for the application error type:

```rust
#[cfg(feature = "mcp")]
impl IntoClapMcpToolError for AppError {
    fn into_tool_error(self) -> ClapMcpToolError {
        ClapMcpToolError::text(self.to_string())
    }
}
```

## MCP wrapper type

When CLI exit semantics differ from MCP success (e.g. validate/compare):

```rust
#[cfg(feature = "mcp")]
#[derive(Serialize, JsonSchema)]
struct McpToolOutput {
    ok: bool,
    result: SubcommandResult,
}
```

## Imperative entry

For hand-built `clap::Command`:

```rust
let matches = clap_mcp::get_matches_or_serve_mcp(cmd);
```

Populate `ClapMcpSchemaMetadata` for skip/requires/serialized when not using derive.

Required for **custom flag parsers** (ripgrep-style) until a thin clap wrapper exists.

## HTTP MCP

With `features = ["http"]` on clap-mcp, `--mcp-http <addr>` is injected by derive. Document env overrides in README: `CLAP_MCP_HTTP_LISTEN`, `CLAP_MCP_HTTP_BIND`, `CLAP_MCP_HTTP_PORT`. See [http.md](../../../../docs/http.md).

## `--export-skills`

Derive injects `--export-skills <dir>`. Document in README for Cursor agent skill export; no extra integrator code unless renaming via `export_skills_flag`. See [export-skills.md](../../../../docs/export-skills.md).
