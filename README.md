# clap-mcp

> **Enrich your Rust CLI with MCP Capabilities**

## Usage

You can take a look at the examples, but this is a **VERY** early draft.

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

Overall, the more you design your service around a service pattern, the more
naturally this crate will behave as an MCP server, and modern CLIs often do
that. At the same time, we shouldn't force CLIs that don't do that, out of the
ecosystem.

## Execution safety configuration

CLIs may differ in how safely they can be invoked over MCP:

- **Reinvocation safety**: When `false` (default), each tool call spawns a fresh subprocess. When `true`, uses in-process execution (no subprocess).
- **Parallel safety**: When `false` (default), tool calls are serialized. When `true`, they may run concurrently.

### Attribute-based config (recommended)

Use `#[derive(ClapMcp)]` and `#[clap_mcp(...)]` on your CLI type:

```rust
#[derive(Debug, Parser, clap_mcp::ClapMcp)]
#[clap_mcp(parallel_safe = false, reinvocation_safe)]
#[command(...)]
enum Cli {
    #[clap_mcp_output = "format!(\"{}\", a + b)"]
    Add { a: i32, b: i32 },
    // ...
}

let cli = clap_mcp::parse_or_serve_mcp_attr::<Cli>();
```

When `reinvocation_safe` is true, add `#[clap_mcp_output = "expr"]` on variants to specify the output string for in-process execution. Omitted variants default to `format!("{:?}", self)`.

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
