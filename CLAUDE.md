# Project Context — Personal Dev Harness

> Project-level context for this repo (distinct from Gary's global `~/.claude/CLAUDE.md`). Auto-loaded each
> session — kept thin on purpose. **Detail and current state live in `wiki/`; this file is the map + the rules.**

This repo **is** the harness — a personal AI-engineering dev harness, re-platformed from the pure-instruction
`dev` agent skill (which lives on at `../Skills/dev/` and evolves on its own track). Process enforced by code,
context kept coherent via git-shaped branch/promote, memory that compounds at ~zero cost on local models
(oMLX), a harness that eventually builds and evolves itself.
**Direction: Rust-native, local-first, hardware-specific, forever-personal** (foundation RESOLVED →
Strategy A; see `wiki/decisions.md`).

## Start here (session protocol — do this first)

1. Read `wiki/index.md` → `wiki/active-work.md`. Brief the user on current status + last breadcrumbs.
2. Check `wiki/decisions.md` before re-opening a settled question (incl. its **Rejected Approaches**).
3. Then work.

## Workspace map

**This repo** (`~/Documents/Work/harness/`):
- `Cargo.toml`, `crates/` — the Rust harness: `provider` (`trait Provider` + oMLX + anthropic), `agent`
  (loop + 3 tools + Align gate on rusqlite), `pi-iso` (lifted from oh-my-pi — worktree isolation).
- `wiki/` — **living project memory. Read first, keep updated.**
- `research/00..16` — research foundation + design v2 + eval/teardown artifacts (point-in-time; see `wiki/index.md`).

**The dev skill** (`../Skills/dev/`) — the **judgment layer** being ported in: ml/production/rag heuristics,
pushback-and-teach, wiki-protocol, the mode playbooks. Pure instruction; evolves independently. Mine it for
content, don't fork it.

**Reference clones** (`../Skills/`, not git-tracked here): `harness-spike/` (validated Pi TS Align-gate spike:
`align-gate.ts`, oMLX config, `run-pi.sh`), `pi-mono/` (Pi), `oh-my-pi/` (Rust-core superset — source of
`pi-iso`), `symphony/` (coordinator), `beads/` (board schema). Mined for *designs*, not runtime deps.

## Conventions (the dev instruction convention)

- **Keep the wiki updated** (per `../Skills/dev/references/wiki-protocol.md`): during work, add breadcrumbs/decisions to
  `active-work.md`; on completion, append to `log.md` and update `index.md` (new pages) + `decisions.md`
  (decisions *and* rejected approaches). Breadcrumbs over summaries (specific findings, not "looked at X").
- **Align before execute.** For non-trivial work, gather context, state assumptions + success/failure
  criteria, and confirm before acting. We are *building* this gate — dogfood it.
- **Gate by a number or an artifact, never a vibe** (tests, proof, checklists — not opinions).
- **Integrity constraints (from `../Skills/dev/SKILL.md`, override everything):** never modify success criteria to fit
  the result; never report success without evidence; never fake results; if stuck >3 attempts, STOP and report.
- **Wu Wei:** is this actually causing a problem (blocking work, bugs, maintenance)? If not, drop it.
- **Dependencies are liabilities.** Standard-library / own-it over wrappers; the bar for a dep is high
  (reinforced by the Rust-native direction — we mine external refs for *designs*, not runtime deps).
- **Pushback-and-teach:** challenge vague/hand-wavy asks, surface the real decisions, narrate the *why*
  (the user is learning the stack through this work).
- **Commit discipline:** only when asked; stage explicit paths (never `git add -A`/`.`); don't stage
  `.DS_Store`; end messages with `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.
- **Local model:** oMLX at `http://127.0.0.1:8000/v1` for local/cheap roles; model-per-role. The key is
  resolved at runtime (`OMLX_API_KEY` env, else `auth.api_key` in `~/.omlx/settings.json`) — never bake it.

## Current focus

→ `wiki/active-work.md` — trunk + pillars + critical path DONE. **The daily-driver claim GRADUATED
(A6, Gary, 2026-08-02)** on the closed 5/5 trial window: new multi-ticket project work defaults to the
spine (board + gates + delegated worker); ad-hoc stays interactive CC **by posture, not shortfall**.
Posture unchanged: **contain Claude Code, don't compete** (decisions.md 2026-07-14).

**Current run — trial window 2** (`wiki/cv-mapper-trial.md` → the plan): the cv-mapper trial evidenced
that the gates **hold on outcome and fail on record** (no red gate survives `report_gate`'s upsert), the
loop **holds for the delegated arm and fails for the native arm**, and the operator tax is **not
established** because A6's two owed instruments were never built. This run fixes both and pays both —
the upfront-estimate column and the comparator arm land before any new measured claim
(`trial-ledger.md:92-94`).

> Anything naming a "next gate" for the daily-driver claim is a pre-08-02 breadcrumb, not a live gate.
> Check `decisions.md` "A6" before treating one as open.
