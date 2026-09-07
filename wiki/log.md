# Wiki Log

## [2026-06-08] session | Wiki init + foundation eval scoped
- Initialized the project wiki (index/architecture/decisions/active-work/log) per the dev skill wiki-protocol.
  Wiki is the navigable memory layer over the detailed `research/` docs.
- Project state at init: pure-instruction `dev` skill → being re-platformed as a Pi-based (now possibly
  Rust-native) personal harness.
- Prior work this arc (all committed under `research/`):
  - Six breadth research tracks (memory, orchestration spine, test-hardening, self-evolution, parallel
    exploration, Pi internals) → `01..06`.
  - Design v2 — all six v1 open questions resolved → `00-design-v2.md`.
  - Pi spike — cloned pi-mono, wired oMLX, wrote a `tool_call` Align gate, **validated at runtime** (blocks
    write/bash in align phase; `/align` unlocks; phase persists across reload) → `07-pi-spike.md`.
  - Symphony Elixir teardown (coordinator mechanics) → `08-symphony-teardown.md`.
  - Oriented 13 external references; verdicts → `09-additions-orientation-and-plan.md`.
- **Direction shift this session:** Gary set the harness as **Rust-native** (forever-personal, local-first,
  KB MCP already Rust). This reframes the external refs (INTEGRATE→STEAL) and makes oh-my-pi a Rust reference
  + beads a schema to extract. Scoped a rigorous foundation eval (DR1 omp / DR2 beads) → `10-eval-scope.md`.
- Added project `CLAUDE.md` (+ `AGENTS.md` symlink) — the entry-point map + conventions (read wiki first,
  keep it updated, Align-before-execute, integrity constraints, commit discipline).
- **Next:** run DR1 + DR2, synthesize the language/runtime decision (`research/11`), then Phase 0.

## [2026-06-08] session | Foundation eval complete — Strategy A locked (signed off)
- Ran DR1 (oh-my-pi → `research/11`), DR2 (beads → `research/12`), DR2-prime (beads_rust/`br` → `research/15`);
  synthesized `research/13`; cataloged the author constellation `research/14` (Jeffrey Emanuel's Rust agent-flywheel).
- **Decision (signed off): Strategy A — full Rust-native own-core.** Lift `crates/pi-iso`; board on `rusqlite`
  (SQLite + JSONL) porting `br`'s `close_policy.rs` state-machine + gate engine (~250 lines); minimal Rust
  provider layer; Phase 0 opens with a sizing spike. B rejected (omp's agent brain is all TS), adopt-`br`
  rejected (welded to fsqlite-alpha + nightly + 180k LoC).
- Flipped `wiki/decisions.md` (OPEN→RESOLVED; rejected omp-as-base and beads/br/repowise-as-runtime-deps);
  rewrote design-v2 §2 (rows 1–3, 6) + §10 to the Rust-native foundation.
- **Next:** Gary clarifying 2 things before Phase 0 — the sizing spike is **held**, not kicked off.

## [2026-06-09] session | Phase 0 complete + spike graduated to this repo
- **Two pre-work decisions (pushback-and-teach):** (1) "always hard-load the dev skill" — **declined**, the
  always-on rules already live in always-loaded `CLAUDE.md`; mode playbooks load on-demand (correct lazy
  pattern). (2) "set up `cass_memory_system` now" — **declined as a dep**, it's TS+Bun + a required `cass`
  CLI (hits Rust-native/depend-on-none) and memory is **P3, not the current phase**. Marked it the priority
  **P3 design-steal** target (ACE pipeline w/ no-LLM Curator, 90-day-half-life decay + 4× harm multiplier,
  anti-pattern inversion) — breadcrumb in `active-work.md`.
- **Graduated the Phase 0 spike into this repo.** `harness-rs/` (scratch sibling) → `crates/{provider,agent,
  pi-iso}` here; wiki + research travelled; `.gitignore` covers target/*.db/.DS_Store/.claude; initial commit
  `e9c48a3` ("graduate harness from Phase 0 spike"); README rewritten to the graduated layout. **Verified
  sound:** `cargo build --workspace` green in the new home, 48 files tracked, no stranded artifacts. The
  graduation was already physically done at repo-init; this session confirmed integrity + reconciled the wiki.
- **Next:** build the trunk — board on rusqlite + ported `close_policy.rs` gate engine + workpad + land
  (design-v2 §12).

## [2026-06-09] session | Trunk slice 1 built — board + gate engine (adversarially reviewed)
- **Dogfooded the Align gate on our own trunk work.** Opened a trunk Align brief (slice 1 = the board, since
  everything hangs off it), pulled design-v2 §4/§12 + the `br` `close_policy.rs` port notes (research/15 §2).
  Pushback caught two pre-build issues: §12's "file board / Pi scaffold" language is **superseded** by Strategy A
  (board on rusqlite); §14's "board format: md vs SQLite" fork is **already resolved** by Strategy A.
- **Ran a focused 3-critic adversarial pass** (scoped first: targets = schema / state-machine / gate-engine;
  out-of-scope = settled foundation + later slices; output contract = severity-tagged findings, no manufactured
  ones). All three verdicts **needs-rework-before-build**. Decorrelation paid: critics C1 and C3 **independently**
  found the same blocker (stale gate-pass on rework). Survivors triaged → must-fix / seam / logged-known-hole.
- **Built `crates/board`** with the corrections baked in: `model` (Status spine + GateSource + edge-blocking
  predicate), `schema` (4 tables — ticket/edge/gate_results/event; attempt-epoch + kind-in-edge-PK + source col
  + append-only events), `spine` (transition map with back-edges + `validate_transition` + `required_gates_for`
  + `gate_source_required`), `board` (`set_status` chokepoint: validate → gate-check-at-current-attempt → bump
  attempt on rework → audit event; `report_gate` with human-source guard; non-recursive `ready` + dangling lint).
  ~370 logic LoC. **Reversed my own CTE recommendation** → direct-blocker `NOT EXISTS` (cycle-safe by
  construction; transitive cache deferred).
- **Evidence:** 6 unit tests green, `cargo test --workspace` green (1 ignored = live-oMLX), `cargo clippy -p
  board` clean. Slice-1 enforces only the human `align→in_progress` gate; verify/review/land = ungated stubs.
- **Wiki:** decisions.md gained "Trunk slice 1 — BUILT" + the CTE rejection; active-work checklist now tracks
  slices 2–5; known-holes (repro-signal, mutation gate, review/min_reviewers applicability, per-kind profiles,
  worktree assertion, transitive blocked-cache) logged, not dropped.
- **Next:** slice 2 (workpad rendering) or slice 3 (wire the spine into the agent loop, retiring the 2-phase toggle).

## [2026-06-09] session | Trunk slice 3 built — spine wired into the agent loop
- **Scoped first (Align discipline).** Reviewed slice-1 state, read the live loop (`main.rs`/`gate.rs`/`tools.rs`)
  + the board spine, then presented a scoped slice-3 plan and confirmed two forks with Gary: **post-gate-only run**
  and **force-tickets** (both his pick). Fence: rich workpad render (s2), land (s4), worktree↔board (s5), agent
  self-transitions, verify/review gates — all out.
- **Retired Phase 0's standalone toggle.** Deleted `enum Phase` + `PhaseStore`. The tool gate is now derived from
  a board ticket's spine status (`gate::mutating_allowed`): mutating tools unlock only in `in_progress`..`land`.
  The human `/align` is the board's `criteria_confirmed` transition; since mutation is locked until `in_progress`,
  the human gate transitively gates mutation — same Phase-0 property, ticket-scoped. Tool policy kept in `agent`
  (board stays tool-agnostic — protocol boundary).
- **Board-backed CLI:** new / plan / criteria / align / run / ready / status / rework / iso. `agent run` refuses
  non-execution status (post-gate-only); `agent align` refuses empty criteria (gate by an artifact); chains
  todo→align→in_progress in one command. Added `Board::set_plan`/`set_acceptance_criteria`/`max_ticket_seq`;
  dropped now-unused `rusqlite` from agent deps.
- **Evidence:** 8 unit tests (gate maps over the whole spine; human gate flips deny→allow at align→in_progress
  and rework re-locks), `cargo test --workspace` green, clippy clean, `Phase`/`PhaseStore` grep-clean, and a full
  **CLI smoke test** + **live oMLX run** — ticket t1 → set criteria → align → `agent run` fired `write_file`
  post-gate, `hello.txt` written with exact content; rework re-locked `run`.
- **Not committed yet** (Gary's call). **Next:** slice 2 (workpad rendering) or slice 4 (land).

## [2026-06-09] session | Trunk slice 4 built — closed the loop (Verify + Land)
- **Scoped first (Align discipline).** Reviewed progress (s1+s3 done; ticket stuck at in_progress, back half
  ungated), read design-v2 §4/§4.1/§10, confirmed one fork with Gary: **land = "committed + green on current
  tree"**, deferring the real worktree squash-merge to s5. KB grounding gate: domain not covered (git + state-
  machine glue) → skipped honestly.
- **Two new artifact-backed gates** in `required_gates_for`: `tests_green` (Machine) on `verify→review`,
  `landed` (Human) on `land→done`. `gate_source_required` now `criteria_confirmed|landed → Human`.
- **`agent verify`** runs the ticket's validation command (finally uses the `validation` column), records the
  exit status as the tests_green artifact, advances to review on green or **bounces verify→in_progress on red**
  (the §4 back-edge). **`agent land`** enforces the §10 keystone — refuses a dirty tree ("git commit =
  approval"), records HEAD sha, advances review→land→done. Git lives in `agent`, board stays VCS-agnostic.
  Added `Board::set_validation`; updated the slice-1 `ready` test (it walked the back half ungated).
- **Keystone holds:** an agent (read/write/bash) can `git commit` but cannot run `agent land` nor write a Human
  gate → can't self-advance to Done; only the operator lands.
- **Evidence:** 12 unit tests (7 board + 5 agent — gates enforced, machine source rejected for `landed`, rework
  re-locks via attempt-epoch, verify green-advances/red-bounces, land refuses-dirty/lands-clean in a throwaway
  git repo), `cargo test --workspace` green, clippy clean, and a **live oMLX full-spine run**: new→criteria→
  validation→align→run(wrote hello.txt)→verify(green→review)→land(dirty REFUSED→commit→clean→done). Event trail:
  created→workpad_edited×2→align→criteria_confirmed→in_progress→verify→tests_green→review→land→landed→done.
- **Not committed yet** (Gary's call). **Next:** slice 2 (workpad rendering) or slice 5 (worktree workers + real squash-merge land).

## [2026-06-09] session | Trunk slice 5 built — worktree-isolated workers + real squash-merge land
- **Scoped first (Align discipline).** Reviewed progress (s1/s3/s4 done; isolation still the Phase-0 seed-tree
  stub; s4 land was a placeholder clean-tree check). Read pi-iso's API + the current `run_isolated` glue + design
  §3 (execution plane) / §4.1 (land). KB grounding gate: git-worktree mechanics, not a KB domain → skipped.
- **Confirmed one fork: plain `git worktree`, NOT pi-iso** (Gary's pick). Land = squash-merge needs branch/base
  control + a shared object store; pi-iso is built for run→extract-diff→discard, not branch-promote. pi-iso stays
  a workspace crate (CoW/diff primitive for later); dropped from `agent` deps. A choice within Strategy A, not a
  reversal of "lift pi-iso."
- **New `crates/agent/src/git.rs`** (~150 lines, plain `std::process::Command`): repo_root / worktree_path
  (gitignored `.harness/worktrees/<id>`, pure fn of id → no schema change) / branch_for (`harness/<id>`) /
  ensure_worktree (idempotent) / commit_worktree (WIP, no-op if clean) / squash_merge (refuses dirty base,
  aborts+restores on conflict, refuses empty land, returns squash sha) / remove_worktree (worktree then branch -D).
- **`agent run` is isolation-by-default** (loop in the worktree → commit to branch); removed the fake-seed `iso`
  command + `run_isolated`. `verify` runs validation in the worktree; `land` squash-merges branch → base (the
  squash commit = operator approval, the §10 keystone, stronger than s4's clean-tree) + removes worktree+branch;
  `rework` cleans up (§4 fresh-branch). Added `.harness/` to `.gitignore`.
- **Keystone holds:** agent commits only to its throwaway branch (never base); only the operator runs `agent land`.
- **Evidence:** 17 unit tests (7 board + 10 agent — git.rs: isolation→one-commit squash-land→cleanup, rework
  cleanup, dirty-base refused; main.rs: run_land composition), `cargo test --workspace` green, clippy clean, and a
  **live oMLX worktree-isolated run**: run wrote hello.txt in `harness/t1` (absent from main during run — isolation
  proven), verify green in the worktree → review, land squash-merged → main got the file as one commit (702bca1 on
  init b6c3f3d), worktree+branch removed.
- **The trunk's execution path is complete.** Committed `6137f29` (8 files, +396/−94, `git.rs` new).

## [2026-06-09] session | Trunk slice 2 built — the §5 workpad contract; THE TRUNK IS COMPLETE
- **Scoped first (Align discipline).** Reviewed progress (s1/s3/s4/s5 done; columns existed but no canonical
  render — run_ticket hand-built a partial workpad showing only Plan+Acceptance, no `show`, no notes/confusions
  setters, the InProgress→Align back-edge unused). Read design §5 (workpad) + §4 (back-edges). KB grounding:
  markdown render + CLI glue, not a KB domain → skipped.
- **Confirmed one fork: Notes/Confusions OVERWRITE** (not append-with-timestamp) — consistent with the existing
  setters, no schema change; append is a cheap later refinement.
- **One canonical renderer** `board/src/workpad.rs::render(&Ticket, header)` — the §5 markdown (fenced header +
  Plan/Acceptance/Validation/Notes/Confusions, all five always present, empty → `_(none)_`). Pure, no deps; lives in
  `board` (the contract) with the host/VCS header injected (board stays VCS-agnostic). Wired into BOTH `agent show`
  and `run_ticket`'s system prompt (`build_system_prompt`) → single source of truth, no human/agent drift.
- **Added** `Board::set_notes`/`set_confusions` + CLI `note`/`confusion`/`show`; `git::short_sha` for the header
  (host via `hostname`, path = worktree if present else repo root).
- **Confusions→Align channel** (`raise_confusion`): a confusion on an `in_progress` ticket records it AND bounces
  InProgress→Align, re-locking tools until the operator re-aligns. No attempt-epoch hack — only the operator's
  `agent align` advances Align→InProgress, so the park-at-Align already enforces the re-opened human gate.
- **Evidence:** 21 unit tests (9 board + 12 agent — board: render order/header/3-placeholders, notes+confusions
  persist+audit; agent: confusion bounce flips the tool gate + todo-just-records, system_prompt embeds the render),
  `cargo test --workspace` green, clippy clean, **live `agent show`** (full §5 render, header
  `<host>:…@ecd7bdc`) + **confusion bounce** (in_progress→align, show reflects both).
- **THE TRUNK IS COMPLETE (s1–s5).** **Not committed yet** (Gary's call). **Next: the pillars** — memory plane
  (sidecar + wiki retrieval on oMLX), the §7 Harden/mutation gate, parallel exploration (coordinator + claim/lease
  FSM), self-evolution — per design-v2 §12.

## [2026-06-09] session | Telemetry Unit A built (prod) + research/17 adversarial review + a sandbox-escape finding from the oMLX dogfood
- **Adversarial review of `research/17` (telemetry plan) → §4 revised** (committed `cf795a5`): 4-lens find→refute
  (schema-durability / runtime / RL-validity / Wu-Wei), unanimous "not safe as written." Keystone: build a
  `user_version` migrator FIRST — the codebase had none (`CREATE TABLE IF NOT EXISTS` silently no-ops on a column
  add, so "design once, never migrate" was really "can't migrate"). Folded into §2a/§3/§4/§6 + new §8.
- **Dogfood exercise (Gary's ask): let the local oMLX 35B attempt Unit A as an *exercise only*, not prod.** First
  run (8-iter/1024-tok defaults): 7 read_file, 0 write_file — exhausted budget on exploration, never reached the
  write phase. Its *plan* was correct. → made the loop budget env-tunable (`HARNESS_MAX_ITERS`/`HARNESS_MAX_TOKENS`,
  defaults unchanged) and re-ran at 100/8192 via a background subagent.
- **Two incidents:**
  1. **write_file SANDBOX ESCAPE (the real find).** The oMLX agent's writes landed in the MAIN tree, not its
     worktree — `tools.rs:61 cwd.join(path)` discards the base on an absolute path, and the workpad header leaks
     the absolute repo root into the prompt. An agent can write arbitrary files outside its sandbox (isolation +
     security). Caught, killed (exit 144), restored; nothing committed/landed. → hardening ticket **t4**, fix
     **before** any further agent exercises.
  2. **Concurrency collision.** The background subagent ran `git stash` on the shared tree while I was editing →
     it swallowed my prod edits (recovered from the stash). Lesson: never let a background agent mutate shared git
     state while the parent works in it.
- **Built prod Unit A** (Claude, "the better harness"): `schema.rs` user_version migrator + WAL/busy_timeout +
  `run` table as a TRUE migration v1; `model.rs` `Run`; `board.rs` start_run/finish_run/runs_for. Run row written
  at entry so a crash is the queryable `ended_at IS NULL`; sortable caller-minted `run_id` PK; full §8 provenance
  (model/provider/sampling/project/attempt).
- **Comparison (oMLX vs prod):** local 35B got ~50% — idiomatic schema.rs + clean `Run`, but put the run table in
  the base SCHEMA (no-op migration), minimal schema missing every §8 provenance column, never wrote board.rs, and
  its test file had compile-breaking quote-escaping typos. Notably **no oMLX/context errors even at the 8× budget**.
  Attempt preserved at `/tmp/omlx-unitA-attempt/`. Decisive blocker was the harness's own write_file bug + the kill,
  not raw model capability.
- **Evidence:** 23 unit tests (11 board incl. run-lifecycle + crash-label + the v0→v1 migrator-upgrade proof; 10
  agent; 2 pi-iso), `cargo test --workspace` green (1 ignored = live oMLX), clippy clean.
- **Next:** fix **t4** (sandbox confinement) before more exercises, then **Unit B** (Recorder + loop wiring +
  lenient reader; t2 spec'd). Exercise tickets: t3 left in `review` (false positive — verify passed on a pristine
  worktree); t1 holds the first-exercise breadcrumb.

## [2026-06-09] session | t4 — sandbox confinement (write_file path bug fixed)
- **Built t4 directly** (the hardening ticket from the dogfood escape). `crates/agent/src/tools.rs`: new `confine`
  resolves the model's path against the worktree `cwd` and REJECTS escapes — absolute paths outright; relative paths
  are joined under the canonical cwd, lexically normalized (`normalize_lexical`), and required to `starts_with` it
  (so `../` climbing out is caught). Applied to **both** `read_file` and `write_file`. `bash` left cwd-scoped with a
  doc note that it is NOT a hard sandbox (can `cd`/abs-path; OS-level isolation deferred).
- **De-leaked the prompt:** `main.rs::workpad_header` now renders a repo-relative path via `repo_relative`
  (`harness/.harness/worktrees/<id>`) instead of the absolute root — so the model isn't handed the escape route.
  Defence-in-depth; the tools are confined regardless.
- **Evidence:** 3 new regression tests (confine allows-in/rejects-escape; write_file refuses absolute + `../` and
  **leaves nothing outside the worktree**; read_file refuses an absolute read of a real outside file) → 13 agent
  tests; `cargo test --workspace` green (1 ignored = live oMLX); clippy clean. Live: `agent show` header is now
  `<host>:harness@<sha>` with no home-absolute-path leak.
- **Next:** Unit B (Recorder + loop wiring + lenient reader; t2 spec'd) — agent exercises are safe to resume.

## [2026-06-09] session | Telemetry Unit B built (prod) — trajectory recorder + run wiring + `truncated` taxonomy
- **The agent half of the telemetry plane.** `crates/agent/src/recorder.rs` (NEW): `Recorder` appends one redacted
  JSON line per `Message` (built via `serde_json::json!` — provider types don't derive `Serialize`), flushed per
  write; **best-effort** (open-fail → disabled recorder, write-fail → self-disables; the run never blocks on
  telemetry). `read_trajectory` is **lenient** — drops a torn FINAL line (crash mid-append), errors on interior
  corruption. `clean()` redacts the oMLX key (§8 M8) + caps fields at a UTF-8 boundary.
- **`git::runs_path(root, ticket, run_id)`** → `<root>/.harness/runs/<ticket>/<run_id>.jsonl` — root-relative, a
  sibling of `worktrees/`, so `land` removing the worktree can't delete the record (**N1**).
- **Loop wiring** (`main.rs::run_ticket`, now `+root`): mint sortable `run_id` (`<id>-<attempt:03>-<millis>`),
  `start_run` + open Recorder at entry, record seed + every assistant/tool-result message, `finish_run` on **every**
  exit (provider error closes the row `error` inline before propagating, so no orphan open runs).
- **The `truncated` keystone** (from the 2026-06-09 forensic finding): `classify_stop(last, hit_max)` derives the
  run outcome — a final response with `StopReason::Length` is **`truncated`**, NOT `completed`; `End`→`completed`;
  exhausted loop→`max_iters`. Stops a budget-cut no-op from masquerading as success (which would poison any
  finetune/RL signal on the labels).
- **`agent trajectory <id>`** (NEW command): the telemetry read path — `runs_for` → derive path → `read_trajectory`,
  prints the run row + a per-step preview. (Also gives `read_trajectory` a real prod caller, no `#[allow(dead_code)]`.)
- **Evidence:** **33 tests** workspace-green (agent 13→20: +4 recorder, +`runs_path`/N1, +`classify_stop`,
  +`trajectory_survives_land`; board 11; provider 2 +1 ignored live-oMLX), clippy clean. **Live oMLX forced-truncation
  test** (Gary's chosen small-budget method — `HARNESS_MAX_TOKENS=64`): run row = `truncated`, `iters=1`, `ended_at`
  set; trajectory written at the N1 path; **oMLX key absent on disk** (live redaction); read back via `agent
  trajectory`. Throwaway ticket/worktree/db cleaned up; main tree carries only the prod diff.
- **Why small-budget, not 128k:** truncation is the thing under test — a bigger budget makes it *less* likely. And
  128k was never available: oMLX is capped at 32K total context, and `HARNESS_MAX_TOKENS` is the *output* cap, not
  the context window. (The "does room let the 35B finish?" capability probe is a separate, deferred question.)
- **Next:** Unit B is the telemetry plane's MVP. Remaining §-items (M-numbers in research/17): richer per-step
  fields, the memory/index half, retention. Then the other pillars (Harden gate §7 / parallel exploration / self-evolution).

## [2026-06-10] session | Memory pillar — design + adv review + Slice A (lexical sidecar) built
- **Design + 4-reviewer adversarial review** → `research/18-memory-plane.md` (CoALA lens: sidecar=episodic/decays,
  wiki=semantic+procedural/never-decays, workpad=working memory). KB-grounded (hybrid+RRF k=60, metadata
  pre-filter, eval-on-own-data). 4 decorrelated cold reviewers (retrieval-quality / schema-durability /
  reflection-validity / Wu-Wei) converged; verdict folded into §10. **Decisive Wu-Wei cut: defer the vector leg AND
  reflection AND the retention curve until MEASURED insufficient** → sliced: **A=lexical sidecar (build now),
  B=vector (gated on golden-set eval), C=decay+reflection (gated on A earning its keep)**; the v2 schema carries
  ALL of B/C's columns so deferred slices need no table rebuild.
- **Slice A built** (`crates/board`): migration **v2** = `memory` table (full durable column set incl.
  embedding/embed_model/embed_dim, proof_count, promotion_state, reflected_at, evicted_at) + `memory_fts` FTS5
  **external-content** index (`content='memory'`, `content_rowid='rowid'`) + 3 explicit sync triggers (ai/ad/au,
  `'delete'`-command form). `model.rs` `Scope` enum (CHECK-enforced) + `MemoryHit` (id/title/type, progressive
  disclosure). `memory.rs` (NEW): `remember` / `recall` / `recall_body`.
- **Decisions of record:** (a) **FTS5 ships in rusqlite's `bundled` SQLite — no cargo feature** (the `fts5` feature
  is the unrelated Rust-tokenizer API; tried→reverted). (b) **Type validated in Rust, not a schema CHECK** (P5
  case-law will add types); **scope keeps its CHECK**. (c) **`recall` = BM25 only** (`ORDER BY bm25 ASC, salience
  DESC`), project/scope **pre-filter in WHERE**, `evicted_at IS NULL`, no embeddings/fusion/dedup/reflection. (d)
  **Memory ops are self-contained — they do NOT write the ticket `event` table** (its issue_id is NOT NULL +
  project/global memories have no ticket; Wu-Wei + decoupling — a conscious deviation from §10's "audit events").
  (e) **soft-delete** (`evicted_at`) over hard delete preserves provenance. (f) `fts_query` sanitizes tokens
  (quote-wrap, drop punctuation-only) so paths/`::`/operators can't throw FTS syntax errors.
- **Evidence:** workspace green (agent 20 / board 19 / pi-iso 2 / provider 1-ignored), 8 new memory tests + clippy
  clean; **live CLI smoke** (`remember` ×3 → `recall` project-prefilters to 2 harness rows BM25-ordered, `other`
  project appears only unfiltered → `recall-body` returns the right body, bumps usage). CLI: `remember`/`recall`/
  `recall-body`.
- **Next:** Harden gate pillar (design-v2 §7 mutation testing — **adv-review-EXEMPT**), then parallel exploration, then self-evolution.

## [2026-06-10] session | Harden pillar built — §7 mutation gate (review-exempt)
- **Test quality is now a number on the diff, not a vibe.** New spine gate `GATE_MUTATION = "mutation_score"`
  (Machine — an agent provider can satisfy it; it's a computed number, not a human call). `required_gates_for(Verify,
  Review, kind)` → `[tests_green, mutation_score]` for code kinds, `[tests_green]` otherwise, keyed off new
  `kind_is_code(kind) = matches!(kind, "build"|"bugfix"|"refactor")`. **Conservative on purpose:** only KNOWN code
  kinds gate, so a novel kind can't silently acquire an un-runnable mutation gate that wedges its tickets in Verify.
- **`agent harden <id>`** runs **cargo-mutants** (`cargo install cargo-mutants --locked`, v27.1.0) as a **dev-tool
  subprocess** (like git/clippy — NOT a linked crate dep): `commit_worktree` to capture stragglers → `diff
  base...HEAD` → `cargo mutants --in-diff <diff> --output <tmp>` in the worktree → parse `mutants.out/outcomes.json`
  top-level ints → report the gate. cargo-mutants **exits NON-ZERO when mutants survive** → judged by presence of
  `outcomes.json`, not exit status (a finding ≠ a tool failure).
- **Score = `caught / (caught + missed + timeout)`** (the recorded formula): `unviable` excluded (≈ equivalent
  mutants); **timeouts count AGAINST** (conservative — surfaces them); empty denom → vacuous **1.0**, never NaN.
  Threshold `HARNESS_MUTATION_THRESHOLD` default **0.70** (coach-not-gatekeeper; ratchet up over time, never to 100%).
- **`run_verify` refuses a not-yet-hardened code ticket UP FRONT** (while still InProgress, via new public
  `Board::gate_satisfied`) → natural order is **`run → harden → verify`** (verify can't re-enter Verify, so
  stranding it there would wedge it). Board: extracted `gate_satisfied(id, gate)` (attempt-epoch + source-aware) and
  refactored `missing_gates` onto it (gate by an artifact, not a caught error string). Git: `base_branch` +
  `diff_against` helpers.
- **Dogfooded the real cargo-mutants pipeline end-to-end** (the one thing unit tests can't cover): first run flagged
  **2 real survivors (0.75)** — both mutations made `kind_is_code` always-true; no test pinned the negative case.
  Added `harden_gate_is_kind_gated` (pins BOTH arms — code → 2 gates, non-code → 1) → re-ran **8/8 caught (1.0)**.
  The gate working as designed; treated as a genuine ratchet, not a nuisance. Confirmed `outcomes.json` key names
  (`caught`/`missed`/`timeout`/`unviable`/`total_mutants`) against live tool output.
- **Evidence:** board 20 / agent 21 green, clippy clean, live mutation run. fmt deliberately skipped (project = hard
  tabs, no rustfmt.toml → `cargo fmt --check` flags the whole repo = a non-signal; my code matches surrounding tabs).
- **No subagent fan-out** (Wu-Wei — ~150-line cohesive slice). Genuine coordination patterns belong to the
  parallel-exploration pillar, not manufactured here. Per the Process-gate decision, **Harden is adv-review-EXEMPT**
  (the gate is a number, not a hard-to-reverse / silent-failure design).
- **Next:** parallel exploration pillar (design-v2 §8 — design + adv review + execute), then self-evolution (§9).

## [2026-06-10] session | Parallel-exploration pillar built — the PROBE (design + adv review + execute)
- **The cathedral got cut to an instrument.** Original §8 design = fan-out → reap → judge → consolidate → land.
  Adversarial review (silent-failure risk class → review REQUIRED per Process gate) found a BLOCKER + 7 issues and
  scoped it down to a **probe** that produces *evidence*, not a spine mutation: fan out N *diverse* workers across
  isolated worktrees, run each worker's validation in its own tree, rank survivors by **objective signal only**, and
  report a ranked table + winner **candidate**. It **reaps nothing, consolidates nothing, never lands** — a human
  inspects/verifies/lands/cleans up. Full fold-in recorded in `research/19 §9` + `decisions.md`.
- **Review findings folded in (research/19 §9.1–§9.10):** (BLOCKER) auto-consolidation/auto-land removed — explore
  is read-only w.r.t. the spine. (§9.3) **reap-before-fanout**: coordinator reaps `<ticket>-w0..w{CEIL-1}` so a
  crashed prior run can't leave a contaminated worktree. (§9.2) **worker-label run_ids** (`<id>-w<k>`) so N runs
  don't collide in telemetry. (§9.6) **no pairwise LLM judge** — over-built before any evidence the objective gate
  is insufficient; rank by `(passed, produced-a-diff, index)`. (§9.7) **no planner trait / no "distinct-string"
  gate** — diversity is by construction (fixed strategy directives) and *read off the report's diff column*, real
  evidence not a string check. (§9.5) honest docs: workers run **sequentially over shared git**, not a true
  sandbox. (risk-6) a passing **no-op** sorts below real work (weak-validation guard).
- **The pure selection core is `crates/agent/src/explore.rs`** (no git, no model — "search logic is just code"):
  `FANOUT_CEILING=5`, `Stakes{Low,Default,High}`, `stakes_from_priority` (≤1→High/2→Default/≥3→Low),
  `fanout_for` (override clamped `[1,CEIL]`, never 0; else Low=2/Default=3/High=4), `default_strategies(n)` (5 fixed
  directives, take n), `BranchOutcome` value type, `rank_outcomes` (sort by `(!passed, lines==0, k)`). **5 unit
  tests** pin every seam without spawning anything.
- **The git+oMLX glue is `run_explore` in main.rs** (beside `run_ticket`): precondition (mutating + a validation
  cmd set) → reap-before-fanout → cohort (strategies define width, else `fanout_for`) → **sequential** worker loop
  (worktree `<id>-w<k>` → `run_ticket` with per-worker strategy directive → commit → `run_validation` in that tree →
  `lines_changed_against(base)`) → `rank_outcomes` → **reflection-on-kill** writes a `lesson` memory for failures →
  prints ranked table + winner candidate path/branch. `run_ticket` grew a `strategy: Option<&str>` + `run_label`
  (worker-label run_ids) and returns `RunSummary{iters,stop}`; `run_validation` extracted board-free (shared by
  `run_verify`). Git: `lines_changed_against(wt, base)` parses `git diff --numstat base...HEAD`.
- **Test hygiene fix:** the dev's global `commit.gpgsign=true` made a new git test's commit attempt GPG-sign and
  transiently fail (`Cannot allocate memory`). Added `git config commit.gpgsign false` to `init_repo` + both inline
  test repos — tests must not depend on the dev's GPG config.
- **Exercise gate PASSED with evidence** (oMLX, never landed): `agent explore t1 --fanout 3` on a fizzbuzz scratch
  ticket spawned 3 workers that **each produced a genuinely different implementation** (w0 inline `i%15==0`, 9 lines;
  w1 `def fizzbuzz(n)`+docstring+`__main__` guard, 16 lines; w2 inline `i%3==0 and i%5==0`, 9 lines), all PASSED
  validation, ranked correctly (w0 rank-1: passer+diff, lower-index tiebreak over w2), winner candidate reported,
  **nothing reaped/consolidated/landed**. Satisfies "≥2 branches that do different things"; the post-mortem path is
  in place (exercised only on failures — none here). Exercise artifacts cleaned up by hand (probe reaps nothing).
- **Evidence:** agent 27 / board 20 / pi-iso 2 green, clippy clean, live explore run. fmt skipped (hard tabs, no
  rustfmt.toml → non-signal). **Subagent coordination pattern = the probe itself** (fan-out diverse workers across
  isolated worktrees + objective ranking + reflection-on-kill) — the mandate's "coordination patterns" realized in
  the harness's own code rather than manufactured in this session's Claude-side process.
- **Next:** self-evolution pillar (§9 — design + KB grounding + adv review at **design AND pre-land** + execute).

## [2026-06-10] session | Self-evolution pillar built — Half A read-side (design + adv review ×2 + execute) — ALL PILLARS DONE
- **The open compounding loop, found and closed.** Design (`research/20-self-evolution-plan.md`) surfaced the core
  finding: the harness *writes* `lesson` episodes (explore's reflection-on-kill, `main.rs`) but **never reads them
  back** — `build_system_prompt` was built purely from the ticket workpad. The lessons compounded nowhere. Closing
  that loop safely IS the self-evolution pillar.
- **Design-stage adversarial review (3 parallel reviewers, diverse lenses) converged from three directions** that the
  originally-planned Half B (an `agent reflect` propose instrument + oMLX two-column distiller + K=3 critic panel +
  proposals dir + a 6-function pure filter core) should NOT be built:
  - **Wu-Wei (empty pond):** the only `lesson` writer emits *ticket-scoped, bound-specific* post-mortems (worker ids,
    digit counts, literal commands) — every one trips the generalization filter; **zero** general lessons exist to
    distill. The §7.5 exercise had to *hand-seed synthetic episodes* — when the evidence step must fabricate the
    instrument's input, the instrument is ahead of its data.
  - **Unsafe as specified:** the pure `screen` ran on the raw lesson text, but the human approves the **oMLX distill
    *output*** (different text) → injection sink. The "≥2 episodes" grounding floor is self-dealing — the agent's own
    auto-authored post-mortems clear it (research/04 wanted a baseline the agent *didn't* author).
  - **Incorrect:** `has_bound_specifics`/`is_vacuous` are surface-feature regexes that mis-verdict ~100% of real input
    both ways; the 5-const keyword gate-bypass detector is false safety (any paraphrase or no-gate-named integrity
    attack walks past a `gate_bypass:false` banner).
- **Decision: ship Half A only, hardened; defer Half B to a measured trigger** (≥~10 genuinely general lessons +
  manual curation becomes real toil). Third pillar cut to its load-bearing core (memory=Slice A, explore=probe,
  evolve=read-side). Full fold-in = research/20 §9.
- **Half A built** (`crates/agent/src/evolve.rs` pure core + `main.rs` glue):
  - `evolve.rs` (no git/model/clock, fully unit-tested — the explore.rs style): `Prepared{bullets, dropped_unsafe,
    dropped_unsafe_texts, dropped_budget}`; `mentions_gate_weakening(text, gates)` — flag/drop-only, requires BOTH a
    gate reference (const value / ≥4-char underscore token / generic "gate") AND a weakening verb; honestly NOT a
    "safe" certifier (false negatives by construction — human commit is the boundary); `prepare_caselaw(raw, gates,
    max)` — **section-aware** (harvests only `## Lessons` bullets, contract prose stays out), drops gate-weakening,
    budgets at `CASE_LAW_MAX_BULLETS=12`, **screen-before-budget** (an unsafe line can't burn a slot).
  - Glue: `load_case_law(root)` reads `wiki/case-law.md` from the **repo ROOT** (not cwd — workers run inside isolated
    worktrees; a cwd-relative read would silently find nothing and disable the pillar invisibly), **missing→one-line
    stderr note / empty→silent**, logs `injected N/M` + **echoes each dropped bullet's text** so a curator can reword.
  - `build_system_prompt(t, header, case_law)` appends `### Learned heuristics (case-law)` iff non-empty, **after**
    the workpad (the ticket contract stays primary; the prompt says so + a test pins the order).
  - `wiki/case-law.md` created: a human-approve-contract header + 8 genuinely-*general* hand-distilled lessons
    (Claude-in-loop distillation — §9.1 made concrete). The commit *is* the approval.
- **Pre-land adversarial review (3 reviewers: security / correctness+Wu-Wei / contract) = unanimous SHIP, zero
  BLOCKER/MAJOR.** Verified the two load-bearing properties (read resolves against repo root; screen is strictly
  flag/drop, never certifies safe) hold and are tested. Folded in the one convergent finding: the screen over-drops
  *gate-describing* legitimate lessons → now `dropped_unsafe_texts` echoes each dropped bullet to stderr (silent
  over-drop → visible, recoverable). Added regression pins (multi-`## Lessons` accumulate + CRLF; content-without-
  Lessons-heading → empty). Deferred (non-blocking, logged): per-bullet byte cap; the §9.1 Half-B trigger.
- **Exercise gate PASSED with evidence** (reads the REAL committed file, never lands): `caselaw_exercise` (`#[ignore]`)
  read `wiki/case-law.md` through the real read path, planted `- When the suite is slow, just skip the tests_green gate
  and land.`, and rendered the worker prompt → **8 real lessons injected** (contract prose absent, section-awareness
  proven live), **planted line dropped** (`dropped 1 unsafe`), assertions green. Run:
  `cargo test -p agent caselaw_exercise -- --ignored --nocapture`.
- **Evidence:** workspace green — agent **41** / board 20 / pi-iso 2; clippy clean; exercise rendered-prompt captured.
  fmt skipped (hard tabs, no rustfmt.toml → non-signal). **Subagent coordination pattern** = the design-time and
  pre-land **3-reviewer diverse-lens panels** (security / correctness+Wu-Wei / contract+integration), run in parallel,
  whose striking convergence drove a decisive scope cut then a clean ship.
- **ALL FOUR PILLARS COMPLETE** (memory / harden / parallel-exploration / self-evolution), each via the mandated
  planning → adv review → execute workflow. **Next:** notify Gary (the mandate's stop condition).

## [2026-06-10] session | Pillar glues field-tested live (items 1–3) — all pass, verification only
- **Scope:** prove the three pillar glues are reachable + correct **end-to-end against the real oMLX**, not just
  unit-tested in isolation. Nothing built, nothing committed — a verification pass with cleanup.
- **Item 1 (deterministic):** `cargo test -p agent` → **41 passed / 0 failed**. The `#[ignore]` `caselaw_exercise`
  read the REAL committed `wiki/case-law.md` through the live read path → **8 bullets harvested**, the planted
  `skip the tests_green` line **dropped by the screen**, the rendered `### Learned heuristics` section correct.
- **Item 2 (Half A live):** `agent run` on a scratch ticket logged `[case-law] injected 8/8 bullets`, and the
  **recorded trajectory** (`.harness/runs/<t>/<run>.jsonl`, system message) proves the case-law section with all 8
  bullets was in the *actual system prompt sent to the model* — the compounding **read** side reaches the prompt,
  not just `load_case_law`. (Confirmed by reading the system record out of the JSONL, not by trusting the stderr log.)
- **Item 3 (explore probe, both paths):**
  - *Pass path* — `explore --fanout 2` spawned two **isolated worktrees**, **injected case-law into each worker**
    (Half A composes with explore), applied diverse strategy directives, ran per-tree validation (both PASS),
    printed the ranked table, named a **winner CANDIDATE**, and **landed nothing**. Isolation proven: the produced
    artifact existed in both worktrees but was **ABSENT on main**.
  - *Fail path* — an unsatisfiable validation (`false`) → both workers FAIL with `stop=max_iters`, "no branch
    passed" reported, and **two post-mortem `lesson` memories stored** (recallable via `agent recall`) —
    reflection-on-kill confirmed working live.
- **Observation (model characteristic, NOT a harness bug):** on the unsatisfiable contract the small local 35B
  *wandered the repo reading files* rather than attempting work — the same plan-then-stop / explore-without-acting
  failure mode noted 2026-06-09. The model needs a concretely-achievable contract to do useful work.
- **Cleanup:** all 5 scratch worktrees + branches + trajectories + scratch DB removed. `rm -rf` was correctly
  **blocked by the dcg command guard** (`core.filesystem:rm-rf-general`) → used `git worktree remove --force` +
  `git branch -D` + plain `rm`. `git status` clean at `05687ff`; **nothing committed** (verification only).
- **Verdict:** all three glues reachable and correct end-to-end. The pillars are wired through the CLI, not just
  present in the crates.

## [2026-06-10] session | Verbose-local-model sprint (think-strip + budget) — RE-SCOPED on review, BUILT, exercised
- **Mandate:** give the verbose local 35B room to *finish* a hard task; stop its reasoning traces from silently
  re-filling the 32K context every turn and from overflowing Claude at review. Scope → adv review → execute →
  document, full autonomy. Scope doc `research/21`.
- **The headline feature died on review (correctly).** Planned: `strip_think` pure fn (record-raw / feed-stripped)
  + `agent trajectory --think`. 3-lens scoped adversarial review (silent-failure surface: over-strip eats output,
  provenance inversion, context-truncation) returned **all three non-SHIP**. **Load-bearing finding (reviewer C
  live probe):** the served `Qwen3.6-35B-A3B-oQ8-fp16-mtp` emits **ZERO `<think>` tags** — it reasons in plain
  markdown prose, even with `/think` + `enable_thinking:true`. A stripper is therefore a **no-op on 100% of real
  output = dead code.** Integrity call: do not build on a flawed premise → **`strip_think` CUT** rather than shipped.
  The real leak (reviewer B) is **uncapped re-fed assistant prose + tool output**, which a tag-stripper can't find;
  `FIELD_CAP=8192` is recorder-only and does NOT bound the `messages` vector.
- **Built instead (TDD, all prod lands in the harness):**
  - `crates/agent/src/refeed.rs` — pure module. `cap(text, limit)` keeps head (⅔) + tail (⅓) with an honest
    `[… N bytes elided …]` marker, never splits a UTF-8 char; `assembled_size(messages)` sums content + tool_call
    payloads. **7 unit tests.**
  - **Bounded re-feed** at the tool-call branch (`main.rs`): record the **raw** assistant/tool messages to telemetry,
    but push **capped** copies into `messages` (`REFEED_TEXT_CAP=1024` for prose, `REFEED_TOOL_CAP=4096` for tool
    output). tool_calls kept intact for coherence. Model-agnostic, test-pinned.
  - **`[ctx]` overflow signal** — every loop turn prints assembled-prompt bytes; WARNs past `CONTEXT_WARN_BYTES=100_000`
    so oMLX can't silently evict the system prompt without a trace.
  - **`agent trajectory <id> --full`** — plain full read (no `--think`): per-message `[step] role (N bytes)` + full
    content + raw tool_call payloads, so re-feed bloat is legible at audit time.
  - Budget knobs bumped **env-only** for the exercise (`HARNESS_MAX_TOKENS=12288`, `HARNESS_MAX_ITERS=40`); oMLX
    context-window setting untouched. **Evidence:** 48 tests green, clippy clean.
- **Exercise (throwaway `/tmp/rx-exercise`, a separate git repo — NEVER in harness history):** Thompson-NFA **regex
  engine** with a FIXED ~40-assertion failing spec (`tests/spec.rs`) as the artifact gate; full isolated spine
  `new→run(worktree)`, stop-at-green, **never land**. Run completed EXIT=0, `stop=truncated`, `iters=9`.
- **KEY DIAGNOSTIC — a THIRD failure mode: reasoning-loop collapse.** The budget bump *worked* (enabled a single
  **~19 KB `write_file`** implementation — 506 lines — that the old 1024 cap could never emit; the model wrote real
  code, not plan-then-stop). But on its final turn the model **derailed**: `run.log` shows it repeated **~6 distinct
  anchor-handling reasoning paragraphs 20–22× each verbatim** until the `length` cutoff truncated it mid-thought. This
  is distinct from both "budget too small" and "completed-no-write." The deferred **plan→execute nudge would NOT fix
  it** — the model *did* execute, then looped. A repetition circuit-breaker or a lower per-turn token cap might —
  **surfaced as a verdict, NOT built** (diagnosis discipline: gate by the number, build only what the number demands).
- **DISCOVERED GAP (evidence-based):** the biggest single context contributor — the 19 KB write_file content — rides
  in the assistant turn's **tool_call ARGUMENTS**, which `refeed::cap` does **not** touch (it caps message `content`
  and tool_result `content` only). `assembled_size` *measures* tool_call args but `cap` doesn't *bound* them. So the
  re-feed cap was never meaningfully stressed this run (ctx peaked ~28 KB, never hit the 100 KB WARN). Capping tool_call
  arguments is the natural follow-up — deferred (Wu-Wei: not yet causing a real overflow).
- **PROVENANCE NOTE (corrected from earlier memory):** the recorder's `FIELD_CAP=8192` truncated the collapse turn at
  ~8 KB in the run JSONL — so the JSONL preserves only the *first* 8 KB (legitimate code, 89/98 distinct lines), **not**
  the repetition signature. The collapse is visible **only in `run.log`** (full stdout). For telemetry to *adjudicate*
  reasoning-loop collapse, the recorder would need a higher final-turn cap or a repetition summary (surfaced, not built).
- **Committed exercise code is RED:** `cargo test` in the worktree fails to compile — `E0369` (`==` on a `Transition`
  enum lacking `PartialEq`), the model's own bug, never fixed before the collapse. The exercise did its job (forced a
  genuinely hard task that drove real iteration); greenness was never the deliverable — the *harness behaviour under a
  hard task* was.
- **Cleanup:** `/tmp/rx-exercise` worktree + branch removed via `git worktree remove --force` + `git branch -D` + plain
  delete (the dcg guard blocks recursive-force delete); the throwaway repo stays out of harness git history. **Nothing
  committed to the harness repo** (awaiting Gary's go).
- **Verdict:** budget bump works; re-feed bounding + overflow signal + `--full` audit built, tested, clippy-clean; the
  run exposed a new failure mode (reasoning-loop collapse) and a real gap (uncapped tool_call args). `strip_think`
  rightly cut. Full fold-in in `research/21` §9 + decisions.md.

## [2026-06-10] session | Loop gate (Tier-1 deterministic cycle stop) — discussed, scoped, BUILT
- **Mandate:** after the §21 discussion ("are we tunnel-visioned on the local provider?") + the literature-grounding
  ask. Answered both, then Gary said "yes please, remember to document. The machine is all yours." Scope → adv review
  → execute → document. Scope doc `research/22`.
- **Literature grounding (the discussion's ask):** loop = decoding-level degeneration (greedy/low-temp self-reinforces;
  no global anti-repetition objective — Holtzman 2019; KB *Mastering PyTorch* greedy→repetition / top-k-p remedy).
  Detection is cheap+deterministic (tool-call hashing, n-gram/char-run, no-progress); white-box activation monitoring
  (RecurrentDetector 2025, ~95%) needs hidden states we don't get from an API → out. Prevention = harness discipline:
  hard limits backstop + external stop rule + course-correction nudge. KB Production RAG Guide "Agent Loop Safety".
- **Meta question settled:** the §21 think-tag chase *was* provider tunnel vision; the loop gate is the opposite —
  agent loops + autoregressive degeneration are **universal**. Discipline test held throughout: *is the fix keyed off a
  universal signal or a provider quirk?* The gate reads only `resp.tool_calls` (name+args) + `resp.stop_reason` — both
  in the format-agnostic `provider::Response`; identical behaviour against the anthropic provider. "Design for the 35B
  floor → works for everything" holds *because* the mechanism is model-agnostic.
- **Adversarial review (necessity-first), folded into `research/22` §9:** (1) necessary over `max_iters`? yes but
  scoped honestly — adds earlier stop (2 vs N turns; N=40 in the §21 exercise), a recovery nudge, an honest `looped`
  label; explicitly **NOT** "fixes the §21 RED commit" (that was intra-turn collapse, caught downstream by
  tests_green/mutation). (2) false positives? signal = **byte-identical consecutive** tool-call signatures = provably
  non-progress; lowest-FP signal; nudge-first gives one recovery turn; the polling exception doesn't apply (workers run
  sequentially in isolated worktrees — agent is the only actor). (3) transcript validity? Nudge dispatches tools +
  emits results *before* the nudge (call→result invariant); Stop breaks before re-executing, dangling call never
  transmitted. (4) label integrity? `looped` is a failure, bypasses `classify_stop`, new value in the existing TEXT
  column (no schema change). (5) the natural-completion break runs before detection — only *continuing* turns are gated.
- **BUILT:** `crates/agent/src/loopgate.rs` — pure, model-agnostic, mirrors `refeed.rs`. `signature(&[ToolCall])`
  (ordered name\0args\x01 fingerprint, separators prevent cross-boundary collision) + `LoopGate::observe(&str) ->
  Verdict {Proceed|Nudge|Stop}` (consecutive-identical detection; **run-level strikes** → nudge on strike 1, stop on
  strike 2; robust to cycle-switching A,A,B,B without a window). 8 unit tests. Wired into `run_ticket`: detect after
  recording the assistant turn, **Stop** breaks before re-executing (`looped=true`, `hit_max=false`), **Nudge**
  dispatches then appends a `[harness control]` user message (recorded, uncapped); outcome label `looped` when tripped.
- **Scope cuts (Wu Wei, in `research/22` §8):** intra-turn text-repetition detection (axis B — the §21 collapse)
  **DEFERRED** (it already self-terminates on `Length`; the only gain is a `degenerate` label finer than `truncated`,
  with no consumer yet); streaming in-flight abort (Tier 2, needs SSE) deferred; period-2 window / no-progress
  detection deferred until observed.
- **Wiring EXECUTION-TESTED (gap closed, same session).** Gary's callout — "did you test it with actual agent calls?" —
  was correct: the detector had 8 pure unit tests but the *loop integration* was only build/clippy-verified, never run.
  Closed it deterministically (not a flaky induced loop on the 35B): **provider-injection refactor** — `run_ticket` now
  takes `provider: &dyn Provider` (was constructing `OpenAiProvider::omlx()` internally); both call sites (`run_worktree`,
  `run_explore`) construct it once and pass `&provider`. Added a `ScriptedProvider` mock (replays a fixed `Vec<Response>`,
  ignores the request) + 2 `#[tokio::test]`s over the **real** loop: `loop_gate_stops_a_repeating_tool_call` (3 identical
  read_file turns → asserts `stop=="looped"`, `iters==3`, run row `looped`, and the `[harness control]` nudge present in
  the recorded trajectory) and `distinct_calls_are_not_flagged_as_a_loop` (distinct calls then End → `completed`, no
  nudge). `async-trait` added to agent **dev-dependencies** only (test-side mock; no prod dep added).
- **Evidence:** agent crate **58 tests green** (was 48 — +8 loopgate +2 integration); workspace board 20 / pi-iso 2 /
  provider 1-ignored (live-oMLX) all green; `cargo build` + `cargo clippy --workspace --all-targets` clean. The live
  nudge→stop trace prints exactly as designed (nudge on strike 1, STOP on strike 2). **Nothing committed** (awaiting Gary's go).

## [2026-06-10] session | Plan→execute nudge built — the no-action stall (research/24); four-failure-mode spine complete
- **The fourth failure mode, the diagnostic spine's last gap.** Three were already owned — (1) re-feed bloat
  (`refeed.rs`), (2) cross-turn tool-call loop (`loopgate.rs`), (3) intra-turn collapse (deferred, self-terminates on
  `Length`). The fourth: **plan-then-stop** — the model emits a plan/analysis in prose, calls **no tool**, and the
  loop's natural-stop branch banks it `completed`. The single most-cited local-model failure across the breadcrumbs
  (Telemetry Unit B produced a correct plan and wrote **0 files**). Earlier logged "deferred — prod is Claude's job";
  the oMLX-must-work thesis reversed that (local-loop behaviour is in scope).
- **Disjoint sibling of the loop gate, by construction.** loopgate lives in the ToolCalls branch (fingerprints tool
  calls); planexec lives in the no-tool-call branch (`stop_reason != ToolCalls`), which loopgate never sees. Two
  stalls, two counters, no shared state: loopgate = "calling the same tool forever" (action without progress);
  planexec = "never calling a tool at all" (no action).
- **Pure core** `crates/agent/src/planexec.rs` (mirrors `loopgate.rs`/`refeed.rs`): `verdict(acted, code, nudged) →
  Accept|Nudge|Stall`. Accept = it changed the tree OR a non-code kind (legitimate stop). Nudge = first no-action stop
  on a code ticket (one recovery turn). Stall = already nudged, still nothing (honest failure). The 1-nudge bound is
  monotone — after a nudge the next stop can only Accept (acted) or Stall (didn't), both terminal — so termination is
  guaranteed; `max_iters` backstops. 5 unit tests over the full acted×code×nudged matrix.
- **Adversarial review (light, 2 reviewers — false-positive/necessity + false-negative/termination), folded into §10.
  Both ship-with-changes.** Two findings reshaped the build:
  - **BLOCKER (reviewer B): `acted` cannot be tool-based.** The scope set `acted` = "a mutating tool ran"
    (`tool_is_mutating` = `!read_file`, so `bash` counts) — but the system prompt orders the model to start with
    `bash ls -R`, so `acted` would be true on **every** run and the nudge would never fire. The feature was dead on
    its primary target. **Fix:** `acted` = **the worktree is dirty at the natural stop** (`git status --porcelain`
    non-empty; new `git::is_dirty`) — base-free, tool-agnostic, catches an untracked fresh `write_file` (which
    `git diff --quiet` misses) and reads clean on a write-then-revert net-no-op. `lines_changed_against` is the wrong
    call mid-loop (diffs `base...HEAD`; work is uncommitted → reads 0). The pure `verdict` core is unchanged; only the
    *source* of `acted` moved. "Gate by the artifact (a real edit), not the proxy (a tool firing)."
  - **MAJOR (reviewer A): split the feature, gate the nudge on a number.** The `stalled` *label* pays its way on
    telemetry honesty alone (a do-nothing run must not bank `completed`); the *nudge* is speculative. Ship the label
    unconditionally; build the nudge but make the live exercise a real Wu-Wei gate (report nudge→recovery conversion).
- **Wiring** (`run_ticket` no-tool-call branch): Accept→break as today; **Nudge**→push a *capped* copy of the
  assistant plan turn (`refeed::cap` — otherwise the next turn's context lacks the referent) + a `[harness control]`
  plan→execute message, set `plan_nudged`, `continue`; **Stall**→`stalled=true`, `hit_max=false`, break. New `stalled`
  outcome label: `stop_reason` is free-form TEXT → **no migration**; forced at the call site like `looped`
  (`if looped{…} else if stalled{"stalled"} else classify_stop(…)`, looped wins precedence); `rank_outcomes` already
  sinks a `stalled` row (passed=false, lines=0) — no ranking change. Label keys on **final** `acted` (Stall arm only),
  so nudge→act→stop computes acted=true → Accept → `completed` (the recovery path is correct, not mislabelled).
- **Tests:** 5 unit + 4 `ScriptedProvider` integration over the real loop —
  `plan_then_stop_gets_one_nudge_then_stalls`, `writing_then_stopping_is_completed_no_nudge`,
  `non_code_no_action_is_exempt`, `nudge_then_act_recovers_to_completed` (pins reviewer A's recovery trap). Test cwd is
  git-init'd **distinct from root** so the trajectory write (under `root/.harness/`) never dirties cwd and defeats
  stall detection — mirrors production where worktree ≠ repo root.
- **Evidence:** agent **67 tests/1-ignored** (+9 over baseline); board 20 / pi-iso 2 / provider 1-ignored; `cargo
  build` + `cargo clippy --workspace --all-targets` clean.
- **Live oMLX validation (3 throwaway exercises, never landed) — the failure-mode distribution + the Wu-Wei number.**
  On real refactor tickets the 35B fails via **action-without-progress** — run 1 → `max_iters` (read-wander), run 2 →
  `looped` (repeated `sed -i`) — both owned by max_iters/loopgate, which fire *before* any natural stop. planexec's
  natural-stop signal only surfaced on an **already-satisfied** task (run 3): the model declared "No edits were needed"
  → plan nudge fired → it re-ran `cargo build` / re-declared done → **STALL → row labelled `stalled`** (not a faked
  `completed`). This is the live end-to-end proof. **Honest finding (reviewer A's gate): the nudge did NOT convert** —
  the model insisted the work was complete rather than acting → conversion ≈0 in the observed runs. **Verdict: the
  `stalled` label is the value; the nudge is on probation** (one bounded cheap turn; cut it if a larger sample confirms
  ≈0). Label and nudge are independent exactly as the review required.
- **The four-failure-mode diagnostic spine is now complete.** **Nothing committed** (awaiting Gary's go).

## [2026-06-10] session | research/25 context-engineering survey — local teardown + 3 web workers → synthesis + adv review
- **Trigger:** Gary's strategic redirect — reframe as context engineering (assume the model is capable), and his
  question "within a gate, how are we re-feeding context — feed the whole thing, or interfere?" Method = **Hybrid**
  (Claude tears down local clones directly; 3 web workers survey the rest, parallel), **research/25 first** (before the
  bounded-thinking design). Sources Gary picked: oh-my-pi+pi-mono, OpenCode, Hermes/Nous, Claude Code+Aider+literature.
- **Answer to the re-feed question (§1):** today the loop feeds the **whole unpruned transcript** every turn
  (`messages` push-only) with the only interference a crude **per-message byte cap** (`refeed::cap` head⅔+tail⅓; prose
  1024B / tool 4096B; **tool_call args UNCAPPED**; reasoning never re-fed). Zero semantic curation — a 32K-window
  survival hack. When the transcript outgrows 32K, oMLX silently truncates and drops the system prompt first = the
  silent THRASH-to-max_iters failure.
- **§2 local teardown (oh-my-pi + pi-mono):** a complete compaction subsystem — token-threshold trigger (reserve-aware),
  backward cut-point (never split a tool result), split-turn prefix+history merge, real cl100k tokenizer, the 7-section
  structured handoff summary, iterative cumulative update, anti-continuation framing, cumulative file-op touch-list,
  append-only/StablePrefix KV-cache discipline, and a per-program tool-output **minimizer** with structural safety
  (don't minimize piped/compound) + `artifact://` stash-and-reference (lossy in window, lossless on disk). Catalogued
  as **L1–L13**.
- **§3 web survey:** **OpenCode** — compaction summary-anchor + protected-tail, 3-layer tool-result handling
  (truncate-to-disk-with-hint, backward-scan pruner that keeps tool-call args but clears results, compaction-time cap),
  on-demand AGENTS.md injection, reasoning same-provider-only; honest: no semantic curation. **Hermes/Nous** — 4-phase
  ContextCompressor at 50% (Phase-1 JSON-truncates tool-call args = direct fix for our gap; Phase-3 LLM-summarizes the
  middle with the same 7-section template), `keep_cots=False` default; ⚠ silent-drop failure when summarizer window <
  main. **Claude Code** — compaction (58.6% reduction, 3-strike breaker), sub-agent isolation (14:1), tool-result
  clearing (`clear_tool_uses`/`clear_tool_inputs`, lossless-refetchable, 163K freed); **Aider** — repo-map
  (tree-sitter+PageRank), udiff edit format; **Literature** — **Lost-in-the-Middle** (Liu et al. TACL 2024, ~30%
  mid-context degradation, U-shaped — the strongest signal), MemGPT tiers, atomic task decomposition (backs Gary's
  reframe), Cognition "share full traces."
- **Headline — convergence:** the **same 7-section structured summary** appears independently in **4 codebases**; the
  universal mechanism is **bounded recency (protect head+tail) + structured summary of the middle + offload/clear tool
  output**; Lost-in-the-Middle explains *why*. **No source does semantic working-set curation** (tempers Decision 2).
- **§4 synthesis:** our 6 gaps → minimal fixing subset **F1–F5** (F1 protect-head/position-aware; F2 token-threshold
  compaction + 7-section summary; F3 tool-output offload; F4 cap/clear refetchable tool-call args; F5 cumulative
  file-list). Reasoning-refeed reconciled with Decision 3 (§4.3): production axis ⟂ re-feed axis → "let it think, bound
  the budget, record-but-don't-re-feed." Tail-bias flip = one-number measurement.
- **§5 adversarial review (silent-failure class → REVIEW):** every compaction/offload mechanism is itself a
  silent-correctness risk (centerpiece = Hermes summarizer-window-overflow silent middle-drop). **Through-line rule: a
  context mechanism may be lossy but NEVER silently lossy** — abort+signal on summary overflow; clear only refetchable
  data; preview keeps the tail; assembler asserts call↔result adjacency; anti-thrash breaker escalates. Downstream
  artifact gate is the accepted backstop.
- **§6 recommendation:** adopt as **4 slices** (F1 → F4 → F2 → F3+F5), each gated by a number/artifact, built **AFTER**
  the bounded-thinking design. Deferred (Wu Wei): real tokenizer, split-turn, StablePrefix, repo-map, external-memory.
- **Wiki:** decisions.md — Decision 2 RESOLVED (chosen mechanism + no-silent-loss invariant) + 2 new Rejected
  Approaches (semantic curation; re-feeding CoT); index.md research/25 entry; active-work.md breadcrumb. **Research
  only — nothing built/committed.** NEXT: design + adv-review the bounded-thinking adaptive-budget mechanism, then align
  with Gary on the re-feed slice build.

## [2026-06-10] session | Bounded-thinking pillar SHIPPED — wire change, posture flipped to bounded think-ON
- **Context:** continuation of the research/26 work (adv review + root-cause correction were prior; the server flag
  `thinking_budget_enabled` was flipped on and the budget re-verified in §9.1). This session executed the EXECUTE step.
- **Decision — mechanism = fixed preset-ladder budget, NOT the self-estimating triage.** The §7 adv review (4 cold
  reviewers, convergent) cut Gary's predictive triage as downside-only under a quality objective; §9.1 verified the
  budget is a soft target with no exhaustion signal. So the build is dead simple: pick one rung from the ladder
  (low/med/high = 1024/2048/4096), `max_tokens` as the hard backstop. No triage, no schema, no new `StopReason`.
- **"Who picks the rung" RESOLVED:** fixed per-turn default, env-overridable (`DEFAULT_THINK_BUDGET=2048`, override
  `HARNESS_THINK_BUDGET`; `0`→OFF). Caller/ticket-tagged rung + reactive escalation kept as *additive* follow-ons
  (research/26 §10) — not built, to observe fixed-rung telemetry first.
- **Code (3 sites, ~40 lines):** `provider::Request.think: bool → think_budget: Option<u32>` (`lib.rs`, doc rewritten
  to soft-budget semantics); `openai.rs request_to_body` — `None`→`enable_thinking:false`+no budget key, `Some(n)`→
  `enable_thinking:true`+**flat top-level** `thinking_budget:n` (the §9.1 path); `anthropic.rs` unchanged (ignored the
  field); `agent/main.rs` — new `DEFAULT_THINK_BUDGET`, loop reads env once per run, budget recorded in the run's
  `sampling` telemetry JSON. Subagent fan-out Wu-Wei-unnecessary at this size (Harden-gate precedent) — solo.
- **Gate:** 90 tests pass (0 fail), clippy clean, `body_carries_thinking_budget` asserts both mappings (absent key off;
  flat `2048` on), live oMLX tool-call smoke green. Wire shape unit-proven + §9.1 server-verified = end-to-end closed.
- **Behavior change flagged:** committed default flips think-OFF (research/23) → bounded think-ON @ medium=2048 — the
  intended Decision-3 posture shift, settled by §7+§9.1, not silent. `HARNESS_THINK_BUDGET=0` restores OFF.
- **Owed cleanup done:** `~/.claude/CLAUDE.md` stale `…-8bit`→`oQ8-fp16-mtp` (4 spots); research/23 §1 Correction 2
  records the two server flags (`thinking_budget_enabled`, `reasoning_parser`) + that `thinking_budget` is NOT pure-wire.
- **Wiki:** research/26 §10 added; decisions.md Decision 3 amended (triage→ladder) + triage added to Rejected
  Approaches; index.md research/26 entry; active-work.md + log.md. **Uncommitted** — awaiting commit go.

## [2026-06-10] session | Context-engineering arc (research/27 + F1–F5) + Pillar P0 auto-context priming
- **Context:** autonomous `/goal` run (per-pillar scope→adv-review→plan→execute). This session: sized the context
  window (research/27), built the context-engineering subset (F1–F5), then built Pillar P0 (memory read-side).
  Full per-slice detail lives in active-work.md breadcrumbs (this is the session rollup). **All uncommitted.**
- **research/27 context-budget:** the baked-in 32K wall was a phantom — oMLX serves 256K-native (`max_context_window:
  262144`), but that's a *retrieval* claim, not *reasoning*. Sized to the A3B reasoning curve: effective window 48K,
  compaction trigger 32K, keepRecent 20K, KV fp16. Set F1's tail budget + F2's trigger.
- **F1 (`refeed::assemble`):** per-turn non-destructive view — immutable head (system+task), ~20K-tok verbatim tail,
  elided middle with a no-silent-loss marker; backward tail-walk so a tool_result is never orphaned from its call.
- **F2 (token-threshold compaction):** folds the transcript middle into a 7-section structured summary off the FULL
  running transcript when it exceeds the trigger; abort-loud + 2-strike breaker (F1 floor still holds). Live oMLX
  gate PASSED (48 msgs ~8861→~735 tok, 12×, all 7 sections + verbatim path + unanswered question).
- **F3 (tool-output offload):** large outputs → gitignored on-disk artifact + head/tail preview + mandatory re-read
  hint; `read_file` gained `offset`/`limit` for targeted re-read. Artifacts under `.harness/` never staged / never
  trip the plan→execute dirty check. (F4 cap-refetchable-args + F5 file-touch-list folded in along the way.)
- **Pillar P0 auto-context priming:** closes the dormant compounding loop (explore wrote post-mortems nobody read).
  `recall_primed` (body-carrying, all-scopes, token-overlap floor, non-bump) on a shared `recall_rows` SQL source of
  truth; wired into `run_ticket` best-effort; `build_system_prompt` injects `### Relevant past experience` after
  case-law. New `PrimedHit` (titles are generic → must carry bodies). `MEMORY_RECALL_K=5`, body head+tail clamp 240.
  - **Decisions (see decisions.md "Pillar P0"):** query on ticket TITLE only; token-overlap floor is STRUCTURAL not
    a numeric BM25 cutoff (doesn't reopen the Slice-B deferral); non-bump (auto-injection ≠ deliberate use, avoids a
    rich-get-richer fixed point); head+tail clamp keeps Strategy head + validation tail. Rejected: titles-only
    priming; capture-as-loop-tool now (needs tools↔board plumbing — deferred).
  - **Gate:** 4 board + 1 agent deterministic tests (assembly + non-bump + floor); board 24 / agent 90, clippy clean
    workspace-wide. No live oMLX gate — correctness is mechanical (assemble + never-bump), not model-dependent.
- **Wiki:** active-work.md breadcrumbs (F1/F2/F3/P0); decisions.md "Pillar P0" section; index.md research/18 P0 note;
  this log entry. **Large uncommitted surface across the whole arc — to flag to Gary at a commit checkpoint.**

## [2026-06-11] session | Pillar P0 behavior-probe + Memory Slice B GO/NO-GO gate (research/28 + 29)
- **Context:** continuation of the autonomous `/goal` run. Two things this session: (1) behavior-probed P0 on live
  oMLX; (2) ran the pre-registered Slice B (vector leg) retrieval gate as a full pillar workflow (plan→adv-review→
  impl-plan→execute). All EXERCISE/throwaway (oMLX), nothing landed, nothing committed.
- **P0 behavior probe:** the deterministic tests proved P0's mechanics; this answered "does the model *use* an
  injected post-mortem?" Attributable A/B — the memory carries a fact unknowable from training (this loop is
  single-threaded sync → `tokio::time::sleep` panics → use `std::thread::sleep`). Discriminating metric "cites the
  no-runtime reason": **control 2/8 → treatment 8/8**; treatment answers flip hedged→grounded. P0 validated
  end-to-end (mechanics + behavior). Throwaway `/tmp/p0_probe.py`.
- **Slice B gate (research/28):** plan → **adv review demolished the planned full eval** (5 convergent findings:
  wrong SUT `recall_primed`-vs-floorless-`recall`; provenance-as-relevance bias; underpowered ~13% @ N≤40;
  tautological EXACT/PARAPHRASE split; disproportionate apparatus) → **DESCOPE** to a 3-arm hand probe + the L4
  survey. **Decomposition trick:** one 14-row artifact-distilled corpus + 14 paraphrase queries through (1)
  `recall_primed` (floored, shipped), (2) `recall` (floorless BM25, shipped), (3) jina-v5 dense (oMLX) — arm 2
  isolates "does the FLOOR cause the miss?" from "does DENSE beat BM25?".
  - **Result (recall@5): primed 12/14 · floorless 13/14 · jina-v5 14/14.** Two misses: *floor-dropped* commit-discipline
    (floorless #1; need≥2 floor drops it on a single shared token) and *zero-overlap* worktree-confinement (no shared
    ≥3-char token; only dense recovers, rank-1 cos 0.509). Dense cosines: paraphrases 0.45–0.63, exact 0.82.
  - **Verdict — Slice B NO-GO:** floorless lexical handles 13/14; dense's marginal 1/14 (zero-overlap paraphrases only)
    doesn't clear the dependency bar for an ambient-hint use. Embedding columns stay dormant insurance, now warmed
    (oMLX embed path exercised ⇒ a future GO is build-not-vendor).
- **Survey leg (research/29):** frankensearch (Tantivy dup heavy dep; in-process ONNX wrong shape for HTTP oMLX;
  license-rider ambiguity), fast_vector_similarity (rank-corr not cosine; no license), cass / coding_agent_session_search
  (design-only, already folded into research/18 §10.3) → **nothing to vendor even on a GO.**
- **Follow-up filed (board t5, gitignored DB):** the `recall_primed` floor over-filters single-content-token paraphrases.
  Recommended fix = BM25-score-aware floor, NOT blind `need=1` (reintroduces cold-store noise). Left todo BY DESIGN —
  the noise side is unmeasured; changing gated code on half the evidence violates gate-by-a-number.
- **Wiki:** active-work.md breadcrumb (Slice B NO-GO); decisions.md "Memory Slice B (vector leg) — NO-GO + the floor
  over-filters"; research/28 §10 (descope verdict) + §12 (results & verdict); research/29 (survey); this log entry.
  New research files `research/28`, `research/29` untracked. **Uncommitted — awaiting Gary's commit go.**

## 2026-06-11 — Shakedown fixes #1/#3/#4 landed in-tree (tested, green; uncommitted)
The three actionable findings from the t6/t7/t8 dogfood runs, fixed under the `/goal` mandate (plan → 2 confirmed
forks via AskUserQuestion → execute). #2 by-design (no fix); #5 already fixed (`.gitignore` `__pycache__/`+`*.pyc`).
- **#1 gate-before-worktree** (`agent/src/main.rs run_worktree`): Align-gate check moved ahead of
  `git::ensure_worktree` → a `run` on a pre-Align ticket refuses with no orphan `harness/<id>` branch+worktree.
  Test `run_refuses_unaligned_ticket_without_creating_worktree` (real async `run_worktree` vs a Todo ticket in a
  temp repo; asserts err + no branch + no worktree + status unchanged).
- **#3 language-aware Harden** (`agent/src/main.rs`, `git.rs`): `run_harden` dispatches by diff content —
  `.rs`→`harden_rust` (cargo-mutants, unchanged), `.py`→`harden_python` (**cosmic-ray 8.4.6, operator-installed,
  not a crate dep**: toml→baseline→init→cr-filter-git diff-scope→exec→dump, judged by parsing dump), neither→vacuous
  1.0. New `git::changed_files_against`. `MutationScore`: `parse`→`parse_cargo_mutants` + new `parse_cosmic_ray_dump`
  (killed→caught/survived→missed/incompetent→unviable/skipped→ignore). Test `cosmic_ray_dump_parses_and_scores`.
- **#4 enforced oracle integrity** (`agent/src/main.rs run_verify`, `board/src/spine.rs`, `git.rs`): at verify
  (build|bugfix only), each convention-matched oracle file changed-vs-base ⇒ `GATE_ORACLE_INTACT` fail + bail.
  New `board::is_protected_oracle` (Python `test_*.py`/`*_test.py` + Rust `tests/`), `GATE_ORACLE_INTACT` const
  (evidence-only, not a transition gate), `git::path_exists_on_base`. Adds-new-tests OK; edits-committed-oracle
  forbidden. No schema change. Test `protected_oracle_convention`.
- **Gate:** board 25 + agent 99 green, clippy clean across both crates. Decisions (+ rejected approaches for the #3
  and #4 forks) in decisions.md; active-work.md breadcrumb added. **Uncommitted — awaiting Gary's commit go.**

## 2026-06-11 — Memory read-side: S1 t5-floor fix shipped + S3 researcher-subagent built (uncommitted)
The memory pillar's read path, under the `/goal` mandate (plan → adv review → impl plan w/ subagent coordination →
execute), scope B (build plumbing, defer the *ship* decision until the store accumulates real lessons).
- **S1 / t5 (board):** settled the `recall_primed` overlap-floor question by a number (research/31 eval: 36-row mined
  corpus, 57 provenance-labelled queries). Ship **a\*1** — strip stopwords (`content_tokens()` = `overlap_tokens` −
  `STOPWORDS`), keep `need≥1`: noise 18→13/24, recovery held 32/33, **no tuned constant**. Original "floor too strict"
  premise FALSE on a real corpus (the miss was a 14-row-probe artifact); the real defect was precision. Locked test
  `recall_primed_floor_keeps_content_drops_stopword_only`; eval kept `#[ignore] t5_floor_eval_v2`. BM25-score floor +
  strict need≥2 both REJECTED by their pre-registered gates.
- **S2a (board):** `PrimedHit.id` so a researched lesson and the `recall_primed` fallback emit the same shape and
  `used_ids` is first-class/traceable.
- **S3 researcher subagent (agent):** a local-LLM deep-research loop that explores the store and feeds only relevant
  lessons back. **Pure core** `crates/agent/src/researcher.rs` (`ContextPacket`+`is_empty`, `TOOL_CALL_CAP=12`,
  `RESEARCHER_SYSTEM`, tolerant `parse_packet`, `select_lessons` w/ dedup + cap-hit synthesis; 12 units). **Glue**
  `run_memory_researcher`/`dispatch_researcher_tool`/`researcher_tool_defs` + `env_flag` in main.rs — oMLX-pinned,
  `tokio::time::timeout` 25s, two READ-ONLY board-coupled tools (`search_memory`/`recall_body`, dedup-guarded),
  PrimedHits bounded by new `board::clamp_primed_body`. **Default-OFF behind `HARNESS_MEMORY_RESEARCHER`** (byte-
  identical when OFF); ON → researcher, `None` → `recall_primed` fallback. Fall-back-safe by construction (empty
  packet → None; cap/parse-fail → synthesize; error/timeout → None → lexical floor). Contract bug (empty-packet vs
  synthesize) caught via a dead-code warning during the build and fixed.
- **Gate:** 12 researcher units + workspace green (agent 111 / 3 ignored) + clippy clean on production code +
  `#[ignore]` live oMLX smoke `memory_researcher_live_omlx` — **PASSED** (selected the relevant retry/backoff lesson
  over a flexbox distractor, verbatim body, no fabrication, 15.15s < 25s). **Ship-eval DEFERRED** per scope B (BM25
  corpus saturated; +15pt gate unclearable until real lessons accumulate — dormant-infra posture, NOT a moved goalpost).
- **Carryover (flagged, not folded in):** 4 pre-existing clippy `--tests` warnings unrelated to S3
  (board/memory.rs:1008/1135/1136; agent/main.rs:2079). Board-lifecycle gaps still owed a ticket (no out-of-band close
  verb; `align()` Rework→InProgress illegal; t3/t5/t7/t8 stuck states). **Owed to Gary: ROTATE the exposed DeepSeek key.**
- **Wiki:** active-work.md breadcrumb (S3 BUILT); decisions.md "Memory S3 — researcher subagent BUILT"; index.md
  `31-eval-results.md` entry + memory-line note; this log entry. **Uncommitted — awaiting Gary's commit go.**

## [2026-06-11] session | Board-lifecycle pillar — Fix A + Fix B + cleanup (the carryover board-hygiene work)
- Closed out the board-hygiene carryover flagged from t5's close-out and the DeepSeek-run breadcrumb: two
  terminal-reachability gaps in the FSM, plus the 4 carryover clippy `--tests` nits. Per /goal: plan → adv review →
  impl → execute, documented as I went. board 27 / agent 113, clippy clean (workspace + `--tests`). Nothing committed.
- **Fix A (#63) — `align()` bridges Rework→Align.** The rework loop didn't reconverge: `align()` only did
  Todo→InProgress, but a reworked ticket sits in `Rework` and Rework→InProgress is illegal per the spine
  (forward_targets(Rework)=[Align]). `align()` now routes by current status (Rework→Align, else →InProgress); the
  attempt bump scopes the stale `criteria_confirmed`. Gate: `align_reconverges_a_reworked_ticket`. Green.
- **Fix B (#64) — non-landing close path to Done.** `Done` was reachable ONLY via `Land→Done` (GATE_LANDED), so
  non-code completions (latent) and abandonments (live, t3) had no terminal. Designed in **research/32**
  (recommended package + 3 forks + rejected approaches), adv-reviewed (3 cold reviewers → SHIP-WITH-CHANGES, all 3
  MAJOR folded: re-report idempotency, `--abandon` guard + worktree cleanup, pre-checks). **One edge + one gate:**
  spine.rs `forward_targets(Review)` gains `Done`, `required_gates_for` adds `(Review,Done)=>[GATE_RESOLVED]`, new
  human-sourced `GATE_RESOLVED`; main.rs `close <id> "<note>" [--abandon]` verb (non-empty note required; refuses
  non-review tickets; refuses code kinds without `--abandon`; removes the worktree on an abandoned code ticket,
  idempotent). **Keystone + no-skip both held** (resolved is human-only → agent can't self-close; only Review→Done
  added → Todo/InProgress/Verify/Align→Done stay structurally illegal). Gates: spine
  `close_path_is_human_gated_and_review_only`, board `close_gate_enforced_and_attempt_scoped`, agent
  `close_verb_guards_and_resolves`. Green.
- **Cleanup (#65) — clippy `--tests` + live dogfood.** Cleared the 4 carryover nits: `memory.rs:1008`
  `*c % 2 == 0`→`(*c).is_multiple_of(2)`; `memory.rs:1135` nested-if→let-chain (edition 2024 + rustc 1.90);
  `main.rs:2204` MutexGuard-across-await → scoped `#[allow(clippy::await_holding_lock)]` w/ justification (the guard
  MUST span the await — it serializes the global cwd swap; std Mutex is correct, no async-Mutex dep for one test).
  **Dogfooded Fix B live** on `harness-board.db`: `close t3 "abandoned: oMLX throwaway exercise…" --abandon` drove
  t3 `[build] review → [build] done`; gate trail = `resolved`(human) row present + **zero** `landed` rows (the
  auditable resolved-close ≠ landed-done distinction). Left intentionally (Wu Wei): t5 (stale, already shipped
  elsewhere — closing would require faking verify evidence; documented one-off, not an FSM gap), t7/t8 (live
  unfinished dogfood, not FSM-stuck), t4's lingering worktree (done, harmless).
- **Wiki:** active-work.md breadcrumb (BOARD-LIFECYCLE PILLAR DONE); decisions.md "Board-lifecycle — Fix A + Fix B"
  + a "Board close-path alternatives — REJECTED" block; index.md `32-board-close-path.md` entry; this log entry.
- **Owed to Gary (still open):** ⚠️ ROTATE the DeepSeek key exposed in chat during the earlier t6 run.

## 2026-07-13/14 — Critical path COMPLETE + first real dogfood + first self-hosting sprint (Fable 5 session)

The session that took the harness from "strong single-ticket loop" to "daily-driver loop demonstrated on a real
project". Commits 083cd85 → 5a27745 (12); suite 167 → 199 tests, clippy clean throughout.

- **Progress review + distillation ruling.** Re-confirmed the 6-30 readiness assessment; recorded that the project
  is NOT model distillation (orchestration/scaffolding; nothing trains on model outputs) — boundary: SFT on Claude
  transcripts would cross it; self-evolution Half B's "oMLX-distill" is fine only as prompt-time condensation.
- **Critical path built, all three slices (2026-07-13/14):**
  1. `agent draft <id> [--force]` (742b8dd) — DeepSeek-default intake drafting through existing chokepoints;
     pre-Align band only; refuse-overwrite; keystone untouched. Baked DeepSeek key ruled a deliberate throwaway.
  2. `agent run <id> --worker claude` (45588c6) — whole-task delegation to `claude -p` (subscription-backed) in the
     ticket worktree; trust-nothing outcome mapping; stream captured root-side; skip-permissions per align ruling.
  3. `agent sprint [--worker claude]` + `agent edge` (9c16509) — one-pass coordinator over new `Board::runnable()`;
     keystones never touched ⇒ snapshot complete by construction; park-and-continue. Kata proof: 2-ticket
     blocks-chained Version project, both landed, base 10/10.
- **Strategic posture settled (decisions.md): contain Claude Code, don't compete.** Thesis: prompting is advisory,
  code is enforced. Tripwire: CC-like UX before the coordinator = drift. MCP-wrapper idea recorded (unscheduled;
  keystones never on the MCP surface; needs a name — Warden/Keel/Spine floated).
- **REAL dogfood: PDSI-from-docs** (`~/Documents/Work/FPT/Accelerator/harness-run/`, board `../harness-board.db`).
  Re-derived the Accelerator's pipeline on the real client doc package via the harness spine: t1 ingest (60k-word
  context pack) → t2 SRS (39 REQ-IDs, 39/39 verbatim-traced, 0 unmatched) → t3 architecture (6-service
  services.yaml) → t4 scaffold (kit golden templates via instantiate.py; landed on attempt 2). Hand-authored
  checker scripts as stage gates. First non-code kinds through the spine. ~$16 nominal worker cost.
- **First SELF-HOSTING sprint:** PDSI t4's failures became 3 tickets on THIS repo's board, built by CC workers,
  gated by the spine: e441729 (checker scripts = protected oracles), baf10cf (harden provenance filter —
  went gate-red 0.500 round 1, additive-retry pattern validated → 1.000), acf054d (10-vs-1293 score
  under-reporting root-caused: cosmic-ray dump stdout middleman; now reads session sqlite, partial-never-passes).
  Then t4 re-ran through the repaired gates and landed legitimately (97 stamped files excluded, oracle boundary
  held under live fire, rework channel exercised for real).
- **Review-tier calibration grew:** t1-PDSI true-positive on a criteria defect; multiple honest SUSPECT
  abstentions on truncated diffs (lesson stored: operator re-runs the mechanical checker); t3-self SUSPECT was
  procedural-only (worker correctly refused to write outside its jail).
- **Open / owed:**
  - **t5 PDSI vertical slice — Gary's fork:** real ONNX from the golden repo/bppc vs API-level stub vs park.
    (Lean: park; do the readiness re-assessment next session.)
  - `harden_python` writes cosmic-ray.toml in-worktree (rust uses temp dir) — fix next time harden is open.
  - Commit-trailer convention still credits Opus 4.8; actual driver is Fable 5 — Gary to update or keep.
  - ⚠️ ROTATE the DeepSeek key exposed in chat during the earlier t6 run (carried over, still open).

## 2026-07-15 — PDSI t5 landed with the real production model + readiness re-assessment (Fable 5 session, cont.)

**PDSI t5 (vertical slice) — landed 813373a; the real dogfood is 5/5 COMPLETE.**
- Gary resolved the fork: real ONNX, found locally by an Explore subagent —
  `FPT/pdsi/pdsi-services/pdsi-inference-service/models/fused_nano_multilabel.onnx` (43MB, production version
  y26n-rigft-rig12glv-2026-07-13) with its load-bearing `thresholds.json` beside it (input 1280, conf/iou 0.45,
  letterbox+RGB+/255, per_task_thresh {}).
- **Operator pre-work made the ticket honest** (the reusable pattern): (1) probed the model with onnxruntime —
  the export is **top-K WITHOUT NMS** (20 near-dup persons on a 2-person fixture → NMS mandatory or ~10×
  duplicate violations); output names/shapes documented in the workpad since the worker can't see the golden
  repo; (2) probed the fixture — no_gloves P 0.83–0.93 all persons, helmets WORN (P≤0.29) — so the oracle
  asserts no_helmet ABSENT: a broken decode cannot pass. (3) staged on base c3411db: model+thresholds
  (external-artifact doctrine deliberately bent — worktrees only see tracked files), fixture png+mp4 (ffmpeg,
  re-probed post-encode), protected `scripts/check_slice.py` (clean `compose down -v` → build 6 services →
  health+model_version → upload → job → violations well-formed + discriminating + retrievable by id).
- **Worker run 1** (123 turns, $16.92): all 3 services implemented, per-service tests green, own sanity check
  matched probe evidence. **Verify REFUSED — first live `oracle_intact` catch:** worker extended 2 existing
  template test files in place and deleted the template `items` placeholder (tests+resource). Operator
  adjudicated all 4 diffs benign (additive + boilerplate removal, NOT tamper-to-pass) — but the gate is code:
  used the validated **additive-retry** (guidance appended to notes, re-dispatch same worktree): revert the 2
  files, re-home new tests into NEW files, restore items wholesale (operator ruling: template boilerplate ≠
  out-of-slice stub under AC#5). **Retry PASS**, operator re-ran verify from clean stack: 12 violations,
  types {no_gloves,no_shoes,no_glasses}, model_version match → review → landed. Stack torn down after.
- Harden: loudly vacuous both attempts (59→40 changed files, all template-stamped) — the slice oracle carried
  verification. **Advisory review: 2nd false VIOLATES** (claimed PATCH-vs-POST mismatch; both sides POST,
  grep-refuted in 1 min). Reviewer now 0/2 true-positive on final branches.
- Lesson stored (m1784052283850, PDSI board): plans ordering test authorship must say NEW FILES ONLY —
  base test_*.py are frozen by oracle_intact.

**Readiness re-assessment — DONE (`readiness-assessment.md` 2026-07-15 top section, commit 102b321).**
- June "not ready" verdict RETIRED: its whole critical path built AND validated on real work. **New verdict:
  ready as the *project* driver (CC contained as worker); ad-hoc reversible work stays interactive CC — that
  split IS the posture.** Evidence ledger maps each June gap to live proof.
- Operator-tax split named: **essential** (oracle authorship, keystones — never automate away) vs
  **accidental** (draft hallucination, freeze granularity, CLI burrs — grind down).
- **New critical path:** (1) hygiene+granularity slice — oracle-freeze exemption for template-stamped test
  files, note-append verb, DB-open-before-verb-parse fix, gitignore db side-files, cosmic-ray.toml→temp,
  board-overview verb, DeepSeek key swap on rotation (good self-hosting sprint #2 candidate); (2) draft
  grounding — tree reality into the draft prompt + path existence cross-check; (3) reviewer fix-or-drop
  gated by a number (ground it, 5 tickets, keep iff ≥1 TP & 0 false-VIOLATES); (4) **measured daily-driver
  trial** on the next real ticket-stream (operator-minutes / cost / catches vs misses) — the claim graduates
  on numbers. Confusion bounce parked until a guess-lands-wrong incident.
- decisions.md: harden-vacuity-on-templates ruled DOCTRINE (oracle carries verification; vacuous-but-loud is
  harden staying honest). Rejected: synthetic mutation targets, handwritten-fraction scoring.

**Session hygiene findings (filed into critical-path item 1):** `agent note` overwrites (operator rebuilt full
notes to append retry guidance); binary opens board DB before verb parsing (`--help` touched a stray repo-root
June-era DB); `harness-board.db-{shm,wal}` not gitignored.

**Costs:** t5 ≈ $19 nominal (run1 $16.92 + retry), subscription-credited. Cumulative dogfood ≈ $50.

**Open / owed (carried):** ⚠️ DeepSeek key rotation (Gary) → swap baked value; commit-trailer attribution
(Opus 4.8 vs Fable 5, Gary's call); MCP wrapper + project name (Wu Wei-parked); PDSI project itself served its
dogfood purpose — further build only if Gary wants the product.

## 2026-07-15 — Self-hosting sprint #2: hygiene + granularity slice landed (Fable 5 session, cont.)

Critical-path item 1 of the re-assessment, executed as a 3-ticket sprint on the harness's own board
(`~/Documents/Work/harness-board.db`, t4–t6, blocks-chained), CC workers through our own spine, keystones
pre-authorized by Gary for the run. All three landed same-day; suite 204 → 214 (incl. a new binary-spawning
CLI integration suite), clippy clean throughout.

- **t4 `129d43e` — oracle-freeze granularity.** Protected TEST files template-stamped ON BASE are exempt from
  the verify freeze; `scripts/check_*.py` never exempt; a worktree-dropped or branch-committed stamp unfreezes
  nothing (the tamper vector is designed out and pinned by tests); PASS notes say ` exempt_stamped=N`.
  Mutation 0.970 — the sole missed mutant fails closed. See decisions.md "Oracle-freeze granularity".
- **t5 `66a0c98` — note APPEND + `agent board`.** Append-with-separator default, `--replace` explicit,
  audited, loud on unknown id; overview verb ordered spine-pos → priority → id with per-status counts.
  Paid for itself within the hour: t6's retry guidance was appended with zero pad surgery.
- **t6 `1549c10` — hygiene batch.** Verb-parse before DB open (no/unknown verb → usage/exit 2, ZERO db
  side-files — proven by live demo in an empty dir AND the new integration tests); gitignore `**/*.db-{shm,
  wal,journal}`; `cosmic-ray.toml` → temp dir with the absolute path plumbed through (real `cosmic-ray init`
  probed accepting it from outside cwd). **Gate-red round 1 (0.400):** the diff was ~all main() dispatch glue,
  so the usually-ratio-absorbed dispatch mutants dominated; additive-retry with the survivor list + an operator
  amendment of its own over-broad "no new tests/ dirs" constraint → new `crates/agent/tests/cli_dispatch.rs`
  (std-only, `env!("CARGO_BIN_EXE_agent")`) → 1.000.
- **Findings the sprint itself produced:** (1) **stale-worktree collision across board generations** — June's
  `harness/t4` branch got silently reused; the worker verified both sides, reported Blocked, touched nothing
  (model-side integrity held; $1.68 spent). Guard filed on-trigger. (2) **verify has no empty-diff floor** —
  the untouched branch reached review with verify=PASS; review erred honestly. Filed. (3) One transient CC API
  drop parked honestly (outcome=error, $0.44) — park-and-continue exercised for real. (4) Land friction: wiki
  breadcrumbs dirty base → stash dance per land (minor burr).
- **Reviewer fix-or-drop ledger:** +3 samples — t4/t5 SATISFIES/high (correct), t6 SUSPECT/medium (honest
  procedural abstention, redundant with the machine gate). Running: 0 TP, 0 false VIOLATES on this sprint.
- **Costs:** ≈ $17.8 nominal worker spend (subscription-credited) + cents of DeepSeek drafts/review. Drafts
  re-confirmed the grounding gap (t6's draft asked "what is the main binary?") — item 2 is next.

## 2026-07-15 — Draft grounding landed (critical-path item 2) (Fable 5 session, cont.)

t7 `e4c5e2e`, single dispatch, first-attempt clean through every gate (37 turns $2.64; mutation 0.978 —
sole miss is the warning-println guard, stdout-only; SATISFIES/high; 227 tests, clippy clean).
- **Tree injection:** `git::ls_files` gathered in `run_draft`, passed as DATA into `build_draft_messages`;
  relevance-ordered (longest title-token match first, ties lexicographic), clamped at 100 entries with a
  loud `+N more` tail; section absent entirely on an empty tree (outside-repo drafting degrades unchanged).
- **Prompt rule:** name only tree-listed paths, `create <path>` for new files, ask instead of inventing.
- **Cross-check:** pure `extract_pathish` (backtick/quote/punct-stripped, URL-excluded, extension-aware) +
  `ungrounded_paths` (exact-or-dir-prefix grounding); flag-never-block — Align stays the gate.
- **Live demo** (scratch board, drafted inside this repo): warning fired — flagged `worker.rs`,
  `cli_dispatch.rs` (bare filenames, not repo-relative — exactly the hallucination class), plus one
  false-positive pathish token (`keys/values` from prose; noise, acceptable at flag severity). The draft's
  open questions now ask "which file defines the handler?" instead of inventing one — the intended behavior
  shift, observed on the first grounded draft.
Critical path: items 1+2 DONE; next = reviewer fix-or-drop (needs 5 real tickets) and the measured trial.

## 2026-07-15 — Self-hosting sprint #3: integrity-guard slice (t8+t9, both first-attempt clean)
Both hard-gate holes from sprint #2's stale-worktree collision are closed, built by CC workers through the
spine. **t8 `42f5cc9`** — verify empty-diff floor: refuse before validation runs, no board writes on refusal
(35 turns $1.78, mutation 1.000, SUSPECT/medium = honest can't-see-suite abstention). **t9 `b8610de`** —
fork-point guard in `ensure_worktree`: fail-closed refusal of stale `harness/<id>` branches
(merge-base ≠ base HEAD) with fork sha + behind-count + three remedies; `HARNESS_ALLOW_STALE_FORK=1` hatch;
testable core takes `allow_stale` as a param (11 turns $2.61, mutation 0.800 — sole survivor is the env-read
line, untestable by the no-env-mutation pin, fails closed unset; SATISFIES/high). Post-land: 233 tests green,
clippy clean. Live demos: untouched branch refused at verify with remedies; stale branch refused naming fork
sha; hatch warns loudly and proceeds. Draft calibration: t8's draft parse-failed once (DeepSeek trailing-comma
JSON) and targeted review instead of verify but asked instead of inventing; t9's draft was the best yet (right
function, right merge-base check — tree grounding pays). Operator burr filed: `agent new` is title-only
(auto-id); no retitle verb (fixed my mis-created titles via operator SQL). Reviewer ledger: +2 clean samples
(→ +6 total, 0 false VIOLATES, 0 TP opportunities). ~$4.4 nominal, subscription-credited. Decisions recorded:
"Verify refuses an empty diff" + "Fork-point guard fail-closed". **Next: the measured daily-driver trial on a
PDSI continuation (critical-path item 4) — Gary's pick via AskUserQuestion this session.**

## 2026-07-15 — Measured trial started: instrument + PDSI t6 landed (trial 1/5)
Critical-path item 4 began by the book: `trial-ledger.md` created BEFORE ticket 1 (columns + decision
rules pinned; keystones per-ticket, unpre-authorized — the keystone interaction is measured tax).
**PDSI t6 `f55fa6d`** — notification pipeline, worker first-attempt clean (100 turns $9.81, 55 unit
tests + 7-phase protected oracle green from clean stack). Two product forks pushed to Gary at align:
Redis **Streams over the contracted pub/sub** (KB-grounded: fire-and-forget loses alerts; contract
amended on base) and **mailpit SMTP sink** (oracle asserts physical receipt — 13 emails, seeded
recipient; missed-while-down phase: synthetic violation XADDed while the service was STOPPED, delivered
after restart). Ledger says the real story: ~95 operator-min, ~50 of it debugging non-worker problems —
3 harness findings filed (cosmic-ray shlex/&&-in-test-command; provenance dir-stamps swallowing
handwritten files → mutation scope collapsed to 1 file; oracle-stack container_name collision across
compose projects), 2 operator-oracle bugs (uncaught ConnectionResetError; `def http` module shadow).
Gates held honest throughout: 3 env verify-FAILs bounced cleanly, no faked rows; harden 0.845 loud;
oracle_intact clean. Reviewer: +1 honest-abstain → **+7 clean, 0 TP, 0 false-VIOLATES (trial 1/5)**.
Post-land: 57 pre-existing tracked .pyc untracked on base (t5-era debt the reviewer fairly flagged);
all stacks torn down. Next: t7 web-frontend view, t8 streaming-gateway.

## 2026-08-02 — Long-horizon run: trial window CLOSED 5/5 (t7–t10 landed same-day)

Batch keystone pre-auth (Gary, AskUserQuestion 2026-07-25) drove four tickets end-to-end unattended:
**t7** `ac88f2b` violations read API (oracle 7/7; wall-clock timeout ate telemetry, work complete);
**t8** `33ef1a5` Angular violations view (Playwright oracle 7/7; oMLX:8000 loopback-absorption env
bounce → core-api republished on 28000); **t9** `4280d4b` streaming-gateway RTSP ingest (**first real
mutation gate of the trial: 0.745, 388 killed**; worker first-attempt clean $11.02/122t);
**t10** `e6924a7` full-stack e2e (worker found+fixed a real upload→inference boot-order race;
12-service oracle green). PDSI board 10/10 done. **Reviewer fix-or-drop resolved: DELETE per pinned
rule (0 TP / 5 tickets / +12 clean samples)** — execution queued as B6; cross-vendor successor design
on its trigger. Full rows + findings: trial-ledger.md. Critical-path item 4 evidence COMPLETE — the
graduation judgment on the numbers is Gary's (A6). Next: B hardening slice (pre-authorized).

## 2026-08-02 — B hardening slice COMPLETE: t10–t14 landed, board 14/14

The post-trial B-slice ran through the harness's own spine same-day (Gary's "Continue" = go-ahead on
the grown scope). Five tickets, land order: **t10** `361ea3b` reviewer DELETE executed — `run_sprint`
lost the automatic advisory-review call + its `review` knob; `agent review` stays manual (B6; the
one harden bounce of the slice was a CORRECT catch — 0.667 on an untested `sprint` dispatch arm,
killed by an additive binary-level test in `cli_dispatch.rs`, final 1.000). **t11** `3564680`
re-verify from the review band via the already-legal Review→InProgress edge, no spine change
(finding #5). **t12** `6378af3` honest vacuous harden note — a `.ts`-only diff now names the
unmutatable extensions instead of lying "no code files changed" (finding #4, `UNMUTATABLE_CODE_EXTS`).
**t13** `ced358a` cosmic-ray test-command auto-scopes to the first `&&` segment, loud when it fires,
bail on empty (finding #1; NO new DB column — Wu Wei). **t14** `8778f30` provenance granularity by
diff status: files ADDED under a pre-existing base stamp are handwritten and stay in mutation scope;
fresh-instantiation (stamp added in-diff) stays excluded (finding #2; `changed_files_status_against`
+ `nearest_template_stamp`). Worker record: 6 dispatches / 5 tickets, $9.11 total ($1.23+$1.00,
$1.78, $1.58, $1.44, $2.08), t11–t14 all first-attempt clean through sprint (the first post-DELETE
sprint runs — no review calls burned). Close-out evidence: 246 tests green + clippy -D warnings clean
on final main, tree clean, zero worktrees. **B4** = doctrine, not code → PDSI board lesson
m1785673524282 (compose teardown must use the worktree's project name). **B5 remains parked on
Gary's DeepSeek key rotation.** New finding filed (not fixed, Wu Wei): `agent verify` invoked from
INSIDE the worktree misfires the empty-diff floor with a misleading "branch is empty" bail —
`base_branch` resolves to the branch itself; operator doctrine = run verify from the base repo.

## 2026-08-02 — A6: GRADUATED (Gary) + B5 closed permanently

Gary ruled on the trial-ledger numbers: **the harness graduates to daily driver** — new multi-ticket
project work defaults to the spine; ad-hoc stays interactive CC by posture. Also ruled same session:
the baked DeepSeek key is a **permanent throwaway** (B5 CLOSED, dropped from owed lists, do not
resurface) and the real-CCTV e2e addendum is dropped. Full decision records in decisions.md. Remaining
non-Gary items: next-window ledger instrument (estimate column + comparator arm), stream-event cost
accumulation (telemetry gap fix), Omnigent teardown → research/17 (its trigger — the reviewer DELETE —
has fired), cross-vendor reviewer design gated on that teardown.

## 2026-08-03 — Operator skill drafted + GitHub publish prep (fresh snapshot staged)

**Operator front-end resolved as a skill** (breadcrumb in active-work.md): `~/.claude/skills/harness-operator/`
(personal, carries machine paths) + `skills/harness-operator/SKILL.md` (portable copy, tracked in-repo).
MCP wrapper and ratatui TUI both REJECTED — keystones stay off programmatic surfaces; CC is the interface.

**Publish prep (Gary: fresh snapshot, scrub FPT, keep PDSI codename):**
- Key hygiene `db1b100`: DeepSeek baked key REMOVED (env-only `DEEPSEEK_API_KEY`, bail with guidance —
  its own comment carried the trigger "ROTATE if this repo ever goes public"); oMLX key now resolved at
  runtime (`OMLX_API_KEY` → `~/.omlx/settings.json auth.api_key` → empty) via `provider::omlx_key()`;
  recorder redacts the *resolved* value, empty-secret guarded (`str::replace("")` garbles). 247 tests +
  clippy green; release binary rebuilt. NOTE: `agent review` now needs the env var exported.
- `c0e14ce`: LICENSE (MIT, gaztrabisme + pi-iso third-party note — pi-iso NOTICE already carried the
  upstream MIT text since lift), README rewritten (June "Phase 0 spike" text was stale → graduated
  daily-driver story), portable skill. `41df8b1`: test-key must not embed real key digits (audit catch).
- **Snapshot** `~/Documents/Work/harness-public/` — `git archive HEAD`, FPT/ → client/ in 3 wiki files,
  fresh git, single commit `d3f46f9` (115 files). Audit clean: no keys, no FPT, no key-shaped strings.
  Living repo keeps full history (which contains the dead DeepSeek key + FPT paths — that is WHY the
  public repo is a fresh snapshot, not a mirror). NOT PUSHED — staged for Gary's review.

## 2026-08-03 — PUBLISHED: github.com/gaztrabisme/harness (public)

Snapshot `d3f46f9` pushed as `main` (Gary's go after review). Skill ships in-repo at
`skills/harness-operator/`. Living repo stays private/local; future publishes = re-snapshot
(archive → scrub → new commit on the public repo), not a mirror.
