---
name: clap-mcp-lib
description: Checklist when editing clap-mcp runtime schema and tool execution
activation: paths
paths:
  - clap-mcp/src/**
---

# Runtime library changes

Before finishing edits under `clap-mcp/src/` (excluding `macros/`):

1. Read [docs/maintainer-testing.md](../../docs/maintainer-testing.md).
2. When changing tool `inputSchema`, `build_tool_argv`, or `validate_tool_argument_names`, run:
   - `cargo test -p clap-mcp --all-features complex_cli`
   - `cargo test -p clap-mcp --all-features example_contract`
3. Root `#[arg(global)]` on struct-root CLIs must appear on nested leaf tool schemas and pass strict `example_contract` assertions (for example `struct_subcommand_globals` with `verbose: true`).
4. Stateful public API changes (`ClapMcpToolExecutorWithState`, `parse_or_serve_mcp_with_state`, `ServeMcpBuilder::for_cli_with_state`): rustdoc must warn against multi-user / untrusted remote callers; keep aligned with [docs/security.md](../../docs/security.md).
5. Preserve-cli entrypoints (`parse_or_serve_mcp_preserve_cli*`, `get_matches_preserve_cli_or_serve_mcp*`, `argv_contains_clap_mcp_flags`): document in [docs/usage.md](../../docs/usage.md); native `Parser::parse` when argv has no clap-mcp flags.
6. Never weaken `example_contract` tests to hide embedder-visible bugs; fix product behavior instead.
