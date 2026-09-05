---
name: clap-mcp-readme
description: Root README link policy for crates.io and GitHub
trigger: auto
paths:
  - README.md
---

# Root README

The published crate uses `readme = "../README.md"` in
[`clap-mcp/Cargo.toml`](../../clap-mcp/Cargo.toml). crates.io rewrites relative
links as if the README lives under `clap-mcp/`, so repo-root paths break on
[crates.io](https://crates.io/crates/clap-mcp) (see
[rust-lang/crates.io#9927](https://github.com/rust-lang/crates.io/issues/9927)).

Before finishing edits to [`README.md`](../../README.md):

1. Link repo-root paths with absolute GitHub URLs:
   `https://github.com/canardleteer/clap-mcp/blob/HEAD/<path>` (preserve
   `#fragment` anchors). Applies to `docs/`, `examples/`, `AGENTS.md`, and
   `.agents/`.
2. Keep relative links in `docs/`, `AGENTS.md`, and other guides — only the
   root README needs absolute URLs for those targets.
3. When adding a new guide, add a Documentation table row and use the same
   absolute URL pattern in README.
4. Verify no forbidden relative links remain:

```shell
! rg '\]\((docs/|examples/|AGENTS\.md|\.agents/)' README.md
```

5. Do **not** rewrite [Protected human prose](../../AGENTS.md#protected-human-prose)
   in the Design section (including the Clanker warning). See
   [`clap-mcp-protected-prose.md`](clap-mcp-protected-prose.md).

`rumdl check README.md` is optional when installed.
