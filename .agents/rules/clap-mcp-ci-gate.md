---
name: clap-mcp-ci-gate
description: Mandatory local CI gate before finishing Rust, workflow, or integration-test changes
trigger: auto
paths:
  - .github/workflows/**
  - clap-mcp/**
  - examples/**
  - xtask/**
---

# Pre-merge CI gate

Run this gate **before** marking the task complete, committing, or pushing when
you changed any matched path above. Do not stop after `cargo test` alone — CI
runs additional steps that fail independently (fmt, clippy, examples smoke,
rustdoc).

Mirror [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml) in order:

```shell
cargo fmt --all -- --check
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo xtask examples-help
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p clap-mcp --all-features
```

On Ubuntu CI only (optional locally): `cargo audit`.

## Integration test and workflow edits

When editing `clap-mcp/tests/**` or `.github/workflows/**`:

1. Read [docs/maintainer-testing.md](../../docs/maintainer-testing.md) for
   targeted test filters; still run the full gate above before finish.
2. Integration tests are Clippy `--all-targets` — `-D warnings` includes test
   crates (`unused_mut`, and similar).
3. After refactors that change ownership, async teardown, or timeouts, run
   **`cargo fmt --all`** and
   **`cargo clippy --all-targets --all-features -- -D warnings`**
   even when targeted tests pass.
4. When changing workflow steps, confirm the shell commands above still match
   CI (including flags and env vars).

## Path-specific rules stack

Also read and follow any other [`.agents/rules/`](.) file whose `paths` globs
match your edit (`clap-mcp-macros`, `clap-mcp-lib`, `clap-mcp-examples`). Run
their extra commands in addition to this gate.
