# MCP conformance baseline (clap-mcp)

This document tracks runs of the official Node harness against the Streamable HTTP example server.

## Harness

```bash
./scripts/run-conformance.sh
```

Uses `@modelcontextprotocol/conformance` against `http://127.0.0.1:<port>/mcp` with the `subcommands_http` example (requires `http` feature).

## Expected gaps (initial baseline)

Not claiming 100% pass on day one. Typical gaps to track:

- clap-specific prompt/resource shapes vs reference server
- DNS rebinding / `Host` validation edge cases on non-loopback binds
- elicitation SEP scenarios (until W5 examples cover them end-to-end)
- OAuth-protected URL scenarios (out of scope for default example)

Update this file with date, rmcp version, pass/fail counts after each conformance run.
