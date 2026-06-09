# Security

> Guide for CLI authors adding clap-mcp. See [README](../README.md) to get started.

[← Documentation index](../README.md#documentation)

clap-mcp helps you expose a CLI as an MCP server. It validates tool calls against
your schema and documents subprocess and in-process hazards. It does **not** turn
your binary into a multi-tenant network service. Treat integration as
**localhost or single-user**: one operator, one MCP client (or a small set of
clients you trust), on one machine.

## Intended deployment model

| Transport | Typical use | Trust assumption |
| --- | --- | --- |
| **stdio** (`--mcp`) | IDE or agent spawns your binary as a child process | The parent process and OS user own the server; no network listeners |
| **HTTP** (`--mcp-http`, `http` feature) | Loopback MCP on `127.0.0.1` | Same machine, same user; see [Streamable HTTP](http.md) |

stdio is the default integration path. HTTP examples and env helpers emphasize
loopback addresses (`127.0.0.1`). That matches how most editors and local agents
attach to tool servers today.

> [!WARNING]
> Binding MCP HTTP to a public interface (`0.0.0.0`, a LAN IP, or a cloud
> instance) exposes every tool your CLI defines to anyone who can reach the port.
> clap-mcp does not authenticate inbound MCP clients, does not provide mTLS, and
> does not isolate callers by identity. Put a reverse proxy, VPN, or separate
> auth layer in front if you must expose MCP beyond localhost, or keep the server
> local.

An earlier `http-oauth` Cargo feature offered OAuth **client** helpers for
calling remote MCP servers. It was scaffolding only, not a release parity
target, and was removed while no simple clap-mcp-shaped integrator pattern has
emerged. The feature is not on the roadmap for now; that is a prioritization
choice, not a permanent decision. It never protected **your** MCP server from
incoming clients. For remote MCP client OAuth, use
[rmcp's OAuth support](https://github.com/modelcontextprotocol/rust-sdk/blob/main/docs/OAUTH_SUPPORT.md)
directly. See
[migration-notes.md](migration-notes.md#removed-scaffolding-http-oauth) for
removed types and env vars.

An earlier `elicitation` Cargo feature offered a `confirm-echo` conformance
spike for server-side user prompts during tool execution. It was scaffolding
only and was removed for the same reason: no simple clap-mcp-shaped integrator
pattern has emerged yet. Agent policy today is covered by `#[clap_mcp(requires)]`
and `#[clap_mcp(skip)]`. See
[migration-notes.md](migration-notes.md#removed-scaffolding-elicitation) for
details.

## Schema validation (tool calls)

The MCP server does **not** trust the client for tool or argument discovery.
Every tool call is validated against the schema before any execution (in-process
or subprocess). The server rejects unknown tools and unknown argument names
immediately with an error; execution proceeds only for schema-defined tools and
arguments.

## In-process execution and shared state

When `reinvocation_safe` is `true`, tool calls run inside the MCP server process.
Implications for a **multi-user** or **shared** deployment:

* **One process, one environment** — cwd, environment variables, open file
  descriptors, and global statics are shared across all concurrent tool calls
  unless you add your own isolation.
* **Stateful tools** — [`parse_or_serve_mcp_with_state`](stateful-tools.md) and
  `#[clap_mcp(stateful)]` keep session state for the **server lifetime**, not
  per MCP client or per OS user. Two clients talking to the same server share
  that state.
* **`parallel_safe`** — controls lock granularity between tools; it does not
  partition state by caller. See [Working directory](execution-safety.md#working-directory-chdir)
  for cwd hazards when `parallel_safe = true`.

> [!WARNING]
> Do not run a long-lived in-process MCP server with shared session state on a
> host where untrusted users or untrusted MCP clients can connect. Prefer
> subprocess mode, no shared state, or one server instance per trusted user.

## Subprocess mode (`reinvocation_safe = false`)

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

## HTTP transport limits

When you enable the `http` feature, clap-mcp serves Streamable HTTP on `/mcp`.
Relevant security properties today:

| Topic | Current behavior |
| --- | --- |
| **Transport encryption** | Plain HTTP from clap-mcp; no TLS or mTLS termination in the library |
| **Inbound authentication** | None; any client that can open a TCP connection to the bind address can call tools (subject to schema validation only) |
| **Per-client sessions** | HTTP session handling is for the MCP protocol stack, not per-user authorization |
| **DNS rebinding** | Loopback-oriented `allowed_hosts` are applied; public binds need extra hardening — see [http.md](http.md) |

TLS termination, client certificates, API tokens, and network ACLs are embedder
or infrastructure responsibilities. clap-mcp does not configure them.

## Hardening checklist (beyond localhost)

If you must expose MCP outside a single-user loopback setup:

1. Terminate TLS and enforce authentication at a reverse proxy or API gateway.
2. Avoid `reinvocation_safe` and [stateful tools](stateful-tools.md) unless each
   trusted caller gets a dedicated server process.
3. Bind to loopback and tunnel (SSH, Tailscale, etc.) instead of `0.0.0.0` when
   possible.
4. Skip or gate tools that mutate global process state, call `exec` / `exit`, or
   assume an interactive TTY — see [Execution safety](execution-safety.md).
5. Treat MCP clients as **fully authorized** for every non-skipped tool; schema
   validation limits argument names, not privilege level.

## Related guides

| Topic | Guide |
| --- | --- |
| Loopback HTTP, env vars, `allowed_hosts` | [http.md](http.md) |
| Stateful session tools | [stateful-tools.md](stateful-tools.md) |
| `parallel_safe`, cwd, skip, exit hazards | [execution-safety.md](execution-safety.md) |
