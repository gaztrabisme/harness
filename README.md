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

This is a **personal** harness — local-first, hardware-specific, opinionated by design. Published
as a working artifact to read and mine, not as a supported product.

## License

MIT — see `LICENSE`. `crates/pi-iso` retains its upstream notice.
