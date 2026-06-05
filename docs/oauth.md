# OAuth and clap-mcp HTTP

clap-mcp CLIs expose tools over MCP. **stdio (`--mcp`) has no OAuth story** — there is no remote HTTP endpoint to protect.

For **Streamable HTTP** (`--mcp-http`, optional `http` feature):

- clap-mcp serves plain MCP on `/mcp` (loopback-oriented defaults). See [http.md](http.md) for listen configuration and env vars.
- **Production auth** is usually a reverse proxy, API gateway, or Bearer middleware in front of the server process.
- clap-mcp does **not** embed an OAuth authorization server.

## OAuth **client** flows (optional `http-oauth` feature)

rmcp ships **OAuth client** support (discovery, PKCE, token refresh) for calling **remote** MCP servers. Enable on `clap-mcp`:

```toml
clap-mcp = { version = "0.0.4-rc.1", features = ["derive", "http", "http-oauth"] }
```

Re-exports live in `clap_mcp::oauth` (`AuthClient`, `StreamableHttpClientTransport`). See rmcp `docs/OAUTH_SUPPORT.md` and `examples/clients` in the rust-sdk repo.

### Environment variables (client config)

For CLIs that call OAuth-protected **remote** MCP servers, load client settings from the environment via [`clap_mcp::oauth::EnvConfig::from_env`](https://docs.rs/clap-mcp/latest/clap_mcp/oauth/struct.EnvConfig.html):

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
// Wire cfg.issuer + oauth_cfg into rmcp AuthClient / discovery (see rmcp docs).
```

Constants for embedders: `clap_mcp::oauth::OAUTH_ISSUER_ENV`, `OAUTH_CLIENT_ID_ENV`, etc.

## CI / automated tests

Full browser PKCE flows are **manual**. Automated smoke tests should use a Bearer-token fixture server (see rmcp `simple_auth_streamhttp` patterns), not live OAuth AS interaction.
