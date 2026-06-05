# Client-side MCP tasks (W7 — blocked)

Server-side task-augmented `tools/call` is **shipped** in clap-mcp on rmcp 1.7.x.

**Client-side task routing** (server polls client via `tasks/*` on `ClientHandler`) is **not** in rmcp 1.7.0. Track [rust-sdk PR #816](https://github.com/modelcontextprotocol/rust-sdk/pull/816).

When #816 lands in a released rmcp version:

- Implement `ClientHandler::{list_tasks, get_task_info, get_task_result, delete_task}` in test/client utilities
- Add an example pairing task-augmented server requests with async client completion
- Document `capabilities.tasks` negotiation on the client

Do not pin git SHAs of unreleased rmcp on main until the API is stable.
