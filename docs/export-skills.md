# Exporting agent skills

> Guide for CLI authors adding clap-mcp. See [README](../README.md) to get started.

[← Documentation index](../README.md#documentation)

You can generate [Agent Skills](https://agentskills.io/specification) (SKILL.md)
from the same tools, resources, and prompts that the MCP server exposes. This
is useful for documenting your CLI for AI agents.

## The `--export-skills` flag

Add the flag with
[`command_with_export_skills_flag`](https://docs.rs/clap-mcp/latest/clap_mcp/fn.command_with_export_skills_flag.html)
or use
[`command_with_mcp_and_export_skills_flags`](https://docs.rs/clap-mcp/latest/clap_mcp/fn.command_with_mcp_and_export_skills_flags.html)
to add both `--mcp` and `--export-skills`:

* `--export-skills` — Generate skills into the default directory (see below) and
  exit.
* `--export-skills=DIR` — Generate skills into `DIR` (e.g.
  `--export-skills=./out`) and exit.

When both `--mcp` and `--export-skills` are present, `--export-skills` wins.
The process exports and exits without starting the MCP server.

## Default output directory

Default directory is **`.agents/skills/`**, where each skill gets a subdirectory
named after the app or tool. Override with `--export-skills=DIR`.

## What gets generated

* One skill per **tool** (from your clap schema), with name/description and
  usage hints.
* A combined **resources-and-prompts** skill when you have custom resources or
  prompts.

Generated files follow the
[Agent Skills specification](https://agentskills.io/specification) (YAML
frontmatter with `name`, `description`, and `allowed-tools`; markdown body with
usage instructions). The `name` field matches the parent directory name as
required by the spec. Each tool skill includes `allowed-tools` listing the MCP
tool it describes; note that this field is still experimental in the spec
with no defined syntax convention. You can also call
[`content::export_skills`](https://docs.rs/clap-mcp/latest/clap_mcp/content/fn.export_skills.html)
programmatically with schema, tools, custom resources, and custom prompts.
