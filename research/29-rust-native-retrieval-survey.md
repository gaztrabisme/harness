# 29 — Rust-Native Retrieval Engine Survey

> L4 survey leg of research/28 §8 and §10's descoped C-track. Build-vs-vendor teardown of the
> Dicklesworthstone constellation candidates (research/14 P3) against our actual memory plane design
> (research/18). Read-only research — no files modified, no deps added. This records the constellation
> finding regardless of the Slice B GO/NO-GO outcome (which the A/B legs of research/28 decide
> separately via the floorless-recall decomposition experiment).

---

## 1. frankensearch teardown

### What it is

`frankensearch` (★57, Rust, MIT + OpenAI/Anthropic Rider, v1.2.5) is an 11-crate workspace implementing
two-tier hybrid local search: fast lexical BM25 (Tantivy) + fast embedding (potion-multilingual-128M via
fastembed) fused by RRF, then optionally refined with a quality embedding (all-MiniLM-L6-v2) plus a
FlashRank cross-encoder reranker. The progressive "Initial → Refined" delivery pattern is its key UX
innovation.

### Architecture components

| Component | Crate | What it does |
|-----------|-------|--------------|
| Lexical BM25 | `frankensearch-lexical` | Tantivy 0.26.1 — immutable segment index |
| Fast embed | `frankensearch-embed` | fastembed + model2vec + hash fallback |
| Vector store | `frankensearch-index` | FSVI on-disk mmap f16 store + SIMD dot-products + optional HNSW (hnsw_rs) |
| Fusion | `frankensearch-fusion` | RRF (k=60) + score blending (α·quality + (1−α)·fast) |
| Rerank | `frankensearch-rerank` | FlashRank cross-encoder (optional) |
| Storage | `frankensearch-storage` | metadata persistence, dedup by content-hash, embedding queue |
| Runtime | (workspace) | `asupersync` not Tokio — capability-gated, deterministic replay |

**Key external deps:** Tantivy 0.26.1, fastembed 5.11.0, ort 2.0.0-rc.12 (ONNX Runtime), hnsw_rs, memmap2,
rayon, wide (SIMD), half (f16), asupersync.

### Does it duplicate what FTS5 already gives us?

Yes, substantially. The Tantivy BM25 leg replicates what SQLite FTS5 already provides — BM25 over
tokenized text, ranked results. The operational model is completely different (Tantivy needs its own
on-disk segment index; FTS5 is a virtual table inside the same SQLite file we already have), but the
retrieval capability is equivalent for our scale. We would be replacing a dependency-free, zero-overhead
virtual table with a heavy external index just to run the same algorithm.

### Is it a drop-in Slice B engine?

No, for three independent reasons:

**1. Architectural mismatch on the embedding leg.** frankensearch is built around *in-process* embedding
(fastembed/MiniLM running inside the same Rust process via ONNX Runtime). Our design's Slice B embeds
via oMLX (jina-embeddings-v5-text-small-retrieval, dim 1024) — an out-of-process HTTP call to a model
server. These are fundamentally different integration shapes. We cannot lift frankensearch's embedding
pipeline and plug oMLX into it without rewriting the entire embed crate, at which point we're writing
Slice B ourselves. The model dimension also differs (MiniLM L6-v2: 384-dim; jina-v5: 1024-dim), so the
FSVI vector store format would need surgery.

**2. Tantivy is a heavyweight dependency.** Tantivy 0.26.1 brings its own index format, writer, reader,
segment merger, tokenizer chain, mmap management, and a substantial compile-time footprint. Adding Tantivy
means vendoring ~500 KB of compiled code for a capability (BM25) we already have via SQLite FTS5 in the
same database file we write for every other operation. The only win would be BM25 performance at scale —
irrelevant at personal-harness row counts (hundreds to low thousands). The "dependencies are liabilities"
principle rules this out sharply.

**3. License rider.** The MIT + OpenAI/Anthropic Rider explicitly restricts Anthropic employees,
contractors, and agents. Using Claude Code (an Anthropic product) to build or operate a harness that
vendors frankensearch likely makes the harness a "pipeline" used "on behalf of" Anthropic under the rider's
broad "use" definition (which includes "benchmarking, testing, analyzing, incorporating"). Even if the legal
risk is low for a personal tool, the rider creates an irreducible ambiguity. Fork/vendor-and-own (the only
viable mode per research/14) does not resolve this — the rider travels with derivative works.

### SIMD / f16 — is there value here?

frankensearch's SIMD dot-products (in `frankensearch-index`, using the `wide` crate for portable SIMD) are
well-implemented. At our scale — a few hundred 1024-dim vectors after pre-filtering — brute-force cosine
over the candidate set takes ~0.5 ms on M3 Max without any SIMD. SIMD would take it to ~0.1 ms. Neither
number is a bottleneck in a conversational agent loop where the embedding call alone costs 20–50 ms. The
f16 quantization is similarly irrelevant: at hundreds of vectors × 1024 dims × 4 bytes = ~400 KB in f32;
f16 halves it to ~200 KB, both trivially in-memory.

### Verdict by component

| Component | Verdict | Reason |
|-----------|---------|--------|
| Tantivy BM25 | **skip** | Duplicates FTS5; heavy dep for identical capability |
| FSVI vector store + SIMD | **mine-for-design** | The mmap + f16 + SIMD brute-force pattern is worth reading; ~40 lines owned is simpler |
| RRF fusion logic | **mine-for-design** | Confirms research/18 §10.1 design (k=60, rank-based, scale-agnostic) — nothing new |
| fastembed / MiniLM in-process | **skip** | oMLX is the embedding provider; in-process ONNX is the wrong integration shape |
| progressive two-phase delivery | **mine-for-design** | Interesting for future UX; not relevant to Slice B's single-shot recall |
| Overall crate | **skip — do not vendor** | License rider + Tantivy dep + architectural mismatch on embed |

---

## 2. fast_vector_similarity teardown

### What it is

`fast_vector_similarity` (★430, Rust+Python, **no LICENSE file found**) provides six rank-correlation and
dependency-measure primitives: Spearman's ρ, Kendall's τ, Approximate Distance Correlation, Jensen-Shannon
Dependency Measure, Hoeffding's D, and Normalized Mutual Information. Exposed to Python via pyo3. Rust
deps: ndarray 0.15 + rayon + statrs + rand. **No SIMD crates.** No explicit license file was found in the
repo root — this is a vendoring blocker independent of any rider.

### Does it implement cosine similarity?

No. The library measures rank correlation and statistical dependence between distributions/vectors. It does
not implement cosine similarity, dot-product, or any of the standard retrieval similarity primitives.
Calling it a "vector similarity" library is mildly misleading from a retrieval-systems perspective —
it is a *statistical dependence* library for comparing ranked lists or numeric distributions.

### Is it useful for the cosine scan in Slice B?

No. Our Slice B cosine scan needs: `dot(q, d) / (|q| · |d|)` over pre-filtered 1024-dim f32 vectors.
That is ~40 lines of owned Rust using `std::iter` or `ndarray`. The SIMD argument is the same as above:
at hundreds of vectors the operation is sub-millisecond without SIMD, and the embedding call dominates.
fast_vector_similarity provides none of these primitives.

### Is its bootstrap-CI code useful for eval harnesses?

Partially, but not as advertised. The implementation does bootstrapping (parallel resampling via rayon)
but computes IQR-trimmed mean and standard deviation over bootstrap samples rather than explicit CI bounds
(lower/upper percentile). That is a robust central-tendency estimate, not a confidence interval in the
statistical sense. For research/28's eval harness (which the adversarial review cut anyway), you would
still need to compute percentile CIs directly — a trivial 15-line addition. The library's Kendall's τ
(merge-sort O(n log n) implementation) and Jensen-Shannon measure are genuinely well-implemented, but
neither is needed for a retrieval eval.

### Vendoring blocker

No license file was found at the repo root. Vendoring code with no explicit license is not permissible —
without a license, all rights are reserved by default under copyright law. Even mining the design carries
some risk without a clear permissive grant.

### Verdict

**skip.** Does not implement cosine similarity (wrong tool for Slice B). Bootstrap stats are partial and
replicable in ~15 lines. No license file = vendoring/forking blocked. The rank-correlation measures are
interesting for retrieval-metric comparison experiments, but that is a future eval harness concern (and
the full eval apparatus was cut by research/28 §10).

---

## 3. cass_memory_system

`cass_memory_system` (★376, TypeScript/Bun) implements a three-layer cognitive architecture (episodic →
working → procedural) for coding agents, with a deterministic Curator (explicitly no LLM to prevent
feedback loops), confidence decay with 90-day half-life, and an Evidence Gate that validates rules
against session history before acceptance. The design ideas worth noting: the deterministic curator
(anti-feedback-loop principle), the anti-pattern inversion (harmful rules become warnings, not deletes),
the evidence gate requiring history corroboration before promotion, and the `graceful degradation` mode
(works without LLM). These map onto research/18 §10.3 concerns (land-gate, proof_count entrenchment,
DELETE inverted) and are largely already folded into our design. The storage backend (YAML playbook + JSON
state + `cass` CLI for episodic) is TypeScript-native and not portable. **Mine-for-design only** — the
deterministic curator pattern and anti-pattern-as-warning idea are the most novel; everything else is
covered by research/01 and research/18.

---

## 4. coding_agent_session_search

`coding_agent_session_search` (★869, Rust) is the `cass` CLI underlying cass_memory_system. It ingests
session history from 20+ agent providers (JSONL, SQLite, Markdown, JSON) into a unified
`Conversation → Message → Snippet` schema in SQLite, then indexes with Tantivy BM25 + edge n-grams +
optional FastEmbed ONNX semantic vectors + RRF (k=60). Notably: atomic index swaps via `renameat2
RENAME_EXCHANGE` (readers always see a complete index generation), quarantine-over-deletion for corrupt
assets, and agent-first JSON contract with golden-file pinning. The architecturally distinctive piece
is the Universal Connector pattern (11 formats → 1 schema) and the atomic swap — both episodic-capture
concerns for a harness that would ingest Claude Code session artifacts. Our harness currently captures
via the trajectory recorder (research/17), not by ingesting provider-native session formats, so the
connector matrix is not needed. The Tantivy + FastEmbed stack is the same pattern as frankensearch
(same vendor/mine verdicts apply). The atomic swap via `renameat2` is a clean Linux-specific pattern for
FTS index rotation that is worth noting if we ever need to rebuild the `memory_fts` index offline —
not relevant to SQLite FTS5 which has its own transactional index management. **Mine-for-design only**
(Universal Connector schema and atomic-swap patterns); no vendoring warranted.

---

## 5. Field delta — is our Slice A+B design stale?

A light pass over material shifts in Rust-native local retrieval/vector-search since early 2026.

### SQLite vector extensions

**sqlite-vec (Alex Garcia, v0.1.x):** Still pre-1.0. Brute-force exact search is stable and has Rust
bindings; ANN (DiskANN/IVF) remains experimental/alpha with active bug fixes. Permissive license
(Apache/MIT). **No change to our design:** research/18 §8 already decided against an ANN index ("at
personal scale a linear cosine scan over the pre-filtered candidate set is fast and dependency-free")
and sqlite-vec's brute-force path simply replicates what we can own in ~40 lines without an extension
dependency. sqlite-vss is effectively superseded by sqlite-vec and not worth re-examining.

**SQLite vec1 (official SQLite team):** Early-stage, feature-incomplete, IVFADC/OPQ implementation.
Interesting long-term (official provenance), not production-ready today. Not relevant to our decision.

**SQLite-Vector (SQLite AI/Cloud):** Elastic License 2.0 — requires commercial license for production.
Ruled out.

**Verdict:** SQLite FTS5 (our Slice A substrate) and a hand-rolled cosine scan (Slice B) remain the
right calls. sqlite-vec is the only viable extension alternative and adds a dependency for a capability
we can own trivially. No change needed.

### Tantivy

v0.26.0 released 2026-03-31. Actively maintained, production-grade. **Irrelevant to our design** —
we chose FTS5 over Tantivy specifically to avoid this dep (same capability, zero additional overhead).
That decision holds.

### Candle / ort embedding runtimes

Both are production-grade as of mid-2026. ort (ONNX Runtime wrapper) is the dominant embedding-runtime
choice in production Rust (used by fastembed, TEI, etc.); Candle is the pure-Rust HuggingFace path.
**Irrelevant to our design:** we deliberately chose oMLX (out-of-process HTTP) as the embedding provider,
not in-process inference. That was a deliberate architectural choice (no model binaries in the harness
process, leverage the Mac GPU, reuse an already-running server). Nothing in the 2026 candle/ort landscape
changes that call.

### LanceDB

Graduated to Rust SDK v1.0.0, actively developed. Embedded-process, columnar (Lance format), IVF-PQ
ANN by default, full-text search now included. Clearly production-grade. **Still not relevant to us:**
LanceDB is the right tool at thousands-to-millions of vectors with ANN requirements. Our `memory` table
at personal scale tops out in the hundreds of pre-filtered candidates. A LanceDB dependency for a
sub-millisecond brute-force cosine scan over a few hundred vectors is disproportionate — exactly the
"premature ANN index" concern in research/18 §8. If the harness ever reaches 10K+ memories and linear
scan latency is measured as a problem, LanceDB would be the first thing to revisit. Not now.

### Overall field delta verdict

**Design is not stale.** SQLite FTS5 + hand-rolled cosine + oMLX embeddings is still the right stack
for our scale, constraints, and dependency posture. The field has matured (LanceDB 1.0, sqlite-vec
stabilizing, ort/candle production-grade) but in ways that validate the direction already chosen, not
challenge it. The one actionable note: sqlite-vec's brute-force path is now stable and could serve as
a design reference if we want to see how production code handles the f32 BLOB → cosine scan pattern
in SQLite; reading its C source is low-cost and may inform our Slice B implementation. That is a
mine-for-design, not a dependency.

---

## 6. Bottom line

**frankensearch — mine-for-design, do not vendor.**
The FSVI mmap+f16+SIMD brute-force vector scan and the RRF(k=60) fusion confirm our research/18 §10.1
design choices but add nothing we don't already know. The Tantivy dep duplicates FTS5, the fastembed/MiniLM
embed leg is the wrong integration shape for oMLX, and the MIT + OpenAI/Anthropic Rider creates an
irreducible legal ambiguity for a harness operated with Claude Code. Do not vendor; read the index and
fusion crates for implementation patterns if Slice B is built.

**fast_vector_similarity — skip.**
Implements rank-correlation measures, not cosine similarity — wrong tool for Slice B. No license file
(vendoring blocked by default). Bootstrap stats are partial and trivially replicable. Nothing here
that serves our retrieval or eval needs.

**cass_memory_system — mine-for-design only.**
The deterministic curator (no LLM at the merge stage) and anti-pattern-as-warning inversion are the
novel ideas. Both are already reflected in research/18 §10.3. TypeScript, not portable. Brief read; no
vendoring.

**coding_agent_session_search — mine-for-design only.**
Universal Connector schema (N provider formats → 1 SQLite schema) and atomic FTS index swap via
`renameat2 RENAME_EXCHANGE` are worth noting for future episodic-capture work. Tantivy + FastEmbed
stack: same vendor/mine verdicts as frankensearch. No vendoring.

**Field delta — no design changes required.**
LanceDB 1.0, sqlite-vec v0.1.x stable, ort/candle production-grade all confirm the direction without
changing the decision. Our FTS5 + hand-rolled cosine + oMLX design is not stale.

**Net across all candidates:** the "mine for design, don't vendor" default holds for every item.
The dependency-is-a-liability bar, FTS5-already-built, and oMLX-provides-embeddings constraints together
rule out every candidate as a runtime dependency. The design signal frankensearch and coding_agent_session_search
provide (RRF k=60, mmap f16 brute-force, atomic index swap) is already captured in research/18 and can
be referenced again during Slice B implementation without importing the crate.
