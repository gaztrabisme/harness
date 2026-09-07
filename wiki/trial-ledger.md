# Trial Ledger — Measured Daily-Driver Trial (critical-path item 4)

> The instrument for the daily-driver claim. One row per ticket, filled at land/close time.
> Columns and decision rules are pinned **before ticket 1 of each window** and are never adjusted to
> fit results (integrity constraint).

## Contents

| block | stream | schema | status |
|---|---|---|---|
| **Window 1** (below) | PDSI continuation on `~/Documents/Work/FPT/Accelerator/harness-run/` (board `../harness-board.db`, t6+) | 9 columns, pinned 2026-07-15 | **CLOSED 5/5, 2026-08-02** → A6 GRADUATED |
| **cv-mapper block** | `~/Documents/Work/Mean/cv-mapper` — first post-graduation outside project, unplanned | Window-1 schema, unmodified | disclosed, 2 rows, **not a window** |
| **Window 2** | harness self-repair (this repo, board `~/Documents/Work/harness-board.db`) | 10 columns — adds `est-min` | **OPEN 2026-08-17** |

**Window 1's rows and rules below are historical and closed. Nothing appended after 2026-08-02
modifies them.** The `est-min` column exists only from Window 2 forward; back-filling it into Window 1
is impossible (no upfront estimates were made) and inventing one would be the exact integrity
violation this instrument exists to prevent.

---

# Window 1 — PDSI continuation (CLOSED)

> Stream: **PDSI continuation** on `~/Documents/Work/FPT/Accelerator/harness-run/`
> (board: `../harness-board.db`, tickets t6+). Columns and decision rules pinned **before ticket 1**
> (2026-07-15).

## Decision rules (pinned up front)

- **Reviewer fix-or-drop** resolves at 5 trial tickets: keep the advisory review call iff **≥1 true
  positive AND 0 false VIOLATES** across the window; else DELETE the call. Ledger entering the trial:
  +6 clean samples, 0 TP opportunities, 0 false VIOLATES — as written, 0 TP at window end = drop.
- **Daily-driver evidence target:** ≥5 real tickets with complete rows. The graduation judgment on the
  numbers is Gary's, not this document's.

## Column definitions

| column | meaning |
|---|---|
| operator-min | honest wall-clock estimate of ALL operator work: probing, oracle authorship, workpad correction, align rounds, adjudications, landing |
| worker $ / turns / attempts | from run telemetry (nominal, subscription-credited) |
| catches | gate refusals that were RIGHT (oracle_intact, harden red, verify red/floor, fork-point guard) |
| misses | defects discovered AFTER land that a gate should have caught — the integrity number |
| bounces | additive-retries + reworks, with one-word cause |
| reviewer | verdict/conf + adjudication: TP / FP / honest-abstain |

## Rows

| ticket | kind | title | operator-min | worker $ / turns / attempts | catches | misses | bounces | reviewer |
|---|---|---|---|---|---|---|---|---|
| t6 | build | notification pipeline: violation → Streams → email + delivery log (landed `f55fa6d`) | ~95 (45 pre: probe/KB/oracle/workpad; 50 mid: harden RCA, docker env, 2 oracle-plumbing bugs) | $9.81 / 100 / 1 (worker first-attempt clean) | oracle_intact clean; verify honestly bounced 3× on env/oracle-plumbing (daemon down, container-name conflict, oracle crash) — no faked rows; harden 0.845 after RCA | none known | 0 worker bounces; 3 verify env-bounces; 2 harden false-starts (harness finding) | SUSPECT/med — honest-abstain (truncated diff; its pycache flag was real but pre-existing t5 debt) |
| t7 | build | core-api violations read API: filters + cameras + camera_name + trends (landed `ac88f2b`) | ~75 (50 pre 07-15: probe agents, oracle authorship + smoke, contract amendments, workpad; 25 on 08-02: timeout diagnosis, operator-run gates, land) | $? / 0 recorded / 1 dispatch (wall-clock timeout ATE the result event — 615KB stream, work complete+committed; cost unrecorded, telemetry gap) | run honestly parked as timeout instead of faking done; harden vacuous-loud 1.000 (16 stamped files — finding #2 pattern, predicted); verify PASS first run: oracle 7/7 phases from clean stack | none known | 1 — wall-clock (2400s too tight for cold-worktree ticket; work was complete, only the done-declaration missing; operator ran gates directly, no re-dispatch burned) | SATISFIES/high — correct (+1 clean; running total +8, 0 TP, 0 false-V; window 2/5) |
| t10 | build | e2e slice: full-stack integration, both ingest paths + mail + UI (landed `e6924a7`) — **WINDOW CLOSES 5/5** | ~35 (20 oracle authorship; 10 workpad; 5 land dance) | $4.74 / 62 / 1 — first-attempt clean; found+fixed a REAL integration gap (upload→inference trigger permanently failed jobs on a boot-order race) | land refused a dirty base (test.db churn — honest floor, operator-caused); harden vacuous-loud (4 stamped files); e2e oracle green: 12 services, video path, physical mail, live RTSP, UI | none known | 1 land-refusal (operator test.db churn, stashed); 1 teardown burr (compose project t10 vs base — finding #3 pattern again) | SATISFIES/high — correct. **WINDOW TALLY: 5 tickets, +12 clean samples total, 0 TP opportunities, 0 false VIOLATES** |
| t9 | build | streaming-gateway RTSP ingest: camera-manager + 640p10fps substream + snapshot + Redis tap (landed `4280d4b`) | ~50 (30 staging: fixture profile, §5 amendment, oracle; 10 gates/review/land; 10 oracle binary-bug fix) | $11.02 / 122 / 1 — **first-attempt clean completion** (pytest-only validation kept the worker inside the clock — the t7/t8 lesson applied) | **harden mutated FOR REAL first time this trial: 0.745 ≥ 0.70 PASS (388 caught / 133 missed)** on unstamped handwritten Python; oracle discriminators all held (frames-differ, tap-at-cap, 404) | none known | 2 oracle-plumbing (mine): verify verb refuses re-run in review band after validation amend (finding #5); redis_cmd strict-UTF8 crash on binary jpeg field. 0 worker bounces | 1 transient ERR (DeepSeek drop), retry SATISFIES/high — correct (+10 total, 0 TP, 0 false-V; window 4/5) |
| t8 | build | web-frontend violations view: Angular 21 list/filters/detail + vitest setup (landed `33ef1a5`) | ~55 (25 oracle authorship; 10 workpad; 15 oMLX-port RCA; 5 land/hygiene) | $? / 0 recorded / 1 dispatch (timeout telemetry gap AGAIN — 1MB stream, feature complete incl. vitest 27 specs + testid contract; worker burned clock running the docker oracle inside its own loop) | verify honestly FAILED round 1 on env (oMLX absorbed loopback :8000 — false-positive health, then upload 404); harden's LYING vacuous note observed live (finding #4 in production: ".ts changed but 'no code files changed'") | none known | 1 env — oMLX:8000 loopback absorption; fixed on base: core-api published on 28000 (redis-16380 doctrine), all 4 oracles repointed | SUSPECT/low — honest-abstain (guard-placement concern refuted by route read; oracle empirically proved no-auth reachability) (+9 total, 0 TP, 0 false-V; window 3/5) |

## Session notes

- 2026-07-15: ledger created before ticket 1 (plan approved; keystones per-ticket, NOT pre-authorized —
  keystone interactions are part of the operator tax being measured).
- 2026-07-15 (t6): reviewer ledger running total: **+7 clean samples, 0 TP opportunities, 0 false
  VIOLATES** (trial window: 1/5). Harness findings filed from t6 (fix later, not mid-trial):
  (1) `harden_python` conflates the validation command with cosmic-ray's per-mutant test-command —
  cosmic-ray `shlex.split`s with NO shell, so `&&`-chained validation passes `&&` as a literal pytest
  arg (baseline "collected 0"); expensive full-stack oracles in validation are also structurally wrong
  for the mutant loop. Candidate: separate `test_command` field or auto-strip to the first `&&` segment.
  (2) provenance fs-walk stamps HANDWRITTEN new files under template-stamped service dirs — t6's
  mutation scope collapsed to the one root-level file (23 files excluded); granularity: stamp should
  bind to files existing at instantiation, not to dirs forever. (3) oracle leave-stack-up-for-post-mortem
  + fixed compose `container_name:` collide across worktree/base compose projects — next ticket's oracle
  can't boot until the previous stack is torn down by ITS project name. Operator lessons stored on the
  PDSI board: pytest multi-service collision (m1784110354400). Environment burr: Docker daemon must be
  up before verify (obvious in hindsight; cost one bounce).
- 2026-08-02 (WINDOW CLOSED — 5/5): **Reviewer fix-or-drop RESOLVES per the pinned rule: DELETE.**
  Rule ("keep iff ≥1 TP AND 0 false VIOLATES") evaluated on the window: t6 abstain, t7 correct-SATISFIES,
  t8 abstain, t9 correct-SATISFIES (+1 transient ERR), t10 correct-SATISFIES → 0 false VIOLATES but
  **0 true positives in 5 real tickets** (every verdict was either an abstention or confirmation of what
  the machine gates already proved). The call gates nothing → delete the automatic advisory-review step
  from the sprint flow (execution = B6 in the hardening slice; the `agent review` verb stays available
  for manual use). Successor design queued: cross-vendor reviewer (Omnigent teardown C2/C3, on its
  trigger — which this DELETE is). Findings tally for the run: #5 verify-verb refuses re-run in review
  band (validation amended post-verify has no path); #3 pattern recurred twice (teardown must use the
  WORKTREE's compose project name); worker-timeout pattern solved operationally (pytest-only validation
  during run/harden + oracle appended at verify — worked 3/3 after t7/t8 losses).
- 2026-08-02 (t7 landed): two run findings. (a) **worker wall-clock telemetry gap**: on timeout the
  result event never arrives, so turns/cost report as 0/? even after 40 min of real work — telemetry
  candidate: accumulate turns/cost from stream events, not only the final result. (b) 2400s is too
  tight for cold-worktree multi-service tickets — run continues with HARNESS_CLAUDE_TIMEOUT=3600 from
  t8 on (operator env choice, disclosed). Also: worker committed `core-api/test.db` binary churn
  (tracked since t5 — pre-existing hygiene debt, filed not fixed).
- 2026-07-25 (long-horizon run start): **INSTRUMENT DEVIATION, disclosed + Gary-authorized:** tickets
  t7–t10 + the post-trial B hardening slice run under **batch keystone pre-authorization**
  (AskUserQuestion, 2026-07-25: full align+land pre-auth + standing commits for run scope). Per-ticket
  keystone interaction is therefore NOT part of measured operator tax this window (columns unchanged;
  operator-min reflects batch-mode operation). Pinned run guardrails: park on genuine product forks
  (batch questions at checkpoints), max 2 additive-retries per ticket then park, land only all-gates-green
  from a clean stack, ticket parks after 2 dispatches.
- 2026-07-15 (session 2 pre-flight): **harness finding (4)** — harden vacuous-passes JS/TS-only diffs:
  `main.rs:2184` dispatch recognizes only `.rs`/`.py`, so a frontend ticket's harden row is a vacuous
  1.000 PASS AND the note LIES ("no code files changed" when `.ts` files DID change — `stamped_code_count`
  at main.rs:2261 also only counts .rs/.py). Wrong-green class, found by probe BEFORE it bit. Gary's
  ruling (AskUserQuestion): file-only, no mid-trial fix; for the frontend ticket this is known doctrine
  (like template vacuity) — oracle + tests are the real gate. Post-trial candidates: honest skip-message
  naming the unmutatable extensions (small); optional Stryker JS backend (bounded-module pattern,
  decisions.md:1155). Also probed this session: "web-frontend violations view" split into **t7 (core-api
  violations read API — Python, mutation gate meaningful) + t8 (Angular view — harden vacuous-known)**;
  streaming-gateway renumbers t9. Frontend ground truth: Angular 21 + PrimeNG (services.yaml wrongly
  says React), no test runner installed, core-api has NO auth endpoints (auth deferred by ruling,
  contract drift to be filed on base).
- 2026-08-02 (A6 — GRADUATION): **Gary ruled on the window's numbers: GRADUATED.** The daily-driver
  claim the instrument existed to test is now held: ≥5 real tickets with complete rows, 0 known
  misses, 0 faked rows. This ledger's trial window is closed; the instrument survives into the next
  window with the two filed upgrades (upfront-estimate column, comparator arm) before any new
  measured claim. Decision record: decisions.md "A6: DAILY-DRIVER CLAIM GRADUATED".

---

# cv-mapper block — disclosed, NOT a window (2026-08-06 → 08-13, mined 08-16)

> The first post-graduation project driven on an outside codebase. It was **not run as a measured
> window** — no rows were kept at the time, no estimates were made, and no decision rule was pinned
> before it started. These two rows are a **post-hoc reconstruction** from the session transcript,
> written by the trial's own analysis (`wiki/cv-mapper-trial.md` §Agent 6) and reproduced here in the
> Window-1 schema, unmodified.

**Three reasons these rows do not join Window 1's tally, stated before the rows so they are not read
past:**

1. **Different stream.** Every Window-1 row is PDSI continuation. cv-mapper is a different codebase and
   a different domain (browser automation vs a multi-service stack), and it ran **after** the window
   closed. Mixing them into one tally would restate a closed graduation on evidence it did not use.
2. **Different measurement method.** Window-1 `operator-min` figures are contemporaneous estimates
   written at land time. cv-mapper's are transcript clustering done ten days later. The column
   definition names no counting rule, so neither is wrong — and they are not the same instrument reading.
3. **Under-powered against the only pinned bar.** 2 tickets against `≥5 real tickets with complete
   rows`, and one of the two cannot produce a complete row at all. **No threshold for a cv-mapper-shaped
   run was ever stated, and none is invented here.** These rows falsify nothing in A6; they narrow it
   in one place (the native arm — see the Window-2 rules below).

| ticket | kind | title | operator-min | worker $ / turns / attempts | catches | misses | bounces | reviewer |
|---|---|---|---|---|---|---|---|---|
| **t1** (cv-mapper) | build | Windows deployment probe: 5-check CDP/Playwright preflight + `run_probe.bat` (landed `baabd0d`) | **~23** (13 hands-on: contract authoring, 128-line oracle, 429 adjudication, runaway kill; 10 keystone interaction). *Column pins no counting rule; this is attended-session clustering at a 10-min gap — the same session reads 88 min at 5-min and 392 at 20-min* | **$11.3732 / 124 / 3 dispatches** (completed 33t · 429-error 20t · completed 71t), all `claude-cli` on `claude-opus-5[1m]`. *Cost is not board state — it survives only in the teed CC `stream-json`; `run` has no cost column* | **2 TRUE.** mutation **0.362 < 0.70** (462 survivors) → additive note → 1,456 test lines → 3 failures adjudicated → **1 real silent-wrong-answer bug fixed** (`_major_version("Chromium 120…")` returned `120`). Then harden **refused to score at all** on a red baseline — correct, and left **no row anywhere**. Final 0.781; `tests_green` first try | **0 found / thin window.** No commit after `baabd0d` touches any t1 artifact. Landed probe was zipped and **executed in its target environment by the recruiter 3 days post-land — 5/5 checks pass, no patch** | **3** — *rate-limit* (work preserved by the harness WIP commit `233211b`); *mutation-red* (1 additive re-dispatch); *harness-caused runaway* (4h25m, 19,414 mutants: `commit_worktree`'s `git add -A` swept the worker's tracked scratch onto the branch). Plus 1 dispatch attempt that opened no `run` row | **n/a** — the automatic advisory call was DELETED 2026-08-02 on the pinned fix-or-drop rule. A decision being honoured, not a step skipped |
| **t2** (cv-mapper — **worker not harness-dispatched**) | build | LinkedIn selector resolution: read-only DOM probe + `resolve_target` (landed `6528be8`) | **~64 attended, of which ~20 harness mechanics** (~44 was cv-mapper product work). **Excludes an 8h52m land-keystone wait** (overnight, unattributable) | **Harness: $0 / 8 native iters / 2 dispatches → 0 landed lines.** One orphan (`agent sprint --help` executed as a real sprint) and one `max_iters` at the compiled default 8, 54s, **12 tool results all reads**. **Every landed line was written by an operator-spawned CC Task subagent** the board never saw — 282 turns, cost unrecoverable. **This cell cannot be filled in the column's own terms: the column reads run telemetry, and the run telemetry describes work that produced nothing** | **1 TRUE, 1 FALSE POSITIVE.** mutation **0.600 < 0.70** (376 survivors) → **+1,304 test lines, 92 tests, 1 real bug** → 0.855. **Not gamed**: 77 survivors provably equivalent under PEP 563; the worker computed the ~0.918 ceiling and refused to rewrite the annotations. `oracle_intact` **INTEGRITY VIOLATION = FALSE POSITIVE** — fired on the operator's own approved amendment already on master (`d9c8a31`), cleared 34s later by `git merge master` **with no byte changed** | **0 found / window thinner than t1's.** The landed module **has never been executed since it landed**, in a DOM the project's own D2b records as changing continuously | **4** — *cli-defect* (`--help` dispatched a real sprint); *iters-cap + wrong-plan*; *mutation-red* (0.600→0.855); *gate-false-positive* (verify → `git merge` → verify) | **n/a** — same deletion as t1 |

## cv-mapper block — session notes

- **2026-08-17 — INSTRUMENT DEVIATION, disclosed.** Two mid-ticket amendments happened on t2 that the
  pinned schema has no cell for, recorded here on the instrument's own existing precedent
  (the 2026-07-25 batch-keystone entry) rather than by adding a column mid-instrument:
  **(a) the oracle amendment** — `check_probe_readonly.py` 162→316 lines on 08-10, operator-authored,
  user-approved, measurement-driven (three scopes matched zero elements), committed to master and
  copied into the worktree. It is **not** "modifying success criteria to fit the result": it narrowed
  the property against live measurement and made it *harder* to satisfy accidentally. But the only
  thing distinguishing it from the prohibited move is prose in a transcript, **and that is the finding**.
  **(b) the criteria amendment** — AC-P1.7 added 08-10, four days after t2's human `criteria_confirmed`
  row, which therefore certifies a text that no longer exists. This has no record at all.
- **(b) becomes the third next-window instrument debt**, alongside A6's two: a **criteria digest in the
  `criteria_confirmed` gate note**, so a future row can prove which text a human confirmed. Until that
  exists, every `catches` and `operator-min` cell is measured against a contract the instrument cannot
  reproduce. → scheduled as Window-2 ticket **h-t2**.
- **Evidence preservation, 2026-08-17.** Both red trees survived only as dangling git objects, one
  `git gc` from deletion. Now tagged in the cv-mapper repo as `trial/t1-red` (`696d306`) and
  `trial/t2-red` (`8516ee6`). t1's two catches are not in `.harness/` at all — those gates ran in the
  foreground and left no log.

---

# Window 2 — harness self-repair (OPEN)

> Stream: **this repo** (`~/Documents/Work/harness/`), board `~/Documents/Work/harness-board.db`,
> tickets `h-t1`…`h-t10`. Columns and decision rules pinned **2026-08-17, before ticket 1**.
> Scope and sequencing: the approved plan (cv-mapper trial repairs).
>
> **A6's two filed upgrades land here, which is what unblocks this window at all:** the `est-min`
> column (below) and the comparator arm (below). `A6` recorded them as owed *"before any new measured
> claim"*.

## What this window is testing

Window 1 tested the daily-driver claim; it graduated and is not re-opened. Window 2 tests the three
things the cv-mapper trial left standing:

1. **Does the record fix work?** (the trial's headline failure: no red gate survives on the board)
2. **What is the operator tax, against a baseline?** (A6 left this NOT ESTABLISHED for want of a comparator)
3. **Does the native oMLX arm produce anything on a real ticket?** (two capped dispatches produced zero)

## Decision rules (pinned up front)

- **R1 — record preservation resolves at window end.** h-t1 HOLDS iff **≥1 red gate occurring in this
  window is reconstructable from the board alone** — no git archaeology, no `.harness/` log file. If
  **zero** red gates occur across the window, the verdict is **UNDEMONSTRATED, not PASS**, and a
  deliberate red is constructed on a scratch ticket to settle it. A fix that nothing tested is not a
  fix that worked.
- **R2 — operator tax is REPORTED, not decided.** Arm A (on-board) and arm B (off-board) operator-min
  are reported side by side with n stated. **No threshold is pinned, deliberately: n=2 in arm B cannot
  support one, and a ratio quoted from it would be false precision** — exactly the error the cv-mapper
  trial caught in its own first draft, where a 3.7× figure turned out to be comparing parallelism to
  sequencing. Any future claim about the tax must cite the n.
- **R3 — estimate calibration resolves at ≥5 complete rows.** `est-min` is written **at align, before
  dispatch, and never revised**. Report actual ÷ est per ticket and the window median. A revised
  estimate voids the row's calibration value; say so in the row rather than editing the number.
- **R4 — the native arm resolves on h-t10, one dispatch.** Dispatched via `agent run h-t10` on the
  native oMLX loop at the new 40-iteration default. If it produces no landed lines, the daily-driver
  claim is **scoped in `decisions.md` to the delegated-Claude arm**, on this evidence rather than on
  cv-mapper's two capped dispatches. One dispatch is the whole rule — no "try again with a better
  ticket", which is how a null result gets talked out of existence.

## Column definitions (Window 2 — 9 inherited + `est-min`)

| column | meaning |
|---|---|
| **est-min** | **NEW.** Upfront estimate of total operator minutes, written at align **before dispatch**, never revised. Owed since 2026-07-25 (delivery-lens instrument gap); filed again at A6. |
| arm | **NEW.** `A` = on-board (board + gates + worktree + delegated worker). `B` = off-board comparator (interactive CC, no board, no gates, straight to master). |
| operator-min | honest wall-clock estimate of ALL operator work: probing, oracle authorship, workpad correction, align rounds, adjudications, landing. *Unchanged from Window 1.* |
| worker $ / turns / attempts | from run telemetry. **From h-t3 forward this is board state** (`run.cost_usd`), not transcript archaeology. |
| catches | gate refusals that were RIGHT |
| misses | defects discovered AFTER land that a gate should have caught — the integrity number |
| bounces | additive-retries + reworks, with one-word cause |
| reviewer | **n/a for this window** — the automatic call was deleted 2026-08-02 on the pinned fix-or-drop rule. Column retained so the schema stays comparable; `agent review` may still be invoked manually and would be recorded here. |

## The comparator arm (pinned before ticket 1)

**Design.** Two of the nine planned tickets are built in **arm B**: interactive Claude Code, no board,
no gates, no worktree, landed straight to master, timed the same way. The two are **B1**
(`DEFAULT_MAX_ITERS` 8→40 + surface the stop reason) and **B2** (memory writer on red gates) — chosen
before any ticket ran, one small and one medium, both same-repo and comparable in size to the arm-A set.

**Two limits, disclosed before the arm runs so they cannot be read as excuses afterward:**

1. **n=2 is under-powered.** This arm cannot produce a significance claim. It produces one honest
   comparison of like-sized work in the same repo in the same week, which is strictly more than the
   zero baselines that exist today. See R2.
2. **The arms differ in worker, not only in process.** Arm A is a delegated Claude worker inside gates;
   arm B is interactive Claude driven by the operator. That is the counterfactual the tax question is
   actually about — what the operator would otherwise do — but it **confounds worker with process**, and
   no arm-B row may be read as isolating the gates' cost.

## Rows

*(filled at land/close time — one row per ticket, arm A and arm B in the same table, `arm` column distinguishing)*

| ticket | arm | kind | title | est-min | operator-min | worker $ / turns / attempts | catches | misses | bounces | reviewer |
|---|---|---|---|---|---|---|---|---|---|---|
| **t15** | A | build | board: preserve gate history — a red gate must survive the green re-run that fixed it (landed `5d5168f`) | **45** (20 spent + 25 forecast; only the 25 is a genuine forecast, see the window conventions) | ~40 (20 workpad authoring + adversarial-review adjudication, which killed the first design; 1 align; ~15 result adjudication incl. migrating a copy of the live board to check it; ~2 land) | **$5.17 / 48 turns / 1 attempt** — first-attempt clean, `claude-cli`, 3600s/60-turn budget (disclosed env choice) | **none — no gate refused.** harden 0.833 PASS (35 caught / 6 missed / 1 timeout / 2 unviable), verify PASS first run, oracle_intact clean. Adjudication that mattered: AC7 required the three named tests to pass UNMODIFIED and they have zero diff mentions, so `verify_reruns_a_review_band_ticket` holds under the new latest-wins reader — the outcome the review predicted | none known (landed 2026-08-18) | **0** | n/a — automatic advisory call DELETED 2026-08-02 on the pinned fix-or-drop rule |

## Window 2 — session notes

- **2026-08-17 (window opened, before ticket 1):** instrument upgraded first, per A6's *"before any new
  measured claim"*. Added `est-min` and `arm`; pinned R1–R4 and the comparator design above. Window 1's
  rows, rules and column definitions were **not** touched — a closed instrument is not edited. The
  cv-mapper rows were appended as a disclosed block, not as t11/t12.
- **Planned scope (pinned so it cannot drift), with the board ids assigned 2026-08-17:**
  arm A = **t15** preserve gate history · **t16** strict argv · **t17** oracle content provenance ·
  **t18** run cost column · **t19** criteria coupling + align render · **t20** worktree scratch
  exclusion · **t21** off-board-work detector, on the live board
  `~/Documents/Work/harness-board.db` (the repo-root `harness-board.db` is the stale June kata board —
  same trap as the 2026-07-15 sprint). Arm B = `B1`, `B2`, off-board by design and therefore
  deliberately absent from the board. Blocking edges: everything blocks on t15; t20 also blocks on t17.
  Adversarial review at design is mandatory for **t15, t17, t19** (decisions.md process gate).
  Anything added to this list mid-window is recorded here with its reason.
- **Two measurement conventions, pinned now because both were discovered while opening the window and
  both would otherwise be settled after the fact — which is how a number gets chosen to fit a result:**
  1. **`est-min` is recorded on the ticket's `notes` field before dispatch**, not held in a session.
     That puts it on the board with a `workpad_edited` event behind it, so a later revision leaves an
     audit trail instead of being invisible. Each estimate is written as
     `total (X already spent authoring the workpad + Y forecast)`. **Disclosure: for this window the
     estimates were written after workpad authoring**, so only the `Y` half is a genuine forecast and
     only `Y` may be used for R3 calibration. From Window 3 the estimate is written before authoring.
  2. **`operator-min` counts attended wall-clock**, as in Window 1 — not agent-execution time. A
     session where the operator reads while an agent works is attended; a land-keystone wait overnight
     is not, and is reported in the row's own cell rather than folded into the number (the cv-mapper
     block's treatment, adopted).
- **Fork decided forward (2026-08-17):** `agent draft` was **not** run on any ticket in this window;
  all seven workpads were operator-authored directly. Reason: three source explorations produced
  file:line-precise designs before drafting began, and the last three recorded drafts were all
  rewritten in the next turn — running the drafter to overwrite it is ceremony. **Consequence for the
  record:** `agent draft`'s acceptance bar (*"Align would be one-round, not a rewrite"*,
  `decisions.md:1246-1249`, calibrated at n=1 and falsified at n=2 and n=3) gains **no** new samples
  from this window and stays at n=3.
- **Keystones are PER-TICKET this window (Gary, 2026-08-17).** Every `align` and `land` comes back to
  the operator. This is deliberately the Window-1-tickets-1-and-2 posture, not the batch pre-authorisation
  that tickets t7–t10 ran under and had to disclose as an instrument deviation
  (`trial-ledger.md` 2026-07-25 note). **Keystone interaction is therefore INSIDE the measured operator
  tax this window** — which matters, because the tax question is the thing Window 2 exists to answer and
  a batch-mode number cannot be compared against a per-ticket one.
- **Operator env choice, disclosed:** `HARNESS_CLAUDE_TIMEOUT=3600` and `HARNESS_CLAUDE_MAX_TURNS=60`
  for every arm-A dispatch, against compiled defaults of 900s / 40 turns
  (`crates/agent/src/worker.rs:25,31`). Reason: these are Rust tickets in a cold worktree, where a
  from-scratch `cargo build` plus `cargo test --workspace` dominates the clock before the worker writes
  a line. Window 1 hit exactly this and moved to 3600s from its ticket t8 onward. Recorded here rather
  than left in a shell so the turns/cost cells can be read against the budget that produced them.
- **Adversarial review outcome, all three (2026-08-17): DON'T-SHIP-AS-DESIGNED, three for three.**
  Every reviewed design was rewritten before a line was written, and each verdict was independently
  reproduced by the operator rather than taken on report:
  **t15** — the premise was false. The reviewer removed `report_gate`'s upsert, widened the PK and
  changed four lines of `gate_satisfied`, and all 247 tests passed including the two the design claimed
  depended on the overwrite. Shipped instead: one append-only store. Independently confirmed that
  `run_close` passes `passed=true` unconditionally, so its cited "self-healing window" is about note
  freshness, not verdict inversion.
  **t17** — the design was a **security regression**. Reproduced on git 2.51: `git branch -f <base>`
  refuses from inside a linked worktree but **`git update-ref refs/heads/<base>` succeeds, rc=0**, so
  anchoring on base-branch-tip content hands the worker the anchor, and the demonstrated attack passes
  the proposed predicate while today's three-dot predicate flags it. The redesign also closes two holes
  that **pass today**: rename-to-evade (`git mv` a protected oracle — the path vanishes from
  `--name-only`, verified grep count 0, while `ls-tree` of the base still lists it) and
  `--skip-worktree` (edit invisible to `add -A`, `status`, and both diffs).
  **t19** — the two halves cancelled: the digest detects "criteria changed since confirmation" and the
  forced re-align refreshes it to match. Plus a partial-apply bug (`set_workpad_field` writes, then
  `set_status` bails on an illegal edge, with no transaction anywhere in `board.rs`) and a direct
  contradiction of `run_review`, which instructs the operator to edit criteria on review-band tickets.
  Shipped instead: enforce at the `land` keystone by comparing the criteria text, warn everywhere else.
  **Correction on the record:** the t19 review was briefly logged as *substituted* after the reviewer
  went idle twice. It landed. The operator's own trace had reached the right answer on the layering
  question and **missed all three blockers**.
- **2026-08-18 (t15 landed `5d5168f`) — R1 status: NOT YET SETTLED, and recorded as such.** t15 shipped
  clean with no gate refusal, so the window has produced no red gate yet and the mechanism is proven by
  its own 8 unit tests (including the cv-mapper replay case) plus a live migration of a **copy of the
  real board** — v2→v3, 77 rows preserved, `seq` added, `created_at` intact — but **not** by a red
  surviving a green re-run on real data. R1 resolves at window end across the remaining six tickets. If
  none goes red, a deliberate red gets constructed on a scratch ticket; the fix is not scored as working
  because nothing tested it.
- **What t15 cannot recover, stated so it is not over-read:** every gate verdict destroyed *before* this
  landed is gone permanently. `agent show t6` now renders a Gates section showing `mutation_score PASS
  1.000` — its 0.400 red, which the wiki records and which drove a real additive retry, is absent and
  unrecoverable. Append-only starts now; it does not reach backwards.
- **Minor finding filed, not fixed (Wu Wei):** `harden` on PASS prints only the score, so t15's 6
  surviving mutants are not retrievable — the `details:` line is emitted only on the below-threshold
  path (`main.rs:2249-2258`). A pass with survivors says nothing about what they are. Same family as the
  visibility complaints the cv-mapper trial catalogued; candidate for a later ticket, not this window.
