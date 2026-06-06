# Agent guide (clap-mcp)

This file orients AI agents and contributors working in this repo. User-facing
docs live in [README.md](README.md); this guide captures conventions and
priorities that are easy to miss when editing code.

## Design principles

The product goals and rationale are in [README.md — Design](README.md#design).
When making API or architecture choices, align with those goals **and** the
priorities below.

**Intent (from README):**

* Make it easy to add an MCP server to existing Rust CLIs that use `clap`.
* Work well enough with guardrails for the common case (~95%).
* Express structured CLI outcomes naturally via MCP when available.
* Support structured logging in MCP responses when the CLI provides it.
* Stay minimally opinionated when the alternative is complicating the primary
  public API.
* Favor service-shaped CLIs without excluding simpler ones.

**Additional priorities for this codebase:**

* **Idiomatic Rust** — Prefer standard Rust patterns (builders, traits, clear
  error types, async/blocking layering) over bespoke APIs. Public surfaces should
  feel like they belong in the ecosystem; avoid stringly-typed config when a
  structured variant fits.
* **Minimal, focused diffs** — Change only what the task requires; match
  surrounding style and abstractions.
* **Stable embedder ergonomics** — Derive path (`parse_or_serve_mcp*`) for most
  users; imperative path (`ServeMcpBuilder`, then lower-level `serve_mcp*`) for
  embedders. Do not remove thin delegators without an explicit breaking-change
  decision.

Early-stage warning: the crate is pre-stable API (`0.0.4-rc.1`). See
[migration notes](docs/migration-notes.md) for recent API slimming.

## Repository layout

| Path | Role |
|------|------|
| `clap-mcp/` | Main library (`lib.rs`, `serve.rs`, `server.rs`, `http.rs`, …) |
| `clap-mcp/macros/` | `#[derive(ClapMcp)]` proc-macro |
| `examples/` | Runnable server/client examples; see [examples/README.md](examples/README.md) |
| `docs/` | CLI author guides and maintainer notes; see [Documentation layout](#documentation-layout) |
| `xtask/` | Maintainer tasks (e.g. MCP conformance harness) |

## Development workflow

### When to run the full gate

Run the checks below **after completing a plan or any heavy agent-driven change**
(API refactors, new public types, example migrations, broad test/doc updates).
They are **required before every release**, with no exceptions.

For day-to-day edits, match the same bar when the change touches public API,
build graphs, or cross-crate behavior; smaller fixes should still pass fmt and
the tests that cover the touched code.

The authoritative checklist is
[`.github/workflows/ci.yml`](.github/workflows/ci.yml). Its steps generally
encapsulate everything needed before merge or release; local runs may add extras
(e.g. coverage, client smoke) but should not skip what CI enforces.

### CI-equivalent commands

Contributors and agents should be able to reproduce CI locally:

* **Format:** `cargo fmt --all -- --check`
* **Test:** `cargo test --all-features`
* **Clippy:** `cargo clippy --all-targets --all-features -- -D warnings` (includes
  examples and tests; no warnings allowed)
* **Audit:** CI uses [rustsec/audit-check](https://github.com/rustsec/audit-check)
  (`rustsec/audit-check@v2.0.0` on Ubuntu); locally `cargo audit` is equivalent
* **Examples:** `cargo xtask examples-help` (builds `clap-mcp-examples` with
  `--all-features`, runs `--help` on each release-validation binary; use
  `cargo xtask examples-help --list` to print the list, `--profile http` or
  `--profile all` for other smoke sets)
* **Rustdoc:** `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p clap-mcp --all-features`
* **Markdown (optional):** `rumdl check README.md AGENTS.md docs/*.md` — see
  [Documentation style guide](#documentation-style-guide)

Also when adding or changing public API:

* Document new public items; add `// SAFETY:` above any `unsafe` block with
  invariants explained.

Optional but useful locally:

```shell
# Example smoke (async embedder path)
cargo run -p clap-mcp-examples --bin client --features tracing -- async-embedder-serve

# Coverage (clap-mcp + macros; examples excluded)
cargo install cargo-llvm-cov
cargo llvm-cov test -p clap-mcp -p clap-mcp-macros --all-features --summary-only
cargo xtask code-coverage-html          # HTML report in target/llvm-cov/html
cargo xtask code-coverage-html --open   # same, then open in browser
```

CI runs the gate above on Ubuntu, Windows, and macOS.

## Testing expectations

* Do not disable, ignore, or delete tests to make CI pass — fix behavior or
  expectations.
* Integration tests under `clap-mcp/tests/` exercise real example binaries via
  stdio MCP; prefer extending existing helpers in `clap-mcp/tests/common/`.
* UI tests (`trybuild`) guard macro diagnostics in `clap-mcp/tests/ui/`.

## Documentation layout

**Code examples.** If a snippet is long enough to be a full `main.rs`, write it
as one: include `fn main`, required imports, and enough structure to compile when
 pasted into a binary crate with `clap-mcp` (and any named features) in
`Cargo.toml`. Do not use `// ...`, `{ ... }`, or placeholder bodies in examples
that size — link to [examples/servers/](examples/servers/) instead. Smaller
fragments (a single attribute line, an API call) may stay incomplete when labeled
as excerpts.

**Documentation is part of the API.** Stale or missing docs are a defect, not a
follow-up. Do not land public API, feature flag, or CLI author behavior changes
without updating the README Documentation index and every affected guide. If you
add `docs/new-topic.md`, you must add it to [README `#documentation`](README.md#documentation)
in the same change. If you move or rename sections, grep the repo for broken
links. Agents: treating docs as optional is incorrect. Fix them before claiming
the task is complete.

**README vs `docs/`.** The root [README.md](README.md) may showcase many
capabilities (overview, quick paths, feature bullets, short examples). Prefer
**depth in an existing topical file under `docs/`** — do not add a new guide when
one already covers the topic; extend that file instead. For anything beyond a
brief mention in README, link to the relevant `docs/*.md` and put the detailed
prose, tables, and long examples there.

### What lives where

| Location | Role |
| --- | --- |
| [README.md](README.md) | Overview: Usage (links to `docs/usage.md`), Design, **Crate Features**, **Documentation** index, CLI compatibility rules, Development — showcase and link; avoid long-form detail |
| [`docs/usage.md`](docs/usage.md) | Top embedder integration patterns (derive minimal / with attributes, imperative) with compilable examples; link out for depth |
| [`docs/*.md`](docs/) (other) | Deep CLI author guides and maintainer notes — one topic per file; extend an existing file before adding a new one; link back to README `#documentation` |
| [examples/README.md](examples/README.md) | Runnable examples index; link to relevant `docs/` guides, do not duplicate them |
| [AGENTS.md](AGENTS.md) | Agent/contributor conventions including this layout |

### `docs/` inventory

Every file below must appear in the README Documentation table.

| File | Audience | Purpose |
| --- | --- | --- |
| [docs/usage.md](docs/usage.md) | CLI authors | Derive (minimal / with attributes), imperative CLI, struct-root pointer |
| [docs/custom-content.md](docs/custom-content.md) | CLI authors | Custom MCP resources and prompts |
| [docs/export-skills.md](docs/export-skills.md) | CLI authors | `--export-skills`, Agent Skills generation |
| [docs/execution-safety.md](docs/execution-safety.md) | CLI authors | `reinvocation_safe`, skip/requires, dual derive, async embedders |
| [docs/mcp-tasks.md](docs/mcp-tasks.md) | CLI authors | Task-augmented `tools/call`, examples, support matrix |
| [docs/stateful-tools.md](docs/stateful-tools.md) | CLI authors | Shared session state, `parse_or_serve_mcp_with_state` |
| [docs/security.md](docs/security.md) | CLI authors | Schema validation, subprocess trust model |
| [docs/tool-output.md](docs/tool-output.md) | CLI authors | `run` return types, structured output, `output-schema` |
| [docs/logging.md](docs/logging.md) | CLI authors | `tracing` / `log` bridges, MCP notifications |
| [docs/http.md](docs/http.md) | CLI authors | Streamable HTTP listen (`--mcp-http`) |
| [docs/oauth.md](docs/oauth.md) | CLI authors | OAuth client helpers (scaffolding) |
| [docs/migration-notes.md](docs/migration-notes.md) | CLI authors / maintainers | 0.0.3-rc.1 → 0.0.4-rc.1 upgrade, breaking renames |
| [docs/conformance-baseline.md](docs/conformance-baseline.md) | Maintainers | MCP conformance harness, baseline YAML |

## Documentation style guide

Write like you are explaining something to a colleague. Be direct, specific, and
concise.

Apply this guide to [README.md](README.md), [AGENTS.md](AGENTS.md), and every
file under [`docs/`](docs/). Embedder guides use this blockquote intro:

```markdown
> Guide for CLI authors adding clap-mcp. See [README](../README.md) to get started.
```

### Voice and tone

* Use active voice. "`clap-mcp` exposes your subcommands as MCP tools when you
  pass `--mcp`" not "MCP tools are exposed from your subcommands by
  `clap-mcp`."
* Use second person ("you") when addressing the reader.
* Use present tense. "The command returns an error" not "The command will return
  an error."
* State facts. Do not hedge with "simply," "just," "easily," or "of course."

### Things to avoid

These patterns are common in hard-to-read text and erode trust with technical
readers. Remove them during review.

| Pattern | Problem | Fix |
| --- | --- | --- |
| Unnecessary bold | "This is a **critical** step" on routine instructions. | Reserve bold for UI labels, parameter names, and genuine warnings. |
| Em dashes everywhere | "`clap-mcp` — which adds MCP to clap — serializes tool calls." | Use commas or split into two sentences. Em dashes are fine sparingly but should not appear multiple times per paragraph. |
| Superlatives | "`clap-mcp` provides a powerful, robust, seamless MCP experience." | Say what it does, not how great it is. |
| Hedge words | "Simply run with `--mcp`" or "You can easily enable logging..." | Drop the adverb. "Run with `--mcp`." |
| Emoji in prose | "🚀 Let's get started!" | No emoji in documentation prose. |
| Rhetorical questions | "Want MCP on your CLI? Look no further!" | State the purpose directly. |

### Formatting rules

* NEVER add line breaks inside an *italic* or **bold** span. If you must wrap a
  line, start the span again on the new line.
* NEVER add line breaks inside `[markdown](links)`.
* End every sentence with a period.
* Use `` `code` `` formatting for CLI commands, file paths, flags, parameter
  names, and values.
* Use `shell` code blocks for copyable CLI examples. Do not prefix commands with
  `$`:

  ```shell
  cargo install cargo-llvm-cov
  cargo llvm-cov test -p clap-mcp --all-features --summary-only
  ```

* Use `text` code blocks for transcripts, log output, and examples that should
  not be copied verbatim.
* Use tables for structured comparisons. Keep tables simple (no nested
  formatting).
* Use GitHub alert syntax for callouts, not bold text: `> [!NOTE]`,
  `> [!TIP]`, and `> [!WARNING]`.
* Use itemized bullet lists when the instructions clearly benefit from them.
* Do not number section titles. Write "Add MCP to a derive CLI" not "Section 1:
  Add MCP" or "Step 3: Verify."
* Do not use colons in titles. Write "Streamable HTTP listen" not "HTTP: Streamable
  listen."
* Use colons only to introduce a list. Do not use colons as general-purpose
  punctuation between clauses.

### Hand-written docs (README, AGENTS, docs/)

* Optional: [`rumdl`](https://github.com/rvben/rumdl) if installed. Config:
  [`.rumdl.toml`](.rumdl.toml) (covers `README.md`, `AGENTS.md`, `docs/*.md`).
  Run: `rumdl check README.md AGENTS.md docs/*.md`

## Documentation touchpoints

When changing **public derive or serve API**, update **all that apply** in the
same change:

* Relevant guide in `docs/` (or add a new file **and** a README Documentation row)
* [Documentation style guide](#documentation-style-guide) — voice, tone, formatting
  rules, and rumdl for README and `docs/*.md`
* [README.md](README.md) — Crate Features bullets, Feature Flags table, CLI
  compatibility pointers, Documentation index
* [examples/README.md](examples/README.md) — if examples or flags change
* [docs/migration-notes.md](docs/migration-notes.md) — breaking renames or API
  slimming
* [AGENTS.md](AGENTS.md) — if layout or agent rules change

When adding or editing **code examples**, follow the code-examples rule under
[Documentation layout](#documentation-layout): full `main.rs`-sized blocks must
compile; link to `examples/servers/` for longer demos.

Common mappings: integration patterns → `docs/usage.md`; HTTP listen →
`docs/http.md`; logging bridges → `docs/logging.md`;
execution flags → `docs/execution-safety.md`; MCP tasks → `docs/mcp-tasks.md`;
stateful derive → `docs/stateful-tools.md`; tool output / schemas →
`docs/tool-output.md`.

## Git

* Do not commit unless the user asks.
* Do not bump the crate version unless explicitly requested (branch may stay on
  `0.0.4-rc.1` during API polish).
