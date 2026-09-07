# cv-mapper — real-run trial findings (rolling window)

> **What this is.** cv-mapper is the first real (non-kata) multi-ticket project driven by the
> harness on an outside codebase. `active-work.md` names that as the open gate for the
> daily-driver claim. This file is the working record of a six-agent sequential investigation
> into **how the harness actually behaved**, not how it was designed to.
>
> **Status: CLOSED (2026-08-16).** Six-agent chain complete. Uncommitted. Two ledger rows are
> **drafted below and not appended** — putting them into `trial-ledger.md` is Gary's call, and the
> instrument's integrity depends on that staying his. The verdict is in `### Agent 6`.

---

## Protocol — read before you write

You are one link in a **chain**, not one of a fan-out. Everything below was written by the
agents before you. Your job:

1. **Read this whole file first.** It is bounded on purpose; you can afford to.
2. **Check whether your evidence overturns anything already in `## Confirmed`.** This is a
   required step, not an optional one. cv-mapper's own most-repeated failure was a figure that
   outlived the configuration it was measured under and kept getting quoted because the number
   itself was never wrong — only its basis was. A chain makes that easier, not harder. If you
   overturn something, move it to `## Retracted` with what killed it.
3. **Investigate your slice.** Your brief names it.
4. **Append what you found**, then compact.

### Rules for what you write

- **Every Confirmed line carries a pointer to its evidence** — a log line, a gate row, a file
  path, a transcript timestamp. A claim with no pointer goes in `## Working`, not `## Confirmed`,
  and says what would settle it.
- **Every measured figure carries the configuration it was measured under, in the same
  sentence.** "8 iterations" is not a fact; "8 iterations, oMLX provider, t2 sprint" is.
- **Distinguish what the harness did from what the operator did.** The whole question is which
  of the two the friction belongs to.
- **A negative result is a result.** "I looked for X in Y and it is not there" belongs in the
  file. Say where you looked.
- **Do not fix anything.** Read-only on cv-mapper, on the harness repo, and on the skills. The
  only file you write is this one.

### Budget

`## Working` has a hard cap of **400 lines**. If your append pushes it over, compact it before
you finish: promote what is settled and evidenced into `## Confirmed`, drop what is superseded,
keep what is still live. `## Confirmed`, `## Open` and `## Retracted` are not capped.

---

## Evidence map

| Source | Path |
|---|---|
| Project | `$HOME/Documents/Work/Mean/cv-mapper` |
| Board DB | `<project>/harness-board.db` — tables `ticket`, `gate_results`, `run`, `event`, `edge`, `memory` |
| Gate/sprint logs | `<project>/.harness/*.log`, `<project>/.harness/runs/` |
| Project wiki | `<project>/wiki/{log,active-work,decisions,index}.md`, `wiki/design/` |
| Worker sessions (t1) | `~/.claude/projects/-Users-<user>-Documents-Work-Mean-cv-mapper--harness-worktrees-t1/*.jsonl` (3 files) |
| Operator session | `~/.claude/projects/-Users-<user>-Documents-Work-Mean-cv-mapper/8d543b6a-*.jsonl` (5.6 MB) |
| Subagent sessions | same dir, `8d543b6a-*/subagents/*.jsonl` (15 files) |
| Harness instrument | `$HOME/Documents/Work/harness/wiki/trial-ledger.md` (column defs + pinned rules) |
| Harness case-law | `$HOME/Documents/Work/harness/wiki/case-law.md` |
| Operator skill | `~/.claude/skills/harness-operator/SKILL.md` |
| Harness source | `$HOME/Documents/Work/harness/crates/` |

**Never read a transcript whole.** The operator session is 5.6 MB and some subagent files exceed
1 MB. Stream and filter with `python3`/`grep`; extract, don't ingest.

---

## Confirmed

*(seeded by the operator before agent 1; each line is evidenced)*

> **Clock convention (agent 1).** The board DB stores **UTC**; git commit stamps and file
> mtimes are **+0700 (Asia/Saigon)**. Every time below is UTC unless marked. Cross-checked:
> `gate_results` `t1 landed` = 2026-08-07 15:54:12 UTC ↔ commit `baabd0d` authored
> `2026-08-07 22:54:11 +0700`.

- **Two tickets reached `done` and nothing else ever entered the board.** `ticket` table holds
  exactly t1 and t2, both `done`. — `harness-board.db`
- **Ten gate rows, all `passed=1`.** t1 and t2 each carry the full five-gate spine —
  `criteria_confirmed` (human `gary`, both at 08-06 16:06:17), `mutation_score`,
  `oracle_intact`, `tests_green`, `landed` (human). — `select count(*) from gate_results` = 10
  *(supersedes the seeded "8 gate rows" — see `## Retracted`)*
- **t1 wall-clock from criteria confirmed to landed: ~23h48m** (08-06 16:06:17 → 08-07 15:54:12).
  — `gate_results.created_at`
- **The oracle-tamper gate caught a real violation.** `INTEGRITY VIOLATION — t2 modified its own
  acceptance oracle (scripts/check_probe_readonly.py)`; re-run after revert passed. —
  `.harness/t2-verify.log`, `.harness/t2-verify2.log`
- **The mutation gate went red once and was cleared honestly.** t2 harden 0.600 < 0.70 with 376
  survivors, then 0.855 (caught=793 missed=135) after test strengthening. —
  `.harness/t2-harden.log`, `.harness/t2-harden2.log`
- **One t2 sprint parked without producing anything:** `[loop] hit max iterations (8)` →
  `[wt] no changes produced`, running the **oMLX** provider (`Qwen3.6-35B-A3B-oQ8-fp16-mtp`),
  not the claude worker. — `.harness/t2-sprint.log`
- **The case-law read path was inactive for the whole project.** `[case-law] no
  $HOME/Documents/Work/Mean/cv-mapper/wiki/case-law.md — loop read-side inactive`. —
  `.harness/t2-sprint.log`
- **The board's `memory` table has 0 rows**, on a project that locked decisions D1–D27. —
  `sqlite3 harness-board.db "select count(*) from memory"`
- **cv-mapper contributed zero rows to `trial-ledger.md`.** The instrument that decides the
  daily-driver claim captured nothing from the run that was supposed to test it. —
  `grep cv-mapper $HOME/Documents/Work/harness/wiki/trial-ledger.md` → empty
- **Five tickets' worth of product code was built outside the board** on 08-13 by three parallel
  subagents, with the mutation gate never run on any of it; disclosed by the operator as D27.
  3,646 lines of source, 3,289 of tests, 587 passing. — `<project>/wiki/log.md` §"What was
  traded away, on purpose", `<project>/wiki/active-work.md`
- **D24 had scoped eleven tickets and four operator keystones** before that bypass happened. —
  `<project>/wiki/log.md` 2026-08-13 entry
- **Repeated operator-visibility friction, six turns across three dates:** `"ETA?"`,
  `"Uuuuh it's been a day"`, `"Done yet?"`, `"I dont see anything moving"`, `"now what? Why is
  there a subagent running?"`, `"What exactly is t2?"`. — operator session, 08-07 / 08-10 / 08-12

### Agent 1 — the spine (mechanical timeline, dispatches, telemetry, provider, caps)

**The dispatch record: five runs, four closed.** — `select * from run` (5 rows)

| run_id | tkt | provider / model | started (UTC) | ended | iters | stop_reason |
|---|---|---|---|---|---|---|
| `t1-000-1786032383116` | t1 | `claude-cli` / `cc-default` | 08-06 16:06:23 | 16:16:38 | 33 | `completed` |
| `t1-000-1786033718128` | t1 | `claude-cli` / `cc-default` | 08-06 16:28:38 | 16:37:42 | 20 | `error` |
| `t1-000-1786065854679` | t1 | `claude-cli` / `cc-default` | 08-07 01:24:14 | 01:35:09 | 71 | `completed` |
| `t2-000-1786352125711` | t2 | `omlx` / `Qwen3.6-35B-A3B-oQ8-fp16-mtp` | 08-10 08:55:25 | **NULL** | **NULL** | **NULL** |
| `t2-000-1786352167614` | t2 | `omlx` / `Qwen3.6-35B-A3B-oQ8-fp16-mtp` | 08-10 08:56:07 | 08:57:01 | 8 | `max_iters` |

- **t1 ran entirely on the delegated Claude worker; t2 ran entirely on the local model.** Three
  `claude-cli` dispatches on t1, two `omlx` dispatches on t2, zero mixing. The claude worker was
  never pointed at t2. — `run.provider`; `.harness/runs/t1/*.claude.jsonl` carry CC
  `stream-json`, `.harness/runs/t2/*.jsonl` carry the native loop's role/step records
- **The three t1 claude dispatches cost $11.3732 and 124 turns, all on `claude-opus-5[1m]`**
  ($3.6436 / 33 turns · $2.7417 / 20 · $4.9879 / 71). Tokens across the three: 14,274 in,
  137,210 out, 9,420,437 cache-read, 316,135 cache-creation. — `type:"result"` line of each
  `.harness/runs/t1/*.claude.jsonl`, fields `total_cost_usd` / `num_turns` / `modelUsage`
- **The board stores turns but never cost.** `run`'s columns are run_id, ticket_id, attempt,
  model, provider, sampling, project, started_at, ended_at, iters, stop_reason — there is no
  cost/token column to be empty. `worker::WorkerResult` *does* parse `total_cost_usd`, but
  `main.rs:1074-1080` puts `num_turns` into `iters` via `finish_run` and prints the cost to
  stdout only. Cost for cv-mapper survives *by accident*, in the teed CC stream. —
  `crates/board/src/schema.rs:75`ff, `crates/agent/src/main.rs:1074-1080`,
  `crates/board/src/board.rs:318-331` (`finish_run` takes only `stop_reason` + `iters`)
- **The local-model runs have no cost/turn telemetry of any kind** — the native-loop recorder
  writes `{step, role, content}` and nothing else; no usage block, no timing, no result event.
  — `.harness/runs/t2/t2-000-1786352167614.jsonl`, all 30 lines
- **`iters` means two different things in the same column.** For an omlx run it is the native
  loop counter (capped at 8); for a claude run it is CC's `num_turns`, which is not what the
  harness's own `--max-turns 40` bounds — run 3 recorded **71**. — `main.rs:1047`
  (`--max-turns 40`) vs `run.iters` = 71
- **The 8-iteration cap is a compiled-in global default, not a provider or operator setting.**
  `const DEFAULT_MAX_ITERS: usize = 8` at `crates/agent/src/main.rs:70`, read as
  `env_or("HARNESS_MAX_ITERS", DEFAULT_MAX_ITERS)` at `main.rs:433`. It binds the **native
  loop only** (`for _ in 0..max_iters`, `main.rs:532`); the delegated claude worker uses
  `WORKER_MAX_TURNS = 40` instead (`worker.rs:26`). No `HARNESS_MAX_ITERS` exists anywhere in
  cv-mapper, in the shell rc files, or in `~/.claude/settings.json`, and cv-mapper has no
  `providers.toml` — so both the oMLX provider and the cap on the t2 sprint were **defaults the
  operator never chose**. — grep across `$HOME/Documents/Work/Mean/cv-mapper`,
  `~/.zshrc`/`~/.zprofile`/`~/.zshenv`, `~/.claude/settings.json`: no hits
  → **closes the `## Open` item.** It is a harness default, and a defect only in the sense that
  8 iterations is not enough for a real ticket: the t2 sprint spent all 8 reading and never
  reached a write. The doc comment on that constant is written entirely about *token* budget
  (`DEFAULT_MAX_TOKENS`), and cites live measurement for **4096 tokens** — but cites nothing
  for **8 iterations**.
- **The sprint fell to oMLX because `--worker claude` was not passed.** `agent sprint` takes an
  optional `--worker claude`; without it `claude_cfg` is `None` and the native loop runs on
  `config::select`'s default, `HARNESS_PROVIDER` unset → `omlx`. —
  `main.rs:15` (usage), `main.rs:276-283`, `main.rs:1508`, `config.rs:45-49`
- **The orphan run is the telemetry gap, and the harness marks it by design rather than losing
  it.** `t2-000-1786352125711` has `ended_at IS NULL`, which `crates/board/src/model.rs:184-186`
  documents as the intended signal for an interrupted run ("so an interrupted run is the
  queryable `ended_at IS NULL` rather than an orphan file"). Its recorder file is 5 lines: system
  prompt, ticket title, one reasoning block, one empty assistant turn, one `ls -R` tool result.
  The replacement run started **42 seconds later** with a different opening plan. So the row is
  detectable but carries **no reason, no duration, no iteration count** — you can find it, you
  cannot learn anything from it. — `.harness/runs/t2/t2-000-1786352125711.jsonl`,
  `crates/board/src/board.rs:314-317`
- **The one non-completion on the claude worker was an Anthropic rate limit, not a harness
  fault.** Run 2 ended `is_error: true`, `terminal_reason: "api_error"`,
  `api_error_status: 429`, result text `"You've hit your session limit · resets 12:10am
  (Asia/Saigon)"` — after burning $2.7417 and 20 turns. The board flattens all of that to
  `stop_reason='error'`. The next dispatch came **8h46m32s** later. —
  `.harness/runs/t1/t1-000-1786033718128.claude.jsonl`, `result` line
- **`gate_results` is last-write-wins and back-dates the surviving row to the failed attempt.**
  `report_gate` does `ON CONFLICT(issue_id, gate, provider, attempt) DO UPDATE SET source,
  passed, note` — `created_at` is deliberately untouched, and `attempt` never incremented
  (both tickets are `attempt = 0`). Three rows in this project are therefore stamped before the
  event that produced their contents:
  - `t2 oracle_intact` reads `passed=1, created_at=2026-08-12 16:11:18` — the exact second the
    **INTEGRITY VIOLATION** was written. The pass came at 16:12:37.
  - `t2 mutation_score` reads `0.855, created_at=2026-08-10 11:22:59` — the second the **0.600
    FAIL** was written. The 0.855 pass came at 08-12 15:36:13, **two days later**.
  - `t1 mutation_score` reads `0.781, created_at=2026-08-06 16:26:42` — but the run that wrote
    the code it scored did not start until 08-07 01:24, and the re-report is at 08-07 15:45:43.
    The row is back-dated **23h19m**.
  — `crates/board/src/board.rs:239-249`; `gate_results` vs `event` rows 21/25/26/27/36/37/38/39;
  `.harness/t2-harden.log` (mtime 08-10 18:22 +0700 = 11:22 UTC), `.harness/t2-verify.log`
  (mtime 08-12 23:11 +0700 = 16:11 UTC)
- **Consequently the board holds no record that any gate ever went red.** `event` rows do log
  each `gate_reported` (t1 `oracle_intact` ×3, t1 `mutation_score` ×2, t2 `mutation_score` ×2,
  t2 `oracle_intact` ×2) but carry **no verdict field** — `kind`, `from_status`, `to_status`,
  `provider`, `note`(= gate name) only. Both real failures survive **only** in the
  `.harness/*.log` files, which survive only because the second run wrote to a `…2.log`
  filename. — `select * from event`, `crates/board/src/schema.rs` (event DDL)
- **The board's `run` table covers dispatches only, never gate executions.** `harden`, `verify`,
  `review` and `land` open no run row, so the two hardens, two verifies and two lands that make
  up most of the visible harness activity leave no telemetry beyond a `gate_reported` event and
  a log file. — no `run` rows at 08-10 11:22, 08-12 15:36, 08-12 16:11/16:12, 08-13 01:05

**Where the wall clock actually went** — board-tracked span 08-06 15:56:28 → 08-13 01:05:42 =
**6d 9h 9m**. Sum of all five dispatches = **~31m 50s**, i.e. **0.35%** of it.

| window (UTC) | elapsed | what occupied it | harness or operator |
|---|---|---|---|
| 08-06 15:56 → 16:06 | 10m | ticket creation, 12 `workpad_edited` events, 1 edge, both Align gates | operator |
| 08-06 16:06 → 16:37 | 31m | t1 dispatches 1–2 (10m15s + 9m04s) + the 0.781/oracle gate pair | harness |
| 08-06 16:37 → 08-07 01:24 | **8h47m** | 429 session limit, then overnight | environment |
| 08-07 01:24 → 01:35 | 11m | t1 dispatch 3 (71 turns, $4.99) | harness |
| 08-07 01:35 → 15:46 | **14h11m** | one lone `oracle_intact` re-report at 11:03; otherwise nothing on the board | operator, off-board |
| 08-07 15:46 → 15:54 | 8m | verify → review → human land `baabd0d` | harness + human |
| 08-07 15:54 → 08-10 08:55 | **2d 17h** | board silent (Fri→Mon). 4 spec/decision commits land 08-10 08:04–10:46 | operator, off-board |
| 08-10 08:55 → 08:57 | 96s | both t2 oMLX dispatches; second dies at `max_iters` with `[wt] no changes produced` | harness |
| 08-10 08:57 → 11:22 | 2h25m | t2's product code gets written — **by something with no `run` row** | unattributed (see `## Working`) |
| 08-10 11:22 | — | harden #1 → 0.600 FAIL, 376 survivors | harness |
| 08-10 11:23 → 08-12 15:36 | **2d 4h** | board silent. 3 decision commits land 08-12 14:00–16:11 | operator, off-board |
| 08-12 15:36 → 16:12 | 36m | harden #2 (0.855) → verify #1 (VIOLATION) → verify #2 (PASS) → review | harness |
| 08-12 16:12 → 08-13 01:05 | **8h53m** | parked at `review` awaiting the human land keystone | human |
| 08-13 01:05 → 15:27 | 14h22m | the off-board build: 3 CC subagents, 7,388 lines in commit `21fb973` | operator, board silent |

- **Dispatch counts, and what ended each.** t1: **three** dispatches — completed (33 turns),
  429 rate-limit error (20 turns), completed (71 turns). t2: **two** dispatches — one orphaned
  at 42s with no stop reason, one exhausted at `max_iters` in 54s having produced nothing.
  — `run` table + the five recorder files
- **No orphaned worktrees, branches or half-created tickets.** `git branch -a` shows `master`
  alone; `git worktree list` shows only the main checkout; `.harness/worktrees/` is empty
  (mtime 08-13 08:05 +0700 = the moment t2 landed). Both `harness/t1` and `harness/t2` were
  cleaned up after landing. The board's silence on the five off-board tickets is total and
  consistent — no stub rows, no dangling edges (`edge` holds exactly one row, t2→t1). —
  `git branch -a -v`, `git worktree list`, `ls -la .harness/worktrees/`
- **The project's own `log.md` is silent for the entire harness window too.** Zero mentions of
  any date between 2026-08-07 and 2026-08-12; entries jump 08-06 (design cell) → 08-13
  (scoping) → 08-13 later (the build). The six days in which both tickets were actually driven
  through the board produced no log entry. — `grep -c "2026-08-0[789]\|2026-08-1[012]"
  <project>/wiki/log.md` → 0

### Agent 2 — the workers (what happened inside the cage)

> **Session map (agent 2).** Operator session = `8d543b6a-…jsonl`, 2,648 records, 08-06 14:22 →
> 08-16 06:19 UTC; line numbers below are 1-based record numbers in that file. t1 worker
> sessions: `17907ebb…` (run 1, 133 rec), `c85b09de…` (run 2, 82 rec), `f2309f22…` (run 3, 289
> rec). t2's real worker: `…/8d543b6a-…/subagents/agent-at2-linkedin-probe-ab9257b6b3c38f54.jsonl`,
> 440 records, 08-10 09:31:51 → 08-12 14:51:13 UTC.

**THE HEADLINE: the harness drove none of the code that landed under t2.** At 08-10 09:31:06
the user typed `"Should deploy an Opus subagent for this"`; at **09:31:51** the operator's own
Claude Code session spawned an in-process Task teammate — `{"agentType":"t2-linkedin-probe",
"model":"opus","taskKind":"in_process_teammate","permissionMode":"bypassPermissions"}` — with a
hand-written 7-AC prompt, pointed at the worktree the failed harness sprint had left behind
(`"Agreed — and it keeps the harness discipline, since the worker already made the worktree"`,
op L1360). That subagent wrote **both** files in the landed commit: `Write
src/cv_mapper/probe/linkedin_selectors.py` at 09:38:58, `Write tests/test_linkedin_selectors.py`
at 09:42:16, rewritten 10:27:42 / 10:30:23, extended across 08-12 13:57–14:51. `6528be8` is
exactly those two files, 4,569 insertions. — subagent transcript + `.meta.json`; op session
L1358/L1360/L1361; `git show --stat 6528be8`
  → **closes agent 1's `## Working` item.** t2 has a ticket number, five green gate rows and a
  squash-landed branch, and the only harness components that touched it are the worktree, the
  gates, and `land`. Its worker was a Claude Code Task subagent that the board has never heard of.

- **t2's real worker cost is not recorded anywhere, and is the larger of the two.** 282
  assistant turns on `claude-opus-5`, 251,864 output tokens, 89,270,333 cache-read, 3,448,517
  cache-creation, 525 input — vs **124 turns / 137,210 out / 9,420,437 cache-read** for all
  three harness-dispatched t1 runs combined ($11.3732). No `total_cost_usd` appears in the
  subagent transcript, so no dollar figure is recoverable from any artifact on disk. — sum of
  `message.usage` over the subagent jsonl; cf. agent 1's t1 figures
- **The operator amended the oracle mid-ticket, with the user's approval, and told the worker
  not to.** At 08-10 10:18:29 the operator sent the subagent the live-run findings (5 attempted
  / 0 resolved: all three §4.1 scopes match zero elements, class names are hashed CSS-module
  tokens, no `<h1>`, `aria-controls` absent, `Save to PDF` absent from `body.innerText` while
  closed) and rewrote the safety property: *"AC-P1.2 as written made this ticket unachievable…
  PERMITTED: clicking the More button… STILL FORBIDDEN… actuating any ITEM inside that dropdown…
  The oracle `scripts/check_probe_readonly.py` is operator-authored — **I will amend it myself**…
  DO NOT EDIT IT."* Committed to master as `d9c8a31` (08-10 10:21 UTC) and `cp`'d into the
  worktree at **10:20:02** (op L1517). — subagent L57; `git show d9c8a31`
- **Containment held on every oracle, in all four workers.** Zero `Write`/`Edit` to any
  `scripts/check_*.py` in run 1, run 2, run 3 or the t2 subagent; the only `scripts/` bash lines
  are `python scripts/check_*.py` invocations plus the subagent's `git diff
  scripts/check_probe_readonly.py` (reading the operator's amendment, which it was told to do).
  Every product `Write`/`Edit` in all four sessions has a `file_path` under its own worktree;
  out-of-tree writes are `/tmp` and session-scratchpad only (run 1 `/tmp/probe_smoke`,
  `/tmp/smoke2`, `mktemp -d`; run 2 `/tmp/pyproject.toml.bak`, `/tmp/t_probe.py`; subagent four
  `…/scratchpad/*.py`). — `file_path` + `"command"` extraction over all four indexes
- **Two prompt rules were broken, both trivially and both read-only.** (a) *"Work ONLY inside the
  current directory. Never read or modify files outside it"* — run 1 and run 2 used `/tmp` for
  smoke tests, and run 3 read its own CC session log under `~/.claude/projects/…` four times to
  recover mutmut's stdout. (b) *"Do NOT commit, branch, push, or otherwise touch git"* — run 1
  ran `git status --porcelain` + `git diff --stat -- scripts/` (**to prove it had not touched the
  oracle**), run 3 ran `git status --short` ×3. No mutating git command in any worker. —
  `worker.rs:100-116` (the rules) vs the four indexes
- **Worker honesty was the strongest thing in the trial. Four for four, unprompted.**
  - Run 1 (33 turns): asserted macOS 15 / Playwright 1.56.0 in a wiki edit, then caught itself —
    *"I claimed version numbers in that wiki entry — I must verify them rather than assert them"*
    → `sw_vers` → *"Both guesses were wrong — macOS is 26.5.2 (not 15) and Playwright is 1.48.0"*
    and corrected the `[measured]` line. Its final report volunteers two deviations from the Plan
    (it refused plan step 3 because it contradicted AC-P2.5) and *"`run_probe.bat` is untested —
    I have no Windows machine here."* — w1 L102/L106/L131
  - Run 3 (71 turns) called 7 of its 16 surviving mutants *"genuinely equivalent"* with a
    demonstration for each, and reported deleting dead code *"rather than declaring them"*. — w3 L287
  - The t2 subagent closed its first cycle with *"`scripts/check_probe_readonly.py` is your
    amendment, untouched by me"*, and wrote its own limit into the code: *"a genuinely restricted
    profile whose action menu carries neither identity row would also be rejected as `wrong_menu`.
    Nothing observed so far distinguishes that from a real wrong menu."* — subagent L172
  - No run reported a gate it had not run. The subagent's `"Both gates green"` refers to pytest +
    the oracle, which it ran; it never quoted a mutation score, leaving that to `agent harden`.
- **Run 3 predicted the 4h25m runaway, and the operator filed the warning as cosmetic.** Run 3's
  report (01:35:08): *"`mutants/` is **committed to git**, and mutmut reused cached per-mutant
  results… Worth deciding: `mutants/` being tracked is what let this happen, and it will happen
  again on the next harden."* At 11:03:25 the operator dispatched `agent harden t1`; at 11:13:38
  it described the same directory as *"~2,500 lines of generated artifacts. That doesn't belong
  in the repo and I'll strip it before landing"* — a tidiness framing, after the harden was
  already running. cosmic-ray, scoped to "t1's changed Python lines over master", then mutated
  the generated copies: **19,414 mutants instead of 726**, discovered 4h25m later at 15:28:48
  (*"19,414 mutants, not ~700… it's my fault for filing it as cosmetic"*). Untracking `mutants/`
  (`696d306`) dropped it to 726 and the gate finished in ~6 min. — w3 L287; op L855/L873/L900/L933/L948
- **The causal chain for that is a harness one.** The worker chose its own mutation tool (mutmut,
  since the ticket named none), left its scratch tree in the worktree, and the harness's own
  `git::commit_worktree` — `git add -A` then commit `"{id}: work in progress"`,
  `crates/agent/src/git.rs:289-297` — swept it onto the branch. `agent harden`'s diff-scoping
  then treated generated code as ticket code. The same `git add -A` is what put the operator's
  `cp`'d oracle onto `harness/t2` (`414f0d2`, 08-10 10:47 UTC; `2fe366c`, 08-12 14:53 UTC).
- **The t1 mutation discrepancy is two different tools, not a scoping drift.** The worker ran
  **mutmut** (`.venv-mut/bin/mutmut run`, config appended to `pyproject.toml` by run 2 at
  16:31:38, scoped to `src/cv_mapper/probe/…`): 476/492 = 0.968. `agent harden` runs
  **cosmic-ray** scoped to the branch diff: 567 caught / 159 missed = 726 = 0.781. Both are
  correct for their tool; neither cites the other, and the board keeps only cosmic-ray's.
  — w3 L102/L106/L223/L266/L287; `.harness/t2-harden.log` line 2; op L933/L948
  → **closes agent 1's `## Working` item on the 0.781 / 0.968 split.**
- **The 429 lost almost nothing, and only 33 minutes of the 8h47m gap was the rate limit.** Run
  2's last successful action (16:37:41.339) was the `Write` of `tests/test_windows_deploy_helpers.py`
  — 1,456 lines — and the 429 landed 0.6s later. The harness's WIP commit preserved it
  (`233211b`); run 3 opened on it (*"the prior session's coverage is extensive"*) and later proved
  it had *already* been sufficient (0.914 with no new tests, once the stale mutmut cache was
  cleared). The limit message said *"resets 12:10am (Asia/Saigon)"* = 08-06 17:10 UTC, i.e. 33
  min after the stop; the operator returned at 08-07 00:33 UTC and re-dispatched at 01:24. What
  the $2.7417 actually bought: the file, minus its validation loop. — w2 L75/L77/L81; op
  L771/L779/L781; w3 L157/L287
- **Between the two, the operator hand-diagnosed the three failures and put them on the board.**
  `agent note t1` at 00:34:51 carried a per-failure diagnosis and an explicit ruling (*"RULING:
  reject Chromium — return None"*), and refused to manufacture progress: *"I'm not going to
  hand-patch tests to manufacture progress."* Run 3 fixed all three. — op L810/L812; w3 L287
- **t1's mutation gate went red too, at 0.362.** 08-06 16:26:42: *"Mutation gate red — 0.362
  against a 0.70 threshold, 462 survivors"*, `KILLED|262 SURVIVED|462` read from
  `harness-cr-t1.sqlite`. That is the timestamp the surviving 0.781 row is back-dated to. — op
  L707/L713; run 3's *"was 0.362"*
- **A sixth dispatch was attempted and left no row.** At 16:28:08 the operator launched `agent
  sprint` with the shell still inside the worktree; it failed with `[run=ERR(git ["worktree",
  "a…])]` before opening a `run` row, and was re-issued 30s later from the base repo. The board's
  five rows therefore undercount dispatch *attempts* by one. — op L733/L747/L749/L752
- **The plan the local model followed was machine-drafted, known-defective, and never fixed.**
  `agent draft t2` (08-06 15:59:14) proposed step 2 *"Create tests/fixtures/linkedin_locale.py
  with a constant LOCALE_TABLE"*; the operator read the draft and rejected it out loud — *"a
  worker satisfies that by hardcoding five dicts. And it misses P1's safety property entirely"*
  — then overwrote **criteria** and **validation** (16:00:00) and **left the plan alone**. `agent
  plan` was never invoked; `select plan from ticket where id='t2'` still returns the draft's four
  steps verbatim. That plan pointed at a file the oracle does not import, and contradicted the
  ticket's own Notes (*"DO NOT: hardcode a locale table"*) and its Validation command (`python
  scripts/check_probe_readonly.py`, which imports `cv_mapper.probe.linkedin_selectors`). It then
  passed the human Align gate at 16:06:17. — op L662/L663/L665/L667; board `ticket.plan`
- **The local model did exactly what that plan said, and ran out of budget one step before
  realising it was wrong.** t2 sprint, oMLX `Qwen3.6-35B-A3B-oQ8-fp16-mtp`, 8 iterations, 54s,
  30 recorder lines: its step-2 reasoning is a near-copy of the drafted plan's four steps. All
  **12 tool results across those 8 turns are reads** — `ls -R`, then `02-linkedin.md` five times
  (truncated full read, two greps, offset 268-367, offset 240-339 — the last two overlap by 72
  lines), plus the oracle, `decisions.md`, and three files it did not need. Only at step 27 (the
  8th and last iteration) does it reach the right conclusion: *"The check_probe_readonly.py
  imports `cv_mapper.probe.linkedin_selectors` and expects a `resolve_target(page, target)`
  function."* Then the loop hit `max_iters` and `[wt] no changes produced`. — the 30 lines of
  `.harness/runs/t2/t2-000-1786352167614.jsonl`
- **The workpad the local model got was materially thinner than the one the operator hand-wrote
  35 minutes later.** The rendered system prompt (5,534 chars) contains **AC-P1.7 only**; AC-P1.1
  through AC-P1.6 are named in prose in the Notes but never stated. The subagent prompt at 09:31
  writes all seven out in full, with the design problem in Target B spelled out. Same ticket,
  same day, two contracts. — `t2-000-1786352167614.jsonl` line 1 vs op L1361
- **The board could have answered three of the visibility complaints and never was asked.**
  → **closes agent 1's `## Open` item.** It is a read-path failure on dispatches and a genuine
  coverage gap on gates, and a durability failure only for cost.

  | user turn (UTC) | what was true at that second | board had it? |
  |---|---|---|
  | `ETA?` 08-07 02:23:44 | run 3 **finished 48 min earlier** — `ended_at 01:35:09`, `completed`, 71 iters | **yes** |
  | `Uuuuh it's been a day` 08-07 11:02:32 | same row; suite green since 01:35; sprint wrapper hung | **yes** |
  | `Done yet?` 08-07 15:27:31 | `agent harden t1` in flight, 4h25m in | no — harden opens no run row |
  | `I think it's been hours?` 08-07 15:28:19 | same | no |
  | `I dont see anything moving` 08-10 08:59:20 | sprint **ended 2m19s earlier** — `08:57:01`, `max_iters`, 8 | **yes** |
  | `Now what?` 08-10 10:17:35 | operator's own live probe runs | no — off-board |
  | `now what? Why is there a subagent running?` 08-12 14:22:04 | the Task subagent | no — never on the board |
  | `it went idle` 08-12 15:10:49 | `agent harden t2` alive, pids 2759/2884 | no — harden opens no run row |

  At `ETA?` the operator ran `ls -lT .harness/runs/t1/` with an `echo "--- current run started ---"`
  label in front of a plain `date`, misread its own label, and predicted *"roughly 10–20 minutes"*
  for a run that had already completed; then waited 8.5h on a background-task notification. At
  11:02 it ran `agent board`, which prints `t1 [build] in_progress attempt=0 "<title>"` and no
  run telemetry at all. — op L832/L833/L840/L848/L873; `.harness/t2-sprint.log`; `run` table
- **`agent trajectory` — "print the latest run's row + recorded trajectory", the exact verb for
  those questions — was never invoked once in the whole project.** Twelve of the 24 verbs never
  were: `plan`, `confusion`, `run`, `explore`, `review`, `trajectory`, `ready`, `status`,
  `rework`, `remember`, `recall`, `recall-body`. `remember`/`recall` never firing is the direct
  cause of the empty `memory` table; `plan` never firing is why the defective drafted plan
  survived; `review` never firing means the decorrelated DeepSeek acceptance review never ran on
  either ticket. — regex over every Bash `command` in the operator session; `main.rs:10-29` (usage)

### Agent 3 — the operator tax

> **Method, stated once (agent 3).** "Operator minutes" = **attended session time**, reconstructed
> by clustering the operator session's records (user turns, assistant turns, tool results;
> metadata records dropped) and breaking a cluster whenever the gap between consecutive records
> exceeds a threshold, then summing cluster spans. Threshold sensitivity is large and is the main
> softness: **190 min (5-min gap) / 301 min (10-min) / 392 min (20-min)** for the whole session.
> All figures below use the **10-minute** threshold unless a band is given. Two known biases, in
> opposite directions: cluster spans **over**count, because machine wait inside a cluster (a 94s
> `agent draft`, a 600s `harden` that timed the Bash tool out) is charged to the operator; and
> they **under**count, because the human's out-of-session work is invisible here — the 8-minute
> 08-10 07:55→08:03 gap is the user running the probe zip on the recruiter's real laptop and
> reads as zero. Every figure below is an estimate; ranges are honest, not decorative.
> — `clusters.py` over `8d543b6a-….jsonl`, 2,648 records

**The two board tickets cost ~88–146 attended operator minutes, and the board mechanics inside
that were ~33 min.** Board-driven span 08-06 15:54:13 → 08-13 01:06:27 sums to **145.7 min** at
the 10-min threshold and **87.6 min** at 5-min. The split inside the 145.7, by hand-attribution
of each cluster (see the two tables below): **~33 min harness mechanics · ~100 min cv-mapper
product work · ~13 min human-keystone waiting** (23% / 69% / 9%). The boundary calls are
judgment — treat the harness share as **30 ± 6 min**. — cluster listing at gaps 300/600/1200

**t1's operator time was ~100% harness tax. The operator did no cv-mapper product work on t1 at
all** — every attended minute went to authoring the contract, adjudicating a gate, or clearing a
keystone, because the delegated Claude worker did all the product thinking.

| t1 slice (UTC) | span | min | category |
|---|---|---|---|
| binary check, `board`, `new` ×2 | 08-06 15:55:26–15:56:32 | 1m06 | pre |
| `draft t1` (94s runtime) → **output rejected out loud** | 15:56:32–15:58:21 | 1m49 | pre |
| hand-written `criteria`/`validation`/`note` | 15:58:21–15:58:48 | 0m27 | pre |
| oracle `check_probe_discriminates.py`, 128 lines, one Write, never revised | 15:58:48–15:59:14 | 0m26 | pre |
| Align keystone wait (AskUserQuestion, covers **both** tickets) | 16:01:08–16:06:11 | 5m03 ÷2 | pre |
| mutation red 0.362 → `note` → re-dispatch (**1 sprint failed: shell inside the worktree**) | 16:26:42–16:28:38 | 1m56 | mid |
| 429 adjudication: read stream, check WIP commit, `harden`, diagnose 3 failures, `note` ruling | 08-07 00:33:00–00:35:05 | 2m05 | mid |
| re-dispatch | 01:24:02–01:24:20 | 0m18 | mid |
| `ETA?` answered wrong off file mtimes | 02:23:44–02:24:19 | 0m35 | mid |
| discover run 3 finished 9h27m earlier; re-run `harden` (**`timeout` not on macOS, 1 retry**) | 11:02:32–11:03:25 | 0m53 | mid |
| Bash 600s timeout on `harden`; apology | 11:13:26–11:13:38 | 0m12 | mid |
| **the 19,414-mutant runaway**: diagnose, kill, untrack `mutants/`, re-run (**3 failed Bash**) | 15:27:31–15:29:41 | 2m10 | mid |
| confirm 726 mutants | 15:39:42–15:39:58 | 0m16 | mid |
| read harden result → `verify` → build land digest | 15:45:43–15:46:22 | 0m39 | post |
| Land keystone wait | 15:46:22–15:54:02 | 7m40 | post |
| `land t1` + report | 15:54:11–15:54:23 | 0m12 | post |

**t1 ≈ 13m04 hands-on + 10m12 keystone waiting ≈ 23 min, all of it tax.** — record numbers
644/651/656/693/702–760/763–812/819–823/830–840/845–873/878–934/940–968

**t2 ran the other way: ~64 attended minutes, of which ~17–20 min was harness mechanics and
~44 min was real cv-mapper work** — the live probe runs against five real profiles, the DOM
diagnosis that killed the design's selector strategy, the D14 Excel discovery. The tax slices:

| t2 harness slice (UTC) | span | min |
|---|---|---|
| `draft t2` (23s) → **rejected** → `criteria`/`validation`/oracle (162 lines)/`note`/`edge` | 08-06 15:59:14–16:00:48 | 1m34 |
| Align keystone wait (other half) | 16:01:08–16:06:11 | 2m32 |
| `criteria --help` / `note --help` → `Error: usage: …` one-liners, no help text | 08-10 08:53:53–08:54:00 | 0m07 |
| `criteria t2` AC-P1.7 + `note t2` + commit | 08:55:00–08:55:25 | 0m25 |
| **`agent sprint --help` fires a real sprint**; recover, re-dispatch properly | 08:55:25–08:56:07 | 0m42 |
| `max_iters` post-mortem → 4 greps into harness source → user cuts it off → re-explain | 08:59:20–09:00:17 | 0m57 |
| `pyproject.toml` mutation-gate re-scope (still pointed at t1's landed file) | 10:11:53–10:12:06 | 0m13 |
| **oracle amendment**: 2 Reads, 3 Edits, 2 discriminate runs, 1 `cp` into the worktree | 10:18:32–10:20:38 | 2m06 |
| `cp` pyproject + oracle into the worktree again, launch `harden` #1 | 10:47:02–10:47:46 | 0m44 |
| mutation red 0.600 → relay 376 survivors to the subagent | 08-12 13:57:02–13:57:36 | 0m34 |
| explain to the user what the running subagent is | 14:22:04–14:22:26 | 0m22 |
| re-verify subagent work, launch `harden` #2 | 14:51:23–14:53:04 | 1m41 |
| **`harden` babysitting** — 10 poll calls over 1h18m, incl. 3 reads of cosmic-ray's private sqlite | 15:03:05–15:23:35 | ~4m30 |
| harden PASS → `verify` → **INTEGRITY VIOLATION** → shasum → `git merge master` → `verify` PASS | 16:11:04–16:13:37 | 2m33 |
| land digest | 16:13:45–16:13:56 | 0m11 |
| Land keystone wait | 08-12 16:13:56–08-13 01:05:35 | **8h52m** |
| `land t2` + `board` + active-work update | 08-13 01:05:35–01:06:27 | 0m52 |

**t2 ≈ 17m31 hands-on + 2m32 align share ≈ 20 min of tax on 64 attended minutes (31%).** — record
numbers 662–680/1266–1347/1358–1364/1406–1544/1584–1621/1625–1685/1693–1731/1733–1824/1832–1893

**The orphan `run` row was caused by `agent sprint --help`, which the harness executed as a real
sprint.** At 08-10 08:55:25.609 the operator ran `$A sprint --help 2>&1 | head -10` to read the
usage; the harness ignored the flag, created the t2 worktree, selected `omlx`, and started
dispatching tool calls — the output the operator got back is the sprint banner, not usage. `run`
row `t2-000-1786352125711` has `started_at = 2026-08-10 08:55:25`, matching to the second. The
`| head -10` then SIGPIPE'd it, which is why `ended_at`/`iters`/`stop_reason` are NULL. The
operator diagnosed it unaided 36 seconds later — *"`sprint` has no `--help` — that invocation
actually started the sprint, and it died when the shell call returned"* — and re-dispatched at
08:56:07. — op L1304/L1307/L1310/L1313; `select started_at from run`
  → **closes agent 1's "the orphan run is the telemetry gap" as a cause question.** The row is not
  an interrupted worker; it is the CLI having no `--help` on a mutating verb.

**Per-verb usage does not exist, and the operator probed for it three times.** `agent criteria
--help` and `agent note --help` (08-10 08:53:53) each returned a single line — `Error: usage:
agent criteria <id> "<text>"` — and `agent sprint --help` dispatched work. The fallback the
operator found is a bare `agent`, which prints one 25-verb pipe-separated line and *"see the
module header for the full command list"* — i.e. read the Rust source. That line also disagrees
with the source it points at: it lists `close`, which the module header does not document, and
omits `review`, which the header does. — op L1266/L1331 outputs; `main.rs:10-29`

**Verb usage, complete: 51 invocations across 42 Bash calls; 13 of 25 verbs used, 12 never.**
`board` 13 · `sprint` 6 (one of them the `--help` accident) · `note` 5 · `harden` 6 · `criteria` 3
· `verify` 3 · `new` 2 · `draft` 2 · `validation` 2 · `align` 2 · `land` 2 · `edge` 1 · `show` 1
(+ 3 `--help` probes counted separately). 110 of the session's 304 Bash calls (36%) touched
harness machinery at all — `.harness/`, the board DB, a worktree, cosmic-ray, or the harness
source. — regex over every Bash `command`
  → **minor correction to agent 2**: the denominator is **25** verbs, not 24, and **13** were used,
  not 12. Agent 2's never-used list of 12 is exactly right; `show` (op L1088) and `edge` (op L679)
  are the two that were used and are missing from their used-count.

**Six of 42 verb-carrying calls needed a second attempt or did something unintended (14%).**
`sprint` from inside the worktree (agent 2's find) · `timeout 590 agent harden` (`timeout` is not
on macOS) · `agent harden` exceeding the Bash tool's 600s ceiling and being backgrounded · `sprint
--help` dispatching · `verify t2` firing INTEGRITY VIOLATION and needing a `git merge` before a
retry · `show t2` written defensively as `$A show t2 || $A board t2` because the operator was not
sure the verb existed. Three further Bash failures (two `dcg` guard blocks on `rm -r -f`, one
`ps -p` syntax error) were incurred *cleaning up after* the mutation runaway, not driving the CLI.

**`agent harden` reports nothing while it runs, so the operator reverse-engineered progress from
cosmic-ray's private sqlite — 25 tool calls across the two hardens.** t2: 10 poll calls between
14:53:04 and 16:11:04 (`pgrep -f "agent harden"`, `tail .harness/t2-harden2.log`, and three
`sqlite3 /var/folders/…/harness-cr-t2.sqlite "select count(*) from work_results"` reads that are
the *only* source of a percentage). t1: 15 calls, six of them the runaway kill sequence. One of
the operator's own `for i in $(seq 1 38); do … sleep 15; done` wait loops expired and produced a
task notification it then had to explain away to the user — *"That notification was my own
wait-loop expiring, not the gate."* The harness supplies no progress channel: `harden` opens no
`run` row (agent 1), prints nothing until the end, and has no `--progress`. — op
L1731/L1734/L1742/L1746/L1762/L1766/L1768/L1787/L1790/L1843; L865–L946

**Oracle authorship: three oracles, 902 lines of operator-written Python, and the cheapest one was
the one nobody revised.**

| oracle | lines | first Write | authoring span | revisions | ticket |
|---|---|---|---|---|---|
| `scripts/check_probe_discriminates.py` | 128 | 08-06 15:59:09 | 26s (15:58:48→15:59:14) | **0** | t1 |
| `scripts/check_probe_readonly.py` | 162 → **316** | 08-06 16:00:26 | 38s, then **2m06** on 08-10 | 3 Edits (`+163/−9`, `d9c8a31`) | t2 |
| `scripts/check_excel_roundtrip.py` | 458 | 08-13 14:19:16 | **9m01** (14:19:16→14:28:17) | **10 Edits**, 3 sabotage runs | *off-board* |

Direct authoring cost on the board tickets is therefore **~3m10 of session time for 444 lines** —
small, and not the real cost. The real costs are downstream and both landed on t2: the amendment
had to be committed to master, `cp`'d into the worktree twice, and still tripped `oracle_intact`
two days later, costing a further **2m33** to clear by `git merge`. **The largest oracle in the
project was written for the off-board lane, where the harness could not require it** — so oracle
authorship is `dev`-skill discipline the operator carries voluntarily, not a cost the harness
imposes. The harness's contribution is the *tamper check*, and the tamper check is what misfired.
— `wc -l scripts/check_*.py`; `git log -- scripts/`; `git show d9c8a31 --stat`; op
L656/L675/L1486–L1529/L2236–L2380

**Pre-dispatch contract authoring: 12,468 characters of hand-written workpad text across 10
writes, and both `agent draft` outputs were thrown away.** `criteria` t1 1,524 · `note` t1 1,246 ·
`criteria` t2 1,557 · `note` t2 1,333 · `note` t1 (harden-red instructions) 2,596 · `note` t1
(429 ruling) 2,052 · `criteria` t2 (AC-P1.7) 716 · `note` t2 (unblock) 1,329 · two `validation`
strings 115. `agent draft` ran twice, cost 94s + 23s of attended wall clock, and was rejected both
times in the next turn — *"Draft is weak — grep-for-string criteria a worker satisfies with stub
files"* (t1, 15:58:22) and *"a worker satisfies that by hardcoding five dicts"* (t2, 15:59:51).
Net value of the `draft` verb on this project: **negative** — it consumed 2 minutes and left
behind the defective plan agent 2 found still sitting in `ticket.plan`. — extraction over every
Bash `command`; op L641/L643/L663/L665

**Compaction cost ~5m25 of blocking time and 13 re-orientation tool calls, lost no board state,
and thinned monotonically.** Three compactions: 08-10 08:05:31→08:07:29 (1m58), 08-13
01:57:46→01:59:38 (1m52), 08-14 01:14:47→01:16:22 (1m35).

| # | summary's harness content | re-orientation | board reads after |
|---|---|---|---|
| 1 | 15,856 chars; a `harness-operator` bullet naming 12 verbs, both oracles, the worktree-nesting failure, the 0.362 red, and `HARNESS_DB` | 08:08:38→08:10:16, ~1m40 | **3** (`board`, `board --ticket t2`, `show t2`) |
| 2 | 13,590 chars; a fuller verb list, the integrity-guard rule, the cosmic-ray sqlite path, the 0.600→0.855 cycle, the INTEGRITY VIOLATION and how it was cleared | 02:02:01→02:03:04, 58s | **1** (`git log && board`) |
| 3 | 12,526 chars; harness reduced to one line (*"Rust `agent` binary; `align`/`land` are human keystones; cosmic-ray mutation gate at 0.70"*), **zero** mentions of `worktree` | 01:17:10→01:18:52, 1m42 | **0** |

The board was the thing that made re-orientation cheap: after compaction 1 the operator recovered
t2's full contract with the project's only `agent show` call, in one command. Nothing had to be
rediscovered from logs. What decayed is the operator's *model of the harness*, not the harness's
state — `board` mentions 14 → 8 → 5, `worktree` 5 → 5 → 0, board reads 3 → 1 → 0. Compaction 3
did preserve one genuine harness finding as an open flag: *"`agent board` shows only t1/t2 while
the wiki describes 11 tickets. Raised, not resolved."* One measurable pure waste: each of the
three re-orientations opened with a probe for `AGENTS.md`, which has never existed in this repo —
three identical negative lookups, one per compaction. — recs 1053/1905/2579 (`isCompactSummary`),
op L1073/L1076/L1086/L1088, L1927/L1929, L2604/L2608

**The counterfactual, in numbers.** The 08-13 off-board build is a clean comparison because it ran
in the same session, the same week, the same codebase, with the same operator.

| | board lane (t1 + t2) | off-board lane (08-13) |
|---|---|---|
| attended operator min | **145.7** (88 at the 5-min threshold) | **36.7** (identical at both thresholds) |
| calendar span | 08-06 15:54 → 08-13 01:06 = **6d 9h 12m** | 14:09:32 → 15:27:25 = **1h 18m** |
| landed | 2 modules, **8,025 lines** (`baabd0d` 3,456 + `6528be8` 4,569) | 5 modules + CLI + oracle, **7,388 lines** (`21fb973`) |
| tests | ~410 passing | **587** passing |
| lines per attended min | **55** | **201** (3.7×) |
| workers | 3 harness `claude-cli` dispatches + 1 Task subagent | 3 parallel Task subagents |
| recorded worker cost | $11.3732 (t1 only; t2's subagent unpriced) | unpriced |

Add the discarded D24 scoping — **11.9 min (5-min gap) to 33.9 min (10-min gap)** on 08-13
02:51→03:25, which produced eleven tickets and four operator keystones that never entered the
board — and the off-board lane's true cost is **49–71 min**, still 2–3× cheaper per line. — `git
show --stat`; cluster listing; `<project>/wiki/log.md` 2026-08-13

**What the board bought for that difference, stated so agent 6 can weigh it.** Four things, and
one non-thing:
1. **Two mutation-red→green cycles that materially strengthened tests.** t1 0.362 → 0.781 (cost a
   whole extra dispatch); t2 0.600 → 0.855, killing 376 survivors. The off-board 7,388 lines were
   never mutation-tested at all — D27 records that as open debt.
2. **Worktree isolation.** t1's runaway generated 19,414 mutants and a `mutants/` tree that the
   harness's own `git add -A` swept onto the branch. It stayed on `harness/t1`; master never saw
   it. The off-board build was written straight into master's working tree.
3. **Two human keystones per ticket.** `align` made the user read the contract before any dispatch
   (5m03); `land` made them look at a diff before merge (7m40 on t1, 8h52m on t2). The off-board
   lane had `ExitPlanMode` instead — approval of a *plan*, not of a diff.
4. **An audit trail.** 5 `run` rows, 10 `gate_results` rows, 39 `event` rows — against one commit
   message for the off-board build. (Agent 1's caveats on what that trail omits still stand.)
5. **Not** the oracle discipline. The off-board lane wrote the project's largest oracle, with a
   `--sabotage` mode, unprompted, with no gate requiring it. That belongs to `dev`, not the
   harness. What the harness added on top — `oracle_intact` — fired once, on the operator, and was
   cleared by changing git provenance alone (agent 2's retraction).

**Negative results — where I looked and found nothing.** (a) No `HARNESS_MAX_ITERS`,
`HARNESS_PROVIDER` or `HARNESS_CLAUDE_*` in any Bash command on t2's dispatches: the two inline
env exports the operator used (`HARNESS_CLAUDE_TIMEOUT=3600 HARNESS_CLAUDE_MAX_TURNS=150`) appear
on **four** command lines, all of them t1 `sprint` calls (op L695/L733/L752/L821), and never on
the t2 sprint or on any `harden`/`verify`/`land`. Agent 2's standing warning is discharged: I
grepped the transcript, not the config files. (b) No `agent harden` was ever run on the off-board
modules — `harden` appears 6 times, all before 08-12 14:53. (c) No `agent remember` / `recall` /
`trajectory` / `plan` / `review` call exists anywhere in the 304 Bash commands, confirming agent
2's list. (d) No fourth compaction: exactly three `isCompactSummary` records.

### Agent 4 — design defects

> **Verdict vocabulary (agent 4).** **DEFECT** = the code does not do what its own doc/decision says.
> **OVERSIGHT** = nobody decided; the behaviour is an accident of composition. **DESIGN** = a
> deliberate choice with a recorded rationale that still holds. **DESIGN-FALSIFIED** = a deliberate
> choice with a recorded rationale that this trial killed. Read-only: I ran no `agent` verb, built
> nothing, and edited only this file.

**1. `--help` on a mutating verb — DEFECT, and it is a class, not a verb.** Nothing anywhere in the
CLI rejects an unrecognised flag. `main()` checks `args[1]` against `KNOWN_VERBS` and exits 2 on a
miss (`main.rs:133-153`); past that, `arg(args, idx, usage)` accepts **any** non-empty string at a
positional index, `--help` included (`main.rs:2935-2940`), and `has_flag`/`flag`/`flags` only *look
for* names they know (`main.rs:2955-2971`). No arm ever asks "is there an argv token I do not
recognise". The intent was written down and scoped one position too narrowly: the doc comment on
`KNOWN_VERBS` says the gate exists "so a no-verb or unknown-verb invocation (`agent`, `agent
--help`) exits on the usage path" (`main.rs:130-132`), and the only test on it asserts
`!known_verb("--help")` — i.e. `--help` **at argv[1]** (`main.rs:3042`,
`crates/agent/tests/cli_dispatch.rs:64-72`). At argv[2] nothing looks.
Consequences beyond `sprint`:
  - **Fires outright on a stray flag** (no required positional): `sprint --help` (the trial's orphan
    run), `new --help` → creates a ticket **titled** `--help`, `remember --help` → mints a memory
    titled `--help`.
  - **Fires normally with the flag ignored** whenever the positionals are valid: `land t1 --help`
    lands, `rework t1 --help` hard-resets and deletes the worktree (`main.rs:209-218`), `harden t1
    --dry-run` hardens.
  - **`--flag=value` is silently dropped** — `flag()` matches `a == name` exactly
    (`main.rs:2960-2962`), so `agent run t1 --worker=claude` parses `--worker` as absent and runs
    the **local oMLX loop** instead of the delegated Claude worker (`main.rs:248-256`). Same for
    `--max=`, `--fanout=`, `--kind=`, `--priority=`, `--strategy=`, `--scope=`, `--k=`. This is the
    same root cause as the `--help` incident and is strictly worse: it fails silently and produces
    a plausible wrong run rather than an obvious one.

**2. `report_gate` erases red gates — DESIGN for the column, OVERSIGHT for the erasure.** The
`attempt` column was never meant to carry gate-retry history: `schema.rs:4-5` states its purpose in
so many words — "`gate_results` PK carries `attempt` + a `source` col → **rework invalidates prior
passes**". It is a *staleness scope*, and `gate_satisfied` reads it that way (`board.rs:258-273`).
`verify`/`harden` pass **no** attempt at all — `report_gate` reads it off the ticket
(`board.rs:238`). It is always 0 because `set_status` increments it **only** on the hop into
`Rework` (`board.rs:198`), and **no red gate causes a rework**: `run_harden` below threshold prints
and returns `Ok` (`main.rs:2251-2258`), `run_verify` on a failed validation bounces
`Verify→InProgress` (`main.rs:2118`), and the oracle violation `bail!`s without any transition
(`main.rs:2070-2077`). So every re-run lands on the same PK and the `DO UPDATE`
(`board.rs:243-244`) overwrites the failure. The red rows genuinely existed — `passed=false` is
written at `main.rs:2069` (oracle), `2111` (tests), `2239` (mutation) — and were destroyed by the
retry that fixed them.
  Two cheap, no-schema-change repairs, both already half-present: the `event` table has a free
  `note TEXT` column (`schema.rs:55-64`) which `report_gate` spends on the **gate name**
  (`board.rs:248`) while the verdict and the score string it already holds go nowhere; and
  `gate_results.note` already carries `score=0.600 (caught=… missed=376)`, so preserving the row
  costs a wider PK or an append-only sibling, not a redesign.

**3. `oracle_intact` — DESIGN-FALSIFIED, and the implementation drifted from its own decision.**
The recorded decision says the check compares **content against the base branch**: "*git already IS
the content snapshot (**base branch vs worktree**); a parallel store would duplicate it and drift.
**Comparing against the base branch is the source of truth**"* — and it explicitly **rejected**
"*store an oracle snapshot/hash in a new board column*" (`wiki/decisions.md:1164-1175`, Shakedown
fix #4, 2026-06-11). The code implements something else: `oracle_tamper_partition`
(`main.rs:2132-2150`) consumes `git::changed_files_against`, which is `git diff --name-only
{base}...HEAD` — **three-dot**, i.e. merge-base vs branch HEAD (`git.rs:204-207`). Three-dot is
correct and deliberate for `harden`'s mutation scope ("the letter describes what the BRANCH did,
never what base advanced past it", `git.rs:209-216`) and wrong here, because it makes the check a
function of *where the merge base sits* rather than of *what the file says*. `git merge master`
moves the merge base forward; the edit leaves the diff; the gate passes with no byte changed. That
is exactly the 14-second clearance agent 2 documented.
  **The property it should check** is content provenance, not diff membership: *the worktree's copy
  of a protected oracle must be byte-identical to that path's content at some commit reachable from
  the base branch.* Cheapest form — compare the worktree blob against **both** the base-branch tip
  and the merge base; identical to either → PASS. That passes the operator's amendment the moment
  it is on master (no `git merge` ceremony, no misfire), still fails a worker that writes content
  no human committed, and needs no new board column, so it survives the Rejected-Approaches ruling.
  **On the align-2026-07-14 trigger** (`worker.rs:9-15`, "the worktree boundary is prompt-level
  here — accepted at align 2026-07-14, **revisit on an observed out-of-tree write**"): the trigger
  did **not** fire. Zero out-of-tree writes by any of the four workers (agent 2). "Observed git
  write" should **not** be folded into it either — the case this trial produced is orthogonal: an
  **in-tree write by the operator**, which no worker jail would have caught and no prompt rule
  forbids. Leave the 07-14 ruling standing; the fix belongs in the gate's predicate, item 3 above.
  **Secondary, and load-bearing for how much any of this matters:** `GATE_ORACLE_INTACT` appears
  nowhere in `required_gates_for` (`spine.rs:53-75`) — decisions.md:1169 says so on purpose
  ("evidence-only (not a transition gate)"). The green row has no authority over any transition;
  `Review→Land` requires nothing at all. The entire enforcement is the in-process `bail!` at
  `main.rs:2070`, which means an operator who simply does not run `agent verify` again is never
  stopped by it.

**4. `commit_worktree`'s `git add -A` — DEFENSIBLE CORE, MISSING GUARD.** The sweep is not gratuitous:
the worker is told *"Do NOT commit, branch, push… Leave your changes uncommitted in the working
tree"* (`worker.rs:114-115`), so the harness must capture everything or lose work — and a narrower
`add` would drop worker output **silently**, which is a worse failure than sweeping too much. Five
live call sites (`main.rs:957, 1090, 1162, 2180, 2863`; `git.rs:289-297`). The author already knew
the hazard and solved it for the harness's **own** artifacts by writing them under a gitignored
`.harness/` — the comment says exactly why: "*under `.harness/` so it is never staged by
`commit_worktree` (`git add -A` skips ignored paths)*" (`main.rs:772-779`). That idea was never
extended to the **worker's** tool scratch, and `ensure_worktree` writes no per-worktree exclude
(`git.rs:120-151`). Cost to narrow, two options, both small:
  (a) seed the worktree's `.git/info/exclude` at `ensure_worktree` with the mutation/venv/cache
      scratch names — ~10 lines, invisible to the project's tracked files, and it kills the
      19,414-mutant sweep at source;
  (b) a staged-count/byte guard inside `commit_worktree` that refuses and prints the top offending
      directories above a threshold — catches the same case *and* every future one, without
      needing to enumerate tool names.
  **It would not have fixed t2.** The operator's `cp` of the oracle into the worktree was
  intentional and had to be swept; what turned that into a violation is item 3, not this.

**5. `[wt] no changes produced` — DEFECT at the print site, and a structural blind spot underneath.**
The label is available and thrown away: `run_worktree` calls `run_ticket(...).await?` and **discards
the `RunSummary`** (`main.rs:955`), then prints a cause-free line (`main.rs:960`), while
`RunSummary.stop` carried `max_iters` and `run_sprint` reads the same value back off the board row
and does print it (`main.rs:1534-1539`). So the sprint summary for t2 did say `run=max_iters …
worker did not complete — parked`; the `[wt]` line beneath it did not.
  **Can the harness distinguish "model failed" from "contract wrong" at all? No — and the reason is
  structural.** The one party that knows is the worker, and it has no channel. The native loop
  offers exactly three tools — `read_file`, `write_file`, `bash` (`tools.rs:25/41/73`) — with no way
  to raise a confusion; the delegated CC worker gets prose advice only (*"If you are genuinely
  blocked, say so plainly in your final message"*, `worker.rs:116-117`), which nothing parses. Yet
  the harness **has** the vocabulary: the `Confusions→Align` back-edge is in the spine
  (`spine.rs:17`) and `raise_confusion` bounces an `in_progress` ticket back to the human gate
  (`main.rs:1932-1942`) — it is simply an **operator-only verb**, and the operator was not in the
  loop. Three unused signals sit at the same point: the tool-call composition (t2: 12 results, zero
  writes), the recorded reasoning (item 7), and `git::is_dirty` — which the loop computes **only**
  in the natural-stop branch (`main.rs:606-648`). That is the deeper blind spot: `planexec`,
  research/24's designated "described work instead of doing it" detector, never runs on a turn that
  called a tool, so a run that tool-calls to exhaustion — precisely t2 — is invisible to it.
  research/24:194 anticipated the mode ("the 35B fails via action-without-progress — run 1 →
  `max_iters` (read-wander)") and assigned it to `max_iters`, which *terminates* it and *diagnoses*
  nothing.

**6. `DEFAULT_MAX_ITERS = 8` — OVERSIGHT, with a paper trail. Not "never measured": judged tiny,
and then fixed in the wrong place.** research/21 §1 (2026-06-10) names it in the same sentence as
the token cap — "*Defaults are tiny: `DEFAULT_MAX_TOKENS=1024`, `DEFAULT_MAX_ITERS=8`*" — and the
resolution is explicit: "*Budget bump (env-only, zero code): `HARNESS_MAX_TOKENS=12288`,
`HARNESS_MAX_ITERS=40`*" (research/21 §3c and §resolution item 1). decisions.md records the same:
"*Budget bumped **env-only** (`HARNESS_MAX_TOKENS`/`HARNESS_MAX_ITERS`)*" (`wiki/decisions.md:1012`).
The **token** half was later promoted to a compiled default when a measurement demanded it —
1024→4096, with the explore-shakedown-#4 citation still in the doc comment (`main.rs:62-71`). The
**iters** half never was. So every evaluation since June ran at 40 by env export, the compiled
default stayed at the number its own research called tiny, and an operator who does not know the
env var exists gets 8. **8 is not defensible for any real ticket**: t2's sprint spent all 8 turns on
orientation reads (12 tool results, zero writes) and reached the right conclusion on the last one.
The failure was not the number alone — the plan was also wrong (agent 2) — but 8 guaranteed no
recovery from either. → **closes the `## Open` item** "was the 8-iteration budget ever measured, or
inherited?": inherited from a decision that deliberately deferred it to an env var nobody set.

**7. `refeed` and the reasoning — DESIGN, and this trial did not falsify it. Retracting the
framing.** "Re-feeding reasoning / CoT into context" is a named **Rejected Approach** with three
independent external corroborations (`wiki/decisions.md` §Rejected Approaches; research/25 §4.3:
Hermes `keep_cots=False`, OpenCode same-provider-only, Claude Code's `clear_thinking`), reconciled
against bounded thinking on two axes — *think this turn, don't re-feed next turn*. Two corrections
to the standing `## Working` item:
  - **The reasoning was not discarded.** `rec.record_reasoning` writes every block to the trajectory
    as a `reasoning`-role line (`recorder.rs:82-92`), and the printed line says so literally —
    `[reason] N bytes (recorded, **not re-fed**)` (`main.rs:601-604`). The 926- and 5,250-byte
    blocks are on disk in `.harness/runs/t2/t2-000-1786352167614.jsonl`, and `agent trajectory t2
    --full` prints them whole, byte counts and all (`main.rs:889-903`).
  - **Re-feed was causally irrelevant to that loss.** The loop ended *on* the turn that produced the
    5,250-byte block (iteration 8 of 8); there was no next turn for it to be re-fed into. The
    posture is not what cost anything here.
  The real defect in the neighbourhood is narrower and belongs to item 5: a `max_iters` exit
  surfaces nothing — not the stop label, not the last reasoning block, not a pointer to the
  trajectory that holds both.

**8. Mid-ticket criterion amendment — the sharpest defect in my slice, and the board already has the
parts.** `board.rs:111` states the invariant in its doc comment: "*`criteria_confirmed` confirms
***these***.*" Nothing enforces it. `set_acceptance_criteria` is a plain
`set_workpad_field` (`board.rs:112-114` → `163-176`): it writes a `workpad_edited` event and touches
**neither status nor gates**, at any status. So at 08-10 08:55 the operator added AC-P1.7 to a
ticket that had been `in_progress` since 08-06 16:06, and t2's human `criteria_confirmed` row still
reads `passed=1, created_at=2026-08-06 16:06:17` — certifying a text that no longer exists. The
board keeps **no copy** of what was certified (`align` calls `report_gate` with `note: None`,
`main.rs:386`) and **no before/after** (the event carries only the column name).
  **A correct mechanism, with no schema change:**
  (a) record a digest of the criteria text in the `criteria_confirmed` gate note — the column
      already exists and is already used this way by every machine gate;
  (b) make `set_acceptance_criteria` on a post-Align ticket take the **existing** `InProgress→Align`
      back-edge (`spine.rs:17`) exactly as `raise_confusion` already does (`main.rs:1932-1942`).
  The amendment then costs one human re-align and leaves a *second* `criteria_confirmed` row as the
  audit artifact. That is what the trial actually wanted: not a prohibition — the narrowing was
  right and measurement-driven — but a record proving a human re-confirmed the narrowed criterion.
  **The asymmetry is the finding.** "Never modify success criteria" is enforced by code against the
  **worker** (`oracle_intact`, and even that only as an in-process bail — item 3) and **not at all**
  against the **operator**, who is the only party with a `criteria` verb.

**9. Plan/criteria coherence at Align — STRUCTURAL, by design, and the design assumed a human who
re-reads.** `align` checks exactly one artifact: acceptance criteria non-empty
(`main.rs:374-376`). `required_gates_for(Align, InProgress)` is `[criteria_confirmed]` and nothing
else (`spine.rs:56`). `plan`, `acceptance_criteria` and `validation` are three independent `TEXT`
columns (`schema.rs:24-26`) with three independent setters and no cross-check anywhere in the
codebase. There is no *coherence* concept in the schema, the spine or the CLI — `draft` writes all
three as a coherent set, the operator may overwrite any subset, and nothing notices the set stopped
being one. The recorded design puts that job on the human ("*Align stays the human back-and-forth*",
`wiki/decisions.md:1219-1220`), and the human read the criteria, rewrote them, and did not re-read
the plan. Cheapest honest fix: have `agent align` print the full rendered workpad — which
`board::render` already produces for `agent show` (`main.rs:219-223`) — before asking for the
keystone, so the human confirms the *contract*, not the field they just typed.
  **Bonus falsification.** `agent draft`'s acceptance bar is recorded as "*Align would be one-round
  (drop a hallucinated `assignee` field), not a rewrite*", at calibration **n=1**
  (`wiki/decisions.md:1246-1249`). cv-mapper is n=2 and n=3, and both were rewrites, in the next
  turn, out loud. First real-project falsification of that bar.

**10. The untouched half of the CLI — one to keep, one parked on purpose, one undiscoverable, one
structurally dead.**
  - **`explore` — not undiscoverable; unusable on this project by construction.** `run_explore`
    hardcodes `OpenAiProvider::omlx()` (`main.rs:1155`) and the compiled `MODEL` const
    (`main.rs:1161`); there is no `--worker claude` arm, unlike `run` and `sprint`. Each of its 2–5
    workers is a `run_ticket` at `DEFAULT_MAX_ITERS = 8`. Pointed at t2 it would have produced 2–5
    copies of the failure the sprint produced once. **Keep the code; it is gated on the local loop
    becoming able to finish a real ticket, not on discoverability.**
  - **`review` — deliberately parked, not neglected.** The automatic sprint call was **deleted** on
    a pre-pinned rule (0 true positives / 0 false VIOLATES across 5 real tickets), with extending
    the window explicitly rejected as moving criteria to fit a hoped-for result; the verb was
    retained for manual use (`wiki/decisions.md:1447-1458`). Its silence on cv-mapper is a decision
    being honoured. Zero runtime cost. **Not dead weight.**
  - **`trajectory` — working machinery, undiscoverable, and the highest-value unused verb in the
    trial.** It prints the latest run row (provider/model/iters/stop/started) **and** the recorded
    trajectory; `--full` prints every message whole with byte counts and raw tool-call payloads
    (`main.rs:864-909`). One command answers three of the eight visibility complaints agent 2
    tabulated *and* the "why did the sprint produce nothing" question the operator instead answered
    with four greps into the harness source. It is listed once in a 25-verb pipe-separated line that
    points the reader at the Rust source, and has no `--help` (item 1). **Fix the discovery, not the
    verb.**
  - **`remember`/`recall` — a read path with no writer on the mainline.** `recall_primed` is
    auto-injected into every worker prompt at three call sites (`main.rs:480`, `1013`, `1405`). The
    **only** automatic writer in the whole harness is `run_explore`'s reflection-on-kill
    (`main.rs:1183-1203`), which fires only for an `explore` worker that failed validation. `run`,
    `sprint`, `harden`, `verify`, `land` and `close` write **no memory at all**. On any project
    driven the normal way the store is empty by construction and the injection is a no-op — which
    is exactly cv-mapper's 0 rows. This is *half*-known: the "Auto-distilled self-evolution before
    a stocked pond" rejection records "*there were **zero** general lessons to distill (the only
    writer emits bound-specific post-mortems)*" (`wiki/decisions.md` §Rejected Approaches) — it
    names the empty pond but blames distillation quality, not the mainline path having no writer.
    Cheapest fix: write a `lesson` on each red gate; the harness already has the note text in hand
    at all three call sites (`main.rs:2069`, `2111`, `2239`).

**Why `agent board` never surfaced the eleven-ticket gap — NEGATIVE RESULT: no verb would have.**
`board` prints `all_tickets()` and per-status counts (`main.rs:239-245`); `ready` prints `ready()`.
Nothing in `KNOWN_VERBS` (`main.rs:133-137`) imports, syncs, reconciles or lints, and no code path
anywhere in `crates/` reads a `wiki/` file — the only external inputs are the DB at `HARNESS_DB`,
git, and the provider. The eleven tickets of D24 were **never created**, so no query could return
them; they existed only in `<project>/wiki/log.md`. The board has no notion of intended-but-uncreated
work, and the compaction-3 flag the operator raised (*"`agent board` shows only t1/t2 while the wiki
describes 11 tickets"*) is a true statement about two disconnected stores, not a read-path failure.

### Agent 5 — catches and misses

> **Method (agent 5).** Read-only: I ran no `agent` verb, no test suite and no oracle. The red
> states are recoverable because **both deleted branches survive as dangling git objects** —
> `git fsck --lost-found` returns `696d306` (tip of `harness/t1`) and `8516ee6` (tip of
> `harness/t2`), and their ancestry carries every `"{id}: work in progress"` commit. Every
> red-vs-green diff below is `git diff <scored-tree> <scored-tree>` against those objects.
> They are unreferenced and one `git gc` from gone.

**The refusal ledger: five refusals, three true catches, one false positive, one non-refusal.**

| # | when (UTC) | what refused | verdict |
|---|---|---|---|
| R1 | 08-06 16:26:42 | t1 `mutation_score` **0.362 < 0.70**, 462 survivors | **TRUE CATCH** |
| R2 | 08-07 00:33:48 | t1 `harden` refuses to score at all: *"cosmic-ray baseline failed — the test suite isn't green on un-mutated code, so a mutation score would be meaningless"* | **TRUE CATCH** (precondition) |
| R3 | 08-10 11:22:59 | t2 `mutation_score` **0.600 < 0.70**, 376 survivors | **TRUE CATCH** |
| R4 | 08-12 16:11:18 | t2 `oracle_intact` **INTEGRITY VIOLATION** | **FALSE POSITIVE** (agent 2) |
| — | 08-07 11:03:27 | t1 `harden` #3 ran 4h25m on 19,414 mutants and was killed | not a refusal — a runaway |

`tests_green` never refused: it passed first try on both tickets (t1 `247 p…`, t2 green at
16:12:38). No `verify` bounce `Verify→InProgress` ever occurred. — `gate_results`, `event` 21–41,
`.harness/t2-*.log`, op L707/L713/L784/L793

**R1 is a true catch, and the defect at the end of its chain is a silent-wrong-answer bug in
landed-critical code.** The chain, each link evidenced: red 0.362 (op L707, `KILLED|262
SURVIVED|462` from `harness-cr-t1.sqlite`) → the operator refuses rework and writes an additive
note naming the untested helpers (`node_path`, `_find_chrome`, `wait_for_port`, `version_string`,
`main`, `download_dir_writable`) → dispatch 2 writes 1,456 lines of helper tests for exactly those
(op L783/L787) → **the new tests fail against the source**, which is what R2 then refuses on → the
operator adjudicates all three failures (op L809–L812): one test unsound (asserts case-sensitivity
through a case-insensitive filesystem — *"my instruction backfiring"*), one arithmetic to re-check,
and **one real code defect**: `_major_version("Chromium 120.0.6099.109")` returned `120`.
Ruling: *"reject Chromium, fix the code, keep the test… a confident version number for the wrong
browser is precisely the silent-wrong-answer failure this ticket exists to prevent."* The fix is in
the landed tree — `_NOT_GOOGLE_CHROME` and an early `return None`, the **only** source change in
`git diff 57ddf5d 696d306 -- src/` (18 lines). The other two failures were resolved by changing
tests, not source. So R1 → R2 → one real defect, two test defects.

**R2 is the cheapest catch in the trial and the least visible.** Refusing to compute a mutation
score on a red suite is exactly right — the number would have been meaningless — and it is the
only refusal that **left no row anywhere**: no `gate_reported` event (the gap between event 24 at
00:34:51 and event 25 at 11:03:27), no `gate_results` row, and no `.harness/` log, because t1's
gates were run in the foreground. It survives only in the operator transcript.

**R3 is a true catch and bought the most.** `git diff 414f0d2 2fe366c` (the tree scored 0.600 vs
the tree scored 0.855): **+1,304 test lines, 92 new tests, and a 14-line source change.** What the
survivors were, from the worker's own clustering of the 376: `format_summary` 69 (*"nothing
asserted on the text a human actually reads"*), `probe_profile` bookkeeping 37, `_outer_html`
truncation 32 (branch never taken), `open_more_menu`/`close_menu` 32, **`main()` +
`connect_over_cdp` 20 — "No CLI test existed at all"**, `_handles` count-fallback 13, and 77
equivalent. The source change is a real fix, found while writing the test: *"Found a real wording
bug while writing the cap test — the downgrade note overcounts opened candidates"* — the probe's
`not_found` note claimed `len(rejected) + 1` candidates had been opened when the attempt cap had
stopped it one short, i.e. a probe whose entire deliverable is evidence was reporting an untrue
count. — subagent L173/L210/L211/L440; `git diff 414f0d2 2fe366c`

**R4 stands as agent 2 retracted it: a diff detector firing on the operator's own approved,
already-on-master amendment, cleared 34s later by `git merge` with no byte changed.** Adding one
thing for the ledger: it is the only refusal of the four that produced **no** change to any
artifact — the remedy it demanded (*"Revert the file(s)… and make the implementation satisfy the
original oracle"*) was correctly ignored.

**Catches: 3 of 4. All three true catches ended in a change that a reviewer would want.**

---

**MISSES: zero found. Here is exactly what I searched, and why zero is a weak number here.**

1. **Git — decisive and negative.** No commit after `baabd0d` (t1 land, 08-07) touches
   `src/cv_mapper/probe/windows_deploy.py`, `tests/test_windows_deploy*.py` or `run_probe.bat`.
   No commit after `6528be8` (t2 land, 08-13) touches `linkedin_selectors.py` or its tests. The
   repo has 18 commits total; the only post-t1-land change to any t1/t2 artifact is `d9c8a31`, the
   operator's own oracle amendment, which predates t2's land. **No landed line was ever corrected.**
   — `git log -- src/cv_mapper/probe/ scripts/ tests/`
2. **The project wiki's decisions — checked each one that postdates a land.** D2a/D2b/D2c
   (08-10) are *pre*-t2-land and are not corrections to landed code: D2b/D2c are DOM findings that
   changed t2's contract mid-ticket (agent 4 item 8), and D2a is a LinkedIn-behaviour finding that
   forces a two-phase onboarding design while leaving t1's five checks intact. D22 (08-13,
   post-t2-land) freezes the probe and copies ~20 helpers into the product rather than importing
   them — a deliberate duplication, not a defect. D14/D14a/D15/D16 concern the workbook, not t1/t2.
3. **The strongest positive evidence, and it is real-world.** t1's landed artifact was packaged
   and **executed in its target environment three days after land**: zipped 08-10 07:55 UTC with an
   embeddable Python and Playwright's `node.exe`, run by the recruiter on her own Windows laptop,
   result pasted back at 08:03:48 — **all five checks `pass`, overall `OK`**, CDP attached on port
   61960 read from `DevToolsActivePort`, `node.exe` v20.18.0. **No patch to the landed code was
   needed**; the operator's only pre-flight was a read-only check that the 13 imports are stdlib.
   A landed probe used for its actual purpose, first try, by a non-technical user, with no defect
   surfaced. — op L976–L1041; `abb14b0`
4. **Both landed modules are still green.** 08-14 01:18 UTC on master: `587 passed`, Excel oracle
   `PASS`. That is 7 days after t1's land and 1 day after t2's. — op L2618/L2620/L2616
5. **Transcript sweep.** Regex for `regression|broke|broken|defect|wrong answer|should have
   caught|slipped through|missed by the gate|had to fix|latent` over every assistant text record
   after 08-07 15:54 UTC: 11 hits, none of them a post-land defect in t1 or t2 code. Separate
   regex for the module names (`windows_deploy|linkedin_selectors|run_probe.bat|_major_version|
   check_probe_discriminates`) over the same window: every hit is packaging, describing, or
   re-scoping — none is a defect report.

**Why 0 is weak, stated so it is not over-quoted.** The observation window is thin in a specific
way: **t2's landed module has never been executed since it landed.** Its three live runs are all
on 08-10, two days before the re-gate and three before the land, and D22 then froze it as evidence
rather than growing it into the product. t1 has exactly one post-land real execution. **What would
settle it:** running `linkedin_selectors.py` against fresh profiles after 08-13 (LinkedIn ships DOM
changes continuously — D2b says so), and re-running the recruiter probe on a second machine.
Neither has happened. **0 misses is "nothing found in a narrow window", not "nothing there".**

**Two limitations were disclosed to the human *before* the land keystone, so they are not misses.**
t1's digest: without the shipped `node.exe`, `check_node_exe_executable` returns *"`unknown`, not
`pass` — which means the D13 packaging question goes unanswered rather than getting a clean bill of
health"* (op L968). t2's digest: *"the probe's More-button disambiguation is **not fully
reliable**. Run 3 abstained on one profile where a video player's menu collided with the profile
menu"* (op L1868). Both digests also carried what the ticket bought. **One limitation surfaced
after t1's land and belongs to its conclusion, not its code:** D23 (08-13) records that P2 was
measured against the Python 3.11.9 embeddable, not a PyInstaller bundle, so its antivirus finding
does not transfer to the packaged product. No machine gate in this harness can catch that class —
mutation, tests and the oracle all check code; nothing checks whether the measured configuration is
the shipping one. The only gate that could is the human contract at Align, and t1's criteria never
named a packaging configuration.

---

**Did the mutation gate buy anything real? Yes on both tickets, and it was not gamed.**

- **The score describes what shipped.** The tree that scored 0.855 is byte-identical to the tree
  that landed: `git diff 2fe366c 8516ee6 -- src/ tests/` is empty and so is `git diff 8516ee6
  6528be8` on both landed files. Same for t1: `git diff 696d306 baabd0d -- src/ tests/` is empty.
  Nothing in the harness enforces this (`land` re-checks nothing — see below); on cv-mapper it held
  by timing.
- **The new tests are behaviour-shaped, not mutant-shaped.** t2's 92 tests pin, in the worker's own
  itemisation: a subtitles menu rejected; `wrong_menu` vs `no_pdf_option` driven from two different
  fixtures with an assertion that they never collapse; the escalation cap stopping short of a
  reachable candidate; an unverified candidate never named; **Escape pressed once per opened menu**;
  **a dead click *not* retried**; `disambiguated_by` proven absent when nothing was excluded; and a
  CLI suite covering `contexts[0]` / `pages[0]` — D2's cookie-less-context trap — on a surface that
  had **zero** tests before the gate fired.
- **The one available way to game the number was found and refused.** 77 of the 376 survivors are
  `ReplaceBinaryOperator_BitOr_*` on `str | None` annotations, unkillable under `from __future__
  import annotations` (PEP 563), capping the achievable score at ~0.918. The worker named them,
  computed the ceiling, and wrote: *"I did not rewrite them to `Optional[...]` to game the count."*
- **It verified its own kills and caught its own false result.** 101 mutants applied one at a time,
  101 killed; then it found one was an artifact — `==` → `<=` is byte-identical in length, the
  restore landed in the same filesystem second, and Python reused the mutant's `.pyc` — and re-ran
  all 101 with `PYTHONDONTWRITEBYTECODE=1`. Two mutants it had first called equivalent turned out to
  be its **fake** lying (interned string literals; a container whose `is_visible()` raises), and
  fixing the fake killed them. — subagent L414/L435/L440
- **Fair counterweight.** The last three assertions were written from a list of known survivors
  (`final_kills.py`, L331/L381) — mutant-directed test-writing, which is what the gate asks for.
  And a large fraction of both survivor populations was noise: 77 of 376 on t2 were provably
  equivalent, and t1's 462 were dominated by `NumberReplacer` (68) and six `ReplaceBinaryOperator_
  Div_*` families at 16 each (op L714). The gate's signal-to-noise on this codebase is roughly
  4:1 by survivor count.
- **Verdict for agent 6: the two red→green cycles are the harness's best evidence, not its most
  expensive ceremony** — but the evidence is not on the board. Both red rows were erased by
  `report_gate`'s `DO UPDATE` (agent 4 item 2), and I could only reconstruct them from
  `.harness/*.log` plus two **unreferenced** git objects.

---

**The off-board lane's exposure — graded, and its own discipline credited.**

I ran nothing, so this is exposure, not asserted defects. Graded by how much acceptance evidence
each module carries, strongest first:

| module | lines | tests | oracle | exposure |
|---|---|---|---|---|
| `excel/{parts,schema,write}.py` | 1,464 | 1,563 lines | `check_excel_roundtrip.py` (458 lines) drives `append_rows` against a copy of the **real workbook**, verified to fail against a hand-written naive writer with 3 named findings | **lowest** — more acceptance evidence than either board ticket |
| `ingest.py` | 896 | 1,094 lines | none, but exercised on **19 real CVs**, 19/19 | low |
| `llm.py` | 1,030 | 632 lines, **all against a fake** | none | high — the wire contract has never met the real API (403 on all 7 models, D26) |
| `cli.py` | 248 | **zero** — imported by no test file and by no `check_*.py` | none | **highest** |

`cli.py` is the specific place `agent harden` would most plausibly have found something, and t2 is
the base rate for saying so: the identical shape there — `main()` + `connect_over_cdp`, *"No CLI
test existed at all"* — produced **20 surviving mutants**, and the CLI tests the gate forced pinned
exactly the trap that mattered. `cli.py` is also the layer whose `add` verb appends to the
recruiter's real workbook, and `map`/`add` have **never executed** (403); only `inspect` and `read`
have been run, manually, once, on 08-14. — `grep -rn "cv_mapper.cli" tests/ scripts/` → no hits

**The off-board lane's own discipline did catch things, and one of them is the mutation gate
applied voluntarily.** (a) The oracle was written **before** the writer, against `parts.py` alone,
then run against a deliberately naive implementation and shown to fail — and it **caught two bugs
in itself before catching any in the writer** (scanning columns B–F when the name column is J;
asserting on shared-formula representation when openpyxl legitimately re-authors them). (b) The
`ingest` builder found and fixed a real bug — two `.docx` CVs were 98.9% and 98.2% base64 image
payload, 276,973 → 3,025 chars and 176,610 → 3,095 (D25) — **then hand-ran mutation testing on its
own fix**, discovered that deleting the image handler entirely still passed because the sweep
produced a byte-identical `Source`, and restructured `_docx_to_html` to return `(html, dropped)` so
a test pins the primary mechanism: *"All six mutations against the new code now die."* That is the
discipline the gate exists to force, applied with no gate present. — `<project>/wiki/log.md`
§"Writing the oracle before the writer was the right order"; ingest subagent final report

---

**What actually stood between the worker's diff and master — real barriers, not nominal ones.**

Code-enforced, in order:
1. **`Verify→Review` requires BOTH `tests_green` AND `mutation_score`** for a code kind —
   `required_gates_for` returns `vec![GATE_TESTS_GREEN, GATE_MUTATION]` when `kind_is_code(kind)`,
   and `"build"` is a code kind (`crates/board/src/spine.rs:63-66`, `:82-84`). **This is the one
   machine barrier with teeth in this trial, and it fired three times.** (Complements agent 4 item
   3 rather than contradicting it: it is `oracle_intact` that gates no transition, not the mutation
   gate.)
2. **`run_verify` executes the validation command**; a non-zero exit bounces `Verify→InProgress`.
3. **The in-process `bail!` on `oracle_intact`** (`main.rs:2070`) — blocking in practice as long as
   `verify` is re-run, worth nothing on the board.

Not barriers:
4. **`Review→Land` requires nothing, and `run_land` re-checks nothing.** It asserts only
   `status == review`, squash-merges, and then **writes the human `landed` gate row itself**
   (`main.rs:2857-2871`). The "human keystone" is the operator choosing to type the verb.
5. **Nothing re-checks that the scored tree is the landed tree.** Here it was, by timing.
6. **Nothing re-checks that the criteria confirmed at Align are the criteria in force** — t2's
   AC-P1.7 was added four days after its `criteria_confirmed` row (agent 4 item 8).
7. `commit_worktree`'s `git add -A` is an anti-barrier (agent 2, agent 4 item 4).

**The barrier that did the most work in this trial was prose.** Both land digests were honest and
carried the known limitations before the human decided (op L968, L1868). That is `dev`-skill
discipline in the operator, not a harness gate — the same shape agent 3 found for oracle authorship.

---

**The one open product question is unchanged, and agent 6 can score the trial anyway.** Nothing
since 08-14 moved extraction accuracy. Verified end-state: the working tree is **clean** at
`6bb1548` (08-13 15:27 UTC), so no code or wiki has changed since the build entry was written. At
08-14 01:17–01:18 UTC the operator re-ran everything from scratch rather than reading the wiki:
`587 passed` in 16.6s, Excel oracle `PASS` against the real tracker, `inspect` reads her workbook
(26 columns, 10 dropdowns, next free row 110), `read` does 19/19 CVs — and a **live** call to
`qwen3.5-flash` with the real 116-char key returns `403 AccessDenied.Unpurchased`. The session's
last substantive record is 08-14 01:25 (*"what's entitlement?"*); the only 08-16 record is a bare
`system` line. So **D21's five thresholds have still never been measured against**, the blocker is
an Alibaba Cloud console action rather than code, and it is orthogonal to the trial: it sits
entirely inside the off-board lane's `llm.py`, and neither board ticket depended on it. The trial
is scoreable; the *product* is not yet answerable. — op L2610–L2633/L2638; `git status --short`
(empty); `wiki/active-work.md` mtime 08-13 22:26 +0700

### Agent 6 — the ledger row and the verdict

> **Scope (agent 6).** Read-only everywhere except this file. I ran no `agent` verb, committed
> nothing, and did not touch `trial-ledger.md`, `active-work.md`, `case-law.md` or `decisions.md`.
> Two facts below I verified at source rather than inheriting: `created_at` is read **nowhere** in
> `crates/` outside the DDL (`grep -rn created_at crates/` → schema only), and `start_run` has
> exactly two call sites, `main.rs:469` (native loop) and `main.rs:1024` (delegated worker) — so
> `harden`/`verify`/`land` opening no run row is confirmed independently of agents 1 and 3.

#### 1. The state of the gate cv-mapper was supposed to test

**The gate was already closed before cv-mapper started, on a different stream, and cv-mapper is
under-powered against the only threshold anyone ever pinned.** Three documents, in order:

- The sentence this trial is scored against — *"Next gate for the daily-driver claim: a real
  (non-kata) multi-ticket project"* — is a **2026-07-14 breadcrumb** (`active-work.md:126`), i.e.
  a log entry, not a live gate.
- It was closed on **2026-08-02**: *"**GRADUATED (A6, Gary, 2026-08-02): the harness is the daily
  driver.** Trial window 5/5 + B-slice landed"* (`active-work.md:12`), with the decision record at
  `decisions.md:1492-1505` — 5 real PDSI tickets, 0 known misses, 0 faked rows, $25.57 recorded.
  PDSI continuation is real client work, not a kata, so the 07-14 gate was satisfied by it.
- The only **pinned** numeric criterion is `trial-ledger.md:13-14`: *"Daily-driver evidence target:
  ≥5 real tickets with complete rows. The graduation judgment on the numbers is Gary's, not this
  document's."* cv-mapper yields **2** tickets, in a different stream, and **one of them cannot
  produce a complete row** (§2, t2).

`CLAUDE.md`'s Current-focus line still names the 07-14 gate as open; it was never updated after A6.
That is a stale pointer in the project instructions, and it is the whole reason this trial reads as
a gate run. **Consequence, stated plainly and not adjusted in either direction:** cv-mapper cannot
pass or fail the daily-driver gate as written — it is 2 tickets against a ≥5 bar that was already
met elsewhere. **No threshold for a cv-mapper-shaped run was ever stated, and I am not inventing
one.** What cv-mapper *is* is the first post-graduation project on an outside codebase, so the
useful question is falsification: **does it break what A6 already ruled?** It does not. It narrows
it, in one specific place (§3b).

#### 2. Drafted ledger rows — NOT appended to `trial-ledger.md`

Pinned schema, unmodified: `ticket | kind | title | operator-min | worker $ / turns / attempts |
catches | misses | bounces | reviewer`. Where cv-mapper does not fit a column I say so **in the
cell**. Two comparability warnings that belong above the rows, not inside them:

- **Different stream.** Every existing row is PDSI continuation on
  `~/Documents/Work/FPT/Accelerator/harness-run/` (`trial-ledger.md:3-4`); cv-mapper is a different
  codebase, a different domain (browser automation vs a multi-service stack), and ran **after** the
  window closed. The rows are appendable as a *second window*, not as t11/t12 of the first. Mixing
  them into one tally would restate a closed graduation on evidence it did not use.
- **Different measurement method.** PDSI `operator-min` figures are contemporaneous honest
  estimates written at land time. cv-mapper's are a post-hoc reconstruction from the session
  transcript (agent 3's clustering). They are not the same instrument reading, and the column
  definition — *"honest wall-clock estimate of ALL operator work"* — names no counting rule, so
  neither is wrong.

| ticket | kind | title | operator-min | worker $ / turns / attempts | catches | misses | bounces | reviewer |
|---|---|---|---|---|---|---|---|---|
| **t1** (cv-mapper — different stream) | build | Windows deployment probe: 5-check CDP/Playwright preflight + `run_probe.bat` (landed `baabd0d`) | **~23** (13 hands-on: contract authoring, 128-line oracle, 429 adjudication, runaway kill; 10 keystone interaction). *Column pins no counting rule; this is attended-session clustering at a 10-min gap — the same session reads 88 min at 5-min and 392 at 20-min* | **$11.3732 / 124 / 3 dispatches** (completed 33t · 429-error 20t · completed 71t), all `claude-cli` on `claude-opus-5[1m]`. *Cost is not board state — it survives only in the teed CC `stream-json`; `run` has no cost column* | **2 TRUE.** mutation **0.362 < 0.70** (462 survivors) → additive note → 1,456 test lines → 3 failures adjudicated → **1 real silent-wrong-answer bug fixed** (`_major_version("Chromium 120…")` returned `120`; `_NOT_GOOGLE_CHROME` + early `return None` — the only source change on the branch). Then harden **refused to score at all** on a red baseline (*"a mutation score would be meaningless"*) — correct, and left **no row anywhere**. Final 0.781; `tests_green` first try | **0 found / thin window.** No commit after `baabd0d` touches any t1 artifact. Landed probe was zipped and **executed in its target environment by the recruiter 3 days post-land — 5/5 checks pass, no patch**. D23 (measured against the embeddable, not a PyInstaller bundle) is an Align-contract gap, not a gate miss | **3** — *rate-limit* (Anthropic session limit; $2.74/20t, work preserved by the harness WIP commit `233211b`); *mutation-red* (1 additive re-dispatch); *harness-caused runaway* (4h25m, 19,414 mutants: `commit_worktree`'s `git add -A` swept the worker's tracked `mutants/` tree onto the branch — predicted by the worker, filed as cosmetic). Plus 1 dispatch attempt that opened no `run` row (shell inside the worktree) | **n/a** — the automatic advisory call was DELETED 2026-08-02 on the pinned fix-or-drop rule; `agent review` not invoked. A decision being honoured, not a step skipped |
| **t2** (cv-mapper — **worker not harness-dispatched**) | build | LinkedIn selector resolution: read-only DOM probe + `resolve_target` (landed `6528be8`) | **~64 attended, of which ~20 harness mechanics** (~44 was cv-mapper product work: live probes on 5 real profiles, the DOM diagnosis that killed the design's selector strategy, D14). **Excludes an 8h52m land-keystone wait** (overnight, unattributable) | **Harness: $0 / 8 native iters / 2 dispatches → 0 landed lines.** One orphan (`agent sprint --help` executed as a real sprint, SIGPIPE'd: `ended_at`/`iters`/`stop_reason` NULL) and one `max_iters` at the compiled default 8, 54s, **12 tool results all reads**, `[wt] no changes produced`, oMLX `Qwen3.6-35B-A3B-oQ8-fp16-mtp`. **Every landed line was written by an operator-spawned Claude Code Task subagent** (`t2-linkedin-probe`, opus, `bypassPermissions`) — 282 assistant turns / 251,864 out / 89.3M cache-read / **cost unrecoverable** (no `total_cost_usd` in the subagent transcript). **This cell cannot be filled in the column's own terms: the column reads run telemetry, and the run telemetry describes work that produced nothing** | **1 TRUE, 1 FALSE POSITIVE.** mutation **0.600 < 0.70** (376 survivors) → **+1,304 test lines, 92 tests, 1 real bug** (the probe's `not_found` note overcounted opened candidates) → 0.855. **Not gamed**: 77 survivors provably equivalent under PEP 563, worker computed the ~0.918 ceiling and refused to rewrite the annotations. `oracle_intact` **INTEGRITY VIOLATION = FALSE POSITIVE** — fired on the operator's own approved amendment already on master (`d9c8a31`), cleared 34s later by `git merge master` **with no byte changed**; its stated remedy was correctly ignored | **0 found / window thinner than t1's.** The landed module **has never been executed since it landed**, in a DOM the project's own D2b records as changing continuously | **4** — *cli-defect* (`--help` dispatched a real sprint); *iters-cap + wrong-plan* (the machine-drafted plan was never replaced and pointed at a file the oracle does not import); *mutation-red* (0.600→0.855); *gate-false-positive* (verify → `git merge` → verify) | **n/a** — same deletion as t1 |

**Two things the pinned schema has no cell for, recorded here rather than by bending a column:**
(a) **the mid-ticket oracle amendment** — `check_probe_readonly.py` 162→316 lines on 08-10, operator-authored,
user-approved, measurement-driven (three §4.1 scopes matched zero elements), committed to master and
`cp`'d into the worktree; and (b) **the mid-ticket criteria amendment** — AC-P1.7 added 08-10, four
days after t2's human `criteria_confirmed` row, which therefore certifies a text that no longer
exists. **Ledger treatment (recommended, Gary's call):** record both as a **disclosed Session note**
using the instrument's own existing precedent — *"INSTRUMENT DEVIATION, disclosed + Gary-authorized"*
(`trial-ledger.md:71-77`, the batch-keystone entry). Do **not** add a column mid-instrument. (a) is
not "modifying success criteria to fit the result": it narrowed the property against live
measurement and made it *harder* to satisfy accidentally — but the only thing distinguishing it from
the prohibited move is prose in a transcript, and that is the finding. (b) has no record at all, and
becomes the **third next-window instrument debt** alongside the two A6 already filed: a **criteria
digest in the `criteria_confirmed` gate note**, so a future row can prove which text a human
confirmed. Until that exists, every `catches` and `operator-min` cell is measured against a contract
the instrument cannot reproduce.

#### 3. The verdict, split three ways

**(a) The gates work — HOLDS on outcome, FAILS on record.**

Numbers: **5 refusals, 3 true catches, 1 false positive** (precision 3/4); `tests_green` passed
first try on both tickets and never refused. Both mutation cycles produced changes a reviewer would
want — **2 genuine bugs** (`_major_version` returning a confident version number for the wrong
browser, the exact silent-wrong-answer class t1 existed to prevent; the probe's candidate
overcount) and **1,304+ test lines / 92 tests** on a surface that had zero. The gate was **not
gamed**: the one available route (rewriting `str | None` to `Optional[...]` to kill 77 unkillable
mutants) was found, named, and refused in writing. `Verify→Review` requiring **both**
`tests_green` **and** `mutation_score` (`spine.rs:63-66`) is the one machine barrier with teeth and
it fired three times. **Integrity: 4/4 clean** — no worker reported a gate it had not run, no gate
row was faked, and the two red gates were cleared by strengthening tests, never by moving a
threshold. Misses: **0 found**, with one piece of genuinely strong positive evidence (t1's artifact
run first-try by a non-technical user in its target environment, 3 days post-land, no patch).

Against, and it is not small: `oracle_intact` produced the trial's only false positive, is a **diff
detector** whose implementation drifted from its own recorded decision (`decisions.md:1164-1175`
specifies base-branch-vs-worktree *content*; `git.rs:204-207` implements three-dot diff
membership), fired on the operator rather than a worker, had its stated remedy correctly ignored,
and gates no transition. And **the board holds no record that any gate ever went red.** Both reds
were erased by `report_gate`'s `DO UPDATE`; `event` logs that a gate was reported and not what it
said; the surviving green rows are **back-dated to their own failures**. Reconstructing the three
true catches required two **dangling git objects** one `git gc` from deletion. **The instrument
that proves the gates work cannot show its own best evidence** — and t1's two catches are not even
in `.harness/`, because those gates ran in the foreground.

Score: outcome **pass**, record **fail**. The failure is the more consequential of the two, because
it is the one that makes the pass unprovable next time.

**(b) The loop works — HOLDS for the delegated-Claude arm, FAILS for the native arm, and the board
cannot tell them apart.**

t1 is the loop working: 3 dispatches, $11.3732, 124 turns, 3,456 lines landed, the WIP commit
preserving 1,456 lines through a 429 that struck 0.6s after a `Write`, complete worktree cleanup
(no orphan branches, worktrees or stub tickets), and the board cheap enough to re-orient from that
after the first compaction the operator recovered t2's whole contract with a single `agent show`.
**Worker containment and worker honesty were perfect across all four workers** — zero writes to any
oracle, zero out-of-tree writes, and four unprompted self-corrections including a worker retracting
its own asserted version numbers and another declaring its fake had been lying. That is the
strongest single result in the trial and it belongs in the verdict at full weight.

t2 is the loop failing: **2 harness dispatches, 0 landed lines.** One orphan created by the CLI
executing `--help` as a real sprint; one `max_iters` at the compiled default of **8**, with 12 tool
results all reads, reaching the right conclusion on the last iteration and then dying. Every landed
line came from a CC Task subagent the board has never heard of, at 282 turns and an unrecoverable
cost. Telemetry across both: **no cost column at all**; `iters` means two different things in one
column; `harden`/`verify`/`land` open no `run` row (verified: `start_run` has two call sites);
`agent trajectory` — the exact verb for three of the eight visibility complaints — was invoked
**zero times**; the `memory` table is empty **by construction**, because the read path is injected
into every worker prompt and the only automatic writer is on a verb that was never run.

Score: **1 of 2 tickets driven; 0 of 2 with a complete telemetry row.** This is the one place
cv-mapper narrows A6: graduation binds *"board + gates + delegated worker"* (`decisions.md:1501`),
and on this project **the delegated-worker arm held and the native-loop arm did not produce a
line**. A6's evidence was 5 PDSI tickets all on the delegated worker, so this is new information,
not a contradiction.

**(c) Worth the operator tax — NOT ESTABLISHED, and the headline ratio does not answer it.**

The 3.7× lines-per-attended-minute gap (**55 board / 201 off-board**) is real and confounded three
ways: the off-board lane ran **three subagents in parallel** in one 78-minute burst while the board
lane ran sequentially across six days containing an 8h47m Anthropic rate limit and two land waits
that were mostly overnight; the tests row of that same comparison was **retracted** (board lane
added 410 tests / 5,023 test lines, off-board 177 / 3,289); and the off-board lane's discarded D24
scoping pushes its true cost to 49–71 min. Quoting 3.7× as the tax verdict compares parallelism to
sequencing and calls the difference overhead.

The number that does answer (c) is marginal: **~33 ± 6 min of harness mechanics across two
tickets** — 23% of 145.7 attended min — plus ~13 min of keystone interaction. What that bought,
each item evidenced: two mutation red→green cycles that fixed two real bugs; worktree isolation
that kept a 19,414-mutant sweep off master; two human keystones per ticket (a diff, not a plan);
and an audit trail of 5 run / 10 gate / 39 event rows. What the lane without it shipped: **7,388
lines with zero mutation testing** (D27, recorded as open debt), `cli.py` at 248 lines with **zero
tests** on the verb that appends to the recruiter's real workbook, and `llm.py` tested only against
a fake whose wire contract has never met the real API. At the margin the tax looks cheap and
well-spent — but part of it is **self-inflicted** (the 4h25m runaway traces to the harness's own
`git add -A`; the orphan run to a CLI that executes `--help`), and the trial ran **no comparator
arm and no upfront estimate** — precisely the two instrument debts A6 filed and nobody has paid.
So (c) is an argument, not a measurement, and it will stay one until those two columns exist.

**Overall.** The harness did not pass its own gate, because the gate was not open and cv-mapper is
half the size of the only bar ever pinned. On the evidence it did produce: the gates earned their
keep and cannot prove it; the loop earned its keep on one of two tickets and lost the other to two
defects that are each a few lines of Rust; the tax question is unanswered by design, because the
instrument that would answer it was never built.

**What would have to change for a failing claim to pass**

- **(a) record** — preserve red gate rows. Cheapest form needs no schema change: `event` is
  documented as the append-only history (`schema.rs:5-6`) and `report_gate` spends its only free
  `note` column on the *gate name* while the verdict and the score string it already holds go
  nowhere. Widening the `gate_results` PK or adding an append-only sibling is the fuller fix.
- **(a) predicate** — replace `oracle_intact`'s diff-membership test with content provenance
  (worktree blob byte-identical to the base tip **or** the merge base). Do this **before** any
  promotion to a transition gate; promoting the current predicate would institutionalise the
  trial's only false positive.
- **(b) native arm** — `DEFAULT_MAX_ITERS` 8 → 40 (the number research/21 already prescribed and
  `decisions.md:1012` deferred to an env var nobody set); reject unrecognised argv tokens at *every*
  position and accept `--flag=value` (today `agent run t1 --worker=claude` silently runs the local
  model); surface the stop label plus a trajectory pointer on a `max_iters` exit. Then re-run a
  t2-shaped ticket on oMLX and see whether it writes a line. Until it does, "local-first" is a
  direction, not a capability.
- **(b) telemetry** — accumulate cost/turns from stream events into a `run` column (already an
  A6-filed debt), and open a `run` row around `harden`/`verify`/`land`.
- **(c)** — run the comparator arm and the upfront-estimate column on the next stream. No number of
  further landed tickets answers (c) without them.

**What would falsify this verdict**

- **(a)** — a post-land defect in `windows_deploy.py` or `linkedin_selectors.py` that mutation,
  tests or the oracle should have caught. The specific test is cheap and named: run
  `linkedin_selectors.py` against fresh LinkedIn profiles today, and re-run the recruiter probe on a
  second machine. Either one producing a defect turns `0 found / thin window` into a real miss and
  drops (a) from HOLDS.
- **(b)** — evidence that a *harness* dispatch wrote landed t2 lines: a `run` row, or a recorder
  file containing a write. I looked; `.harness/runs/t2/*.jsonl` is 5 lines and 30 lines, and every
  tool result in both is a read.
- **(c)** — a comparator arm showing the off-board lane's 7,388 ungated lines accrue no more
  post-land defects than the board lane's 8,025 over a comparable window. That would make both
  mutation cycles ceremony rather than value, and it is the single result that would most change
  what this harness should be.

#### 4. The `## Open` items addressed to me

**Should a red gate be recoverable from the board? Yes — this is the highest-value change the
trial produced.** Adjudicating the three true catches required reconstructing both scored trees, and
the board could supply neither: they came from `git fsck --lost-found` on two unreferenced objects
(`696d306`, `8516ee6`). t1's two refusals are not in `.harness/` either, because those gates ran in
the foreground; they survive only in a chat transcript. The harness's central claim is that its
gates catch things, and its own store keeps **no evidence any gate ever refused anything**. Fix as
in §3 (a) record.

**Is the back-dated `created_at` load-bearing anywhere? Verified: no in the code, yes in the
analysis.** `grep -rn created_at crates/` returns the DDL only — `gate_satisfied` keys on
`(issue_id, gate, attempt, passed, source)` (`board.rs:257-273`) and never reads the timestamp, so
no transition, gate or guard depends on it. It corrupts **derived figures**, and it already has:
this file's seeded t2 span was computed off that column and was wrong at both ends (retracted by
agent 1). **Ruling for the ledger: never derive a duration from `gate_results.created_at`.** With
red rows preserved (above) the column becomes meaningful again for free; without them it is a
timestamp of the first *failure* wearing a passing row's clothes.

**Does the harness have any notion of "this ticket was worked by something I did not dispatch"?
No — nothing in the schema distinguishes gated work the harness drove from gated work it
inherited.** t2 carries five green gate rows, a squash-landed branch and `done`, and its only two
`run` rows produced zero lines. **The ruling the file asked for:** A6 binds the claim to *"board +
gates + delegated worker"* (`decisions.md:1501`), so **t2 is evidence for (a) and is not evidence
for (b)**, and the ledger row says so in the column itself rather than in a footnote. **A cheap
detector exists and is unbuilt:** a ticket reaching `verify` with a non-empty branch diff whose
every `run` row ended `max_iters`/NULL having produced no changes is exactly the inherited-work
signature. The harness holds all three facts and joins none of them. That is my proposal, not
observed behaviour.

**Ledger treatment of the mid-ticket oracle amendment** — answered in §2: disclosed Session note on
the instrument's existing deviation precedent; the *criteria* amendment becomes the third
next-window instrument debt (a criteria digest in the `criteria_confirmed` note). No column added.

**`agent draft` — should it exist? Keep it, with one guard; do not cut.** Its value on cv-mapper was
negative (117s, both outputs rejected in the next turn, and the one field the operator did *not*
overwrite steered the local model into the wrong file), and it falsified its own recorded acceptance
bar — *"Align would be one-round … not a rewrite"* at calibration n=1 (`decisions.md:1246-1249`) —
at n=2 and n=3, both rewrites. But the defect that hurt is **partial overwrite**, not drafting:
`draft` writes plan + criteria + validation as a coherent set, the operator may overwrite any
subset, and nothing notices the set stopped being one. Cheapest fix is agent 4 item 9's: have
`agent align` print the full rendered workpad (`board::render` already exists) before the keystone,
so the human confirms the contract rather than the field they just typed. Revise the recorded n=1
bar to match what the verb is actually for.

**Does `harden` need a progress channel? It needs a `run` row first; the rest is Gary's call.**
Independently verified that it opens none. That single absence causes five of the eight visibility
complaints, ~25 polling tool calls, one wrong ETA and one spurious task notification the operator
had to explain away. A `start_run`/`finish_run` pair around `harden` makes `agent board` and `agent
trajectory` answer "alive, since when" with no new surface. Whether to also stream cosmic-ray's
`work_results` count — which the operator read by hand out of a private sqlite under `/var/folders/`
— is a design choice I am leaving open with that reason.

**Does the memory plane have a writer on the mainline? Factually closed; the design question stays
open.** No writer: `recall_primed` is injected at three call sites, the only automatic `remember` is
`run_explore`'s reflection-on-kill, and `explore` is oMLX-pinned and was never run. **Add the
red-gate `lesson` writer** — it is nearly free (the note text is already in hand at `main.rs:2069`,
`2111`, `2239`) and the one lesson it would have captured is exactly the one that recurred at a
4h25m cost. **Leave open** whether the `memory` table is the right home at all: cv-mapper's real
memory was `wiki/decisions.md` (27 decisions), it survived three compactions, and every
re-orientation read it. n=1 does not settle table-vs-wiki.

**What is `operator-min` measured against? Recommendation, not an edit — I am read-only on the
instrument.** Do **not** redefine the column: the five graduated rows already fold oracle authorship,
probe agents and workpad correction into "ALL operator work", and narrowing it now would silently
restate a closed graduation. Instead **add the counting rule** the column never had (attended-session
clustering at a 10-minute gap, machine wait inside a cluster charged to the operator, human-keystone
*waiting* excluded and reported separately) and, if a second number is wanted, a **separate
harness-mechanics-share** figure — which is the number that actually bears on claim (c). Under the
column as written, cv-mapper reads t1 ~23 and t2 ~64.

**Left open, with reasons:** the harden progress *mechanism* (above); memory table vs wiki (n=1);
and agent 3's ~4m30 harden-babysitting estimate, which nothing on disk can settle.

#### 5. What this cost

Six agents in a strict chain, each reading the whole file before appending. I cannot price it: no
per-agent cost or turn telemetry was captured for this investigation either — the same gap the trial
found in the harness, reproduced in the instrument used to study it. What is countable: the file
reached **1,387 lines** before this section; my own read of it was ~46K tokens for the first page
alone; and the chain is strictly serial, so its wall clock is the sum of six passes, roughly 6× a
disjoint fan-out over the same slices.

**The retraction step earned its keep, and the count in my brief was low.** `## Retracted` holds
**nine** entries, not seven. Four killed **seed** claims — "8 gate rows" (there are 10), "t2 spans
08-06 → 08-12" (read off the back-dated column), "the oracle-tamper gate caught a real violation"
(it caught a diff, on the operator), and "the mutation gate went red once" (twice). Five killed
predecessors' or inherited claims: reasoning "discarded" (it was recorded), `review` never firing
implying a skipped gate (it was a deliberate deletion), 12-of-24 verbs (25 and 13), 410-vs-587 tests
(587 contains the 410), and `--max-turns 40` binding run 3 (150 was exported per-command).

**Was the seed sloppy?** Partly, and mostly not. One retraction is a plain miscount that one query
would have caught. **Three of the four seed errors came from reading the board at face value** — the
back-dated timestamp, the erased red row, the gate log without its provenance — which is the exact
defect the trial went on to find. The seed inherited the flaw it was written to investigate. That is
the argument for the chain: a six-way fan-out would have handed the same four wrong premises to six
agents working in parallel, each would have built on them independently, and nothing in the design
would have caught it, because the errors were in the shared premise rather than in any one slice.
Serial-with-mandatory-retraction was the right engine **for an investigation whose subject is which
numbers survive their basis**. For slice-disjoint work with independent premises it would be a 6×
wall-clock tax for nothing.

---

## Working

*(agent 1's five entries were all settled by agents 2 and 4 and are dropped; the findings live in
`## Confirmed` under those agents, and the retracted framings in `## Retracted`.)*

*(agent 3's entries)*

- ~~**The harness/product split inside t2's 64 attended minutes is a judgment call at the
  boundaries.**~~ **SETTLED (agent 6): agent 3's boundary stands as drawn.** Worker adjudication
  counts as *product* work — it survives deleting the harness. The ledger row therefore reads t2 as
  **~64 attended / ~20 harness mechanics**, and the alternative reading (~35 for t2) is noted here
  and not used. The column definition itself is not being changed — see §4, `operator-min`.
- ~~**8h52m of the t2 land wait and 7h40m of the t1 land wait are unattributable.**~~ **SETTLED
  (agent 6): excluded from `operator-min`, reported in the row's own cell.** Both waits are real
  project delay and neither is operator work; the t2 wait spans 08-12 23:13 → 08-13 08:05 local,
  i.e. overnight. "Time blocked on a human" is a reasonable future column and is **not** being added
  mid-instrument.
- **~4m30 is the estimate for `harden` babysitting on t2, from 10 poll calls inside a 22.8-min
  cluster that also contains deliberate wiki work.** True figure somewhere in 3–7 min. **Still open
  — reason:** nothing on disk can separate the polling from the wiki work inside that cluster. It is
  the softest number in agent 3's section and does not change any verdict at either end of its band.

*(agent 5's entry)*

- **`misses = 0` is a search result, not a measurement, and it is the softest number I produced.**
  Everything I could search on disk is negative and consistent (git history, the project's 27
  decisions, two defect-language regex sweeps over the post-land transcript, and the suite still
  green at 587), and one piece of positive evidence is genuinely strong — t1's landed artifact was
  run in its target environment by the recruiter three days after land and returned five passes
  with no patch. But **t2's landed module has not been executed once since it landed**, and its
  domain is a DOM that D2b itself records as changing continuously. So the number is honest for
  the window and the window is thin. **Settled by:** one live `linkedin_selectors.py` run against
  fresh profiles after 08-13, and a second recruiter-laptop run. Until then agent 6 should write
  the ledger cell as **`0 found / thin window`**, not `0`.
  **Agent 6: adopted verbatim in both drafted rows, and named as the falsifier for claim (a).** The
  entry stays open — reason: the two runs that would settle it have still not happened, and neither
  is expensive.

## Open

- ~~Does the harness record per-run cost/turns for cv-mapper at all, or did the timeout
  telemetry gap bite again here?~~ **ANSWERED (agent 1).** Turns yes, cost never — `run` has an
  `iters` column and no cost column, and `finish_run` takes only `(stop_reason, iters)`. The
  $11.3732 of t1 spend exists only in the teed CC `stream-json`, and the local-model runs have
  no telemetry at all. The gap that bit here was not a *timeout* but an **orphan**: run
  `t2-000-1786352125711` left `ended_at`/`iters`/`stop_reason` NULL. The harness documents that
  NULL as the intended interrupted-run marker, so it is queryable — but it carries no reason.
- ~~Was the 8-iteration cap an oMLX-provider default, a sprint default, or a config the operator
  set?~~ **ANSWERED (agent 1).** Neither provider-specific nor operator-set: `DEFAULT_MAX_ITERS
  = 8` compiled into `crates/agent/src/main.rs:70`, binding the native loop only, with no
  `HARNESS_MAX_ITERS` set anywhere on this machine. It never touched t1, which ran the claude
  worker at `--max-turns 40`.
- ~~**Should a red gate be recoverable from the board?**~~ **ANSWERED (agent 6): yes — the highest-value
  board change this trial produced.** `created_at` is read nowhere in `crates/` outside the DDL, so
  preserving the reds costs nothing in control flow and restores the column's meaning for free; the
  cheapest form spends `event.note` on the verdict + score instead of the gate name. Detail below,
  and in `### Agent 6` §3(a) / §4. The evidence, as the chain left it: `report_gate`
  upserts on `(issue_id, gate, provider, attempt)` without bumping `attempt`, so the 0.600
  mutation FAIL and the oracle INTEGRITY VIOLATION — the two most valuable events in this whole
  trial, and the two the harness is proudest of catching — are absent from `gate_results`, and
  `event` records that a gate was reported but not what it said. The instrument that proves the
  gates work cannot show its own best evidence. → agent 6, for the ledger and the verdict
  **Agent 5 adds the decisive evidence.** Adjudicating the three true catches required
  reconstructing both red trees, and the board could not supply either. They came from
  `git fsck --lost-found`: the tips of the two deleted branches (`696d306`, `8516ee6`) are
  **dangling objects**, and every `"{id}: work in progress"` commit hangs off them. One `git gc`
  and the only surviving record of what the gates caught is four small text files in `.harness/`
  — and t1's two refusals are not even there, because t1's gates ran in the foreground.
  **The harness's best evidence currently survives by accident, in unreferenced git objects.**
- ~~**Is the back-dated `created_at` load-bearing anywhere?**~~ **ANSWERED (agent 6): not in the
  code, yes in the analysis.** Verified at source — `grep -rn created_at crates/` returns the DDL
  only, and `gate_satisfied` keys on `(issue_id, gate, attempt, passed, source)`
  (`board.rs:257-273`), so no transition, gate or guard reads the timestamp. It corrupts derived
  figures, and already has: this file's seeded t2 span was computed off it and was wrong at both
  ends. **Ruling: never derive a duration from `gate_results.created_at`** until red rows are
  preserved, after which the column means what it appears to mean again.
- ~~**Was the 8-iteration budget ever measured, or inherited?**~~ **ANSWERED (agent 4).** Inherited
  from a decision that deferred it to an env var. research/21 §1 called `DEFAULT_MAX_ITERS=8`
  "tiny" alongside `DEFAULT_MAX_TOKENS=1024` and prescribed `HARNESS_MAX_ITERS=40`; decisions.md:1012
  records the bump as **env-only** for both knobs. The token half was later promoted to a compiled
  default (1024→4096) with its measurement citation intact (`main.rs:62-71`); the iters half never
  was. Every evaluation since ran at 40 by export; the default an operator gets is 8.
- ~~**The `[work ] outcome: … turns=N cost=$X` line is printed and then lost.** … Is the
  visibility complaint really a *durability* complaint? → agent 2 (does the operator ever ask
  for a number the board could have answered?)~~ **ANSWERED (agent 2).** Three of the eight
  friction turns asked for something the `run` table held at that second (see the table in
  `## Confirmed` → Agent 2); the operator answered all three from file mtimes, `pgrep` and
  background-task output instead, and never once ran `agent trajectory`. Four more asked about
  work the board structurally cannot see — `harden`/`verify` open no run row, and the t2 Task
  subagent was never on the board at all. So: **a read-path failure on dispatches, a coverage
  gap on gates, and a durability failure only for cost.** The durability claim is real but
  narrow, and it bit precisely once: at 08-07 00:33 and 00:35 the operator quoted t1's spend as
  `$6.38 across three runs` and then `$9.12 on t1` two minutes apart, after two dispatches. The
  true two-run total was $6.3853; $9.12 double-counts run 2; and the final three-run total was
  $11.3732. Nothing on disk could have corrected it, because there is no cost column. — op
  L787/L812; agent 1's `type:"result"` figures
- ~~**Does the harness have any notion of "this ticket was worked by something I did not
  dispatch"?**~~ **ANSWERED (agent 6): no — and the ruling the item asked for is that t2 is
  evidence for the gates and not for the loop**, since A6 binds the claim to "board + gates +
  delegated worker" (`decisions.md:1501`). A cheap unbuilt detector: a ticket reaching `verify`
  with a non-empty branch diff whose every `run` row ended `max_iters`/NULL having produced no
  changes. See `### Agent 6` §4. As the chain left it:
  t2 carries five green gate rows, a landed branch and a `done` status for code the
  board never saw written; the two `run` rows it does have both produced nothing. Nothing in the
  schema distinguishes "gated work the harness drove" from "gated work the harness inherited".
  If the daily-driver claim rests on the gates rather than the dispatches, that may be fine and
  should be said out loud; if it rests on the loop, t2 is not evidence for it. → agent 6
- ~~**The oracle amendment was the right call and the gate treated it as an attack.**~~
  **ANSWERED on the design side (agent 4); still open for the ledger (agent 6).** The harness has
  the parts and does not wire them: `set_acceptance_criteria` is a bare column write that touches
  neither status nor gates at any status (`board.rs:112-114` → `163-176`), so t2's human
  `criteria_confirmed` row certifies a text that was edited 4 days later — against the very doc
  comment on the setter ("*`criteria_confirmed` confirms **these***", `board.rs:111`). Correct
  mechanism, no schema change: a criteria digest in the gate note + route a post-Align criteria
  edit through the existing `InProgress→Align` back-edge as `raise_confusion` already does. The
  asymmetry is the point: "never modify success criteria" is enforced by code against the *worker*
  only, and the operator is the only party with a `criteria` verb. See `## Confirmed` → Agent 4
  item 8. ~~→ agent 6 for the ledger column.~~ **CLOSED (agent 6): no column.** Both amendments go
  into the ledger as a **disclosed Session note** on the instrument's existing deviation precedent
  (`trial-ledger.md:71-77`), and the criteria digest in the `criteria_confirmed` note becomes the
  third next-window instrument debt. `### Agent 6` §2.
- ~~**Should a mutating verb be reachable by typo?**~~ **ANSWERED (agent 4): it is a class, and it
  is wider than `--help`.** No arm anywhere rejects an unrecognised argv token; `arg()` accepts
  `--help` as a positional (`main.rs:2935-2940`) and `flag()` matches names exactly, so
  `--worker=claude` is silently dropped and `agent run t1 --worker=claude` runs the **local model**
  (`main.rs:2960-2962`, `248-256`). The recorded intent covers argv[1] only (`main.rs:130-132`,
  test at `cli_dispatch.rs:64-72`). `sprint --help` dispatches; `new --help` mints a ticket titled
  `--help`; `land t1 --help` lands. See `## Confirmed` → Agent 4 item 1. ~~→ agent 6 (ledger).~~
  **CLOSED (agent 6): it enters t2's `bounces` cell as cause `cli-defect`**, and it also degrades
  the `worker $ / turns / attempts` cell, because the orphan `run` row it created is one of the two
  dispatches that cell has to report. One CLI defect corrupts two ledger columns.
- **Does the memory plane have any writer on the mainline path?** (new, agent 4) No: the only
  automatic `remember` in the harness is `run_explore`'s reflection-on-kill (`main.rs:1183-1203`),
  while `recall_primed` is auto-injected into every worker prompt at three call sites. Read path
  always on, write path reachable only through a verb that is oMLX-pinned and was never run — so
  the store is empty by construction on any normally-driven project, and the "memory compounds at
  ~zero cost" claim in CLAUDE.md has no mechanism behind it yet. Is that a gap to close (write a
  `lesson` on every red gate — the note text is already in hand at `main.rs:2069/2111/2239`), or is
  the wiki the real memory and the `memory` table premature? → agent 5/6
  **Agent 5, on whether the empty table cost anything observable here: once, and expensively.**
  cv-mapper's real memory was `wiki/decisions.md` (27 decisions) and it worked — the wiki is what
  survived three compactions and what every re-orientation read. But the single lesson the proposed
  "write a `lesson` on each red gate" fix would have captured is precisely the one that recurred:
  run 3 predicted the tracked-`mutants/` runaway in its final report, the operator filed it as
  cosmetic, and the next `harden` on the same ticket cost **4h25m and 19,414 mutants** (agent 2).
  A `lesson` written at the 0.362 red would have been in the prompt for every later dispatch. That
  is n=1, and it argues for adding the writer, not for the table being the right home. ~~→ agent 6~~
  **PART-CLOSED (agent 6): add the red-gate `lesson` writer** — nearly free, and the one lesson it
  would have captured is the one that recurred at a 4h25m cost. **Still open: whether the `memory`
  table is the right home at all** — reason: cv-mapper's real memory was `wiki/decisions.md`, it
  worked, and n=1 does not settle table-vs-wiki.
- ~~**`GATE_ORACLE_INTACT` is evidence-only and gates no transition… Does the daily-driver claim
  need it to be a transition gate?**~~ **ANSWERED on the evidence (agent 5): no — fix the predicate
  first; promoting it now would institutionalise a misfire.** In this trial the gate fired exactly
  once, on the operator's own approved amendment, and was cleared by `git merge` with no byte
  changed (agent 2). Promoting a diff detector to a transition gate would have made that false
  positive *harder* to clear without making containment stronger, because containment needed no
  help: zero oracle `Write`/`Edit` and zero out-of-tree writes across all four workers (agent 2),
  and the t2 worker volunteered *"your amendment, untouched by me"* unprompted. Sequence for the
  ledger: agent 4 item 3's content-provenance predicate first, promotion to `required_gates_for`
  only after it has fired correctly on a real worker. Agent 4's structural facts stand unchanged —
  `Review→Land` requires nothing, and `run_land` (`main.rs:2857-2871`) re-checks nothing and writes
  its own `landed` row. → agent 6 for the ledger call
- **Does `harden` need a progress channel, or does it need to stop being the operator's problem?**
  It opens no `run` row, prints nothing for its whole runtime (1h18m on t2, 4h25m on the t1
  runaway), and the only readable progress is `select count(*) from work_results` in cosmic-ray's
  private sqlite under `/var/folders/…`. That single missing signal is the direct cause of five of
  the eight visibility complaints agent 2 tabulated, ~25 polling tool calls, one wrong ETA, and
  one spurious task notification the operator had to explain away.
  **PART-CLOSED (agent 6): it needs a `run` row first** — independently verified that `start_run`
  has exactly two call sites (`main.rs:469`, `main.rs:1024`), neither in `harden`. That pair alone
  makes `agent board`/`agent trajectory` answer "alive, since when" with no new surface. **Still
  open: whether to stream cosmic-ray's `work_results` count** — reason: that is a design choice
  with a real cost, and it is Gary's, not a finding. Rest of the item as the chain left it.
  **Agent 4 confirms the source
  shape:** `run_harden` is fully synchronous and prints only after the backend returns — the
  `[harden] PASS` / `BELOW THRESHOLD` block is the first and only output (`main.rs:2249-2258`), and
  it opens no `run` row (no `start_run` call anywhere in it). Nothing streams the backend's progress
  even though cosmic-ray is writing verdicts into a sqlite file the whole time. → agent 5/6
- **`agent draft` produced negative value on both tickets. Should it exist?** Two invocations,
  117s of attended wall clock, both outputs rejected in the very next turn, and the one field the
  operator did *not* overwrite (`plan`) is the defective one that later steered the local model
  into the wrong file. A verb whose output must be discarded but whose leftovers are load-bearing
  is worse than no verb. **Agent 4 adds the recorded bar it failed:** the draft decision's own
  acceptance criterion is "*Align would be one-round … not a rewrite*", at calibration **n=1**
  (`wiki/decisions.md:1246-1249`). cv-mapper is n=2 and n=3, both rewrites — the first real-project
  falsification. The partial-overwrite hazard is separately structural (item 9: nothing cross-checks
  plan against criteria). ~~→ agent 6 (ledger): keep-with-a-plan-overwrite-guard vs cut.~~
  **CLOSED (agent 6): keep, with the guard.** The verb's failure mode is partial overwrite, not
  drafting; the fix is `agent align` printing the full rendered workpad before the keystone, and
  revising the recorded n=1 acceptance bar. `### Agent 6` §4.
- **What is `operator-min` measured against?** `trial-ledger.md` defines it as "honest wall-clock
  estimate of ALL operator work" without naming a baseline or a counting rule. Three readings give
  88, 146 and 392 minutes for the same two tickets. Until the column says which, cv-mapper's row
  is not comparable to any kata row. ~~→ agent 6~~ **ANSWERED (agent 6): do not redefine the
  column** — the five graduated rows already fold oracle authorship, probe agents and workpad
  correction into "ALL operator work", and narrowing it now would silently restate a closed
  graduation. Add the **counting rule** it never had (attended clustering at a 10-min gap, machine
  wait inside a cluster charged to the operator, keystone *waiting* excluded and reported
  separately), and optionally a separate harness-mechanics-share figure — the number that actually
  bears on claim (c). I am read-only on the instrument; **the edit is Gary's.** `### Agent 6` §4.

## Retracted

- ~~**Board lane: ~410 tests passing. Off-board lane: 587 tests passing.**~~ (agent 3's
  counterfactual table; inherited from `<project>/wiki/log.md` and `active-work.md`) — **agent 5.**
  The two cells are not comparable: **587 is the whole suite, and it contains the 410.** t1's
  `tests_green` gate note reads `247 p…`; the t2 worker reported *"92 new tests (163 in the file,
  410 in the suite)"* — 247 + 163 = 410, the board lane's total. Master then reports `587 passed`
  (op L2620, 08-14). **The off-board lane added 177 tests, not 587.** By lines it is starker still:
  the board lane wrote **5,023 test lines** (511 + 1,731 + 2,781) for 410 tests; the off-board lane
  wrote **3,289** for 177. The accurate row is *"tests added: 410 (board) vs 177 (off-board)"*, and
  the lines-per-attended-minute row is unaffected because it counts source, not tests. This is the
  file's own most-repeated failure once more — a correct figure (587 *is* the passing count) quoted
  against a basis it does not share. — `gate_results.t1 tests_green.note`; subagent L440; op L2620;
  `wc -l tests/*.py`
- ~~**The model's reasoning was discarded between turns while its 8-iteration budget burned.**~~
  (agents 1 + 2, `## Working`) — **agent 4.** "Discarded" is wrong twice over. (a) Every reasoning
  block is written to the trajectory as a `reasoning`-role record (`recorder.rs:82-92`); the loop's
  own log line says exactly this — `[reason] N bytes (**recorded**, not re-fed)` (`main.rs:601-604`)
  — and `agent trajectory t2 --full` prints them whole (`main.rs:889-903`). The 926- and 5,250-byte
  blocks are on disk in `.harness/runs/t2/t2-000-1786352167614.jsonl` and always were. (b) Re-feed
  was causally irrelevant to the t2 loss: the run ended *on* the turn that produced the 5,250-byte
  block (iteration 8 of 8), so there was no subsequent turn for a re-feed policy to affect. The
  posture itself is a named **Rejected Approach** with three independent external corroborations
  (`wiki/decisions.md` §Rejected Approaches "Re-feeding reasoning / CoT into context"; research/25
  §4.3). The accurate statement: *the reasoning was recorded and never read; the defect is that a
  `max_iters` exit surfaces neither the stop label nor a pointer to the trajectory that holds it.*
- ~~**`agent review` never firing means the decorrelated DeepSeek acceptance review never ran on
  either ticket.**~~ (agent 2, as an implied gap) — **agent 4, partial.** The fact is right; the
  implication that something was skipped is not. The **automatic** sprint call was deliberately
  **deleted** on 2026-08-02 against a pre-pinned rule — 0 true positives / 0 false VIOLATES across
  5 real tickets, with window-extension explicitly rejected as moving criteria to fit a hoped-for
  result — and the verb was retained for manual use (`wiki/decisions.md:1447-1458`). cv-mapper
  running without it is a decision being honoured, not a gate being missed.
- ~~**Twelve of the 24 verbs were never invoked.**~~ (agent 2, partial) — **agent 3.** The list of
  twelve is exactly right and stands. The denominator is **25**, and **13** verbs were used, not
  12: `show` (op L1088, the project's only invocation, used to recover t2's contract after the
  first compaction) and `edge` (op L679, `$AG edge t2 t1`, which produced the single row in the
  `edge` table) are both missing from agent 2's used-count. The accurate statement: *25 verbs
  exist, 13 were used 51 times across 42 Bash calls, 12 were never used.* Separately, the
  top-level usage line and the module header disagree with each other — the usage line lists
  `close` (undocumented in the header) and omits `review` (documented). — `main.rs:10-29`; op
  L1331 output; regex over all 304 Bash commands
- ~~**All 8 gate rows passed.**~~ — **agent 1.** There are **10** gate rows (5 per ticket), not
  8; `select count(*) from gate_results` = 10. More importantly the sentence reads as "no gate
  ever failed", which is false: t2's mutation gate failed at 0.600 and t2's oracle gate reported
  an INTEGRITY VIOLATION. Both were erased by the `ON CONFLICT … DO UPDATE` in
  `crates/board/src/board.rs:239-249`. The accurate statement is: *ten gate rows survive, all
  green, and the board retains no trace of the two that went red.*
- ~~**t2 spans 08-06 16:06 → 08-12 16:11 on its gate rows.**~~ — **agent 1.** Both endpoints are
  wrong for the purpose. The `landed` row is `2026-08-13 01:05:42`, so the span is 08-06 16:06:17
  → 08-13 01:05:42 = **6d 8h 59m**. And 08-12 16:11:18 is the `oracle_intact` row, whose
  `created_at` is the back-dated timestamp of the **failed** verify — the pass was at 16:12:37.
  This is the file's own warning about a figure outliving its basis, committed in the seed: the
  number was read off a column that does not mean what it appears to mean.
- ~~**The oracle-tamper gate caught a real violation.**~~ — **agent 2.** It caught a real
  *diff*; there was no violation. The change to `scripts/check_probe_readonly.py` was
  **operator-authored, user-approved, and already on master** as `d9c8a31` before the gate ever
  ran. The operator `cp`'d it into the worktree at 08-10 10:20:02 (op L1517) *and told the
  worker in writing not to touch it* — *"The oracle … is operator-authored — I will amend it
  myself to match the new rule. DO NOT EDIT IT"* (subagent L57). The harness's own
  `commit_worktree` (`git add -A`, `git.rs:289-297`) then swept it onto `harness/t2` as
  `414f0d2`. Two days later `agent verify` compared branch-vs-base, found the oracle in t2's
  diff, and fired. The operator's remediation was **not** the one the gate demanded ("Revert the
  file(s) … and make the implementation satisfy the original oracle"): it ran `git merge master`
  to move the change into the merge base, verified the worktree copy was byte-identical to
  master's (`f387fe1c3be5` both sides), and re-ran — PASS, 34 seconds later, **with no byte of
  any file changed**. — `.harness/t2-verify.log` / `t2-verify2.log`; op L1848-L1859; `git log
  2fe366c`; `main.rs:2049-2081`. The accurate statement: *the gate is a diff detector, it fired
  on the operator rather than on a worker, its stated remedy was ignored, and it was cleared by
  changing git provenance alone.* Both worker containment and worker honesty were in fact
  perfect on this point — the t2 subagent volunteered `"your amendment, untouched by me"`
  unprompted (subagent L172).
- ~~**The mutation gate went red once and was cleared honestly.**~~ — **agent 2.** It went red
  **twice**, and both were cleared honestly. t1: **0.362**, 462 survivors, 08-06 16:26:42 UTC
  (`KILLED|262 SURVIVED|462` out of `harness-cr-t1.sqlite`; op L707/L713) → re-dispatched, later
  0.781. t2: 0.600, 376 survivors → 0.855. The t1 failure matters for the same reason the t2 one
  does: `gate_results.t1 mutation_score.created_at = 2026-08-06 16:26:42` is the timestamp of the
  **0.362 FAIL**, so the board's only surviving trace of it is a back-date on the row that
  replaced it.
- ~~**run 3 recorded 71 turns, which is not what the harness's own `--max-turns 40` bounds.**~~
  (agent 1, inside the `iters` line) — **agent 2.** The 40 was never in force. Every t1 dispatch
  ran with `HARNESS_CLAUDE_MAX_TURNS=150 HARNESS_CLAUDE_TIMEOUT=3600` exported inline on the
  operator's own command line (op L695, L733, L752, L821), and the harness printed it back:
  `[work ] t1 — delegated to `claude` (max 150 turns, 3600s wall clock, skip-permissions)`. So 71
  turns was comfortably under a cap the operator chose, and `main.rs:981` reads
  `env_or("HARNESS_CLAUDE_MAX_TURNS", WORKER_MAX_TURNS)`. Agent 1's larger point stands
  unchanged — `iters` still means CC's `num_turns` for a claude run and the native loop counter
  for an omlx run — and so does the `HARNESS_MAX_ITERS` finding, since the operator set env vars
  **per-command** rather than in a shell rc, which is why a grep of `~/.zshrc` and
  `~/.claude/settings.json` found nothing. Later agents: grep the transcript's Bash commands,
  not just the config files.
