# Track 1: Memory & Learning Loop

## Scope & what it grounds

Grounds **P3 (memory substrate)** and the **memory side of P5 (learning loop)** for the harness. Our design is a two-store memory plane:

- **Wiki** — curated, human+AI-readable prose (the "semantic/procedural" durable knowledge: conventions, architecture notes, distilled lessons, reusable recipes).
- **Sidecar** — raw machine store of classified observations with embeddings (the "episodic" stream: what happened, captured per tool-call/turn).
- **Local MLX embeddings** — embeddings + extraction/reflection LLM calls run on the local OpenAI-compatible MLX server, so marginal cost is ~zero. This is the lever that lets us be generous with capture/reflection where cloud-API systems (claude-mem, Mem0, Hindsight) must ration.
- **Retrieval is top-k per task**, never whole-store injection.

This track answers: (1) observation schema, (2) decay/consolidation policy sidecar→wiki, (3) hybrid retrieval scoring, (4) hot/warm/cold tiering, (5) reflection that generalizes without overfitting.

The mapping to use throughout: **sidecar = episodic memory; wiki = semantic + procedural memory; active workpad/context = working memory.** This is the CoALA taxonomy, and it is the single most useful organizing lens in the literature.

## Landscape

- **MemGPT / Letta** — OS-style virtual context management: a fixed in-context **core memory** + **recall** (conversation) + **archival** (vector store), with the agent *self-editing* memory via tool calls (paging in/out). LongMemEval ~83%. [paper 2310.08560](https://arxiv.org/abs/2310.08560) · [letta](https://github.com/letta-ai/letta)
- **Generative Agents (Park et al., Stanford)** — natural-language **memory stream** + retrieval scored as `recency + importance + relevance`, plus periodic **reflection** that synthesizes higher-level inferences into the same stream. The canonical retrieval formula. [2304.03442](https://arxiv.org/abs/2304.03442)
- **Reflexion (Shinn et al., NeurIPS'23)** — **verbal self-reflection as memory**: convert task feedback into a written lesson, store in an episodic buffer, inject on the next attempt. +8% over episodic-memory-only ablation. The cleanest "learn from failure without weight updates" pattern. [2303.11366](https://arxiv.org/abs/2303.11366)
- **CoALA (Sumers et al.)** — the **memory taxonomy**: working / episodic / semantic / procedural, grounded in Tulving/ACT-R. Descriptive framework that classifies all the others. Key warning: don't apply episodic decay to semantic facts. [2309.02427](https://arxiv.org/abs/2309.02427)
- **Voyager (Wang et al.)** — **skill library as procedural memory**: verified code skills, embedding-indexed by NL description, top-5 retrieved and composed. Accretion, not fine-tuning; transfers across worlds; "alleviates catastrophic forgetting." [2305.16291](https://arxiv.org/abs/2305.16291)
- **Mem0** — fact-centric: an **extraction phase** distills salient facts, then an **update phase** runs ADD/UPDATE/DELETE/NOOP against existing memories to resolve contradictions. Token-efficient, passive. Graph variant `Mem0^g`. [mem0](https://github.com/mem0ai/mem0) · [emergentmind](https://www.emergentmind.com/topics/mem0-system)
- **A-MEM** — **Zettelkasten** agentic memory: atomic notes (context, keywords, tags, timestamp, embedding) with autonomously generated links; new notes *retroactively evolve* old ones. Embedding pre-filter → LLM link/evolve. [2502.12110](https://arxiv.org/pdf/2502.12110)
- **claude-mem** — Claude Code plugin: lifecycle hooks Capture→Compress→Inject; tool outputs (1–10K tok) compressed to ~500-tok typed observations in SQLite+FTS5; **progressive disclosure** at injection (index → drill down) for ~10x token savings. [thedotmack/claude-mem](https://github.com/thedotmack/claude-mem)
- **Nous Hermes "Hindsight"** — **Retain / Recall / Reflect** over memory *banks*; observations are "deduplicated beliefs grounded in evidence, with proof counts and freshness signals" sitting above raw `world`/`experience` facts. Multi-strategy recall (semantic + entity graph). SOTA on LongMemEval. [README](https://github.com/NousResearch/hermes-agent/blob/main/plugins/memory/hindsight/README.md)
- **Forgetting/consolidation theory** — Ebbinghaus exponential decay `R = e^(−t/S)`; memory-strength → slower decay; spaced repetition (each use extends the interval). The biological prior for our decay rule. [forgetting curve](https://en.wikipedia.org/wiki/Forgetting_curve)

## Transferable techniques

**1. CoALA memory taxonomy → store typing.** Working=active workpad, episodic=sidecar, semantic+procedural=wiki. *Map:* tag every wiki entry as `semantic` (facts/conventions) or `procedural` (recipes/skills) and every sidecar row as `episodic`. Enforce CoALA's warning at the policy level: **decay/forgetting applies only to episodic sidecar rows, never to curated wiki prose** — wiki entries are edited or retired by reflection, not aged out by a timer.

**2. Generative-Agents retrieval score → our hybrid ranker.** `score = α·relevance(cosine) + β·recency(exp decay) + γ·importance(salience)`, min-max normalized, top-k that fits a token budget. *Map:* this is exactly our sidecar retriever. Local MLX embeddings make the relevance leg free; recency/importance are stored scalar fields. Start α=1, β=0.5, γ=0.8 (relevance-dominant for a code agent; importance matters more than recency because a 3-week-old "this API rate-limits at N" still bites).

**3. Reflexion verbal reflection → failure→lesson capture.** On a failed/abandoned ticket, generate a short written lesson ("X failed because Y; next time Z") and store it. *Map:* this is the highest-value episodic→procedural promotion path. It feeds the wiki, not just the next attempt. Keep lessons *imperative and conditional* ("when touching auth, run the integration suite — unit tests miss the session bug") so they generalize.

**4. Voyager skill library → wiki procedural section.** Verified, reusable recipes indexed by NL description, top-k retrieved, composed. *Map:* the procedural half of the wiki = a skill library of "how we do X in this repo" entries. **Gate accretion on verification** (Voyager only stores a skill after self-verification passes) — only promote a recipe to the wiki after the work it describes actually landed through the land-gate. This single rule prevents the junk-drawer failure.

**5. Mem0 ADD/UPDATE/DELETE/NOOP → consolidation operations.** Don't append-only; reconcile. *Map:* the sidecar→wiki consolidation pass classifies each candidate insight as ADD (new), UPDATE (refines existing wiki entry), DELETE (contradicts/obsoletes — e.g., we migrated off that library), or NOOP (already known). Cheap with the local LLM. This is what keeps the wiki small and current instead of monotonically growing.

**6. A-MEM linking + retroactive evolution → wiki cross-refs.** Atomic notes with embeddings, auto-linked, and *old notes updated when new ones arrive.* *Map:* nice-to-have for the wiki — when a new lesson supersedes context in an existing entry, edit the old entry rather than stacking a contradiction. The full Zettelkasten graph is over-engineering for v1; adopt the "atomic note + retroactive update" discipline, skip the link graph initially.

**7. MemGPT tiering + self-editing → hot/warm/cold paging.** Fixed in-context core that the agent edits; external stores paged on demand. *Map:* directly informs our tier model (below). Borrow the **paging** idea (load on demand, evict to external) but be cautious of full **self-editing** — see anti-patterns.

**8. claude-mem progressive disclosure → injection discipline.** Inject a lightweight index first (title/type/date, ~50–100 tok each); fetch full narrative only for the IDs the agent drills into. *Map:* our retriever returns a ranked *index* of observation titles to the coordinator; workers fetch full bodies for the few they need. This is the primary defense against context bloat as the store grows. Scope all retrieval to the current project by default (claude-mem does this).

**9. Hindsight observation-vs-evidence layering → two-level sidecar.** Raw `world`/`experience` facts feed deduplicated `observation` beliefs carrying **proof counts + freshness signals**. *Map:* a belief recalled/confirmed N times is more trustworthy and more promotable. Store a `proof_count`/`usage_count` on consolidated insights; use it as the promotion signal.

**10. Ebbinghaus decay + spaced-repetition reinforcement → the decay scalar.** `retention = salience · e^(−λ·Δt) + reinforcement(usage)`. Each use boosts retention and effectively extends the interval (memory strength). *Map:* this is our eviction function for the sidecar. Concrete values harvested from the field: dedup near-identical at **cosine ≥ 0.92**; mark evictable below a **cold threshold ≈ 0.15**; reinforcement boost ≈ `Σ 1/daysSinceAccess`.

## Anti-patterns / what to avoid

- **Whole-store injection.** The thing we're explicitly avoiding; the literature agrees retrieval must be top-k. Even claude-mem's index-first approach is a response to this.
- **Why claude-mem got expensive — two cost centers, both relevant to us.** (a) *Compression cost*: every session end runs an LLM summarization pass over raw tool outputs. claude-mem keeps this cheap by using Haiku (~16.5K tok/session, <\$1/mo) — **for us this cost is ~zero on local MLX, so we can compress more aggressively and more often.** (b) *Injection cost (the real, unavoidable one)*: injected memory permanently competes with task reasoning for context window. Local embeddings do **not** solve this — every token recalled is a token not spent on the task. Mitigation is retrieval discipline (top-k + progressive disclosure + project scoping), not cheaper compute. Don't let "embeddings are free" tempt us into recalling more.
- **Append-only stores / "junk drawer" semantic memory.** The most common failure: systems nail write+read, neglect *manage*. Without ADD/UPDATE/DELETE consolidation and verification-gated promotion, the wiki rots into contradictory sludge. Mem0's own documented weakness is residual duplicates.
- **Applying episodic decay to semantic facts (CoALA's warning).** Aging out a wiki convention because it wasn't "accessed recently" causes correctness regressions. Decay is sidecar-only.
- **Unbounded self-editing memory (MemGPT risk).** "If the model fails to save it, it's gone," and every memory op burns inference tokens + invites the agent to corrupt its own store. Prefer a **deterministic capture pipeline** (hooks write the sidecar automatically) over relying on the agent's judgment to remember. Reserve LLM judgment for consolidation/reflection, not primary capture.
- **Over-fit reflections.** A lesson like "rename `foo` to `bar` in `auth.py`" is useless next week. Force reflections through a generalization prompt ("state the transferable rule, not the specific edit") and discard ones that don't generalize.
- **Premature graph complexity.** A-MEM/Zep knowledge graphs are powerful but heavy; flat vector + scalar scoring (Generative Agents style) gets ~80% of the value for v1.
- **Security: claude-mem shipped an unauthenticated HTTP API on :37777 (rated HIGH risk).** If our sidecar exposes any local API, bind localhost + auth from day one.

## Recommendation for our design

### Observation schema (sidecar row)

Synthesized from claude-mem (typed/FTS), Generative Agents (importance), Hindsight (proof/freshness), A-MEM (atomic+embedding):

```
id                serial
ts                epoch (indexed desc)        # recency leg
type              enum: decision|bugfix|feature|refactor|discovery|lesson|constraint
title             short heading (the index line shown at injection, ~1 line)
body              narrative (fetched only on drill-down)
salience          float 0–1                   # LLM-rated importance, Generative-Agents style
scope             enum: ticket|project|global # drives hot/warm/cold tiering
entities          string[]                    # files, symbols, libs, APIs (auto-extracted)
files             string[]                    # files_read/modified, for code-locality recall
project           string                      # default retrieval filter
ticket_id         string                      # links observation → work item
embedding         vector (local MLX)          # relevance leg
usage_count       int default 0               # incremented on retrieval; reinforcement
last_used_ts      epoch                        # spaced-repetition input
links             id[] (optional)             # A-MEM-style, deferred to v2
```

`type`, `salience`, `scope`, `entities`, and `embedding` are the five fields that make a row *retrievable and useful* — type for filtering, salience for ranking, scope for tiering, entities for code-locality, embedding for semantic match.

### Decay rule (sidecar eviction + promotion)

Per row, recompute on access and on a scheduled "sleep" pass:

```
retention = salience · e^(−λ · Δt_days) + Σ_uses (1 / days_since_use)
```

- `Δt` from `last_used_ts`; λ tuned so an unused salience-0.5 row drops below threshold in ~2–3 weeks.
- `retention < 0.15` → **evictable** from sidecar (cold-delete or archive-compress).
- High `usage_count` + high `salience` → **promotion candidate** to wiki (it's proven useful repeatedly = it's a durable fact/recipe, not a transient event).
- Dedup near-identical rows at **cosine ≥ 0.92** before they accumulate.
- **Wiki entries do not decay** — they are UPDATE/DELETEd by consolidation, per CoALA.

### Retrieval scoring (top-k hybrid)

```
score = 1.0·relevance + 0.5·recency + 0.8·salience   (each min-max normalized to [0,1])
recency = e^(−0.005 · hours_since_last_used)          # Generative-Agents decay factor
```

- Pre-filter by `project` (+ `scope` ≥ current tier); optional `type`/`entities` filter.
- Return a **ranked index** (id + title + type + ts) to the coordinator → progressive disclosure; workers fetch `body` only for chosen ids, then bump `usage_count`/`last_used_ts`.
- Top-k chosen by token budget, not fixed count (fill the allotted memory budget, stop).
- v2: rerank the top ~30 with a local cross-encoder / RRF fusion of dense+FTS for precision.

### Tier model (hot / warm / cold)

| Tier | What | Backing | Paging |
|------|------|---------|--------|
| **Hot** | active ticket: workpad + this ticket's observations (`scope=ticket`) | in-context / working memory | always present; evicted to warm on ticket close |
| **Warm** | current project: sidecar rows + project wiki (`scope=project`) | local vector store + wiki files | retrieved top-k per task (the common path) |
| **Cold** | cross-project: global lessons/conventions (`scope=global`) | global wiki + global sidecar | retrieved only on explicit miss or broad query; lower default weight |

MemGPT-style paging: load on demand, evict outward (hot→warm→cold) as scope widens; never hold cold in context by default. Scope tagging at capture time drives the tier; this is cheaper and more predictable than agent self-paging.

### Reflection method (episodic → generalized lesson)

Two triggers, Generative-Agents + Reflexion hybrid:

1. **Importance-accumulation trigger** (Generative Agents): when summed `salience` of new sidecar rows since last reflection exceeds a threshold, run a reflection pass.
2. **Land-gate / failure trigger** (Reflexion + Voyager): on ticket land *or* abandonment, distill a lesson.

The reflection pass: cluster recent rows (embedding) → for each cluster, LLM (local) extracts a **generalized, conditional, imperative** lesson → classify against existing wiki via **ADD/UPDATE/DELETE/NOOP** → only ADD/UPDATE durable, transferable rules to the wiki. **Verification gate**: a procedural recipe is promoted only if the work it describes passed the land-gate (Voyager's self-verification analog). Keep generalization honest with an explicit prompt constraint ("state the rule independent of these specific filenames; if you can't, discard"). Carry a `proof_count` so a lesson confirmed by multiple tickets ranks above a one-shot guess.

## Deep-dive questions (to chase later)

- λ and the three retrieval weights (α/β/γ) need empirical tuning on our own traffic — start with the values above, instrument `usage_count` to learn whether recency or salience is actually predictive of usefulness for code work.
- Local cross-encoder reranker vs. RRF(dense, FTS5): which gives better top-k precision at our store sizes, and is the latency acceptable on the MLX box?
- Conflict resolution policy when a new lesson contradicts a curated wiki convention — auto-DELETE, or flag for human review in the curation step? (Human+AI wiki implies a review affordance.)
- Cross-project promotion: when does a `project`-scoped lesson earn `global` scope? Threshold on how many distinct projects independently produced it?
- Reflection cadence: per-ticket vs. nightly "sleep" batch — does batching produce better-generalized lessons (more context to cluster) at the cost of staleness?
- Embedding model choice on MLX: which local embedder balances quality vs. throughput for continuous capture, and do we re-embed on model upgrade?
- Should the sidecar store *failed* trajectories as first-class (Reflexion) or only distilled lessons? Storage is cheap locally; retrieval noise is the cost.

## Sources

- MemGPT/Letta: https://arxiv.org/abs/2310.08560 · https://github.com/letta-ai/letta · https://research.memgpt.ai/
- Generative Agents: https://arxiv.org/abs/2304.03442
- Reflexion: https://arxiv.org/abs/2303.11366 · https://proceedings.neurips.cc/paper_files/paper/2023/file/1b44b878bb782e6954cd888628510e90-Paper-Conference.pdf
- CoALA: https://arxiv.org/abs/2309.02427
- Voyager: https://arxiv.org/abs/2305.16291 · https://voyager.minedojo.org/
- Mem0: https://github.com/mem0ai/mem0 · https://www.emergentmind.com/topics/mem0-system
- A-MEM: https://arxiv.org/pdf/2502.12110
- claude-mem: https://github.com/thedotmack/claude-mem · https://docs.claude-mem.ai/architecture/database
- Nous Hermes Hindsight: https://github.com/NousResearch/hermes-agent/blob/main/plugins/memory/hindsight/README.md · https://hindsight.vectorize.io/
- Forgetting curve / consolidation: https://en.wikipedia.org/wiki/Forgetting_curve · https://pmc.ncbi.nlm.nih.gov/articles/PMC4492928/
- Consolidation/promotion patterns: https://towardsdatascience.com/a-practical-guide-to-memory-for-autonomous-llm-agents/ · https://github.com/TeleAI-UAGI/Awesome-Agent-Memory
