# clap-mcp

> **Enrich your Rust CLI with MCP Capabilities**

[![crates.io](https://img.shields.io/crates/v/clap-mcp.svg)](https://crates.io/crates/clap-mcp)
[![docs.rs](https://docs.rs/clap-mcp/badge.svg)](https://docs.rs/clap-mcp)

## Usage

This is still a draft, and we're exposing a rapidly evolving specification (MCP)
through a relatively stable one (`clap`). That mismatch in velocity will err
towards instability of the public API surface.

> **In general, you should be able to adapt any `clap` CLI binary to use
> `clap-mcp` with natural API semantics.**

* **[Derive with attributes (recommended)](docs/usage.md#derive-with-attributes-recommended)** —
  `#[clap_mcp(...)]` for execution safety and in-process tools
* **[Derive (minimal)](docs/usage.md#derive-minimal)** — `#[derive(ClapMcp)]` on a
  `Parser` enum with `parse_or_serve_mcp`
* **[Imperative (existing clap CLI)](docs/usage.md#imperative-existing-clap-cli)** —
  add MCP to a hand-built `clap::Command` with `get_matches_or_serve_mcp`

Runnable server binaries and feature demos:
[examples/servers](examples/servers).

## Design

Compared to a Command Line Interface, I'm not a huge fan of the
[Model Context Protocol](https://modelcontextprotocol.io/docs/getting-started/intro),
but my feelings don't represent real world usage patterns. I feel MCP would do better
with gRPC and Protobuf as it's "transport." All that being said, I'm not bitter
about it, so I'm letting a model do the development work and deal with its
own self-generated mess.

**The intent is generally:**

* Make it easy to add a MCP server to current Rust CLIs that use `clap`.
* Have it work well enough and provide enough guardrails to cover the 95% case.
* If there is structured information available from the CLI as an outcome, we
  should provide a way to express it naturally via MCP.
* Provide a way to express structured logging information (if available) as part
  of the response if requested.
* Avoid being opinionated if we don't have to be, accept being as little
  opinionated as possible if the alternative is complicating the primary public
  API.

Overall, the more you design your CLI around a service pattern, the more
naturally this crate will behave as an MCP server, and modern CLIs often do
that. At the same time, we shouldn't force CLIs that don't do that, out of the
ecosystem.

> [!WARNING]
> Clanker generated code, running an auto-release pipeline, without a stable API
> yet.

## Crate Features

Add `clap-mcp` to your `Cargo.toml` (the default `derive` feature includes the
macro):

```toml
[dependencies]
clap-mcp = "0.0.4-rc.1"
```

* Opt-in MCP server on existing `clap` CLIs (`--mcp` stdio, `--mcp-http` with
  `http` feature)
* `#[derive(ClapMcp)]` — subcommands exposed as MCP tools with shared `run`
  output
* Execution modes: subprocess (default) or in-process (`reinvocation_safe`);
  for async tool bodies, separate runtime per call (default) or shared MCP
  async runtime (`share_runtime`); see
  [execution-safety](docs/execution-safety.md#async-tools-and-share_runtime)
* Opt-in in-process panic catching (`catch_in_process_panics` on
  `#[clap_mcp(...)]`); panics become MCP errors instead of crashing the server;
  see [Crash and panic behavior](docs/execution-safety.md#crash-and-panic-behavior)
* Logging forwarded to MCP clients as `notifications/message` (`tracing` /
  `log` features); see [logging](docs/logging.md)
* Structured tool output and optional JSON `outputSchema`; see
  [tool-output](docs/tool-output.md)
* Custom MCP resources and prompts; see [custom-content](docs/custom-content.md)
* Agent Skills export (`--export-skills`); see
  [export-skills](docs/export-skills.md)
* Stateful in-process session tools; see
  [stateful-tools](docs/stateful-tools.md)
* MCP task-augmented `tools/call`; see [MCP tasks support](docs/mcp-tasks.md)

### Feature Flags

Cargo features and their maturity:

| Maturity | Meaning |
| --- | --- |
| **Shipped** | Supported public API surface; exercised in CI and examples. |
| **Scaffolding** | Exploratory spike — API and behavior may change; not a conformance or release parity target. |

| Flag | Maturity | Enables |
| --- | --- | --- |
| `derive` (default) | Shipped | `#[derive(ClapMcp)]` proc-macro and `ParseOrServeMcp` |
| `tracing` | Shipped | `ClapMcpTracingLayer` — a `tracing_subscriber::Layer` that forwards tracing events to MCP clients via `notifications/message`. |
| `log` | Shipped | `ClapMcpLogBridge` — a `log::Log` implementation that forwards `log` crate messages to MCP clients. |
| `output-schema` | Shipped | `schemars`-based JSON schema generation for structured tool output. Enables [`output_schema_for_type`], [`output_schema_one_of!`], and `#[clap_mcp_output_type]` / `#[clap_mcp_output_one_of]` to set each tool's `output_schema` for MCP clients. |
| `http` | Shipped | Streamable HTTP MCP server (`--mcp-http`); see [http.md](docs/http.md). |
| `http-oauth` | Scaffolding | OAuth client helpers for calling remote MCP servers; see [oauth.md](docs/oauth.md). |
| `elicitation` | Scaffolding | Server-side elicitation during tool execution (`confirm-echo` intercept only). |

Enable features in `Cargo.toml`:

```toml
[dependencies]
clap-mcp = { version = "0.0.4-rc.1", features = ["tracing"] }
```

## When and when not to use `clap-mcp`

**Use clap-mcp** when you already have (or are building) a **`clap` binary** that
users or agents invoke as **discrete, one-shot, argv-shaped tools**, especially
when each subcommand has predictable inputs and outputs.

**Strong fits:**

* **Plain CLI utilities** — search, format, inspect, convert — where you
  understand concurrency and side effects. Stateless read-only tools map cleanly
  to in-process execution; mutating or lock-heavy tools can stay on subprocess
  defaults until you have measured behavior.
* **CLI frontends for remote services** — a binary that parses args and calls
  HTTP, gRPC, or another RPC backend. MCP belongs on the **CLI users run**
  (`myctl get …`, `myctl apply …`); the service process itself may use `clap`,
  a different framework, or no CLI at all. Agents get your existing subcommands
  and flags; your CLI keeps doing the RPC/client work it already does.

**Weaker fits (often still possible with care):**

* **Interactive TUIs** (`lazygit`, `gitui`) — full-screen terminal apps assume a
  human at the keyboard; MCP tool calls are non-interactive unless you expose
  separate non-TUI subcommands.
* **Long-running dev loops** (`bacon`, `trunk serve`, `cargo watch`) — MCP tools
  are modeled as invocations with a result, not an always-on watcher or server
  you leave running inside one tool call.
* **Library-only crates** — clap-mcp attaches to a **binary**; wrap library APIs
  in a CLI first, or MCP-ify an existing bin target.

The table below is **illustrative** — suggested starting points for familiar
Rust CLIs, not official guidance from those projects. Tune flags after you know
your tool's locking, I/O, and global state. Details:
[Execution safety](docs/execution-safety.md),
[Usage patterns](docs/usage.md).

| Rust CLI | Typical role | Suggested starting config | Why |
| --- | --- | --- | --- |
| [`ripgrep`](https://github.com/BurntSushi/ripgrep) (`rg`) | Stateless search | `reinvocation_safe`, `parallel_safe = true` | Read-only; no shared process state between calls |
| [`fd`](https://github.com/sharkdp/fd) | Stateless find | `reinvocation_safe`, `parallel_safe = true` | Same pattern as `rg` |
| [`bat`](https://github.com/sharkdp/bat) | Read/pretty-print files | `reinvocation_safe`, `parallel_safe = true` | Read-mostly; safe to overlap |
| [`tokei`](https://github.com/XAMPPRocky/tokei) | Count lines / stats | `reinvocation_safe`, `parallel_safe = true` | Walks trees read-only |
| [`xh`](https://github.com/ducaale/xh) | HTTP client (calls remote API) | `reinvocation_safe`, `parallel_safe = true` | Thin CLI over network; backend is separate |
| gRPC/`tonic` admin CLI | RPC client (calls remote service) | `reinvocation_safe`, `parallel_safe = true` | Your user's case: MCP on the client binary |
| [`cargo`](https://github.com/rust-lang/cargo) | Build / resolve / lock | Default (subprocess) or `reinvocation_safe`, `parallel_safe = true`, `#[clap_mcp(serialized)]` on mutating subcommands | Registry and artifact locks; see [topical serialization](docs/execution-safety.md#topical-serialization) |
| [`sqlx-cli`](https://github.com/launchbadge/sqlx) | DB migrate / prepare | Default (subprocess) | Schema mutations; prefer process isolation |
| [`wasm-pack`](https://github.com/rustwasm/wasm-pack) | Build wasm via cargo | Default (subprocess) | Spawns nested toolchains |
| [`hyperfine`](https://github.com/sharkdp/hyperfine) | Benchmark runner | Default (subprocess) | Spawns arbitrary shell commands per run |
| In-process session tool (e.g. counter) | Shared MCP session state | `reinvocation_safe`, `stateful` | See [stateful-tools](docs/stateful-tools.md) |
| Long-running subcommand (sleep, batch job) | Task-augmented `tools/call` | `reinvocation_safe`, `task_augmented_tools` | See [mcp-tasks](docs/mcp-tasks.md) |
| [`lazygit`](https://github.com/jesseduffield/lazygit) | Full-screen TUI | **Poor fit** as-is | Needs dedicated non-TUI subcommands for MCP |
| [`bacon`](https://github.com/Canop/bacon) | Test/file watcher | **Poor fit** | Long-lived loop, not a one-shot tool |
| Backend service only (no user-facing bin) | gRPC/HTTP server | **Wrong layer** | Add a CLI (or MCP elsewhere); clap-mcp targets the invoke binary |

**Default (no attributes)** is always valid: subprocess execution
(`reinvocation_safe = false`) serializes calls and respawns your binary per tool
invocation — the safest baseline when you are unsure.

## Documentation

Every guide in [`docs/`](docs/) is listed below. See also
[examples/README.md](examples/README.md) for runnable binaries.

### Guides for CLI authors

| Guide | Topics |
| --- | --- |
| [Usage patterns](docs/usage.md) | Derive (minimal / with attributes), imperative CLI, struct root |
| [Custom resources and prompts](docs/custom-content.md) | `ClapMcpServeOptions`, static/dynamic content |
| [Exporting agent skills](docs/export-skills.md) | `--export-skills`, SKILL.md generation |
| [Execution safety](docs/execution-safety.md) | `reinvocation_safe`, topical serialization, skip/requires, nested metadata, dual derive, async embedders |
| [MCP tasks support](docs/mcp-tasks.md) | Task-augmented `tools/call`, examples, support matrix |
| [Stateful MCP tools](docs/stateful-tools.md) | Shared session state, `parse_or_serve_mcp_with_state` |
| [Security](docs/security.md) | Schema validation, subprocess model, trust boundaries |
| [Tool output](docs/tool-output.md) | `run` return types, structured output, `output-schema` |
| [Logging](docs/logging.md) | `tracing` / `log` bridges, MCP notifications |
| [Streamable HTTP](docs/http.md) | `--mcp-http`, listen env vars |
| [OAuth (scaffolding)](docs/oauth.md) | Remote MCP client helpers |
| [Migration notes (0.0.3 → 0.0.4)](docs/migration-notes.md) | Breaking changes, rmcp port, API renames |

### Maintainer notes

| Guide | Topics |
| --- | --- |
| [Conformance baseline](docs/conformance-baseline.md) | `cargo xtask conformance`, baseline YAML |
| [Maintainer testing](docs/maintainer-testing.md) | Macro checklist, `complex_cli` / `example_contract` filters, `.agents/rules/` |

## CLI compatibility

For derive usage, `use clap_mcp::ClapMcp` so you can write `#[derive(ClapMcp)]`.
Integration patterns: [Usage patterns](docs/usage.md).

Adding clap-mcp should not change how your CLI runs unless you explicitly opt
into MCP.

1. **MCP is flag-opt-in only.** A server starts only when the user passes **`--mcp`**
   (stdio) or **`--mcp-http`** ([`http`](docs/http.md) feature). Normal invocations
   never accidentally enter MCP mode.

2. **Non-MCP behavior is unchanged.** Any argv **without** an MCP flag must
   parse and run the same as before you added clap-mcp: same errors, same
   success paths, same subcommand rules. Swap `Cli::parse()` for
   [`ParseOrServeMcp::parse_or_serve_mcp`] (or [`get_matches_or_serve_mcp`]
   imperatively) — **do not** change subcommand types or `subcommand_required`
   unless you already planned to.

3. **`--mcp` does not require `Option<Commands>`.** If your CLI already uses a
   required subcommand (`command: Commands` + `subcommand_required = true`),
   keep it. clap-mcp checks for `--mcp` **before** clap's subcommand validation,
   so `myapp --mcp` works while bare `myapp` still errors exactly as clap did
   before.

| Invocation | Flat enum CLI (no struct subcommand) | Required struct subcommand | Optional struct subcommand (`Option<Commands>`) |
|---|---|---|---|
| Normal args (no MCP flag) | Unchanged | Unchanged | Unchanged |
| Bare root (no subcommand) | N/A or app-defined | **Still clap error** | Parses; `main` handles `None` |
| `--mcp` / `--mcp-http` | MCP server | MCP server | MCP server |

**Do not migrate to `Option<Commands>` solely for MCP** — that changes bare-invocation
behavior for CLIs that previously required a subcommand. See
[Execution safety — dual derive](docs/execution-safety.md#dual-derive--root-and-subcommand)
and **struct_subcommand_required** in [examples/README.md](examples/README.md).

Passthrough (`--`), renaming builtin MCP flags, struct-root derive, and the
three integration patterns (derive / imperative): [Usage patterns](docs/usage.md)
and [Execution safety — CLI compatibility details](docs/execution-safety.md#cli-compatibility-details).

## Development

Contributors should follow these conventions. AI agents should also read
[AGENTS.md](AGENTS.md) for design priorities, documentation style, and doc
touchpoints.

* Format code with `cargo fmt`. CI runs `cargo fmt --all -- --check`.
* Run `cargo clippy --all-targets --all-features -- -D warnings` before
  submitting; CI enforces this.
* Document public API items and add a `// SAFETY:` comment above any `unsafe`
  block explaining invariants.

MCP task support matrix (including limitations) is in
[MCP tasks support](docs/mcp-tasks.md).

Run all tests (including feature-gated logging tests):

```shell
cargo test --all-features
```

### Code coverage

Coverage is measured with
[cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov). Install and run:

```shell
cargo install cargo-llvm-cov
cargo llvm-cov test --workspace --all-features --summary-only
```

For an HTML report:

```shell
cargo xtask code-coverage-html
```

Add `--open` to launch the report in a browser when it finishes:

```shell
cargo xtask code-coverage-html --open
```

Coverage focuses on the `clap-mcp` and `clap-mcp-macros` crates; the `examples`
crate is excluded from coverage targets.

Release prep runs example smoke via `cargo xtask examples-help` (builds with
`--all-features`, runs `--help` on each release-validation binary); see
[examples/README.md](examples/README.md).
