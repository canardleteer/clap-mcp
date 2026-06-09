# OAuth client helpers (scaffolding)

> Guide for CLI authors adding clap-mcp. See [README](../README.md) to get started.

[← Documentation index](../README.md#documentation)

The `http-oauth` Cargo feature is scaffolding. API and behavior may change; it
is not a release parity target. See the Feature Flags table in
[README](../README.md#feature-flags).

## Why this may not ship

We have not found a concrete use case for this feature in a clap-mcp-shaped CLI.
In practice, agents are the main callers of MCP. For agent-side or
agent-to-agent integration we would reach for ACP or A2A, not a Rust CLI binary
wired as an OAuth MCP HTTP client. What exists today is mostly an rmcp
re-export spike. We may drop `http-oauth` in a future release if no concrete need
emerges. Do not build production flows on it without pinning and an exit plan.

clap-mcp turns local CLIs into MCP servers (stdio or loopback HTTP). Serving MCP
does not require OAuth. clap-mcp does not ship an authorization server and does
not OAuth-protect inbound access to your MCP server.

## When to enable `http-oauth`

Enable the feature when this binary is an **MCP client** calling a **remote
HTTP MCP server** that requires OAuth (discovery, PKCE, token refresh via rmcp).
That is what the re-exports target: `AuthClient` and
`StreamableHttpClientTransport` for MCP-over-HTTP client connections.

Do **not** enable `http-oauth` merely because your CLI also serves local tools
with `--mcp` — serving and client OAuth are separate concerns in the same binary.

## What `http-oauth` is not

| Goal | Use `http-oauth`? |
| --- | --- |
| MCP client → remote MCP server (OAuth) | Yes (scaffolding) |
| MCP server tools call an **external** OAuth-gated API (GitHub, your REST service, …) | **No** — handle tokens in your `run` / tool logic with a general OAuth or HTTP client crate (`oauth2`, `reqwest`, …). This feature wires rmcp's MCP client transport, not arbitrary APIs. |
| Require OAuth before clients may call **your** MCP server | **No** — not implemented; see [http.md](http.md) for loopback hardening only. |

Serving MCP and calling external OAuth-gated APIs in tool code is a normal
pattern; you do not need `http-oauth` for the latter.

```toml
clap-mcp = { version = "0.0.4", features = ["derive", "http-oauth"] }
```

(`http-oauth` implies `http` and rmcp's auth + streamable HTTP client
transport.)

Re-exports: `clap_mcp::oauth::AuthClient`, `StreamableHttpClientTransport`.
See [OAuth in rmcp](https://github.com/modelcontextprotocol/rust-sdk/blob/main/docs/OAUTH_SUPPORT.md)
and [client examples](https://github.com/modelcontextprotocol/rust-sdk/tree/main/examples/clients).
Sketch:
[`examples/clients/oauth_http_client.rs`](../examples/clients/oauth_http_client.rs).

## Environment variables

Load client settings with
[`clap_mcp::oauth::EnvConfig::from_env`](https://docs.rs/clap-mcp/latest/clap_mcp/oauth/struct.EnvConfig.html):

| Variable | Required | Purpose |
|----------|----------|---------|
| `CLAP_MCP_OAUTH_ISSUER` | yes | OIDC issuer / discovery base URL |
| `CLAP_MCP_OAUTH_CLIENT_ID` | yes | OAuth client id |
| `CLAP_MCP_OAUTH_REDIRECT_URI` | yes | Callback URL for desktop/CLI flows |
| `CLAP_MCP_OAUTH_CLIENT_SECRET` | no | Omit for public PKCE clients |
| `CLAP_MCP_OAUTH_SCOPES` | no | Space-separated scopes; default `openid profile` |

```rust
let cfg = clap_mcp::oauth::EnvConfig::from_env()?;
let oauth_cfg = cfg.oauth_client_config();
// Wire cfg.issuer + oauth_cfg into rmcp AuthClient / discovery (see OAUTH_SUPPORT.md in rust-sdk).
```

Constants: `clap_mcp::oauth::OAUTH_ISSUER_ENV`, `OAUTH_CLIENT_ID_ENV`, etc.

Browser PKCE flows are manual to run end-to-end; the example above only loads
and validates env-based client config.
