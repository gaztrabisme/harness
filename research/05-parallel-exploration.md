# Track 5: Parallel Exploration & Pruning

## Scope & what it grounds

Grounds **P4: parallel exploration with pruning** for the harness. The pain it fixes: on autonomous research/experiment tickets, the agent brainstorms many directions but then tunnels down ONE branch to a dead end and never backtracks. Sunk-cost tunneling, no breadth, no abandon signal.

The design being grounded: for research/experiment tickets, **fan out N approaches in parallel** (isolated workers), **score each against the Align-gate success/failure criteria** (the criteria ARE the pruning function), **prune dead ends**, let survivors continue — i.e. **beam search over approaches**, with breadth scaling to stakes under a token budget.

This track maps the search-over-approaches literature onto that design and answers five operational questions: fan-out count + diversity seeding, scoring/pruning rule, dead-end detection, budget control, and synthesis of winner + runner-up ideas.

Key reframe from the literature: our design is **best-first / beam search where the "thought" is an entire experiment trajectory, not a token-level reasoning step, and the verifier is the Align gate's pre-written criteria rather than a learned reward model.** That distinction drives most of the recommendations below.

---

## Landscape

### Tree of Thoughts (ToT) — Yao et al. 2023
Maintains a *tree* of intermediate "thoughts" (coherent semantic units, not tokens). At each node the LM (a) generates candidate next thoughts and (b) **self-evaluates** each via a deliberate value/vote prompt. Classic search (BFS keeping top-`b≤5` per level; DFS exploring the best, pruning a state when its value falls below a threshold and **backtracking to the parent**) drives systematic exploration with lookahead + backtracking. Ablation: a greedy `b=1` variant (no backtracking) collapses to ~20% word success on crosswords — **backtracking is load-bearing, not decorative.** [arxiv.org/abs/2305.10601](https://arxiv.org/abs/2305.10601)

### Graph of Thoughts (GoT) — Besta et al. 2023
Generalizes ToT from a tree to an **arbitrary DAG**: thoughts are vertices, dependencies are edges. The added primitive over ToT is the **Aggregation Transformation** — a node with multiple incoming edges that *merges* separate reasoning chains, "combining and reinforcing their advantages while mitigating their disadvantages." Also supports refinement loops and distillation/pruning. Reports +62% sort quality vs ToT at >31% lower cost. This is the formal model for our **synthesis step** (merge winner with runner-up ideas). [arxiv.org/abs/2308.09687](https://arxiv.org/abs/2308.09687)

### Language Agent Tree Search (LATS) — Zhou et al. 2023/ICML 2024
Adapts **MCTS** to language *agents* (reasoning + acting + planning), no training required. Six ops: selection (UCT, balances explore/exploit), expansion, evaluation (LM value function → scalar), simulation, backpropagation, and **reflection** (on a failed trajectory, the LM writes a verbal self-critique that is fed back as context for the next iteration). Key enabler: "we can conveniently back up to any state by setting the input to be the context and previous output" — **state is just context, so backtracking is free.** 94.4% pass@1 on HumanEval; doubles ReAct on HotPotQA. Reflection ablation = ~0.05 drop. [arxiv.org/abs/2310.04406](https://arxiv.org/abs/2310.04406)

### Self-consistency — Wang et al. 2022
Decoding strategy: sample N diverse CoT paths (temp 0.5–0.8), then **majority-vote the final answers**, marginalizing over reasoning paths. "Wisdom of the crowd": correct paths converge, errors scatter. +17.9% GSM8K. No verifier, no training. Vote share doubles as a **confidence signal**. Maps to our cheapest fan-out mode for tickets with a checkable answer. [arxiv.org/abs/2203.11171](https://arxiv.org/abs/2203.11171)

### LLM-as-judge / verifier-guided selection — (survey 2412.05579; Pairwise RM; Generative Verifiers 2408.15240)
Best-of-N: generate N candidates, score with a verifier (trained ORM/PRM **or** a prompted LLM judge), keep the top scorer. Two judge modes:
- **Pointwise** — score each candidate against criteria independently (scales to listwise; suffers absolute-score noise/inconsistency).
- **Pairwise** — compare two at a time; run a **single-elimination knockout tournament** (N−1 comparisons) to find the best. More reliable for correctness judgments; cross-validates candidates. But pairwise scales poorly to full rankings (order bias, non-transitivity, context limits).
PRMs enable **early pruning mid-trajectory** (beam search prunes low-scoring steps before completion). [survey](https://arxiv.org/pdf/2412.05579) · [Generative Verifiers](https://arxiv.org/abs/2408.15240)

### Multi-agent debate — Du, Li, Torralba, Tenenbaum, Mordatch 2023/ICML 2024
N model instances independently answer, then **read each other's answers and reasoning and revise over several rounds**, converging on a consensus. Improves math/strategic reasoning and **factuality** (fewer hallucinations). Crucially, "debate does not just amplify one correct answer" — there are cases where *all* agents start wrong and the debate converges them to right. Maps to a **synthesis/reconciliation** alternative to hard pruning. [arxiv.org/abs/2305.14325](https://arxiv.org/abs/2305.14325)

### AlphaCode — DeepMind 2022 (generate-and-filter at scale)
The canonical "spawn many, filter hard, submit few" pipeline: generate **millions** of programs (diversity via random metadata tags/ratings + high temperature), **filter on the example tests** (kills ~99%), then **cluster surviving programs by behavior** (same outputs on generated inputs → same cluster) and submit one per cluster, largest cluster first, capped at 10. Two-stage funnel: cheap behavioral filter → diversity-preserving selection. The **execution-based filter** (does it pass the given tests?) is the model for our objective pruning gate. [arxiv.org/abs/2203.07814](https://arxiv.org/abs/2203.07814)

### Compute-optimal test-time scaling — Snell et al. 2024
**No single search strategy is optimal — optimality depends on the compute budget and prompt difficulty.** Empirically: low budget → short/greedy; medium → beam search; high → majority voting. Adaptive allocation by difficulty beats best-of-N by >4×, and lets a small model + search beat a 14× larger model. Directly grounds our **budget rule** (breadth scales to stakes; pick the mode by budget tier). [arxiv.org/abs/2408.03314](https://arxiv.org/abs/2408.03314)

### Production parallel-agent patterns — Claude Code Dynamic Workflows / orchestrator-worker
Orchestrator decomposes the goal, fans out workers each scoped to one unit of work in isolated context (worktrees for file isolation), collects results, and the **"reduce" step synthesizes** — repeatedly flagged as "the most difficult part." Cost is real (hundreds of xhigh workers = millions of tokens), so per-agent model routing (cheap workers, expensive synthesizer) and "only parallelize genuinely independent work" are the operating constraints. [code.claude.com/docs/en/agents](https://code.claude.com/docs/en/agents)

---

## Transferable techniques

For each: **what it is → how it maps to our fan-out / prune-against-criteria design.**

1. **Semantic-unit thoughts (ToT).** Search over coherent units the model can self-evaluate, not tokens.
   → Our "thought" = a **whole approach/experiment trajectory** (hypothesis + method + result). One worker = one branch. The unit is large, so we can afford very few branches but must evaluate them well.

2. **LM self-evaluation as the search heuristic (ToT/LATS value function).** The model scores progress in language; the heuristic is prompted, not learned.
   → Our heuristic is **pre-committed**: the Align gate's success/failure criteria. We don't invent a value function at search time — the gate's criteria ARE the scoring rubric. This removes the biggest ToT weakness (ad-hoc self-eval drift) by fixing the rubric before the search starts.

3. **DFS prune-and-backtrack with a value threshold (ToT).** Kill a state when value ≤ threshold; backtrack to parent.
   → A branch whose interim result fails a **hard** failure criterion is killed immediately. "Backtrack to parent" = the orchestrator reallocates that branch's remaining budget to survivors or seeds a fresh approach. The ablation result (greedy = 20%) is the direct evidence for why our harness must keep ≥2 branches alive past the first checkpoint.

4. **MCTS + reflection (LATS).** UCT balances explore/exploit; failed trajectories produce a verbal self-critique fed forward.
   → We likely **don't** need full UCT/backprop (too many rollouts for expensive experiment trajectories). We **do** want **reflection-on-kill**: when a branch is pruned, the worker writes a short post-mortem ("died because X assumption was false") that (a) is fed to surviving/new branches so they don't repeat it, and (b) becomes a negative-result memory entry. Cheap, high-value, and directly attacks "tunneling without learning."

5. **State = context, so backup is free (LATS).** Reverting to an earlier node is just resetting the context.
   → Each worker runs in an **isolated worktree/context**. Abandoning a branch costs nothing structural — no shared-state unwind. This is what makes parallel exploration safe to prune aggressively.

6. **Self-consistency vote (Wang).** Sample N, majority-vote, vote-share = confidence.
   → For tickets with a **checkable answer** (a number, a pass/fail, a metric target), the cheapest fan-out: N independent attempts, take the consensus, use agreement as confidence. Use when a real verifier is unavailable.

7. **Pairwise knockout judge (Pairwise RM).** Compare candidates two at a time in a tournament; avoids absolute-score noise.
   → When survivors aren't separable by hard criteria (all "passed," differ in quality), rank them with **pairwise comparisons against the Align criteria**, not absolute 1–10 scores. N−1 comparisons, more reliable than pointwise scoring for "which is better."

8. **Execution-based filter + behavioral clustering (AlphaCode).** Cheap objective filter first, then cluster to dedupe near-identical survivors.
   → Two-stage prune: (1) **objective gate first** — run the experiment / the test / the metric; anything that fails hard criteria dies with zero LLM-judge cost. (2) **cluster/dedupe survivors** so we don't keep three branches that converged on the same answer (also a diversity check — see anti-patterns).

9. **Aggregation transformation (GoT) + debate (Du).** Merge separate chains into one reinforced result; or have branches read each other and revise.
   → The **synthesis step**: don't just pick the winner and discard runners-up. Feed the winner + the best fragments/post-mortems of the runners-up into a final aggregation pass. GoT is the formal "merge two parents" op; debate is the "let survivors reconcile" variant for when there's no single dominant winner.

10. **Compute-optimal budget allocation (Snell).** Match strategy to budget; allocate more compute to harder prompts.
    → **Breadth scales to stakes.** Low-stakes ticket → N=1–2, shallow. High-stakes/ambiguous → N=4–5, deeper, with a synthesis pass. Don't fix N; pick the tier from ticket stakes × budget.

11. **Orchestrator-worker + isolated context + cheap-worker/expensive-synthesizer routing (Claude Code).**
    → Our concrete execution substrate: orchestrator (the harness) seeds branches, workers explore in isolation, a single higher-effort synthesis call does the reduce. Route exploration workers cheaper, the judge/synthesizer richer.

12. **Loop/dead-end signatures from trajectory analysis (TIDE; agent-failure literature).** A cycle = state returns to a prior node (or no state change) with no goal progress; **dead-end trajectories run LONGER, not shorter** — heavy-tailed, hundreds of ineffective rounds, "prolonged persistence following early reasoning collapse." More effort ≠ progress.
    → This is the **dead-end detector** the user actually needs (see below). The counterintuitive finding — that a branch *consuming more steps* is evidence it's dying, not thriving — is the single most important transferable insight for the tunneling problem.

---

## Anti-patterns / what to avoid

- **N near-duplicate branches.** Sampling N times at the same temperature with the same prompt yields N paraphrases, not N approaches — you pay N× for ~1 approach's coverage. Mitigation: seed branches with **explicitly different strategies/assumptions** (diversity by construction, not by temperature), and **cluster/dedupe** survivors (AlphaCode) to detect collapse.
- **No backtracking (greedy tunneling).** The user's core pain, and ToT's ablation proves the cost (20% vs full search). Committing to branch #1 and overwriting forward = greedy `b=1`. Never let breadth collapse to 1 before the first checkpoint.
- **"More effort = making progress" fallacy.** The deadliest failure: a stuck branch *looks busy* (more tool calls, more tokens, longer trajectory) and the orchestrator reads activity as health. The trajectory literature is explicit — dead ends are **longer** than successes. A step/round budget per branch is mandatory.
- **Deterministic loop lock-in.** Low-temp branch hits an error (e.g., API keeps failing) and retries the identical action forever. Mitigation: loop-guard (repeated identical tool call / no state change) + perturb (seed/temperature/approach change) on detection.
- **Runaway parallel cost.** Hundreds of high-effort workers = millions of tokens for marginal benefit; parallelizing sequential/dependent work buys nothing. Mitigation: hard global token budget, branch caps, cheap-worker routing, and only fan out when sub-approaches are genuinely independent.
- **Ad-hoc self-evaluation drift.** Inventing the scoring rubric *during* the search lets the judge rationalize whatever survived. Mitigation: the rubric is the **pre-committed Align-gate criteria** — frozen before fan-out.
- **Absolute-score judging for close calls.** Pointwise 1–10 scores are noisy/inconsistent (Pairwise-RM finding). For separating "all passed" survivors, use pairwise comparison, not absolute scores.
- **Pruning the only diverse idea on a noisy score.** A verifier is itself fallible (OOD PRM problem, Snell). Don't kill a branch on a *soft* judge score alone — kill on **hard failure criteria** (objective), demote on soft scores.
- **Discarding runner-up insight.** Pick-the-winner-discard-the-rest throws away the partial wins debate/GoT show are recoverable (all-wrong→right convergence). Always run a synthesis pass that harvests runner-up fragments + post-mortems.

---

## Recommendation for our design

**Shape: beam search over whole-approach trajectories, with the Align gate as the verifier, isolated workers, and a mandatory synthesis pass. Tiered by stakes.**

### 1. Fan-out count + diversity seeding
- **Tier by stakes (Snell-style adaptive allocation), don't fix N:**
  - Low-stakes / well-specified ticket → **N = 1–2**, shallow.
  - Default research/experiment ticket → **N = 3**.
  - High-stakes or ambiguous ("we don't know which approach works") → **N = 4–5**.
- **Seed diversity by construction, never by temperature alone.** Before fan-out, have the planner enumerate **distinct strategies** (different algorithm/library/assumption/data-source per branch) and assign one per worker. Each worker's brief explicitly names what makes it different. This is the fix for "N near-duplicates."
- **Cap depth low** for expensive trajectories (we're not doing MCTS rollouts): 1–3 checkpoints per branch.

### 2. Scoring / pruning rule (the criteria ARE the pruning function)
- **Two-stage funnel (AlphaCode pattern):**
  1. **Objective gate first** — run the experiment / test / metric. Any branch that violates a **hard failure criterion** from the Align gate is **killed immediately**, zero judge cost. This is the primary pruner.
  2. **Judge only the survivors.** If multiple branches pass hard criteria, rank them by **pairwise comparison against the Align success criteria** (knockout tournament, N−1 comparisons) — not absolute scores.
- **Hard-kill vs soft-demote split:** hard criteria (objective, checkable) kill a branch outright; soft criteria (quality, elegance) only re-rank. Never hard-kill on a soft judge score alone (verifiers are fallible).
- **Dedupe survivors by behavior** (cluster on outputs/results); collapse near-identical branches into one to reclaim budget.

### 3. Dead-end detection (the user's core pain)
Kill/backtrack a branch when ANY fires:
- **Hard criterion violated** — the Align gate says this result is a failure. (primary)
- **No-progress / cycle** — the branch's state returns to a prior state, or a tool call repeats with no state change (loop-guard). Treat "no state change" as a cycle.
- **Effort-without-progress** — branch exceeds its per-branch step/round budget without clearing a checkpoint. **Explicitly encode the counterintuitive signal: a longer-than-typical trajectory with no checkpoint cleared is evidence of death, not health.** Compare each branch's round count to the cohort; heavy-tailed outliers are dying.
- **Self-assessed dead end** — at each checkpoint the worker answers "could this still meet the success criteria? what would have to be true?" A confident no → abandon.
On kill: the worker writes a **one-paragraph post-mortem** (LATS reflection) — *why* it died — which is (a) injected into surviving/replacement branches to prevent repeat failures and (b) stored as a negative-result memory. Backtracking is cheap because each branch is an isolated context (LATS: state = context).

### 4. Budget control
- **Global token/time budget per ticket, scaled to stakes** (breadth scales to stakes under a budget — the P4 mandate).
- **Per-branch step budget** so no single branch can run away (directly kills the effort-without-progress failure).
- **Reallocate freed budget** from pruned branches to survivors or to seeding one fresh approach informed by the post-mortems — not to deepening a single branch indefinitely.
- **Model routing:** exploration workers cheap; the pairwise judge + final synthesizer richer/higher-effort.
- **Strategy by budget tier (Snell):** tiny budget → single best attempt; medium → this beam-prune loop; large → add a self-consistency vote or extra branches.

### 5. Synthesis (combine winner + runner-up ideas)
- **Never pick-and-discard.** After pruning, run **one synthesis pass** that takes the winning branch **plus** the best fragments and post-mortems of the runners-up and produces the final result (GoT Aggregation Transformation).
- **When there's no dominant winner** (survivors close on pairwise), use a **debate/reconcile round** (Du et al.): let surviving branches read each other and converge — exploits the "all-wrong→right" effect rather than forcing a coin-flip.
- Synthesis is the documented hard part of orchestrator-worker; give it the richest model and an explicit rubric (the Align success criteria again).

**One-line shape:** *Planner seeds 3 (1–5 by stakes) deliberately-distinct approaches → isolated workers explore under a per-branch step budget → objective Align-criteria gate hard-kills failures (with a reflection post-mortem each) → pairwise judge ranks survivors → synthesis pass merges winner + runner-up insight. Breadth and depth tiered to stakes under a global token budget.*

---

## Deep-dive questions

1. **Checkpoint cadence.** How many interim checkpoints per branch before the objective gate runs — and can checkpoints be ticket-defined (Align gate emits milestone criteria, not just final criteria)? Too few = late kills; too many = judge overhead.
2. **Hard vs soft criterion taxonomy.** Does the Align gate already distinguish hard (kill) from soft (demote) criteria, or do we need to add that field? This split is load-bearing for safe pruning.
3. **Diversity enforcement mechanism.** Is "branches must be distinct strategies" enforced by the planner prompt, by a diversity check before launch (reject N near-dupes), or by post-hoc clustering only? Probably all three.
4. **Reflection routing.** Where do branch post-mortems go — only to live siblings, or persisted to long-term negative-result memory (Track 1 overlap)? Likely both; needs a contract with the memory track.
5. **When to debate vs hard-prune.** Threshold for "survivors too close to separate → reconcile" vs "clear winner → synthesize." Pairwise margin? Confidence gap?
6. **Budget accounting unit.** Token budget vs wall-clock vs tool-call count per branch — which does the orchestrator meter, given experiments may be I/O- or GPU-bound, not token-bound?
7. **Verifier trust.** Self-judge (cheap, OOD-risky) vs a stronger separate judge model vs objective execution only. For experiment tickets, can we lean almost entirely on **objective execution** (AlphaCode-style) and use the LLM judge only for unrunnable/qualitative tickets?
8. **Do we ever need true MCTS/UCT?** For most experiment tickets, beam + prune suffices. Is there a ticket class (deep sequential planning) where LATS-style rollouts pay off despite cost?

---

## Sources

- Tree of Thoughts — Yao et al. 2023 — [arxiv.org/abs/2305.10601](https://arxiv.org/abs/2305.10601)
- Graph of Thoughts — Besta et al. 2023 — [arxiv.org/abs/2308.09687](https://arxiv.org/abs/2308.09687)
- Language Agent Tree Search (LATS) — Zhou et al. 2023/ICML 2024 — [arxiv.org/abs/2310.04406](https://arxiv.org/abs/2310.04406)
- Self-Consistency Improves CoT — Wang et al. 2022 — [arxiv.org/abs/2203.11171](https://arxiv.org/abs/2203.11171)
- LLMs-as-Judges survey — 2024 — [arxiv.org/pdf/2412.05579](https://arxiv.org/pdf/2412.05579)
- Generative Verifiers: Reward Modeling as Next-Token Prediction — 2024 — [arxiv.org/abs/2408.15240](https://arxiv.org/abs/2408.15240)
- Pairwise RM + Knockout Tournament (Best-of-N) — [medium summary](https://medium.com/@techsachin/pairwise-rm-test-time-scaling-of-llm-via-best-of-n-sampling-with-knockout-tournament-0d36287f95f5)
- Multiagent Debate — Du, Li, Torralba, Tenenbaum, Mordatch 2023/ICML 2024 — [arxiv.org/abs/2305.14325](https://arxiv.org/abs/2305.14325) · [code](https://github.com/composable-models/llm_multiagent_debate)
- AlphaCode — DeepMind 2022 — [arxiv.org/abs/2203.07814](https://arxiv.org/abs/2203.07814)
- Scaling LLM Test-Time Compute Optimally — Snell et al. 2024 — [arxiv.org/abs/2408.03314](https://arxiv.org/abs/2408.03314)
- TIDE: Trajectory-based Diagnostic Evaluation of Test-Time Improvement — [arxiv.org/pdf/2602.02196](https://arxiv.org/pdf/2602.02196)
- Agent loop / dead-end & stop-condition patterns — [dev.to: llm-stop-conditions](https://dev.to/mukundakatta/your-agent-loop-needs-a-real-exit-llm-stop-conditions-15bf) · [futureagi: infinite loop](https://futureagi.com/glossary/infinite-loop/)
- Claude Code parallel agents / Dynamic Workflows — [code.claude.com/docs/en/agents](https://code.claude.com/docs/en/agents) · [apidog: Opus 4.8 parallel subagents](https://apidog.com/blog/claude-code-dynamic-workflows-opus-4-8/)
