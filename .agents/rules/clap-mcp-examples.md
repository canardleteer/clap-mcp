---
name: clap-mcp-examples
description: Checklist when adding or changing clap-mcp example binaries
activation: paths
paths:
  - examples/**
---

# Example changes

Before finishing edits under `examples/`:

1. Read [docs/maintainer-testing.md](../../docs/maintainer-testing.md) (adding an example).
2. Add `[[bin]]` in [examples/Cargo.toml](../../examples/Cargo.toml) and document in [examples/README.md](../../examples/README.md).
3. Run `cargo xtask examples-help`. Do not edit an opt-in bin list; use `RELEASE_VALIDATION_EXCLUDE` only when `--help` smoke is inappropriate.
4. Add an `example_contract` test when the example encodes MCP shape invariants (`preserve_cli_parse`, `flat_struct_root`, `flatten_skip`, `flatten_subcommand_skip_flat`, `flatten_subcommand_skip_nested`, and similar shape demos).
