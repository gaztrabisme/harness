---
name: harness-operator
description: "Drive the Rust dev harness (board/gates/worktrees/delegated CC workers) from inside a Claude Code session — the session IS the operator front-end; the user never types agent verbs. USE WHEN starting or continuing multi-ticket build work on any project that runs through the harness board: creating/drafting/aligning tickets, dispatching claude workers, watching sprints, landing branches, reading board state, or setting up a fresh board for a new project. Keywords: harness, board, ticket, spine, align, land, sprint, worktree, keystone, agent new, agent run, mutation gate, harden, verify, oracle."
license: MIT
---

# Harness Operator

Run the harness **conversationally**: the user talks, the Claude Code session translates to `agent` verbs, and the two human keystones (align, land) always come back to the user as explicit questions before the verb fires. This skill is deliberately **thin and mechanical** — verbs, protocol, doctrine. Judgment about *what* to build belongs to you and whatever engineering-judgment skills you run alongside.

The harness posture is **contain Claude Code, don't compete**: CC is the interface and the worker; the harness is the process cage around both.

## Hard rules (override everything)

1. **Keystones are human.** `align` (criteria_confirmed), `land` (landed), `close` (resolved) fire only after the user's explicit confirmation *in this conversation* (AskUserQuestion or an unambiguous instruction). Never expose them on any MCP/programmatic surface; never batch-clear them without a batch pre-authorization the user granted explicitly.
2. **Integrity constraints:** never modify success criteria to fit a result; never report success without evidence; never fake results; stuck >3 attempts → STOP and report.
3. **Workers stay contained.** Workers act only inside their worktree; board DB writes are operator-only; protected oracles (`scripts/check_*.py`) are never worker-edited.
4. **Gate by a number or an artifact, never a vibe.**

## Setup

Per-machine values live in the user's global CLAUDE.md, not here:

| Item | Where it comes from |
|------|---------------------|
| Binary | `<harness repo>/target/release/agent` — record the path in global CLAUDE.md |
| Board DB | `HARNESS_DB=<path>` — **per project**, recorded in that project's CLAUDE.md. A fresh path = a fresh board (schema auto-created). |
| Worker knobs | `HARNESS_CLAUDE_TIMEOUT` (secs) `HARNESS_CLAUDE_MAX_TURNS` — raise for big tickets |
| Mutation gate | `HARNESS_MUTATION_THRESHOLD` (default 0.70) |
| Credentials | oMLX: `OMLX_API_KEY` or `~/.omlx/settings.json`; DeepSeek reviewer (optional): `DEEPSEEK_API_KEY` |

**Always operate from the project's base repo, never from inside a worktree** — `verify`/`harden` misfire on the empty-diff floor when run inside a worktree (doctrine, not a bug to silently work around).

After landing tickets that change the harness itself, rebuild the release binary so the landed change drives the next ticket.

## Session protocol

On session start in a harness-driven project: run `agent board`, read the project wiki, and brief the user — counts, land queue, what's blocked, last breadcrumbs. Then wait for direction.

## The conversational loop

The user never types verbs. Map their intent:

| User says | Operator does |
|-----------|---------------|
| "next ticket: X" / "add a ticket for X" | `agent new "X" --kind build\|bugfix\|refactor` → `agent draft <id>` → **vet the draft against the codebase** (are these the right criteria? does the validation command discriminate?) → correct via `plan`/`criteria`/`validation`/`note` → present a **digest** (goal, criteria, validation cmd, oracle plan — ≤10 lines; full workpad on request) |
| confirms the contract | **Keystone 1:** `agent align <id>` — only after explicit confirmation |
| "run it" / "go" | `agent sprint --worker claude --max 1` (or `--max N` for a chain) as a **background task**; notify on park. Sequential when tickets touch the same files. |
| (sprint parks at review) | Inspect the diff in the worktree; present a land digest (files, what changed, gate rows, worker cost). **Keystone 2:** `agent land <id>` only after explicit confirmation. Land needs a clean base tree — commit breadcrumbs first (explicit paths only). |
| "status" / "where are we" | `agent board` + `agent show <id>` for anything in flight, translated to prose |
| gate goes red | Don't rework. Read the failure (survivor mutant / failing test), `agent note <id> "<precise additive fix>"`, dispatch again. **Max 2 dispatches per ticket** — a second bounce means the contract was mis-scoped: stop, re-draft, re-align. |
| "this ticket is wrong" | `agent confusion <id> "..."` (bounces to align) or `agent rework <id>` (hard reset) — confirm before rework, it discards work |
| lesson worth keeping | `agent remember "<title>" --type lesson --salience 0.0–1.0 --body "..."` — lessons auto-prime future workers on that board |

Dependencies: `agent edge <child> <parent>`; land parents first, then re-sprint so children fork from the new main.

## Doctrine (learned, not theoretical)

- **Validation command shape:** cheap test runner **first** — `pytest tests/ && python scripts/check_x.py`. Harden auto-scopes mutation testing to the first `&&` segment; the oracle after `&&` runs only at verify. Never chain two pytest invocations.
- **Oracles discriminate.** A protected `scripts/check_*.py` must prove behavior (subset filters, fetchable URLs, rendered rows), not just exit 0. Operator-authored, staged on base, before dispatch.
- **JS/TS-only diffs:** harden reports an honest skip ("no mutation backend"), NOT test strength — the oracle + app tests are the whole gate there. Say so in the land digest.
- **Vacuous mutation rows** (template-stamped or no-code diffs) are understood, never trusted.
- **Timed-out workers record no cost/turns** — a missing figure is a telemetry gap, not a free run.

## Composition (when sibling skills exist)

These hand-offs apply if you run presale/delivery/engineering-judgment skills alongside; skip freely if you don't.

- **From a solution-architect layer:** acceptance-annex/WBS rows become tickets; the criterion ID rides into `agent criteria` (`"AC-12: ..."`). **One ID spine** — never invent a second numbering system.
- **To a delivery layer:** `agent board` + gate rows are the honest inputs to status reports and acceptance evidence. **Board `done` = built and landed — NOT client-accepted**; the client's signature lives in the delivery tracker, the operator keystone is only the internal one. Client scope changes are change requests first; if approved they become **new tickets** — never mutate a landed ticket's criteria.
- **With an engineering-judgment layer:** it fires at the two points the machine can't cover — vetting/correcting drafted contracts before align, and inside the worker. Keep a project wiki alongside the board: breadcrumbs during, log + decisions (incl. rejected approaches) on completion.

## Project onboarding (new project → harness-driven)

1. Pick a DB path; add to the project's CLAUDE.md: the `HARNESS_DB` line + "multi-ticket work runs through the harness (skill: harness-operator)".
2. Confirm validation tooling exists (test runner; for Python/Rust the mutation gate is real; for JS/TS plan an oracle).
3. First ticket through the full loop with the user watching the digests; then normal cadence.
