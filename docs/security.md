# Security

> Guide for CLI authors adding clap-mcp. See [README](../README.md) to get started.

[← Documentation index](../README.md#documentation)

The MCP server does **not** trust the client for tool or argument discovery.
Every tool call is validated against the schema before any execution (in-process
or subprocess). The server rejects unknown tools and unknown argument names
immediately with an error; execution proceeds only for schema-defined tools and
arguments.

When `reinvocation_safe` is `false` (the default), each tool call spawns a fresh
subprocess of your binary. Consider the following:

**Shell injection is not a concern.** Arguments are passed via
`std::process::Command::arg()` directly to the executable as `argv` — no shell
is invoked, so metacharacters (`;`, `|`, `$()`, etc.) are not interpreted.

**Unknown tools and arguments are rejected.** The server validates every tool
name and argument name against the schema before execution. Invalid requests fail
with `CallToolError::unknown_tool` or `CallToolError::invalid_arguments`; no
subprocess is spawned and no in-process handler runs for invalid calls.

**Argument values come from the MCP client.** The schema constrains which
argument names are accepted, but values pass through unvalidated. If your CLI
uses those values unsafely (e.g., in file paths, system calls, or other
sensitive operations), a malicious or compromised MCP client could exploit that.
Validate and sanitize all inputs in your CLI.

**Environment and working directory are inherited.** The subprocess inherits the
full environment and CWD of the MCP server. Sensitive env vars (API keys,
tokens) are visible to every subprocess; relative paths resolve against the
server's CWD.

**Resource usage.** Each tool call spawns a new process. With
`parallel_safe = true`, many concurrent calls can create many processes.
clap-mcp applies no timeouts or resource limits on subprocess execution.
