# 17 — Telemetry + Memory plane (designed together; telemetry built first)

**Status:** DESIGN, **adversarial review DONE → §4 revised**. Per the process gate (decisions.md "adversarial
review is mandatory for hard-to-reverse or silent-failure designs"), four independent reviewers attacked this doc
(schema-durability / runtime-failure / RL-data-validity / Wu-Wei-scope lenses). Verdict: **not safe as originally
written**; the corrected scope is **§4 (rewritten below)** and the full findings + resolved tensions are in **§8**.
§2a/§3 below carry the post-review decisions; the original "open questions" (§7) are now answered by §8. §5–6
(memory) remain the seam this slice must not foreclose.

---

## 1. Why these two are one design

Two requests, one substrate:
- **Audit/measure the harness** — read the run logs, judge quality (north star: "assume the design is perfect and
  a strong model is the better harness; how close did the local run get?").
- **Longer horizon** — accumulate trajectories to finetune / RL (GRPO or newer) a local model to *be* the
  harness's brain.

Both read the **same data with two readers**, and we already own half of it:
- The board `event` table is an append-only audit log (created / status_changed / gate_reported / workpad_edited).
- The `gate_results` rows + `attempt` counter are **objective outcome labels** — landed?, tests_green first-try
  or bounced?, how many rework epochs? The spine already emits the reward signal; we don't invent a reward model.
- **Missing:** the agent's *trajectory* — the per-iteration messages, tool calls, and tool results. Today that is
  ephemeral (printed in `run_ticket`, never persisted).

So the substrate decision is: **persist the trajectory at the loop boundary, keyed to (ticket, attempt), with the
board events/gates as its labels.** Audit and RL are then *views* over the same log.

**The memory seam (why design them together):** the §6 episodic sidecar is **a curated, salience-rated, embedded,
decayed *view* over this same trajectory** — reflection clusters trajectory steps on land/abandon → a generalized
lesson → a sidecar row. If we design the trajectory schema without memory in mind we hit the seam twice. Hence:
design the data model once; build telemetry first; build memory on top later.

---

## 2. The shared data model

Three layers, raw → curated:

```
trajectory log  (raw, every step, append-on-write)      ← telemetry slice (build first)
      │  reflection (cluster on land/abandon, local LLM)  ← memory slice
      ▼
episodic sidecar (salience-rated, embedded, decayed)     ← memory slice
      │  promotion (high usage·salience, gated on landed)
      ▼
wiki (semantic/procedural, never decays)                 ← already exists (human + reflection)
```

### 2a. Storage shape — SQLite index row + referenced JSONL blob (POST-REVIEW)

Mirrors the existing pattern (schema.rs: board events are SQLite append-only with a JSONL *export* as history).
The `run` row is the **authoritative index** (existence + outcome); the JSONL is a **blob it references by a
*derived* path** — one source of truth for "a run exists," not two (review B1/§8-M2). Behind a `user_version`
migrator with WAL (§8-M1/M3).

- **`run` row (SQLite, in the board DB)** — one per execution. **Written at loop ENTRY** (`ended_at` NULL =
  in-flight) and UPDATEd on every exit path, so crash/hang is the queryable `ended_at IS NULL` + stale, not an
  orphan file (review B1):
  ```
  run_id        TEXT PRIMARY KEY      -- sortable (ULID / ts-prefixed); the one sticky PK choice (§8-M4)
  ticket_id     TEXT NOT NULL
  attempt       INTEGER NOT NULL      -- rework epoch at run time (§3)
  model         TEXT NOT NULL         -- run-level; today a constant. Doubles as teacher-vs-local provenance (§6)
  provider      TEXT NOT NULL         -- oMLX | anthropic — the non-circular training-split key (§6/§8)
  sampling      TEXT                  -- json {temperature,...}: GRPO needs to confirm groups were sampled hot
  project       TEXT                  -- shared with the sidecar (§5); pre-filter key for retrieval
  started_at    TEXT NOT NULL, ended_at TEXT      -- ended_at NULL ⇒ in-flight or (if stale) crashed
  iters         INTEGER
  stop_reason   TEXT                  -- completed | max_iters | provider_error | killed  (derived from observed
                                      --   loop state; killed/provider_error written by a reaper, not the run §8-S3)
  -- trajectory_path is NOT stored: derived = fn(root, ticket, attempt, run_id), mirroring git::worktree_path (§8-M5)
  -- terminal_status DROPPED: always in_progress at loop exit, goes stale; the board is the source of truth (§8-N2)
  -- tokens DROPPED for now: provider::Response discards usage today → would be 100% NULL (§8-S1); add with a
  --   migrator ALTER once Response carries usage
  ```
- **trajectory JSONL (`.harness/runs/…`, gitignored, root-relative — NEVER worktree-relative, §8-M5/N1)** — the
  in-memory `messages` array dumped on exit (lean core, §4). The array already contains the frozen system+user
  prompt (`messages[0..2]`) and every tool call/result, so the GRPO frozen-prompt and the tool-error stream come
  for free. Per-step streaming/flush is deferred to the RL slice (same format, additive — §8 DEFER). **Secrets are
  redacted at the write boundary** (mask the oMLX key + env-var patterns) — a secret baked into a training file is
  unredactable later (§8-M8). Record shape (one per message):
  ```
  {step, ts, role, content, tool_call:{name,args}?, tool_result:{ok,output(redacted, len-capped)}?}
  ```

**Why index-row + referenced-blob, not pure-SQLite or pure-JSONL:**
- pure-SQLite bloats the operational board with large message blobs that are write-once / bulk-export-rarely;
- pure-JSONL isn't queryable/joinable with gates+events and needs a discovery convention;
- the row indexes (existence + outcome, queryable, joinable on `(ticket, attempt)`), the JSONL is its payload
  (the RL export format). The path is derived, not stored, so there is no second authority to drift (§8-M5).

### 2b. Labels are captured raw, never pre-scalarized

The `run` row + joined `gate_results`/`event` rows carry the **raw** outcome signals (terminal status, stop_reason,
gate pass/fail+timing, attempt/rework count, verify-bounce count, tokens). The **reward function `f(signals)` is
deferred to the RL slice** and kept *out* of capture — because the reward will evolve and we must not bake one
definition into the stored data. Capture is neutral; interpretation is downstream.

---

## 3. Three counters, kept distinct (decided now to avoid a migration)

- **`attempt`** — the *rework epoch* (sequential; bumped only on entry to Rework). Already exists; scopes gates.
  NOTE the GRPO caveat (§8): rework epochs do **not** share a prompt (the workpad mutates between attempts), so
  `attempt` siblings are *not* a valid GRPO group — only parallel same-prompt siblings are.
- **`run_id`** — a unique, **sortable** id per *execution* (ULID / ts-prefixed). A single attempt can be run more
  than once (resume → new `run_id`, never append to a prior run's JSONL). The sticky PK choice; minted at entry. ✅ now.
- **`group_id`** *(DEFERRED — nullable `ALTER` when the coordinator slice exists, §8)* — ties *parallel sibling*
  runs on one ticket into a **GRPO rollout group**. Originally proposed as "reserve now"; the review (Wu-Wei lens)
  correctly cut it: with the `user_version` migrator (§8-M1), `ALTER TABLE ADD COLUMN` is the one migration SQLite
  does trivially online, so reserving schema for an unbuilt pillar is speculative generality. Add it — plus a
  **frozen-prompt snapshot** proving group members are same-prompt — in the coordinator slice.

**GRPO alignment (the payoff, kept as a note, costs zero schema today):** coordinator's N *parallel same-prompt*
runs on one ticket *is* a GRPO group; the gates *are* the group reward. The parallel-exploration pillar and the
RL-data pillar are the same machine viewed twice — realized when `group_id` + the frozen-prompt snapshot land.

---

## 4. The telemetry slice (FIRST BUILD — revised post-review)

Leanest version that serves audit-now and the eval/"am-I-a-good-harness" goal **without foreclosing RL** — because
the migrator (M1) makes every deferred column a safe later `ALTER`. ~60 lines (a bit over Wu-Wei's 45; the migrator
and entry-row earn it). Order matters — M1 is the prerequisite that makes the rest fixable-later not fixable-never.

**Build (the five must-fix are M1–M5; the rest are corrections):**
1. **M1 — schema migrator.** Add `PRAGMA user_version` handling to `schema::init`: read version → run ordered
   `ALTER`s → bump. Turns "sticky schema" from a wall into a ratchet; prerequisite for everything else.
2. **M3 — WAL + busy_timeout.** `schema::init` sets `PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;`. Correct
   even single-writer; mandatory before the coordinator runs two Recorders (else `SQLITE_BUSY` silently drops rows).
   Each Recorder gets its **own** `Connection`.
3. **Schema + Board ops** — add the `run` table (§2a) via the migrator. `Board::start_run(...)` (writes the entry
   row, `ended_at` NULL, mints the sortable `run_id`) and `Board::finish_run(run_id, stop_reason, iters)` (the exit
   UPDATE). `Board::runs_for(ticket)` reader. Board stays VCS-agnostic — the runs-root is **passed in** (it never
   stores or derives a path).
4. **M5 — `Recorder` (agent side), path derived + root-relative.** Resolves `.harness/runs/` from
   `git::repo_root()`, **never** from the loop's worktree cwd (else `land`'s `remove_worktree --force` deletes
   successful trajectories — review N1). Adds a `git::runs_path(root, ticket, attempt, run_id)` mirroring
   `worktree_path`. Dumps the in-memory `messages` array to that JSONL on exit, **redacting secrets at the write
   boundary** (M8: mask the oMLX key + env-var patterns; len-cap `tool_result.output`). Recorder failure is
   **non-fatal to the work but loud** (M6/S4): a failed JSONL open ⇒ metadata-only run row + stderr warning; never
   crashes the actual run.
5. **M2 — wire into `run_ticket`.** `start_run` before the loop; `finish_run` on **every** exit path (natural stop,
   `max_iters`, error) — derive `stop_reason` from the observed exit point, not after the fact (S3: drop the
   unreachable `tool_error`; `killed`/`provider_error` are reaper-written). No change to gate/spine logic.
6. **Lenient reader** — the JSONL parser drops/logs an unparseable **final** line (the crash design intends a torn
   tail) and never fails the whole file (S3).
7. **Read path (DEFERRED, §8):** no `agent runs`/`agent trace` CLI yet — `sqlite3 board.db 'select … from run'` +
   `jq` over the JSONL suffice until an actual audit hurts (the "absence causes failure" trigger hasn't fired).

**Success (artifacts):** unit tests — (a) a run writes an entry row then a matching exit row whose
`stop_reason`/`iters` agree with the loop; (b) the JSONL round-trips **and** a *torn-tail fixture* parses without
erroring; (c) the crash label is queryable (`ended_at IS NULL`), not a filesystem anti-join; (d) **the trajectory
file still exists after `land`** (the N1 invariant test); (e) a redaction test (the oMLX key never appears in a
written JSONL). Live oMLX run produces a real trajectory + entry/exit rows. `cargo test --workspace` green + clippy.

**Deferred — safe because of M1 (nullable `ALTER` when the consuming pillar lands):** `group_id` + frozen-prompt
snapshot, `split`/`parent_run_id`, per-step provider/model/sampling (run-level suffices until roles mix providers),
tokens (until `Response` carries usage), `agent runs`/`agent trace` CLI, per-step streaming/flush.
**Derived at consumption, not captured:** diff-churn (`git diff --numstat`), tool-error counts (in the dump),
wall-clock (the entry/exit timestamps).
**Deferred to the memory/eval slices:** embeddings, the sidecar table, salience/decay/promotion, hybrid retrieval,
reflection, the scalar reward `f` + RL export format, the model-judge rubric.

---

## 5. Memory plane (seam only — built after telemetry; gets its own review)

Per §6 of design-v2. The sidecar row is *derived from* trajectory+outcome; it shares the `(ticket_id, attempt)`
key and the `project`/`scope` tags with the `run` row. Schema (§6): `type, salience, scope, entities, embedding` +
`title/body/files/usage_count/last_used_ts`. Retrieval = hybrid top-k (`1.0·relevance + 0.5·recency +
0.8·salience`, pre-filtered by project/scope) returning a **ranked index** (id+title+type); bodies fetched only on
selection (progressive disclosure). Reflection (episodic→lesson) is the same engine as P5 case-law and is **gated
on landed** (Voyager self-verification). The claude-mem lesson holds: local embeddings kill *compression* cost,
not *injection* cost — discipline (top-k + progressive disclosure + scoping) is the only fix.

**KB grounding (gate):** memory retrieval is a KB-covered domain (RAG). Before the memory slice: `search` the KB
(`production-rag-guide`) for chunking / hybrid-fusion / eval-harness prior art and record one line per the gate.
Telemetry itself (structured logging + SQLite/JSONL) is not a KB domain → grounding skipped for §4.

---

## 6. The circularity trap (a constraint on the audit view, not on capture)

If the same strong model both **generates** the gold trajectories and **judges** them, the eval confirms itself.
Constraint for the eval/audit view (downstream of this slice): **lead with objective gate outcomes** (landed?,
mutation score?, rework count?) as ground truth; use the model-judge **only** for what no gate sees (was the plan
sane?, is the code clean?), never as the sole signal where a gate exists. Same lesson as coverage-vs-mutation:
don't let a soft proxy stand in for a hard signal that already exists.

**Review correction (§8):** the original claim that circularity is "a constraint on the audit view, not on
capture" was **wrong**. The stated horizon is to *train* a local model; if teacher-generated (anthropic)
trajectories train it and the gates are tuned to what the teacher does, the *training objective* is circular
regardless of who judges — you'd measure imitation of the teacher and mistake it for capability. The fix is a
**capture** constraint: every run records `provider`/`model` (§2a) so the corpus can be partitioned teacher-vs-local
and never silently mixed. The objective gates remain the only non-circular anchor — which is also why §8 insists
they not be the *sole, sparse* reward.

---

## 7. Open questions that drove the adversarial review (now ANSWERED in §8)

> These were the pre-review attack targets. Each is resolved in §8 — kept here for traceability.


1. **Hybrid storage** — is SQLite-meta + JSONL-trajectory the right split, or does a single source of truth win?
   Failure modes: two-sources drift, partial JSONL on crash, `trajectory_path` dangling after `rework`/worktree
   cleanup (note: worktree removal must NOT delete `.harness/runs/`).
2. **Atomic unit** — is per-step (with tool results) necessary, or is the final message array enough? RL/reflection
   want the observation stream; audit may want less. Over/under-capture?
3. **Counter semantics** — `attempt` vs `run_id` vs reserved `group_id`: is this the right factoring, or does
   conflating any pair bite the coordinator/GRPO slice later?
4. **Labels sufficiency/bias** — are terminal_status + gate verdicts + rework/bounce counts + tokens a *sufficient
   and unbiased* outcome record for a future reward `f`? What objective signal is missing (wall-clock? edit churn?
   diff size? human override?)?
5. **Crash/hang capture** — is "JSONL present, no run row" a robust crash label, or do we need an explicit
   heartbeat / `started` row written up front?
6. **Privacy/footprint** — trajectories embed full file contents and bash output; `.harness/runs/` growth and any
   secret-capture risk (e.g. the oMLX key in env dumps). Retention/rotation policy needed now or later?
7. **Wu Wei** — is any of this premature given current run volume? The counter-argument: retrofitting loses banked
   trajectories and the schema is sticky. Is "build telemetry before the pillars generate data" actually justified?

---

## 8. Adversarial review findings (of record)

Four independent reviewers, distinct lenses — **schema-durability**, **runtime-failure**, **RL-data-validity**,
**Wu-Wei-scope** — each instructed to refute, not rubber-stamp. **Unanimous verdict: NOT safe to build §4 as
originally written.** §4 above is the corrected scope; this section is the reasoning of record.

### The keystone (M1) and the tension it dissolves
The doc's "design once, never migrate" premise was false: the codebase has **no migration tooling**, only
`CREATE TABLE IF NOT EXISTS`, which **silently no-ops on a column add**. "We won't migrate" actually meant "we
*can't*." The RL lens therefore (correctly, *under that assumption*) flagged ~7 "capture-now-or-lose-it" gaps; the
Wu-Wei lens (correctly) said build ~45 lines not a data platform. **A `PRAGMA user_version` migrator resolves the
contradiction:** with online `ALTER TABLE ADD COLUMN`, deferral is safe, so the lean core wins for *aggregate
columns*, and only the genuinely unrecoverable set — **trajectory content + per-run provenance + crash
observability** — must be captured now. The Wu-Wei "dump the in-memory `messages` array" already captures content,
the frozen prompt (`messages[0..2]`), and the tool-error stream for free, shrinking "now-or-never" to almost nothing.

### Must-fix before the slice (each ≥2 lenses or unrecoverable)
- **M1 — `user_version` migrator** in `schema::init`. The pin; without it every deferral is permanent loss.
- **M2 — write the `run` row at loop ENTRY, UPDATE on exit** (schema + runtime, independently). "No row = crash" is
  provably ambiguous with *in-progress* and *lost-write*; `ended_at IS NULL` + stale is the robust, queryable label.
- **M3 — WAL + `busy_timeout`** in `schema::init`; each Recorder its own `Connection`. Else the coordinator (the
  doc's own target) hits `SQLITE_BUSY` on the 2nd parallel writer and silently drops rows → false-crash labels.
- **M4 — `run_id` as a sortable PK**, minted at entry. The one PK choice that *is* sticky (no tooling to redo it);
  Wu-Wei conceded this is worth doing now even as it cut `group_id`.
- **M5 — derive the trajectory path, don't store it; resolve it root-relative.** Mirrors `git::worktree_path`
  (pure-fn-of-id, the codebase grain). The N1 trap: the loop's cwd is the *worktree*, which `land` force-deletes —
  a cwd-relative runs dir means **land silently deletes every successful trajectory and keeps only failures.** Test
  the invariant (file survives land).

### Doc corrections folded in
- **S3 — `stop_reason` derived from observed loop state.** `tool_error` is unreachable (tools feed errors back as
  results, never terminate the loop); `killed`/`provider_error` can't be self-written (a dead/erroring run runs no
  exit code) → written by a reaper from the entry row. Enum trimmed accordingly.
- **N2 — drop `terminal_status`.** Always `in_progress` at loop exit and goes stale; the board is the source of truth.
- **S1 — drop tokens for now.** `provider::Response` discards the wire `usage` object → the columns would be 100%
  NULL. Add via `ALTER` once `Response` carries usage.
- **S3 — lenient JSONL reader.** The crash design intends a torn final line; the reader must drop/log it, never
  fail the file. Tested with a torn-tail fixture.
- **M8 — secret redaction at the write boundary.** Trajectories capture bash stdout verbatim and are destined for
  training; the oMLX key / env dumps must be masked at write time (unredactable after baking). Pulled forward
  despite the personal-box context because the *stated goal* is to consume this data for RL.
- **M6/S4 — Recorder failure non-fatal but loud.** Telemetry must never crash real work; a write failure downgrades
  to a metadata-only row + stderr warning.

### Resolved RL-vs-Wu-Wei split (the deferral list, safe via M1)
- **Capture now (unrecoverable):** `messages`-array dump; run-level `model`/`provider`/`sampling` (provenance — also
  the non-circular training-split key, §6); entry/exit timestamps.
- **Defer (nullable `ALTER` when the pillar lands):** `group_id` + frozen-prompt snapshot; `split`/`parent_run_id`
  (dedup/leakage guard); per-step provider/model/sampling (run-level suffices until roles mix providers); tokens;
  `agent runs`/`agent trace` CLI; per-step streaming/flush.
- **Derive at consumption (don't capture):** diff-churn (`git diff --numstat`), tool-error counts (in the dump),
  wall-clock (from the timestamps).

### Flagged, accepted for now
- The binary gates (`tests_green`, `landed`) are a **sparse, gameable** reward (`tests_green` = success of an
  agent-authored bash string; `squash_merge` already refuses empty lands, closing the empty-diff hole but not the
  gamed-validator hole). Not a telemetry-slice fix — but when the reward `f` is built, it must use the dense
  covariates (churn, tool-errors, validation-cmd hash) and partition by provenance, **not** the two bits alone.
- `temperature: 0.3` is near-greedy → low intra-group variance; matters for GRPO, hence `sampling` is captured so a
  group can later be confirmed sampled hot enough. Tuning is a coordinator-slice concern.

### Wu-Wei "is this premature?" — resolved
Zero runs exist today. The **maximalist** (RL data-platform) version *would* be premature; the **lean** version is
not — it's ~60 lines, matches the existing schema bar, and is the substrate the eval/audit goal Gary explicitly
asked for needs. Build lean now; the migrator means we forfeit nothing we can backfill.
