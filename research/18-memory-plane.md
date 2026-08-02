# 18 — Memory Plane (buildable design)

> Pillar P1 of design-v2 §6, the memory plane. Substrate (telemetry trajectory + `run` index) is
> already built (research/17, Units A+B). This doc is the buildable design for the memory plane proper:
> **sidecar capture → hybrid retrieval → decay/promotion → reflection**. Folds in research/17 §5-6,
> design-v2 §6, the RAG KB grounding, and a live oMLX embeddings probe. It is written to be *attacked*
> by the adversarial review (the next gate) — claims are testable, decisions are binary, no hand-waving.

## 0. Grounding (recorded per the KB gate + live probes)

**KB (RAG domain — "Production RAG Guide"):**
- `KB: searched "hybrid retrieval dense BM25 fusion" → applied hybrid vector+lexical is the production default; fuse with Reciprocal Rank Fusion RRF_score(d)=Σ 1/(k+rank_i(d)), k=60 standard (production-rag-guide)`
- `KB: searched "chunking strategy chunk size tokens" → applied 200–500 token sweet spot; our "chunks" are whole memory rows (title+body), already in this range — no sub-chunking needed (production-rag-guide)`
- `KB: searched "retrieval evaluation recall precision golden set" → applied Recall@k/MRR on a golden query set (100–500 annotated); evaluate the embedding model on OUR data, not MTEB (production-rag-guide)`
- KB also: metadata **pre-filtering** (not post) for project/scope isolation; cross-encoder reranking only when precision is a *measured* bottleneck (deferred — we have no measurement yet); contextual retrieval (~49% better BM25) is a slice-3+ enrichment experiment, not slice 1.

**Code (what we extend):**
- `crates/board/src/schema.rs` — DDL + `user_version` migrator (v1 already adds `run`). The memory table lands as **migration v2** (additive, the ratchet already exists). WAL + busy_timeout already set for parallel writers.
- `crates/board/src/board.rs` — ops house style: explicit `params![]`, `?N` placeholders, every mutation appends an audit `event`, `set_status` is the single chokepoint. Memory ops mirror this (insert/query/bump + an event kind).
- `crates/agent/src/recorder.rs` — the trajectory: redaction at the write boundary, lenient reader, best-effort (never blocks the loop). Reflection (slice 3) *reads* trajectories via `read_trajectory`.

**oMLX embeddings (live probe, 2026-06-10):**
- Endpoint `/v1/embeddings` works. Model `jina-embeddings-v5-text-small-retrieval-mlx`, **dim 1024**.
- Semantic sanity: two related harness sentences cosine **0.61**; unrelated **0.09** — strong separation, fit for purpose.
- Gotcha (environment, not code): the model shipped `architectures: null` (custom remote-code `model.py`), so oMLX first misclassified it as an LLM. Gary corrected the model-type classification to unblock. **Implication for the design: embedding availability is an environment dependency that can silently regress** → the vector leg must degrade to lexical-only, never hard-fail (see §3, §6 adversarial-anticipation).

## 1. The CoALA lens (what decays, what doesn't)

| CoALA memory | Our store | Decays? | Owner |
|---|---|---|---|
| Working | active workpad (ticket Plan/AC/Validation/Notes/Confusions) | n/a (per-ticket) | board (built) |
| Episodic | **sidecar** (`memory` table) — what happened, run-by-run | **yes** (retention curve) | this doc |
| Semantic + Procedural | **wiki** (`wiki/*.md`) — distilled, durable knowledge | **never** | reflection promotes into it |

The pipeline (design-v2 §11) is **raw → distilled → promoted**, built once and reused thrice (memory, telemetry, self-evolution case-law): raw trajectory (telemetry, built) → distilled sidecar rows (this doc, capture) → promoted wiki lessons (this doc, reflection). The reflection engine here is the *same engine* P5 self-evolution reuses for case-law — which is why P5 is genuinely last.

## 2. Sidecar schema (migration v2)

```sql
CREATE TABLE IF NOT EXISTS memory (
    id            TEXT PRIMARY KEY,          -- sortable mint (ts-prefixed, like run_id)
    ts            TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    type          TEXT NOT NULL CHECK (type IN
                    ('decision','bugfix','feature','refactor','discovery','lesson','constraint')),
    title         TEXT NOT NULL,             -- the INDEX line; what retrieval shows (≤120 chars)
    body          TEXT,                      -- drill-down only; fetched on selection (progressive disclosure)
    salience      REAL NOT NULL DEFAULT 0.5 CHECK (salience >= 0 AND salience <= 1),
    scope         TEXT NOT NULL CHECK (scope IN ('ticket','project','global')),
    entities      TEXT,                      -- JSON array: symbols/files/concepts (FTS + filter)
    files         TEXT,                      -- JSON array: paths touched
    project       TEXT,                      -- pre-filter key (metadata pre-filtering)
    ticket_id     TEXT,                      -- provenance; ticket-scope pre-filter key
    embedding     BLOB,                      -- 1024 × f32 LE = 4096 bytes; NULL until embedded (degrade path)
    usage_count   INTEGER NOT NULL DEFAULT 0,
    last_used_ts  TEXT,
    created_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    -- links[] deferred (research/17 §5): no consumer yet → Wu Wei, add when a consumer exists
);

-- Lexical leg. FTS5 external-content over the title/body/entities of `memory`.
CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
    title, body, entities, content='memory', content_rowid='rowid'
);
-- triggers keep memory_fts in sync (insert/delete/update) — standard FTS5 external-content pattern.
```

**Five load-bearing fields** (research/17 §5, why these and not more): `type` (reflection clusters by it), `salience` (decay + ranking), `scope` (isolation pre-filter), `entities` (lexical + filter), `embedding` (vector leg). Everything else is provenance or housekeeping. We do **not** add `links` until a consumer exists.

**Why `embedding` is nullable:** the vector leg is slice 2 and the oMLX dependency can regress (§0 gotcha). A NULL embedding means "lexical-only for this row" — retrieval still works, just without the vector contribution for that row. No row is ever unretrievable for lack of an embedding.

## 3. Retrieval (hybrid, top-k, progressive disclosure)

The injection-cost lesson (claude-mem): local embeddings make *capture/compression* free but do **nothing** for *injection* cost — the only fix is discipline. So retrieval returns an **index**, not bodies.

```
recall(query, project, scope_filter, k=8) -> [ {id, title, type, score} ]   # NO bodies
recall_body(id) -> body                                                      # fetch on selection; bumps usage
```

Algorithm:
1. **Pre-filter** (metadata, KB grounding): restrict candidate set by `project` and an allowed `scope` set *before* ranking. Ticket-scope rows from other tickets are excluded here, not after. (Pre-filter, never post-filter.)
2. **Two legs over the filtered set:**
   - **Lexical** — FTS5 BM25 over `memory_fts` for `query`.
   - **Vector** — cosine of `embed(query)` against each candidate's `embedding` (rows with NULL embedding score 0 on this leg only).
3. **Fuse with RRF** (k=60): `relevance(d) = 1/(60+rank_lex(d)) + 1/(60+rank_vec(d))`. RRF needs only ranks, so the two legs' incomparable score scales never have to be normalized against each other — the documented reason it's the production default.
4. **Re-rank with the memory-specific signals** (research/17 §6), each min-max normalized over the candidate set:
   `final(d) = 1.0·relevancê(d) + 0.5·recencŷ(d) + 0.8·saliencê(d)`
   recency from `ts`/`last_used_ts`; salience from the column.
5. Return top-k **index lines**. On `recall_body(id)`, return the body and **bump** `usage_count += 1`, `last_used_ts = now` (feeds both decay and the recency/usage signal — used memories resist eviction).

**Degrade path (no embeddings):** if `embed(query)` fails (oMLX down / model regressed), skip leg 2 and fuse leg 1 alone (RRF of one list = its own order). Retrieval is **lexical-only but never broken**. This is loud in logs, silent to the loop. (The adversarial review's "silent retrieval failure" lens lives here.)

**Loop integration:** the loop injects the top-k index block into the system context for an in-progress ticket (small, bounded). The agent pulls a body with `recall_body` only when a title looks relevant. This caps per-turn injection at k titles.

## 4. Capture (how rows are born)

Two sources, both producing the same row shape:
- **Explicit** — a `remember` agent tool: the agent, mid-work, records a decision/discovery/constraint it judges worth keeping. Agent rates `salience`, picks `type`/`scope`, lists `entities`/`files`. (Agency + cheapest signal.)
- **Derived** — reflection (slice 3) over a landed run's trajectory + workpad extracts rows the agent didn't explicitly save.

At capture: embed `title` (slice 2; jina-v5 query/passage symmetric enough for our scale), **dedup** against same-scope rows at cosine **≥ 0.92** — on a hit, bump the existing row's `usage_count` instead of inserting a near-duplicate (prevents the store bloating with restatements). Mirror board.rs: append a `memory_captured` audit event.

## 5. Decay & promotion

**Retention** (research/17 §6): `retention(d) = salience·e^(−λ·Δt) + Σ_uses 1/days_since_use`.
- First term: salient memories fade slowly, trivia fast. Second term: every use injects a recency-weighted bump, so *used* memories survive regardless of age (spaced-repetition shape).
- **Evict** sidecar rows with `retention < 0.15`. Wiki never decays.
- Eviction runs as a maintenance pass (a `memory gc` command + opportunistically post-land), not inline — keeps the hot path clean.

**Promotion candidates:** rows with high `usage_count` *and* high `salience` are surfaced to reflection as promotion candidates (episodic fact that keeps proving useful → belongs in semantic wiki).

## 6. Reflection (episodic → semantic; the shared engine)

Trigger: **on land/abandon** (primary) or salience accumulation (secondary). Steps:
1. **Cluster** recent sidecar rows (by `type` + embedding proximity) for the landed ticket.
2. **Extract** (local LLM, oMLX 35B): from a cluster, write one *generalized / conditional / imperative* lesson ("when X, do Y because Z") — not a restatement of one event.
3. **Classify vs wiki** — `ADD | UPDATE | DELETE | NOOP` against existing wiki content (the amendment shape P5 reuses).
4. **Gate on landed** (Voyager self-verification): only promote lessons whose source work actually **landed** (git commit = human approval, the §10 keystone). A lesson from abandoned work is not knowledge.
5. Carries `proof_count` (how many landed runs corroborate) — UPDATE increments it; high proof_count resists DELETE.

Human stays in the loop for wiki writes initially (reflection *proposes* the ADD/UPDATE diff; a human lands it), matching the self-evolution gate philosophy. Full auto-promotion is a later, separately-gated decision (P5 territory).

## 7. Slices (build order — each independently testable)

- **Slice A — store + capture + lexical retrieval.** Migration v2 (`memory` + `memory_fts` + triggers), memory ops (`insert`/`recall`/`recall_body`/`bump`/`gc`) in the board crate mirroring board.rs, the `remember` tool, RRF over the single lexical leg, progressive-disclosure index. **No embedding dependency** → fully unit-testable with deterministic fixtures. Ships a working memory plane today.
- **Slice B — vector leg + hybrid fusion.** `embedding` populated via an `Embedder` seam (oMLX impl + a deterministic test fake — the second impl that justifies the trait), cosine leg, RRF fusion of both legs, dedup at 0.92, the degrade path. Fusion math unit-tested with injected vectors (no live oMLX in unit tests; oMLX is exercise-tested).
- **Slice C — decay + promotion + reflection.** retention/eviction (`memory gc`), reflection (cluster→extract→classify→land-gate→propose wiki diff), promotion candidates. Reflection extraction is oMLX (exercise-only, never auto-landed).

Subagent coordination patterns for execution are the **implementation-plan** artifact (task #8), drafted after the adversarial review folds its must-fixes into this doc.

## 8. What this design deliberately does NOT do (Wu Wei + anticipating the review)

- **No reranker** (cross-encoder) — KB says add only when precision is a *measured* bottleneck; we have no measurement. Slice C can add a retrieval eval harness (golden query set) first; a reranker is justified only by its numbers.
- **No `links[]` graph** — no consumer yet.
- **No sub-chunking** — a memory row (title+body) is already in the 200–500 token band.
- **No separate vector DB / ANN index** — at personal scale (thousands of rows, not millions) a linear cosine scan over the pre-filtered candidate set is fast and dependency-free; an ANN index is premature (dependencies are liabilities). Revisit only if a measured latency problem appears.
- **No auto-write to the wiki** — reflection proposes; a human lands. A silently self-rewriting memory is the catastrophic-silent-failure mode the process gate exists to prevent.

## 9. Open questions for the adversarial review to pressure-test

1. **Retrieval quality:** is RRF(lexical, vector) + the 1.0/0.5/0.8 re-rank actually better than either leg alone on harness-shaped queries? (Needs the golden-set eval — is that eval slice-A or slice-C?)
2. **Schema durability:** are the 5 load-bearing fields enough for reflection's clustering, or does clustering need a field we haven't added (and can't add without a migration)? Get the schema right before data accrues.
3. **Reflection validity:** can the land-gate + proof_count actually stop a plausible-but-wrong lesson from being promoted? What stops a confidently-wrong oMLX extraction?
4. **Wu Wei:** is the whole vector leg (slice B) worth it, or does lexical-only (slice A) already serve a personal-scale harness — i.e. should slice B be deferred until slice A is *measured* insufficient?

---

## 10. Adversarial review — verdict and folded must-fixes (2026-06-10)

Four decorrelated cold reviewers, one lens each (retrieval-quality / schema-durability / reflection-validity / Wu-Wei), each instructed to refute. They **converged**: the three deep-flaw lenses shredded exactly the parts the Wu-Wei lens said to defer. Verdict: **get the schema fully right now (cheap), ship slice A lexical-only, gate the vector leg + reflection behind a golden-set measurement.** Dispositions below; §2/§3/§6/§7 are superseded by this section where they conflict.

### 10.1 Retrieval (folded)
- **[CRITICAL → FIXED] min-max of fused RRF lets salience overrule relevance.** Worked example: an exact match at rank-1 (relevancê=1.0, mid salience) scores ~1.0 while a marginal rank-6 row with salience 0.95 used yesterday scores 0.3 + 0.5 + 0.8 = 1.6 and **wins**. The salience+recency mass (1.3) exceeds the whole relevance range (1.0). **Fix:** drop value min-max entirely. **All signals are rank-fused**: `score(d) = w_rel/(60+rank_rel) + w_rec/(60+rank_rec) + w_sal/(60+rank_sal)` — everything on the same 1/(k+rank) scale, outlier-robust, no degenerate div-by-zero, and relevance stays dominant because its rank spread is the widest. (Subsumes the ADVISORY div-by-zero and outlier-collapse findings.)
- **[CRITICAL → FIXED in slice B] degrade path catches failure, not garbage.** A 200-response with zero/wrong-dim/wrong-model vectors silently poisons ranks (worse than lexical-only). **Fix:** before using a query embedding, assert `dim==expected` + non-zero norm + a **startup/periodic canary** (embed a fixed probe, assert cosine vs a stored reference ≥ ~0.6 per the §0 probe); on canary failure force lexical-only. "Degrade on garbage," not just "degrade on exception."
- **[WARNING → FIXED by deferral] unmeasured weights / golden set.** Slice A ships **no weighted blend** — BM25 order with salience as a pure tiebreak. The weighted rank-fusion above is slice B, gated on the golden-set eval existing first.
- **[WARNING → FIXED] cold-start noise.** Add a relevance floor (drop candidates below a min BM25 score; return <k or empty) — empty beats noise injected into the loop.

### 10.2 Schema (folded — all into v2 now, before rows accrue)
The durable v2 DDL (supersedes §2):
```sql
CREATE TABLE IF NOT EXISTS memory (
    id            TEXT PRIMARY KEY,          -- ts(ms)+monotonic-counter mint (no same-tick collision)
    ts            TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    type          TEXT NOT NULL,             -- validated in Rust (board.rs chokepoint), NOT a SQL CHECK
                                             --   (P5 case-law will add types; CHECK alter = table rebuild)
    title         TEXT NOT NULL,
    body          TEXT,
    salience      REAL NOT NULL DEFAULT 0.5 CHECK (salience >= 0 AND salience <= 1),
    scope         TEXT NOT NULL CHECK (scope IN ('ticket','project','global')),  -- stable enum, CHECK ok
    entities      TEXT, files TEXT, project TEXT, ticket_id TEXT,
    embedding     BLOB,                      -- nullable; populated in slice B
    embed_model   TEXT,                      -- which model produced `embedding` (NULL ⇔ embedding NULL)
    embed_dim     INTEGER,                   -- vector length; cosine SKIPS rows whose embed_model≠current
    usage_count   INTEGER NOT NULL DEFAULT 0,    -- bumped ONLY by recall_body (real retrieval use)
    last_used_ts  TEXT,
    proof_count   INTEGER NOT NULL DEFAULT 0,    -- INDEPENDENT landed corroborations (partition by model)
    promotion_state TEXT NOT NULL DEFAULT 'none'
                    CHECK (promotion_state IN ('none','candidate','promoted','rejected')),
    reflected_at  TEXT,                      -- last reflection pass that processed this row (idempotency)
    evicted_at    TEXT,                      -- SOFT delete; filtered from recall, preserves provenance
    created_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```
- **[CRITICAL → FIXED] `proof_count`, `promotion_state`+`reflected_at`, `embed_model`+`embed_dim`** all present at v2 birth (reflection idempotency + model-swap corruption guard). Cosine skips rows whose `embed_model` ≠ the current query model → a model swap degrades to lexical for stale rows, never silently mixes vector spaces.
- **[WARNING → FIXED] `type` CHECK dropped**, validated in Rust at the ops chokepoint (board.rs style) so reflection/P5 can add types without a rebuild. `scope` keeps its CHECK (bounded, stable).
- **[WARNING → FIXED] FTS5 triggers written explicitly** (the `'delete'`-command external-content form) in the v2 migration; usage/embedding/proof updates must not touch FTS-indexed columns (title/body/entities). Unit test: insert → update title → old title returns nothing, new returns the row.
- **[WARNING → FIXED] dedup merge policy:** on a ≥0.92 hit, union `entities`/`files`, take the higher `salience`, and emit a `memory_deduped` event — never silently drop a better-worded restatement. Dedup does **not** bump `usage_count` (that signal is retrieval-use only).
- **[ADVISORY → FIXED] id mint** = ts(ms) + per-process monotonic counter; **eviction is soft** (`evicted_at`), preserving reflection provenance and making an over-aggressive policy reversible.

### 10.3 Reflection (folded — design corrected now, BUILT later in slice C)
Root cause the reviewer named: *the pipeline rewards corroboration and punishes refutation — backwards for a store whose dominant cost is silent false-promotion.* Corrections (supersede §6):
- **[CRITICAL → FIXED] land-gate gates the diff, not the lesson.** Stop calling landing "self-verification." The load-bearing gate is a **human approving the lesson TEXT**, made a **structural invariant**: reflection's only output is a *proposed diff artifact*; it has **no code path that writes `wiki/*.md`**. Full-auto then requires writing new code a reviewer must consciously approve — not flipping a flag. Extraction is constrained to gate-*observed* facts (diff/files/AC outcomes), forbidden from promoting narrated causal "why" alone.
- **[CRITICAL → FIXED] proof_count entrenchment.** Count only **independent** corroboration — partition by `provider`/`model` (already captured in `run`); N corroborations from one model = 1 signal. Refuting evidence outweighs corroborating repetition.
- **[CRITICAL → FIXED] DELETE inverted.** Evidence-based DELETE/UPDATE is the **easy** path: one credible refutation (a contradicting landed artifact) **overrides** any proof_count. proof_count resists only *unsourced* deletion/churn, never evidence-based correction.
- **[WARNING → FIXED] generalization-by-number:** min distinct **tickets** (not rows) per cluster, a cluster-cohesion threshold (reject coincidental embedding-neighbors), and a confidence tier (n=2 → "tentative", promotable only after independent proof). No vibes.
- **[WARNING → FIXED] cluster contamination:** filter non-landed rows at **cluster construction** (join `run` terminal state on `run_id`), not just at promotion output. Abandoned rows stay in episodic recall ("we tried X, it failed") but never seed a promotion cluster.

### 10.4 Wu Wei — the build verdict (decisive)
- **[CRITICAL → ACCEPTED] defer the vector leg (slice B)** until a golden-set eval shows lexical Recall@k is actually insufficient. Harness queries are exact-token (symbols, paths, error strings) — FTS5's strength. Keep the `embedding`/`embed_*` columns (cheap insurance); build no embedder/cosine/fusion/dedup yet. **No `Embedder` trait** (fails "no ABC until two real impls"; a test fake doesn't count) — call oMLX directly if/when built.
- **[CRITICAL → ACCEPTED] defer the full retention curve.** Slice-A/C eviction starts as soft-delete `WHERE usage_count=0 AND created_at < now−N`; the `salience·e^(−λt)+Σ1/days` curve only if the simple rule is measured insufficient. No `memory gc` command yet.
- **[CRITICAL → ACCEPTED] defer reflection (slice C).** The human curates the wiki today; manual promotion from the sidecar index is the status quo and nothing breaks without automation. Build reflection (with §10.3 gates) only after slice A proves the sidecar earns its keep.
- **[ADVISORY → ACCEPTED] no RRF wrapper in slice A** (RRF of one list = its order); rank by BM25 then salience tiebreak directly.

### 10.5 Revised slice plan (authoritative)
- **Slice A — lexical sidecar (BUILD NOW).** Migration v2 = the full durable DDL above + `memory_fts` external-content + the 3 explicit triggers. Memory ops (`insert`/`recall`/`recall_body`/`bump`/dedup-merge) in the board crate, mirroring board.rs (`params!`, audit events, validation at the chokepoint). The `remember` agent tool. Retrieval = FTS5 BM25, project/scope **pre-filter**, relevance floor, progressive-disclosure index (id,title,type), BM25 order + salience tiebreak. Manual promotion. **No embeddings, no fusion, no reflection.** Fully unit-testable with deterministic fixtures (no live oMLX in unit tests).
- **Slice B — vector leg (GATED: only after a golden-set eval shows lexical insufficient).** Golden set (≥30 harness queries w/ annotated relevant rows) **first** — it's the gate. Then direct-oMLX embeddings, `embed_model`/`embed_dim`, canary validation, rank-fusion (§10.1), dedup-at-0.92 with merge. Fusion math unit-tested with injected vectors; oMLX is exercise-tested.
- **Slice C — decay + reflection (GATED: only after slice A proves the sidecar earns its keep).** Simple soft-delete eviction first. Reflection with every §10.3 gate, human-lesson-approval as a structural invariant. This is the engine P5 self-evolution reuses — it must pass its own pre-land review there too.

**Net:** the review cut the pillar from three speculative slices to **one shippable slice + two measurement-gated slices**, while making the v2 schema durable enough that the deferred slices need no table rebuild. Re-review not required (zero CRITICALs remain open; all are FIXED or ACCEPTED-as-deferral).
