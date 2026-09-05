---
name: clap-mcp-integration
description: >-
  Integrates clap-mcp into Rust CLIs: feature-gated deps, derive-first wiring,
  async in-process execution with topical serialization, structured output, and
  schema smoke tests. Covers struct-root subcommands, passthrough/search CLIs
  (ripgrep-shaped), and optional distribution binaries. Use when adding MCP
  server mode (--mcp, --mcp-http), wiring clap-mcp, or enabling grep/search
  tools for agents.
disable-model-invocation: true
---

# clap-mcp integration

Wire [clap-mcp](https://github.com/canardleteer/clap-mcp) into a Rust CLI so `tools/list` and `tools/call` expose leaf subcommands. Keep the **existing clap tree idiomatic**; absorb MCP in a thin layer (shared `execute`, derive attributes, optional feature gate).

**Bias:** Prefer clap-mcp's natural patterns (`parse_or_serve_mcp`, `#[clap_mcp(...)]`, `AsStructured`, topical `serialized`) for vanilla CLIs. Use the **setup-then-serve embedder** path (`parse` → `ServeMcpBuilder::for_cli`) when Phase 1 finds setup-first workflows, custom `serve` subcommands, or existing JSON-RPC multiplexers. Slim integration means **no CLI flattening** — not skipping structured output, async dispatch, or schema tests when they align cheaply.

When this skill is copied into another repo, read upstream docs from your **resolved `clap-mcp` dependency source** (path checkout, git checkout, or crate under `~/.cargo/registry/src/`), not from pinned URLs in this skill.

---

## When to run

Run when the user asks to add MCP to a Rust CLI, wire `--mcp` / `--mcp-http`, expose subcommands as MCP tools, enable **ripgrep-shaped** search tools (trailing paths, pattern flags, `--` passthrough), or embed MCP after setup (`--config`, init) or on a custom `serve` subcommand / existing JSON-RPC pipe.

---

## Phase 0 — Feature gate (ask first)

Before editing `Cargo.toml`, ask whether MCP should be an **optional Cargo feature** (recommended for published CLIs) or **always enabled**.

Use **AskQuestion** when available:

| Option | When |
|--------|------|
| **Optional `mcp` feature** (default recommendation) | crates.io library+CLI, minimal default deps, separate dist binary OK |
| **Always on** | Internal-only binary, MCP is the primary interface |

Record the choice in the PR/commit message. If optional:

- `mcp = ["dep:clap-mcp", "dep:schemars"]` (add `schemars` only when using `output-schema`)
- `#[cfg(feature = "mcp")]` / `#[cfg_attr(feature = "mcp", …)]` on derives and MCP-only types
- Default `cargo test` / `cargo build` unchanged; CI matrix includes `--features mcp` when MCP ships

See [reference-distribution.md](resources/reference-distribution.md) for dual-binary layout (`mycli-mcp` workspace member).

---

## Workflow

Copy this checklist and track progress:

```
Task progress:
- [ ] 0. User chose feature-gated vs always-on MCP
- [ ] 1. Inventory clap root shape + hazards (Phase 1)
- [ ] 2. Add clap-mcp dependency (reference-dependency.md)
- [ ] 3. Shared dispatch + parse_or_serve_mcp (reference-patterns.md)
- [ ] 4. Execution config: reinvocation_safe, parallel_safe, topical serialized (reference-execution.md)
- [ ] 5. Structured output + skip/requires metadata
- [ ] 6. Optional logging bridge (only if project already uses tracing/log)
- [ ] 7. Schema smoke test + compile both feature modes
- [ ] 8. Smoke tools/list + one tools/call (stdio)
```

---

## Phase 1 — Inventory (read-only)

Read the target repo before changing code:

| Item | Where to look |
|------|----------------|
| Root shape | `Parser` struct + `#[command(subcommand)]` vs enum root |
| Nesting depth | Nested `Subcommand` enums |
| Global flags | `#[arg(global = true)]` on struct root |
| Positionals / stdin | `Option` positionals that read stdin when omitted |
| Passthrough / trailing paths | `Vec<PathBuf>`, `last = true`, `allow_hyphen_values` |
| Interactive / TTY | `dialoguer`, `is_terminal`, blocking prompts |
| Global init hazards | `tracing_subscriber::init()`, `rustls`, `process::exit` in tool path |
| Existing `-o json` | Human-default vs JSON opt-in |
| Async tool bodies | `async fn` handlers, shared tokio runtime |
| Setup-before-serve | `--config`, init hooks that must run before MCP |
| Existing JSON-RPC / custom transport | Multiplexer, socket pair, non-stdio MCP entry |

Use `rg` (ripgrep) for discovery — e.g. `rg 'derive\(Parser|Subcommand|process::exit|tracing_subscriber' src/`.

**Ripgrep-shaped CLIs:** If the tool searches files (patterns + trailing paths, or `--` passthrough), read [reference-search-cli.md](resources/reference-search-cli.md) before choosing schema attributes.

---

## Phase 2 — Add dependency

Follow [reference-dependency.md](resources/reference-dependency.md).

**Suggested feature set** (trim if unused):

```toml
[features]
mcp = ["dep:clap-mcp", "dep:schemars"]

[dependencies]
clap-mcp = { version = "0.1.0", optional = true, features = ["output-schema", "http"] }
schemars = { version = "1", optional = true, features = ["derive"] }
```

Add `tracing` or `log` clap-mcp features **only** when wiring a logging bridge (Phase 6).

---

## Phase 3 — Wire integration (derive-first)

Full patterns: [reference-patterns.md](resources/reference-patterns.md). Upstream: [usage.md](../../../docs/usage.md), [supported-cli-shapes.md](../../../docs/supported-cli-shapes.md).

### Minimum viable (subprocess-safe)

Swap `Cli::parse()` → `Cli::parse_or_serve_mcp()` and add `#[clap_mcp_output_from = "run"]`. No `reinvocation_safe` — each tool call respawns the binary. Use only when in-process is unsafe and topical serialization cannot help.

### Recommended (struct root + subcommands)

Match the **sem-tool** pattern when the CLI uses a struct root with required subcommand:

1. Derive `ClapMcp` on **both** root struct and subcommand enum.
2. Root: `#[clap_mcp(skip_root_when_subcommands)]` — leaf tools are subcommands, not the root.
3. Root fields that are CLI-only (e.g. `-o`): `#[clap_mcp(skip)]`.
4. Subcommand enum: `#[clap_mcp(reinvocation_safe, parallel_safe)]` + `output_from` / `output_type`.
5. **Single dispatch:** `execute(cmd) -> Result<…>` shared by CLI and MCP; MCP `run` wraps with `AsStructured`.

Keep **required** `#[command(subcommand)]` — do not switch to `Option<Commands>` only for MCP (clap-mcp intercepts `--mcp` before subcommand checks).

### Setup then serve (embedder)

When Phase 1 finds setup-before-serve, a custom MCP entry (for example `myapp serve`),
or an existing JSON-RPC multiplexer:

1. Parse with `Cli::parse()` (or imperative `get_matches()`).
2. Run setup (load `--config`, init logging).
3. On the MCP branch, call `ServeMcpBuilder::for_cli::<T>(McpListen::Stdio)` (or
   `.new()` for hand-built schemas), then `.serve().await` or `.serve_blocking()`.

Do **not** use `parse_or_serve_mcp*` or `command_with_mcp_flag` unless the user
wants clap-mcp's builtin `--mcp`. Mark embedder-only subcommands with
`#[clap_mcp(skip)]`.

For an existing JSON-RPC multiplexer, chain
[`ServeMcpBuilder::stdio_io`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ServeMcpBuilder.html#method.stdio_io)(read, write)
on `McpListen::Stdio` (default is process stdin/stdout). Not valid with HTTP.

Upstream: [usage.md — Setup then serve](../../../docs/usage.md#setup-then-serve-embedder).
Examples: **setup_then_serve**, **async_embedder_serve**, **placeholder_server**.

### Preserve CLI parse

When native clap Usage on invalid human argv matters, use `parse_or_serve_mcp_preserve_cli()` instead of `parse_or_serve_mcp()`. See [usage.md — Preserve CLI parse](../../../docs/usage.md#preserve-cli-parse) and example **preserve_cli_parse** in [examples/README.md](../../../examples/README.md).

### Struct root with globals in `run`

When MCP tools must see global root flags, put `#[clap_mcp_output_from = "run_cli"]` on the struct and `#[clap_mcp(schema_only)]` on nested enums. Example: **struct_subcommand_globals** in [examples/README.md](../../../examples/README.md).

### Nested subcommands

Derive `ClapMcp` on each level; use `#[clap_mcp(schema_only)]` on intermediate enums. See [supported-cli-shapes.md](../../../docs/supported-cli-shapes.md) and **nested_subcommands** in [examples/README.md](../../../examples/README.md).

### Custom builtin flags

When `--mcp` collides with an existing flag, rename via `mcp_flag` / `ClapMcpBuiltinFlags`. See [execution-safety.md — Renaming clap-mcp builtin flags](../../../docs/execution-safety.md) and **custom_mcp_flags**.

### Stateful session tools

Only when tools need shared in-process session state: `parse_or_serve_mcp_with_state`, `#[clap_mcp(stateful)]`, `output_from_with_state`. See [stateful-tools.md](../../../docs/stateful-tools.md) and **stateful_counter**.

### Imperative clap

Custom flag systems (e.g. ripgrep's non-derive flags) use `get_matches_or_serve_mcp` + `ClapMcpSchemaMetadata`. Do not force `#[derive(ClapMcp)]` on non-clap types.

---

## Phase 4 — Execution config (async-first)

Details: [reference-execution.md](resources/reference-execution.md). Upstream: [execution-safety.md](../../../docs/execution-safety.md).

**Decision ladder** (prefer higher rungs when hazards are absent):

| Priority | Config | Use when |
|----------|--------|----------|
| 1 | `reinvocation_safe`, `parallel_safe = true`, topical `#[clap_mcp(serialized)]` | Most stateless/read-heavy tools; only a few writers conflict |
| 2 | Same + `share_runtime = true` + `clap_mcp::run_async_tool` | Async tool bodies; avoid nested runtime per call |
| 3 | `reinvocation_safe`, `parallel_safe = false` | Unsure which tools conflict; temporary until probed |
| 4 | Subprocess (default) | `process::exit` in tool path, non-idempotent global init, or hazards not fixable |

**Do not** set `parallel_safe = false` globally when only one subcommand touches shared mutable state — mark that variant `#[clap_mcp(serialized)]` or `#[clap_mcp(serialized = "path")]` instead.

**Skip** `#[clap_mcp(task)]` / `task_augmented_tools` unless the user explicitly wants MCP tasks.

**Opt-in panic catching:** `catch_in_process_panics = true` on `#[clap_mcp(...)]` when in-process tools may panic but the server should stay up. See [execution-safety.md — Crash and panic behavior](../../../docs/execution-safety.md#crash-exit-and-panic-behavior).

Fix hazards before enabling in-process:

- `tracing_subscriber::init()` → `try_init()` or init once before serve loop
- Interactive stdin → `#[clap_mcp(skip)]` or `#[clap_mcp(requires = "…")]`
- TTY branches → ensure headless MCP path completes

---

## Phase 5 — Structured output and schema metadata

Upstream: [tool-output.md](../../../docs/tool-output.md).

1. **`#[clap_mcp_output_from = "run"]`** — one `run` for CLI + MCP.
2. Return `Result<AsStructured<T>, E>` where `T: Serialize + JsonSchema` and `E: IntoClapMcpToolError`.
3. With `output-schema` feature: `#[clap_mcp(output_type = "…")]` or
   `#[clap_mcp_output_type = "…"]` on a **leaf** variant (not on a variant that
   only wraps `#[command(subcommand)]`).
4. Gate `JsonSchema` derives with `#[cfg_attr(feature = "mcp", derive(JsonSchema))]` on shared result types.

Subprocess mode does not set `structuredContent` from return types; in-process `AsStructured` does. See the subprocess vs in-process table in [tool-output.md](../../../docs/tool-output.md). Do not advertise `outputSchema` for tools that stay on the default subprocess path.

On Unix in-process tools, optional `ClapMcpServeOptions::capture_stdout` merges human stdout into text results (see [tool-output.md — capture_stdout](../../../docs/tool-output.md)). That redirects **process stdout during tool execution**, not the MCP transport. Custom transport I/O uses `ServeMcpBuilder::stdio_io`; see [logging.md — MCP transport I/O vs tool stdout](../../../docs/logging.md#mcp-transport-io-vs-tool-stdout).

**Metadata checklist:**

| clap pattern | Attribute |
|--------------|-----------|
| CLI-only variant (completion, shell, daemon) | bare `#[clap_mcp(skip)]` (hides the whole tool) |
| Hide specific args, keep the tool | `#[clap_mcp(skip = "arg_id,…")]` on the variant or field |
| Sensitive globals on every tool | `#[clap_mcp(skip_global = "…")]` on the root |
| `Option` positional / stdin fallback | `#[clap_mcp(requires = "field")]` |
| `hide = true` but still listed in MCP | `#[clap_mcp(skip)]` — `hide` does not hide from MCP ([hide vs skip](../../../docs/execution-safety.md#hide-vs-clap_mcpskip)) |
| Long-running command | `#[clap_mcp(task)]` (only if tasks ship) |

Keep human-default CLI output; MCP gets `structuredContent` from `run`'s return type — do not require agents to pass `-o json`.

---

## Phase 6 — Logging bridge (optional)

Wire **only** when the project already uses `tracing` or `log` and MCP clients should receive `notifications/message`.

- `tracing`: `ClapMcpTracingLayer` composes with existing layers; pass `log_rx` via `ClapMcpServeOptions` on [`ClapMcpRunOptions`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ClapMcpRunOptions.html) or [`ServeMcpBuilder::serve_options`](https://docs.rs/clap-mcp/latest/clap_mcp/struct.ServeMcpBuilder.html).
- `log`: `ClapMcpLogBridge` replaces the global logger — skip unless acceptable.

**Subprocess stderr:** default is [`SubprocessStderr::Capture`](https://docs.rs/clap-mcp/latest/clap_mcp/enum.SubprocessStderr.html) (append to result text; no `notifications/message`). Use `Notify` only when clients should receive stderr as logging notifications. See [logging.md — Subprocess stderr policy](../../../docs/logging.md#subprocess-stderr-policy).

If wiring cost exceeds value (no existing subscriber, CLI is silent), **omit** — do not add logging deps solely for MCP.

**Protocol deprecation (SEP-2577):** MCP logging (`notifications/message`,
`logging/setLevel`, `ServerCapabilities.logging`) is deprecated upstream.
clap-mcp still ships this bridge for **agent-facing** CLI diagnostics while a
tool runs; it remains supported on current rmcp. Spec migration advice (stderr /
OpenTelemetry) does not replace that client-visible stream. Prefer tool results
/`capture_stdout` for durable output; use the bridge when live notifications
matter. Track removal as a product risk — see
[logging.md](../../../docs/logging.md).

Upstream: [logging.md](../../../docs/logging.md).

---

## Phase 7 — Schema smoke test

Add a `#[cfg(feature = "mcp")]` test (pattern from [config_tests.rs](../../../clap-mcp/tests/config_tests.rs)):

```rust
#[cfg(feature = "mcp")]
#[test]
fn mcp_schema_exposes_leaf_tools_and_respects_skip() {
    use clap_mcp::schema_from_command_with_metadata;
    let schema = schema_from_command_with_metadata(
        &MyRoot::command(),
        &MyRoot::clap_mcp_schema_metadata(),
    );
    let leaves: Vec<_> = schema.root.all_commands()
        .filter(|c| c.subcommands.is_empty())
        .map(|c| c.canonical_name().to_string())
        .collect();
    assert!(leaves.iter().any(|n| n == "expected-tool"));
    assert!(!leaves.iter().any(|n| n == "skipped-variant"));
}
```

Run:

```shell
cargo test --features mcp
cargo test   # default features still pass
```

---

## Phase 8 — Runtime smoke

After compile succeeds:

1. `cargo run --features mcp -- --help` — expect `--mcp`, `--export-skills` (and `--mcp-http` if `http` feature), unless the embedder path uses a custom entry (for example `myapp serve`).
2. Start stdio MCP: `cargo run --features mcp -- --mcp` (or dist binary / custom `serve` subcommand).
3. Send MCP `tools/list`; confirm leaf tool names match intent.
4. `tools/call` one read-only tool; confirm `structuredContent` or text.

Use clap-mcp **client** example or project MCP tooling — not fabricated PASS results.

---

## Reference (read when implementing)

| Topic | Resource |
|-------|----------|
| Dependency (crates.io / path / git) | [reference-dependency.md](resources/reference-dependency.md) |
| Derive patterns, dual binary, CI | [reference-patterns.md](resources/reference-patterns.md) |
| Async, parallel_safe, serialized topics | [reference-execution.md](resources/reference-execution.md) |
| Search / ripgrep-shaped passthrough | [reference-search-cli.md](resources/reference-search-cli.md) |
| Feature gate, dist binary, CI matrix | [reference-distribution.md](resources/reference-distribution.md) |

### Upstream docs (in-tree)

| Topic | Doc |
|-------|-----|
| Usage / dual derive | [usage.md](../../../docs/usage.md) |
| Setup then serve (embedder) | [usage.md — Setup then serve](../../../docs/usage.md#setup-then-serve-embedder) |
| Execution safety | [execution-safety.md](../../../docs/execution-safety.md) |
| Structured output | [tool-output.md](../../../docs/tool-output.md) |
| CLI shapes | [supported-cli-shapes.md](../../../docs/supported-cli-shapes.md) |
| Stateful tools | [stateful-tools.md](../../../docs/stateful-tools.md) |
| HTTP listen | [http.md](../../../docs/http.md) |
| Logging | [logging.md](../../../docs/logging.md) |
| MCP tasks | [mcp-tasks.md](../../../docs/mcp-tasks.md) |
| Migration / bumps | [migration-notes.md](../../../docs/migration-notes.md) |
| Examples index | [examples/README.md](../../../examples/README.md) |
| Documentation index | [README.md](../../../README.md#documentation) |

---

## Anti-patterns

| Do not | Do instead |
|--------|------------|
| Flatten subcommands for MCP | `skip_root_when_subcommands`, leaf tools |
| Global `parallel_safe = false` for one conflicting tool | `#[clap_mcp(serialized)]` on that variant |
| Duplicate business logic in MCP handlers | Shared `execute` + `run` wrapper |
| `Option<Subcommand>` only for MCP | Keep required subcommand; clap-mcp handles `--mcp` |
| `parse_or_serve_mcp` when config/globals must load first | `parse` → setup → `ServeMcpBuilder::for_cli` |
| Conflate MCP transport with process stdout / `capture_stdout` | `stdio_io` for transport; `capture_stdout` only for tool execution |
| `stdio_io` with `McpListen::Http` | Stdio transport only; use HTTP listen without `stdio_io` |
| Skip schema test | `schema_from_command_with_metadata` unit test |
| Rely on `hide = true` to exclude MCP tools | bare `#[clap_mcp(skip)]` |
| Use `#[clap_mcp(skip = "a,b")]` to hide a whole variant | bare `#[clap_mcp(skip)]` ([migration notes](../../../docs/migration-notes.md#010-rc1--010-rc2)) |
| Add tracing bridge to a silent CLI | Omit Phase 6 |
| Expect default subprocess stderr as `notifications/message` | Default is `Capture`; use `SubprocessStderr::Notify` to opt in |

---

## Completion checklist

- [ ] User answered feature-gate question (Phase 0)
- [ ] clap-mcp added per [reference-dependency.md](resources/reference-dependency.md)
- [ ] `parse_or_serve_mcp` wired; shared `execute` dispatch
- [ ] Execution config justified (`parallel_safe` + topical `serialized` when possible)
- [ ] `skip` / `requires` on stdin, interactive, and CLI-only paths
- [ ] Structured output via `AsStructured` + optional `output_type`
- [ ] Schema smoke test passes under `--features mcp`
- [ ] Default-feature `cargo test` passes
- [ ] At least one live `tools/list` + `tools/call` smoke
