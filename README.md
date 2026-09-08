# harness

**A personal dev harness that makes AI coding agents work like contractors instead of cowboys.**

AI agents happily write code and report "done, everything works!" — and some of the time it doesn't.
This harness stops trusting the agent's word. Every task becomes a small written contract first; the
agent works in an isolated git worktree; and machines check the claim before a human ever sees it:
tests must pass, and a mutation tester breaks the code on purpose to prove the *tests* aren't fake.
Nothing reaches `main` until the gates are green **and** a human signs off twice — once on the
contract before work starts, once on the final merge. Those two signatures are the only steps the
machine can never perform for itself.

Posture: **contain Claude Code, don't compete.** Claude Code is the interface and the delegated
worker; the harness is the process cage around both.

## How a ticket flows

```
agent new  ──►  agent draft  ──►  [ALIGN: human keystone]  ──►  agent run/sprint (CC worker,
   todo         plan/criteria/       criteria confirmed          isolated worktree)
                validation                                            │
                                                                      ▼
agent land  ◄──  [review: human]  ◄──  agent verify  ◄──  agent harden
squash-merge                          validation cmd      mutation testing on the diff,
to main                               must exit 0         score ≥ 0.70 or bounce
```

- **Board**: 8-state spine (`todo → align → in_progress → verify → review → land → done`, plus
  `rework`) in a single SQLite file. Gates are rows with evidence, not vibes; the three human
  keystones (align, land, close) are never machine-cleared and never exposed programmatically.
- **Workers**: `claude -p` dispatched per-ticket with turn/wall-clock budgets, or a local model via
  oMLX (OpenAI-compatible) for cheap roles — drafting, memory, review.
- **Harden**: mutation testing scoped to the ticket's diff (cargo-mutants for Rust, cosmic-ray for
  Python), with provenance filters so template/generated files don't inflate the score, and honest
  skip notes when no mutation backend exists (JS/TS).
- **Oracles**: operator-authored `scripts/check_*.py` acceptance checks that workers cannot edit.
- **Memory**: `agent remember`/`recall` — lessons stored per-board auto-prime future workers.

## Verbs for the pi board extension

Machine-facing verbs so a pi extension can drive the board on macOS, Linux and Windows without
bash or the sqlite3 CLI:

- `agent gate <id> <name> pass|fail [--note T] [--json]` — record a gate row (provider `board`,
  machine source; `wiki-close` is a housekeeping gate, not a human keystone):
  `agent gate t1 wiki-close pass --note "log updated"`
- `agent close-check [--json]` — exit 0 iff every open ticket has a passing `wiki-close` gate
  dated today: `agent close-check --json`
- `agent wiki check [--root DIR] [--json]` — the numeric wiki housekeeping gate (active-work
  ≤ 400 lines / ≤ 4000 tokens / ≤ 24k bytes, index ≤ 120 lines, log ≤ 2000, index ↔ disk,
  facts.md checks): `agent wiki check --root . --json`

## Session preparation: `agent pi prepare`

The four local launcher steps (seed the agent directory, render `models.json`, the project-root
guard, wiki init) have one implementation, this verb; launchers and apps call it, they do not copy
it. Cross-platform, no bash, no sqlite3.

```bash
agent pi prepare \
  --template "$AGENT_TEMPLATE_DIR" \
  --agent-dir "$AGENT_DIR" \
  --cwd "$PROJECT_DIR" \
  [--stamp S] [--version V] \
  [--bppc-host H] [--omlx-key-env VAR] \
  [--allow-any-dir] [--no-wiki] [--json]
```

- **Seed** copies the managed set (`AGENTS.md`, `settings.json`, `settings.README.md`,
  `models.json.tmpl`, `agents/`, `extensions/`, `prompts/`, `skills/`) into the agent dir. User
  files (`auth.json`, `models.json`, `mcp.json`, `sessions/`, `logs/`, `wiki/`, `agent-hub/`,
  `tool-output-artifacts/`) are never touched. A re-seed happens only when the stamp differs;
  the stamp is the FNV hash of the template tree (prefixed by `--version`, or given outright via
  `--stamp`). The adjacent `.terax-seed-manifest` records SHA-256 template bytes, so edited
  managed files are kept and reported while unedited files are refreshed. When `--agent-dir`
  equals `--template`, seeding is skipped and the dir is used in place (the checkout case).
- **Render** writes `<agent-dir>/models.json` from `models.json.tmpl`, replacing `__BPPC_HOST__`
  (blank host defaults to `127.0.0.1`), `__OMLX_KEY__`, and `__OPENROUTER_KEY__` from
  `OPENROUTER_API_KEY` (blank when unset). Substitutions are JSON-escaped and the result must be
  valid JSON. An unresolved uppercase placeholder produces `WARN` while preserving the normal
  successful exit code. The oMLX key comes from the env var named by `--omlx-key-env` (default
  `OMLX_API_KEY`), falling back to `auth.api_key` in `$HOME/.omlx/settings.json`. Keys are never
  printed, in any mode.
- **Root guard**: `--cwd` must hold `.git` (any kind), `CLAUDE.md`, `AGENTS.md` or `wiki/`,
  unless `--allow-any-dir`; a failed guard skips wiki initialization and writes nothing under
  `--cwd`.
- **Wiki init**: creates `wiki/index.md`, `active-work.md`, `decisions.md` and `log.md` under the
  cwd, each only when missing; `--no-wiki` skips.

Progress prints `[k/4] name ... OK|WARN|FAIL|SKIPPED (detail)` lines on stderr; `--json` adds one
object on stdout (`steps`, `agentDir`, `modelsJson` as a path, `env.PI_CODING_AGENT_DIR`). Exit
codes: `0` when every step is OK, WARN or SKIPPED, `9` when the root guard fails, `6` on any other
failure, `2` on a usage error.

## Does it work?

Graduated to daily-driver status after a measured trial: 5 real tickets on a production project
plus 5 more on the harness itself, all through the full spine, with the gates catching real
failures (a fake-green test suite, an empty diff, a stale merge base) that would otherwise have
merged. The trial ledger, findings, and decision log live in `wiki/` — the repo's living memory,
kept honest by convention: breadcrumbs during work, decisions *and rejected approaches* recorded,
no success reported without evidence.

## Layout

- `crates/provider` — `trait Provider` + normalized `Request`/`Response`; openai-completions
  (oMLX/DeepSeek dialects) and anthropic-messages impls behind one trait.
- `crates/board` — the SQLite board: spine state machine, gates, keystones, memory.
- `crates/agent` — the CLI: loop, tools, draft/align/run/sprint/harden/verify/land, worker dispatch.
- `crates/pi-iso` — worktree/sandbox isolation, lifted verbatim from
  [oh-my-pi](https://github.com/can1357/oh-my-pi) (MIT, Copyright 2025 Mario Zechner, 2025-2026
  Can Bölük — license preserved in `crates/pi-iso/NOTICE`).
- `wiki/` — living project memory (state, decisions, log, trial ledger).
- `research/` — point-in-time research and design notes the decisions cite.
- `skills/harness-operator/` — a Claude Code skill that makes a CC session the conversational
  front-end: you speak intent, the session runs the verbs, keystones come back as explicit questions.

## Build

```bash
cargo build --release -p agent
```

The binary lands at `target/release/agent` (`target/release/agent.exe` on Windows).

## Releases

Releases are automated: push a `vX.Y.Z` tag on `main` and GitHub Actions builds the `agent` binary
for every supported platform and attaches it to the tag's release. Assets are named
`agent-<target-triple>[.exe]` plus a combined `SHA256SUMS` checksum file. The five targets:
`x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `aarch64-apple-darwin`,
`x86_64-apple-darwin`, `x86_64-pc-windows-msvc`.

## Setup

```bash
cargo build --release
export HARNESS_DB=~/path/to/board.db          # per project; fresh path = fresh board
target/release/agent new "first ticket" --kind build
```

Credentials are never baked: oMLX key via `OMLX_API_KEY` or `~/.omlx/settings.json`
(`auth.api_key`); DeepSeek reviewer (optional, advisory) via `DEEPSEEK_API_KEY`.

Every invocation also accepts a global `--db <path>` flag (before or after the verb)
that overrides `HARNESS_DB` for that run.

This is a **personal** harness — local-first, hardware-specific, opinionated by design. Published
as a working artifact to read and mine, not as a supported product.

## License

MIT — see `LICENSE`. `crates/pi-iso` retains its upstream notice.
