# Changelog

All notable changes to this project will be documented in this file.

## [0.0.4-rc.1] - 2026-06-04

### Breaking

- **API slim (derive path):** Removed `parse_or_serve_mcp`, `parse_or_serve_mcp_attr`, `parse_or_serve_mcp_with_config`, and `parse_or_serve_mcp_with_config_and_options`. Use [`ParseOrServeMcp::parse_or_serve_mcp`](https://docs.rs/clap-mcp/latest/clap_mcp/trait.ParseOrServeMcp.html) or [`parse_or_serve_mcp_with`](https://docs.rs/clap-mcp/latest/clap_mcp/fn.parse_or_serve_mcp_with.html) with [`ClapMcpRunOptions`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapMcpRunOptions.html).
- **Tools builder:** Removed `tools_from_schema` and `tools_from_schema_with_config`. Use [`tools_from_schema_with_metadata`](https://docs.rs/clap-mcp/latest/clap_mcp/fn.tools_from_schema_with_metadata.html).
- **Serve API:** Removed public `serve_schema_json_over_stdio`, `serve_schema_json_over_stdio_blocking`, `serve_schema_json_over_http`, and `serve_schema_json_over_http_blocking`. Use [`serve_mcp_blocking`](https://docs.rs/clap-mcp/latest/clap_mcp/fn.serve_mcp_blocking.html) with [`McpListen`](https://docs.rs/clap-mcp/latest/clap_mcp/enum.McpListen.html).
- **Task augmentation config:** Removed `ClapMcpConfig::task_augmented_tools`. Enable with `#[clap_mcp(task_augmented_tools)]` (sets [`ClapMcpSchemaMetadata::task_augmented_tools`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapMcpSchemaMetadata.html#structfield.task_augmented_tools)); imperative servers set metadata manually.
- **Removed public `tool_task_eligible`** and **`ClapMcpServer` / `build_clap_mcp_server`** (now crate-internal).

**Unchanged:** `get_matches_or_serve_mcp`, `get_matches_or_serve_mcp_with_config`, and `get_matches_or_serve_mcp_with_config_and_metadata`.

### Added

- **Streamable HTTP** (`http` feature): `--mcp-http`, env vars `CLAP_MCP_HTTP_LISTEN`, `CLAP_MCP_HTTP_BIND`, `CLAP_MCP_HTTP_PORT`; embedder guide [docs/http.md](docs/http.md).
- **OAuth client env** (`http-oauth` feature): [`clap_mcp::oauth::EnvConfig`](https://docs.rs/clap-mcp/latest/clap_mcp/oauth/struct.EnvConfig.html); [docs/oauth.md](docs/oauth.md).
- **Elicitation** (`elicitation` feature): opt-in server elicitation for confirm-style tools.
- **Types:** `ClapMcpRunOptions`, `McpListen`, `ClapMcpErrorData` (alias for `rmcp::model::ErrorData`).
- **MCP tasks (in-process, serialized):** task-augmented `tools/call` with `#[clap_mcp(task)]` / `#[clap_mcp(task_augmented_tools)]`.
- **Conformance harness:** `cargo xtask conformance` (local Docker), GitHub Action, [conformance-baseline.yml](conformance-baseline.yml).

### Changed

- **MCP stack:** migrate from `rust-mcp-sdk` 0.9 to official [`rmcp`](https://docs.rs/rmcp) **1.7.x**. See [docs/rmcp-migration-notes.md](docs/rmcp-migration-notes.md).

## [0.0.3-rc.1] - 2025-03-05

### Breaking

- **Per-variant output attributes removed.** Enums that derive `ClapMcp` must now use `#[clap_mcp_output_from = "run"]` (or another function path) and implement a single `run(YourEnum) -> T` where `T: IntoClapMcpResult`. The following attributes are no longer supported:
  - `#[clap_mcp_output = "expr"]`
  - `#[clap_mcp_output_json = "expr"]`
  - `#[clap_mcp_output_literal = "string"]`
  - `#[clap_mcp_output_result]`
  - `#[clap_mcp_error_type = "TypeName"]`
- **`clap_mcp::opt_str` removed.** Use `name.as_deref().unwrap_or("default")` (or similar) inside your `run` function instead.

Migration: add `#[clap_mcp_output_from = "run"]` to each enum and implement `fn run(cmd: YourEnum) -> T` with the same logic you previously expressed in per-variant attributes. For `Result`-returning tools, have `run` return `Result<O, E>` and implement `IntoClapMcpToolError` for `E` when you want structured error JSON.

[0.0.4-rc.1]: https://github.com/canardleteer/clap-mcp/compare/v0.0.3-rc.1...v0.0.4-rc.1
[0.0.3-rc.1]: https://github.com/canardleteer/clap-mcp/compare/v0.0.2-rc.3...v0.0.3-rc.1
