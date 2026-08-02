# research/31 — RAG memory + the deep-research retrieval sub-agent

**Status:** DESIGN v2 (planning step of the /goal workflow). Adversarial review DONE (3 cold reviewers,
find→refute) and folded in — see §8. Next: impl plan → execute S1.
**Date:** 2026-06-11
**Author:** Claude (Opus 4.8), at Gary's direction.

## 0. The ask (Gary, verbatim intent)

> "I think it's time we do full RAG with embedding and reranker too … since we run local LLM we can
> just use it as a deep research deploy with its own context tree that can explore the memory on its
> own and then feed back to the main working agent the relevant context."

Two things, and the second is the spine:
1. **Full RAG substrate** — dense embeddings + hybrid fusion (+ reranker) over the memory store, vs today's
   BM25-only `recall_primed`.
2. **A memory-researcher sub-agent** — a local-LLM (oMLX) deep-research role with its *own* context budget that
   explores the memory store iteratively and hands the main worker a *distilled* relevant-context packet.

## 1. This reopens the Slice B NO-GO — honestly, and under a different use case

`decisions.md` carries **"Memory Slice B (vector leg) — NO-GO"** (research/28). That verdict was deliberately
*narrow*: it answered exactly one question — *does a dense leg beat our **measured** sparse leg by enough to
justify a live oMLX dependency on the **recall hot path**, for an **ambient-hint** use?* On a 14-row probe the
answer was no (floorless sparse 13/14, dense recovers +1, confined to zero-token-overlap paraphrases). The
NO-GO entry *itself* pre-declared the reopen condition ("if the real query distribution proves more
paraphrase-heavy … revive §§2–7 then") and scoped its verdict to the hot-path ambient use — so this is not
relitigation; it walks through a door the prior verdict left open.

Gary's directive changes the **use case**:

| axis | Slice B (NO-GO'd) | This design |
|---|---|---|
| trigger | every worker turn (hot path) | once per ticket, at align/run time (cold path) |
| cost tolerance | must be ~free (ambient) | can afford embed + multi-query + an LLM judge pass |
| mechanism | one-shot BM25 top-5 inject | agentic multi-hop explore + synthesize |

The value proposition is **not** "dense beats sparse" (we measured that it barely does — +1/14, zero-overlap
only). It is the **agentic researcher** — multi-query, body-reading, cross-lesson (multi-hop) synthesis — a
capability the one-shot inject cannot have *regardless* of dense vs sparse. The frontier agrees: the
knowledge-base's `rag-frontier-2026.md` + FrugalRAG (arXiv 2507.07634) put **iterative agentic retrieval**, not
a fancier embedder, at the 2026 frontier.

> **A cost, not a feature (adv-review fix):** the cold/agentic path *introduces* a new, louder failure mode — a
> *confident-wrong* packet the worker trusts (vs an ignorable weak hint). That is a liability this design must
> spend effort to contain (§3 selection-first, §7.3), not a point in its favour. Listed here so it isn't
> mistaken for an upside.

**Integrity rule for this pillar (pre-registered, adv-review-hardened):** we gate by a number on a *properly
powered* golden set (§4), and **the ship bar is floorless BM25 — baseline (b), 13/14 — not the floored 12/14
baseline (a).** Beating (a) is a low bar the t5 floor-fix alone clears (§8 data); the honest question is whether
dense/agentic beats our *best lexical* leg. If it does not, we ship none of it — same discipline that made the
knowledge-base revert enrichment.

## 2. Verified substrate (config-first, not guessed — research/26 lesson)

Probed oMLX live (2026-06-11):
- **Dense embed: LIVE.** `jina-embeddings-v5-text-small-retrieval-mlx`, **1024-dim**, `/v1/embeddings` returns
  clean vectors. Same model family the knowledge-base settled on (Jina v5). Zero new runtime dep — it's an HTTP call.
- **BM25: already shipped.** `memory_fts` FTS5 external-content index (Slice A). `recall`/`recall_primed` use it.
- **Vectors in SQLite: columns already exist, dormant.** `embedding`/`embed_model`/`embed_dim` on the `memory`
  table (migration v2). Brute-force cosine over f32 blobs at our scale (hundreds → low-thousands of lessons) is
  **sub-millisecond** — verified by a reviewer: 2K × 1024-dim f32 = 8 MB, one matmul. **No Qdrant, no vector-DB
  dep** (the knowledge-base needs Qdrant for 48K book chunks; we do not).
- **Reranker: route exists (`/v1/rerank`), no cross-encoder model loaded** (400, not 404). ⇒ a separate
  cross-encoder is a *load-a-model-on-Gary's-box* step, NOT available today. **We do not need it:** the
  researcher sub-agent (the 35B) IS the reranker — it reads candidate bodies and judges/selects in one pass.
  Cross-encoder rerank stays an optional, deferred lever (S4) if the eval says the LLM judge is too slow/weak.

**Net: zero-new-runtime-dep** (oMLX over HTTP + SQLite + Rust cosine/RRF).

## 3. Architecture

Two layers. Layer 1 is reusable plumbing; Layer 2 is the spine Gary asked for. **Adv-review reshaped Layer 2
from "synthesize a distilled packet" to "select-and-cite, synthesis additive" — see the boxed rule below.**

### Layer 1 — hybrid retrieval substrate (`board`, zero-dep)
```
query ─┬─► FTS5 BM25  ───► top-N_bm25  ┐
       └─► oMLX embed ─► cosine over    ├─► RRF fuse ─► top-K candidates (id,title,body,scores)
            SQLite f32 vectors ─► top-N_dense ┘
```
- **Embed-on-write:** `remember()` (and a one-shot backfill) calls oMLX `/v1/embeddings`, stores the 1024-d
  vector + `embed_model`/`embed_dim`. Best-effort: embed failure ⇒ row still written, vector NULL, dense leg
  skips it (degrade, never block — P0's best-effort recall posture).
- **Embed-on-query:** query string → oMLX embed → cosine vs all non-NULL vectors (brute force).
- **One `embed_input(text)` fn, shared by write and query** (adv-review risk 5). It assembles `title + "\n" +
  body` — **identical to the FTS-indexed columns** so the dense and lexical legs agree on what "a document" is.
  (Entities are excluded from both the embed input and the overlap floor; documented so they don't silently
  diverge.) One function, two callers, no drift.
- **Fusion:** **RRF** (`1/(k+rank)`, k=60) over the two ranked lists. RRF not DBSF — DBSF is a Qdrant built-in;
  RRF is ~10 lines of Rust, rank-based, correct at our scale. (DBSF/weighted is a deferred S4 lever.)
- **Model-swap guard:** a vector is usable for cosine only if its `embed_model`/`embed_dim` match the current
  query embedder; mismatches fall back to BM25-only for that row.

### Layer 2 — the memory-researcher sub-agent (reuse the agent loop + a memory tool)
**Decision (Gary, 2026-06-11): do NOT hand-roll a bespoke ReAct loop.** We already have the pattern —
`explore.rs` is an oMLX-pinned subagent that runs the **full agent loop** in isolation and "produces evidence,
never lands." The researcher reuses that plumbing: spawn an **oMLX-pinned subagent**, give it a **read-only
`search_memory` + `recall_body` tool**, and let the real agent loop do the multi-hop exploration on its own
context budget. The agent loop *is* the ReAct loop — seed/reformulate/read/judge fall out of normal tool use,
so there is no second loop to write or maintain. Invoked **once per ticket** at the existing `recall_primed`
integration point in `run_ticket` (main.rs:375). oMLX-pinned (local, free, throwaway — never the paid landing
backend, same rule as `explore`). 35B tool-calling is confirmed supported (Gary); S3's gate still includes a
live tool-call behavior probe (config-first dogfood, costs nothing).

```
researcher(ticket) = subagent(
    system: "You research the memory store for lessons relevant to THIS ticket. Use search_memory to find
             candidates, recall_body to read them. Reformulate queries as needed. Return a ContextPacket:
             selected lesson_ids + ONE optional connective note each. If none are relevant, return NONE.",
    tools: [ search_memory(query) -> [{id,title,type,scores}],     // Layer-1 hybrid, read-only
             recall_body(id) -> body ],                            // read-only
    backend: oMLX (35B), bounded: TOOL_CALL_CAP=12, wall-clock 25s
) -> ContextPacket{ items:[{lesson_id, verbatim_body, note?}], used_ids:[...] }   (or empty → fall back)
```

The two new tools are the *only* new plumbing — the loop today has `read_file`/`write_file`/`bash` (all
worktree-confined, no board access). `search_memory`/`recall_body` are **non-mutating**, so `gate.rs` allows
them in any spine status with **no gate surgery**. The subagent gets the board (read-only) but not the
filesystem tools (it researches memory, it doesn't touch the tree).

> **Selection-first, synthesis-additive (the adv-review blocker fix).** The packet's load-bearing content is the
> **verbatim body** of each selected lesson (the same clamped body `recall_primed` injects today), tagged with
> its `lesson_id`. Any judge-written prose is a SHORT *connective note* shown **beside** the source body, never
> replacing it and never the only thing the worker sees. This kills the "correct-citation + wrong-summary"
> silent-failure: the worker always reads the real lesson, so a bad note is visibly contradicted by the body it
> sits next to. We do not ask the model to distill-and-replace; we ask it to *pick* (the thing LLMs are reliable
> at) and optionally *connect* (the thing it's useful for), with the ground truth always present.

**Floor vs baseline (adv-review fix — no silent regression).** The candidate pool **always includes the
baseline `recall_primed` hits**. The packet is the judge's selection ∪ (a guard); if the judge drops a row the
baseline would have injected, that drop is **logged** (`packet ⊉ baseline`) so per-ticket regression is visible,
and the eval (§4) measures it. NONE / researcher-failure / deadline-exceeded ⇒ **fall back to raw
`recall_primed`** (safety over precision on the empty case). This makes the researcher strictly an *improver or
equal* in the worst case, never a silent context-loss.

**Plumbing:** `PrimedHit` gains an `id` field (today it carries `{title,type,body}` only — the citation
mitigation is unbuildable without it; threading `id` through `recall_primed` is an S2 task).

**Latency budget (adv-review fix — numbers, not vibes).** Cold path, but the unattended `agent run` path has no
human to absorb the wait, so it is bounded hard: per-LLM-call timeout **8 s**, whole-researcher wall-clock
deadline **25 s**, `TOOL_CALL_CAP = 12` tool calls (the subagent loop's hard stop — bounds both bodies read and
re-query rounds in one number). Deadline/cap/timeout exceeded ⇒ return what's gathered, else fall back to
`recall_primed`. Researcher latency is a **reported eval metric**, not an afterthought.

**Why a sub-agent and not just rerank:** multi-hop. "The rework loop doesn't reconverge" should surface *both*
the spine state-machine lesson *and* the align() lesson and connect them — a cross-encoder ranks independently;
an LLM judge reasons across the set. That's the "deep research" Gary wants.

## 4. The eval gate (build FIRST — it is the real deliverable)

The knowledge-base's hardest-won lesson: **build the eval harness first, gate every change by a number on a hard
set.** Its own `gate-audit.md` then taught the *second* lesson the hard way — an underpowered, leaky, single-
positive gate is worse than none because it launders a non-result into a "decision." Adv-review caught this
design repeating those exact failures; §4 is rewritten to not.

**Power, honestly (adv-review blocker fix).** The knowledge-base gate ran at n=200 and its audit *still* called
the power a "decisive failure" (per-query σ≈0.31 on NDCG; recommended n≈500+). A hand-built 50-query set cannot
overturn a *small* effect — and Slice B's dense signal was small (+1/14). So we **do not pretend** a small set
resolves a small effect. Instead we **pre-register a Minimum Detectable Effect and size to it**:
- Primary metric is **Recall@5** (binary per-query hit — a proportion, the audit's most robust metric class),
  compared **paired** (same queries, both arms) via **McNemar on discordant pairs**.
- **Pre-registered MDE:** we only care to ship the researcher if it is a *large, unambiguous* win over best-
  lexical — concretely **≥ +15 percentage-points Recall@5** (e.g. 0.70→0.85). A win smaller than that is, by Wu
  Wei, not worth a per-ticket multi-call LLM dependency. McNemar at 80% power for a 15-pt shift with moderate
  discordance needs **~40–60 paired queries** — which is why ~50 is the *right* size *for this MDE* (and why it
  would be the wrong size for chasing a 3-pt dense delta, which we therefore explicitly decline to chase).
- A sub-MDE result is a **documented null → ship nothing new** (ship only the t5 floor fix, §8). Pre-registering
  the MDE *before* seeing numbers is the integrity mechanism; it cannot be moved afterward (constitution: never
  modify success criteria to fit the result).

**Noise-set = clean precision ground truth (adv-review fix; validated by §8).** Packet precision does NOT need
per-candidate multi-label judgments (which a single-positive golden set can't support). Instead the golden set
includes a **noise arm: queries whose correct answer is EMPTY** (topics with no relevant memory). Every hit on a
noise query is an unambiguous false positive. "Packet precision" = fraction of noise queries that correctly
return empty. This is exactly the t5 noise measurement (§8) generalized, and it sidesteps the unmeasurable-
precision objection entirely.

**Anti-leakage (adv-review fix — the source's *known-failed* pass is not enough).** The knowledge-base's harden
script fed the gold chunk to the paraphraser (author sees the answer); its audit found leakage survived. We do
three concrete things the source didn't:
1. **Provenance split, labeled and reported separately.** (clean) queries are **real ticket titles authored
   before the lesson existed** — structurally leak-free. (suspect) queries are symptom-paraphrases. Recall is
   reported **per provenance**; the ship decision weights the clean arm.
2. **Held-out authoring for the suspect arm:** the paraphrase is written **from the symptom only, with the
   lesson body hidden** from the query author. (Operationally: a separate oMLX pass that sees only a
   one-line symptom, never the target row.)
3. **Overlap threshold drops a query:** any query whose content-word overlap with its target lesson exceeds a
   pre-set ceiling is *cut* (too easy / leaky), and the cut count is reported. Measured overlap is a gate, not
   just a thermometer.

**Arms (all from shipped or buildable code), isolating each lever (adv-review fix):**
- (a) `recall_primed` — BM25 + `need≥2` floor (today's default; the *floor* baseline).
- (b) floorless `recall` — BM25, no floor (**the ship bar**; §8 shows it's the honest baseline at 13/14).
- (b′) **BM25-score-floored** — `need≥1 & bm25≤θ`, θ calibrated on this set (the t5 fix candidate; §8 shows it
  Pareto-dominates (a) on the 14-row probe — must re-confirm on the full set before it ships).
- (c) hybrid substrate (Layer 1), single-query, no agent — isolates the **dense** lever.
- (c′) hybrid, **multi-query, no agent** — isolates the **query-expansion** lever from the agent.
- (d) full researcher (Layer 2) — isolates the **agentic/judge** lever (the delta (d)−(c′) is the agent's true
  contribution; if ~0, ship c′ and stop — Wu Wei).

**Pre-registered ship rule (quantified):**
- t5 floor (b′) ships iff it beats (a) on Recall@5 **and** does not increase noise-set leakage, at p<0.05
  (McNemar), on the full set.
- Researcher (d) ships iff Recall@5(d) − Recall@5(b) ≥ **+15 pts** at **p<0.05** (McNemar, clean-provenance
  arm) **and** noise-set leakage(d) ≤ leakage(b) **and** the agent delta (d)−(c′) is itself positive at p<0.05.
- Per-query result vectors are **persisted** so any paired test is reconstructable later (the knowledge-base
  couldn't re-test its rejection because it stored only aggregates — we won't repeat that).

## 5. Slices (each gated by a number/artifact)

- **S1 — eval harness + golden set + t5 fix.** (`board` test-rig + `research/31` corpus.) The deliverable.
  ~50 paired queries with provenance split + a noise arm; held-out suspect authoring; overlap-cut reporting;
  per-query persistence; McNemar. Encodes arms (a)/(b). **Calibrate and, if it passes the §4 rule, ship (b′)
  the t5 BM25-score floor** — this is the one shippable thing that doesn't depend on the rest. *Gate: harness
  runs, baselines reproduce §8's directional result, (b′) decision recorded with a number.*
- **S2 — Layer-1 hybrid substrate** (embed-on-write + backfill + shared `embed_input` + cosine + RRF + `id`
  threaded through `PrimedHit`). Activates the dormant columns. *Gate: arms (c)/(c′) measured vs (b). **The
  dense leg ships into the substrate only if (c)/(c′) beat (b) under §4; otherwise S2's code stays behind a
  flag, NOT carried "because S3 needs it"** (adv-review: the dense substrate must have a gate that can actually
  stop it). If the researcher (S3) is later shown to need dense, that need is itself a measured §4 result.*
- **S3 — Layer-2 researcher sub-agent** (reuse the agent loop: oMLX-pinned subagent + read-only
  `search_memory`/`recall_body` tools, selection-first packet, wired at the `recall_primed` callsite behind
  `HARNESS_MEMORY_RESEARCHER`). *Gate: arm (d) clears the §4 ship rule (incl. the (d)−(c′) agent-delta test and
  the noise floor), AND a live oMLX tool-call behavior probe. Selection-first packet + verbatim-body +
  regression logging are part of the slice, not follow-ups.*
- **S4 (deferred levers)** — cross-encoder rerank via oMLX `/v1/rerank` (needs Gary to load a reranker);
  DBSF/weighted fusion; worker-callable `search_memory` tool. Only if S1's harness says S3 ran out of headroom.

## 6. Subagent coordination (the /goal "patterns" step)

- **This design's adv review: DONE.** 3 cold reviewers (find→refute), parallel Agent fan-out, over §1–§5 —
  targeting (i) the NO-GO-reopening logic, (ii) eval-set validity/leakage, (iii) the researcher's silent-failure
  surface. All three returned actionable surviving objections; folded in §3/§4/§5 and logged in §8. Two blockers
  (silent-wrong synthesis; underpowered gate) are resolved by the selection-first packet and the MDE/noise-set
  reframe respectively.
- **Build-time:** S1/S2 are solo (mechanical, deterministic-test-gated). S3's researcher is itself a subagent;
  its *eval* can fan out (one judge call per golden query in parallel) — runtime, not coordination.
- **The researcher is NOT the harness's explore-fanout** — a new, read-only role over the board (no worktree).
  It may later share explore's isolation plumbing, but v1 needs none.

## 7. Risks / adversarial seeds (status after review)

1. **Reopening a measured NO-GO on vibes.** RESOLVED: §4 MDE pre-registration; ship bar bound to floorless (b);
   null allowed. Reviewer 1 confirmed the reopening logic is sound (the NO-GO invited it).
2. **Eval-set leakage** — RESOLVED in design: provenance split + held-out suspect authoring + overlap-cut (§4).
   Was the source's documented failure; we go beyond its known-insufficient fix.
3. **Silent-wrong synthesis** — RESOLVED by **selection-first** (§3 box): verbatim body always present, judge
   prose is additive connective notes only. The worst case is an unhelpful note beside a correct lesson.
4. **Latency** — RESOLVED: hard budget (8 s/call, 25 s total, CAND_CAP 12, R=2), fall back on exceed, latency
   reported as a metric (§3).
5. **Embed/recall drift** — RESOLVED: one shared `embed_input(text)` = `title\nbody`, matching FTS columns (§3).
6. **Wu Wei** — is the researcher worth it over best-lexical? The §4 (d)−(c′) agent-delta test + the +15-pt MDE
   answer this *before* S3 ships. §8 already shows the t5 floor fix recovers most of the headroom at zero dep —
   the bar for the researcher is correspondingly high, by design.

## 8. Measured data + adversarial review folded in (2026-06-11)

### 8.1 t5 floor precision counter-probe — RAN (the "noise side" t5 was blocked on)
`cargo test -p board t5_floor_precision_counter_probe -- --ignored --nocapture`, over the real FTS5/BM25 engine,
research/28 14-row corpus, 14 recovery queries + 12 noise queries (topics with no relevant memory), K=5:

```
policy                      recovery    noise(false-pos queries)
A: need≥2 (SHIPPED)         12/14       9/12
B: need≥1 (blind relax)     13/14       12/12
C: need≥1 & bm25≤ -2.0      13/14       6/12      ← Pareto-dominates A: +1 recovery, −3 noise
C: need≥1 & bm25≤ -3.0       9/14       5/12
C: need≥1 & bm25≤ -5.0       5/14       1/12
```
**Findings:** (1) the shipped `need≥2` floor already leaks 9/12 noise — the "floor keeps noise out" premise was
weak. (2) Blind relax (B) recovers the +1 but leaks *everything* (12/12) — strictly worse precision. (3) A
BM25-score floor (`need≥1 & bm25≤−2.0`) **beats shipped on both axes** (13/14 recovery, 6/12 noise). (4) The
last 1/14 recovery gap is the true zero-overlap paraphrase — the *only* place dense could add value (marginal;
gate it). (5) The residual 6/12 noise is what an LLM judge can reject but a score threshold can't — the
researcher's real precision job. **Caveat:** θ=−2.0 is overfit on 14 rows/12 noise queries; S1 re-calibrates and
re-tests on the full set before (b′) touches gated `recall_primed`. This unblocks t5 with a measured direction.

### 8.2 Surviving objections from the 3 cold reviewers, and disposition
**R1 (NO-GO logic) — reopening sound; two fixes:** (i) bind ship bar to floorless (b) not floored (a) → done,
§1/§4. (ii) the dense substrate must have a gate that can stop it; add a researcher-on-BM25-only isolation arm →
done as arm (c′) + S2 gate language. Nit: table "must-gate" row reframed as a cost, §1. *Refuted: relitigation,
precedent-erosion, dense-pointless.*
**R2 (eval/leakage) — gate not fit as written; blocker + 4 should-fixes:** underpowered n (blocker) → MDE
pre-registration + sizing rationale, §4. Anti-leakage hand-waving → provenance split + held-out authoring +
overlap-cut, §4. Unmeasurable precision → noise-set ground truth, §4 (validated by §8.1). Vague ship rule →
quantified McNemar/p<0.05/+15-pt + persisted per-query vectors, §4. Confounded arms → arms (c′)/(d) isolate the
levers, §4. *Refuted: brute-force cosine latency (confirmed sub-ms).*
**R3 (researcher silent-failure) — blocker + 4 should-fixes:** ungrounded synthesis (blocker) → selection-first
verbatim-body packet, §3 box. No runtime floor vs baseline (leaning-blocker) → candidate pool ⊇ baseline +
drop-logging + fall-back, §3. NONE-semantics undefined → fall back to `recall_primed` on empty, §3. Unbudgeted
latency → hard numbers, §3. `PrimedHit` has no `id` → threaded in S2, §3/§5. Nit: `embed_input` document
definition pinned, §3. *Refuted: embed-drift (no second builder exists yet), unbounded recursion (R=2 caps it).*

## 9. Ship-eval status: MEASURED-DEFERRED + the gate is wrong-axis (2026-06-11)

**This does NOT modify §4's pre-registered rule** (integrity: never modify success criteria to fit a result).
It records a measured finding that determines whether §4 *can fire*, and corrects which axis a future eval must
target. The §4 +15pt Recall@5 / McNemar rule stands frozen as written.

**What was measured.** S2a (`PrimedHit.id`) + S3 (researcher.rs + `run_memory_researcher`, behind
`HARNESS_MEMORY_RESEARCHER`, default-OFF) are BUILT (live oMLX tool-call gate passed, 15.15 s). The §4 ship-eval
was deferred on "corpus saturated, +15pt unclearable until the store accumulates real lessons." The lesson store
has since grown **1 → 21 real project-scope lessons**, so I re-tested the premise before building the ~50-query
apparatus. **Hand probe (research/28 §10-B style), live 21-row corpus, shipped `agent recall` = floorless arm (b),
10 ticket-shaped paraphrase-leaning queries (9 real + 1 noise), recall@5:**

- **9 / 9 real queries HIT** — relevant lesson in top-5 every time.
- The deliberate **zero-overlap** query (sandbox ↔ *worktree boundary*, no shared ≥3-char content token with the
  title) still hit **rank 2** (body-vocab overlap carried it — the one case research/28's 14-row probe had dense
  recover, here lexical gets it because the real bodies share file/tool vocabulary).
- The **multi-relevant** query surfaced **both** target gate lessons in the top-3 (one-shot inject would
  rank-separate them; connecting them is the multi-hop job).
- The 1 **noise** query (no relevant lesson exists) leaked 5 false positives — floorless `recall` has no floor.

**Finding 1 — recall is SATURATED (~100%), so §4's +15pt Recall@5 MDE is structurally unclearable.** The corpus
grew but stayed in-domain (lessons reuse harness vocab), so ticket-title queries share tokens with their answers
— the §28-§12 prediction, now holding at 21 rows. There is no recall headroom for the researcher to win; running
the apparatus would manufacture a documented null. **S3 ship-eval stays DEFERRED — measured, not asserted.**

**Finding 2 — the live value-axis is PRECISION, not recall** (consistent with the shipped t5 reframe,
decisions.md: *"the actual defect the eval surfaced was precision, not recall"*). The two axes the probe shows
are actually open: **(a) noise rejection** — the floorless arm leaks on the empty-answer query; an LLM judge can
return NONE where a score threshold can't (the §4 noise-arm, generalized); **(b) multi-hop synthesis** — fusing
the 2+ connected lessons a single query legitimately surfaces into one connective note. **A future researcher
eval must gate on noise-precision + multi-relevant precision, NOT recall@5** — and even that is Wu-Wei-marginal
on a personal tool until the real query distribution proves more paraphrase-heavy than ticket-title reflection
naturally produces. Reviving §§2–7 as a *recall* eval is explicitly NOT warranted; a precision/noise eval is the
only version that could ever fire, and only on a materially different corpus.
