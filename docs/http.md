# Streamable HTTP embedder guide

clap-mcp can serve MCP over **Streamable HTTP** (optional `http` feature)
instead of stdio. This document is for authors embedding clap-mcp in another
CLI or service binary.

## Enable the feature

```toml
clap-mcp = { version = "0.0.4-rc.1", features = ["derive", "http"] }
```

Derive `ClapMcp` on your CLI type and call
[`ParseOrServeMcp::parse_or_serve_mcp`] or
[`parse_or_serve_mcp_with`](https://docs.rs/clap-mcp/latest/clap_mcp/fn.parse_or_serve_mcp_with.html).
clap-mcp adds global flags `--mcp` (stdio) and `--mcp-http` (HTTP).

## Listen address resolution

Precedence (first match wins):

1. `--mcp-http=HOST:PORT` or `--mcp-http HOST:PORT`
2. **`--mcp-http` alone** → environment (see below)
3. `CLAP_MCP_HTTP_LISTEN=HOST:PORT`
   ([`MCP_HTTP_LISTEN_ENV`](https://docs.rs/clap-mcp/latest/clap_mcp/constant.MCP_HTTP_LISTEN_ENV.html))
4. `CLAP_MCP_HTTP_BIND` + `CLAP_MCP_HTTP_PORT` together when
   `CLAP_MCP_HTTP_LISTEN` is unset

There is **no silent default port**. If HTTP mode is requested without a
resolvable address, the process exits with an error listing the options above.

### Examples

```bash
export CLAP_MCP_HTTP_LISTEN=127.0.0.1:8080
mycli --mcp-http

export CLAP_MCP_HTTP_BIND=127.0.0.1
export CLAP_MCP_HTTP_PORT=8080
mycli --mcp-http

mycli --mcp-http 0.0.0.0:9000
```

### Coexistence with host CLI env vars

clap-mcp only reads **`CLAP_MCP_*`** variables. If your application already
uses `APP_PORT` or similar, read those in `main` and pass an explicit address:

```bash
mycli --mcp-http "127.0.0.1:${APP_PORT}"
```

## Server behavior

| Topic | Behavior |
|-------|----------|
| Route | `/mcp` |
| Mutual exclusion | `--mcp` and `--mcp-http` cannot be combined |
| Before subcommand | When `allow_mcp_without_subcommand` is true (default), MCP flags work without a subcommand — same as stdio |
| DNS rebinding | Loopback-oriented `allowed_hosts` are applied; public binds (`0.0.0.0`) need reverse-proxy hardening |
| Tokio runtime | When `reinvocation_safe` and (`share_runtime` or `parallel_safe`), embedders need a multi-thread runtime for [`ServeMcpBuilder::serve`] or clap-mcp creates one for [`ServeMcpBuilder::serve_blocking`] |

## Low-level embed API

For servers that build schema JSON without the derive path, use
[`ServeMcpBuilder`] from `#[tokio::main]` or [`ServeMcpBuilder::serve_blocking`]
from sync `main`. Lower-level [`serve_mcp`] / [`serve_mcp_blocking`] free
functions delegate to the builder. See also the **Async embedders** section in
[README.md](../README.md).

**Async (`#[tokio::main]`):**

```rust
use clap_mcp::{ServeMcpBuilder, McpListen, ClapMcpServeOptions};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), clap_mcp::ClapMcpError> {
    ServeMcpBuilder::new()
        .listen(McpListen::Http("127.0.0.1:8080".parse()?))
        .schema_json(schema_json)
        .config(config)
        .metadata(metadata)
        .executable_path(executable_path)
        .in_process_handler(in_process_handler)
        .serve_options(ClapMcpServeOptions::default())
        .serve()
        .await
}
```

**Blocking (sync `main`):**

```rust
use clap_mcp::{ServeMcpBuilder, McpListen, ClapMcpServeOptions};

ServeMcpBuilder::for_cli::<Cli>(McpListen::Http("127.0.0.1:8080".parse()?))
    .serve_options(ClapMcpServeOptions::default())
    .serve_blocking()?;
```

## OAuth (server vs client)

HTTP **server** auth is out of scope for clap-mcp — use a reverse proxy or
middleware. OAuth **client** helpers for calling remote MCP servers live under
the `http-oauth` feature; see [oauth.md](oauth.md).

## Conformance

Maintainers: `cargo xtask conformance` runs the official MCP conformance
harness against the `subcommands_http` example. See
[conformance-baseline.md](conformance-baseline.md).
