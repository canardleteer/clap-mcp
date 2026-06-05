# rmcp migration notes (rust-mcp-sdk 0.9 → rmcp 1.7)

Workstream **W0** reference for the clap-mcp port. Handler rewrites are
**W1–W2**; this document captures dependency pinning and API mapping only.

**Sources:** `rmcp` 1.7.0 crate (`Cargo.toml` features, `handler/server.rs`,
`transport.rs`), [modelcontextprotocol/rust-sdk](https://github.com/modelcontextprotocol/rust-sdk)
[CHANGELOG](https://github.com/modelcontextprotocol/rust-sdk/blob/main/crates/rmcp/CHANGELOG.md)
(0.x → 1.x, especially 1.0.0 / 1.0.0-alpha “api ergonomics follow-up”
[#720](https://github.com/modelcontextprotocol/rust-sdk/pull/720)), and current
clap-mcp `rust_mcp_sdk` usage.

## Workspace dependency (W0)

```toml
rmcp = { version = "1.7", default-features = false, features = [
  "server",
  "client",                 # parity with old rust-mcp-sdk "client" (example clients, tests)
  "macros",
  "transport-io",           # replaces rust-mcp-sdk "stdio" (server: rmcp::transport::stdio)
  "transport-child-process" # replaces StdioTransport::create_with_server_launch for clients
] }
```

Confirmed feature names in **rmcp 1.7.0** `Cargo.toml` (others exist for
HTTP/OAuth; out of scope for stdio port):

| Feature | Role |
|---------|------|
| `server` | `ServerHandler`, `serve_server`, `RoleServer` (implies `transport-async-rw`) |
| `client` | `ClientHandler`, `serve_client`, `RoleClient` |
| `macros` | `rmcp-macros`, `#[tool_handler]`, etc. |
| `transport-io` | `rmcp::transport::stdio()` — stdin/stdout server transport |
| `transport-child-process` | `TokioChildProcess` — spawn server binary for integration tests / example clients |

`default` on rmcp enables `base64`, `macros`, `server` — we set
`default-features = false` and enable explicitly.

## Module / type mapping

| rust-mcp-sdk 0.9 | rmcp 1.7 |
|------------------|----------|
| `rust_mcp_sdk::schema::*` | `rmcp::model::*` |
| `rust_mcp_sdk::mcp_server::ServerHandler` | `rmcp::ServerHandler` |
| `rust_mcp_sdk::mcp_server::ToMcpServerHandler` | Implement `ServerHandler` on the handler type directly (`Service<RoleServer>` blanket impl) |
| `rust_mcp_sdk::McpServer` (`Arc<dyn McpServer>`) | `rmcp::Peer<RoleServer>` via `RequestContext<RoleServer>` / `RunningService::peer()` |
| `rust_mcp_sdk::StdioTransport` | `rmcp::transport::stdio()` or `(stdin, stdout)` tuple (`IntoTransport`) |
| `rust_mcp_sdk::TransportOptions` | Transport-specific setup (e.g. child process builder); no direct equivalent |
| `mcp_server::server_runtime::create_server` + `McpServerOptions` | `handler.serve(transport).await?` / `rmcp::serve_server` + `ServerHandler::get_info()` / `InitializeResult` |
| `mcp_client::client_runtime::create_client` + `McpClientOptions` | `ClientHandler` + `serve_client` / `().serve(transport)` |
| `schema_utils::ResultFromServer` | `RunningService` / `Peer` typed request helpers |
| `rust_mcp_sdk::task_store::{InMemoryTaskStore, ServerTaskCreator, CreateTaskOptions}` | `ServerHandler::enqueue_task` + `rmcp::task_manager::OperationProcessor` (W5) |
| `rust_mcp_sdk::error::McpSdkError` | `rmcp::RmcpError` / `rmcp::Error` |
| `rust_mcp_sdk::TransportError` | `rmcp::ServiceError` / transport adapter errors |

## `RpcError` → `ErrorData`

rust-mcp-sdk handlers and prompt providers return
`rust_mcp_sdk::schema::RpcError` with builder-style helpers (e.g.
`.with_message(...)`).

rmcp uses **`ErrorData`** (`rmcp::ErrorData`, also exposed from
`rmcp::error`). In server code, `crate::error::ErrorData` is often aliased as
**`McpError`** in `ServerHandler` method signatures.

| rust-mcp-sdk `RpcError` | rmcp `ErrorData` |
|-------------------------|------------------|
| `RpcError::invalid_params().with_message(msg)` | `ErrorData::invalid_params(msg, None)` |
| `RpcError::invalid_request().with_message(msg)` | `ErrorData::invalid_request(msg, None)` |
| `RpcError::internal_error().with_message(msg)` | `ErrorData::internal_error(msg, None)` |
| `RpcError::method_not_found()` | `ErrorData::method_not_found::<M>()` or `ErrorData::new(ErrorCode::METHOD_NOT_FOUND, ...)` |

**clap-mcp impact:** `content.rs` prompt provider traits (`resolve_template`,
`resolve_messages`) and all `ServerHandler` methods in `lib.rs` must switch
return error type to `ErrorData` (public API note for CHANGELOG).

## `ServerHandler` method mapping

rust-mcp-sdk uses **`async fn handle_*_request`** with an
`Arc<dyn McpServer>` runtime argument.

rmcp uses **short names**, **`RequestContext<RoleServer>`** (peer,
extensions), and **`impl Future<Output = Result<..., McpError>>`** (not
`async fn` on the trait).

| rust-mcp-sdk `ServerHandler` | rmcp `ServerHandler` |
|------------------------------|----------------------|
| `on_initialized(runtime)` | `on_initialized(NotificationContext<RoleServer>)` |
| `handle_initialize_request(params, runtime)` | `initialize(params, context)` — default sets peer info from request |
| `handle_ping_request(_, runtime)` | `ping(context)` |
| `handle_list_resources_request(params, runtime)` | `list_resources(params, context)` |
| `handle_read_resource_request(params, runtime)` | `read_resource(params, context)` |
| `handle_list_tools_request(params, runtime)` | `list_tools(params, context)` |
| `handle_call_tool_request(params, runtime)` | `call_tool(params, context)` — or task path via `enqueue_task` when `params.task` is set |
| `handle_list_prompts_request(params, runtime)` | `list_prompts(params, context)` |
| `handle_get_prompt_request(params, runtime)` | `get_prompt(params, context)` |
| `handle_get_task_request` / payload (task APIs) | `get_task_info`, `get_task_result`, `list_tasks`, `cancel_task` |
| `runtime.notify_log_message(params)` | Peer notification via `context.peer` / service APIs (W4) |

Task-augmented `tools/call`: rmcp **`Service<RoleServer>`** dispatches to
`enqueue_task` when `CallToolRequestParams.task` is `Some`; clap-mcp’s
`OperationProcessor` / serialization queue maps to **W5**.

## Transport & stdio (1.x ergonomics)

* **Server:** `rmcp::transport::stdio()` → `serve_server(handler,
  transport).await` or `handler.serve(rmcp::transport::stdio()).await`.
* **Client (tests/examples):** `TokioChildProcess::new(command)` (feature
  `transport-child-process`) instead of
  `StdioTransport::create_with_server_launch`.
* **No** `rust-mcp-sdk` `TransportOptions` /
  `StdioTransport::<ClientMessage>::new` — rmcp uses `IntoTransport` on
  `(AsyncRead, AsyncWrite)` or child-process adapter.

## Errors & `ClapMcpError` (W1)

| `ClapMcpError` variant today | Target |
|------------------------------|--------|
| `Transport(TransportError)` | Map from `ServiceError` / I/O errors |
| `McpSdk(McpSdkError)` | Map from `RmcpError` |

## Key 1.x differences (from upstream [CHANGELOG](https://github.com/modelcontextprotocol/rust-sdk/blob/main/crates/rmcp/CHANGELOG.md) / API)

1. **Crate split:** Official **`rmcp`** on crates.io (not `rust-mcp-stack` /
   `rust-mcp-sdk`).
2. **Handler model:** Handlers are **`Service<RoleServer>`**; JSON-RPC routing
   is internal; methods are granular `list_tools`, `call_tool`, etc.
3. **Runtime parameter removed:** Use **`RequestContext`** / **`Peer`**
   instead of `Arc<dyn McpServer>`.
4. **Tasks:** Spec-aligned names (`get_task_info`, `get_task_result`);
   built-in task dispatch on `CallToolRequest` with `task` metadata.
5. **Structured tool output:** `Tool.outputSchema` /
   `CallToolResult.structuredContent` (already used by clap-mcp output-schema
   feature).
6. **SSE transport removed** in 0.11+ (not used by clap-mcp stdio path).

## W0 compile expectation

After W0, `cargo check -p clap-mcp` **should fail** on unresolved
`rust_mcp_sdk` imports in `lib.rs`, `content.rs`, `logging.rs` until **W1**
(schema/errors) and **W2** (server handler / `serve_schema_json_over_stdio`)
land.

## Readiness for W1 + W2

| Workstream | Uses this doc for |
|------------|-------------------|
| **W1** | `model::*` imports, `ErrorData`, tool/resource/prompt constructors, `ClapMcpError` mapping |
| **W2** | `ServerHandler` rewrite, `serve_schema_json_over_stdio`, `Peer` for logging bridge hookup |
| **W5** | `enqueue_task`, `OperationProcessor`, task RPC names |
| **W6** | `client` + `transport-child-process`, `serve_client`, replace `client_runtime` harness |

## 0.0.4-rc.1 API slim (post-rmcp port)

Public embedder surface after `0.0.4-rc.1`:

* **Derive entry:** `ParseOrServeMcp::parse_or_serve_mcp()` /
  `parse_or_serve_mcp_with(ClapMcpRunOptions { .. })`
* **Imperative entry:** unchanged `get_matches_or_serve_mcp*` ladder
* **Low-level serve:** `serve_mcp_blocking(McpListen::Stdio |
  McpListen::Http(addr), ...)`
* **Tasks flag:** `ClapMcpSchemaMetadata::task_augmented_tools` (from
  `#[clap_mcp(task_augmented_tools)]`)
* **Errors alias:** `ClapMcpErrorData` = `rmcp::model::ErrorData`

Removed: freestanding `parse_or_serve_mcp*`, `tools_from_schema*`, public
`serve_schema_json_over_*`, `ClapMcpConfig::task_augmented_tools`, public
`tool_task_eligible`, public `ClapMcpServer` / `build_clap_mcp_server`.

## Task-augmented `tools/call` addendum (0.0.4-rc.1+)

Shipped beyond the initial serialized baseline:

* **`parallel_safe = true`** with `task_augmented_tools` — task and plain tool
  bodies may overlap; per-task logging context (`tokio::task_local!` / thread-local
  for dedicated async-tool threads) keeps `meta.taskId` correct under concurrency.
* **`catch_in_process_panics = true`** with `task_augmented_tools` — panics in
  task-scheduled work map to `CallToolResult` error payloads on `tasks/result`;
  sync panics in `run()` use `catch_unwind`; async panics on dedicated threads
  are caught at join when `catch_in_process_panics` is enabled.

Still **not supported:** subprocess (`reinvocation_safe = false`) + tasks (derive
compile error).

Examples: `task_parallel_probe_*`, `task_panic_catch`, `task_panic_catch_parallel`
(integration-test helpers); user-facing demos in `task_tools_*` and
`task_panic_catch`.
