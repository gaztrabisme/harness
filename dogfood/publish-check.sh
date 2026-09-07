#!/usr/bin/env bash
# publish-check.sh — pre-publish leak gate.
#
# Greps every file git would publish (tracked + untracked-but-not-ignored, i.e.
# `git ls-files -co --exclude-standard`) for personal-info patterns: absolute
# home paths, the operator username, tailnet/LAN IPs, and API-key literals.
# Exits 1 listing file:line matches; exits 0 when clean. Also fails if any
# *.db / *.db-shm / *.db-wal file would be published.
#
# This script excludes itself from the scan — it must contain the patterns
# it greps for.
#
# Usage: bash dogfood/publish-check.sh   (from anywhere inside the repo)
set -u

cd "$(git rev-parse --show-toplevel)" || { echo "publish-check: not inside a git repo" >&2; exit 1; }

SELF="dogfood/publish-check.sh"

PATTERNS='/Users/|GaryT|100\.106\.|192\.168\.|73067799|sk-[A-Za-z0-9]{8,}'

# Files git would publish, minus this script.
files=$(git ls-files -co --exclude-standard | grep -vxF "$SELF")
if [ -z "$files" ]; then
  echo "publish-check: nothing to scan"
  exit 0
fi

status=0

# 1) Local databases must never publish.
db_hits=$(printf '%s\n' "$files" | grep -E '\.db$|\.db-shm$|\.db-wal$')
if [ -n "$db_hits" ]; then
  echo "FAIL: database files would be published:"
  printf '%s\n' "$db_hits"
  status=1
fi

# 2) Forbidden content patterns (reported as file:line:match).
matches=$(printf '%s\n' "$files" | tr '\n' '\0' | xargs -0 grep -nE -- "$PATTERNS" 2>/dev/null)
if [ -n "$matches" ]; then
  echo "FAIL: forbidden patterns found:"
  printf '%s\n' "$matches"
  status=1
fi

if [ "$status" -eq 0 ]; then
  echo "publish-check: clean — $(printf '%s\n' "$files" | wc -l | tr -d ' ') files scanned, no leaks, no db files."
fi
exit $status
