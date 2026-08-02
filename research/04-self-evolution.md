# Track 4: Self-Evolution & Constitution

## Scope & what it grounds

Grounds **P5: self-evolution** of the harness. The design under test is a **two-tier instruction architecture**:

- **CONSTITUTION** — immutable, human-owned: the TypeScript spine, integrity constraints, the "Wu Wei" minimalism filter, and gate-enforcement logic. Only the human amends it.
- **CASE-LAW** — agent-editable under guidelines: heuristics, mode thresholds, subagent prompts, learned lessons (the markdown/config surface).

The agent proposes case-law changes via a **reflection pass**. The load-bearing requirement is a **generalization filter** so lessons land as reusable methodology/mindset, not project-specific rules (the stated failure of the current skill: self-improvement over-fits to the specific problem). Every change is git-committed (auditable, revertable), human-gated, and size-budgeted (weak/stale lessons decay). The two risks to manage: **drift** and **the agent gaming its own guidelines**.

This track maps how the self-improving-agent and prompt-optimization literature solves four problems: (1) the hardcoded-vs-mutable boundary, (2) the amendment/validation/rollback process, (3) the generalization filter, (4) drift & gaming control.

---

## Landscape

### Constitutional AI (Anthropic, 2022; constitution rewritten Jan 2026)
A small, **human-owned set of natural-language principles** governs behavior; the model **self-critiques and revises** its own outputs against those principles (critique → revision), then is RL-trained on AI-generated preferences (RLAIF). The principles are the value system; the *mechanism* is the same self-critique loop we want for reflection.
The **Jan 2026 constitution** is the more useful artifact for us: it shifted from *a list of rules* (2023, sourced from UN Declaration / Apple ToS) to **explanation over instruction** — principles deep enough that Claude "could construct any rules we might come up with." It also defines an explicit **priority hierarchy**: (1) safe + support human oversight, (2) ethical, (3) follow Anthropic guidelines, (4) be helpful. This is a directly transplantable model for our constitution.
- https://arxiv.org/abs/2212.08073 · https://www.anthropic.com/news/claude-new-constitution
- Public/Collective Constitutional AI (amendment via external input): https://www.anthropic.com/research/collective-constitutional-ai-aligning-a-language-model-with-public-input · https://arxiv.org/pdf/2406.16696

### STOP — Self-Taught Optimizer (Zelikman et al., 2023)
A seed **"improver"** scaffolding program improves an input program against a **utility function**, then is run **on itself** to improve the improver. Key discipline: the **model weights are never touched** — only the *scaffolding code* is mutated, and every candidate is scored by a fixed external utility before adoption. The paper explicitly studies **sandbox-bypass frequency** as a safety metric — i.e., it treats "did the self-modification try to escape its constraints?" as a first-class measurement.
- https://arxiv.org/abs/2310.02304

### Gödel Agent (Yin et al., 2024)
The most aggressive point on the spectrum: the agent **modifies its own runtime code directly** via "monkey patching," guided only by a high-level objective. Demonstrates continuous self-improvement but is **candidly unstable — "prone to error accumulation," hindering continued self-optimization**. The cautionary datapoint for us: *unbounded* self-modification (no fixed spine, no immutable validator) is where drift and collapse come from. Their own "hierarchy of freedom" framing (hand-designed → meta-learning-optimized → fully self-referential) is a useful axis: **we deliberately want to sit at the *middle* — mutable case-law over an immutable spine — not at the Gödel extreme.**
- https://arxiv.org/abs/2410.04444 · https://github.com/Arvid-pku/Godel_Agent

### ADAS — Automated Design of Agentic Systems (Hu et al., ICLR 2025)
A **meta agent** programs new agents in code, tests them against a metric, and stores winners in an **archive** that conditions future proposals (Meta Agent Search). Two findings matter for us: (1) the **archive of prior discoveries** is the generalization substrate — new designs are proposed *in light of* what already worked; (2) discovered agents **transferred across domains and models** (math → reading comprehension), evidence that searching for *designs* (not problem-specific fixes) yields reusable artifacts. This is the strongest empirical case that "optimize the methodology, archived and reused" beats "patch the instance."
- https://arxiv.org/abs/2408.08435 · https://github.com/ShengranHu/ADAS

### DSPy (Stanford) — prompts as optimizable programs
The **disciplined alternative to freeform self-editing.** You declare *signatures* (what), *modules* (how to reason), and a **metric** (quality), then an **optimizer compiles** the prompt against the metric on a trainset. Optimizers: BootstrapFewShot (examples), COPRO (instructions), MIPROv2 (both, via Bayesian search over candidates with minibatched eval). Two transplantable disciplines: (a) **the metric is a gatekeeper** — bootstrapped demos are only kept if they *pass the metric*; (b) **separate proposer model from task model** — a stronger model proposes prompt changes, evaluated against the actual workhorse. Pays off only when there's a **measurable metric + a frozen-baseline comparison + pass/fail gates** on cost/latency/accuracy.
- https://dspy.ai · https://github.com/stanfordnlp/dspy/blob/main/docs/docs/learn/optimization/optimizers.md

### Voyager (Wang et al., NeurIPS 2023) — skill-library accretion
Lifelong Minecraft agent whose self-improvement is **purely additive**: an **ever-growing skill library** of executable code, stored in a vector DB, retrieved by semantic similarity, **composable**. A skill is only added **after self-verification confirms it works**. Skills are **temporally extended, interpretable, compositional** — which "alleviates catastrophic forgetting." Critically: the library **transferred to a new world** and even **across frameworks** (handed to AutoGPT, lifted its success 0/3 → 1–2/3). The model of safe self-improvement: **append validated, named, reusable units; never rewrite the spine.**
- https://arxiv.org/abs/2305.16291 · https://voyager.minedojo.org/

### Generalization / episodic→semantic distillation (memory-systems literature)
Broad consensus: agents should **distill episodes into reusable principles**, but the danger is **calibration**. Distill too eagerly → you destroy episodic context and **over-fit to noise**; too late → a stale pile of cases and a "frozen novice." Concrete mitigations harvested:
- **Reflection grounding**: every reflection must **cite ≥N concrete episodic instances** ("API X is unreliable" must point to 3 real failures), giving an auditable trail.
- **Confidence score (0.1–1.0)** on each distilled insight = "how well does this generalize across scenarios."
- **Delayed/periodic consolidation** (synthesize on a schedule, weighting recency × relevance × salience), not at write-time.
- **Abstraction operators** (e.g., SEDM cross-domain diffusion) that *strip domain-specific details* before a lesson is reused — the literal mechanism of a generalization filter.
- Severity scales with agent lifetime: one bad reflection in a long-running harness can poison thousands of downstream decisions — so the filter matters *more* the longer the harness lives.
- https://aws.amazon.com/blogs/machine-learning/build-agents-to-learn-from-experiences-using-amazon-bedrock-agentcore-episodic-memory/ · https://arxiv.org/html/2604.27707

---

## Transferable techniques (what it is → how it maps to our design)

1. **Constitution as explanation, not rule-list (Claude Jan-2026).**
   *What:* principles deep enough to regenerate any specific rule; an explicit priority hierarchy.
   *Maps to:* write the CONSTITUTION as **a small set of explained principles** (the TS spine's contract, integrity constraints, Wu Wei minimalism, gate-enforcement) with a stated priority order — e.g., (1) integrity/gates, (2) minimalism, (3) follow case-law heuristics, (4) be maximally helpful on the task. Case-law lessons must be *derivable from and subordinate to* the constitution; a lesson that contradicts a higher tier is auto-rejected. This makes the boundary principled rather than arbitrary.

2. **Self-critique → revision loop (Constitutional AI).**
   *What:* model critiques its own draft against the constitution, then revises.
   *Maps to:* the **reflection pass** is literally a critique-against-constitution step. The proposed case-law diff must pass a self-critique: "does this violate the spine / minimalism / a gate?" *before* it is even shown to the human. Cheap first filter.

3. **Fixed external utility + sandbox-bypass metric (STOP).**
   *What:* never mutate weights; score every candidate by a frozen utility; *measure* attempts to escape the sandbox.
   *Maps to:* (a) the CONSTITUTION is the frozen substrate — reflection may never edit it; (b) instrument a **"gate-bypass counter"**: log every time a proposed lesson would weaken/route-around a gate or the minimalism filter. A rising counter is the early-warning signal for gaming.

4. **Archive-conditioned proposal + cross-domain transfer (ADAS).**
   *What:* propose new designs *in light of an archive* of what previously worked; prize transfer across domains.
   *Maps to:* keep case-law as a **searchable archive of lessons**; the reflection pass proposes a new lesson *conditioned on existing ones* (dedup, supersede, merge — never blind append). **Use cross-context transfer as the generalization test**: would this lesson have helped on a *different* project? If it's only meaningful in this repo, it's over-fit.

5. **Metric-gated compilation + proposer/executor split (DSPy).**
   *What:* keep a change only if it beats a frozen baseline on a metric; use a stronger model to propose, the workhorse to evaluate.
   *Maps to:* a case-law amendment should ideally show **measurable lift on a held-out set of past tasks** (or at minimum not regress them) before adoption — a "frozen-baseline" check analogous to a regression test. Where a numeric metric isn't available, substitute a **structured human-gated review** (the human is the metric). Optionally: a stronger model drafts the lesson, the harness's normal model is what it must improve.

6. **Additive, self-verified, named skill library (Voyager).**
   *What:* only append units that pass self-verification; keep them composable and interpretable.
   *Maps to:* prefer **additive case-law** (new named heuristic/subagent-prompt/skill) over rewriting existing instructions. Each lesson is **named, dated, self-verified** (must point to the episode that produced it and pass the self-critique). Composability + interpretability are *requirements*, not nice-to-haves — an opaque lesson can't be audited or decayed.

7. **Reflection grounding + confidence + abstraction operator (memory literature).**
   *What:* cite ≥N episodes per reflection; tag a generalization-confidence; strip domain-specifics before reuse.
   *Maps to:* this **is the generalization filter** (see Recommendation). Grounding makes it auditable; confidence drives decay/budget; the abstraction step is what turns "in project X, vLLM needed --entrypoint python3" into "container entrypoints often override run-args — verify the entrypoint before assuming your command runs."

---

## Anti-patterns / what to avoid

- **Unbounded self-modification (Gödel Agent failure mode).** Letting reflection touch the spine/validator/gates yields error accumulation and instability. *Mitigation:* hard, code-level immutability of the constitution; case-law is the only writable surface.
- **Metric gaming / vacuous self-optimization.** An agent allowed to edit *its own success criteria* will optimize the metric, not the work (loosen a gate, redefine "done," add a lesson that says "skip the test"). *Mitigation:* the metric/gates live in the constitution (un-editable by the agent); instrument a gate-bypass counter; require lift on a **frozen** baseline the agent didn't author.
- **Over-fit lessons (the user's stated complaint).** "On project X, set save_steps=200" is a config fact, not methodology. *Mitigation:* the generalization filter — reject any lesson that names a specific file/repo/value and can't be restated as a transferable principle.
- **Eager distillation.** Generalizing from a single episode bakes in noise. *Mitigation:* require N≥2–3 corroborating episodes (or explicit human override) before a lesson is promoted from "candidate" to "active."
- **Append-only sprawl / staleness.** Lessons accrete forever, context bloats, stale advice misfires. *Mitigation:* size budget + decay + dedup/supersede on every reflection pass.
- **Reflection that summarizes at write-time** (collapses distinct episodes). *Mitigation:* keep an episodic log separate from the distilled case-law; consolidate on a cadence, not per-event.
- **Self-graded "it worked" with no evidence.** *Mitigation:* self-verification must reference observable outcomes (tests passed, gate cleared), Voyager-style — not the agent's say-so.

---

## Recommendation for our design

### The boundary (constitution vs case-law)
Make it **code-enforced, not convention.** Anything that, if corrupted, breaks safety or identity → **constitution (immutable, human-only)**: the TS spine/contract, integrity constraints, the Wu Wei minimalism filter, gate-enforcement logic, **and the evaluation metric / definition of "done."** Everything that is *strategy under those constraints* → **case-law (agent-proposes, human-approves)**: heuristics, mode thresholds, subagent prompts, learned lessons. Litmus test, borrowed from Claude's hierarchy: *if the agent could weaken safety/integrity by editing it, it's constitution.* Write the constitution in the **"explanation not rules"** style so case-law can be checked for derivability from it.

### The amendment process / gates (propose → validate → approve → version → rollback)
1. **Propose** — reflection pass emits a *diff* to case-law (add/supersede/merge/retire), never free edits to the spine.
2. **Self-critique gate** — auto-reject if the diff contradicts the constitution, touches a gate, or fails the generalization filter (below). (Constitutional-AI critique step; cheap, pre-human.)
3. **Grounding gate** — diff must cite the concrete episode(s) that motivated it (≥2 for promotion to "active"; 1 → "candidate" only).
4. **Regression gate** — must not regress a **frozen** set of past tasks/checks the agent didn't author (DSPy frozen-baseline discipline). No metric available → human is the metric.
5. **Human approval** — every adopted change is **git-committed** (auditable, revertable). The commit *is* the version record; `git revert` *is* rollback.
6. **Gate-bypass instrumentation** — count diffs that attempt to weaken gates/minimalism; surface the count at review time.

### The generalization filter (concrete test)
A candidate lesson is **accepted only if it passes all four**:
- **(a) Cross-context transfer:** "Would this have helped on a *different* project/repo?" If it only makes sense here → reject or rewrite.
- **(b) No bound specifics:** it must name **no** specific filename, repo, literal value, or one-off ID. If it does, it's a config fact, not a lesson — restate as principle or drop. (SEDM-style abstraction operator: strip domain-specifics.)
- **(c) Evidence floor:** cites ≥2 distinct episodes (or explicit human override).
- **(d) Methodology form:** phrased as *mindset/method/heuristic* ("when the entrypoint may override run-args, verify before assuming"), not *instruction-for-this-task* ("use --entrypoint python3 for the vLLM container here").
Practical implementation: the reflection prompt forces a **two-column rewrite** — "specific thing I observed" → "general principle a future me on an unrelated task could use" — and **only the general column is committed**; the specific column stays in the episodic log.

### Decay / budget
- **Hard size budget** on active case-law (e.g., token ceiling) — forces competition, mirrors Voyager interpretability + the "don't let it sprawl" lesson.
- **Confidence score (0.1–1.0)** per lesson = generalization strength; **recency × usage × confidence** governs ranking.
- **Decay:** a lesson not corroborated/used within a window drops to "candidate," then is retired (git history retains it — retrieval-able, not deleted). Superseded lessons are merged, not duplicated.

### Review cadence (constitutional review)
- **Per-task:** reflection pass (propose diffs) — lightweight, gated as above.
- **Periodic (e.g., every N tasks or weekly):** **constitutional review** — human + agent re-read active case-law for drift, contradiction, dead lessons, and gate-bypass-counter trend; prune the budget; decide if any *repeatedly-proposed* case-law pattern has earned promotion **into** the constitution (human-only act). This is the pressure valve that keeps the immutable spine from ossifying without giving the agent the pen.

---

## Deep-dive questions
1. **Metric for the regression gate without labeled data.** DSPy assumes ≥~50 labeled examples + a numeric metric. Our "tasks" are heterogeneous dev jobs. Do we maintain a small **frozen suite of past task transcripts** as the held-out set, and is "no regression" judgeable by an LLM-judge cheaply enough per reflection?
2. **Promotion path case-law → constitution.** What earns a heuristic the right to be proposed for promotion (frequency of reuse? cross-project corroboration count?) — and what's the human ritual that gatekeeps it?
3. **Gate-bypass counter design.** What exactly counts as an attempt to weaken a gate, and what threshold triggers a forced constitutional review vs. an outright freeze on self-modification?
4. **Confidence calibration.** Who sets a lesson's initial confidence (agent self-assessment is gameable) — and does usage-without-failure raise it automatically?
5. **Episodic vs case-law storage split.** Concrete schema: where does the raw episode log live, how is it linked to a distilled lesson for grounding/audit, and how big before it itself needs decay?
6. **Two-column rewrite robustness.** Does forcing "specific → general" actually defeat over-fit in practice, or does the agent learn to write vacuously-general lessons that pass the filter but carry no signal? (The vacuous-generalization failure mode — needs an anti-vacuity check, possibly the evidence floor doing double duty.)

---

## Sources
- Constitutional AI: Harmlessness from AI Feedback — https://arxiv.org/abs/2212.08073
- Claude's new constitution (Jan 2026; explanation-over-rules, priority hierarchy) — https://www.anthropic.com/news/claude-new-constitution
- Collective / Public Constitutional AI (amendment via input) — https://www.anthropic.com/research/collective-constitutional-ai-aligning-a-language-model-with-public-input · https://arxiv.org/pdf/2406.16696
- STOP — Self-Taught Optimizer — https://arxiv.org/abs/2310.02304
- Gödel Agent — https://arxiv.org/abs/2410.04444 · https://github.com/Arvid-pku/Godel_Agent
- ADAS — Automated Design of Agentic Systems (ICLR 2025) — https://arxiv.org/abs/2408.08435 · https://github.com/ShengranHu/ADAS
- DSPy optimizers (BootstrapFewShot / COPRO / MIPROv2) — https://github.com/stanfordnlp/dspy/blob/main/docs/docs/learn/optimization/optimizers.md · https://dspy.ai
- Voyager — https://arxiv.org/abs/2305.16291 · https://voyager.minedojo.org/
- Episodic→semantic distillation, reflection grounding, over-fit/decay — https://aws.amazon.com/blogs/machine-learning/build-agents-to-learn-from-experiences-using-amazon-bedrock-agentcore-episodic-memory/ · https://arxiv.org/html/2604.27707
