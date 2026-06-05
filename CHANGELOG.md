# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

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

[Unreleased]: https://github.com/canardleteer/clap-mcp/compare/v0.0.3-rc.1...HEAD
[0.0.3-rc.1]: https://github.com/canardleteer/clap-mcp/compare/v0.0.2-rc.3...v0.0.3-rc.1
