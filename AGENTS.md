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
[rmcp migration notes](docs/rmcp-migration-notes.md) for recent API slimming.

## Repository layout

| Path | Role |
|------|------|
| `clap-mcp/` | Main library (`lib.rs`, `serve.rs`, `server.rs`, `http.rs`, …) |
| `clap-mcp/macros/` | `#[derive(ClapMcp)]` proc-macro |
| `examples/` | Runnable server/client examples; see [examples/README.md](examples/README.md) |
| `docs/` | HTTP, OAuth, conformance, migration notes |
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

Also when adding or changing public API:

* Document new public items; add `// SAFETY:` above any `unsafe` block with
  invariants explained.

Optional but useful locally:

```bash
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

## Documentation touchpoints

When changing public embedder or derive APIs, update as appropriate:

* [README.md](README.md) — user-facing overview and async embedder section
* [AGENTS.md](AGENTS.md) — this file; keep design priorities in sync with README
* [examples/README.md](examples/README.md) — how to run examples
* [docs/http.md](docs/http.md) — Streamable HTTP listen
* [docs/rmcp-migration-notes.md](docs/rmcp-migration-notes.md) — API migration table

## Git

* Do not commit unless the user asks.
* Do not bump the crate version unless explicitly requested (branch may stay on
  `0.0.4-rc.1` during API polish).
