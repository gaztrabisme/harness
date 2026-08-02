# Key Decisions

Context → Decision. The "why the alternative lost" is the reusable part.

## Standing operational note: DeepSeek key — no rotation reminders
The DeepSeek API key is a **throwaway, ~$5-capped** key Gary owns and manages. It is **hardcode-authorized for
use as-is**; rotation is Gary's call and his alone. **Do not surface "rotate the key" as a next-action, a fork,
or a nag** — ever. Keep it out of *committed files* (env `DEEPSEEK_API_KEY` only, per research/30) as ordinary
hygiene, but treat the key itself as available and not a blocker.

## Process gate: adversarial review is mandatory for hard-to-reverse or silent-failure designs
**Context:** the trunk's spine/gate semantics got a design-stage adversarial review (the C1/C2/C3 blockers baked
into the board came from a critical pass over the `close_policy` port, research/15). It paid off because the FSM
is load-bearing — a wrong primitive is inherited by everything above it. As we move to the pillars we must not
let that review become honor-system / skipped-when-busy.
**Decision — the trigger rule (apply before the first build slice of any pillar):** run a **design-stage
adversarial review** when a mistake would be either **(a) hard to reverse** (a sticky schema, accumulated data,
an FSM/lease primitive) **or (b) silent** (concurrency races, retrieval quality, self-modification that weakens a
gate). If neither holds — small, settled, loud-failure work — a review is theatre; skip it (Wu Wei) and rely on
normal Verify.
**Applied to the roadmap:**
- **Telemetry/memory plane** → REVIEW (sticky trajectory + sidecar schema; retrieval failure is silent). ← next.
- **Coordinator / parallel** → REVIEW (claim/lease races, stall/deadlock, reconcile semantics).
- **Self-evolution** → REVIEW at design AND pre-land (a weakened gate is silent + catastrophic).
- **Harden gate** → EXEMPT (small, settled = mutation-gating already decided, loud failure). Normal Verify only.
**How:** a focused adversarial subagent / Plan pass over the design doc for the lighter ones; a multi-agent
find→refute workflow (explicit opt-in) for the heavy ones. The review critiques the *design artifact* (e.g.
`research/NN-*.md`) before any slice is cut.
**Dogfood target (deferred):** make this a real gate — a `design`-kind ticket cannot leave Align without an
`adversarial_reviewed` artifact, same shape as `criteria_confirmed`. We are building the thing that would enforce
this rule; eventually it should.

## Telemetry slice — design reviewed, scope revised (research/17 §4 + §8)
**Context:** first dogfood of the adversarial-review gate (above). Four independent reviewers
(schema-durability / runtime-failure / RL-data-validity / Wu-Wei-scope) attacked the telemetry plan; **unanimous
"not safe as written."**
**Decision (the corrected slice):** build a **`user_version` migrator FIRST** (the codebase had none — only
`CREATE TABLE IF NOT EXISTS`, which silently no-ops on column adds; "design once, never migrate" was really "can't
migrate"). With a migrator, `ALTER ADD COLUMN` makes deferral safe → build the **lean core** (~60 lines): one `run`
table written **at loop entry + updated on every exit** (crash = queryable `ended_at IS NULL`, not an orphan file),
`run_id` sortable PK, trajectory = the in-memory `messages` array dumped to a **derived, root-relative** JSONL with
**secret redaction at the write boundary**, run-level `model`/`provider`/`sampling` provenance, WAL + busy_timeout,
lenient (torn-tail-tolerant) reader, Recorder-failure non-fatal-but-loud. Defer `group_id`/`split`/per-step
provenance/tokens/CLI to a nullable `ALTER` when the consuming pillar lands; derive churn/tool-errors/wall-clock at
consumption.
**Why the alternatives lost (Rejected Approaches):**
- *End-write-only run row ("no row = crash, for free")* — ambiguous with in-flight and lost-write; the single most
  informative label (the harness fell over) would live in a directory listing, not SQL. → write at entry.
- *Reserve `group_id` now for GRPO groups* — speculative generality for an unbuilt pillar; with the migrator a
  nullable `ALTER` later is trivial. Also: rework `attempt`s are NOT a valid GRPO group (the prompt mutates between
  attempts) — only parallel same-prompt siblings are, needing a frozen-prompt snapshot built with the coordinator.
- *Store `trajectory_path` as a string* — it's a pure function of `(ticket,attempt,run_id)`; storing it regresses
  from `git::worktree_path`'s pattern and invites the dangling-path failure. → derive it.
- *Capture-everything-now-for-RL (per-step provider/sampling, diff-churn, etc.)* — the RL lens's panic assumed no
  migrator; once it exists, only trajectory content + per-run provenance + crash observability are truly
  unrecoverable, and the `messages`-array dump already carries content + frozen prompt + tool-errors for free.
- *Circularity is "an audit-view constraint, not capture"* — wrong: training on teacher trajectories is circular at
  the objective regardless of the judge. → capture `provider`/`model` so the corpus partitions teacher-vs-local.

## Telemetry Unit A — BUILT (board migrator + run table + ops)
**Context:** first build slice of the telemetry plane, post-review.
**Decision (shipped):** a `user_version` migrator in `schema::init` (read version → apply ordered `MIGRATIONS` →
bump), WAL + busy_timeout pragmas, and the `run` table as a **true migration v1** (NOT in the base SCHEMA — so the
migrator is actually exercised, proven by a v0→v1 upgrade test). The `run` row is written at loop **entry**
(`ended_at` NULL = the queryable crash label) and closed at exit; sortable caller-minted `run_id` TEXT PK; full
provenance (model/provider/sampling/project/attempt). `Board::start_run`/`finish_run`/`runs_for`. 23 tests, clippy clean.
**Rejected approach (the oMLX exercise's own attempt):** putting the `run` table in the base SCHEMA and "migrating"
by just bumping `user_version` — the bump then guards a no-op, so the ratchet is decorative; and a minimal `run`
schema (autoinc id, no provenance) loses the columns the RL/audit views need. The migration must *be* the table
creation, and the schema must carry provenance from day one (banked rows can't backfill it).

## Telemetry Unit B — BUILT (recorder + loop wiring) + the `truncated` stop_reason
**Context:** the agent half of the telemetry plane. While diagnosing *why* the oMLX Unit B exercise wrote nothing
(it didn't "decide to stop" — it spent its whole 8192-token output budget narrating a plan and was **truncated
mid-line**, and the loop treats any non-tool-call response as done), the planned `stop_reason` taxonomy
(`completed | max_iters | error`) was caught mislabeling: a natural loop-return maps to `completed`, so this
truncated **no-op would have been recorded as a success**.
**Decision (shipped):** add **`truncated`** as a first-class outcome. `classify_stop(last, hit_max)` derives the
run label from the loop's exit — `StopReason::Length → truncated`, `End → completed`, exhausted → `max_iters`,
provider-error → `error` (closed inline before the `?` propagates, so no orphan open rows). The provider already
preserved `finish_reason: "length" → StopReason::Length`; the loop had been *discarding* it (only checked
`!= ToolCalls`). Recorder writes redacted per-message JSONL at a **root-relative** path (`runs_path`, N1: survives
land); `read_trajectory` is lenient (torn-tail tolerant); `agent trajectory <id>` is the read path. **33 tests +
live forced-truncation run** (`max_tokens=64` → real `truncated` row + trajectory; key redacted on disk).
**Rejected approach:** collapsing `Length` into `completed` (what the loop did) — it makes a budget-cut no-op
indistinguishable from a genuine sign-off, which silently poisons any finetune/RL signal trained on the labels.
The label must encode *how* the turn ended, not just *that* the loop returned.
**Rejected approach (test method):** a bigger token budget (Gary's 128k instinct) to "test" truncation — truncation
is the thing being triggered, so a bigger budget makes it *less* likely; the right test FORCES it with a tiny
budget. (Also: 128k was never available — oMLX is capped at 32K total context, and `HARNESS_MAX_TOKENS` is the
*output* cap, not the context window. "Does room let the 35B finish?" is a separate capability probe, deferred.)
**Known-hole (deferred, Wu Wei):** the loop counts a no-tool-call / criteria-unmet turn as success at all, and
nothing forces plan→execute. That's loop-behavior tuning, separate from telemetry; prod is Claude's job for now.

## Memory pillar — Slice A BUILT (lexical sidecar) + design review (research/18)
**Context:** first memory pillar build, post adversarial review. The review's decisive Wu-Wei cut was to **defer the
vector leg, reflection, and the retention curve until MEASURED insufficient**, and slice the work: **A = lexical
sidecar (build now), B = vector leg (gated on a golden-set eval), C = decay+reflection (gated on A earning its
keep)**. The v2 schema carries ALL of B/C's columns so the deferred slices need no table rebuild.
**Decision (shipped, Slice A):** migration **v2** = `memory` table (full durable column set: body/salience/scope/
entities/files/project/ticket_id + the deferred-slice columns embedding/embed_model/embed_dim, usage_count/
last_used_ts, proof_count, promotion_state/reflected_at, evicted_at) + a `memory_fts` FTS5 **external-content**
index (`content='memory'`, `content_rowid='rowid'`) + **3 explicit triggers** (ai/ad/au, `'delete'`-command form)
keeping the index in sync. `recall` = **FTS5 BM25 only** (`ORDER BY bm25(memory_fts) ASC, salience DESC` — salience
is a pure tiebreak), with project/scope **pre-filter in WHERE**, `evicted_at IS NULL`, and a progressive-disclosure
projection (id/title/type). `remember` validates type ∈ the 7-set and salience ∈ [0,1]. `recall_body` returns the
full body and bumps usage_count/last_used_ts. `fts_query` sanitizes tokens (quote-wrap, drop punctuation-only).
8 tests, clippy clean, workspace green; live CLI smoke (`remember`/`recall`/`recall-body`) verified pre-filter +
BM25 order + body fetch.
**Decision — memory ops are self-contained (do NOT write the ticket `event` table):** the `event` audit table has
`issue_id` NOT NULL and is ticket-scoped, but memories can be project- or global-scoped (no ticket). Wiring memory
mutations through it would force a fake ticket id and couple two planes. Wu-Wei + decoupling → memory ops own their
own provenance (the row's `created_at`/`usage_count`/`evicted_at`); this is a **conscious deviation** from §10's
"audit events" phrasing, recorded here so it isn't mistaken for an oversight.
**Decision — type validated in Rust, scope validated in SQL:** `scope` keeps its CHECK (a closed 3-set); `type` is
validated in Rust against a const set (NO schema CHECK) because P5 case-law will grow the type vocabulary and a
CHECK would force a migration each time.
**Rejected approach — the `fts5` rusqlite cargo feature:** tried adding it, build failed ("rusqlite does not have
that feature"). FTS5 SQL (`CREATE VIRTUAL TABLE ... USING fts5`) is compiled into rusqlite's **`bundled` SQLite**
already; the `fts5` feature is the unrelated *Rust tokenizer* API we don't need. Reverted Cargo.toml — no feature,
no code change.
**Rejected approach — hard DELETE on eviction:** Slice C needs provenance (what was evicted, when) and Slice B's
model-swap guard reasons over historical rows; a hard delete destroys that. Chose **soft-delete** (`evicted_at`
timestamp; all reads filter `evicted_at IS NULL`).
**Rejected approach — embeddings/fusion/dedup/reflection in Slice A:** all deferred. Slice A is deliberately a pure
lexical leg so the golden-set eval can measure whether BM25 alone is insufficient *before* paying for a vector leg
and an `Embedder` abstraction (no second real impl exists yet — premature).

## Pillar P0 — auto-context priming (memory read-side wired into the loop) BUILT (2026-06-10)
**Context:** Slice A built the memory *store* + `recall`/`recall_body`, but `recall` was only ever called in the CLI
arm. Meanwhile explore's reflection-on-kill writes `lesson` post-mortems via `board.remember()` that **nobody read
back** — the compounding loop was open (write side live, read side dormant). P0 closes it: `run_ticket` recalls and
auto-injects relevant past experience into every worker's system prompt (research/18 §3 "loop integration"). Design
+ 2-reviewer adversarial review run on the new decisions; both converged on cold-start noise (CRITICAL) + body
truncation / signal integrity / SQL drift — all folded in BEFORE coding.
**Decision (shipped):** new `recall_primed(query, k, project) -> Vec<PrimedHit>` (body-carrying, all-scopes) on a
shared private `recall_rows(query, k, project, scope, include_body)` — the **single SQL source of truth** that
`recall` (index-only, scope-filtered, `include_body=false`) and `recall_primed` (body, all-scopes) both delegate to,
so the two query paths can never drift. New `PrimedHit{title,type,body}` (vs `MemoryHit`'s id/title/type) because the
auto-written post-mortem **titles are generic** ("explore <t> w<k>: failed approach") — titles-only priming carries
no signal; the lesson is the body. `run_ticket` calls it best-effort (Err → `[mem ] WARN` + no priming, never sinks
the run); `build_system_prompt` injects a `### Relevant past experience (recalled)` section after case-law, omitted
when empty. `MEMORY_RECALL_K=5` count ceiling (mirrors `CASE_LAW_MAX_BULLETS`). Gate = 4 board + 1 agent
deterministic tests (assembly + non-bump + floor); clippy clean, workspace green.
**Decision — query on the ticket TITLE only (not the whole workpad):** the title is signal-dense; the full workpad is
mostly boilerplate section headers that over-match every memory. Narrow query = precision.
**Decision — token-overlap floor is STRUCTURAL, not a numeric BM25 cutoff:** each hit is kept only if it shares
≥`min(2, n_query_tokens)` distinct ≥3-char tokens (lowercased, de-duped) with the query over `title+body`. This is a
*set-overlap* gate computed in Rust, deliberately NOT a `bm25() > threshold` SQL cutoff — so it does **not** reopen
the settled "defer the numeric relevance cutoff + its golden-set tuning to Slice B" decision (that one forbids
inventing the magic number now; this invents no number, it requires shared vocabulary). It is what makes the
all-scopes recall safe: a ticket-scoped post-mortem can surface on related work without an unrelated ticket flooding
the prompt.
**Decision — priming does NOT bump usage_count/last_used_ts (non-bump):** ambient auto-injection is not deliberate
retrieval use. `recall_body` (the human's deliberate "show me this one") remains the only path that bumps. Bumping on
auto-injection would, once a decay/eviction consumer exists (Slice C), create a rich-get-richer immortal fixed point
(primed → bumped → ranks higher → primed again) with nothing to counter it. Documented as "no decay consumer yet" so
the reasoning is re-checkable when Slice C lands, not asserted as eternal principle.
**Decision — head+tail body clamp (cap 240, head-heavy 2:1):** a post-mortem's Strategy is at the head and the
validation outcome at the tail; a head-only truncation would drop the "why it failed" tail. `clamp_body` keeps both
ends with a ` … ` elision marker, char-boundary-safe. Mirrors `refeed::cap`'s both-ends philosophy.
**Rejected approach — titles-only priming (reuse `MemoryHit`):** the auto-written post-mortem titles are generic by
construction, so a titles-only section injects N near-identical "failed approach" lines with zero actionable content.
Priming MUST carry bodies inline; hence the separate `PrimedHit` shape.
**Rejected approach — capture-as-loop-tool now (a `remember` agent tool):** letting the worker write its own memories
mid-run needs tools↔board plumbing the loop doesn't have (tools are currently board-free). Deferred until the
read-side proves its worth; the write side is already covered by explore's reflection-on-kill.

## Memory Slice B (vector leg) — NO-GO + the floor over-filters (research/28 + 29) (2026-06-11)
**Context:** the pre-registered Slice B GO/NO-GO retrieval eval (research/18 §10.5). A 4-reviewer adversarial pass
**demolished** the originally-planned full eval (research/28 §10) on five convergent findings: wrong system-under-test
(`recall_primed`, which has a floor, vs the floorless `recall`); a provenance-relevance label biased toward NO-GO;
underpowered at N≤40 (~13% detectable effect); a tautological EXACT/PARAPHRASE split; disproportionate apparatus for
an ambient-hint use. Resolution = **DESCOPE** to a 3-arm hand probe + keep the L4 Rust-native survey (research/29).
**Method:** 14-row corpus distilled from our own artifacts, 14 paraphrase queries, **3 arms** — (1) `recall_primed`
(floored, shipped), (2) `recall` (floorless BM25, shipped — the decomposition trick: isolates "does the FLOOR cause
the miss?" from "does DENSE beat BM25?"), (3) jina-v5 dense via oMLX. Throwaway only (`#[ignore]` test deleted after
run; `/tmp` dense probe; oMLX exercise, nothing landed).
**Result (recall@5):** primed **12/14** · floorless **13/14** · jina-v5 dense **14/14**. Two distinct misses:
(a) *floor-dropped* "rules for what to put in a git commit" → *commit discipline* — floorless ranks it **#1**, the
`need≥2` floor drops it (query shares only ONE content token, "commit"); (b) *zero-overlap* "stop the agent from
escaping its isolated work area…" ↔ *worktree confinement* — shares NO ≥3-char token, both lexical arms miss, only
dense recovers (rank-1, cos 0.509). Dense cosines: true paraphrases 0.45–0.63, exact 0.82.
**Decision — Slice B vector leg: NO-GO (for now).** Floorless lexical handles 13/14; dense's marginal **1/14**
recovery is confined to zero-token-overlap paraphrases and does not clear the dependency bar for an ambient-hint
use. The `embedding`/`embed_*` columns stay **dormant insurance** — now "warmed" (the wiring is understood, the
oMLX embed path exercised) so a future GO is build-not-vendor.
**Decision — survey leg: nothing to vendor (research/29).** frankensearch = mine-for-design (Tantivy duplicates the
FTS5 heavy dep; in-process ONNX is the wrong shape for HTTP oMLX; license-rider ambiguity). fast_vector_similarity =
skip (rank-correlation not cosine; no license file). cass_memory_system / coding_agent_session_search = design-only,
already folded into research/18 §10.3. Field delta confirms the design isn't stale. Even on a GO, Slice B is built,
not adopted.
**Clarification — two questions, don't conflate them (the "just use Dicklesworthstone's memory system?" challenge).**
The NO-GO is **not** "dense retrieval is useless." It answers exactly one question: *does the dense leg beat our
MEASURED sparse leg by enough to justify a live oMLX dependency on the recall hot path, for OUR ambient-hint use?*
On our 14-row corpus it does not (sparse 13/14, dense recovers +1). That is narrower than the rag-heuristics
"dense alone misses paraphrase, sparse alone misses exact terms — **use both**" default, and deliberately so:
that heuristic is the right prior for an *unknown* workload; we have a *measurement* on our actual one, and
**gate-by-a-number beats gate-by-the-heuristic**. The one real gap (zero-token-overlap paraphrase) is better closed
by **query-side expansion pushed to the LLM caller** (HyDE / multi-query — zero inference dependency, the caller is
already an LLM) than by standing up a dense index. "Should we adopt cass_memory_system?" is a *separate* question
with its own answer: **no — adopt the design, not the system.** cass is TypeScript + Bun + a required separate `cass`
CLI; adopting it means operating a Node/Bun sidecar next to a Rust harness, the exact inverse of Strategy A
(Rust-native, depend-on-none). The license rider (MIT + OpenAI/Anthropic Rider, author declines PRs) means
fork-and-own forever with no upstream — fine for *reading*, costly for *depending*. The valuable part of cass is its
**design** (ACE pipeline: Generator→Reflector→Validator→**Curator-with-no-LLM** to prevent context collapse; 90-day
half-life decay w/ 4× harm multiplier; anti-pattern inversion = harmful rule → warning, never silent delete), and
that lands in **Slice C** (curation/decay), not Slice B (vector retrieval). Net: keep lexical-only now, build cass's
curation design into Slice C when volume justifies it, leave the embedding columns dormant.
**Rejected approach — the full golden-set/CI eval apparatus:** underpowered (N≤40) and partly circular
(provenance-as-relevance, EXACT/PARAPHRASE tautology). A targeted hand probe answered the actual question (floor vs
dense) at a fraction of the cost. Gate-by-a-number still honored — the number is recall@5 over a fixed probe set.
**Follow-up (filed, NOT slammed in) — `recall_primed` floor over-filters single-content-token paraphrases.** The
`need = q_tokens.len().clamp(1,2)` floor cost a free recovery (the commit-discipline row floorless ranks #1).
Recommended fix = a **BM25-score-aware** floor, NOT a blind `need=1` (which reintroduces cold-store noise the floor
was added to suppress). Not changed now: the noise-suppression side of the tradeoff is unmeasured, and changing
gated code on half the evidence violates gate-by-a-number. Its own ticket, with the 1/14 datum attached.
**→ RESOLVED (t5, 2026-06-11) — see "t5 priming-floor fix" below. The eval REFUTED this recommended BM25-score
floor; the actual defect was precision, not recall. Shipped a\*1 (strip stopwords, keep need≥1).**

## t5 priming-floor fix — strip stopwords, keep need≥1 (a\*1) (research/31) (2026-06-11)
**Context:** the filed t5 follow-up (above) claimed the `recall_primed` overlap floor `need=clamp(len,1,2)` was too
strict — it dropped single-content-token paraphrases (the 1/14 commit-discipline datum). Recommended fix on file:
a BM25-score-aware floor. S1 built a real eval to settle it by a number before touching gated code.
**Method (research/31):** 36-row corpus mined from project history, 57 provenance-labelled queries (12 clean / 21
para / 24 noise), top-K=5. Per-axis **recovery** (target survives floor) vs **noise-leak** (noise query keeps ≥1).
Significance by exact two-sided binomial **McNemar, decomposed per-axis** (mixed-set McNemar cancels a precision
gain against a recall cost ~1:1 → p≈1.0, wrong frame). Even/odd calibration/holdout split; BM25 θ chosen only on
calibration, pre-registered. Six arms: `a` (need≥2 raw, shipped-pre), `b` (need≥1 raw), `b′` (need≥1 raw & bm25≤θ),
`a*` (need≥2 content), **`a*1` (need≥1 content)**, `c` (need≥1 content & bm25≤θ).
**Result (n=57):** a=33/33 rec,18/24 noise · a*=22/33,1/24 · **a\*1=32/33,13/24** · b′/c=22/33,0/24. Decision metric
F1/F2 (recall-weighted is correct for *ambient* priming — a missed lesson costs more than a mildly-irrelevant injected
one the model ignores): **a\*1 wins F1=0.819 and F2=0.908** (both best). a* wins only F0.5 (over-penalizes the cheap
false-positive). The decisive panel — content-overlap==1 (the *true* t5 regime, n=10): a\*1 **10/10**, a* **0/10**,
b′/c **2/10**. a vs a\*1 McNemar: RECALL p=1.0 (recall held — the one "loss" `mint_id_collision` shared only stopwords,
content-overlap 0, correct to drop), NOISE net +5 blocked (directional, no tuned constant).
**Decision — ship a\*1:** strip stopwords from the `recall_primed` floor via `content_tokens()`, keep `need≥1`. Noise
18→13/24, recovery held 32/33, **no tuned constant**. Code: `STOPWORDS` + `content_tokens()` in `memory.rs`; floor
keeps a hit iff it shares ≥1 content token with the query. Locked regression test
`recall_primed_floor_keeps_content_drops_stopword_only` pins a\*1 between the old floor (drops the kept row) and naive
need≥1-raw (leaks the dropped row). Eval kept as `#[ignore] t5_floor_eval_v2`.
**Honest reframe:** the original t5 premise (floor too strict) is **FALSE on a real corpus** — `a` recovers 33/33 via
stopword inflation; the 1/14 miss was a 14-row-probe artifact. The actual defect the eval surfaced was **precision,
not recall**. Residual 13/24 leak = coincidental single-token matches ("version"/"memory"/"search") — no lexical floor
separates a topical single-token match from a coincidental one; that is structurally a **Slice-B (embed+rerank)**
problem, and the 36-row corpus overstates it (weak single-token matches rarely survive top-K at scale).

### Rejected approaches (t5)
- **BM25-score-aware floor (`b′`, `c`) — REJECTED (research/31).** θ-swept, it only retraces the *same* recall/precision
  frontier the count knob already spans (b′@-7≈a*, b′@-3≈a), adding nothing the count lever doesn't — at the cost of a
  corpus-calibrated magic θ the module doc explicitly defers to Slice B. Its pre-registered gate ("b′ beats `a` on
  recovery") is unreachable since `a` recovers 33/33 → honest null → no-ship (criteria not moved to fit). This refutes
  the fix recommended in the filed follow-up above.
- **Strict content floor (`a*`, need≥2 content) — REJECTED (research/31).** Buys precision (1/24 noise) by **dropping
  the entire t5 regime** (0/10 single-content-token paraphrases) — it regresses the exact axis the ticket was filed for.
- **Blind need≥1 raw (`b`) — REJECTED.** Relaxing the count without stripping stopwords leaks 23/24 noise (every
  stopword-only FTS match survives) — strictly worse than a\*1's 13/24.

## Memory S3 — researcher subagent BUILT, default-OFF + fall-back-safe; ship-eval deferred (research/31) (2026-06-11)
**Context:** `recall_primed` is a single lexical shot — a BM25 floor over titles/bodies, no model in the loop. The S3
ambition (Gary): use the local LLM as a *deep-research* subagent with its own context tree that explores the memory
store and feeds only the relevant lessons back. The §5-mandated live behavior probe (breadcrumb 2026-06-11) confirmed
the served 35B reliably drives a `search→read→return` loop, with a **safe** failure mode (redundant calls → runs out
of turns; never wrong content). Scope B (Gary, verbatim "Go with B"): build the plumbing, **defer the ship decision**
until the store has real lessons.
**Decision — build it default-OFF behind `HARNESS_MEMORY_RESEARCHER`, fall back to `recall_primed` on any
empty/failure/timeout.** Pure selection core in `crates/agent/src/researcher.rs` (explore.rs pattern: value types +
contract prompt + `parse_packet` + `select_lessons`, NO board/model — 12 units); board-coupled tool dispatch + the
oMLX loop glue (`run_memory_researcher`, two READ-ONLY tools `search_memory`/`recall_body`) in `main.rs` beside
`run_explore`, oMLX-pinned, 25s `tokio::time::timeout`, `TOOL_CALL_CAP=12`. The probe's two failure mitigations are
baked in: a **recall_body dedup guard** (cached echo on repeat) and a **cap-hit synthesis** (`select_lessons` with no
usable citation → assemble from already-recalled bodies). A researched lesson and the `recall_primed` fallback emit the
**same `PrimedHit` shape** (S2a added `PrimedHit.id`), bounded by the same `board::clamp_primed_body` ceiling. Default
OFF ⇒ byte-identical to today.
**The contract distinction that earns the design:** an *empty* packet (`{"used_ids":[],"context":""}`) is a valid
"nothing relevant" → the caller returns None and falls back to the lexical floor; a cap-hit/parse-failure → synthesize
from what was recalled. These are different code paths (`Some(p) if p.is_empty() => return None` vs `select_lessons(&recalled, &[], k)`).
A dead-code warning during the build exposed that an early version collapsed them (empty → wrongly synthesized) — fixed.
**Gate:** 12 deterministic researcher units + workspace green (agent 111/3-ignored) + clippy clean on production code +
a `#[ignore]` live oMLX smoke (`memory_researcher_live_omlx`) — PASSED (selected the relevant retry/backoff lesson over
a flexbox distractor, verbatim body, no fabrication, 15.15s).
**Why the ship-eval is deferred, not skipped (NOT a moved goalpost):** the BM25 corpus is saturated (t5 eval: 32-33/33
recovery), so a pre-registered "+15pt over `recall_primed`" ship gate is **unclearable today** — there is no headroom to
measure against. Shipping the *measurement* now would be gating on a number the corpus can't move. The infra is built and
proven correct (units + live smoke); the ship decision waits for an accumulated store, exactly the posture of the
dormant embedding columns (Slice B NO-GO) — dormant infra warmed and ready, activated by a future number.
**Rejected approaches (S3):**
- *Default-ON / replace `recall_primed`* — REJECTED. With no ship-eval headroom, flipping the default would change live
  behavior on faith; default-OFF keeps the proven floor as the contract and makes the researcher opt-in until a number
  justifies it.
- *Put `search_memory`/`recall_body` in the worktree-confined `tools.rs`* — REJECTED. They are board-coupled (read the
  rusqlite store), not filesystem tools; dispatching them inline in `main.rs` keeps `tools.rs` to the sandboxed
  write/bash/read surface (the t4 confinement invariant) and keeps the researcher's tools READ-ONLY by construction.
- *Run the researcher on the selected landing backend* — REJECTED. Like `explore`, it's a throwaway probe that never
  lands; pinning it to oMLX means a research detour never spends paid cloud compute.

## Board-lifecycle — Fix A (rework reconvergence) + Fix B (non-landing close) BUILT (research/32) (2026-06-11)
**Context:** dogfooding the harness surfaced two terminal-reachability gaps in the FSM (spine.rs), batched from t5's
close-out into a board-hygiene pillar. (A) `align()` only did Todo→InProgress, but a reworked ticket sits in
`Rework` and Rework→InProgress is illegal (forward_targets(Rework)=[Align]) — so `rework`+`align` could not
re-align a ticket: the hard-reset loop didn't reconverge. (B) `Done` was reachable ONLY via `Land→Done`
(GATE_LANDED, which needs a committed tree), so a non-code ticket (research/docs/spike — nothing to land) and an
abandoned ticket (t3, a throwaway oMLX exercise stuck in review) had **no path to a terminal state**.
**Decision (A):** `align()` routes by current status — `Rework→Align`, else `→InProgress`. The reworked ticket
re-enters Align (re-locking tools until the operator re-confirms criteria); the attempt bump scopes the stale
`criteria_confirmed` so it must be re-cleared.
**Decision (B):** the minimal FSM surgery — **one edge + one gate**. `forward_targets(Review)` gains `Done`;
`required_gates_for` adds `(Review,Done)=>[GATE_RESOLVED]`; new `GATE_RESOLVED` const is **human-sourced** in
`gate_source_required`. A `close <id> "<note>" [--abandon]` verb requires a non-empty resolution note (the
artifact), refuses non-review tickets (drive it to review first, or `rework` to reset), refuses **code** kinds
without `--abandon` (a code ticket SHOULD land, not silently close), and removes the worktree on an abandoned code
ticket (idempotent). The **absence** of a `landed` gate row makes "resolved-closed" auditable vs "landed-done".
**Both invariants HELD:** the keystone (an agent provider with read/write/bash can never self-promote to a
terminal) survives because GATE_RESOLVED is human-only — the loop cannot self-close, exactly as it cannot
self-land; the no-skip invariant (`lib.rs` criterion #2, `validate_transition(Todo,Done)` is Err) survives because
only `Review→Done` is added — the protection stays **structural** (in the transition map), not deferred to a gate.
**Gates:** spine `close_path_is_human_gated_and_review_only` + `align_reconverges_a_reworked_ticket`; board
`close_gate_enforced_and_attempt_scoped`; agent `close_verb_guards_and_resolves`. Live dogfood: t3 `review→done`
via `close --abandon`, gate trail = `resolved`(human) present + **zero** `landed` rows. board 27 / agent 113,
clippy clean (workspace + `--tests`). The 4 carryover clippy `--tests` nits cleared in the same pass.
**Why the alternatives lost (Rejected Approaches):** see the research/32 entries folded into "## Rejected Approaches".

## Sandbox escape: tool writes must be confined to the worktree (write_file path bug)
**Context:** the oMLX dogfood exercise wrote its files into the MAIN working tree, not its isolated worktree.
**Root cause:** `tools.rs:61 cwd.join(path)` — Rust's `Path::join` *discards the base* when the argument is
absolute, so `write_file`/`bash` with an absolute path escape the worktree to anywhere on disk; and the workpad
header leaks the absolute repo root into the prompt, handing the model the escape route.
**Decision:** an isolation + security hole, not cosmetic — tracked as hardening ticket **t4**, fixed **before** any
further agent-run exercises. Fix = resolve+canonicalize against cwd and reject escapes (absolute / `../`), and stop
leaking the absolute root in the prompt.
**Rejected approach:** trusting `cwd.join(model_path)` to stay within `cwd`. It does not. Per-tool *status* gating
(the existing gate) controls *whether* a tool runs, never *where* it writes — path confinement is a separate axis.
**Process lesson (logged):** never let a background subagent mutate shared git state — it ran `git stash` mid-edit
and swallowed the parent's working changes. Background agents stay in their own worktree/sandbox.
**RESOLVED (t4):** `tools::confine` resolves the model's path against the worktree and rejects absolute paths +
`../`-escapes (lexical normalize + `starts_with` the canonical cwd); applied to `read_file` *and* `write_file`
(3 regression tests, incl. "nothing written outside the worktree"). `bash` is documented as cwd-scoped but NOT a
hard sandbox (OS-level isolation deferred — it can still `cd`/abs-path). `workpad_header` now renders a repo-relative
path (`harness/.harness/worktrees/<id>`), so the absolute host layout is no longer leaked into the prompt
(defence-in-depth; the tools are confined regardless). 13 agent tests, clippy clean.
**Validated end-to-end (Unit B oMLX exercise, 2026-06-09):** the same loop that escaped to the MAIN tree on the
Unit A exercise ran again post-fix and stayed fully inside its worktree — a monitor watched main-tree `crates/`
for the whole run and saw zero modifications. Confinement holds in the real loop, not just the unit tests.

## Build a harness, not stay a pure-instruction skill
**Context:** the `dev` skill's judgment layer (heuristics) is good; the *orchestration* layer is advisory
prose the model can ignore — that's the ceiling.
**Decision:** build a harness where process is load-bearing control flow. The skill's `references/*` port as
the judgment layer.

## The Align gate is the core primitive — VALIDATED
**Context:** friction #1/#5/#6 (retyping "gather context, confirm before acting"); need it enforced, not asked.
**Decision:** a gate that blocks execution tools until a plan is confirmed. Proven at runtime in the spike
(`research/07`): a `tool_call` hook returning `{block}` denied write/bash in align phase, `/align` unlocked,
phase persisted across reload. The architecture's keystone.

## Memory = curated wiki + decaying sidecar, local embeddings
**Context:** claude-mem worked but burned usage as memory grew (friction #4).
**Decision:** CoALA split — wiki (semantic+procedural, never decays) + episodic sidecar (decays), embeddings
on local oMLX (~zero cost). Top-k retrieval w/ progressive disclosure. Detail: `research/01`.

## Test quality is a number (mutation), not a review
**Context:** stopped reviewing AI-written tests → the adversarial-review-of-tests became theater (friction).
**Decision:** a **Harden gate** between Test and Implement, gated on **mutation score**, mutator on a
different provider than the author. Runs *before* implementation so tests can't be shaped to the code.
Detail: `research/03`.

## Harden pillar — BUILT (§7 mutation gate, review-exempt) (2026-06-10)
**Context:** the design decision above ("test quality is a number") made concrete. Per the Process-gate decision,
Harden is **adv-review-EXEMPT** (the gate is a computed number, not a hard-to-reverse or silent-failure design).
**Decision (shipped):** new spine gate `GATE_MUTATION = "mutation_score"` (source **Machine** — an agent provider,
which has read/write/bash, *can* satisfy it: it's a computed number, not a human call). `required_gates_for(Verify,
Review, kind)` returns `[tests_green, mutation_score]` for code kinds and `[tests_green]` otherwise, keyed off new
`kind_is_code(kind) = matches!(kind, "build" | "bugfix" | "refactor")`. New `agent harden <id>` runs **cargo-mutants**
(v27.1.0) as a **dev-tool subprocess** (like git/clippy — NOT a linked crate dep): commits worktree stragglers,
diffs `base...HEAD`, `cargo mutants --in-diff <diff> --output <tmp>`, parses `mutants.out/outcomes.json` top-level
ints, reports the gate at threshold `HARNESS_MUTATION_THRESHOLD` (default **0.70**). `run_verify` refuses a
not-yet-hardened code ticket **up front** (while still InProgress, via new public `Board::gate_satisfied`) so the
order is **`run → harden → verify`**. 20 board + 21 agent tests, clippy clean; live cargo-mutants dogfood (8/8).
**Decision — `kind_is_code` is conservative (allow-list, not deny-list):** only KNOWN code kinds gate on mutation.
A novel kind defaults to NON-code, so it leaves Verify on `tests_green` alone rather than silently acquiring an
un-runnable mutation gate that would wedge every ticket of that kind in Verify forever. Widening is a one-line,
test-pinned change (`harden_gate_is_kind_gated` covers both arms).
**Decision — mutation score = `caught / (caught + missed + timeout)`:** `unviable` excluded (un-compilable mutants
≈ equivalent mutants, not a test gap); **timeouts count AGAINST** (conservative — a timeout is an un-killed mutant,
surfaced not hidden); empty denominator → vacuous **1.0** (an empty/trivial diff passes, never NaN/div-by-zero).
**Decision — judge cargo-mutants by its artifact, not its exit code:** the tool **exits NON-ZERO whenever mutants
survive**, which is a *finding* (low score → gate fails honestly), not a tool failure. `run_harden` keys off the
presence of `outcomes.json`; a missing file is the real error.
**Decision — threshold default 0.70, env-tunable, ratchet up never to 100%:** coach-not-gatekeeper (§7). The gate
exists to surface untested logic, not to demand equivalent-mutant-chasing perfection.
**Rejected approach — gate `verify` by catching its error and parsing the message:** instead added a clean
read-only `Board::gate_satisfied(id, gate)` so verify checks the Harden *artifact* directly ("gate by an artifact,
not a caught error string"). `missing_gates` was refactored onto the same primitive.
**Rejected approach — manufacture subagent fan-out for this slice:** Wu-Wei — a ~150-line cohesive change does not
need a coordinator. Genuine subagent coordination patterns are the *subject* of the parallel-exploration pillar,
not a ceremony to bolt onto unrelated work.
**Rejected approach — running `cargo fmt`:** the project uses hard tabs with no `rustfmt.toml`, so `cargo fmt
--check` flags the entire repo (≈90 untouched files). fmt is a **non-signal** here; new code matches the
surrounding tab style by hand.

## Self-evolution under constitution / case-law
**Context:** manual evolve, and lessons over-fit to the specific problem (friction #3).
**Decision:** immutable constitution (spine, integrity, gates, the eval metric) vs agent-editable case-law
(heuristics, prompts, lessons); a **generalization filter** (two-column rewrite, commit only the general
column); git-versioned + human-gated. Detail: `research/04`.

## Model-per-role with local oMLX
**Decision:** recon→local/Haiku, build→Sonnet, coordination+adversarial→Opus, test-author vs mutator on
different providers. Validated: oMLX drops into Pi via `openai-completions` + `baseUrl`.

---

## Foundation: Rust-native own-core — RESOLVED (signed off 2026-06-08)
**Context:** the language/runtime strategy gated Phase 0; pressure-tested via DR1 (oh-my-pi, `research/11`),
DR2 (beads, `research/12`), DR2-prime (beads_rust, `research/15`); synthesized in `research/13`.
**Decision:** **Strategy A — full Rust-native own-core.** Own agent loop + tool dispatch + a typed `enum Phase`
Align gate; **lift `crates/pi-iso`** (oh-my-pi) for worktree isolation; board hand-rolled on **`rusqlite`
(SQLite + JSONL)**, **porting `beads_rust`'s `close_policy.rs` state-machine + gate engine** (~250 lines);
minimal Rust provider layer (oMLX + Anthropic, ~1.5k LoC). Mine pi-mono/omp/symphony/beads for designs; depend
on none. **Phase 0 opens with a bounded sizing spike** (own loop + 2 providers + Rust gate + pi-iso → oMLX)
before the trunk; fallback if sizing balloons = C (stay TS-on-Pi).
**Why B/C lost:** B (Rust core + TS brain, the omp model) — DR1 found omp's agent loop/tools/providers/gate are
100% TS, so B hands us none of A's hard parts and couples us to a solo fork; the one severable Rust asset
(`pi-iso`) we lift anyway. C (stay TS-on-Pi) — couples us to two upstreams forever and forfeits the
compiler-as-guardrail the self-building endgame wants.

## Ticket `kind` is first-class; gate profiles select by kind — DECIDED (2026-06-09)
**Context:** Gary works both pure-technical and business-first (brainstorm → intel → analysis → grounding →
build); the harness must carry non-code work as first-class, not as a chat side-channel.
**Decision:** `ticket.kind` (`business-grounding | research | build | …`) is a schema field; the single spine
selects a **per-kind gate profile** per transition (`br`'s `close_policy.rs` already does config-driven
gates-per-transition — reinforces the lift). `business-grounding` gates on a **scored + claims-verified
artifact** (BXT scoring + every claim sourced/dated). The existing `business-intelligence` /
`ms-ai-discovery` skills plug in there **as judgment-layer content the gate consumes (peer to ml-heuristics),
not as harness mechanics** — harness owns enforcement, skills own "what good looks like." Grounding tickets
**block** build tickets via the `edge` table; the artifact promotes to the wiki as P0 priming.

## Headless core behind a protocol boundary; frontends pluggable — DECIDED (2026-06-09)
**Context:** beyond CLI, Gary wants a Zed+Warp-class desktop GUI eventually (both now OSS).
**Decision:** the Rust core is a library + a stable command/event protocol; **CLI / TUI (`ratatui`) / a later
GPUI app are interchangeable clients.** Design the seam from the start (cheap module boundary). The GPUI
desktop frontend — **GPUI = Apache-2.0**; reimplement Warp's **MIT** block-UX *pattern* on it, fork neither
app — is a **later pillar, foundation-independent.** Watch GPUI's transitive-GPL issue (#55470) before
shipping. Resolves design-v2 §14 fork #3.
**Rejected approach — adopt Terax UI as the harness frontend (2026-06-30).** Terax (crynta/terax-ai) is a
7MB AI terminal: Tauri 2 + **Rust backend** but a **React 19 / TypeScript frontend** (xterm.js, CodeMirror 6,
Vercel AI SDK, Zustand, shadcn/ui). "Customize its chat/terminal + add a Jira-board visual" = writing React
and operating a Node/Tauri sidecar next to the Rust harness — the **exact inverse of Strategy A** and the same
shape already rejected for cass (this file, Memory Slice B). It also reopens *this* settled decision: the GUI
path is **GPUI**, in-language, dep-light. Two further category errors: (a) intake is *backend drafting logic*,
not a GUI — a chat pane is one presentation of the Align loop, not the loop; building a Terax frontend would
build the presentation before the logic exists; (b) presentation is already decided here. **Keep Terax on the
mine-for-design shelf** (next to Warp/Zed/cass) for the GPUI frontend pillar — the keeper idea is rendering the
board (tickets/edges/gates) as a **kanban**, which only pays off once multi-ticket orchestration drives it.
Approval-gated exec ≈ Align, `TERAX.md` ≈ our wiki+board, block-UX ≈ the planned Warp-on-GPUI pattern.

## Task intake: fidelity is a dial onto the Align gate, not file-vs-chat — DESIGN (2026-06-10, deferred)
**Context:** how does a user hand the (fully-built) harness a task — drop a complete planning file, or chat
back-and-forth to lay the foundation? Asked at the system-design altitude (the harness's *front door*), assuming
all pillars exist.
**Decision (the reframe): it's not file-vs-chat — it's seed-at-any-fidelity → system DRAFTS a workpad → the Align
gate IS the back-and-forth.** The mechanism already exists; this names it as "intake" and adds one missing step:
1. **Seed at whatever fidelity you have** — one line *or* a full spec. Both valid; fidelity is a dial, not a mode.
2. **The system drafts the workpad** — proposes Plan / Acceptance Criteria / Validation from the seed. (This is
   exactly the *agent-drafts-plan-in-align* step slice 3 explicitly deferred — the one new build piece.)
3. **Align is the leveler.** Complete spec → Align is a one-shot confirm. Vague line → Align is several
   pushback-and-teach rounds until criteria are concrete. **Same Human gate (`criteria_confirmed`), variable
   depth.** Nothing executes pre-confirm — the keystone already enforces this. You never *choose* file or chat; the
   gap between your seed and "concrete enough to build" *is* the conversation, exactly as long as it needs to be.
**Second axis — sync vs async (both supported, falls out of existing primitives):**
- **File-drop = batch/queue:** the board already holds multiple tickets with priority → queue N overnight.
- **Chat = tight loop:** sit in Align on one gnarly ticket.
- **Confusions→Align makes the back-and-forth *deferred*:** even a fire-and-forget ticket bounces *itself* back to
  Align mid-flight on a confusion, re-locking tools until the operator weighs in. File it, walk away, get pulled
  back **only when genuinely stuck** — not babysitting, not blind fire-and-forget. The forever-personal ideal.
**Compounding (post-memory-pillar):** the draft step (#2) pre-fills criteria from past similar tickets, so the
Align loop *shortens over time* — the system needs less from you the more it's seen. Intake fidelity compounds.
**Why the alternatives lost:**
- *Pure file-drop (hand over a full spec, fire and forget)* — most tasks aren't specified well enough up front; the
  user doesn't know what they don't know. No gate against underspecification → garbage-in. (The draft+Align loop is
  precisely the underspecification catch.)
- *Pure chat (always interrogate from one line)* — forces a conversation even when the spec is already complete;
  high-friction for well-thought-through tasks. (The fidelity dial collapses Align to one-shot when the seed is rich.)
**Status:** design-only, **foundation-independent**, slots onto the existing Align primitive + the headless-core
protocol boundary (below). Build piece = the workpad-draft step; flesh out the draft-quality bar (how good a
criteria draft must be before it's worth confirming) when the slice is cut.

## Claude integration has TWO shapes; subscription backs only the worker — DECIDED (2026-06-09)
**Context:** Gary has a Claude Pro/Max **subscription** and wants to avoid metered `api.anthropic.com`
per-token billing. Question: can the subscription back the harness? (Verified via `claude-code-guide`,
sourced to code.claude.com/support.claude.com docs.)
**Finding (the hard constraint):** subscription OAuth (`claude setup-token` → `CLAUDE_CODE_OAUTH_TOKEN`)
authenticates **only** Claude Code's own agentic loop — `claude -p` / Agent SDK, which run *whole tasks*,
not single turns. Raw `/v1/messages` (messages-in → assistant+tool_calls-out, what `trait Provider` needs)
**rejects** subscription OAuth and **requires a metered API key.** There is **no subscription-backed
single-turn completion endpoint.** (From 2026-06-15 `claude -p`/SDK draw on a separate monthly Agent-SDK
credit — $20 Pro / $100 Max5x / $200 Max20x — distinct from interactive limits. ToS: personal automation
fine; reselling/proxying the sub to others is not.)
**Decision:** the harness carries **two distinct Claude integration shapes, not one:**
1. **`trait Provider` (single-turn):** **oMLX is the default workhorse** (free, local). **Raw-Anthropic is a
   structural-proof-only impl** — proves one normalized trait spans both wire formats; the *proof is
   compilation*, a live metered round-trip buys nothing about the abstraction and costs money. Not the daily
   path.
2. **Delegated worker (whole-task):** `claude -p --output-format stream-json` under `CLAUDE_CODE_OAUTH_TOKEN`
   — the **subscription-backed** escape hatch for hard roles. It runs *its own* loop, so you hand it a
   **task, not a turn** — which is exactly what a Pi-style isolated worker *is*. ~Zero marginal cost under
   the plan's Agent-SDK credit. **Trunk-era item (a worker backend), out of Phase-0 fence.**
**Why the obvious hope lost:** "drive my own single-turn loop on the subscription" is structurally
impossible — Anthropic gates single-turn completions behind the metered key and reserves OAuth for its own
agent loop. So Claude-for-cheap means *delegating tasks*, not *borrowing a completion endpoint*.

## Trunk slice 1: board + gate engine — BUILT (2026-06-09)
**Context:** first real trunk code (design-v2 §12); ported `br`'s `close_policy.rs` design onto our own
`rusqlite` board. Gated behind an **Align pass + a 3-critic adversarial review** (schema / state-machine /
gate-engine) before a line was written — both verdicts: needs-rework-before-build. The corrections are baked in.
**Decision:** `crates/board` — four tables (`ticket`, `edge`, `gate_results`, `event`) + a pure-function spine
(`validate_transition` + `required_gates_for` + `gate_source_required`) + a `Board::set_status` chokepoint.
~370 logic LoC, 6 tests green, clippy clean. The adversarial corrections that became load-bearing:
- **`attempt` epoch on `ticket` + in the `gate_results` PK** — entering `Rework` bumps the counter; gates are
  checked only at the current attempt, so a prior attempt's `pass` cannot satisfy a re-entered gate. (Killed
  the cross-confirmed blocker: stale-pass bypass on rework.)
- **Human-source gate class** — `gate_results.source ∈ {human,machine}`; `criteria_confirmed` is human-only and
  `report_gate` **rejects** a machine source. The Align gate stays human-cleared; an agent provider cannot
  self-certify it. (Killed: human-gate dissolution.)
- **`kind` in the `edge` PK** `(issue_id, depends_on_id, kind)` — a pair can be both `parent-child` AND `blocks`
  without overwrite.
- **Non-destructive back-edges** in the transition map (`in_progress→align` for Confusions, `verify→in_progress`
  / `review→in_progress` for minor misses); `Rework` reserved for genuine rejection; `any→rework` excludes
  terminal `Done`. (The spine can now express design-v2 §4's Confusions→Align without a branch-nuke.)
- **Append-only `event` table** — the JSONL export will be a history, not a state dump.
- **`required_gates_for(from,to,kind)` takes `kind` now** (ignored in slice 1) — per-kind gate profiles become a
  body change later, not a callsite churn (seam designed in).
**Slice-1 scope deliberately enforces only ONE gate** (`align→in_progress`, human). verify/review/land transitions
exist but ship **ungated stubs**; their real gates land with their slices.
**Known-holes (logged, not silently dropped):** repro-signal gate (bug-kind In-Progress entry, §4:109);
harden/`mutation_score` gate (verify slice); `review_approved`/`min_reviewers` — and whether `min_reviewers`
even applies to a single-operator harness (review slice); per-kind gate profiles (when non-build kinds land);
worktree-created assertion (enforced by the loop + pi-iso, slice 3/5); transitive blocked-cache + parent-down
inheritance (not needed for the ready predicate — see below).

## Trunk slice 3: spine wired into the agent loop — BUILT (2026-06-09)
**Context:** the agent loop ran on Phase 0's standalone `enum Phase {Align, Execute}` + `PhaseStore` (a global
binary toggle). The board (slice 1) is the real spine; the loop needed to consult it.
**Decision:** retire the toggle. The tool gate is now `agent::gate::mutating_allowed(status)` — mutating tools
unlock only in the execution band (`in_progress`..`land`); `todo`/`align`/`rework` are read-only, `done` is
finished. The human `/align` becomes the board's `criteria_confirmed` transition (`align→in_progress`), which —
because mutation is locked until `in_progress` — *transitively* gates mutating tools (the exact Phase-0 property,
now ticket-scoped and spine-native). Confirmed forks: **(a) post-gate-only run** — `agent run <id>` requires an
execution status and refuses otherwise (operator sets criteria via CLI; agent-drafts-plan-in-align deferred to
the workpad slice); **(b) force-tickets** — dropped the ad-hoc `run "<task>"` string form so every run is
board-scoped (the §12 "load-bearing on itself" bar). Tool policy lives in `agent`, NOT `board` (board stays
tool-agnostic — the headless-core protocol boundary). `agent align` refuses to confirm *empty* acceptance
criteria (gate by an artifact). Added thin `Board::set_plan`/`set_acceptance_criteria`/`max_ticket_seq`.
**Evidence:** 8 unit tests (gate maps over the whole spine; the human gate flips deny→allow exactly at
align→in_progress and rework re-locks), `cargo test --workspace` green, clippy clean, `Phase`/`PhaseStore`
grep-clean, and a **live oMLX run** (ticket → criteria → align → `run` fired `write_file` post-gate).
**Deferred (logged):** agent self-transitions (kept out — agent can't walk its own ticket past the human gate);
auto-driving in_progress→verify on loop completion (manual for now); rich workpad render + Confusions→Align (s2).

## Trunk slice 4: close the loop — Verify + Land gates — BUILT (2026-06-09)
**Context:** after s3 a ticket stopped at `in_progress`; the back half of the spine (`verify/review/land/done`)
had legal transitions but **no gates and no CLI** — `required_gates_for` returned `[]` past Align, and the
`validation` column was never used. The harness couldn't take a ticket to Done.
**Decision:** add two artifact-backed gates and the CLI to walk them. **`tests_green` (Machine)** on
`verify→review`: `agent verify` runs the ticket's validation command, records its exit status (output tail as the
note), advances on green or **bounces `verify→in_progress` on red** (the §4 verify-miss back-edge). **`landed`
(Human)** on `land→done`: the §10 keystone — `agent land` refuses a **dirty working tree** ("git commit = human
approval"), records HEAD sha as proof, advances `review→land→done`. **Confirmed fork:** land = "committed + green
on the current tree" — the real worktree **squash-merge to main** is deferred to s5 (needs per-ticket branches);
buildable now, dogfoods on this repo.
**Why the keystone holds:** an agent has only read/write/bash — it can `git commit`, but it cannot invoke
`agent land` (not in its toolset) nor write a Human gate row, so it **cannot self-advance to Done**; only the
operator lands. Same shape as `criteria_confirmed`. `tests_green` is Machine (an agent legitimately runs tests),
so `gate_source_required` maps `criteria_confirmed|landed → Human`, everything else → Machine.
**Boundary:** git operations live in `agent` (like tool policy), not `board` — the board stays VCS-agnostic, the
headless-core protocol boundary. Added `Board::set_validation`.
**Evidence:** 12 unit tests (board: verify/land gates enforced, machine source REJECTED for `landed`, rework
re-locks both via attempt-epoch; agent: green-advances / red-bounces, land refuses-dirty / lands-clean in a
throwaway git repo), `cargo test --workspace` green, clippy clean, and a **live oMLX full-spine run**
(new→align→run→verify→land→done; dirty tree refused then committed+landed; event trail
criteria_confirmed→tests_green→landed in order).
**Deferred (logged known-holes):** worktree-isolated squash-merge to main (s5); §7 Harden/mutation-score gate;
re-verify the *exact landed sha* (s4 checks tests-green-at-review + clean-tree, not that the committed delta was
re-tested); ticket-linked-commit verification (unambiguous once s5 gives each ticket a branch); real PR/CI watch
(no remote — collapses to local tests-green); repro-signal gate for bug-kind / per-kind profiles.

## Trunk slice 5: worktree-isolated workers + real squash-merge land — BUILT (2026-06-09)
**Context:** isolation was still the Phase-0 stub — `run_isolated` fabricated a throwaway `lower/SEED.txt` tree,
COW-cloned it, ran, diffed, and discarded; it never touched the real repo, made a branch, told the board, or
landed. And s4's `land` was a placeholder (clean-tree check on the *main* repo), with the real squash-merge
deferred here. This is the last trunk execution piece.
**Fork (confirmed by Gary): plain `git worktree`, NOT pi-iso.** The slice was originally tagged "pi-iso," but the
land-native path pulls the other way: land = squash-merge needs **branch/base control + a shared object store** so
merge-back is trivial, and pi-iso's lifecycle is built for run→extract-diff→discard (ephemeral, `git apply`
downstream), not branch-promote. So the worktree path is ~150 lines of plain `git` in a new `agent/src/git.rs`,
and land's squash-merge/cleanup is our own git regardless. **pi-iso is not discarded** — it remains a workspace
crate (the CoW content-isolation + diff primitive, and the non-git fallback) for when a workload actually needs
it; it was just dropped from `agent`'s deps. This is a choice *within* Strategy A (own-core), not a reversal of
"lift pi-iso."
**Decision:** `agent run` is **isolation-by-default** — it runs the loop in the ticket's worktree at a gitignored
`.harness/worktrees/<id>` on branch `harness/<id>` (both pure functions of the id → **no schema change**, and the
"worktree-created" gate becomes moot: `run` structurally guarantees it), then commits the work to the branch.
`verify` runs validation in the worktree. `land` squash-merges the branch → base as one commit (refusing a dirty
base, aborting+restoring on conflict, refusing an empty land), records the **squash sha** as the `landed`
artifact, then removes the worktree + branch. `rework` removes the worktree+branch (the §4 fresh-branch property).
The fake-seed `iso` command + `run_isolated` were removed.
**Keystone, stronger:** the squash commit on base IS the operator's approval. The agent commits only to its
*throwaway* branch (never base) and cannot run `agent land` nor write the Human `landed` gate → still can't
self-advance to Done.
**Evidence:** 17 unit tests (git.rs: worktree isolates writes then squash-lands as exactly one commit + cleanup;
rework removes worktree+branch; dirty-base refused. main.rs: `run_land` composition squash-merges, advances
review→land→done, removes worktree+branch), `cargo test --workspace` green, clippy clean, and a **live oMLX
worktree-isolated run** (hello.txt written in `harness/t1`, absent from main during the run; verify green in the
worktree; land squash-merged → main gained the file as one commit `702bca1`; worktree+branch gone).
**Deferred (logged known-holes):** the coordinator + claim/lease FSM + parallel dispatch + stall detection (the
execution-plane pillar — §13-P1 de-risked, not this slice); merge-conflict **auto**-resolution (s5 reports the
conflict and stops; operator resolves); memory capture from archived workpads on rework; repro-signal / per-kind
gates (carried from s1/s4).

## Trunk slice 2: the §5 workpad contract — BUILT (2026-06-09) — TRUNK COMPLETE
**Context:** the columns (plan/acceptance_criteria/validation/notes/confusions) existed but there was no canonical
render — `run_ticket` hand-built a *partial* workpad (title + Plan + Acceptance only; the agent never saw
Validation/Notes/Confusions), there was no human `show`, no notes/confusions setters, and the `InProgress→Align`
back-edge existed in the spine but nothing used it.
**Fork (confirmed by Gary): Notes/Confusions OVERWRITE, not append-with-timestamp.** Consistent with the existing
set_plan/criteria/validation setters; no schema change; the renderer + Confusions→Align bounce are the load-bearing
parts, append is a cheap later refinement if history turns out to matter.
**Decision:** one canonical renderer `board::render(&Ticket, header) -> String` (`board/src/workpad.rs`) — the §5
markdown: fenced header line then Plan / Acceptance Criteria / Validation / Notes / Confusions, **all five always
present** (empty → `_(none)_` so the contract shape never collapses and the agent always sees every heading). Pure,
no deps; lives in `board` (it IS the contract) with the **header injected** (`<host>:<abs-path>@<short-sha>`) so
board stays VCS/host-agnostic. Wired into BOTH `agent show <id>` (human view) and `run_ticket`'s system prompt
(via `build_system_prompt`) — single source of truth, the agent and operator never see different versions. Added
`Board::set_notes`/`set_confusions` + CLI `note`/`confusion`/`show`; `git::short_sha` for the header; host via
`hostname`, path = worktree if present else repo root.
**Confusions→Align (§5's "our addition"):** `raise_confusion` records the text and, if `in_progress`, bounces
InProgress→Align — re-locking the tool gate (mutating tools denied in Align), parking the ticket until the operator
re-runs `agent align`. **Why no attempt-epoch bump or row deletion:** the *only* path that advances Align→InProgress
is the operator's `agent align` (the agent loop refuses to run unless already in an execution status), so the ticket
genuinely can't resume without a human acting — the stale `criteria_confirmed` is harmless. Wu Wei: don't add epoch
machinery the park-at-Align already enforces.
**Evidence:** 21 unit tests (board: render — all 5 sections in order + header + 3 placeholders; notes/confusions
persist + audit ×2. agent: confusion bounces in_progress→align and flips `gate_allows(write_file)` to false / todo
just records; system_prompt embeds the rendered workpad incl. all 5 sections + header), `cargo test --workspace`
green, clippy clean, **live `agent show`** (full §5 render, header `McBob.local:/private/tmp/…@ecd7bdc`) and the
**confusion bounce** (in_progress→align; `show` reflects status + recorded text).
**Deferred (logged known-holes):** reconcile-first-on-entry (a Verify/Review behavior); **agent-raised** confusions
(needs a 4th tool / structured stop — the loop's tools are read/write/bash only, so confusions are CLI-raised for
now); rich Obsidian card render; Notes append-with-timestamp.
**Trunk status: COMPLETE (s1–s5).** Next is the pillars (memory / Harden gate / parallel exploration /
self-evolution), per design-v2 §12.

---

## Parallel exploration (P4 / §8) — design + adversarial review DONE, scoped to a PROBE (2026-06-10)
**Design** (research/19): a coordinator that fans out N *diverse* workers across isolated worktrees, prunes by
the pre-committed Align criteria (objective gate first, zero judge cost), and selects a winner — built on the
existing primitives (`run_worktree`, `runs` telemetry, board gates, memory sidecar). No board/schema change:
the `Run` model is already N-runs-per-ticket; the N branches live in the ephemeral coordinator plane (design-v2
§3), not on the spine.

**Adversarial review** (3 decorrelated cold subagents in parallel — dogfooding the subagent pattern on our own
review rule) returned a decisive verdict: **§3–§6 over-built a cathedral for an exercise-only output. Build the
PROBE, not the cathedral.** Folded in (research/19 §9):
- **BLOCKER (R2):** branch-move consolidation to `harness/<ticket>` silently targets the wrong tree
  (`worktree_or_cwd` keys path on canonical id; the worktree stays at `-wK`). → **don't auto-consolidate;**
  report winner's worktree path + branch id for manual human verify/land.
- **R2:** `run_ticket`'s single `id` conflates board-id and worktree-id; `mint_run_id` same-ms collision for N
  workers. → thread a worker label into the run_id, ticket_id stays the board ticket.
- **R2:** `ensure_worktree` silently reuses a crashed/contaminated worktree. → **reap-before-fanout.**
- **R2:** "shared-git safe by construction" is FALSE (`bash` is unconfined, tools.rs:79–82). → retract the
  false safety claim; sequential workers make it a non-issue for the probe; document the real boundary.
- **R1:** premature reaping destroys the evidence the probe exists to produce. → **reap nothing;** report a
  ranked table over surviving worktrees.
- **R1:** the pairwise-knockout judge is over-built and adds ordering noise. → **drop the judge;** rank
  survivors by objective signal only (passed, then diffstat/iters).
- **R1:** the distinct-descriptor "diversity gate" is a vibe wearing a number's clothes; planner-trait is
  premature. → strategies from a fixed list / `--strategy` flags (no planner oMLX call); diversity is read off
  the diffstat table (did branches do different things?), not a string check.
- **R3:** synthesis — explore never lands, so auto-consolidation/judge/synthesis/concurrency are all
  speculative machinery around an output a human reads. The lean probe satisfies all three reviewers at once.

**The probe** (`agent explore <ticket> [--fanout N] [--strategy S ...]`): reap stale `<ticket>-w*` → N =
`fanout_for(stakes, override)` ceiling 5 → sequential workers (worktree `<ticket>-w<k>`, worker-label run_id,
strategy-in-prompt, validation in tree) → rank survivors by objective signal → killed branches →
`remember(lesson)` post-mortem → print ranked table + winner's path/branch. **Reaps nothing, consolidates
nothing, never lands** (oMLX exercise). Pure seams (the gate): `fanout_for`, `rank_outcomes`. Git seam:
worktree fan-out lifecycle (integration-tested w/ stub worker). Exercise: one live run produces ≥2 branches
that do different things + ≥1 post-mortem, captured as evidence.

**Deferred (gated on probe evidence, NOT built):** planner-trait + diversity gate · pairwise/round-robin judge
· auto-consolidation (done correctly w/ `git worktree move`) · concurrency · GoT synthesis ·
effort-without-progress detection · token-budget reallocation. Each fenced behind a specific probe finding.

---

## Self-evolution (P5 / §9) — design + adv review at design AND pre-land DONE, scoped to Half A only (2026-06-10)
**The finding that defined the pillar:** the harness has an **open compounding loop** — it *writes* `lesson`
episodes (explore's reflection-on-kill) but **never reads them back**; `build_system_prompt` was built purely
from the ticket workpad. Closing that loop safely IS self-evolution. The amendable substrate is **case-law**
(agent-distilled heuristics, human-approved) sitting under an immutable **constitution** (spine, integrity
constraints, gate-enforcement — human-only), grounded in research/04 (Constitutional AI, STOP, Gödel Agent).

**Design-stage adversarial review** (3 parallel reviewers, diverse lenses — required: a weakened gate is silent
+ catastrophic) **converged from three directions** that the planned Half B (`agent reflect` instrument + oMLX
two-column distiller + K=3 critic panel + proposals dir + 6-function pure filter core) should NOT be built:
- **Wu-Wei — the pond is empty *and* mis-stocked:** the only `lesson` writer emits ticket-scoped, bound-specific
  post-mortems (worker ids, digits, literal commands); every one trips the generalization filter → **zero**
  general lessons to distill. The exercise had to hand-seed fakes — the instrument was ahead of its data.
  Claude-in-the-loop already distills better than an oMLX pass.
- **Unsafe as specified:** screen ran on the raw lesson text but the human approves the *distill output* →
  injection sink; the "≥2 episodes" floor is self-dealing (cleared by the agent's own auto-authored post-mortems;
  research/04 wanted a baseline the agent *didn't* author).
- **Incorrect:** `has_bound_specifics`/`is_vacuous` regexes mis-verdict ~100% of real input both ways; the
  5-const keyword gate-bypass detector is *false safety* (any paraphrase / no-gate-named integrity attack walks
  past a `gate_bypass:false` banner — worse than no check).

**Decision: ship Half A only, hardened; defer Half B to a measured trigger.** Half A = **the loop reads the
committed, human-approved `wiki/case-law.md` and injects it into every worker's system prompt** — the entire
irreversible compounding payoff, ~20 lines. Third pillar cut to its load-bearing core (memory=Slice A,
explore=probe, evolve=read-side). Built:
- `crates/agent/src/evolve.rs` (pure, no git/model/clock): `mentions_gate_weakening` (flag/drop-only — requires
  BOTH a gate reference AND a weakening verb; honestly NOT a "safe" certifier, human commit is the boundary);
  `prepare_caselaw` (**section-aware** — only `## Lessons` bullets; drop gate-weakening; budget 12;
  **screen-before-budget**). `load_case_law` reads from **repo ROOT** (not cwd — workers run in worktrees),
  **missing→stderr note / empty→silent**, reports `injected N/M` + echoes dropped texts. `build_system_prompt`
  gained a `case_law` arg → `### Learned heuristics (case-law)` after the workpad (contract stays primary).
- `wiki/case-law.md`: human-approve-contract header + 8 genuinely-general hand-distilled lessons.

**Pre-land adversarial review** (3 reviewers: security / correctness+Wu-Wei / contract) = **unanimous SHIP, zero
BLOCKER/MAJOR**. Verified the two load-bearing properties (read resolves against repo root; screen strictly
flag/drop). Folded in the one convergent finding: the screen over-drops *gate-describing* legitimate lessons →
now echoes each dropped bullet so a curator can reword (silent over-drop → visible). **Evidence:** agent 41 /
board 20 / pi-iso 2 green, clippy clean, exercise rendered the real committed case-law into a worker prompt (8
lessons in, planted gate-weakening line dropped).

**Deferred (gated on the §9.1 trigger — ≥~10 general lessons + curation becomes toil; NOT built):** `agent
reflect`, the oMLX two-column distiller, the K=3 critic panel, the proposals dir, the full 6-function filter
core. When built, the §9.2 fixes are mandatory (screen the oMLX *output*; require distinct-provenance citations;
flag-only gate-bypass detector behind the critic panel with a persisted counter; lead the artifact with raw
cited episodes; read bodies via a non-mutating path; pass `now` into any ranker). Also non-blocking: a per-bullet
byte cap on injected lessons. **All four pillars now complete.**

---

## Local-model verbosity — default-think-OFF wiring BUILT (research/23) (2026-06-10)
**Context:** the verbosity research (research/23) verified oMLX's per-request reasoning knobs and recommended
**reactive escalation: default every turn to think-OFF, escalate only on a detected failure** — NOT a predictive
effort-router, NO graded `thinking_budget` ladder. The §7 residual measurement then settled the last open question
(think-OFF silently fails ~30% on generative code, but naive think-ON is unusable — `max_tokens` does NOT bound
`reasoning_content` — and **the downstream `tests_green` artifact gate is the real catch**), unblocking the wiring.
**Decision (shipped — the default half only):** add `think: bool` to the normalized `provider::Request`. The agent
loop sets it **false** on every Request. The Qwen-specific wire mapping is **quarantined in the oMLX provider**:
`OpenAiProvider::request_to_body` (extracted from `complete` so the wire shape is unit-testable without a live
server) emits `chat_template_kwargs:{enable_thinking: req.think}` — inference control on the request, NOT oMLX
server/profile config (which stays in the dashboard). anthropic.rs ignores `think` (its default is no extended
thinking ≈ think:false). **Evidence:** build + `clippy --workspace --all-targets` clean; suite green (agent 58/1-ign,
board 20, pi-iso 2, provider 2 incl. new `body_carries_enable_thinking` asserting `enable_thinking` carries both
bools faithfully).
**Why `think: bool` and not a `thinking_budget: Option<u32>` ladder:** research/23's empirical matrix reproduced the
**token-elasticity trap** — a tight-but-nonzero budget just relocates the spiral from `reasoning_content` into
`content`, saving nothing; only the binary extremes (think-off vs think-on) are clean. A bool is the honest shape of
the only lever that works. A graded ladder would be machinery around a non-effect.
**Why reactive escalation is documented but NOT built (Wu Wei):** the universal trigger to flip `think→true`
(`stop_reason==Length` / loopgate strike / gate red) is model-agnostic, but the §7 finding is that the **downstream
`tests_green` gate already catches** a silently-wrong artifact at `verify` — a silently-wrong `write_file` fails the
gate, it doesn't ship. So escalation is a latency/quality optimization, not a safety requirement; defer it until a
number shows the wasted first attempts cost more than the gate catch. When built, it **must be `thinking_budget`-
bounded** (naive think-ON ran to 8091 tokens / 830 s).
**Rejected approach — a predictive "effort-router" LLM call before each turn (Gary's original two-stage idea):**
shelved. A triage call cannot see a *content-level near-miss* (the 30% silent-fail mode is a 22–24/25 artifact with
no loop and a clean `stop`) any better than a reactive trigger can — and it pays a full extra call every turn for a
prediction the downstream gate makes moot. Reactive (default-OFF, escalate on a detected failure) is strictly
cheaper: think-OFF is so cheap a wasted first attempt beats a per-turn triage call.
**SUPERSEDED IN POSTURE (2026-06-10, see "Working-assumption flip" below):** the §6 default-think-OFF wiring and the
shelved effort-router are both reopened — NOT because the evidence changed (it didn't; the findings here all stand)
but because Gary changed the *objective* from cost/safety to output-quality-under-assumed-capability. The reactive
escalation here was the right answer to "minimize cost, the gate is the catch"; the question is now "maximize quality,
assume the model can." The §7 facts (`max_tokens` doesn't bound reasoning → a bound is mandatory; the token-elasticity
trap → a *fixed tight* budget is bad) become **design constraints on the new bounded-thinking scheme**, not arguments
against thinking.

---

## Working-assumption flip — assume the model is CAPABLE; attack context engineering + bounded thinking (2026-06-10)
**Context:** the four-failure-mode guardrail spine is complete (refeed/loopgate/[deferred collapse]/planexec), but the
honest planexec finding was decisive: guardrails ≠ capability. planexec's only live firing was a **false positive**
(run-3 was an already-satisfied task; the model correctly did nothing, but `acted = git::is_dirty` cannot tell
"correctly did nothing" from "stalled"), and the nudge had **~zero conversion** (the 35B re-asserted completion). The
35B's real failure texture is **thrash** (act-without-progress: max_iters, looped), not clean plan-then-stop. The
evidence points at a capability gap.
**Gary's call (verbatim intent):** "I know the evidences point to capability gap … but since this is a research project
for personal usage, let's all assume the model can for the sake of moving forward." → adopt a **working assumption**,
explicitly chosen (not evidence-backed), as the posture for everything downstream.
**Decision 1 — the capability assumption.** Treat the local model as **capable of the work given proper context
engineering**. Reclassify failures: a stall/thrash is a **context-engineering failure first** (task not decomposed
atomically enough; the model was handed more than it could hold in one window), not a capability ceiling — until a
context-engineered retry also fails. This is a *posture for moving forward on a personal research project*, recorded
as such so it is never mistaken for a measured result. (Integrity: the research/23 + planexec evidence is NOT
overwritten or softened; it stands. We are choosing to *act past* it, not to *deny* it.)
**Decision 2 — context engineering gets its own research project (research/25), not an immediate build.** The honest
diagnosis of the current re-feed path (grounded in `main.rs:314–490` + `refeed.rs`): the loop feeds the **whole
unpruned transcript** every turn (`messages` is push-only, never curated) and the *only* interference is a crude
**per-message byte cap** (`refeed::cap`: head ⅔ + tail ⅓ + elision marker; assistant prose → `REFEED_TEXT_CAP` 1024 B,
tool results → `REFEED_TOOL_CAP` 4096 B; **tool_call arguments UNCAPPED** — a `write_file` body rides there at full
size; **reasoning recorded but never re-fed**). That is a 32K-window survival hack, **not** semantic context
engineering — no atomic task scoping, no relevance selection, no working-set curation. The research project surveys
the literature + forums + how other frameworks do it (**Hermes Agent, oh-my-pi, OpenCode, …**) before choosing a
lever. Candidate levers (NOT yet decided — output of the research): atomic task decomposition at the board/ticket
level vs curated re-feed at the loop level vs both, sequenced.
**Decision 3 — thinking is assumed helpful, allow it BOUNDED; mechanism = fixed preset-ladder budget (SHIPPED
2026-06-10).** Reverses the §6 default-think-OFF posture (objective change, see SUPERSEDED note above). Working
assumption: the think block is computation — more deliberation → better output, within a limit.

> **AMENDED 2026-06-10 — the mechanism is the preset ladder, NOT the self-assessed triage.** This decision originally
> recorded Gary's adaptive *self-assessed budget* design (a cheap think-OFF JSON pre-step where the model estimates its
> own budget, then re-calls at that estimate). The research/26 adversarial review (4 cold reviewers, convergent) **cut
> that triage**: under a quality objective, routing a task *down* to a smaller budget is pure downside, and
> effort-self-prediction is the same unreliable self-prediction research/23 measured. The self-assessed triage is now
> in **Rejected Approaches**. What ships instead ↓.

**Mechanism (shipped):** the harness picks one rung from Gary's **preset ladder — low/med/high = 1024/2048/4096** — as a
**soft** `thinking_budget` target, with `max_tokens` as the hard backstop. No triage, no JSON schema pre-step, no new
`StopReason`, no exhaustion-signal plumbing. Verified against oMLX (research/26 §9.1): the budget is a soft target (the
model can run slightly over/under and wraps up gracefully), there is no exhaustion signal (reasoning ends → answer →
`finish_reason:stop`), and bounded think-ON is loop-safe. **Prereq (server-side, research/26 §9):** the served model
needs `thinking_budget_enabled:true` + `reasoning_parser:"qwen_3_5"` in `~/.omlx/model_settings.json` (read live; both
now on). **Wire (shipped):** `provider::Request.think: bool → think_budget: Option<u32>` — `None` →
`enable_thinking:false`; `Some(n)` → `enable_thinking:true` + **flat top-level** `thinking_budget:n`.
**"Who picks the rung" — RESOLVED: fixed per-turn default, env-overridable** (`DEFAULT_THINK_BUDGET=2048`, override
`HARNESS_THINK_BUDGET`; `0`→OFF). Simplest thing that ships the posture; reconfigurable without recompile; commits to
nothing that blocks the two *additive* follow-ons left open — **caller/ticket-tagged rung** (needs a board field; defer
until a ticket kind wants a different rung) and **reactive escalation** (bump on Length/loop/gate-red; deferred to
observe fixed-rung telemetry first). **Behavior change:** committed default flips from think-OFF (research/23) to
bounded think-ON @ medium=2048 — the intended posture shift, settled by the §7 review + §9.1 verification, not a silent
change.
**Why the prior posture lost (and why this isn't flip-flopping):** research/23's "think: bool, default-OFF, no graded
budget" was the correct answer to *minimize cost; the downstream `tests_green` gate is the safety net*. Under the new
objective — *assume capable, maximize quality* — a wasted-cost argument no longer dominates, and the elasticity trap
(which killed a *fixed* ladder) does not apply to a *self-sized* budget. Same evidence, different objective, different
answer. Both are recorded; the switch is dated and attributed.

**Decision 2 — RESOLVED by the research/25 survey (2026-06-10).** The survey (6 sources: oh-my-pi + pi-mono direct
read; OpenCode, Hermes/Nous, Claude Code + Aider + literature web workers) settled the lever. **Chosen re-feed
design = the industry-convergent mechanism, NOT a semantic curator:** **protect head+tail (position-aware, motivated
by Lost-in-the-Middle's ~30% mid-context degradation) · structured 7-section summary of the middle · offload/clear
tool output · cap both args and results.** The 7-section handoff (Goal/Constraints/Progress/Decisions/NextSteps/
RelevantFiles/CriticalContext) is confirmed across **4 independent codebases**. **Hard invariant (from the §5 adv
review, silent-failure class): a context mechanism may be lossy but NEVER silently lossy** — summary-overflow ⇒ abort
+ signal; clear only refetchable data; anti-thrash breaker must escalate not give up; assembler asserts
never-split-call-from-result. **Both candidate levers are kept and sequenced:** atomic task decomposition (board/ticket
level — literature C6, backs Gary's own reframe) AND curated re-feed (loop level — F1–F5 in research/25 §4.2),
built as 4 slices (F1 protect-head → F4 cap refetchable args → F2 compaction+summary → F3+F5 offload+file-list),
**AFTER the bounded-thinking design**. Decision 3's reasoning-refeed reconciles cleanly (§4.3): "let it think, bound
the budget, record-but-don't-re-feed" — production axis ⟂ re-feed axis.

---

## Rejected Approaches

### Board close-path alternatives — REJECTED (`research/32`) (2026-06-11)
**Context:** giving non-landing / abandoned tickets a path to a terminal (Fix B). Three shapes were weighed against
the recommended one-edge + one-gate package.
**`Cancelled` as a distinct terminal (now) — REJECTED.** A separate terminal would cleanly split "succeeded" from
"abandoned", but it costs a **table-rebuild migration** (the `ticket.status` CHECK constraint, schema.rs) **and** a
`ready()` blocker-semantics fix (board.rs `b.status != 'done'` would strand dependents of a *cancelled* blocker).
Not earned for a personal harness — the disposition lives in the resolution note + the auditable absence of a
`landed` gate row. Revisit only if a reporting need actually splits Done/Cancelled.
**Wildcard `any-non-terminal → Done` gated by GATE_RESOLVED — REJECTED.** Smallest code, but it moves the
"no structural skip to a terminal" guarantee from the **transition map** to a **gate** — breaking the `lib.rs`
`validate_transition(Todo,Done) is Err` invariant in spirit (a forgotten/miswired gate would re-open Todo→Done).
Keep the protection structural: add exactly one edge (`Review→Done`).
**`close` refuses code kinds — REJECTED.** Tidy ("code must land"), but it would leave t3 (a code ticket abandoned
mid-flight) with no FSM disposal. Abandonment is kind-agnostic; the success-vs-abandon distinction is the note +
the `landed`-row presence, not the kind. Instead the `close` verb *guards* code kinds behind an explicit
`--abandon` flag (refuse by default, allow with intent) — keeps the "code should land" nudge without the dead end.

### Predictive self-estimating thinking-budget triage — REJECTED (`research/26` §7–§8) (2026-06-10)
**Why considered:** Gary's original Decision 3 mechanism — a cheap think-OFF, JSON-schema pre-step where the model
self-assesses how many reasoning tokens the task needs, then the real call runs at that estimate. Intuitively appealing
("right-size each task, dodge the elasticity trap of a fixed-tight budget").
**Why rejected:** the research/26 adversarial review (4 cold reviewers, convergent) found it downside-only under a
**quality** objective. Routing a task *down* to a smaller budget can only hurt output; routing *up* is already covered
by just setting a generous fixed budget — so the triage's one degree of freedom (predict-then-shrink) is pure risk. And
the prediction itself is the **same unreliable self-prediction research/23 measured** (a 35B estimating its own future
effort). It also adds a per-turn extra call, a schema, and a silent-failure class (a bad estimate quietly degrades
quality). **What ships instead:** a fixed preset-ladder budget (low/med/high = 1024/2048/4096), one rung per run,
env-overridable — see Decision 3 (amended). Revisit only if fixed-rung telemetry shows a real need to vary per task,
and even then prefer caller/ticket-tagged rungs (declared, not predicted) over model self-estimation.

### Semantic working-set curation for re-feed — REJECTED (`research/25` §4.1) (2026-06-10)
**Why considered:** the intuitive "context engineering" goal is to *select the relevant subset* of history each turn —
a model- or embedding-driven curator that keeps only what matters. Decision 2 originally listed "curated re-feed" as a
candidate lever.
**Why rejected:** the survey found **no framework does this** — oh-my-pi, pi-mono, OpenCode, and Hermes all use
**bounded recency + structured summary + offload**, keyed off token/byte budgets, with **zero semantic selection**
(both the OpenCode and Hermes workers flagged this explicitly as an unsolved problem). Cognition's "don't build
multi-agents" warns the opposite direction (share *more* trace, not less). Chasing a semantic curator the SOTA lacks
violates Wu Wei (building a research problem into the critical path) and the integrity rule against inventing
capability we can't verify. **The chosen design is the mechanical convergent answer** (head+tail+summary+offload),
which the Lost-in-the-Middle result independently justifies. Revisit only if the mechanical design is *measured*
insufficient on a real long task.

### Re-feeding reasoning / CoT into context — REJECTED (`research/25` §4.3, corroborates `research/23`) (2026-06-10)
**Why considered:** Decision 3 assumes thinking helps quality, which could suggest carrying the thinking forward.
**Why rejected:** three independent sources DROP reasoning before re-feed — Hermes `keep_cots=False` (default;
`True` is experiment-only), OpenCode strips on compaction + re-feeds only same-provider (signed-thinking), Claude Code
has a dedicated `clear_thinking` primitive. Re-feeding CoT burns the window for no gain (local Qwen has no
signed-thinking chain to preserve). The reconciliation: thinking helps **on the turn that produces the output**
(bound the budget there); it does **not** help re-fed. Record-but-never-refeed stands — and is now what we already do.

### Notes/Confusions append-with-timestamp (slice 2)
**Why considered:** §5 frames Notes as "timestamped milestones / reproduction signal / sync evidence" — naturally
append-oriented, and append preserves history across a ticket's life.
**Why rejected (for now):** more code (read-modify-write + a deterministic timestamp source) and a divergence from
how plan/criteria/validation are set, for value the trunk doesn't need yet. Chose overwrite (consistent with the
other setters, no schema change). Cheap to revisit if losing note history bites.

### A separate "rich card" workpad renderer per consumer (slice 2)
**Why rejected:** two renderers (one for the human `show`, one for the agent prompt) is exactly the drift the §5
"one workpad, reconcile-first" contract exists to prevent. One `render()` feeds both; the only per-consumer
variation is the injected header string.

### Surface-feature regexes as safety/quality *verdicts* (self-evolution Half B core)
**Why considered:** a pure, deterministic, unit-testable filter (`has_bound_specifics`, `is_vacuous`,
`mentions_gate_weakening` as a certifier) to decide which distilled lessons are safe/general enough to keep —
"gate by an artifact, not a vibe."
**Why rejected:** keyword/regex predicates mis-verdict real natural language ~100% both ways — they reject good
input (a general lesson with a digit) *and* launder bad input (a dense platitude stamped "not vacuous"). A
`gate_bypass:false` banner is **worse than no check** (false safety). Generalization/vacuity/safety are
LLM-or-human judgments, never code verdicts. The **one** predicate that survives is `mentions_gate_weakening` and
only as **flag/drop, never certify-safe** — it removes obvious matches but the real boundary is human commit.

### Auto-distilled self-evolution before a stocked pond (Half B, premature)
**Why considered:** an `agent reflect` instrument + oMLX two-column distiller + critic panel to turn `lesson`
episodes into proposed case-law automatically — the "compounds at ~zero cost on local models" vision.
**Why rejected (deferred):** there were **zero** general lessons to distill (the only writer emits bound-specific
post-mortems), the grounding floor was self-dealing (cleared by the agent's own auto-authored episodes), and the
screen guarded the wrong text (raw lesson, not the distill output the human approves). Build it at the §9.1
trigger (≥~10 general lessons + curation toil), with the §9.2 fixes mandatory. Until then, Claude-in-the-loop
distills lessons into `wiki/case-law.md` by hand — better quality, no machinery.

### claude-mem as-is
**Why considered:** capture→compress→inject memory for coding agents.
**Why rejected:** runs an API call per observation → cost grows with memory. We move compression/embeddings
to local oMLX. (Note: local fixes *compression* cost, not *injection* cost — retrieval discipline still required.)

### Whole-store memory injection
**Why rejected:** every recalled token competes with task reasoning. Top-k + progressive disclosure + project
scoping instead.

### Coverage as the test-quality gate
**Why rejected:** 100% line coverage with zero meaningful assertions is trivial. Meta ACH data: 49% of
fault-catching tests added zero line coverage. Mutation-gating instead.

### Symphony's full-autonomy posture
**Why considered:** Symphony is our closest work-model blueprint.
**Why rejected:** `approval_policy: never` / "user-input = hard failure" is the opposite of our gated-interactive
model. We borrow its structure (board/workpad/state-machine/land) but invert the human-gate valence.

### openlumara as a memory source
**Why rejected:** flat MessagePack save/recall, no embeddings or top-k — below our P3 bar. Reference its
module-toggle ergonomics at most.

### llm-council as a pillar (multi-model deliberation for ticket generation) — REJECTED (shelved)
**Why considered:** Karpathy's `llm-council` (N models answer → anonymized cross-review/rank → a chairman
synthesizes); Gary asked if it's needed to "simulate team dynamic" when generating complementary BE/FE tickets.
**Why rejected:** complementary BE/FE generation is a **decomposition + shared-contract** problem, not a
deliberation/consensus one. Coherence comes from a **shared interface artifact** (a grounding ticket both BE
and FE tickets depend on via `edge`) + optional role-specialized workers negotiating it through the inherited
**agent-mail thread** pattern — not a vote. llm-council's useful kernel (cross-review to decorrelate blind
spots) is **already present** twice: the Harden gate (mutator on a *different* provider) and P4's
debate/reconcile synthesis round (design-v2 §8). Kept on the shelf as an optional upgrade to P4 synthesis *if*
single-model reconcile proves weak — not a foundation pillar.

### Recursive CTE for the ready/blocked query — REJECTED (slice 1)
**Why considered:** my own slice-1 recommendation — `br` only used app-side Rust BFS because **fsqlite can't run
recursive CTEs**; we're on `rusqlite` (real bundled SQLite 3) which runs them fine, so DR2's original "port the
CTE ready-query" looked back on the table.
**Why rejected (adversarial review C2, then simplified further on build):** (1) real SQLite still **infinite-recurses
on a dependency cycle** — it does no cycle detection — and `br` rewrote to BFS for *cycle-correctness*, not only
the engine limitation; (2) one CTE can't cleanly express both graph directions (transitive depends-on **+**
parent-with-open-children). **Then the deeper realisation:** the *ready predicate needs no recursion at all* — a
ticket is ready iff its **direct** blockers are terminal; a blocker that is itself blocked is simply still
non-terminal, so the direct check already excludes the dependent, and cycles can't hang **by construction**.
Shipped as a single non-recursive `NOT EXISTS` over direct blocking edges. The transitive blocked-cache +
parent-down inheritance (`br`'s full computation) is deferred — not needed for dispatch.

### oh-my-pi as the base (Strategy B) — REJECTED
**Why considered:** omp is a maintained superset of pi-mono with a ~27k-LoC Rust core; looked like it'd hand us
~60% of the Execution + Memory planes for free.
**Why rejected (DR1, `research/11`):** the Rust core is **leaf systems-primitives only** — the agent loop, tool
execution, providers, and the gate are 100% TypeScript. Building on omp buys none of the hard parts (we'd
reimplement them under a Rust core anyway) and couples us to a solo fork. We **lift only `crates/pi-iso`**
(cleanly severable) and own the rest.

### beads / beads_rust (`br`) / repowise as runtime dependencies — REJECTED
**Why considered:** beads + br = purpose-built agent boards; repowise = code-health intelligence.
**Why rejected (DR2 `research/12`, DR2-prime `research/15`):** beads is Go + Dolt; **`br` is welded to `fsqlite`**
(single-maintainer *alpha* pure-Rust SQLite) + pinned nightly + ~180k LoC we'd use ~10% of; repowise is AGPL.
Under a Rust-native core we **steal the designs** — port br's `close_policy.rs` gate engine + its simpler edge
table onto our own `rusqlite` board — and depend on none. (br's gate engine corrected DR2's assumption that
beads had no state machine: br has one, already built + tested.)

### Verbose-local-model handling: bound the re-feed, don't strip tags (RESOLVED, `research/21`)
**Context:** the served local 35B is verbose; its reasoning silently re-fills the 32K context every loop turn and
threatened to overflow Claude at review time. First plan: a `strip_think` pure fn (record raw / feed stripped) +
`agent trajectory --think`.
**Decision (after 3-lens adv review):** the leak is real but the planned fix was wrong. **Bound the re-feed**
(`crates/agent/src/refeed.rs`: `cap` head+tail with an honest elision marker; record raw to telemetry, push capped
into `messages`; `REFEED_TEXT_CAP=1024`, `REFEED_TOOL_CAP=4096`) + an **`[ctx]` overflow signal** per turn +
**`agent trajectory --full`** for audit. Budget bumped **env-only** (`HARNESS_MAX_TOKENS`/`HARNESS_MAX_ITERS`), oMLX
context-window setting untouched. Model-agnostic, test-pinned (48 tests, clippy clean).
**Surfaced, NOT built (diagnosis discipline — gate by the number):** (1) a repetition circuit-breaker / lower per-turn
cap for the newly-observed **reasoning-loop collapse** failure mode; (2) capping tool_call **arguments** in the
re-feed (the discovered gap — write_file content rides there, uncapped); (3) raising the recorder's `FIELD_CAP` or
adding a repetition summary so the JSONL *adjudicates* a collapse (today only `run.log` shows it). All deferred until a
number demands them. *(Update: the loop gate below addresses the **cross-turn tool-call cycle** axis — not the
intra-turn collapse, which stays deferred.)*

### Loop gate: nudge-then-stop on identical consecutive tool calls (RESOLVED, `research/22`)
**Context:** the canonical "agent stuck in a loop" — a model calling the same tool with the same arguments turn after
turn — runs to `max_iters` (default 8; 40 in the §21 exercise) with a misleading `max_iters` label and no recovery.
The §21 discussion also raised whether building for the local 35B is tunnel vision.
**Decision (after necessity-first adv review):** build a **Tier-1 deterministic loop gate**, keyed strictly off
**universal** signals so it is model-agnostic. `crates/agent/src/loopgate.rs` (pure, mirrors `refeed.rs`):
`signature(&[ToolCall])` fingerprints a turn's calls; `LoopGate::observe` flags a **byte-identical consecutive**
signature as a non-progress strike — **strike 1 nudges** (a `[harness control]` message giving one recovery turn),
**strike 2 stops** with a new `looped` outcome label. Strikes are run-level (robust to cycle-switching). Wired into
`run_ticket` (Stop breaks before re-executing; Nudge dispatches tools then appends the nudge, preserving the
call→result invariant). Over `max_iters` this adds earlier stop + a recovery path + an honest failure label.
**The discipline test (why this is NOT tunnel vision):** *is the fix keyed off a universal signal or a provider
quirk?* The gate reads only `resp.tool_calls` + `resp.stop_reason` (both format-agnostic) — identical behaviour on the
anthropic provider. Provider quirks (think tags, `reasoning_content`, SSE) stay quarantined in the provider layer.
**Surfaced, NOT built (Wu Wei, `research/22` §8):** intra-turn text-repetition detection (axis B — the §21 collapse;
already self-terminates on `Length`, no consumer for a finer label yet); streaming in-flight abort (Tier 2, needs SSE);
period-2 window / no-progress detection (deferred until observed); provider-injection refactor to unit-test the wiring
(detector is pure-tested; wiring is thin + reviewed). Evidence: agent 56 tests / clippy clean / build clean.

### Plan→execute nudge: one nudge then `stalled` on a no-action stop (RESOLVED, `research/24`)
**Context:** the **fourth** local-model failure mode (the diagnostic spine's last gap) — the model emits a plan or
analysis in prose, calls **no tool**, and the loop's natural-stop branch banks it `completed` (Telemetry Unit B wrote a
correct plan and **0 files**). Disjoint sibling of the loop gate: loopgate catches "same tool forever" in the ToolCalls
branch; this catches "no tool at all" in the no-tool-call branch — two counters, no shared state.
**Decision (after a light 2-reviewer adv review, §10):** `crates/agent/src/planexec.rs` (pure, mirrors `loopgate.rs`):
`verdict(acted, code, nudged) → Accept|Nudge|Stall`. On a natural stop of a **code kind** that **changed nothing**:
strike 1 injects one `[harness control]` plan→execute nudge (giving one recovery turn), strike 2 accepts the exit but
labels it **`stalled`** (an honest failure), never `completed`. Non-code kinds and any run that changed the tree are
exempt (`Accept`). The 1-nudge bound is monotone → termination guaranteed; `max_iters` backstops.
**The BLOCKER fix (reviewer B) — `acted` is worktree-dirty, NOT tool-based.** The original scope set `acted` = "a
mutating tool ran", but the system prompt orders the model to start with `bash ls -R`, so a tool-based proxy is true on
**every** run and the gate would never fire. `acted` = **`git status --porcelain` non-empty at the natural stop** (new
`git::is_dirty`): base-free, tool-agnostic, catches an untracked fresh `write_file` (which `git diff --quiet` misses)
and correctly reads clean on a write-then-revert net-no-op. `lines_changed_against` is the wrong call mid-loop (diffs
`base...HEAD`; the work is uncommitted → reads 0). The pure `verdict` core is unchanged; only the *source* of `acted`
moved.
**Reviewer A split — ship the label, gate the nudge on a number.** The `stalled` *label* pays its way on telemetry
honesty alone (a do-nothing run must not poison the signal as `completed`); the *nudge* is speculative. Built both;
made the live exercise a real Wu-Wei gate. **Live finding: the nudge did NOT convert to action** (the 35B insisted the
work was complete rather than acting → conversion ≈0 in the observed runs) — so the **label is the value, the nudge is
on probation** (kept for now: one bounded cheap turn; cut if a larger sample confirms ≈0). Label plumbing: `stop_reason`
is free-form TEXT → **no migration**; forced at the call site like `looped` (`if looped{…} else if stalled{"stalled"}
else classify_stop(…)`, looped wins); `rank_outcomes` already sinks it (passed=false, lines=0).
**Evidence:** agent 67 tests/1-ignored (+9: 5 unit matrix + 4 `ScriptedProvider` integration incl. the recovery-trap
pin), board 20 / pi-iso 2 / provider 1-ignored; build + clippy clean. Live oMLX run-3 produced a real `stalled` row.
**This completes the four-failure-mode spine:** re-feed bloat (`refeed.rs`), cross-turn tool loop (`loopgate.rs`),
intra-turn collapse (deferred — self-terminates on `Length`), plan-then-stop (`planexec.rs`).

### Loop gate via recent-window or activation monitoring — REJECTED (`research/22` §5–6)
**Why considered:** a recent-window signature match (catches period-2 A,B,A,B) or white-box activation monitoring
(RecurrentDetector 2025, ~95% acc) detect more loop shapes than consecutive-identical.
**Why rejected:** a window over-fires on legitimate "re-read a reference file between productive edits" (the repeated
read is in-window but real progress is happening) — the consecutive-identical signal is the **lowest false-positive**
choice, and `max_iters` backstops the rare period-2 case (build the window only if we observe one). Activation
monitoring needs hidden states **we don't get from an API** — white-box-only, so it fails the model-agnostic /
universal-signal test by construction.

### `strip_think` reasoning-tag stripper — REJECTED (`research/21` §9)
**Why considered:** the verbose local model appeared to emit `<think>…</think>` reasoning that was re-fed each turn
(context leak) and would bloat Claude at review; a tolerant stripper (record-raw / feed-stripped) looked like the fix.
**Why rejected (3-lens adv review, all non-SHIP; reviewer-C live probe load-bearing):** the served
`Qwen3.6-35B-A3B-oQ8-fp16-mtp` emits **ZERO `<think>` tags** — it reasons in **plain markdown prose**, confirmed even
with `/think` + `enable_thinking:true`. A tag-stripper is therefore a **no-op on 100% of real output = dead code**, and
worse, it *looks* like a working safeguard. The actual leak is **uncapped re-fed prose + tool output**, which no
stripper can locate. Cutting it (rather than shipping dead code on a flawed premise) is the integrity call. Replaced by
bounded re-feed (above). The live trajectory confirmed it post-hoc: every tool-call turn carried **0 bytes** of
accompanying prose — there was never anything to strip.

### Tool-based `acted` signal for the plan→execute nudge — REJECTED (`research/24` §10)
**Why considered:** the natural fit for "did this run do work?" looked like "did a gate-allowed mutating tool run?"
(`gate::tool_is_mutating` = `write_file`/`bash`, never `read_file`) — a signal the loop already has in hand, no git call.
**Why rejected (BLOCKER, reviewer B):** the system prompt (`main.rs:731`) **orders the model to start with `bash ls
-R`**, so `bash` runs on essentially every run → a tool-based `acted` is true on every run and the nudge **never
fires** — the feature is dead on its primary target. Replaced by **worktree-dirty-at-stop** (`git status --porcelain`),
which measures the *artifact* (a real edit) not the *activity* (a tool firing) — `bash ls` and a read-then-stop both
read clean, a `bash sed` edit and an untracked `write_file` both read dirty. "Gate by the artifact, not the proxy."

### Graded / multiple plan→execute nudges — REJECTED for now (`research/24` §6, §9)
**Why considered:** more than one nudge, or escalating wording, might convert a stubborn no-action stop into work.
**Why rejected:** cap = 1, mirroring loopgate's one-recovery-turn shape — and the live number argues *down*, not up:
the single nudge already converted ≈0 (the 35B re-asserted completion rather than acting). Adding nudges multiplies a
move that isn't working; the honest `stalled` label is what carries the value. Widen only if a number ever shows
conversion is real but under-served by one turn.

## Context budget — sized to the reasoning curve, not the hard window (research/27)
**Context:** Gary challenged a baked-in 32K-token window assumption (*"coding harnesses allow up to 128k"*). Verified
live: `~/.omlx/settings.json` → `max_context_window: 262144`; the served Qwen is **256K-native** (no RoPE scaling).
The 32K wall was a **phantom** — stale in three places (`CONTEXT_WARN_BYTES` docstring, research/21's truncation
premise, the floated ~96 KB F1 budget). But "256K-native" is a **retrieval/capacity** claim (only that the server
*accepts* 262144), **not** a reasoning-quality claim. A coding agent *reasons* over its context (multi-hop), it
doesn't merely retrieve — and reasoning erodes far earlier than literal recall (NoLiMa two-hop 57→26% by 32K while
NIAH stays ~98%; RULER effective ≈ ½ advertised; Qwen3-30B-A3B → ~64K effective, the binding factor being ~3B
active params).
**Decision:** *Effective context budget is sized to the A3B reasoning curve (~32K full / ~48K cap), not the 256K hard
window or the 128K folklore. Selection policy = `window − fixed_reserve` (window=40K, reserve=16K, keepRecent=20K),
token-estimated at chars/4, KV fp16.* Concretely: set the hard window to **40K** → `window−reserve = 24K`,
`0.8×window = 32K` → **compaction trigger 32K**; live transcript oscillates ~20K (post-compact) ↔ 32K (trigger),
squarely in the full-quality band. F1 (`refeed::assemble`) tail budget = `keepRecentTokens ≈ 20K tok` (~80 KB at
chars/4 — close to the old byte value, now principled); head always kept. At ≤48K, fp16 KV is only ~2.5-4 GB → 4-bit
KV is **unnecessary** (and sidesteps the Qwen per-head-collapse risk).
**Free levers (independent of the number):** pin the active task spec to head/tail (Lost-in-the-Middle >30% mid
drop); give lexical anchors — name exact files/symbols, don't rely on latent multi-hop (NoLiMa rescue 26→87%); keep
the head stable (system prompt + case-law) to maximize prefix-cache hits and kill cold-prefill TTFT.
**Why the alternatives lost (Rejected Approaches):**
- *Size to the 256K hard window* — confuses "the server accepts it" with "the model reasons well over it." Memory
  binds ~500K (non-binding) and latency is prefix-cache-mitigated; the binding constraint is reasoning erosion at
  ~32-64K. Filling 256K spends KV + TTFT to *degrade* answer quality.
- *Trust "256K-native" as a reasoning guarantee* — it's a capacity/retrieval property (RoPE range), verified only as
  "accepts 262144." Retrieval ≠ reasoning; NoLiMa shows the gap is ~2× at 32K.
- *The "128K sweet spot"* — folklore/marketing anchor, not a measured effective length. The real finding is
  "effective ≈ ½ advertised," which for an A3B lands lower than 128K, not at it. (Gary's 128K instinct was
  directionally right — our 32K was a phantom wall — but the honest A3B figure is ~32-64K, not 128K.)
- *4-bit KV at our window* — buys nothing at ≤48K (KV already ~2.5-4 GB) and imports the KIVI/Qwen worst-case
  per-head collapse hidden by good average PPL. Reserve it for if we ever push toward 256K.
**Empirical gate (deferred until a number bites):** a NoLiMa-lite oMLX probe — plant a 2-hop fact at varying depths,
measure recall@8K/16K/32K/48K — to confirm/tighten the 32K/48K priors on *our* model. Throwaway `/tmp`; don't land.

## Shakedown fixes #1/#3/#4 — gate-before-worktree, language-aware Harden (cosmic-ray), enforced oracle integrity (2026-06-11)
**Context:** the t6/t7/t8 dogfood runs surfaced five findings; three were actionable (#1 fail-fast wart, #3 Harden
Rust-only, #4 oracle protection by-instruction-not-enforced). #2 (verify-behind-harden) is by-design; #5 (Python
bytecode) was a one-line `.gitignore` fix. #3 and #4 each carried a fork, resolved with Gary via AskUserQuestion.

**Decision #1 — gate before worktree.** `run_worktree` checks `gate::mutating_allowed(status)` *before*
`git::ensure_worktree`, so a `run` on a pre-Align ticket refuses with no orphan `harness/<id>` branch+worktree.
Defence-in-depth: `run_ticket` keeps its own recheck. (Cheap, no fork — obvious correctness fix.)

**Decision #3 — BUILD a Python Harden backend (cosmic-ray), don't accept Rust-only.** `run_harden` dispatches by
**diff content** (`.rs` → cargo-mutants, else `.py` → cosmic-ray, else vacuous 1.0). Python mutation testing =
**cosmic-ray 8.4.6, operator-installed (`pip install cosmic-ray`), NOT a crate dep** — the cargo-mutants analog,
with native git diff-scoping (`cr-filter-git` = the `--in-diff` analog). Judge by parsing `dump` (exit code lies).
`MutationScore` got a second parser (`parse_cosmic_ray_dump`) mapping killed→caught / survived→missed /
incompetent→unviable[excluded] / skipped→ignore — same score convention as cargo-mutants on the same diff.
**Why the alternative lost (Rejected):**
- *Accept Rust-only Harden (gate Step-2 to a Rust target)* — would permanently couple the §7 quality gate to one
  language while the rest of the loop (gate→isolation→tools→telemetry) is language-agnostic; the harness must
  dogfood on Python katas (`dogfood/`), so a Rust-only verify leg can never complete those runs. Building the
  cross-language backend is ~one bounded module + a parser, and keeps the gate honest everywhere.
- *Detect language by repo layout* — this repo is a Cargo workspace that ALSO holds Python under `dogfood/`, so
  layout is ambiguous; the **diff content** is the unambiguous signal (what did THIS ticket change).
- *Vendor a Rust-native mutation engine for Python* — none exists; cosmic-ray is the mature tool. Operator-install
  (not a crate dep) keeps it out of the build graph — invoked as a subprocess, same posture as `cargo mutants`.

**Decision #4 — ENFORCE oracle integrity by CONVENTION (auto-detect), not an explicit per-ticket allowlist.** At
verify (build|bugfix only — refactor legitimately rewrites tests), **git is the snapshot**: a convention-matched
oracle file that existed on base AND changed on the branch ⇒ `GATE_ORACLE_INTACT` fail + bail ("never modify
success criteria", enforced by code). Convention (`is_protected_oracle`): Python `test_*.py`/`*_test.py` + Rust
`tests/` integration files. Naturally allows ADDING new tests (absent on base) while forbidding edits to the
committed oracle. `GATE_ORACLE_INTACT` is evidence-only (not a transition gate). **No schema change.**
**Why the alternatives lost (Rejected):**
- *Explicit per-ticket allowlist of protected paths* — more ceremony per ticket, easy to forget (a forgotten entry
  silently disables the guard — the worst failure mode), and redundant with a naming convention the ecosystem
  already enforces. Convention is zero-config and fails safe.
- *Store an oracle snapshot/hash in a new board column* — git already IS the content snapshot (base branch vs
  worktree); a parallel store would duplicate it and drift. Comparing against the base branch is the source of truth.
- *Protect Rust inline `#[cfg(test)]` units too* — impossible without freezing the source file the agent must edit
  (the test shares the module under change). Scoped out and documented: a guarded Rust oracle must live in `tests/`.

**Evidence:** board 25 + agent 99 tests green, clippy clean; 3 new tests (`cosmic_ray_dump_parses_and_scores`,
`protected_oracle_convention`, `run_refuses_unaligned_ticket_without_creating_worktree`). Uncommitted.

## `edit_file` — a surgical anchor-by-exact-text editor as the 4th tool (2026-06-11)
**Context:** across the dogfood katas (calc-modulo, big-edit) the 35B's only edit failure was **precision, not
comprehension**: t12-w2 navigated correctly but used `bash sed -i` with miscomputed line numbers and landed the
`is_prime` guard *inside the docstring* (dead string text) → FAIL. With only `write_file` (full-overwrite) and
`bash` (line arithmetic), there was no primitive for "change these few lines without seeing/owning the whole
file" — and full-overwrite is actively dangerous when the file is offload-elided (rewrite-from-preview clobbers
the elided middle, the big-edit data-loss risk).
**Decision — add `edit_file{path, old_string, new_string, replace_all?}`:** read → exact-substring match →
replace first (or all) → write. Anchor by *content*, not position. LLM-failure guards are the point, not an
afterthought: empty `old_string`, `old==new`, **0 matches** (with a `no_match_hint` that distinguishes
indentation/whitespace drift, CRLF-vs-LF, and retyped-from-memory), and **>1 match without `replace_all`**
(ambiguous → name surrounding lines) each bail with a corrective message. Same `confine(cwd,path)` worktree jail
as the other file tools. **No gate change needed** — `tool_is_mutating = !matches!(name,"read_file")` auto-classes
it as mutating, so it unlocks only post-Align (asserted in `gate.rs`). System prompt teaches: prefer edit_file
with `old_string` copied verbatim from a `read_file`; never rewrite a whole file to change a few lines; never
compute line numbers for sed.
**Why the alternatives lost (Rejected):**
- *Keep write_file + bash sed only* — this IS the rejected status quo; it produced the one documented FAIL.
  sed/line-arithmetic is fragile across re-reads, and full-overwrite clobbers offload-elided regions. The A/B is
  decisive: the identical "test-first, add the guard" strategy FAILED with sed (t12-w2) and PASSED with edit_file
  (t15-w2).
- *Line-number / range-based edit (`edit_lines(start,end,text)`)* — reintroduces exactly the line-arithmetic the
  35B gets wrong, and breaks the moment the file is offloaded (the model doesn't have true line numbers for the
  elided span). Content-anchoring needs neither the full file nor a line number.
- *Fuzzy / whitespace-insensitive matching* — silently editing a near-match is the dangerous failure mode for an
  edit tool. Exact-match + a *hint* that explains the near-miss keeps the human/agent in control (copy the real
  text) without guessing. This is the Claude Code Edit-tool contract, adopted deliberately.
- *Diff/patch application (unified-diff tool)* — heavier format the model must emit perfectly (hunk headers, line
  counts); a single anchor string is lower-ceremony and the failure modes are easier to hint on.
**Evidence:** 8 new unit tests; full agent suite **124 pass / 0 fail / 3 ignored**; live oMLX `explore` t15 =
**3/3 PASS**, all diffs byte-identical (blob `de8a1f5`), oracle untouched, fix not landed. Adoption proof: a
worker whose strategy never named edit_file chose it over sed from the system-prompt guidance alone. **Scope
caveat:** validated on a localized point-insertion; big-region rewrite and cross-file signature ripple remain
untested.

## Intake drafting — `agent draft <id>`: strong provider proposes the workpad, keystone untouched (2026-07-13)
**Context:** the 6-30 readiness assessment named "no intake/drafting" as the #1 daily-driver gap (operator
hand-authors Plan/Criteria/Validation per ticket) and scoped the slice: system drafts the workpad from a seed at
any fidelity, Align stays the human back-and-forth (decisions.md "Task intake" reframe). Review-exempt because
reversible + loud — a verdict that HOLDS ONLY while (a) drafting never touches gates/status and (b) it never
silently overwrites operator content. Both are load-bearing invariants, not conveniences.
**Decision — new verb `agent draft <id> [--force]`, pure core `draft.rs` + glue `run_draft`:**
- **Separate verb, not folded into `new`:** `new` stays cheap; a draft is re-runnable (rework re-entry, seed
  enrichment) and independently gateable. A one-liner CLI flow is still two commands — acceptable.
- **Provider = strong-by-default, decoupled from the landing run:** default `deepseek` (drafting is the judgment
  task where the strong-worker gap bites first — assessment §critical-path), overridable via
  `HARNESS_DRAFT_PROVIDER`, resolved by new `config::select_named` (ignores `HARNESS_PROVIDER`/`HARNESS_MODEL`).
  Only the *default* degrades — loudly — to local oMLX when DeepSeek is unavailable; an explicit override is
  never silently substituted.
- **Overwrite guard:** any non-empty plan/criteria/validation → refuse without `--force`; with `--force` the
  fields ride into the prompt as "Operator-provided (refine, don't discard)" so a re-draft refines rather than
  clobbers. Guards fire BEFORE provider construction (hermetically testable, no network).
- **Keystone untouched:** `run_draft` writes only via `set_plan`/`set_acceptance_criteria`/`set_validation`;
  never gates, never status. Verified by artifact on the live run: ticket still `todo`, attempt 0, **0 rows** in
  `gate_results`.
- **Memory pre-fill:** same title-keyed best-effort `recall_primed` as `run_ticket` feeds past lessons into the
  prompt (the compounding channel from the intake decision).
- **Truncation before parse** (review.rs finding reused): `StopReason::Length` bails before `parse_draft`; a
  parse-fail or unusable draft (missing plan/criteria) writes NOTHING. Empty validation → warn, not reject
  (non-code kinds may lack a one-command check).
**Evidence:** 11 new tests (9 pure-core parse/build/usability + 2 glue guards), suite 178 pass / 0 fail, clippy
clean (incl. a pre-existing `collapsible_if` in refeed.rs fixed in passing). Live gate: one-line seed
("add a --json flag to agent status") → DeepSeek drafted 5 enumerated testable criteria + jq-based validation +
4 genuinely-open questions; Align would be one-round (drop a hallucinated `assignee` field), not a rewrite —
the assessment's acceptance bar. Calibration n=1; drafted-vs-hand-authored sample still to accumulate.
**Finding (surfaced → RULED 2026-07-13):** the live run exposed a **hardcoded DeepSeek API key** (`BAKED_KEY`,
`provider/src/openai.rs:56`, committed in 42396ce) contradicting research/30's recorded posture ("key lives ONLY
in `DEEPSEEK_API_KEY` env, never hardcoded/filed"). **Gary's ruling: the baked key is a deliberate throwaway —
leak-harmless, stays as-is.** This AMENDS research/30's env-only posture for this key specifically (env var still
wins when set); a non-throwaway key must never be baked. Known side-effect: `select_named("deepseek")` can never
fail at construction, so `run_draft`'s omlx fallback is unreachable dormant code — it goes live only if the baked
key is ever removed.

## Strategic posture — don't compete with Claude Code, CONTAIN it (2026-07-14)
**Context:** Gary's meta-question after the intake slice: is the harness overengineered, and why not just use
agent-skill prompting in Claude Code? Settled here so future sessions don't re-litigate.
**Verdict — not overengineered per-slice, one strategic trap.** The per-slice record is evidence-driven, not
speculative: loop gate after an observed 20× repetition loop, `edit_file` after a real sed-into-docstring FAIL,
plan-exec after a real 0-file stall, think-stripping CUT by probe, memory Slice B NO-GO'd by its own gate,
coordinator still unbuilt because nothing demanded it. Honest overweight: the make-the-weak-35B-safe arc
(refeed/compaction/verbosity) was expensive relative to "just pay for a strong worker" — defensible (measured,
cheap per slice, worker-agnostic telemetry) but the closest thing to real bloat.
**The problem the harness fixes (the thesis, one line): prompting is advisory; code is enforced.** Every skill
integrity constraint is enforced by the model it constrains — the model grades its own homework, and the record
proves it fails even at Claude altitude (research/26 §9 fabricated-causality incident). In the harness the same
failures are *structurally impossible*: `criteria_confirmed` is human-source-only in the schema, mutating tools
are status-locked pre-Align, verify needs a real `tests_green`, land is the human squash-merge path. The skill
itself CONVERGED on this (its evolutions were reverse-engineered from the harness) — the skill is the judgment
layer; it cannot be the enforcement layer. Secondary: compounding structured memory, vendor independence +
own-trajectory RL option, and Gary learning the stack by building it.
**The trap, named: accidentally rebuilding Claude Code** (chasing its worker quality/UX/orchestration on a
personal budget against Anthropic's ship rate — unwinnable, pointless). **Resolution = the critical path already
chosen: CC becomes the best interchangeable WORKER inside a structure that doesn't trust any worker's
self-report.** The harness owns process (gates/memory/isolation/audit); `claude -p` brings the intelligence.
Tripwire: building CC-like UX (e.g. a TUI chat panel) before the coordinator exists = drift; stop and reread.

## Delegated Claude worker — `agent run <id> --worker claude` BUILT (2026-07-14)
**Context:** critical-path item #2 (readiness assessment); the "contain, don't compete" resolution made
concrete. Subscription OAuth can't back single turns (decisions.md "TWO shapes"), so CC enters as a WHOLE-TASK
worker: `claude -p --output-format stream-json` spawned in the ticket's worktree, own loop, harness gates
untouched around it.
**Decision — pure core `worker.rs` + `run_claude_ticket` glue, default OFF (`--worker claude` opt-in):**
- **Same fail-fast pre-Align gate as `run_worktree`** (no orphan worktree, no telemetry row on refusal — parity
  pinned by test). Same telemetry (`start_run`/`finish_run`, provider `claude-cli`, cost + num_turns recorded),
  same `commit_worktree` step, same downstream gates.
- **Outcome mapping is trust-nothing:** timeout (kill) > non-zero exit > missing result event > `is_error`; a
  clean exit with no parseable `type:"result"` event is `error`, never success; a result event missing
  `is_error` reads as error (absent success signal ≠ success). Labels join the open vocabulary (`completed`/
  `error`/`timeout` — the `stalled` precedent).
- **Permission posture (Gary's align ruling): `--dangerously-skip-permissions`** — same trust as interactive CC;
  worktree jail is prompt-level for this worker (native tools stay `confine()`d); accepted residual risk,
  revisit on an observed out-of-tree write. Bounds: `--max-turns` 40 + wall-clock 900s (env-overridable
  HARNESS_CLAUDE_{BIN,TIMEOUT,MAX_TURNS,MODEL}).
- **Diagnostic spine does NOT apply inside CC** (it manages its own context); the harness's job is outcome
  mapping + audit. Stream captured verbatim to `.harness/runs/<t>/<run>.claude.jsonl` (root-side, N1 — survives
  land). Memory priming DOES apply (same `recall_primed` channel — compounding is worker-agnostic).
- **Cfg as data (`WorkerCfg`), env read once at dispatch:** tests construct it directly — no `set_var` (unsafe
  in edition 2024, racy under the parallel runner).
- **Non-goals (follow-ons):** mid-run Confusions bounce, coordinator, draft-via-claude, explore-with-CC-workers.
**Evidence:** 188 tests green (+10: 6 pure-core parse/outcome/prompt + 4 hermetic glue via a fake-`claude` shell
binary: success/garbage+is_error/timeout/pre-Align-refusal), clippy clean. **Live full spine on a fresh kata**
(parse_version, seeded-tests-as-oracle): align → delegated run (`completed`, 4 turns, $0.59 nominal, worker ran
validation itself, left git untouched per prompt contract) → harden PASS 0.909 → verify PASS → DeepSeek review
SATISFIES/high → land squash 6bdc17e → done; `todo!` absent from base (artifact check). Auth probe: headless
`claude -p` works via macOS keychain on this Mac — no setup-token needed (CLI 2.1.208).

## Sprint coordinator — `agent sprint [--worker claude] [--max N]` + `agent edge` BUILT (2026-07-14)
**Context:** critical-path #3, the readiness assessment's last structural gap (board had edges/blocking; nothing
drove the queue). With intake (`draft`) and the delegated CC worker already landed, this closes the assessment's
entire critical path.
**Decision — one-pass coordinator over `Board::runnable()`, keystones untouched:**
- **`runnable()`** (new, board): `in_progress` + every direct blocker terminal — the post-Align sibling of
  `ready()` (shared SQL via `unblocked_with_status`; same priority ordering). A ticket aligned before its blocker
  landed PARKS (running it would build on a base missing the dependency's work).
- **Per ticket: worker run → harden → verify → advisory review → PARK.** The sprint never aligns and never lands —
  the two human keystones stay human. Consequence (load-bearing): the runnable set cannot grow mid-sprint (only a
  human land makes a blocker done), so a **single snapshot is complete by construction**; the rhythm is
  sprint → human lands/aligns → sprint. No re-scan loop, no scheduler.
- **Park-and-continue failure posture:** one ticket's failure (worker error, gate refusal, bounced verify) is
  recorded in its entry and the sprint moves on. **Nothing auto-retries** — a bounce/rework is a human decision;
  auto-retry would burn budget re-running a worker the gates just rejected.
- **Artifact-based reporting:** run judged by its recorded row's `stop_reason` (not the glue's `Ok(())`);
  `final_status` read back from the board after the phases. Summary ends with the two human queues — land queue
  (parked at review) + align queue (`ready()`) — a sprint ends by saying exactly where the keystones are needed.
- **Review knob is test-only:** hermetic tests pass `review=false` to skip the metered DeepSeek call; the CLI
  always passes true (a planned gate skipped silently is an integrity miss).
- **`agent edge <id> <dep> [--kind blocks]`** (surfaced by this slice): `runnable()` dispatches by edges, so edges
  must be CLI-authorable; both endpoints validated loudly (a typo'd id fails at authoring, not later as a
  dangling-edge lint). Serial v1; parallel CC workers = follow-on (worktrees already isolate).
**Evidence:** 192 tests green (+4: board `runnable` blocked/unblocked/todo matrix; sprint summary rendering ×2;
hermetic end-to-end sprint via fake-`claude` — runnable→review, blocked never dispatched, todo untouched; shared
module-level CWD_LOCK so cwd-swapping tests can't race), clippy clean. **Live: FIRST MULTI-TICKET PROJECT through
the full daily-driver loop** — t2/t3 with a `blocks` edge, both workpads DRAFTED (DeepSeek; t2 = one-round align
settling the drafter's own leading-zeros question, t3 = one-shot confirm — the fidelity dial live), sprint pass 1
drove t2 to review (CC worker 5 turns $0.72, mutation 1.000, verify PASS, DeepSeek SATISFIES/high) while t3
parked blocked; human landed t2 (f32c503); sprint pass 2 freed and drove t3 (mutation 1.000, SATISFIES/high);
human landed t3 (dd055a7); base green 10/10 tests. **The assessment's critical path (intake → strong worker →
coordinator) is COMPLETE.** Next trigger: a real (non-kata) multi-ticket project — the daily-driver claim's
remaining gate.

## Self-sprint: the harness fixed its own gates via its own spine (h-t1/t2/t3, 2026-07-14)
**Context:** PDSI t4 surfaced three defects; per the fix-the-instrument-not-the-verdict ruling they became tickets
on THIS repo's board (`~/Documents/Work/harness-board.db`), drafted-workpad → aligned → `agent sprint --worker
claude` → landed. First self-hosting run.
**Landed:**
- **t2 `e441729` — checker scripts are oracles.** `is_protected_oracle` extended: `.py` starting `check_` whose
  immediate parent dir is `scripts` (any depth). Closes the PDSI tamper window structurally. First-pass clean
  (mutation 1.000, SATISFIES/high, 5 turns $0.92).
- **t1 `baf10cf` — harden provenance filter.** Files under a `.template-stamp.json`-stamped ancestor are excluded
  from mutation scope in BOTH backends (pure `is_template_stamped` + `git::diff_against_paths` with the
  zero-paths→empty-diff guard); an all-generated diff takes the vacuous-pass path with a loud note. **Round 1
  hit the gate red (0.500)** — the mutation gate caught undertested code in the fix to the mutation gate.
  Operator appended the survivor list to the workpad notes + guidance (kill the pure `||` with operand-isolating
  cases; extract glue decisions into pure fns rather than executing `run_harden` in tests) and re-dispatched the
  SAME worker in the SAME worktree → 1.000 (23/0). **This additive retry loop (gate-red → operator annotates
  notes with machine evidence → re-dispatch, no attempt bump) is now a validated pattern** — distinct from
  `rework` (hard reset), which remains for discard-and-redo.
- **t3 `acf054d` — score under-reporting root-caused + fixed.** The 10-vs-1293: score was parsed from
  `cosmic-ray dump` STDOUT (truncatable middleman), dump's exit status ignored (a rule valid only for `exec`),
  verdict-less rows invisible, no completeness cross-check. Fix: read the session sqlite directly
  (`work_items LEFT JOIN work_results`), `CrSessionTally{total,skipped,unresolved}`, `partial = unresolved>0`,
  and **`passed = score≥threshold && !partial`** — a partial session can never pass. Regression test encodes the
  exact observed shape. Mutation 0.920; DeepSeek SUSPECT/medium was procedural-only (the notes-append AC the
  worker CORRECTLY refused — board DB is outside its jail, it asked the operator; containment working as spec'd).
**Evidence:** 199 tests green on landed main, clippy clean, 3 squash commits. Worker cost ≈ $13.5 nominal
(subscription credit). **Meta-finding:** the two-direction loop closed — a real project's failure fed fixes back
through the harness's own board, gated by the gates being fixed.

## Harden on template-heavy projects: vacuous is DOCTRINE, the oracle carries verification (2026-07-15)
**Context:** PDSI t4/t5 — every changed code file sat under a `.template-stamp.json` dir, so the provenance
filter (t1 above) correctly left nothing to mutate; both tickets took the loud vacuous-pass path and the
hand-authored protected `check_*.py` oracle carried the whole verification load (t5's fixture-grounded
`check_slice.py` asserted real-model discrimination: no_gloves present AND no_helmet absent — a broken decode
cannot pass it).
**Decision:** this is working as designed, not a gap. On template/scaffold projects the operator-authored
oracle IS the gate; harden's job there is only to stay honest about having nothing to say (the loud vacuous
note). No handwritten-lines heuristic gets built absent a measured failure it would have caught.
**Corollary (re-assessment 2026-07-15):** operator oracle authorship is the *essential* operator tax — reduce
the accidental tax (draft grounding, freeze granularity), never this one.
**Rejected:** treating vacuous harden as a red flag requiring a synthetic mutation target; per-file
handwritten-fraction scoring (speculative structure, no failing run to point at).

## Oracle-freeze granularity: template-stamped test files exempt, resolved against BASE (2026-07-15, t4 `129d43e`)
**Context:** PDSI t5 burned a retry dispatch (~$3, ~20 min) because ALL base `test_*.py` are frozen — including
template boilerplate the ticket may legitimately regenerate. Re-assessment ranked the fix #2 (small, hits every
template project).
**Decision:** the verify tamper check exempts a protected TEST file iff it is `.template-stamp.json`-stamped
**as resolved against base history** (`git::stamped_on_base`: ancestor-dir walk via `git cat-file -e
<base>:<dir>/.template-stamp.json`). Two hard edges: (1) `scripts/check_*.py` operator checkers are NEVER
exempt, stamp or no stamp (new pure `board::is_operator_checker` carves them out — the checker is the
operator's validation contract even inside a stamped dir); (2) a stamp that exists only in the worktree —
dropped on the fs OR committed on the branch — unfreezes NOTHING (tests pin both). PASS notes say
` exempt_stamped=N` when the exemption fired (loud-when-fired, silent-when-not, same contract as harden's
`stamped_note_suffix`).
**Why base-resolved (the load-bearing bit):** the freeze is a hard integrity gate; if the stamp were read from
the worktree fs (like harden's `is_template_stamped`), a worker could stamp its way past the freeze. Base is
pre-worker, so base history is the only trustworthy provenance source at verify time.
**Accepted-loud residual:** harden's provenance partition stays worktree-fs-based — a worker could stamp-dodge
the mutation gate, but that path is advisory + loud (`excl_stamped=N` + vacuous-pass warning), and legitimate
in-worktree template instantiation (PDSI t4's shape) requires it. Revisit only on an observed dodge.
**Rejected:** reusing `is_template_stamped` at verify (the tamper vector above); exempting checkers when
stamped (defeats the whole point of t2's protection).

## Workpad notes APPEND by default; `--replace` is the explicit overwrite (2026-07-15, t5 `66a0c98`)
**Context:** notes accumulate operator guidance across additive retries; the plain-UPDATE overwrite forced the
operator to rebuild the entire pad to add one retry note (burned live during PDSI t5 and again scoping this
sprint).
**Decision:** `agent note <id> "<text>"` appends with a blank-line separator (`Board::append_notes`, audited
like every workpad edit, loud on unknown id); `--replace` keeps the old overwrite. **Confusions stay
overwrite BY DESIGN** — the Align bounce reads one current confusion, not a history (trunk-slice-2 confirmed
fork, unchanged). New `agent board` overview verb (spine-position → priority → id, plus per-status counts)
closes the no-board-overview burr. First live payoff same session: t6's retry guidance appended under the
original note with zero pad surgery.

## Verify refuses an empty diff against base (2026-07-15, t8 `42f5cc9`)
**Context:** sprint #2's stale-worktree collision exposed that verify had no "did the branch change anything"
floor — an untouched branch sailed to review with verify=PASS because validation ran green on the old tree.
Review already had the floor (`main.rs` empty-diff bail); verify didn't.
**Decision:** `run_verify` resolves base + the branch's changed-file set ONCE (shared with the oracle-integrity
block, which no longer re-shells) and refuses an empty diff BEFORE the validation command runs. The refusal
happens before any board write: ticket stays `in_progress`, NO gate row is recorded — **a refusal is not a
gate result** (the event-count assertion in the test pins exactly this). Outside a git repo the floor degrades
to skipped, mirroring every other git-dependent check. Error names the remedies (re-dispatch / `rework`).
**Ripple accepted:** the sprint-contract test's fake worker now writes a file — a no-op fake would correctly
hit the new floor; the fixture had to follow the contract change (loudly commented in the test).

## Fork-point guard on worktree/branch reuse: fail-closed + explicit escape hatch (2026-07-15, t9 `b8610de`)
**Context:** `ensure_worktree` reused any surviving `harness/<id>` branch blindly; June's leftover branch
silently attached a worker to a pre-everything main ($1.68 wasted; had that branch carried unlanded commits,
land would have squash-merged foreign work into main). `remove_worktree` already reaps branch+worktree on
land/rework/close-abandon — the debris source is only never-landed tickets, so the guard is reuse-time.
**Decision:** both reuse paths (existing worktree dir; attach-to-surviving-branch) require
`merge-base(branch, HEAD) == HEAD`. Mismatch → **refuse**, naming branch, fork sha, commits-behind count, and
three remedies (rename to `stale/<id>` preserving work / `rework` with a DESTROYS warning / the hatch).
`HARNESS_ALLOW_STALE_FORK=1` downgrades to a loud stderr warning — it exists for the one legit mismatch:
additive-retry after base advanced under a live branch (wiki commits land on main mid-sprint).
**Why fail-closed:** the false positive costs the operator one manual step; the false negative squash-merges
foreign commits into main. Testability pin: the core (`ensure_worktree_with`) takes `allow_stale: bool`; only
the outer fn reads env — no test mutates process env (parallel-test races designed out).
**Known residual (recorded honestly):** the one surviving mutant (0.800) is the env-read comparison itself
(`v == "1"` → `v != "1"`) — untestable *because* of the no-env-mutation pin; unset-env behavior fails CLOSED
either way, so the exposure is only a deliberately-set-but-wrong hatch value.
**Rejected:** warn-and-proceed (wouldn't have stopped June's collision if the stale branch had carried
commits); board-unique ticket ids (pushes the problem to naming discipline instead of designing it out).

## Reviewer fix-or-drop: RESOLVED → DELETE the automatic advisory-review call (2026-08-02)

The rule was pinned before trial ticket 1 (trial-ledger.md): keep iff ≥1 true positive AND 0 false
VIOLATES across 5 real tickets. Window result: **0 TP / 0 false-V / +12 clean samples** — every verdict
was an abstention or a confirmation of what the machine gates had already proven. A call that never
changes an outcome gates nothing; per the pinned rule it is deleted from the sprint flow (execution =
B6; `agent review` verb retained for manual use). **Rejected alternatives:** (a) extending the window
("maybe TPs come later") — rejected as moving pinned criteria to fit a hoped-for result, the exact
integrity violation the rule exists to prevent; (b) keeping it as "free" signal — rejected: it costs a
provider call + operator adjudication per ticket and twice cried wolf pre-trial (0/2 TP on final
branches). **Successor:** cross-vendor reviewer design (Omnigent teardown, research/17 candidate) —
this DELETE is its trigger; build only if the design survives the teardown.

## Harden test-command: auto-scope to the first `&&` segment (2026-08-02)

**Decision:** cosmic-ray's per-mutant test-command is the ticket's validation auto-scoped to the text
before the first `&&` (t13 `ced358a`). Loud when scoping fires (printed notice + gate-note marker),
byte-identical passthrough when there is no `&&`, hard bail on an empty first segment. Codifies the
trial doctrine: first segment = cheap unit tests, everything after `&&` = verify-time oracles that are
structurally wrong per-mutant (finding #1: cosmic-ray shlex-splits with NO shell, so `&&` arrived as a
literal pytest arg → baseline "collected 0"). **Rejected:** (a) a dedicated `harden_cmd` DB column —
Wu Wei: the auto-scope solves the only observed failure; add the column only if auto-scope ever picks
wrong; (b) `sh -c` wrapping the full validation — mechanically fixes `&&` but runs the expensive
full-stack oracle on every mutant, which is the deeper structural error.

## Provenance granularity: diff-status, not dir-forever (2026-08-02)

**Decision:** a changed file under a template-stamped dir is template output only if it existed on
base (Modified) or its governing stamp was itself added in this diff (fresh instantiation); a file
ADDED under a stamp that pre-exists on base is the ticket's handwriting and STAYS in mutation scope
(t14 `8778f30` — closes finding #2, where t6's mutation scope collapsed to one file with 23
handwritten files wrongly excluded). Loud when the keep-exemption fires. **Rejected:** (a) stamp
manifest formats (list files at instantiation) — requires changing what writes stamps, and
accelerator-kit is FROZEN; (b) mtime heuristics — unreliable across git checkouts. **Accepted edge
(conservative):** an EDIT to a generated file is partially handwriting but stays excluded — mutating
it still mostly measures the template.

## DeepSeek baked key: permanent throwaway — B5 CLOSED (2026-08-02, Gary)

**Decision (Gary, verbatim ruling):** the baked DeepSeek key in `crates/provider/src/openai.rs` is a
throwaway, permanently — no rotation coming, no swap owed. B5 is CLOSED, not parked; drop it from
every owed/open list and do not resurface it. **Rejected:** keeping it on the owed list as a standing
reminder — it would nag forever about a risk the owner has explicitly accepted for a metered,
low-value key.

## A6: DAILY-DRIVER CLAIM GRADUATED (2026-08-02, Gary)

**Decision (Gary, on the trial-ledger numbers — "Graduate it"):** the harness is the daily driver for
real project work. Evidence base: 5/5 real PDSI tickets landed with complete ledger rows; 0 known
misses (no post-land defects a gate should have caught); 0 faked gate rows across every bounce; all
worker bounces were environment/oracle-plumbing, never worker dishonesty; $25.57 recorded worker cost
(nominal, subscription-credited; t7/t8 lost to the telemetry gap, true ~$35–40); plus the B-slice
(t10–t14, $9.11) landed through the harness's own spine same-day. Posture unchanged: contain CC as
worker, keystones human, oracle discipline carries verification (harden earns its keep only on
handwritten Rust/Python). **What graduation binds:** new multi-ticket project work defaults to the
spine (board + gates + delegated worker), not ad-hoc interactive CC; ad-hoc stays interactive by
posture, not shortfall. **Next-window instrument debts (unchanged by graduation):** upfront-estimate
column + comparator arm; worker cost/turns accumulated from stream events so a timeout can't erase
the tally.
