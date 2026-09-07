#!/usr/bin/env bash
# Dogfood: the --json machine-readable face of `agent board|show|status`,
# asserted against a throwaway board DB (the same drive the integration test
# performs in Rust). Builds a fresh ticket through new → criteria → validation
# → note → align, then pins the JSON contract of all three verbs.
# Exits non-zero with a message on any failure; prints "board-json ok" on
# success. The mktemp dir is deliberately left behind (no recursive deletes).
set -euo pipefail

die() { echo "board-json-check: FAIL: $*" >&2; exit 1; }
command -v python3 >/dev/null 2>&1 || die "python3 not on PATH"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${HARNESS_AGENT_BIN:-$ROOT/target/release/agent}"
[ -x "$BIN" ] || die "no agent binary at $BIN (run: cargo build --release)"

WORK="$(mktemp -d)"
DB="$WORK/board.db"
export HARNESS_DB="$DB"

id="$("$BIN" new "json check" --kind question)" || die "agent new failed"
id="$(echo "$id" | awk '{print $1}')"
[ -n "$id" ] || die "could not parse the new ticket id from: agent new"
echo "== ticket $id =="

"$BIN" criteria "$id" "states/counts/tickets round-trip as JSON" >/dev/null || die "criteria failed"
"$BIN" validation "$id" "true" >/dev/null || die "validation failed"
"$BIN" note "$id" "seeded by board-json-check.sh" >/dev/null || die "note failed"
"$BIN" align "$id" >/dev/null || die "align failed"

# board --json: spine states, zero-filled counts, one in_progress ticket with
# the criteria we set and a (list) gate history.
"$BIN" board --json | python3 -c '
import json, sys
b = json.load(sys.stdin)
assert isinstance(b["states"], list) and len(b["states"]) == 8, b["states"]
assert b["states"][0] == "todo", b["states"]
ts = b["tickets"]
assert isinstance(ts, list) and len(ts) == 1, ts
assert ts[0]["status"] == "in_progress", ts[0]["status"]
assert b["counts"]["in_progress"] == 1, b["counts"]
assert ts[0]["workpad"]["criteria"] == "states/counts/tickets round-trip as JSON", ts[0]["workpad"]
assert isinstance(ts[0]["gates"], list), ts[0]["gates"]
' || die "board --json assertions failed"

# show <id> --json: one ticket object — the same shape, standalone.
"$BIN" show "$id" --json | python3 -c '
import json, sys
t = json.load(sys.stdin)
assert t["status"] == "in_progress", t["status"]
assert t["workpad"]["criteria"] == "states/counts/tickets round-trip as JSON", t["workpad"]
assert isinstance(t["gates"], list), t["gates"]
assert isinstance(t["red_gates"], int), t["red_gates"]
' || die "show --json assertions failed"

# status <id> --json: exactly the three keys.
"$BIN" status "$id" --json | python3 -c '
import json, sys
s = json.load(sys.stdin)
assert set(s.keys()) == {"id", "status", "attempt"}, s.keys()
assert s["status"] == "in_progress" and s["id"], s
' || die "status --json assertions failed"

echo "board-json ok"
