# clap-mcp dependency

Read when editing `Cargo.toml` for MCP integration.

## Default (crates.io)

Use a compatible semver range and let Cargo resolve the latest matching release:

```toml
[features]
mcp = ["dep:clap-mcp", "dep:schemars"]

[dependencies]
clap-mcp = { version = "0.0.4", optional = true, features = ["output-schema", "http"] }
schemars = { version = "0.8", optional = true, features = ["derive"] }
```

Or: `cargo add clap-mcp --optional --features output-schema,http`

Add `schemars` to the `mcp` feature only when using `output-schema`.

## Path dependency (monorepo / dogfood)

When integrating against a local checkout, docs and API always match the tree:

```toml
clap-mcp = { path = "../clap-mcp", optional = true, features = ["output-schema", "http"] }
```

## Git dependency (unreleased fixes)

Use only when crates.io lacks a fix you need. Pick branch or tag at integration time in **your** project; do not hardcode a `rev` in shared skill text. Record the chosen ref in your repo's commit message or lockfile.

## clap-mcp feature flags

| Feature | Maturity | Enable when |
|---------|----------|-------------|
| `output-schema` | Shipped | `#[clap_mcp_output_type]` / `JsonSchema` on tool output |
| `http` | Shipped | Ship `--mcp-http` |
| `tracing` | Shipped | Phase 6 tracing bridge |
| `log` | Shipped | Phase 6 log bridge (replaces global logger) |

Default `derive` works without extra features beyond what you import. See [README feature flags](../../../../README.md#feature-flags).

## Lockfile

After dependency changes, run `cargo build --features mcp` (or `cargo update -p clap-mcp`) so `Cargo.lock` records the resolved version.

## Bump procedure

1. Update the dependency (or run `cargo update -p clap-mcp`).
2. Read [`docs/migration-notes.md`](../../../../docs/migration-notes.md) from the **same source tree** as the resolved dependency (path checkout, `cargo vendor`, or registry extract under `~/.cargo/registry/src/`).
3. Re-run `cargo test --features mcp` and the schema smoke test.
4. Re-run runtime smoke (`tools/list`, one `tools/call`) if execution config or hazard surface changed.

## Reading docs when this skill is copied elsewhere

When this skill lives outside the clap-mcp repo, open the same relative paths (`docs/usage.md`, etc.) from your resolved `clap-mcp` dependency source, not from pinned GitHub URLs.
