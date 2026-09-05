---
name: clap-mcp-protected-prose
description: Human-authored README Design and logging NOTE must not be rewritten by agents
trigger: auto
paths:
  - README.md
  - docs/logging.md
---

# Protected human prose

These passages are **maintainer voice**. Agents must not rewrite, paraphrase,
shorten, “tone-check,” or relocate them. You may edit other parts of the same
files.

## Protected regions

1. **[README.md — Design](../../README.md#design)** — the full section from the
   `## Design` heading through the end of the Clanker `> [!WARNING]` callout
   (including the preceding first-person rationale and intent bullets).
2. **[docs/logging.md](../../docs/logging.md)** — the author `> [!NOTE]` block
   immediately after the SEP-2577 `> [!WARNING]` under Logging and
   observability (the note that begins “This was probably one of the most
   useful features…”).

Authoritative list and rationale:
[AGENTS.md — Protected human prose](../../AGENTS.md#protected-human-prose).

If a task seems to require changing protected text, stop and ask the
maintainer. Do not “improve” these blocks as a drive-by.
