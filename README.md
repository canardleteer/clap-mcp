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

- **Reinvocation safety**: Some CLIs can be called multiple times without tearing down the process; others require a fresh process per call (the default).
- **Parallel safety**: Some CLIs use lock files or shared state and must not run concurrently with other tool calls.

Use `ClapMcpConfig` with `parse_or_serve_mcp_with_config` or `get_matches_or_serve_mcp_with_config`:

```rust
clap_mcp::parse_or_serve_mcp_with_config::<Cli>(clap_mcp::ClapMcpConfig {
    reinvocation_safe: true,   // reserves future in-process execution
    parallel_safe: false,      // serialize tool calls (e.g. for lock files)
    ..Default::default()
})
```

When `parallel_safe` is false, tool calls are serialized. Tools include `meta.clapMcp` with these hints for clients.
