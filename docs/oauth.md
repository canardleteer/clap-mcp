# OAuth client (optional)

clap-mcp turns **local** CLIs into MCP servers (stdio or loopback HTTP). That
path has no OAuth story — there is nothing to authorize on the server side, and
clap-mcp does not ship an authorization server.

The optional **`http-oauth`** feature is only for **clients**: CLIs or tools
that call a **remote** MCP server over HTTP when that server requires OAuth
(discovery, PKCE, token refresh via rmcp). Enable it when your binary acts as an
MCP client, not when it is serving local tools.

```toml
clap-mcp = { version = "0.0.4-rc.1", features = ["derive", "http-oauth"] }
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
