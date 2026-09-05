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

Early-stage warning: the public API is still early on the `0.1.x` line (see
workspace `Cargo.toml`). Prefer [migration notes](docs/migration-notes.md) when
porting across RC boundaries or rmcp / MCP protocol bumps.

## Repository layout

| Path | Role |
|------|------|
| `clap-mcp/` | Main library (`lib.rs`, `serve.rs`, `server.rs`, `http.rs`, …) |
| `clap-mcp/macros/` | `#[derive(ClapMcp)]` proc-macro |
| `examples/` | Runnable server/client examples; see [examples/README.md](examples/README.md) |
| `docs/` | CLI author guides and maintainer notes; see [Documentation layout](#documentation-layout) |
| `xtask/` | Maintainer tasks (e.g. MCP conformance harness) |

## Agent rules (required)

Hand-authored, path-scoped checklists live in
[`.agents/rules/`](.agents/rules/) ([agent-rules-spec RFC](https://github.com/rameshsunkara/agent-rules-spec)
draft). Generated `--export-skills` output is separate under `.agents/skills/`
(gitignored).

**Agents must use the rules, not duplicated prose in this file.**

1. After editing, read every rule with `trigger: auto` whose `paths` globs match
   a file you touched (or `trigger: always` rules). You do not need to parse the
   full schema — read the markdown body and run the commands it lists.
2. Treat rule bodies as mandatory when their paths match; they override summary
   bullets here.
3. Before finish, commit, or push on Rust / workflow / test changes, follow
   [`.agents/rules/clap-mcp-ci-gate.md`](.agents/rules/clap-mcp-ci-gate.md).

| Rule | Paths (summary) | When |
| --- | --- | --- |
| [`clap-mcp-ci-gate.md`](.agents/rules/clap-mcp-ci-gate.md) | `.github/workflows/**`, `clap-mcp/**`, `examples/**`, `xtask/**` | Full local CI gate before finish |
| [`clap-mcp-macros.md`](.agents/rules/clap-mcp-macros.md) | `clap-mcp/macros/**` | Proc-macro / derive edits |
| [`clap-mcp-lib.md`](.agents/rules/clap-mcp-lib.md) | `clap-mcp/src/**` | Runtime library edits |
| [`clap-mcp-examples.md`](.agents/rules/clap-mcp-examples.md) | `examples/**` | Example binary edits |
| [`clap-mcp-readme.md`](.agents/rules/clap-mcp-readme.md) | `README.md` | crates.io-safe absolute links in root README |
| [`clap-mcp-protected-prose.md`](.agents/rules/clap-mcp-protected-prose.md) | `README.md`, `docs/logging.md` | Do not rewrite protected human prose |

Human-oriented detail and tables: [docs/maintainer-testing.md](docs/maintainer-testing.md).

## Protected human prose

These passages are **maintainer voice**. Agents must not rewrite, paraphrase,
shorten, “tone-check,” relocate, or delete them. You may edit other parts of the
same files. If a task seems to require changing protected text, stop and ask the
maintainer.

| Location | Protected content |
| --- | --- |
| [README.md — Design](README.md#design) | The full Design section: first-person rationale, intent bullets, closing paragraph, and the Clanker `> [!WARNING]` callout |
| [docs/logging.md](docs/logging.md) | The author `> [!NOTE]` immediately after the SEP-2577 `> [!WARNING]` (begins “This was probably one of the most useful features…”) |

Path-scoped checklist:
[`.agents/rules/clap-mcp-protected-prose.md`](.agents/rules/clap-mcp-protected-prose.md).

## Development workflow

### When to run the full gate

Run the gate in
[`.agents/rules/clap-mcp-ci-gate.md`](.agents/rules/clap-mcp-ci-gate.md)
**after completing a plan or any heavy agent-driven change** (API refactors, new
public types, example migrations, broad test/doc updates, CI workflow edits).
Required before every release, with no exceptions.

For day-to-day edits, match the same bar when the change touches public API,
build graphs, or cross-crate behavior. Smaller fixes still require fmt and the
tests that cover touched code; the ci-gate rule lists the full CI order.

The authoritative workflow definition is
[`.github/workflows/ci.yml`](.github/workflows/ci.yml). Local runs may add extras
(e.g. coverage, client smoke) but must not skip what CI enforces.

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

# MCP conformance harness (requires Docker; part of ci-gate)
cargo xtask conformance                 # active@2025-11-25 + all@2026-07-28; stops stale servers first
cargo xtask conformance-stop            # stop conformance-server / orphan fixture
# conformance-server is debug-only — see docs/conformance-baseline.md#local-safety-conformance-server
```

CI runs the ci-gate commands on Ubuntu, Windows, and macOS. Conformance runs on
Ubuntu (Docker) via the dedicated `conformance` job.

### When bumping workspace `rmcp`

After changing the workspace `rmcp` version or feature flags in root
[`Cargo.toml`](Cargo.toml), vet known touchpoints (not exhaustive; add rows as
you discover more):

* [rmcp CHANGELOG](https://github.com/modelcontextprotocol/rust-sdk/blob/main/crates/rmcp/CHANGELOG.md)
  — breaking renames, transport, and task API changes
* [docs/migration-notes.md](docs/migration-notes.md) — port notes and confirmed
  rmcp feature names
* [docs/mcp-tasks.md](docs/mcp-tasks.md#task-support-matrix) — task support
  matrix and **Upcoming** rows (for example [rust-sdk PR #816](https://github.com/modelcontextprotocol/rust-sdk/pull/816))
* [`clap-mcp/Cargo.toml`](clap-mcp/Cargo.toml) `http` feature → rmcp transport
  sub-features
* HTTP and stdio integration tests under `clap-mcp/tests/` (`http_inprocess`,
  `task_augmented`, `example_contract`, conformance fixture tests)
* rmcp client examples under `examples/` (`client`, `task_augmented_client`)
* [docs/http.md](docs/http.md) and [docs/conformance-baseline.md](docs/conformance-baseline.md)
  if streamable HTTP or harness expectations shift

Run [`.agents/rules/clap-mcp-ci-gate.md`](.agents/rules/clap-mcp-ci-gate.md) after the bump.

## Testing expectations

* Do not disable, ignore, or delete tests to make CI pass — fix behavior or
  expectations.
* Integration tests under `clap-mcp/tests/` exercise real example binaries via
  stdio MCP; prefer extending existing helpers in `clap-mcp/tests/common/`.
* UI tests (`trybuild`) guard macro diagnostics in `clap-mcp/tests/ui/`.

### Macro and complex CLI testing

Follow [`.agents/rules/clap-mcp-macros.md`](.agents/rules/clap-mcp-macros.md).
See [docs/maintainer-testing.md](docs/maintainer-testing.md) for the macro
checklist table. Canonical regression tree:
[`clap-mcp/tests/complex_cli_fixture/`](clap-mcp/tests/complex_cli_fixture/mod.rs).

### Runtime library changes

Follow [`.agents/rules/clap-mcp-lib.md`](.agents/rules/clap-mcp-lib.md) and
[docs/maintainer-testing.md](docs/maintainer-testing.md).

### Examples changes

Follow [`.agents/rules/clap-mcp-examples.md`](.agents/rules/clap-mcp-examples.md)
and [docs/maintainer-testing.md](docs/maintainer-testing.md) (adding an example).

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
| [docs/usage.md](docs/usage.md) | CLI authors | Derive (minimal / with attributes), imperative CLI, skip/default filters, struct-root pointer |
| [docs/custom-content.md](docs/custom-content.md) | CLI authors | Custom MCP resources and prompts |
| [docs/export-skills.md](docs/export-skills.md) | CLI authors | `--export-skills`, Agent Skills generation |
| [docs/execution-safety.md](docs/execution-safety.md) | CLI authors | `reinvocation_safe`, skip/requires, dual derive, async embedders |
| [docs/mcp-tasks.md](docs/mcp-tasks.md) | CLI authors | Task-augmented `tools/call`, examples, support matrix |
| [docs/stateful-tools.md](docs/stateful-tools.md) | CLI authors | Shared session state, `parse_or_serve_mcp_with_state` |
| [docs/security.md](docs/security.md) | CLI authors | Schema validation, deployment trust model, subprocess and HTTP limits |
| [docs/tool-output.md](docs/tool-output.md) | CLI authors | `run` return types, structured output, per-tool `output_type`, `output-schema` |
| [docs/logging.md](docs/logging.md) | CLI authors | `tracing` / `log` bridges, MCP notifications, `SubprocessStderr` |
| [docs/http.md](docs/http.md) | CLI authors | Streamable HTTP listen (`--mcp-http`) |
| [docs/migration-notes.md](docs/migration-notes.md) | CLI authors / maintainers | RC → `0.1.0`, rmcp 3.0 / MCP 2026-07-28, historical ports |
| [docs/conformance-baseline.md](docs/conformance-baseline.md) | Maintainers | MCP conformance harness, baseline YAML |
| [docs/maintainer-testing.md](docs/maintainer-testing.md) | Maintainers | Macro checklist, test filters, example contracts |
| [docs/supported-cli-shapes.md](docs/supported-cli-shapes.md) | CLI authors | Pattern matrix, attributes per shape, non-goals |

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

* [`rumdl`](https://github.com/rvben/rumdl) on Ubuntu CI ([`rvben/rumdl@v0`](https://rumdl.dev/usage/ci-cd/));
  optional locally if installed. Config: [`.rumdl.toml`](.rumdl.toml) (covers
  `README.md`, `AGENTS.md`, `docs/*.md`, `.agents/rules/*.md`).
  Run: `rumdl check README.md AGENTS.md docs/*.md .agents/rules/*.md`

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

### Version strings in documentation

* **Single source of truth for “what version to write”** (in order):
  1. An **explicit user override** for this task (for example “use
     `0.1.0`”, “target the next RC”, or “keep copy-paste on the version on
     `main`”). The override must be stated; do not invent one. When given, it
     wins over the sources below for copy-paste examples in that change.
  2. An open GitHub PR whose title starts with `chore: release` (from
     release-plz). Read the **PR branch source** — typically root
     [`Cargo.toml`](Cargo.toml) `workspace.package.version` on that branch, or
     the `x.y.z -> a.b.c` lines in the PR body. That is the upcoming published
     version. The PR **title** may lag after release-plz refreshes the branch;
     never prefer the title over the branch/`Cargo.toml` contents.
  3. Otherwise the workspace version on the branch you are editing (usually
     `main` / root [`Cargo.toml`](Cargo.toml)).
* **Copy-paste examples** in README, `docs/*.md`, and hand-authored skills under
  `.agents/skills/`: match that version **exactly**, including any `-rc.N`
  pre-release suffix while still on an RC (for example `version = "0.1.0"` when
  the workspace is on the first non-RC `0.1.0`). After `0.1.0` (or later),
  copy-paste may use `"0.1.0"` without an RC pin unless you are documenting a
  specific pre-release.
* **Exempt from bump sweeps:** [`CHANGELOG.md`](CHANGELOG.md) historical
  entries, and historical semver literals inside
  [`docs/migration-notes.md`](docs/migration-notes.md) that document past
  release boundaries. Current-release guidance in migration-notes (including
  copy-paste `Cargo.toml` snippets) still tracks the version source above.
* **After a workspace version bump** (or when aligning docs ahead of a pending
  `chore: release` merge): run:

  ```shell
  gh pr list --search 'chore: release' --state open
  rg 'clap-mcp = |"clap-mcp"' README.md docs/*.md AGENTS.md examples/README.md .agents/skills
  ```

  Align every copy-paste dependency version with the user override when one was
  given, else the release-PR branch `Cargo.toml` when one is open, otherwise
  with root `Cargo.toml` (including `-rc.N` while on an RC).

## Git

* Do not commit unless the user asks.
* Do not bump the crate version unless explicitly requested. Prefer letting the
  open `chore: release` PR land the workspace bump; document copy-paste strings
  against that upcoming version when release-plz has already opened the PR,
  unless the user gave a different override (see
  [Version strings in documentation](#version-strings-in-documentation)).

### CHANGELOG (`release-plz`)

[`CHANGELOG.md`](CHANGELOG.md) is **not** hand-edited by agents or contributors.
[release-plz](https://release-plz.dev/) owns it via
[`release-plz.toml`](release-plz.toml) and
[`.github/workflows/release-plz.yml`](.github/workflows/release-plz.yml).

On pushes to `main`, the `release-plz` workflow opens or updates a release PR
(`chore: release …`) that bumps crate versions and rewrites `CHANGELOG.md` from
conventional commit messages since the last release. When that PR is open, treat
its branch `Cargo.toml` as the version for documentation copy-paste (see
[Version strings in documentation](#version-strings-in-documentation)).

**Agent rule:** do not add or edit `CHANGELOG.md` in task PRs. Document user-facing
changes in `docs/` (and README when needed). Write commit messages release-plz can
parse (for example `fix(server): …`, `feat(derive): …`). release-plz groups entries
by commit type when the release PR lands.
