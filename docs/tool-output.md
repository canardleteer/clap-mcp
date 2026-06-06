# Tool output attributes

> Embedder guide for clap-mcp. See [README](../README.md) for getting started.

[← Documentation index](../README.md#documentation)

When using `#[derive(ClapMcp)]`, you control how each subcommand's output is
returned to MCP clients. The **idiomatic** approach is a **single output
function**
(`#[clap_mcp_output_from = "run"]`): one `run` implements both CLI and MCP
behavior, so you
avoid duplicating logic. Per-variant attributes are available for edge cases but
are not
the default.

## `#[clap_mcp_output_from = "run"]` — single output function (recommended)

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

## `ClapMcpServeOptions::capture_stdout`

When `true` and running in-process, captures stdout written during tool
execution
and merges it with Text output. Only has effect when `reinvocation_safe = true`
(in-process execution). **Unix only** — the field is not present on Windows, so
code that sets `capture_stdout` will not compile on Windows. Subprocess mode
already captures stdout via `Command::output()`.

With the **`output-schema`** feature enabled, you can attach a JSON schema to
each tool's
`outputSchema` field so MCP clients know the shape of the tool's output.

## `#[clap_mcp_output_type = "TypeName"]`

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

## `#[clap_mcp_output_one_of = "T1, T2, T3"]`

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
