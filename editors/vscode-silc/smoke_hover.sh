#!/usr/bin/env bash
# Smoke-test sil-lsp: initialize, open a tiny doc, request hover on `list`.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BIN="${SILC_LSP_BIN:-$ROOT/target/release/sil-lsp}"
if [[ ! -x "$BIN" ]]; then
  BIN="$ROOT/target/debug/sil-lsp"
fi
if [[ ! -x "$BIN" ]]; then
  echo "Building sil-lsp (debug)…"
  cargo build -p sil-lsp --manifest-path "$ROOT/Cargo.toml"
  BIN="$ROOT/target/debug/sil-lsp"
fi

SRC='contract Article { has UUID $.id; has Str $.title; }
resource Articles for Article { query list; }
component Page {
    query $.articles = Articles.list();
    method render() { ui::text(:content("x")) }
}
'

# list() starts around the query line — compute offset of "list" after Articles.
LIST_OFFSET="$(python3 - <<'PY'
src = """contract Article { has UUID $.id; has Str $.title; }
resource Articles for Article { query list; }
component Page {
    query $.articles = Articles.list();
    method render() { ui::text(:content("x")) }
}
"""
idx = src.index("Articles.list()") + len("Articles.")
# line/character
line = src[:idx].count("\n")
col = idx - (src.rfind("\n", 0, idx) + 1)
print(line, col, idx)
PY
)"
LINE="$(echo "$LIST_OFFSET" | awk '{print $1}')"
COL="$(echo "$LIST_OFFSET" | awk '{print $2}')"

send() {
  local body="$1"
  local len
  len="$(printf '%s' "$body" | wc -c | tr -d ' ')"
  printf 'Content-Length: %s\r\n\r\n%s' "$len" "$body"
}

{
  send '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":null,"capabilities":{},"clientInfo":{"name":"smoke"},"rootUri":null}}'
  send '{"jsonrpc":"2.0","method":"initialized","params":{}}'
  # Escape source for JSON
  DOC_JSON="$(python3 - <<PY
import json,sys
print(json.dumps("""$SRC"""))
PY
)"
  send "{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{\"textDocument\":{\"uri\":\"file:///smoke.silc\",\"languageId\":\"silc\",\"version\":1,\"text\":$DOC_JSON}}}"
  send "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/hover\",\"params\":{\"textDocument\":{\"uri\":\"file:///smoke.silc\"},\"position\":{\"line\":$LINE,\"character\":$COL}}}"
  send '{"jsonrpc":"2.0","id":3,"method":"shutdown","params":null}'
  send '{"jsonrpc":"2.0","method":"exit","params":null}'
} | "$BIN" > /tmp/sil-lsp-smoke.out 2>/tmp/sil-lsp-smoke.err || true

if ! grep -q '"id":2' /tmp/sil-lsp-smoke.out; then
  echo "error: no hover response from sil-lsp" >&2
  cat /tmp/sil-lsp-smoke.err >&2 || true
  cat /tmp/sil-lsp-smoke.out >&2 || true
  exit 1
fi

if ! grep -q 'list' /tmp/sil-lsp-smoke.out; then
  echo "error: hover response did not mention list" >&2
  cat /tmp/sil-lsp-smoke.out >&2
  exit 1
fi

echo "sil-lsp smoke hover OK (list)"
