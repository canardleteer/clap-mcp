# Migration notes

> Upgrade reference for CLI authors. See [README](../README.md) to get started.

[← Documentation index](../README.md#documentation)

## 0.0.5 → 0.1.0-rc.1

`0.1.0-rc.1` is a **breaking** release. It bumps workspace [`rmcp`](https://docs.rs/rmcp)
from **1.7** to **2.2**, moves `output-schema` to **schemars 1.x**, and keeps the
derive / `ServeMcpBuilder` public surface from the 0.0.4 line. Copy-paste
dependency examples use `version = "0.1.0-rc.1"` (match the workspace RC).

Primary break for embedders: rmcp model types. See
[rmcp 1.7 → 2.2](#rmcp-17--22). Derive-only CLIs that do not construct rmcp
types usually need only a dependency bump.

Later sections document the historical `0.0.3-rc.1` → `0.0.4-rc.1` port from
rust-mcp-sdk to rmcp 1.7.

## rmcp 1.7 → 2.2

clap-mcp `0.1.0-rc.1` depends on **rmcp 2.2**. The JSON wire format is
unchanged; the Rust model API aligns with MCP 2025-11-25. Upstream guide:
[Migrating to 2.x](https://github.com/modelcontextprotocol/rust-sdk/discussions/926).

If you only use clap-mcp derive entrypoints (`parse_or_serve_mcp*`) and do not
construct rmcp types yourself, you typically only need to bump dependencies.
If you build custom prompts/resources or an MCP task client, apply these
renames:

| Area | rmcp 1.x | rmcp 2.x |
| --- | --- | --- |
| Tool / prompt content | `Content` / `RawContent` / `PromptMessageContent` | `ContentBlock` |
| Prompt roles | `PromptMessageRole` | `Role` |
| Resources | `RawResource` + `Resource::new(raw, annotations)` | `Resource::new(uri, name).with_*()` |
| Resource body | `ResourceContents::TextResourceContents { .. }` literals | `ResourceContents::text(..).with_mime_type(..)` (types are `#[non_exhaustive]`) |
| Task poll params | `GetTaskInfoParams` / `GetTaskResultParams` | `GetTaskParams` / `GetTaskPayloadParams` |
| Task client requests | `ClientRequest::GetTaskInfoRequest` / `GetTaskResultRequest` | `GetTaskRequest` / `GetTaskPayloadRequest` |
| Task-augmented call | `with_task(object!({}))` | `with_task(TaskMetadata::new())` |
| `output-schema` | `schemars` 0.8 | `schemars` 1.x (matches rmcp’s `server` feature) |

Additive on the same line: `ResourceContent::StaticBlob { base64 }` for MCP
`blob` resource reads. [`resolve_resource_content`](https://docs.rs/clap-mcp/latest/clap_mcp/content/fn.resolve_resource_content.html)
now returns [`ResolvedResourceBody`](https://docs.rs/clap-mcp/latest/clap_mcp/content/enum.ResolvedResourceBody.html)
(`Text` or `Blob`) instead of `String`. See [custom-content](custom-content.md).

MCP logging (`LoggingLevel`, `notifications/message`) remains supported through
clap-mcp’s logging bridge. Upstream marks logging deprecated
([SEP-2577](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2577));
it still works in rmcp 2.2. See [logging.md](logging.md) for the agent-facing
diagnostics product risk (stderr / OpenTelemetry are not a drop-in replacement).

## Historical notes (0.0.3-rc.1 → 0.0.4-rc.1)

`0.0.4-rc.1` replaced the workspace dependency on **rust-mcp-sdk 0.9** with
official **[rmcp](https://github.com/modelcontextprotocol/rust-sdk)**, slimmed the
public API, and added MCP task-augmented `tools/call`, concurrent execution
options, and stateful in-process tools.

### Breaking API changes (0.0.4-rc.1)

Public API surface after `0.0.4-rc.1`:

* **Derive entry:** `ParseOrServeMcp::parse_or_serve_mcp()` /
  `parse_or_serve_mcp_with(ClapMcpRunOptions { .. })`; preserve native CLI
  error UX with `parse_or_serve_mcp_preserve_cli()` /
  `parse_or_serve_mcp_preserve_cli_with`
* **Imperative entry:** `get_matches_or_serve_mcp*` ladder; preserve native CLI
  parse with `get_matches_preserve_cli_or_serve_mcp*`
* **Low-level serve:**
  [`ServeMcpBuilder`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ServeMcpBuilder.html)
  (recommended; `.serve().await` or `.serve_blocking()`) with
  [`ServeMcpBuilder::for_cli`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ServeMcpBuilder.html#method.for_cli)
  for derive CLIs or `.new()` for hand-built schemas;
  [`serve_mcp`](https://docs.rs/clap-mcp/latest/clap_mcp/fn.serve_mcp.html) /
  [`serve_mcp_blocking`](https://docs.rs/clap-mcp/latest/clap_mcp/fn.serve_mcp_blocking.html)
  remain as lower-level 7-arg delegators with
  [`McpListen::Stdio`](https://docs.rs/clap-mcp/latest/clap_mcp/enum.McpListen.html)
  or `McpListen::Http(addr)` (`http` feature)
* **Tasks flag:** `ClapMcpSchemaMetadata::task_augmented_tools` (from
  `#[clap_mcp(task_augmented_tools)]`)
* **Errors alias:** `ClapMcpErrorData` = `rmcp::model::ErrorData`

Additive (embedder serve, no break to `parse_or_serve_mcp*`):

| Added | Role |
| --- | --- |
| [Usage — Setup then serve](usage.md#setup-then-serve-embedder) | Documented embedder path: `parse` → setup → `ServeMcpBuilder::for_cli` |
| [`ServeMcpBuilder::stdio_io`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ServeMcpBuilder.html#method.stdio_io) | Custom `AsyncRead` + `AsyncWrite` for stdio MCP (default remains process stdio) |

Removed (hard break — use unified serve API above):

| Removed | Replacement |
|---------|-------------|
| `serve_schema_json_over_stdio` | `ServeMcpBuilder::new().listen(McpListen::Stdio).…serve().await` or `serve_mcp(...)` |
| `serve_schema_json_over_stdio_blocking` | `ServeMcpBuilder::…serve_blocking()` or `serve_mcp_blocking(...)` |
| `serve_schema_json_over_http` | `ServeMcpBuilder::new().listen(McpListen::Http(addr)).…serve().await` |
| `serve_schema_json_over_http_blocking` | `ServeMcpBuilder::…serve_blocking()` or `serve_mcp_blocking(...)` |

Also removed: `parse_or_serve_mcp_with_config*` (use
`parse_or_serve_mcp_with(ClapMcpRunOptions { .. })`), freestanding
`tools_from_schema*` (use `tools_from_schema_with_metadata`),
`ClapMcpConfig::task_augmented_tools`, public `tool_task_eligible`, public
`ClapMcpServer` / `build_clap_mcp_server`.

## Workspace dependency (rmcp 2.2)

```toml
rmcp = { version = "2.2", default-features = false, features = [
  "server",
  "client",                 # example clients, integration tests
  "macros",
  "transport-io",           # server: rmcp::transport::stdio
  "transport-child-process" # client: TokioChildProcess for spawned servers
] }
```

Confirmed feature names in **rmcp 2.2.0** (HTTP/OAuth features exist
separately):

| Feature | Role |
|---------|------|
| `server` | `ServerHandler`, `serve_server`, `RoleServer` (implies `transport-async-rw`; pulls schemars 1.x) |
| `client` | `ClientHandler`, `serve_client`, `RoleClient` |
| `macros` | `rmcp-macros`, `#[tool_handler]`, etc. |
| `transport-io` | `rmcp::transport::stdio()` — stdin/stdout server transport |
| `transport-child-process` | `TokioChildProcess` — spawn server binary for tests / example clients |

`default` on rmcp enables `base64`, `macros`, `server` — clap-mcp sets
`default-features = false` and enables explicitly.

HTTP feature on clap-mcp still enables
`rmcp/transport-streamable-http-server` and
`rmcp/transport-streamable-http-client-reqwest`.

## Module / type mapping (historical 0.0.4 port)

| rust-mcp-sdk 0.9 | rmcp 1.7+ |
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
| `rust_mcp_sdk::task_store::{InMemoryTaskStore, ServerTaskCreator, CreateTaskOptions}` | `ServerHandler::enqueue_task` + `rmcp::task_manager::OperationProcessor` |
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

**clap-mcp impact:** `content.rs` prompt provider traits and all `ServerHandler`
methods return `ErrorData` (public API note for CHANGELOG).

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
| `runtime.notify_log_message(params)` | Peer notification via logging bridge + `RequestContext` peer capture |

Task-augmented `tools/call`: rmcp **`Service<RoleServer>`** dispatches to
`enqueue_task` when `CallToolRequestParams.task` is `Some`; clap-mcp uses
`OperationProcessor` and optional per-server serialization.

## Transport & stdio

* **Server:** `rmcp::transport::stdio()` → `serve_server(handler,
  transport).await` or `handler.serve(rmcp::transport::stdio()).await`.
* **Client (tests/examples):** `TokioChildProcess::new(command)` (feature
  `transport-child-process`) instead of
  `StdioTransport::create_with_server_launch`.
* **No** `rust-mcp-sdk` `TransportOptions` /
  `StdioTransport::<ClientMessage>::new` — rmcp uses `IntoTransport` on
  `(AsyncRead, AsyncWrite)` or child-process adapter.

## `ClapMcpError` mapping

| `ClapMcpError` variant (0.0.3-rc.1) | Target (0.0.4-rc.1) |
|-------------------------------------|---------------------|
| `Transport(TransportError)` | Map from `ServiceError` / I/O errors |
| `McpSdk(McpSdkError)` | Map from `RmcpError` |

## Key rmcp 1.x differences (upstream)

From the official
[CHANGELOG](https://github.com/modelcontextprotocol/rust-sdk/blob/main/crates/rmcp/CHANGELOG.md)
and API docs:

1. **Crate split:** Official **`rmcp`** on crates.io (not `rust-mcp-stack` /
   `rust-mcp-sdk`).
2. **Handler model:** Handlers are **`Service<RoleServer>`**; JSON-RPC routing
   is internal; methods are granular `list_tools`, `call_tool`, etc.
3. **Runtime parameter removed:** Use **`RequestContext`** / **`Peer`**
   instead of `Arc<dyn McpServer>`.
4. **Tasks:** Spec-aligned names (`get_task_info`, `get_task_result`);
   built-in task dispatch on `CallToolRequest` with `task` metadata.
5. **Structured tool output:** `Tool.outputSchema` /
   `CallToolResult.structuredContent` (clap-mcp `output-schema` feature).
6. **SSE transport removed** in rmcp 0.11+ (not used by clap-mcp stdio path).

## Task-augmented `tools/call` (0.0.4-rc.1+)

Beyond the initial serialized baseline:

* **`parallel_safe = true`** with `task_augmented_tools` — task and plain tool
  bodies may overlap.
* **`catch_in_process_panics = true`** with `task_augmented_tools` — panics in
  task-scheduled work map to `CallToolResult` error payloads on `tasks/result`;
  sync panics in `run()` use `catch_unwind`; async panics on dedicated threads
  are caught at join when `catch_in_process_panics` is enabled.

Still **not supported:** subprocess (`reinvocation_safe = false`) + tasks
(derive compile error).

**Logging during concurrent tasks:** When `ClapMcpServeOptions::log_rx` is set,
log notifications from a task-augmented tool body include `meta.taskId` matching
`CreateTaskResult.task.task_id`. clap-mcp installs per-task context via
[`run_with_mcp_task_id`](https://docs.rs/clap-mcp/latest/clap_mcp/logging/fn.run_with_mcp_task_id.html)
(task-local + thread-local). With **`share_runtime = false`** (default),
`run_async_tool` copies the active task id onto the dedicated async-tool thread
(`McpTaskIdGuard`). With **`share_runtime = true`**, it re-installs the task id
inside `Handle::block_on` — tokio task-local from the outer MCP task body does
**not** always propagate into the nested future polled by `block_on`, especially
under concurrent `parallel_safe` load; omitting that re-scope dropped
`meta.taskId` from forwarded logs (not platform-specific). See
[Logging](logging.md#task-augmented-tools-and-metataskid) and
[MCP tasks](mcp-tasks.md).

Examples: `task_parallel_probe_*`, `task_panic_catch`,
`task_panic_catch_parallel`; user-facing demos in `task_tools_*` and
`task_panic_catch`.

## Stateful tools + positional guard

Additive API on `0.0.4-rc.1+` (ports
[#11](https://github.com/canardleteer/clap-mcp/pull/11) and
[#12](https://github.com/canardleteer/clap-mcp/pull/12)):

* **`ClapMcpToolExecutorWithState`** — in-process execution with associated
  `State`; tool code receives `&Self::State` (server holds `Arc` internally)
* Derive: `#[clap_mcp_output_from_with_state = "run"]` +
  `#[clap_mcp_state_type = "Type"]` on **leaf** enums (`Type` must match `run`'s
  second parameter); `#[clap_mcp(stateful)]` on struct roots / delegating enums
* **`ParseOrServeMcpWithState`**, `parse_or_serve_mcp_with_state`,
  **`ServeMcpBuilder::for_cli_with_state`**
* **`compile_error!`** when a derive target has two or more bare positional
  scalar fields (use `#[arg(long)]` instead) — see
  [PR #12](https://github.com/canardleteer/clap-mcp/pull/12)

Example: `examples/servers/stateful_counter.rs`; integration test:
`clap-mcp/tests/stateful_server_tests.rs`.

## Topical serialization (next release after 0.0.4-rc.1)

Additive API (no migration required for existing embedders):

* **`#[clap_mcp(serialized)]`** on a subcommand variant — tool-wide lock topic
  when
  [`ClapMcpConfig::parallel_safe`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapMcpConfig.html#structfield.parallel_safe)
  is true
* **`#[clap_mcp(serialized = "arg1, arg2")]`** — arg-scoped lock topic
  (clap arg ids)
* **`ClapMcpSchemaMetadata::serialize_tools`** and
  [`ClapMcpSerializeScope`](https://docs.rs/clap-mcp/latest/clap_mcp/enum.ClapMcpSerializeScope.html)
  for imperative metadata
* **`meta.clapMcp` hints** on `list_tools`: `serialized`, `serializeScope`,
  `serializeArgs`, optional `serializeTopicArgs`
* **`#[clap_mcp(serialize_topic)]`** on fields listed in arg-scoped
  `serialized`, with optional
  [`ClapMcpSerializeTopic`](https://docs.rs/clap-mcp/latest/clap_mcp/trait.ClapMcpSerializeTopic.html)
  (`impl_serialize_topic_hash_eq!`, `impl_serialize_topic_serde_eq!`)
* **`ClapMcpSchemaMetadata::serialize_topic_args`** for imperative typed topics

When `parallel_safe = false`, global serialization is unchanged (topical
metadata is ignored). See
[Execution safety — Topical serialization](execution-safety.md#topical-serialization).

Derive metadata keys (`skip`, `requires`, `serialized`, `serialize_topic`) now
resolve clap arg ids from `#[arg(id = "...")]` when present (field ident
otherwise), matching MCP `inputSchema` property names.

Example: `examples/servers/topical_serialization.rs`,
`examples/servers/topical_serial_probe.rs`; integration tests:
`clap-mcp/tests/topical_serialization_tests.rs`.

## Arg group hints (0.0.4-rc.1+)

Additive API (no migration required):

* **[`ClapArgGroup`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapArgGroup.html)**
  and **`ClapCommand::arg_groups`** on schema nodes
* **`meta.clapMcp.argGroups`** on `list_tools` when clap ArgGroups are present
* **Description suffix** on tools with groups (parse-time hint; same extraction
  as meta)

Hints are advisory; clap parse remains authoritative. No JSON Schema `oneOf`.
See [Execution safety — Arg groups](execution-safety.md#arg-groups).

Example: `examples/servers/arg_group_hints.rs`; integration test:
`example_contract_arg_group_hints_meta_and_parse` in
`clap-mcp/tests/example_contract_tests.rs`.

## Nested metadata, schema-only enums, struct `output_from` (0.0.4-rc.1+)

Additive API for deeply nested CLIs:

* **`ClapMcpSchemaMetadata::merge_from`** — imperative deep merge of skip,
  requires, task, and serialization metadata
* **Derive deep-merge** — nested `#[command(subcommand)]` enum metadata folds
  into ancestors automatically
* **`#[clap_mcp(schema_only)]`** on intermediate subcommand enums — metadata
  only; no `output_from` stub when an ancestor owns execution
* **`#[clap_mcp_output_from = "run"]` on struct roots** — `run` receives the
  full parsed struct (global flags and subcommand field)
* **Skipped variants** — multi-positional compile guard no longer applies to
  `#[clap_mcp(skip)]` variants

Examples: `nested_subcommands`, `struct_subcommand_globals`; see
[Execution safety](execution-safety.md).

## Removed scaffolding: http-oauth {#removed-scaffolding-http-oauth}

The `http-oauth` Cargo feature shipped briefly as scaffolding on the `0.0.4`
pre-release line and was dropped while no simple, clap-mcp-shaped integrator
pattern has emerged.

**Removed:**

* Cargo feature `http-oauth`
* Module `clap_mcp::oauth`
* Types `EnvConfig`, `FromEnvError`
* Environment constants `CLAP_MCP_OAUTH_*` (`OAUTH_ISSUER_ENV`,
  `OAUTH_CLIENT_ID_ENV`, …)
* Re-exports `AuthClient`, `StreamableHttpClientTransport` from
  `clap_mcp::oauth`

**Migration:** depend on `rmcp` with `auth` and
`transport-streamable-http-client-reqwest` features; load OAuth config in your
binary or follow examples under
[`rust-sdk/examples/clients/`](https://github.com/modelcontextprotocol/rust-sdk/tree/main/examples/clients).
See
[OAuth in rmcp](https://github.com/modelcontextprotocol/rust-sdk/blob/main/docs/OAUTH_SUPPORT.md).

**Unaffected:** `--mcp` / `--mcp-http` server serving; tool code calling
arbitrary OAuth APIs via `oauth2`, `reqwest`, or similar.

**Roadmap:** This feature is not on the clap-mcp roadmap until a simple,
clap-mcp-shaped client OAuth pattern emerges (for example env-driven config for
calling remote MCP servers from a `clap` binary). That is a prioritization
choice, not a permanent rejection of OAuth in MCP.

## Removed scaffolding: elicitation {#removed-scaffolding-elicitation}

The `elicitation` Cargo feature shipped as scaffolding on the `0.0.4`
pre-release line. It was a hardcoded `confirm-echo` server intercept, not a
pattern derived from `clap` CLI structure. clap-mcp already covers most agent
policy via `#[clap_mcp(requires)]` and `#[clap_mcp(skip)]`; MCP elicitation
targets runtime user input that does not map cleanly onto argv-shaped tools,
especially in subprocess mode.

**Removed:**

* Cargo feature `elicitation`
* `ClapMcpServeOptions::elicitation_enabled`
* Server-side `confirm-echo` intercept in tool dispatch
* Example binary `elicitation_confirm`

**Migration:**

* Agent policy: use `#[clap_mcp(requires)]` and `#[clap_mcp(skip)]` per
  [Execution safety](execution-safety.md).
* Interactive or TTY subcommands: keep `#[clap_mcp(skip)]` and document shell
  invocation for operators.
* Full MCP elicitation: depend on `rmcp` with its `elicitation` feature and
  implement elicitation in custom server code outside clap-mcp's derive path.

**Roadmap:** This feature is not on the clap-mcp roadmap until a simple,
clap-mcp-shaped integrator pattern emerges (for example declarative per-tool
confirm policy derived from CLI metadata). That is a prioritization choice, not
a permanent decision against elicitation in MCP.
