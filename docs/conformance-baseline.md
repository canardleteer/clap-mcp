# MCP conformance baseline (clap-mcp)

> Maintainer guide — MCP conformance harness and expected failures. See
> [README](../README.md#documentation).

Tracks runs of the official MCP conformance harness against the maintainer HTTP
conformance fixture (`clap-mcp-conformance-http`).

## Harness

**Local and GitHub CI** (Rust + Docker; no host Node):

```shell
cargo xtask conformance
```

Builds `clap-mcp-conformance-http`, starts it on an ephemeral port, and runs
`@modelcontextprotocol/conformance` from the pinned Docker image
([`docker/conformance/VERSION`](../docker/conformance/VERSION), currently
git SHA `c321dd32035556e6769d3724a8ee97d87c3faaac`, untagged
`0.2.0-alpha.11`) **twice**:

| Pass | Suite | `--spec-version` | Baseline |
| --- | --- | --- | --- |
| Legacy | `active` | `2025-11-25` | [`conformance-profiles/conformance-2025-11-25.yml`](../conformance-profiles/conformance-2025-11-25.yml) |
| Current | `all` | `2026-07-28` | [`conformance-profiles/conformance-2026-07-28.yml`](../conformance-profiles/conformance-2026-07-28.yml) |

Override with `--suite active|draft|all|…`, `--spec-version`, `--baseline`, or
`--current-baseline` when debugging a single pass.

> [!NOTE]
> The harness still puts scenarios with `introducedIn: 2026-07-28` in a suite
> named `draft`, even though that protocol date is released. The default second
> pass uses suite `all` with `--spec-version 2026-07-28` so those scenarios run
> under the dated release identifier. Upstream `draft` as an evolving
> specification directory remains separate; clap-mcp does not advertise a
> `draft` protocol string. You may run
> `cargo xtask conformance --suite draft --spec-version 2026-07-28` to exercise
> only the harness's `introducedIn: 2026-07-28` bucket.

CI jobs in [`.github/workflows/ci.yml`](../.github/workflows/ci.yml) and
[`.github/workflows/conformance.yml`](../.github/workflows/conformance.yml) call
the same `cargo xtask conformance` entrypoint.

The conformance binary is **maintainer-only** (not listed in
[`examples/README.md`](../examples/README.md)). User-facing HTTP demos remain
[`subcommands_http`](../examples/servers/subcommands_http.rs).

## Baseline files

Profiles live under [`conformance-profiles/`](../conformance-profiles/) and are
named by MCP protocol version (`conformance-YYYY-MM-DD.yml`), for example
`conformance-2026-07-28.yml` for the released `2026-07-28` pass.

The YAML files are the machine-readable allow-lists the harness consumes. Every
ID in those files **must** appear in the inventories below with a concrete
reason and disposition. Short comments in the YAML point here; do not add a
baseline entry without updating this guide.

Harness semantics:

* Failures in the baseline → exit 0 (expected)
* Failures not in the baseline → exit 1 (regression)
* Passes for baselined scenarios → exit 1 (stale baseline; remove entry)

Refresh after a verbose run:

```shell
cargo xtask conformance --verbose
```

### Disposition labels

| Label | Meaning |
| --- | --- |
| `out-of-scope` | Conflicts with clap-mcp as a CLI→MCP bridge (text/JSON tools, no agent loop) |
| `deferred` | Plausible product feature; needs design or larger rmcp surface |
| `upstream` | Blocked on rmcp / Streamable HTTP, or an unmerged conformance harness defect |
| `fixture-only` | Library already supports it; only the maintainer fixture is incomplete |

When a scenario becomes shipped clap-mcp behavior, remove it from the YAML and
from the table in the same change.

## Known unmerged harness defects

These open [modelcontextprotocol/conformance](https://github.com/modelcontextprotocol/conformance)
PRs are defects in the harness, not in clap-mcp. Do not treat related
failures, warnings, or missing scenarios as product regressions. Re-vet after
they merge; do not "fix" clap-mcp to satisfy the broken check.

| PR | Defect | clap-mcp impact on this pin |
| --- | --- | --- |
| [#381](https://github.com/modelcontextprotocol/conformance/pull/381) | Missing `json-rpc-batch-rejection` (Streamable HTTP MUST reject a JSON-RPC batch array) | Scenario is not in SHA `c321dd32035556e6769d3724a8ee97d87c3faaac`. When it lands, a fail is a transport/rmcp question, not a clap CLI gap |
| [#380](https://github.com/modelcontextprotocol/conformance/pull/380) | `tools-name-format` on `tools-list` uses stale SEP-986 rules and MUST-level failure instead of 2025-11-25 SHOULD + `WARNING` | `tools-list` is green on this pin. A later warning from the old check is harness, not a clap-mcp name bug |
| [#346](https://github.com/modelcontextprotocol/conformance/pull/346) | Client CLI / everything-client drift (`elicitation-sep1034-client-defaults`, `sse-retry`) | clap-mcp runs **server** scenarios only. Ignore client-suite fallout from this |

## Schedule

[`.github/workflows/conformance.yml`](../.github/workflows/conformance.yml) runs
on **weekly cron** (skips when the repo and harness fingerprint are unchanged
since the last successful run) and on **workflow_dispatch** (always runs). Use
**Run workflow** for on-demand validation.

## Stable baseline inventory

Source: [`conformance-profiles/conformance-2025-11-25.yml`](../conformance-profiles/conformance-2025-11-25.yml)
(`active` @ `2025-11-25`).

| Scenario | What the harness requires | Why clap-mcp does not | Disposition |
| --- | --- | --- | --- |
| `tools-call-image` | Tool `test_image_content` returns `ContentBlock` image | Derive/`IntoClapMcpResult` only emit text or structured JSON, not image blocks | `out-of-scope` |
| `tools-call-audio` | Tool `test_audio_content` returns audio content | Same: no audio tool-result path | `out-of-scope` |
| `tools-call-embedded-resource` | Tool returns embedded resource content block | Tools do not return embedded `resource` content blocks | `out-of-scope` |
| `tools-call-mixed-content` | Tool returns mixed text+image(+…) content | “Mixed” in clap-mcp means text + structured JSON, not multi-media blocks | `out-of-scope` |
| `tools-call-with-progress` | Tool sends `notifications/progress` with a progress token | No progress-notification API; logging uses `notifications/message` only | `deferred` (progress bridge if a clap-shaped pattern emerges) |
| `tools-call-sampling` | Tool calls client `sampling/createMessage` | Server→client sampling not offered; see [mcp-tasks](mcp-tasks.md) | `out-of-scope` (agent loop, not CLI bridge) |
| `tools-call-elicitation` | Tool calls client `elicitation/create` | Elicitation scaffolding removed; see [migration-notes](migration-notes.md#removed-scaffolding-elicitation) | `deferred` (revisit when a clap-shaped confirm/prompt pattern exists) |
| `elicitation-sep1034-defaults` | Elicitation schema with primitive defaults (SEP-1034) | Same elicitation gap | `deferred` |
| `elicitation-sep1330-enums` | Elicitation enum schema variants (SEP-1330) | Same elicitation gap | `deferred` |

### Passing on the stable pass (not baselined)

* Lifecycle/utilities: `server-initialize`, `server-session-lifecycle`,
  `logging-set-level`, `ping`, `completion-complete`
* Lists: `tools-list`, `resources-list`, `prompts-list`
* Shipped capabilities: `tools-call-simple-text`, `tools-call-with-logging`,
  `tools-call-error`, `resources-read-text`, `resources-read-binary`
  (`ResourceContent::StaticBlob`), `resources-templates-read` (simple
  `{param}` templates), `resources-subscribe`, `resources-unsubscribe`
  (capability + RPC success; no update notifications), `prompts-get-simple`,
  `prompts-get-with-args`, `prompts-get-with-image`,
  `prompts-get-embedded-resource` (image/embedded via custom `PromptMessage`
  pass-through, not tool-result media)
* Transport: `server-sse-multiple-streams`, `dns-rebinding-protection`

## 2026-07-28 baseline inventory

Source: [`conformance-profiles/conformance-2026-07-28.yml`](../conformance-profiles/conformance-2026-07-28.yml)
(`all` @ `2026-07-28`).

These scenarios exercise the released `2026-07-28` protocol version. clap-mcp
remains a dual-era server that negotiates `2025-11-25` or `2026-07-28` in
`initialize`, but does not implement the full `2026-07-28` server surface.

Shared with the legacy pass (still applicable under `2026-07-28`):

| Scenario | Disposition |
| --- | --- |
| `tools-call-image` / `tools-call-audio` / `tools-call-embedded-resource` / `tools-call-mixed-content` | `out-of-scope` (text/JSON tools only) |
| `tools-call-with-progress` | `deferred` |

`2026-07-28`-introduced scenarios:

| Scenario | What the harness requires | Why clap-mcp does not | Disposition |
| --- | --- | --- | --- |
| `server-stateless` | SEP-2575: `server/discover`, per-request `_meta`, structural capability checks | Sessionful Streamable HTTP via rmcp; no `server/discover` or modern per-request metadata path | `deferred` (dual-era / stateless HTTP is a large transport project) |
| `http-custom-header-server-validation` | SEP-2243: `Mcp-Param` / `x-mcp-header` Base64 rules | No custom MCP header param mapping from clap args | `deferred` (needs clap↔header mapping design) |
| `input-required-result-basic-elicitation` | SEP-2322 `InputRequiredResult` + elicitation input | Multi-turn input + elicitation; neither offered | `deferred` (depends on elicitation + InputRequiredResult types) |
| `input-required-result-basic-sampling` | SEP-2322 + sampling input request | Sampling not offered | `out-of-scope` / `deferred` with sampling |
| `input-required-result-basic-list-roots` | SEP-2322 + `roots/list` input request | No InputRequiredResult surface | `deferred` |
| `input-required-result-request-state` | SEP-2322 `requestState` round-trip | Same InputRequiredResult gap | `deferred` |
| `input-required-result-multiple-input-requests` | Multiple `inputRequests` in one result | Same | `deferred` |
| `input-required-result-multi-round` | Multi-round InputRequiredResult | Same | `deferred` |
| `input-required-result-missing-input-response` | Re-request on missing/wrong `inputResponses` | Same InputRequiredResult gap. Harness `0.2.0-alpha.11` scores this as a warning, not a hard fail; it still must stay in the expected-failures list | `deferred` |
| `input-required-result-non-tool-request` | InputRequiredResult on `prompts/get` | Same | `deferred` |
| `input-required-result-result-type` | Explicit `resultType` on InputRequiredResult | Same | `deferred` |
| `input-required-result-tampered-state` | Reject tampered integrity-protected `requestState` | Same | `deferred` |
| `input-required-result-capability-check` | Only request inputs for client-declared caps | Same | `deferred` |
| `input-required-result-ignore-extra-params` | Ignore unknown keys in `inputResponses` | Same InputRequiredResult gap. Harness `0.2.0-alpha.11` scores this as a warning; keep it in the expected-failures list | `deferred` |

### Passing on the 2026-07-28 pass (not baselined)

Newly green under rmcp 3.0 (remove from baseline if they regress):

* `sep-2164-resource-not-found`
* `http-header-validation`
* `input-required-result-unsupported-methods`
* `input-required-result-validate-input`
* `json-schema-2020-12` (clap tools advertise `$schema`; fixture registers
  `json_schema_2020_12_tool` via `custom_tools` for SEP-1613 / SEP-2106 keyword
  preservation)
* `caching` (SEP-2549 `ttlMs` / `cacheScope` via [`CacheHints`] on
  [`ClapMcpServeOptions`]; defaults `ttl_ms: 0`, `cacheScope: public`)

Also passing when applicable: `tools-call-simple-text`, `tools-call-error`,
list/read resource and prompt scenarios shared with the legacy pass,
`server-sse-multiple-streams`, `dns-rebinding-protection`.

### Partial progress (still baselined)

| Scenario | Library status | Remaining gap |
| --- | --- | --- |
| `server-stateless` | Dual-era initialize path only | Full SEP-2575 discover / per-request `_meta` |

### Feature backlog suggested by these baselines

Ordered by how close they are to clap-mcp’s product shape:

1. **Progress notifications** (`tools-call-with-progress`) — optional bridge from
   long-running tools when embedders can supply a progress token.
2. **Elicitation / InputRequiredResult** — revisit only with a clear clap UX
   (confirm flags, interactive prompts); currently deferred after scaffolding
   removal.
3. **Stateless / SEP-2575 HTTP** and **SEP-2243 custom header rules** — largely
   transport/`rmcp` work; track upstream before inventing a clap-mcp layer.

Shipped from this backlog: binary custom resources (`ResourceContent::StaticBlob`);
resource subscribe/unsubscribe RPCs (no update notifications); simple `{param}`
URI templates (`CustomResourceTemplate`); JSON Schema 2020-12 `$schema` on clap
tool `inputSchema` plus `custom_tools` / `json_schema_2020_12_tool` for rich
vocabulary preservation; SEP-2549 list/read cache hints (`CacheHints`).

## Remote CI debugging

On failed or manual workflow runs: **Actions → run → Artifacts →
`conformance-debug-*`** (server log, port file, baseline yml). Harness output
remains in the job log (`--verbose`).

## Local safety (`conformance-server`)

Prefer **`cargo xtask conformance`** for local runs. It starts the fixture, runs
the harness, stops stale servers first, and tears down the server when the
harness exits.

Stop/cleanup:

```shell
cargo xtask conformance-stop
```

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

[`docker/conformance/VERSION`](../docker/conformance/VERSION) is the harness pin
for the local Docker image. A 40-character hex SHA clones
`modelcontextprotocol/conformance` at that commit, builds it, and installs the
result (current pin is untagged `0.2.0-alpha.11` at
`c321dd32035556e6769d3724a8ee97d87c3faaac`). Any other value is an npm version
or dist-tag (`@modelcontextprotocol/conformance@…`). xtask tags the image
`clap-mcp-conformance:<pin>` so changing the pin rebuilds. GitHub Actions and
local xtask both build and run that image.

## Protocol versions clap-mcp advertises

[`clap_mcp::protocol`](https://docs.rs/clap-mcp/latest/clap_mcp/protocol/) lists
the MCP protocol versions the library accepts in `initialize` negotiation:

* `PROTOCOL_VERSION_STABLE` = `2025-11-25` (primary advertise / fallback)
* `PROTOCOL_VERSION_CURRENT` = `2026-07-28` (released protocol version)

clap-mcp does not advertise a `draft` protocol string. Those constants match the
dual conformance passes above. Older rmcp-known dates are not echoed. Stdio
uses `serve_directly` plus clap-mcp's `initialize` negotiation. Streamable HTTP
uses rmcp 3.1 `ServerHandler::supported_protocol_versions`, which returns the
same [`SUPPORTED_PROTOCOL_VERSIONS`](https://docs.rs/clap-mcp/latest/clap_mcp/protocol/constant.SUPPORTED_PROTOCOL_VERSIONS.html)
set for discover, initialize, and per-request version checks.
