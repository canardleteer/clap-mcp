# MCP conformance baseline (clap-mcp)

> Maintainer guide — MCP conformance harness and expected failures. See
> [README](../README.md#documentation).

Tracks runs of the official MCP conformance harness against the maintainer HTTP
conformance fixture (`clap-mcp-conformance-http`).

## Harness

**Local** (Rust + Docker; no host Node):

```shell
cargo xtask conformance
```

Or run `./scripts/run-conformance.sh`.

Builds `clap-mcp-conformance-http`, starts it on an ephemeral port, runs
`@modelcontextprotocol/conformance` from the pinned Docker image
([`docker/conformance/VERSION`](../docker/conformance/VERSION)).

**GitHub CI** uses
[`modelcontextprotocol/conformance`](https://github.com/modelcontextprotocol/conformance)
Action at the same version pin in
[`.github/workflows/ci.yml`](../.github/workflows/ci.yml) (PR conformance job)
and [`.github/workflows/conformance.yml`](../.github/workflows/conformance.yml)
(scheduled/dispatch); `cargo xtask conformance-server` starts the fixture.

The conformance binary is **maintainer-only** (not listed in
[`examples/README.md`](../examples/README.md)). User-facing HTTP demos remain
[`subcommands_http`](../examples/servers/subcommands_http.rs).

## Baseline file

[`conformance-baseline.yml`](../conformance-baseline.yml) lists **expected**
failing scenarios for our clap-derived fixture (not a reference “everything”
server). CI passes `--expected-failures` so:

* Failures in the baseline → exit 0 (expected)
* Failures not in the baseline → exit 1 (regression)
* Passes for baselined scenarios → exit 1 (stale baseline; remove entry)

Refresh after a verbose run:

```shell
cargo xtask conformance --verbose
```

Add new scenario names under `server:` with a one-line rationale in the sections
below.

## Schedule

[`.github/workflows/conformance.yml`](../.github/workflows/conformance.yml) runs
on **weekly cron** (skips when the repo and harness fingerprint are unchanged
since the last successful run) and on **workflow_dispatch** (always runs). Use
**Run workflow** for on-demand validation.

## Baseline categories (conformance 0.1.16)

### Permanent — reference harness tools (not our product)

The conformance harness uses **fixed scenario IDs** (e.g.
`tools-call-simple-text`) that call **reference tool names** from the MCP
“everything server” contract (e.g. `test_simple_text`, `test_tool_with_image`).
Those names describe the harness fixture, not generic clap-mcp capabilities — our
maintainer fixture only implements tools that exercise **shipped** behavior
(e.g. `test_tool_with_logging`, `test_error_handling`).

clap-mcp is a CLI bridge: tools are clap subcommands; results are text or JSON
only. We do not implement the full reference tool catalog or rich media shapes
(image/audio/mixed content, progress, sampling).

* `tools-call-simple-text` → `test_simple_text`; `tools-call-image` → image tool;
  `tools-call-audio`, `tools-call-embedded-resource`, `tools-call-mixed-content`,
  `tools-call-with-progress`, `tools-call-sampling` — reference media/progress
  tools
* `resources-read-binary`, `resources-templates-read`,
  `prompts-get-embedded-resource`, `prompts-get-with-image`

### Permanent — SCAFFOLDING (elicitation)

Elicitation is scaffolding (`confirm-echo` spike only), not a shipped public
API. Covered by in-process integration tests, not harness shrink.

* `tools-call-elicitation`, `elicitation-sep1034-defaults`,
  `elicitation-sep1330-enums`

### Permanent — not implemented

* `resources-subscribe`, `resources-unsubscribe`

### Passing with the conformance fixture (not baselined)

After the `clap-mcp-conformance-http` fixture and `logging/setLevel` handler:

* Lifecycle/utilities: `server-initialize`, `logging-set-level`, `ping`,
  `completion-complete`
* Lists: `tools-list`, `resources-list`, `prompts-list`
* Shipped capabilities: `tools-call-with-logging`, `tools-call-error`,
  `resources-read-text`, `prompts-get-simple`, `prompts-get-with-args`
* Transport: `server-sse-multiple-streams`, `dns-rebinding-protection`

## Remote CI debugging

On failed or manual workflow runs: **Actions → run → Artifacts →
`conformance-debug-*`** (server log, port file, baseline yml). Action output
remains in the job log (`verbose: true`).

## Local safety (`conformance-server`)

Prefer **`cargo xtask conformance`** (or `./scripts/run-conformance.sh`) for local
runs. It starts the fixture, runs the harness, stops stale servers first, and
tears down the server when the harness exits.

Stop/cleanup:

```shell
cargo xtask conformance-stop
```

Or `./scripts/stop-conformance-server.sh` (same command).

`cargo xtask conformance-server` exists for CI and advanced debugging only. It
redirects fixture stdout/stderr to `target/conformance-server.log` (default) and
keeps the process alive until the parent exits. Do not background it for ad-hoc
local harness runs; an orphaned server can still fill disk if stderr tracing is
verbose (log cap below limits growth but does not replace a proper stop).

Safeguards:

* `conformance-server` refuses to start when a pid file or orphan
  `clap-mcp-conformance-http` process is present unless you pass `--force`.
* Fixture stderr tracing uses a default `EnvFilter` of `warn,clap_mcp=info`
  (override with `RUST_LOG` when debugging).
* On Linux and macOS, `--log-max-mb` (default `10`) sets `RLIMIT_FSIZE` on the
  server child so the capture file cannot grow without bound.
* The server child pid is written to `target/conformance-server.pid` and removed
  when `conformance-server` or `conformance-stop` exits; the child is sent
  `SIGTERM` if the parent dies (`PR_SET_PDEATHSIG` on Linux).

Manual fallback if `conformance-stop` is unavailable:

```shell
kill "$(cat target/conformance-server.pid)" 2>/dev/null || true
rm -f target/conformance-server.log target/conformance-server.pid target/conformance-port
```

## Version pin

Single source: [`docker/conformance/VERSION`](../docker/conformance/VERSION) —
used for local Docker image and `modelcontextprotocol/conformance@v…` in
[`.github/workflows/ci.yml`](../.github/workflows/ci.yml) and
[`.github/workflows/conformance.yml`](../.github/workflows/conformance.yml).
