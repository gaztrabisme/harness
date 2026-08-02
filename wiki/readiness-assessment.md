# Readiness Assessment

# Tri-lens suitability assessment — 2026-07-25

> Gary's ask: assess the harness's suitability as the daily project driver through three skill lenses —
> solution-architect (is the architecture defensible?), delivery (can it run an engagement?), dev
> (is the engineering sound?). Method: coordinator-direct synthesis over live evidence already in hand
> (trial ledger 1/5, both dogfood boards, this session's probes). KB gate: skipped, stated — own-process
> tooling, not a KB-covered domain. Context trigger: the Omnigent question ("should we design a
> meta-harness instead of the MCP wrapper?") — dispositioned under the SA lens below.

## Verdict (all three lenses)

**Suitable as the daily *project* driver — with one structural caveat each lens independently surfaces:
the mutation tier of the gate stack only earns its keep on hand-written Rust/Python; everywhere else
(template-stamped dirs, JS/TS, doc kinds) the hand-authored oracle carries verification alone.** That is
documented doctrine, not a defect — but it means "the gates" is really "the oracle discipline," and the
operator tax of oracle authorship is the load-bearing cost. The 2026-07-15 verdict (project driver yes,
ad-hoc no, by posture) stands unchanged; the trial (1/5) remains the right instrument and should run to 5.

## SA lens — architecture defensibility

- **Problem-before-solution: satisfied.** The problem (process enforced by code, CC contained, memory
  compounding) predates the build; requirements (forever-personal, local-first, Rust-native) are explicit
  in decisions.md with rejected alternatives recorded — SA's decision-record discipline is met unusually well.
- **Options analysis: current as of 2026-07-25.** Omnigent (omnigent-ai/omnigent, 7.9k★) is the "buy"
  option that did not exist at the Strategy-A decision. Disposition: it validates the contain-don't-compete
  thesis rather than displacing the build — its collab/any-device platform half is our recorded tripwire
  (CC-like UX on a personal budget), and our hard requirements (keystones as terminal-only human gates,
  oMLX-local roles, forever-personal) diverge from its direction. Mine designs, not runtime: (a) worker-
  adapter shape for a second-vendor worker, (b) cross-vendor reviewer routing — the latter slots exactly
  into the reviewer fix-or-drop hole if the DELETE fires at trial end. Bounded teardown (research/17
  candidate, DR1/DR2 shape) on trigger: trial end or reviewer resolution. MCP wrapper stays Wu Wei-parked —
  it solves ergonomics (conversational front-end), not architecture, and the operator terminal already is one.
- **NFR posture (the SA contribution):** integrity/reliability strong (gates as DB rows, fail-closed
  guards t8/t9, oracle protection, provenance — each with a live catch behind it). Security acceptable-
  for-personal and documented (prompt-level worktree jail for the CC worker = accepted residual;
  `--dangerously-skip-permissions` = same trust as interactive CC; board DB writable by any local
  process = convention not enforcement; DeepSeek key rotation still owed). Observability good (telemetry
  rows, trajectory JSONL, ledger). Language coverage is the real NFR gap — see the shared caveat.
- **Traceability: adequate.** PDSI tickets carry REQ back-references; the harness's own critical path is
  tracked in this file. No silent gaps found.

## Delivery lens — can it run an engagement?

- **Baseline discipline: structurally enforced.** Workpad criteria pinned at align = frozen per-ticket
  scope; trial-ledger columns pinned before ticket 1 = instrument frozen before measurement. Delivery's
  first principle ("baseline is the promise") is a *mechanism* here, not a habit.
- **Progress measurement: the harness's strongest delivery property.** Gate rows are 0/100 acceptance-
  style measurements; keystones (`criteria_confirmed`, `landed`) are the client-signature analogue —
  Human-source rows the worker cannot self-clear. Percent-complete theatre and acceptance-without-
  evidence are designed out, not policed out.
- **Change control: informal but disclosed.** Mid-flight amendments live in notes/breadcrumbs with
  disclosure (t6's over-broad constraint amendment; t7's presign-descope). No re-baseline record per se;
  at personal scale the ledger's bounce column carries it. Adequate — watch it if ticket counts grow.
- **Two instrument gaps (delivery-lens findings, both deferred to the NEXT trial window — the current
  window's columns are pinned and mid-window instrument changes are their own integrity violation):
  (1) no upfront operator-effort estimate per ticket, so variance-vs-promise is incomputable (only
  absolute actuals exist); (2) no comparator arm — nothing measures what the same ticket costs in plain
  interactive CC, so the graduation judgment compares against recall, not data.** Both are noted for
  Gary's graduation judgment, not retro-fitted.

## Dev lens — engineering health

- **Sound.** 233 tests green post-land, clippy clean, the harness mutation-tests itself, self-hosting
  sprints run through its own spine with first-attempt-clean majorities; zero unexplained red.
- **Findings backlog (4 filed, none load-bearing-broken) shares one root:** the harden tier degrades
  outside hand-written Rust/Python — (1) cosmic-ray test-command conflation (`&&`/shlex), (2) provenance
  dir-stamp swallowing handwritten files, (4) JS/TS vacuous-pass with a lying note; plus (3) the
  container_name/compose-project collision in oracle ops. Items 1/2/4 are the same story the shared
  caveat names. Post-trial slice candidates, ranked: honest-skip message for #4 (small), dir-stamp
  granularity #2 (medium), test-command split #1 (medium), Stryker backend (only if frontend work
  becomes a recurring stream).
- **Reviewer: +7 clean, 0 TP, 0 false-VIOLATES — on track for DELETE at ticket 5 as pinned.** The
  cross-vendor reviewer design (Omnigent steal) is the queued successor if the slot opens.

## What this assessment changes

Nothing structural — it confirms the posture and sharpens the risk register. Actions carried: continue
trial (t7 prepped, awaiting align); Omnigent teardown filed as research/17 candidate on trigger; next-
window ledger gains estimate column + (Gary's call) a comparator ticket; post-trial harden slice ranked
above. The daily-driver claim still graduates on the trial numbers, not on this document.

# Re-assessment — 2026-07-15

> Re-scores the 2026-06-30 verdict (below, preserved) against live evidence: the critical path it prescribed
> was built (742b8dd draft → 45588c6 claude worker → 9c16509 sprint/edge), then exercised on a **real
> multi-ticket project** (PDSI-from-docs, 5/5 stages landed incl. a working vertical slice on the real
> production ONNX) and a **self-hosting sprint** (3 harness gate fixes built by CC workers through our own
> spine). Every claim here has a commit SHA or a gate row behind it.

## New TL;DR

- **June's "not ready" verdict is retired.** All three critical-path items are built AND validated on real
  work. The harness ran an ambiguous, multi-file, multi-ticket project end-to-end with delegated CC workers;
  every gate fired live at least once and held.
- **New verdict: ready as the *project* driver, not the *ad-hoc* driver — and that split is the posture,
  not a shortfall.** Per "contain CC, don't compete" (decisions.md 2026-07-14): work worth gating (multi-
  ticket, artifact-gated, irreversible-if-wrong) goes through the harness with CC contained as worker;
  quick reversible ad-hoc work stays interactive CC. The June question "replace Claude Code?" was the wrong
  axis — the right claim is **"the harness is now the enforcement layer CC works inside, on real projects."**
- **The measured cost that remains is the operator tax, and it is partly load-bearing.** Per PDSI ticket the
  operator (Claude-in-CC or Gary) probed contracts, authored oracles, adjudicated bounces. That tax IS the
  quality mechanism (t5's fixture-grounded oracle is *why* a broken decode couldn't pass) — reduce its
  accidental part (grounding, ergonomic burrs), never its essential part (oracle authorship, keystones).

## What the live runs proved (evidence ledger)

| June gap | Status | Live evidence |
|---|---|---|
| No intake/drafting | **Built + used** | `agent draft` on every PDSI ticket; 1 defect class found: drafted ACs hallucinated example filenames (t1) → lesson stored, fix ranked below |
| Weak worker | **Built + used** | `claude -p` worker did whole tickets (t5: 123 turns, 3 services, tests green); outcome mapping + timeout + telemetry all exercised; ~$50 nominal total, subscription-credited |
| No orchestration | **Built + used** | `agent sprint` drove the 3-ticket self-hosting sprint; park-and-continue + additive-retry (notes + same-worktree re-dispatch) validated twice (h-t1 0.500→1.000, t5 oracle_intact bounce) |
| Code kinds only | **Worked in practice** | research/design kinds (t1–t3) went through the spine gated by hand-authored protected `check_*.py` oracles; per-kind gate *profiles* still unbuilt — Wu Wei: not blocking, doctrine is "the oracle carries it" |
| Integrity enforcement (untested claim) | **Fired live** | `oracle_intact` caught a real worker edit of frozen tests (t5, benign, correctly refused); worker respected the oracle boundary after t2's protection extension; keystones never self-cleared |

## New weakest links (ranked, impact ÷ effort)

1. **Draft grounding (accidental operator tax).** Drafted ACs hallucinate file lists; the fix is mechanical —
   feed tree/`ls` reality into the draft prompt and cross-check named paths exist at draft time, flag the rest.
   Hits every ticket. Small.
2. **Oracle-freeze granularity.** t5 burned a retry dispatch (~$3, ~20 min) on a benign restructure because
   ALL base `test_*.py` are frozen. Candidate: exempt `.template-stamp.json`-stamped test files (symmetric
   with the provenance filter) — template placeholders are boilerplate, not operator oracles. Small; the
   stored lesson ("new tests in NEW files") is the interim mitigation.
3. **No mid-run confusion bounce for the claude worker.** On ambiguity the worker guesses instead of parking
   to Align (native loop has the bounce; delegated worker doesn't). Medium effort: teach the task prompt a
   confusion marker, parse it from the result, park the ticket. Becomes the top item the first time a guess
   lands wrong — currently zero observed incidents, so it ranks below the two cheap fixes.
4. **Advisory reviewer calibration: 0/2 true-positive on final branches** (t1 hallucinated-filename VIOLATES,
   t5 PATCH-vs-POST VIOLATES refuted by one grep), plus honest SUSPECT abstentions on >24K diffs. Decision
   needed, Wu Wei framed: it cried wolf twice and gates nothing — either ground it (give it repo access /
   un-truncated diffs, measure on the next 5 tickets) or drop the call. Don't keep paying for noise.
5. **Harden vacuity on template-heavy projects** — mutation gate contributed nothing to t4/t5 (all diffs
   template-stamped; loud vacuous pass both times). On handwritten Rust (self-sprint) it was load-bearing
   (h-t1 red→retry→1.000). Verdict: working as designed; DOCTRINE, not build — record "on template projects
   the hand-authored oracle carries verification" in decisions.md and move on.
6. **Ergonomic burr batch** (each observed live this session): `note` overwrites instead of appending (operator
   must rebuild full text to add retry guidance); binary opens the board DB before verb parsing (`--help`
   touched a stray repo-root DB); `harness-board.db-{shm,wal}` not gitignored; `harden_python` writes
   `cosmic-ray.toml` in-worktree; no board-overview verb (`status` is per-id, `ready`/`runnable` are bands).
   One mechanical slice.

## New critical path

1. **Hygiene + granularity slice** — items 2, 5-doc, 6 above (+ DeepSeek key swap when Gary rotates it).
   Cheap, mostly mechanical; a good self-hosting sprint candidate.
2. **Draft grounding slice** — item 1. The last accidental tax on every ticket's intake.
3. **Reviewer fix-or-drop** — item 4, gated by a number: ground it, run 5 real tickets, keep iff ≥1 true
   positive with 0 false VIOLATES; else delete the call.
4. **The real gate for the daily-driver claim: a measured trial.** Use the harness for the next real project
   ticket-stream (whatever Gary's actual work serves up), tracking per ticket: operator-minutes, worker cost,
   gate catches vs. misses, bounces. The claim graduates on numbers, not on this document.

Confusion bounce (item 3) is deliberately parked until a guess-lands-wrong incident supplies the trigger.
Unchanged from June: memory, gates, isolation don't block anything. MCP wrapper + naming stay Wu Wei-parked.

---

# Original assessment — 2026-06-30 (superseded where the ledger above says so)

> Assess-mode artifact. Two questions: (1) has the evolved `dev` skill drifted from this harness's design?
> (2) is the harness ready to replace Claude Code on real projects? Evidence-based; gate-by-artifact.

## TL;DR

- **Skill drift:** Not a threat — *convergent*. Most of the skill's recent growth was reverse-engineered
  **from** this harness (skill Evolution 2). The drift that exists is in the **judgment plane**, not the
  process plane, and one long-standing plan ("port `references/*` as the judgment layer") is **stale and was
  never executed**.
- **Project-ready?** **Not yet as a Claude Code replacement.** It is a strong, well-tested *single-ticket
  gated build loop on a local model* — validated on small katas. Missing for real projects: **intake/drafting,
  a stronger worker, multi-ticket orchestration, non-code modes.** Foundation (board/gates/isolation/memory)
  is genuinely solid.

## Evidence base

- ~13.8k LoC across 4 crates; ~250+ tests; clippy-clean; disciplined adversarial review throughout.
- CLI lifecycle complete: `new · plan · criteria · validation · note · confusion · align · rework · show ·
  status · trajectory · ready · run · explore · verify · harden · review · land · close · remember · recall`.
- Dogfood corpus = 9 shakedowns, all **small algorithmic katas** (calc-modulo, lru-cache, merge-intervals,
  regex-match, terax-keymap, big-edit, sig-ripple, fix-merge, test-author), 1–10 files each. Mechanics
  validated (isolation never leaked; mutation gate fired; reviewer caught a lookup-table overfit). **No real
  multi-ticket project has been run.**
- Default worker = local oMLX 35B. research/23 *measured* it silently failing ~30% on generative code; it
  passes only because downstream gates + rework catch failures on small tasks.

## Q1 — Skill drift

The `dev` skill was heavily rewritten 2026-06-30 (5 evolutions; every file touched). It is **converging with
the harness**, because the harness fed it: skill Evolution 2 = *"harness engineering reverse-engineered into
skill prose."* The major new skill principles are harness inventions:

| New/strengthened in skill | Already load-bearing in harness |
|---|---|
| Gate by artifact not proxy + discrimination floor | `gate_satisfied`, `tests_green`/`landed`, the spine |
| Planned-gate-skip discipline | the adversarial-review trigger rule (decisions.md) |
| Signal integrity / labels encode HOW | `truncated`/`stalled`/`looped`/`rejected` outcomes |
| Adversarial-review trigger (hard-to-reverse OR silent) | verbatim a harness decision |

So the skill's **process/integrity** layer ratifies the thesis (process must be code, not prose). Three real
deltas, all in the **judgment** plane, none invalidating the architecture:

1. **The "port `references/*` as the judgment layer" plan is stale and undone.** There is no `references/`,
   no `ml-heuristics`/`rag-heuristics`/`production-thinking`/`pushback-and-teach` in the harness. A worker's
   entire judgment layer = `wiki/case-law.md` (**8 generic bullets**) + memory priming. The skill's
   references have since grown new sections (instrument-must-resolve-delta, experiment-design cluster, RAG
   frontier) on a separate Claude-altitude track. **Reframe:** those references are the skill's layer *for
   Claude-as-worker*; they are not destined to be copied in. The harness's real judgment channel is
   case-law + memory — and case-law is thin.
2. **Pattern Gate (skill Evo 5) has no harness equivalent.** The skill now *mandates* declaring
   coordinator-direct / fan-out / sim-first. The harness has `explore` (a fan-out probe that lands nothing)
   and single `run` — **no coordinator, no sprint.** This is the new skill idea that points squarely at an
   unbuilt harness capability.
3. **KB Grounding Gate (skill Evo 1) doesn't exist for workers.** Harness workers have no Knowledge-Base
   tool; grounding is a Claude-in-the-loop discipline only.

## Q2 — Project readiness gaps (priority order)

1. **No intake/drafting.** The operator hand-authors plan + criteria + validation for *every* ticket via CLI.
   The agent-drafts-the-workpad step is explicitly deferred (decisions.md "Task intake"). This is the biggest
   day-to-day friction vs Claude Code, which drafts all of that from one sentence.
2. **Worker capability.** Local 35B silently fails ~30% on generative code (measured). The strong-worker path
   (`claude -p` delegated, subscription-backed — decisions.md "Claude integration has TWO shapes") is
   **designed but unbuilt**. DeepSeek V4 is wired but metered and landing-only. `config.rs` has dialects
   omlx/deepseek/anthropic — **no claude-cli delegated-worker dialect.**
3. **No multi-ticket orchestration.** A project = a backlog with dependencies. The board *has* edges/blocking,
   but nothing drives a queue. Only single `run` + the `explore` probe exist. No Sprint/coordinator ⇒ the
   Pattern Gate's fan-out has no engine.
4. **Code kinds only.** build/bugfix/refactor gate properly (`kind_is_code`). research/business-grounding/
   design are schema-acknowledged but their per-kind gate profiles + judgment aren't built. The harness does
   Build; Claude+skill also does Design/Assess/Train/Analyze.
5. **Thin judgment + CLI-only ergonomics.** 8 case-law bullets, no heuristics; TUI/GPUI frontends deferred.

**Ready for:** single-ticket, well-specified, testable build/bugfix/refactor work on a local model, with
strong gates and compounding memory. That is real and working — keep dogfooding there.

**Not ready for:** being the daily driver that takes on a real, ambiguous, multi-file/multi-ticket project.

## Critical path to "daily driver"

1. **Intake/drafting** (this assessment's recommended next slice — see below). Highest impact-to-effort;
   removes the per-ticket authoring tax; slots onto the existing Align primitive.
2. **`claude -p` delegated worker.** Decouples output quality from the local 35B at ~zero marginal cost under
   the subscription's Agent-SDK credit (decisions.md confirmed it backs *whole-task* delegation, not turns).
3. **Sprint/coordinator.** Drive the existing board backlog (edges already model blocking) to land N tickets.

Memory, gates, and isolation are already there and do not block this path.

---

## Proposed next slice — Intake / workpad drafting

**Problem.** Today every ticket needs the operator to write plan + acceptance criteria + validation by hand
before `align`. That is the manual scaffolding Claude Code does for free, and it's the single biggest reason
the harness isn't a daily driver.

**The reframe (already decided, decisions.md "Task intake: fidelity is a dial onto the Align gate").** It is
not file-vs-chat. Seed at any fidelity → the **system drafts the workpad** → the **Align gate is the
back-and-forth**. The only missing build piece is step 2: *agent-drafts-plan-in-align* (slice 3 explicitly
deferred it).

**Scope (one slice).**
- New verb: `agent draft <id>` (or fold into `new`) — given a ticket title/seed, call a provider to propose
  **Plan / Acceptance Criteria / Validation**, write them via the existing `set_plan`/`set_acceptance_criteria`/
  `set_validation` chokepoints. Ticket stays pre-Align (read-only band) — nothing executes.
- The draft is a **proposal, not a gate pass.** `criteria_confirmed` stays human-only (the keystone is
  untouched). Align remains the leveler: rich seed → one-shot confirm; vague seed → pushback rounds.
- **Provider:** drafting is a judgment task → default it to the **strong** backend (DeepSeek now; `claude -p`
  when built), *not* the local 35B. This is the first place the strong-worker gap actually bites — worth
  sequencing #2 close behind, or accepting DeepSeek-metered drafting in the interim.
- **Acceptance for the slice itself:** a one-line seed produces a workpad whose criteria are concrete enough
  that Align is a confirm-or-one-round, not a rewrite. Gate by a sample of drafted-vs-hand-authored tickets,
  not by "the call returned."

**Adversarial-review trigger check:** drafting is **reversible and loud** (operator reads the draft at Align
before anything runs; a bad draft is visibly bad and costs nothing). → **review-exempt**, normal Verify. Do
*not* let a draft auto-confirm criteria — that would make it silent + irreversible and flip the verdict.

**Compounding (post-memory, free):** the draft step pre-fills criteria from past similar tickets via
`recall_primed`, so the Align loop shortens over time. The plumbing already exists.

**Rejected alternatives** (from the intake decision): pure file-drop (no underspecification catch → garbage-in)
and pure chat (forces conversation even on a complete spec). The draft+Align loop subsumes both.
