# Parallel exploration (P4 / design-v2 §8) — build plan

> Grounds design-v2 §8 ("beam search over approaches") + research/05 onto the **existing** harness
> primitives. Status entering this doc: single-worker isolation (`run_worktree`), telemetry (`runs` +
> trajectory JSONL), board gates, and the memory sidecar (`remember`/`recall`) all exist and are green.
> This pillar adds a **coordinator** that fans out N *diverse* workers across isolated worktrees, prunes
> by the pre-committed Align criteria, and selects/consolidates a winner. **Adversarial review required**
> (silent-failure design — per the Process gate). This doc is the pre-review design; the review folds in
> below before any code.

## 0. KB grounding

`KB: searched "parallel search over candidates with a verifier / beam search / multi-agent orchestration"
→ nothing new beyond research/05.` Three priors confirmed, none redirecting the §8 design:
- **Beam search** (Mastering PyTorch ch54): keep top-k by *whole-sequence* score, not next-token greedy —
  maps to "each thought is a whole approach trajectory; rank trajectories, not steps."
- **MCTS/UCB** (DRL Hands-On ch200): search under an *unreliable* signal, balancing explore/exploit —
  the model's self-assessment is the unreliable signal; the **objective gate** (validation) is the
  reliable one, which is why §8 runs the objective gate *first* (zero judge cost) before any LLM judge.
- **Simulated annealing** (MTSF ch88): accept-worse-to-escape-local-optima — the philosophical case for
  breadth>1 (greedy b=1 gets stuck); ToT's 20%→ ablation is the same lesson with numbers.

The KB had nothing on LLM-agent coordinator/worker orchestration specifically; research/05 (ToT, GoT,
LATS, AlphaCode, Snell-2024 compute-optimal) remains the grounding substrate.

## 1. What already exists (build on, don't rebuild)

| Primitive | Where | Reused as |
|---|---|---|
| Worktree isolation | `git::ensure_worktree(root, id)` → `.harness/worktrees/<id>` on branch `harness/<id>` | per-worker workspace, id = `<ticket>-w<k>` |
| The worker loop | `run_ticket(board, id, cwd, root)` — oMLX loop, tool gate per call, telemetry | one branch's trajectory (unchanged loop) |
| WIP commit | `git::commit_worktree(wt, id)` | freeze each branch's result for inspection/merge |
| Squash-land | `git::squash_merge(root, id, title)` (refuses dirty/empty/conflict) | land the winner (after consolidation) |
| Cleanup | `git::remove_worktree(root, id)` (worktree then `branch -D`) | reap losers |
| Telemetry | `board.start_run / finish_run`, `runs_path(root, ticket, run_id)` | **N runs, one ticket** — the model already fits |
| Negative memory | `board.remember(NewMemory{ type:"lesson"|"discovery", ticket_id, .. })` | reflection-on-kill post-mortems |
| The verifier | ticket `acceptance_criteria` + `validation` (run by `run_verify`) | the **pre-committed** objective gate |

**Key fit:** the `Run` model is *already* N-runs-per-ticket (distinct `run_id`, shared `ticket_id`, shared
`attempt` epoch). Parallel exploration is N runs against one ticket — **no schema change**. The board sees
one ticket on the spine; the N branches are ephemeral worktrees (claim/lease is explicitly ephemeral,
design-v2 §3), exactly the layer that "rebuilds on restart."

## 2. Architecture placement — agent-crate orchestration, board untouched

The coordinator is **agent-crate only**. It does not add tables, does not put branches on the board, does
not touch the spine. It composes: git worktrees (fan-out/reap) + `run_ticket` (the worker) + `runs`
(telemetry) + `remember` (negative memory) + a small set of **pure selection functions** (the prune/pick
logic). This is the Wu Wei placement: the board's one-ticket-one-spine-node invariant is preserved; the
N-attempt structure lives where ephemeral concurrency already belongs.

**Two FSMs stay separate** (design-v2 §3): the ticket moves Todo…Done once; the coordinator's per-branch
claim/lease (created → running → {survived|killed} → reaped) is ephemeral and never persisted to the
spine.

## 3. The coordinator — roles and flow

Cheap workers, rich judge/synthesizer (design-v2 §8; Snell-2024 compute-optimal — spend the big model
where it discriminates, not where it generates).

```
agent explore <ticket>
  0. precondition: ticket past Align (criteria_confirmed), has validation + acceptance.   [reuse gate]
  1. fan-out N by stakes.                                                                  [§3.1]
  2. planner assigns N DISTINCT strategies (diversity by construction).        [oMLX, rich] [§3.2]
  3. for k in 0..N:  (sequential MVP; concurrency = later slice)
       create worktree <ticket>-w<k>;  run_ticket seeded with strategy_k;  commit.  [oMLX, cheap]
  4. STAGE-1 prune (objective gate, zero judge cost):                                      [§3.3]
       run each branch's validation in its worktree → survivors = passers.
       each killed branch: post-mortem → remember(negative) ; reap worktree.
  5. STAGE-2 select (only if >1 survivor): pairwise-knockout judge vs criteria.  [oMLX,rich][§3.4]
  6. consolidate winner → branch harness/<ticket>; reap loser worktrees.                   [§3.5]
  7. report: winner branch + per-branch outcome table + stored post-mortems.
     (oMLX exercise — explore NEVER lands; the human/Claude reviews + lands.)
```

### 3.1 Fan-out by stakes (don't fix N)
Stakes come from the ticket, not a flag default: `low → 2, default → 3, high/ambiguous → 4`. MVP source =
ticket `priority` (already on the row) + an explicit `--fanout N` override. **Hard ceiling** (e.g. 5) so a
mis-set stakes field can't spawn a swarm. Single-survivor and N=1 short-circuit the judge (no pairwise
round needed).

### 3.2 Diversity by construction (the fix for N near-duplicates)
The planner (rich model) is given the plan + criteria and must emit **N distinct strategy descriptors**
(distinct approach/assumption per worker) — temperature alone is NOT diversity (research/05). Each
descriptor is injected into that worker's system prompt. **Testable seam:** the *parse + dispatch* (N
descriptors → N distinct worker prompts, all non-empty, all distinct) is a pure function, unit-tested
without oMLX; the *quality* of diversity is exercise-measured.

### 3.3 Stage-1 prune — objective gate first (the load-bearing ordering)
Run each branch's `validation` command in its worktree (reuse `run_verify`'s machinery). A branch that
violates a **hard** Align failure-criterion (validation red) **dies at zero judge cost** — no LLM call.
This is the §8 keystone: the reliable signal gates before the unreliable one. Dead-end signals in the MVP:
(a) hard-criterion violation (validation red), (b) per-branch **step budget** exhausted
(`HARNESS_MAX_ITERS` already caps this; a branch that hit max_iters without a clean End is a dead-end —
`classify_stop` already labels it `max_iters`). **Deferred:** mid-run effort-without-progress detection
(needs a per-iter progress checkpoint that doesn't exist yet; tuning it against zero real runs would
violate gate-by-a-number — slice it later, gated on the core earning its keep).

### 3.4 Stage-2 select — pairwise-knockout judge (not noisy absolute scores)
Only runs on >1 survivor. The judge (rich model) compares survivors **pairwise against the success
criteria** and the winner advances (knockout bracket). Pairwise > pointwise because absolute LLM scores
are noisy (research/05; mirrors reranker design). **Testable seam:** the bracket mechanics (N survivors →
N-1 comparisons → one winner, deterministic given a comparison oracle) is a pure function unit-tested with
a stub oracle; the *judgment* is the oMLX call.

### 3.5 Consolidate — winner promotion, loser reaping (serial, coordinator-owned)
**All shared-git mutation is serialized in the coordinator** (workers only ever touch their own confined
worktree — never shared state; the standing constraint "never let a background subagent mutate shared git
state" holds by construction). Consolidation: move the winner's branch to the canonical `harness/<ticket>`
(so the existing `agent verify`/`agent land` flow works unchanged), then `remove_worktree` each loser.
**MVP = pick, not merge:** the winner is promoted whole. GoT-style synthesis (merge winner + best
runner-up fragments) is **deferred** (§4) — pick-the-best is the honest first cut; synthesis is the
refinement gated on evidence that a runner-up carried a fragment the winner lacked.

### 3.6 Reflection-on-kill → negative memory (the compounding payoff)
Every killed branch writes a **one-paragraph post-mortem** (why it failed against which criterion) stored
via `remember(type="lesson", scope=Ticket, ticket_id, salience)`. These are (a) surfaced to later
explorations of related tickets via `recall`, and (b) the negative-result corpus design-v2 §8 wants. This
is the cheapest high-value piece — it's why parallel exploration *compounds* instead of just costing N×.

## 4. Scope — MVP core (this pillar) vs deferred (gated on the core)

**MVP (build now, each slice gated by a number/artifact):**
fan-out by stakes · diversity-by-construction dispatch · sequential worker runs · stage-1 objective prune
· stage-2 pairwise-knockout select · winner consolidation · reflection-on-kill → memory · per-branch +
global step budget.

**Deferred (Wu Wei — speculative until the core runs; tuning against zero data violates gate-by-a-number):**
- **Concurrency** — sequential MVP delivers the *search* benefit (breadth→better answer, ToT's whole
  point) without wall-clock parallelism. Concurrent workers over oMLX's 8-way batch are a pure *latency*
  optimization, gated on the core working. (Safe to add later: workers are already worktree-isolated and
  file-confined; coordinator pre-creates all worktrees serially.)
- **Mid-run effort-without-progress** dead-end detection (§3.3) — needs a per-iter progress signal.
- **GoT synthesis / debate-on-tie** (§3.5) — pick-the-best first; merge only when evidence shows a
  runner-up fragment mattered.
- **Token-budget reallocation** to survivors — MVP uses fixed per-branch step budgets; reallocation is an
  optimization.

This split is the same discipline the prior pillars used (memory Slice A/B/C; harden ratchet): land the
irreducible, verifiable core; fence the speculative refinements behind a measurement gate.

## 5. Pure-function seams (test without oMLX) vs model-driven (exercise)

The integrity rule from prior pillars: **every slice has a `(no oMLX)` success criterion** — a pure-Rust
property test — plus an exercise criterion for the model-driven quality.

| Concern | Pure-Rust testable (the gate) | oMLX exercise (quality) |
|---|---|---|
| Fan-out | stakes → N (incl. ceiling, override) | — |
| Diversity dispatch | N descriptors → N distinct non-empty prompts | are the strategies actually diverse |
| Worker run | worktree lifecycle: N distinct worktrees created/committed/reaped, no collision | the work itself |
| Stage-1 prune | survivors = passers; killers reaped; post-mortems written (over **synthetic** branch outcomes) | validation truly discriminates |
| Stage-2 select | knockout bracket over a **stub** comparison oracle → deterministic winner | judge picks the better branch |
| Consolidate | winner branch == `harness/<ticket>`; losers gone (git assertions) | — |
| Reflection | killed branch ⇒ a `recall`-able negative memory row exists | post-mortem is useful |

The selection logic (stage-1 filter + stage-2 bracket) is extracted as **pure functions over a
`BranchOutcome` value type** — no git, no oMLX — so the search algorithm is unit-tested in isolation, and
the I/O (git, model) is thin glue around it. This is the "visible control flow" / "metrics are just code"
engineering style applied to the coordinator.

## 6. Implementation plan — slices + subagent coordination

Three slices. Each lands independently, green tests, explicit-path commit. **Subagent coordination
pattern for the *build*** (the mandate's ask): the slices have a clean dependency spine
(types → selection logic → git fan-out → glue), so the build is mostly sequential, but the **adversarial
review fans out** (decorrelated cold reviewers in parallel) and **slice-2's two halves are independent**
(stage-1 filter and stage-2 bracket are separable pure functions) — those parallelize.

- **Slice P4.1 — selection core (pure, no git, no oMLX).** `BranchOutcome` value type; `stage1_prune`
  (outcomes → survivors + killed); `knockout` (survivors + comparison oracle → winner); `fanout_for`
  (stakes → N, ceiling, override). *Gate:* unit tests for each — empty/single/all-killed/tie edge cases;
  bracket determinism under a stub oracle.
- **Slice P4.2 — worktree fan-out lifecycle (git, no oMLX).** Coordinator creates `<ticket>-w<k>`
  worktrees serially, runs a **stub worker** (a closure, so the loop is testable without the model),
  commits, reaps losers, consolidates winner → `harness/<ticket>`. *Gate:* a git integration test —
  N worktrees created on a temp repo, winner consolidated, losers gone, no leftover branches/worktrees.
- **Slice P4.3 — wire the real roles + reflection (oMLX glue + memory).** Replace the stub worker with
  `run_ticket`; add the planner (diversity) and judge (pairwise) oMLX calls behind the pure interfaces;
  killed branches → `remember`. *Gate (no oMLX):* reflection writes a `recall`-able row given a synthetic
  kill; the planner/judge are behind traits so a stub satisfies the unit test. *Gate (exercise):* one
  live `agent explore` on a real low-stakes ticket produces ≥2 diverse branches, prunes correctly, picks
  a winner, and stores ≥1 post-mortem — captured as evidence, NOT landed.

## 7. Risk register (seed for the adversarial review)

Silent-failure modes this design must survive — the review should attack these hardest:
1. **Breadth collapse to 1** — if diversity generation fails (all workers get the same/empty strategy),
   the search degenerates to greedy (ToT's 20% case) *while still costing N×*. Mitigation: the
   distinct-non-empty-descriptor gate (§3.2) fails loudly rather than running near-duplicates.
2. **Judge kills the winner** — a noisy pairwise judge can eliminate the best branch. Mitigations:
   objective gate runs first (a branch that passes validation can't be killed by the judge, only
   *ranked*); pairwise-knockout (not absolute scores); the winner is human-reviewed before land (explore
   never lands).
3. **Shared-git corruption** — two workers mutating the same branch/worktree. Mitigation by construction:
   distinct worktree/branch per worker; all shared-git ops serialized in the coordinator; workers
   file-confined to their own worktree.
4. **Runaway cost** — N × max_iters tokens with no cap. Mitigation: fan-out ceiling + per-branch step
   budget (existing `HARNESS_MAX_ITERS`) + (deferred) global token budget.
5. **Consolidation loses work** — promoting the winner clobbers something, or a loser's worktree isn't
   reaped (disk leak / stale branch poisons the next run). Mitigation: `squash_merge` already refuses
   dirty/empty/conflict; the git integration test asserts no leftover worktrees/branches.
6. **"Successful" no-op branches** — a branch that does nothing can pass a weak validation. Mitigation:
   this is a *ticket-criteria-quality* problem, not a coordinator problem — but the coordinator should
   surface "winner produced no diff" rather than silently consolidate an empty branch.

## 8. Open questions for the review
- Is sequential-MVP defensible, or does the pillar's value *require* concurrency in the first cut?
- Is "pick, not merge" (defer synthesis) the right first cut, or is synthesis load-bearing enough that
  pick-the-best is a strawman?
- Fan-out from `priority` — is that the right stakes proxy, or does it need its own ticket field?
- Winner consolidation by branch-move vs. leaving N worktrees and reporting — which is less surprising?

---

## 9. Review fold-in (DECISION — supersedes §3–§6 scope)

Three decorrelated cold reviewers attacked this design in parallel (dogfooding the subagent-coordination
pattern on our own review rule). Their findings converge on one verdict: **§3–§6 over-build a cathedral for
an exercise-only output. Build the evidence-producing PROBE, not the cathedral.** The probe is the
instrument that earns the deferred pieces — none of them get built until the probe produces evidence they're
needed. Findings, by severity:

### 9.1 BLOCKER — branch-move consolidation silently targets the wrong tree (R2)
§3.5's "move winner branch to `harness/<ticket>`" is **broken and passes a green gate while broken**. The
worktree stays registered at its `<ticket>-wK` path; `git branch -m` renames the branch but `worktree_or_cwd`
(main.rs:408) keys the path on the canonical id, so a later `agent verify`/`agent land` resolves to a path
that no longer carries the winner's commits — or worse, a stale one. The consolidation "succeeds" and the
land operates on the wrong tree.
**Fold-in: do NOT auto-consolidate.** The probe reaps nothing and **reports the winner's worktree path +
branch id** for a human to verify/land manually. Branch-move consolidation is deferred until it can be done
correctly (`git worktree move` + `git branch -m` together, with a path-resolution test) — and only if
evidence shows manual hand-off is the bottleneck.

### 9.2 Major — run_ticket's single `id` conflates board-id and worktree-id (R2)
`run_ticket(board, id, cwd, root)` uses `id` for BOTH `board.get` and `mint_run_id`. N workers sharing one
ticket need a distinct run label per worker, but the board lookup must stay the canonical ticket. Also
`mint_run_id` = `{id}-{attempt:03}-{millis}` → **same-millisecond collision** for N workers minted in a loop.
**Fold-in:** thread a worker label into the run id (`<ticket>-w<k>` in the run_id, `ticket_id` stays the
board ticket). Small, surgical — not a signature overhaul.

### 9.3 Major — ensure_worktree idempotence silently reuses a contaminated worktree (R2)
If a prior crashed `explore` left `<ticket>-w0` on disk, `ensure_worktree` reuses it — the new worker
inherits a dirty, half-finished tree and its run is quietly invalid.
**Fold-in: reap-before-fanout.** The probe removes any stale `<ticket>-w*` worktrees at the start of a run
before creating fresh ones. Loud, deterministic starting state.

### 9.4 Major — "shared-git safe by construction" is FALSE (R2)
§3.5/risk-3 claim workers can't touch shared git. But `bash` is **not confined** (tools.rs:79–82 — only the
file tools are; fully confining bash needs OS-level isolation, deferred). A worker can `cd` out or use
absolute paths and mutate shared state.
**Fold-in:** retract the "by construction" claim in the docs (it's false and a false safety claim is worse
than none). Sequential workers make this a non-issue *for the probe* (one worker runs at a time, no
concurrent shared-state race). Document the real boundary honestly; the cheap guardrail (assert worktree
HEAD advanced / branch unchanged on shared refs) is a probe-output check, not a hard sandbox.

### 9.5 Major — premature reaping destroys the evidence the probe exists to produce (R1)
Reaping losers mid-run (§3 step 4/6) deletes exactly the trees a human needs to inspect to judge whether the
search worked.
**Fold-in:** the probe **reaps nothing**. It runs all branches, validates each, and reports a ranked
pass/fail + diffstat table over the surviving worktrees. Human inspects, then verifies/lands/cleans up.

### 9.6 Major — the judge is over-built; knockout adds noise (R1)
Pairwise-knockout (§3.4) introduces ordering noise and N−1 oMLX calls before there's any evidence the
objective gate is insufficient.
**Fold-in: drop the judge from the probe entirely.** Rank survivors by **objective signal only** (validation
pass + diffstat / iters-to-green). The judge (and whether pairwise/round-robin/Copeland) is deferred, gated
on probe evidence that objective ranking is ambiguous in practice.

### 9.7 Major — fake diversity gate / pick-not-merge destroys synthesis inputs (R1)
The distinct-non-empty-descriptor gate proves strings differ, not that *approaches* differ — it's a vibe
wearing a number's clothes. And the planner trait is premature.
**Fold-in:** the probe's diversity is **strategy-per-worker injected from a fixed list / `--strategy`
flags** (no planner oMLX call, no fake gate). Whether strategies were actually diverse is read off the
probe's diffstat table (did the branches do different things?) — real evidence, not a string check. The
planner-trait is deferred.

### 9.8 (R3) — the whole coordinator is over-built for exercise-only output
The synthesis of all three: explore never lands (it's an oMLX exercise), so every auto-consolidation /
judge / synthesis / concurrency feature is speculative machinery around an output a human reads. R3's lean
probe is the correct first cut.

### 9.9 What the probe IS (the scoped build)

```
agent explore <ticket> [--fanout N] [--strategy S ...]
  0. precondition: ticket past Align, has validation + acceptance.            [reuse gate]
  1. reap any stale <ticket>-w* worktrees (loud clean start).                 [9.3]
  2. N = fanout_for(stakes, --fanout override), hard ceiling 5.               [pure fn, §3.1]
  3. strategies: from --strategy flags, else a fixed default list of N.       [9.7 — no planner]
  4. for k in 0..N (SEQUENTIAL):
       create worktree <ticket>-w<k>;
       run_ticket with worker-label run_id + strategy_k in prompt;            [9.2]
       run validation in the worktree (extracted non-board-mutating helper);
       record BranchOutcome{ k, strategy, passed, diffstat, iters, stop }.
  5. rank survivors by objective signal only (passed, then diffstat/iters).   [9.6 — no judge]
  6. killed branches → remember(type="lesson", ticket_id) post-mortem.        [§3.6 kept — cheap, compounding]
  7. print ranked table; report winner candidate's worktree PATH + branch id. [9.1, 9.5 — reap nothing, no consolidate]
     (oMLX exercise — explore NEVER lands; human inspects + verifies + lands + cleans up.)
```

**Pure seams (the gate, no oMLX, no git):** `fanout_for(stakes, override) → N` (ceiling, override);
`rank_outcomes(Vec<BranchOutcome>) → ranked` (passers first, deterministic tiebreak). Unit-tested in
isolation. **Git seam:** the worktree fan-out lifecycle (create N, reap stale first, no leftover on the
*reap-stale* path) — integration-tested on a temp repo with a stub worker closure. **oMLX exercise:** one
live `agent explore` on a low-stakes ticket produces ≥2 branches that do *different* things (read off the
diffstat table) and ≥1 post-mortem — captured as evidence, not landed.

### 9.10 Deferred (gated on probe evidence — NOT built now)
planner-trait + diversity-quality gate · pairwise/round-robin/Copeland judge · auto-consolidation
(branch-move done correctly) · concurrency over oMLX's 8-way batch · GoT synthesis (merge fragments) ·
mid-run effort-without-progress detection · token-budget reallocation. Each is fenced behind a specific
probe finding that would justify it (recorded in §4 + here so the next session doesn't re-litigate).
