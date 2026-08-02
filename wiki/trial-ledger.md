# Trial Ledger — Measured Daily-Driver Trial (critical-path item 4)

> The instrument for the daily-driver claim. Stream: **PDSI continuation** on
> `~/Documents/Work/client/Accelerator/harness-run/` (board: `../harness-board.db`, tickets t6+).
> Columns and decision rules were pinned **before ticket 1** (2026-07-15) and are never adjusted to
> fit results (integrity constraint). One row per ticket, filled at land/close time.

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
