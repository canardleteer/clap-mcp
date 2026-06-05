#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

PORT="${CONFORMANCE_PORT:-0}"
if [[ "$PORT" == "0" ]]; then
  PORT="$(python3 - <<'PY'
import socket
s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()
PY
)"
fi
ADDR="127.0.0.1:${PORT}"
BIN="$ROOT/target/debug/subcommands_http${EXE_SUFFIX:-}"

echo "Building subcommands_http (http feature)..."
cargo build -p clap-mcp-examples --bin subcommands_http --features http

echo "Starting MCP HTTP server on ${ADDR}..."
"$BIN" --mcp-http "$ADDR" &
SERVER_PID=$!
cleanup() { kill "$SERVER_PID" 2>/dev/null || true; }
trap cleanup EXIT

for _ in $(seq 1 60); do
  if curl -sf "http://${ADDR}/mcp" -o /dev/null -X POST \
    -H 'Content-Type: application/json' \
    -H 'Accept: application/json, text/event-stream' \
    -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"smoke","version":"1.0"}}}' \
    >/dev/null 2>&1; then
    break
  fi
  sleep 0.25
done

echo "Running Node conformance CLI (suite active)..."
set +e
npx --yes @modelcontextprotocol/conformance server \
  --url "http://${ADDR}/mcp" \
  --suite active
STATUS=$?
set -e

echo "Conformance exit code: ${STATUS}"
echo "See docs/conformance-baseline.md to record results."
exit "$STATUS"
