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
([`docker/conformance/VERSION`](../docker/conformance/VERSION)) **twice**:

| Pass | Suite | `--spec-version` | Baseline |
| --- | --- | --- | --- |
| Stable | `active` | `2025-11-25` | [`conformance-baseline.yml`](../conformance-baseline.yml) |
| Draft | `draft` | `draft` (`2026-07-28`) | [`conformance-baseline-draft.yml`](../conformance-baseline-draft.yml) |

Override with `--suite active|draft|all|…`, `--spec-version`, `--baseline`, or
`--draft-baseline` when debugging a single pass.

CI jobs in [`.github/workflows/ci.yml`](../.github/workflows/ci.yml) and
[`.github/workflows/conformance.yml`](../.github/workflows/conformance.yml) call
the same `cargo xtask conformance` entrypoint.

The conformance binary is **maintainer-only** (not listed in
[`examples/README.md`](../examples/README.md)). User-facing HTTP demos remain
[`subcommands_http`](../examples/servers/subcommands_http.rs).

## Baseline files

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
| `upstream` | Blocked on rmcp / Streamable HTTP transport behavior |
| `fixture-only` | Library already supports it; only the maintainer fixture is incomplete |

When a scenario becomes shipped clap-mcp behavior, remove it from the YAML and
from the table in the same change.

## Schedule

[`.github/workflows/conformance.yml`](../.github/workflows/conformance.yml) runs
on **weekly cron** (skips when the repo and harness fingerprint are unchanged
since the last successful run) and on **workflow_dispatch** (always runs). Use
**Run workflow** for on-demand validation.

## Stable baseline inventory

Source: [`conformance-baseline.yml`](../conformance-baseline.yml)
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

* Lifecycle/utilities: `server-initialize`, `logging-set-level`, `ping`,
  `completion-complete`
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

## Draft baseline inventory

Source: [`conformance-baseline-draft.yml`](../conformance-baseline-draft.yml)
(`draft` @ `2026-07-28`).

These are draft-era protocol features. clap-mcp remains a dual-era server that
speaks legacy `initialize` for `2025-11-25` and accepts draft version
negotiation, but does not implement the full draft server surface.

| Scenario | What the harness requires | Why clap-mcp does not | Disposition |
| --- | --- | --- | --- |
| `server-stateless` | SEP-2575: `server/discover`, per-request `_meta`, structural capability checks | Sessionful Streamable HTTP via rmcp; no `server/discover` or modern per-request metadata path | `deferred` (dual-era / stateless HTTP is a large transport project) |
| `sep-2164-resource-not-found` | Unknown `resources/read` URI returns JSON-RPC error `-32602` with `data.uri` | Handler already returns `resource_not_found` with `data.uri`. Under draft HTTP the transport answers `422` with a non-JSON body, so the harness never sees the JSON-RPC error | `upstream` (keep until rmcp draft error encoding matches SEP-2164) |
| `http-header-validation` | SEP-2243: validate `Mcp-Method` / `Mcp-Name` vs body | Validation lives in rmcp Streamable HTTP; clap-mcp does not add a second layer | `upstream` |
| `http-custom-header-server-validation` | SEP-2243: `Mcp-Param` / `x-mcp-header` Base64 rules | No custom MCP header param mapping from clap args | `deferred` (needs clap↔header mapping design) |
| `caching` | SEP-2549: `ttlMs` + `cacheScope` on list/read results | List/read results do not attach cache hints | `deferred` (optional cache metadata on list responses) |
| `input-required-result-basic-elicitation` | SEP-2322 `InputRequiredResult` + elicitation input | Draft multi-turn input + elicitation; neither offered | `deferred` (depends on elicitation + draft result types) |
| `input-required-result-basic-sampling` | SEP-2322 + sampling input request | Sampling not offered | `out-of-scope` / `deferred` with sampling |
| `input-required-result-basic-list-roots` | SEP-2322 + `roots/list` input request | No InputRequiredResult surface | `deferred` |
| `input-required-result-request-state` | SEP-2322 `requestState` round-trip | Same InputRequiredResult gap | `deferred` |
| `input-required-result-multiple-input-requests` | Multiple `inputRequests` in one result | Same | `deferred` |
| `input-required-result-multi-round` | Multi-round InputRequiredResult | Same | `deferred` |
| `input-required-result-missing-input-response` | Re-request on missing/wrong `inputResponses` | Same | `deferred` |
| `input-required-result-non-tool-request` | InputRequiredResult on `prompts/get` | Same | `deferred` |
| `input-required-result-result-type` | Explicit `resultType` on InputRequiredResult | Same | `deferred` |
| `input-required-result-unsupported-methods` | Must not emit InputRequiredResult on wrong methods | Same surface; scenario expects draft-aware refusal semantics | `deferred` |
| `input-required-result-tampered-state` | Reject tampered integrity-protected `requestState` | Same | `deferred` |
| `input-required-result-capability-check` | Only request inputs for client-declared caps | Same | `deferred` |
| `input-required-result-ignore-extra-params` | Ignore unknown keys in `inputResponses` | Same | `deferred` |
| `input-required-result-validate-input` | Validate malformed `inputResponses` | Same | `deferred` |

### Partial progress (still baselined)

| Scenario | Library status | Remaining gap |
| --- | --- | --- |
| `sep-2164-resource-not-found` | `read_resource` returns `resource_not_found` with `data.uri` | Draft Streamable HTTP surfaces the error as HTTP `422` instead of a JSON-RPC error body (`upstream`) |

### Feature backlog suggested by these baselines

Ordered by how close they are to clap-mcp’s product shape:

1. **List-result cache hints** (`caching`) — optional `ttlMs` / `cacheScope` on
   tools/resources/prompts list results.
2. **Progress notifications** (`tools-call-with-progress`) — optional bridge from
   long-running tools when embedders can supply a progress token.
3. **Elicitation / InputRequiredResult** — revisit only with a clear clap UX
   (confirm flags, interactive prompts); currently deferred after scaffolding
   removal.
4. **Stateless / SEP-2575 HTTP**, **SEP-2243 header rules**, and **draft JSON-RPC
   error encoding** (`sep-2164` under draft) — largely transport/`rmcp` work;
   track upstream before inventing a clap-mcp layer.

Shipped from this backlog: binary custom resources (`ResourceContent::StaticBlob`);
resource subscribe/unsubscribe RPCs (no update notifications); simple `{param}`
URI templates (`CustomResourceTemplate`).

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

[`docker/conformance/VERSION`](../docker/conformance/VERSION) is the npm package
version for the local Docker image (`@modelcontextprotocol/conformance@…`).
GitHub Actions and local xtask both build and run that image.

## Protocol versions clap-mcp advertises

[`clap_mcp::protocol`](https://docs.rs/clap-mcp/latest/clap_mcp/protocol/) lists
the MCP protocol versions the library accepts in `initialize` negotiation:

* `2025-11-25` (stable advertise / fallback)
* `2026-07-28` (draft; alias `draft` in the harness)

Those match the dual conformance passes above. Older rmcp-known dates are not
echoed on stdio (clap-mcp uses `serve_directly` plus its own negotiation).
Streamable HTTP still goes through rmcp `serve_server`, which may echo other
known versions until upstream allows a custom supported set; clients that
request `2025-11-25` or `2026-07-28` are unaffected.
