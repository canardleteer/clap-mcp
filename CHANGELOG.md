# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Changed

- **MCP stack:** migrate from `rust-mcp-sdk` 0.9 to official [`rmcp`](https://docs.rs/rmcp) **1.7.x** (stdio server + example/test clients). Internal schema types are now `rmcp::model::*`; custom prompt/resource providers return `rmcp::model::ErrorData` instead of `RpcError`. Unknown tool/resource/prompt requests surface as RPC `ErrorData` (`invalid_params`) where appropriate. Logging `meta.taskId` is carried in notification **extensions** (`Meta`) on the rmcp path. See [docs/rmcp-migration-notes.md](docs/rmcp-migration-notes.md).

### Added

- **MCP task-augmented `tools/call` (in-process, serialized):** `ClapMcpConfig::task_augmented_tools`, `InitializeResult.capabilities.tasks`, `McpServerOptions.task_store`, `ServerHandler::handle_task_augmented_tool_call` with the same execution path as plain in-process calls and a shared lock when `parallel_safe` is false. Optional `#[clap_mcp(task)]` on enum variants fills `ClapMcpSchemaMetadata::task_tool_names` and `meta.clapMcp.taskAugmented` in `list_tools`. `#[clap_mcp(task_augmented_tools)]` without `reinvocation_safe` fails to compile in the derive. Task-augmented runs set `CreateTaskResult.meta.taskId` and, when logging is enabled, `LoggingMessageNotificationParams.meta.taskId` for the active task body.
- **Examples:** `task_tools_dedicated`, `task_tools_shared`, and `task_augmented_client` (see [examples/README.md](examples/README.md)).

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
