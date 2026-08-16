# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]
## `clap-mcp` - [0.1.0-rc.1](https://github.com/canardleteer/clap-mcp/compare/clap-mcp-v0.0.5...clap-mcp-v0.1.0-rc.1) - 2026-08-16

### Added
- *(server)* absorb rmcp 3.1 and pin conformance to 0.2.0-alpha.11- *(server)* emit SEP-2549 ttlMs and cacheScope on list and read- *(schema)* advertise JSON Schema 2020-12 on tool inputSchema- *(server)* absorb rmcp 3.0 and released MCP 2026-07-28- *(content)* simple URI templates for custom resources- *(server)* accept resources subscribe and unsubscribe RPCs- *(content)* support binary custom resources for MCP blob reads- *(conformance)* dual-protocol harness and documented baselines- [**breaking**] bump to 0.1.0-rc.1 with rmcp 2.2 and schemars 1

### Fixed
- *(tests)* harden conformance HTTP teardown on macOS CI

### Other
- *(protocol)* treat 2026-07-28 as the current released MCP version- *(protocol)* drop broken rustdoc link and stale harness notes
## `clap-mcp` - [0.0.5](https://github.com/canardleteer/clap-mcp/compare/clap-mcp-v0.0.4...clap-mcp-v0.0.5) - 2026-06-12

### Added
- *(serve)* custom stdio read/write for embedders ([#21](https://github.com/canardleteer/clap-mcp/pull/21))

### Fixed
- agent-rules section

### Other
- *(usage)* setup-then-serve embedder pattern ([#22](https://github.com/canardleteer/clap-mcp/pull/22))- *(readme)* fix crates.io links with absolute GitHub URLs
## `clap-mcp` - [0.0.4](https://github.com/canardleteer/clap-mcp/compare/clap-mcp-v0.0.4-rc.2...clap-mcp-v0.0.4) - 2026-06-10

### Added
- integration skill + small docs true up

### Fixed
- *(tests)* bound conformance HTTP fixture tests on macOS CI- *(tests)* drop unused mut after conformance child shutdown refactor- *(ci)* cap hung macOS test runs with job and poll timeouts

### Other
- *(agents)* add ci-gate rule and align with agent-rules-spec- rustfmt conformance fixture shutdown helper- note on agent rules spec- *(elicitation)* drop scaffolding feature and document rationale- *(http-oauth)* drop scaffolding OAuth client feature
## `clap-mcp` - [0.0.4-rc.2](https://github.com/canardleteer/clap-mcp/compare/clap-mcp-v0.0.4-rc.1...clap-mcp-v0.0.4-rc.2) - 2026-06-07

### Added
- export ArgGroup membership as meta.clapMcp.argGroups hints- add preserve-cli parse helpers for native shell error UX- skip flattened subcommands and collect serialize_topic in nested Args- include root global args in leaf MCP tool schemas- support #[clap_mcp_output_from] on struct roots- add #[clap_mcp(schema_only)] for nested subcommand enums- deep-merge nested subcommand schema metadata- add topical serialization for parallel-safe MCP tools

### Fixed
- align derive metadata keys with clap arg ids- skip all arg ids when a flattened field is skipped from MCP- apply task_augmented_tools on struct-root metadata delegate- skip positional guard for #[clap_mcp(skip)] variants

### Other
- Add flatten subcommand skip examples and conformance-stop lifecycle- add example contracts and preserve-cli argv checks- hide imperative flatten helpers from public rustdoc- apply cargo fmt to args_metadata attribute guard- flatten limits, state warnings in rustdoc, and security boundaries- flat CLI shapes, flatten skip, and embedder parse patterns- update "when and when not" section- add example-driven MCP contract tests- add supported CLI shapes matrix- add maintainer testing guide and agent rules- add canonical complex_cli fixture and contract tests- roundup for nested CLIs, MCP skip policy, and tool output- add usage guide, style guide, and documentation cleanup- better first glance Usage
## `clap-mcp-macros` - [0.0.4-rc.2](https://github.com/canardleteer/clap-mcp/compare/clap-mcp-macros-v0.0.4-rc.1...clap-mcp-macros-v0.0.4-rc.2) - 2026-06-07

### Added
- skip flattened subcommands and collect serialize_topic in nested Args- support #[clap_mcp_output_from] on struct roots- add #[clap_mcp(schema_only)] for nested subcommand enums- deep-merge nested subcommand schema metadata- add topical serialization for parallel-safe MCP tools

### Fixed
- align derive metadata keys with clap arg ids- skip all arg ids when a flattened field is skipped from MCP- apply task_augmented_tools on struct-root metadata delegate- skip positional guard for #[clap_mcp(skip)] variants

### Other
- apply cargo fmt to args_metadata attribute guard- flat CLI shapes, flatten skip, and embedder parse patterns- add usage guide, style guide, and documentation cleanup
## `clap-mcp` - [0.0.4-rc.2](https://github.com/canardleteer/clap-mcp/compare/clap-mcp-v0.0.4-rc.1...clap-mcp-v0.0.4-rc.2) - 2026-06-07

### Added
- export ArgGroup membership as meta.clapMcp.argGroups hints- add preserve-cli parse helpers for native shell error UX- skip flattened subcommands and collect serialize_topic in nested Args- include root global args in leaf MCP tool schemas- support #[clap_mcp_output_from] on struct roots- add #[clap_mcp(schema_only)] for nested subcommand enums- deep-merge nested subcommand schema metadata- add topical serialization for parallel-safe MCP tools

### Fixed
- align derive metadata keys with clap arg ids- skip all arg ids when a flattened field is skipped from MCP- apply task_augmented_tools on struct-root metadata delegate- skip positional guard for #[clap_mcp(skip)] variants

### Other
- Add flatten subcommand skip examples and conformance-stop lifecycle- add example contracts and preserve-cli argv checks- hide imperative flatten helpers from public rustdoc- apply cargo fmt to args_metadata attribute guard- flatten limits, state warnings in rustdoc, and security boundaries- flat CLI shapes, flatten skip, and embedder parse patterns- update "when and when not" section- add example-driven MCP contract tests- add supported CLI shapes matrix- add maintainer testing guide and agent rules- add canonical complex_cli fixture and contract tests- roundup for nested CLIs, MCP skip policy, and tool output- add usage guide, style guide, and documentation cleanup- better first glance Usage
## `clap-mcp-macros` - [0.0.4-rc.2](https://github.com/canardleteer/clap-mcp/compare/clap-mcp-macros-v0.0.4-rc.1...clap-mcp-macros-v0.0.4-rc.2) - 2026-06-07

### Added
- skip flattened subcommands and collect serialize_topic in nested Args- support #[clap_mcp_output_from] on struct roots- add #[clap_mcp(schema_only)] for nested subcommand enums- deep-merge nested subcommand schema metadata- add topical serialization for parallel-safe MCP tools

### Fixed
- align derive metadata keys with clap arg ids- skip all arg ids when a flattened field is skipped from MCP- apply task_augmented_tools on struct-root metadata delegate- skip positional guard for #[clap_mcp(skip)] variants

### Other
- apply cargo fmt to args_metadata attribute guard- flat CLI shapes, flatten skip, and embedder parse patterns- add usage guide, style guide, and documentation cleanup
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
