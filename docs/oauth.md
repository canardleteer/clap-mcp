# OAuth and clap-mcp HTTP

clap-mcp CLIs expose tools over MCP. **stdio (`--mcp`) has no OAuth story** — there is no remote HTTP endpoint to protect.

For **Streamable HTTP** (`--mcp-http`, optional `http` feature):

- clap-mcp serves plain MCP on `/mcp` (loopback-oriented defaults).
- **Production auth** is usually a reverse proxy, API gateway, or Bearer middleware in front of the server process.
- clap-mcp does **not** embed an OAuth authorization server.

## OAuth **client** flows (optional `http-oauth` feature)

rmcp ships **OAuth client** support (discovery, PKCE, token refresh) for calling **remote** MCP servers. Enable on `clap-mcp`:

```toml
clap-mcp = { version = "...", features = ["derive", "http", "http-oauth"] }
```

Re-exports live in `clap_mcp::oauth` (`AuthClient`, `StreamableHttpClientTransport`). See rmcp `docs/OAUTH_SUPPORT.md` and `examples/clients` in the rust-sdk repo.

## CI / automated tests

Full browser PKCE flows are **manual**. Automated smoke tests should use a Bearer-token fixture server (see rmcp `simple_auth_streamhttp` patterns), not live OAuth AS interaction.
