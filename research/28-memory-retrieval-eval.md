# 28 — Memory Retrieval Eval (Slice B GO/NO-GO + Rust-native survey)

> **The research project.** research/18 §10.5 pre-registered exactly one gate for the memory vector leg
> (Slice B): *build it only if a golden-set eval shows lexical FTS5 is measurably insufficient on
> harness-shaped queries.* This doc is the eval design — written to be **attacked** by the adversarial
> review (the next gate). Claims are testable, the decision rule is **pre-registered** (locked before
> results, per the integrity constraint "never modify success criteria to fit the result"), thresholds are
> numeric. Second leg: a Rust-native retrieval-engine survey (frankensearch / fast_vector_similarity) that
> turns a GO into a **build-vs-vendor** call.

## 0. Grounding (KB gate + prior research)

**KB (Production RAG Guide, ch.1/4/8 — searched "retrieval evaluation methodology golden set relevance judgments Recall MRR nDCG"):**
- `KB → applied` Recall@k / MRR / nDCG@k are the retrieval-quality metrics; **separate retrieval eval from generation eval** (we only test retrieval here). Formulas locked in §4.
- `KB → applied` **prototype phase = 20–50 queries, manual spot-check**; pre-production = 100–500. Our ~40 is correct for our phase — the small-N is *by the book*, not a shortcut (report CIs anyway, §6).
- `KB → applied` **LLM-as-judge has self-preference bias** (prefers its own model family) + position/verbosity/sycophancy → *use a different model as judge, never rely on it alone*. This is the spine of the anti-circularity design (§5).
- `KB → applied` **exact-match relevance is "too strict" — relevant docs may not share query terms** → mandates a **paraphrase-query subset** (§3) as the discriminating test: the only place vector can beat lexical.
- `KB → applied` quality-gate exemplar `recall@10 > 0.8, MRR > 0.7` → calibrates our §7 thresholds.

**Prior research (already done — don't redo):**
- **research/01** — full *literature* survey (CoALA, Generative Agents, Reflexion, Voyager, Mem0, A-MEM, claude-mem, Hindsight, Ebbinghaus). The paper leg is complete; this project does **not** re-survey papers.
- **research/14** — Rust-native constellation. P3 candidates: **`frankensearch`** (Tantivy BM25 + MiniLM vector + RRF — architectural twin of our Slice A+B+fusion), **`fast_vector_similarity`** (vector scoring primitives), `cass_memory_system`, `coding_agent_session_search`.
- **research/18** — buildable design + adversarial review. Slice A (lexical FTS5 + `recall`/`recall_body`) BUILT; §10.1 fixed the fusion math (all-signals rank-fusion); §10.5 pre-registered this gate.

## 1. The single decision this gates

**Do we build the Slice B vector leg?** (oMLX jina-v5 embeddings + cosine scan + canary + RRF fusion + dedup —
real dependency + complexity.) research/18 §10.4 already *predicts NO-GO* on a prior: "harness queries are
exact-token (symbols, paths, error strings) — FTS5's strength." This eval converts that prior into a measured
verdict. **Either outcome is a win:** NO-GO closes the pillar honestly (lexical suffices, dependency avoided);
GO unlocks Slice B with the build-vs-vendor leg (§8) ready.

Non-goals: not testing whether memory *helps task outcomes* (that's the end-to-end study, separate; today's P0
probe already showed the model *uses* primed memory). Not testing reflection/decay (Slice C). Retrieval only.

## 2. Corpus (synthesized from our artifacts — Gary's call)

A retrieval eval needs a store to retrieve from; the real `memory` table is near-empty. We synthesize a corpus
**from our own real artifacts** so the distribution matches what reflection will actually produce (external
validity), preserving real phrasing.

**Sources (real, provenance-tagged):** `wiki/decisions.md` (decisions + rejected approaches), `wiki/log.md`
(session lessons), `research/*.md` (findings/verdicts), git commit messages + the explore reflection-on-kill
post-mortems already in the loop, `wiki/active-work.md` breadcrumbs.

**Row shape** = the real v2 `memory` schema (research/18 §10.2): `type` (decision/lesson/constraint/discovery/
bugfix/feature/refactor), `title` (≤120ch index line), `body`, `entities[]`, `files[]`, `scope`, `project`,
and a **`source_ref`** (which artifact + line it was distilled from — the provenance anchor for ground truth,
§5). Target **~150–250 rows** (a stressing set: more rows = more chances for a near-miss to outrank the truth).

**Realism gate:** before eval, a human (Gary) or a decorrelated reader samples ~15 rows and confirms they read
like genuine harness memories, not LLM boilerplate. A corpus that fails realism invalidates the verdict — stop
and regenerate.

## 3. Golden query set (~40 queries, two subsets by construction)

Queries are **ticket-shaped** (what `recall_primed(ticket.title, …)` actually receives — the real query
distribution). Sources: real past ticket titles from the board + realistic near-future task phrasings from the
roadmap (§12.2 pillars).

**Two pre-registered subsets** (the discriminating split — analyzed separately, §7):
- **EXACT (~25):** the query shares surface tokens (symbols/paths/error strings) with its relevant rows —
  FTS5's home turf. If lexical loses *here*, something is broken.
- **PARAPHRASE (~15):** the query is deliberately worded to **avoid** lexical overlap with the relevant rows
  (synonyms, conceptual phrasing — "stop the worker from clobbering files outside its sandbox" vs a memory
  titled "worktree confinement for file tools"). **This is the only subset where vector can win.** If lexical
  holds up on PARAPHRASE, the vector leg has no job.

**Hard negatives:** the corpus includes token-colliding distractors (e.g. "Align gate" vs "CI gate", "context
window" vs "context priming") so high Recall isn't a free lunch from a sparse store.

**Difficulty tags** (KB): easy (1 relevant row) / medium (2–4, synthesis) / hard (ambiguous). **Negative
queries** (~5): no relevant row exists → the system should return empty/low-score, testing the §10.1 relevance
floor (noise-injection guard), not Recall.

## 4. Metrics (KB-locked formulas)

Per query, over each system's ranked output:
- **Recall@k** = |relevant ∩ top_k| / |relevant|, for **k ∈ {1,3,5,8}**.
- **MRR** = mean over queries of 1/rank(first relevant).
- **nDCG@8** = DCG@8 / IDCG@8, DCG = Σ (2^rel−1)/log₂(i+1) (binary rel ∈ {0,1} here).
- **Precision@5** (secondary — injection cleanliness; noise competes with task reasoning, research/01).
- For NEGATIVE queries: **false-positive rate** = fraction returning a row above the floor.

Reported per-system **× per-subset** (EXACT / PARAPHRASE), with **bootstrap 95% CIs** (resample queries) so the
decision rule (§7) can require the lift to clear the noise band, not just the point estimate.

## 5. Ground-truth relevance — the anti-circularity design (load-bearing)

**The trap (KB self-preference bias):** if one LLM writes the corpus *and* writes the queries *and* judges
relevance, the eval measures self-consistency, not retrieval. Three structural defenses:

1. **Provenance-anchored ground truth, not opinion.** Each corpus row carries `source_ref`. Each query is
   authored *against a known set of source artifacts* it should surface. Relevance is then **derived by
   construction**: row M is relevant to query Q iff M's `source_ref` ∈ Q's target artifact set. No LLM is asked
   "is M relevant to Q?" in the labelling path — relevance is a join on provenance, deterministic and auditable.
2. **Generator ≠ author ≠ judge.** Corpus synthesis, query authoring, and the realism/relevance spot-check are
   **three separate agent roles** (and the spot-check uses a different model than generation, or Gary). The
   model that wrote a row never gets to vote that the row is the answer.
3. **Human verification of a sample.** Gary (or a decorrelated reviewer) spot-checks ~10 query→relevant-set
   pairs. If construction-derived relevance disagrees with human judgment >20% of the sample, the ground truth
   is broken → fix construction before trusting any number.

**Lexical system under test = the REAL `board::recall`** (FTS5 BM25 + salience tiebreak), not a reimplementation
— we evaluate shipped code. Vector + hybrid are eval-only (Python/throwaway Rust bin), since they aren't built.

## 6. Threats to validity (pre-empting the adv review)

| Threat | Mitigation |
|---|---|
| **Circularity** (gen=judge → self-consistency) | §5: provenance-derived relevance; generator≠author≠judge; human sample-check |
| **Small-N** (~40 q → wide CIs) | KB says 20–50 is prototype-norm; report bootstrap CIs; decision rule must clear the CI band (§7), not the point estimate |
| **Corpus distribution invalid** (synthetic ≠ real reflection output) | synthesize FROM real artifacts; preserve phrasing; §2 realism gate |
| **Deck-stacking** (queries written to favour a system) | query author blind to system internals; EXACT/PARAPHRASE split fixed *before* running; subset analysis pre-registered |
| **Embedding ceiling** (jina-v5 weak on our domain) | startup canary (research/18 §10.1): embed a probe, assert cosine vs stored reference ≥0.6 (prior probe: 0.61 related / 0.09 unrelated); report it |
| **Evaluating a reimplementation** | lexical = the real `board::recall` code path |
| **Garbage embeddings poison ranks** | assert dim==1024 + non-zero norm before use (§10.1 "degrade on garbage") |

## 7. Pre-registered decision rule (LOCKED before results)

Let `L5`, `H5` = Recall@5 of lexical and hybrid; lift `Δ = H5 − L5`. Thresholds (proposed; **adv review locks
the final numbers**, then they are frozen):

- **GO (build Slice B vector leg)** iff **both**:
  1. lexical is *insufficient*: **L5 < 0.80** on the **overall** set (KB gate exemplar 0.8), **and**
  2. vector/hybrid *fixes it*: **Δ ≥ 0.10 absolute** on Recall@5 **and the 95% CI of Δ excludes 0**, corroborated by **MRR lift ≥ 0.05**, **driven by the PARAPHRASE subset** (where lexical structurally can't compete).
- **NO-GO (lexical suffices — close the pillar)** iff **L5 ≥ 0.80** OR the lift fails to clear the CI band / the PARAPHRASE driver. Lexical-only stays; `embedding`/`embed_*` columns remain as cheap dormant insurance (already in schema).
- **INCONCLUSIVE** (e.g. L5 borderline + CI straddles): report as such, do **not** default to GO. Default action on inconclusive = **NO-GO** (Wu Wei: don't add a dependency on a tie).

This rule means a marginal, noise-band, exact-token-only lift does **not** justify the vector leg — exactly the
research/18 §10.4 posture, now numeric.

## 8. Rust-native survey leg (build-vs-vendor — only bites on GO)

Independent of the eval (runs in parallel). Teardown of the §14 candidates against *our* needs:
- **`frankensearch`** — Tantivy BM25 + MiniLM vector + RRF, f16 SIMD. Is it a drop-in Slice B engine, or does
  it duplicate what FTS5 already gives us (and pull Tantivy as a heavy dep)? License rider? It uses MiniLM, not
  our oMLX jina-v5 — does that matter? Verdict: vendor / mine-for-design / skip.
- **`fast_vector_similarity`** — cosine/rank-correlation primitives + bootstrapped CIs. Useful for the *eval
  harness itself* (the CIs in §6) even on NO-GO? Or trivial enough to own in ~40 lines?
- **Field delta:** a *light* check — has anything material shifted since research/01 (gathered earlier)? Not a
  re-survey; just "is our design stale?"

Output: a one-paragraph build-vs-vendor recommendation that the verdict integrates **iff GO**. On NO-GO this leg
still records the constellation finding for the record (cheap, and Gary asked for thoroughness).

## 9. Deliverables

1. `research/28` (this doc) + the adv-review verdict folded in (§10, mirroring research/18's pattern).
2. A reproducible eval: corpus generator + golden set + harness (drives real `board::recall`, computes §4
   metrics with CIs). Eval data + scripts live in a **gitignored `eval/` or `/tmp`** (throwaway; oMLX exercise
   rules apply — embeddings are exercise-only, never landed); the *design + verdict* are what's committed.
3. The **GO/NO-GO verdict** (with the numbers) + build-vs-vendor note → `wiki/decisions.md`, breadcrumb →
   `active-work.md`, rollup → `log.md`.

## 10. Adversarial review — verdict and folded must-fixes

Four decorrelated reviewers (opus, one lens each: **eval-validity/circularity**, **statistical-power**,
**methodology-bias/fairness**, **Wu-Wei/worth-it**) attacked §§1–9. They **converged**, independently, on one
conclusion: *the full eval as specced will not produce a trustworthy GO/NO-GO.* It is confounded toward NO-GO
(or unfalsifiably toward GO), tests the wrong code path, uses biased ground truth, is statistically
underpowered at N≤40, and is disproportionate to deciding one optional leg on a personal tool. The adv-review
gate did its job: **it killed the apparatus before we built it.**

### The five convergent killer findings

1. **Wrong lexical system-under-test (CRITICAL — methodology lens).** The agent loop primes via
   `recall_primed`, *not* `recall`. `recall_primed` carries a **token-overlap floor** (`memory.rs:153–157`:
   requires `min(2, n_query_tokens)` distinct ≥3-char shared tokens) that returns **EMPTY on a true PARAPHRASE
   query by construction** — no shared tokens → no rows. §5's "lexical SUT = real `board::recall`" therefore
   evaluates code the loop never calls on the one subset that matters. **The floor, not BM25, is what fails
   paraphrases — and the floor is one line.**
2. **Provenance-derived relevance is biased toward NO-GO (CRITICAL — eval-validity + Wu-Wei).** §5's "row M
   relevant to Q iff M.source_ref ∈ Q's target artifacts" *penalizes the vector leg for retrieving a
   topically-correct row from a different source artifact* — i.e. it defines away exactly the capability the
   vector leg exists to provide. Ground truth that is structurally hostile to the system under test cannot
   adjudicate it.
3. **Underpowered toward NO-GO (CRITICAL — stats lens).** "CI of Δ excludes 0" on ~15 PARAPHRASE queries has
   **~13% power at the Δ=0.10 bar**; the true effect would need Δ≈0.30 (3× the threshold) to fire reliably.
   Recall@5 on 1–4 relevant rows is a coarse {0, 0.25, 0.5, …} ladder that bootstraps into a jagged CI. The
   rule is **nearly unachievable at this N regardless of truth** — it reproduces the §10.4 prior by
   construction, not by measurement.
4. **EXACT/PARAPHRASE split is a definitional tautology (CRITICAL — methodology lens).** Partitioning queries
   *by their lexical relationship to the answer* makes the per-subset result predetermined; the "finding"
   reduces to the author-chosen 25:15 ratio. A blind tag (sample real ticket titles, classify *after*
   authoring) is the only non-circular version — and even then see finding 3.
5. **Disproportionate (CRITICAL — Wu-Wei).** A multi-agent corpus-synthesis + golden-set + harness +
   bootstrap-CI apparatus, to decide *one optional leg* whose columns already sit dormant in the schema as
   cheap insurance, on a single-user tool. The eval costs more than the thing it gates. **Impact ÷ Effort says
   replace it.**

### Folded resolution — DESCOPE (replaces §§2–7 as the execution plan)

The reviewers' convergent recommendation, adopted. The full corpus/golden-set/CI apparatus is **cut**.
Replaced by three cheap things that answer the *actual* question — "is lexical recall good enough that the
vector leg isn't worth a dependency?" — without the circular machinery:

- **A. Floorless-`recall` decomposition experiment (the decisive, near-free test).** Take ~10–12 *real*
  paraphrase-shaped queries against a small real/seeded corpus and run **three arms**: (1) shipped
  `recall_primed` (with floor), (2) **`recall` with the token-overlap floor removed** (floorless BM25), (3)
  oMLX jina-v5 cosine. This separates two questions the original eval fused: *does the floor cause the
  paraphrase miss?* vs *does dense retrieval beat BM25?* **Decision tree:**
  - If floorless `recall` recovers the paraphrase rows → the fix is **delete/relax the floor (one line)**.
    NO-GO on the vector leg; the entire embedding dependency is avoided for free.
  - Only if floorless BM25 *still* misses rows that jina-v5 catches → the vector leg has a real, isolated job,
    and *then* (and only then) a larger eval is warranted.
- **B. Hand probe, eyeballed (~30 min).** The same ~10 queries against the live FTS5 store, results read by a
  human (Gary or a decorrelated reader). No bootstrap CI theater — at this N the eyeball *is* the instrument,
  and it's honest about being one.
- **C. Keep the L4 Rust-native survey (§8) in full** — `frankensearch` / `fast_vector_similarity` teardown +
  field-delta. This is what Gary explicitly asked for ("interesting repos/tools from Dicklesworthstone"), it's
  independent of the GO/NO-GO, and it records the constellation finding regardless of outcome.
- **Output:** a one-paragraph verdict → `wiki/decisions.md` (+ the floor decision, which is actionable either
  way), breadcrumb → `active-work.md`, the survey teardown → its own short note. **No corpus generator, no
  golden-set agents, no CI harness gets built or landed.**

### What changes in this doc

- §7's pre-registered numeric rule is **retired** — it was underpowered and circular (findings 3, 4). The
  replacement decision is the **A decision-tree** above (qualitative-but-decisive: does floorless BM25 recover
  the rows a human says are relevant?), not a CI threshold.
- §§2–6 (corpus synthesis, 40-query golden set, provenance ground truth, bootstrap metrics) are **not
  executed**. Retained above as a record of the design that was reviewed and cut, and as the spec to revive
  *only if* finding-A's narrow gate (floorless BM25 still loses to dense) actually fires.
- §5's "lexical SUT" is corrected: the loop's real path is **`recall_primed` + its floor**; the floor is the
  prime suspect and the cheapest possible fix.

### Subagent coordination (revised for the descoped plan)

The fan-out/barrier four-leg plan in the prior draft is overkill for three cheap things. Revised:
- **L4 survey** (one Explore/general-purpose agent, background) — runs independently, §8 scope.
- **A + B** (coordinator = me, foreground) — small real query list + a throwaway harness with the three arms
  (oMLX exercise rules: `/tmp`, embeddings never landed), eyeballed. No separate corpus/golden-set agents —
  the apparatus they existed to build is cut.
- **Synthesis** (me) — floor decision + GO/NO-GO paragraph + survey rollup → wiki.

## 11. Status

Adv-review **COMPLETE → descoped** (this §10). Original open questions (provenance soundness, threshold
correctness, corpus pre-bias, N-power) were all answered by the review: each is a real defect, and together
they justified the descope rather than a patch. Executed the descoped A/B/C plan → §12.

## 12. Execution results & verdict (descoped plan A/B/C)

**Method (exercise — throwaway, nothing landed).** A 14-row corpus distilled from real artifacts
(decisions/log/research/memory.rs phrasing) + 14 ticket-shaped **paraphrase** queries with hand-marked
ground truth. Three arms, two of them *shipped code*: (1) `recall_primed` (token-overlap floor),
(2) `recall` (floorless BM25 — the shipped lexical path with no floor), (3) jina-v5 cosine via oMLX. The
Rust probe (`board/tests/floor_probe.rs`, deleted after run) drove arms 1–2; `/tmp/dense_probe.py` drove
arm 3. This decomposition separated the two questions the original eval fused: *does the floor cause the
paraphrase miss?* vs *does dense beat BM25?*

**Results (recall@5 over 14 queries):**

| Arm | recall@5 | what it misses |
|---|---|---|
| `recall_primed` (floored, shipped) | **12 / 14** | the floor-dropped case + the zero-overlap case |
| `recall` (floorless, shipped) | **13 / 14** | only the one zero-token-overlap paraphrase |
| jina-v5 dense (oMLX) | **14 / 14** | nothing; true paraphrases rank-1, cos 0.45–0.63 (0.82 exact) |

- **Floor-dropped = 1** ("rules for what to put in a git commit" → relevant row *commit discipline*).
  Floorless `recall` ranks it #1; the `need≥2` floor drops it because the query shares only the single
  *content* token "commit". **The floor over-filters single-strong-content-token paraphrases** — a free
  recovery (see follow-up below), independent of the vector question.
- **Zero-overlap = 1** ("escaping its isolated work area" ↔ *worktree confinement*). Shares **no** ≥3-char
  token with the answer, so *both* lexical arms miss it by construction; **only dense recovers it** (rank-1,
  cos 0.509). This is the vector leg's genuine, isolated job.

**The §10-A gate fired — narrowly.** Floorless BM25 still missed a row dense caught, so dense has a real job.
But the magnitude is **1/14 (~7%) on a set deliberately seeded with hard paraphrases**, and it appears *only*
when a query shares zero content tokens with its answer — the minority of the real distribution (ticket
titles reuse domain vocabulary; 13/14 here shared enough). Against that: the priming use is an **ambient
hint** (a miss = no hint, not a wrong answer — degrades gracefully), and the cost is a real dependency
(oMLX-availability coupling + cosine scan + RRF fusion + canary + dedup) on a personal tool.

**Survey leg (research/29, L4).** `frankensearch` = mine-for-design (Tantivy duplicates FTS5 as a heavy dep;
in-process ONNX is the wrong shape for our HTTP oMLX; license rider is a legal ambiguity). `fast_vector_similarity`
= **skip** (rank-correlation measures, not cosine; no license file). `cass_memory_system` / `coding_agent_session_search`
= mine-for-design only (patterns already folded into research/18 §10.3). **Field delta: design not stale** —
sqlite-vec brute-force, LanceDB 1.0, ort/candle all *validate* FTS5 + hand-rolled cosine + oMLX without changing
it. **Nothing to vendor.** So even on a GO, Slice B would be **built, not adopted**.

### VERDICT

- **Slice B vector leg: NO-GO (for now).** Floorless lexical handles 13/14; dense's marginal 1/14 on
  zero-overlap paraphrases does not clear the dependencies-are-liabilities bar for an ambient-hint use on a
  personal tool. The `embedding`/`embed_*` columns stay **dormant insurance** (zero carrying cost), and this
  run *warmed* that insurance: jina-v5 behaves well (clean rank-1, healthy cosine separation), so if the real
  query distribution proves more paraphrase-heavy than ticket titles suggest, Slice B is a fast, **build-not-vendor**
  follow — revive §§2–7 only then.
- **Free win (follow-up, not done here): relax the `recall_primed` floor.** It demonstrably costs 1/14 recall
  by treating a strong content token ("commit") the same as a boilerplate one. The principled fix is a
  *BM25-score-aware* floor (the §10.2-deferred, golden-calibrated cutoff), **not** a blind `need=1` (which
  would reintroduce the cold-store single-boilerplate-token noise the floor was added to stop). Filed as its
  own ticket with this data attached — deliberately *not* slammed in here, because the **noise side** of the
  tradeoff (how often the floor correctly suppresses) was descoped and is unmeasured; changing gated code on
  half the evidence would violate gate-by-a-number.

**Both legs land as a win:** the pillar closes honestly (lexical suffices; no dependency added; insurance warm)
*and* the eval surfaced a concrete, data-backed lexical improvement that the original full-apparatus design
would have buried under its own machinery.
