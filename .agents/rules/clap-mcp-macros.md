---
name: clap-mcp-macros
description: Checklist when editing the clap-mcp proc-macro crate
activation: paths
paths:
  - clap-mcp/macros/**
---

# Macro changes

Before finishing edits under `clap-mcp/macros/`:

1. Read [docs/maintainer-testing.md](../../docs/maintainer-testing.md) (macro change checklist).
2. Run `cargo test -p clap-mcp --all-features complex_cli` and `cargo test -p clap-mcp --test trybuild`.
3. If behavior is embedder-visible, update [docs/supported-cli-shapes.md](../../docs/supported-cli-shapes.md) and add or extend `example_contract` tests when an example encodes the invariant.
4. Struct-root metadata delegate: OR root flags (`task_augmented_tools`, `skip_root_when_subcommands`, `output_schema`) onto delegated nested metadata on the light path, not only when the merge branch runs.
5. Never weaken `example_contract` tests to hide embedder-visible bugs; fix product behavior instead.
