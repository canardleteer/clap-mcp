# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]
## `clap-mcp` - [0.0.4-rc.1](https://github.com/canardleteer/clap-mcp/compare/clap-mcp-v0.0.3-rc.1...clap-mcp-v0.0.4-rc.1) - 2026-06-06

### Added
- conformance changes and true up- xtask for quicker code coverage view- MCP passthrough, `--` semantics, and configurable builtin flags- *(stateful)* derive macro attrs and compile-time guards- *(stateful)* ClapMcpToolExecutorWithState and handler plumbing- add ServeMcpBuilder embedder API and refresh docs- restore async serve_mcp embedder API for tokio apps- concurrent task-augmented tools and panic-in-task handling- [**breaking**] release 0.0.4-rc.1 with slim derive API and embedder config- additional conformance and simplifications- task support

### Fixed
- re-scope task logging context in share_runtime block_on- gate unix-only subprocess test import for Windows clippy

### Other
- re-structure and split up README- increase code coverage on new features- add an additional intent- CLI compatibility guide and required-subcommand example- *(stateful)* associated-type API and shared serve prep- stateful MCP tools and positional arg guard- stateful and positional guard coverage- Add optional HTTP, OAuth, elicitation, and conformance tooling.- Complete rmcp port integrator gate and docs- Rewrite integration tests and example clients for rmcp- Port MCP server core to rmcp 1.7- pin rmcp 1.7 and add migration notes- crate updates
## `clap-mcp-macros` - [0.0.4-rc.1](https://github.com/canardleteer/clap-mcp/compare/clap-mcp-macros-v0.0.3-rc.1...clap-mcp-macros-v0.0.4-rc.1) - 2026-06-06

### Added
- MCP passthrough, `--` semantics, and configurable builtin flags- *(stateful)* derive macro attrs and compile-time guards- concurrent task-augmented tools and panic-in-task handling- [**breaking**] release 0.0.4-rc.1 with slim derive API and embedder config- task support

### Other
- CLI compatibility guide and required-subcommand example- *(stateful)* associated-type API and shared serve prep

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
