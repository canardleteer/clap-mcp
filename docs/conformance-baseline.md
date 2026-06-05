# MCP conformance baseline (clap-mcp)

Tracks runs of the official MCP conformance harness against the Streamable HTTP
example server (`subcommands_http`).

## Harness

**Local** (Rust + Docker; no host Node):

```bash
cargo xtask conformance
# or: ./scripts/run-conformance.sh
```

Builds `subcommands_http`, starts it on an ephemeral port, runs
`@modelcontextprotocol/conformance` from the pinned Docker image
([`docker/conformance/VERSION`](../docker/conformance/VERSION)).

**GitHub CI** uses
[`modelcontextprotocol/conformance`](https://github.com/modelcontextprotocol/conformance)
Action at the same version pin; `cargo xtask conformance-server` starts the
fixture.

## Baseline file

[`conformance-baseline.yml`](../conformance-baseline.yml) lists **expected**
failing scenarios for our clap-derived fixture (not a reference “everything”
server). CI passes `--expected-failures` so:

* Failures in the baseline → exit 0 (expected)
* Failures not in the baseline → exit 1 (regression)
* Passes for baselined scenarios → exit 1 (stale baseline; remove entry)

Refresh after a verbose run:

```bash
cargo xtask conformance --verbose
```

Add new scenario names under `server:` with a one-line rationale below.

## Current expected gaps (2026-06-05, conformance 0.1.11)

Fixture exposes clap subcommands (`greet`, `add`, `sub`) and `clap://schema` —
not reference-server tools (`test_simple_text`, rich prompts/resources,
elicitation SEP cases, progress, sampling, etc.). Scenarios covered by the yml:

* Reference-tool calls (text/image/audio/mixed/progress/error/logging/sampling/elicitation)
* Elicitation SEP cases (server does not advertise elicitation on this fixture)
* Rich resource/prompt shapes beyond clap defaults
* `logging-set-level` (logging not enabled on conformance fixture)
* Resource subscribe/unsubscribe (not implemented)

**Passing without baseline:** initialize, ping, completion, tools-list,
resources-list, prompts-list, SSE streams, DNS rebinding (loopback).

## Remote CI debugging

On failed or manual workflow runs: **Actions → run → Artifacts →
`conformance-debug-*`** (server log, port file, baseline yml). Action output
remains in the job log (`verbose: true`).

## Version pin

Single source: [`docker/conformance/VERSION`](../docker/conformance/VERSION) —
used for local Docker image and `modelcontextprotocol/conformance@v…` in
[`.github/workflows/conformance.yml`](../.github/workflows/conformance.yml).
