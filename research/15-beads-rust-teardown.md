# beads_rust (br) Teardown — Board Substrate Decision (DR2-prime)

> Source: `git clone --depth 1 https://github.com/Dicklesworthstone/beads_rust`
> → `/Users/GaryT/Documents/Work/Skills/beads_rust`. Read the actual Rust source (storage schema, model,
> close_policy, coordination, mcp, lib layout, LICENSE), not just the README. `br` is **Jeffrey Emanuel's**
> independent Rust reimplementation of Steve Yegge's beads — agent-first issue tracker, **SQLite + JSONL,
> no Dolt**, ~180k LoC, edition 2024, nightly-pinned, MIT-with-rider. Version 0.2.15.
>
> **Headline:** this teardown **overrides a load-bearing DR2 premise.** DR2 (against the *Go* beads) said
> "beads has **no transition enforcement and no gate states** — that's our net-new value." That is **false for
> `br`.** `br` already ships a **configurable workflow state machine** (`.beads/policy.yaml`:
> allowed-status set + allowed-transition map + per-transition **gate engine** with provider verdicts in a
> `gate_results` table, enforced at the update/close chokepoint). The exact thing DR2 said we'd hand-roll
> on top of beads **already exists in `br`.** That reframes the whole decision.

---

## 1. Storage schema (vs our DR2 schema)

**Engine:** pure-Rust SQLite — **`fsqlite` 0.1.7** ("frankensqlite") + 15 `fsqlite-*` transitive crates, NOT
`rusqlite`/`sqlx`. This is a major liability (see §3). Schema header (`src/storage/schema.rs:15`): *"Schema
matches classic bd (Go) for interoperability."* So `br`'s on-disk shape ≈ DR2's extracted Go schema —
**minus Dolt, minus the wisp/await/swarm overlay columns.** `CURRENT_SCHEMA_VERSION = 13`; migrations are
imperative Rust (`run_migrations`, `ensure_columns`, `rebuild_issues_table`) not numbered `.up.sql` files.

**`issues` table** (`schema.rs:21`) — the one fat table, ~38 columns. The workpad-relevant core:
`id TEXT PK`, `content_hash`, `title CHECK(len<=500)`, `description`, **`design`**, **`acceptance_criteria`**,
**`notes`** (← three of our four WORKPAD fields already exist as columns), `status TEXT DEFAULT 'open'`,
`priority INT CHECK 0..4`, `issue_type`, `assignee`/`owner`/`created_by`, `created_at`/`updated_at`/
`closed_at`, `close_reason`, `closed_by_session`, `due_at`/`defer_until`, `external_ref`, `ephemeral`/`pinned`/
`is_template`, `compaction_level`/`compacted_at`/`original_size`, and two new-to-`br` columns worth noting:
- **`agent_context TEXT`** (v11, #297) — *canonical-JSON governing instructions inherited by descendants* on
  `br update --status in_progress`/`--claim` and `br show`. This is a **constitution/case-law-ish inheritance
  primitive baked into the row** — directly relevant to our P5 governance split.
- A **`CHECK` invariant** binding `closed_at` to `status='closed'` (closed ⇒ closed_at set; non-terminal ⇒ null).

There is **no `started_at` column** (Go beads had one; `br` derives "started" from events). Note: unlike DR2's
ideal schema, the four workpad fields exist but **`plan` and `validation` do not** — `br` has
`design`/`acceptance_criteria`/`notes` (3 of 4). `validation`/proof-of-work has no dedicated column here;
it lives in `close_reason` + `gate_results` + `close_metadata`.

**`dependencies` table** (`schema.rs:116`) — **simpler than Go beads, closer to a plain edge.** Columns:
`issue_id`, `depends_on_id`, `type TEXT DEFAULT 'blocks'`, `created_at`, `created_by`, `metadata`, `thread_id`.
**PK `(issue_id, depends_on_id)`.** Differences from DR2's Go schema: **no tagged-union target split**
(`depends_on_issue_id`/`wisp`/`external` + CHECK) — it's one `depends_on_id` column, with the FK on
`depends_on_id` **intentionally removed** so it can point at external/not-yet-created issues. **No
deterministic surrogate edge `id`** — the natural `(issue_id, depends_on_id)` pair *is* the key (which is the
merge-safe property the Go `0050` migration spent effort to recover; `br` gets it for free by not having a
UUID column). The edge `type` taxonomy lives in the model (`DependencyType`, `model/mod.rs:270`): the **11
well-known kinds** `blocks, parent-child, conditional-blocks, waits-for, related, discovered-from, replies-to,
relates-to, duplicates, supersedes, caused-by` + `Custom(String)`. The single load-bearing predicate is
**identical to DR2's**: `affects_ready_work()` = `is_blocking()` = `{Blocks, ParentChild, ConditionalBlocks,
WaitsFor}` (`model/mod.rs:326`). A composite partial index `idx_dependencies_blocking` materializes exactly
this 4-type subset.

**`blocked`/`ready` — NOT a recursive CTE.** This is the biggest *implementation* divergence from Go beads
(and from DR2's ported design). `br` uses an **app-side materialized cache table `blocked_issues_cache`**
(`issue_id` PK, `blocked_by` = JSON array), rebuilt **in Rust** by
`SqliteStorage::compute_blocked_issues_map_impl` (`sqlite.rs:5668`): load direct blockers →
`propagate_blocked_parents` (parent→child inheritance) → add parents-with-open-children. `ready` =
`status='open' AND ephemeral=0 AND pinned=0 AND is_template=0 AND id NOT IN blocked_issues_cache`
(`idx_issues_ready` partial index + `sqlite.rs:5003`). **Why hand-rolled, not a CTE:** explicit code comments —
*"the embedded SQLite backend mishandles recursive CTEs"* (`sqlite.rs:4922`), and cycle detection was rewritten
as *"Iterative BFS … (replaces recursive CTE)"* (`sqlite.rs:2380`). **fsqlite cannot reliably run recursive
CTEs.** This means DR2's "port beads' recursive-CTE `ready_issues` view" plan is **not** what `br` does, and is
**not** portable onto fsqlite — the transitive-blocked walk is Rust BFS over loaded edges.

**Side tables:** `labels` (issue_id,label), `comments` (autoinc id, author, text), `events` (audit:
created/updated/status_changed/closed/reopened/dependency_added… + Tier-1 attribution `agent_name`/`harness`/
`model`), `config`/`metadata` (KV, no PK — *"storage engine does not reliably maintain unique autoindexes"*),
`dirty_issues`+`export_hashes` (incremental JSONL export), `blocked_issues_cache`, `child_counters` (hierarchical
IDs), and **three governance tables new to `br`:** `close_metadata` (per-close attribution + `bypassed_policy` +
`policy_gates_fired`), **`gate_results`** (`(issue_id, gate, provider)` → pass/fail/note — see §2), and the
close-policy machinery.

**ID scheme** (`util/id.rs`, `util/hash.rs`): `prefix-<base36hash>`, adaptive length via birthday-bound
(`max_collision_prob 0.25`, length grows on collision), nonce 0..10 appended on collision; `content_hash` =
sha256 over Go-bd canonical field order (cross-tool dedup). Hierarchical child IDs `bd-abc.1.2` via
`child_counters`. Matches DR2's description of the Go scheme.

**Fit vs DR2 schema:** ~90% overlap on the *core* (issues + typed edges + the 4-kind ready predicate +
hierarchical IDs + content-hash + snapshot/decay bookkeeping). `br` is **leaner on the edge table** (no
tagged-union, no surrogate id — both *simplifications* DR2 would accept) and **richer on governance** (gate
engine + agent_context + close_metadata, which DR2 didn't know existed). The decay/compaction columns exist
but the AI-summarizer pipeline is thinner than Go beads.

## 2. State model + cost to add our spine/workpad

**Built-in `Status`** (`model/mod.rs:58`): `Open, InProgress, Blocked, Deferred, Draft, Closed, Tombstone,
Pinned, Custom(String)`. `Custom` means **any string is a valid status** — free-set at the type level, exactly
as DR2 reported. `is_terminal` = {Closed, Tombstone}; `is_active` = {Open, InProgress}. `Priority(0..4)`,
`IssueType` (task/bug/feature/epic/chore/docs/question + Custom).

**BUT transition enforcement EXISTS — this is the override.** `src/close_policy.rs` implements a full,
opt-in, config-driven **workflow state machine** loaded from `.beads/policy.yaml` (`Workflow` struct,
`close_policy.rs:155`):

```yaml
workflow:
  strict: true
  statuses: [open, in_progress, in_review, blocked, deferred, draft, closed]   # allowed-status SET
  transitions:                       # allowed-transition TABLE (from -> [to...])
    open: [in_progress, deferred, closed]
    in_progress: [in_review, blocked, open]
    in_review: [closed, in_progress]
    # any: [closed]                  # wildcard source; initial: [...] for create
  gates:                             # per-transition GATE ARTIFACTS
    "in_review -> closed":
      require_all: [ci_green, {min_reviewers: 1}]
      require_if:
        - {label: security-sensitive, gate: security_sign_off}
        - {priority: [0,1], gate: security_sign_off}
```

- `Workflow::validate_transition(from, to)` (`close_policy.rs:683`) **rejects any `from->to` not in the map**
  on `br update`, gated on `strict && !transitions.is_empty()`. No-op `from==to` always allowed.
- **Gate engine** (`close_policy.rs:455` `evaluate_gates`, `:385` `required_gates_for`): a transition can
  require named provider verdicts (`GateSpec::Named` satisfied by a recorded `pass` in `gate_results`) or the
  built-in `GateSpec::MinReviewers(n)` (n distinct `reviewer*` providers passed), plus **conditional gates**
  matched by label and/or priority (`ConditionalGate`). Verdicts are written via `br gate report <id> --gate
  --provider --status pass|fail` → `gate_results`, read by `br gate list`. Enforcement is hooked at the
  close/update chokepoint; bypassable via `--bypass-policy` (audited in `close_metadata.bypassed_policy`,
  itself disable-able via `allow_bypass: false`).
- **Close-time policy** (`ClosePolicy`): required-field validation (close-reason min length/regex, unchecked
  acceptance-criteria boxes), self-close forbiddance, Tier-1 agent attribution capture.

**What this means for our spine.** Map `Todo→Align→In Progress→Verify→Review→Land→Done (+Rework)` onto
`br` as **custom statuses + a `transitions` map + `gates` rules**, *with zero schema or source changes*:
- `align, verify, review, landing` become custom statuses in `workflow.statuses`; the spine becomes the
  `transitions` map (`todo:[align]`, `align:[in_progress]`, `in_progress:[verify]`, … `any:[rework]`,
  `rework:[align]`). `validate_transition` enforces it.
- The **four keystone gates** become `gates` rules: `"align -> in_progress"` requires gate `criteria_confirmed`;
  `"verify -> review"` requires `mutation_score` (a provider verdict — our harness runs the mutator and calls
  `br gate report --gate mutation_score --status pass`); `"review -> landing"` requires `min_reviewers`/
  `review_approved`; `"landing -> done"` requires `land_proof`. **Our align-gate / Verify harden gate / Land
  proof become provider-reported `gate_results`, and `br` blocks the transition until they pass.** This is
  *exactly* the "allowed-transition table + per-gate artifact" delta DR2 budgeted as net-new — **already built.**
- **Workpad fields:** `design`, `acceptance_criteria`, `notes` exist as columns (3/4). **Missing: `plan` and a
  dedicated `validation`/proof column.** `acceptance_criteria` = our Acceptance (the Verify oracle); `notes` =
  Notes/Confusions; `design` ≈ Plan (reusable). `validation` evidence currently rides in `gate_results` +
  `close_reason` rather than a column. Adding true `plan`/`validation` columns = two `ensure_columns` entries
  + `Issue` struct fields — trivial in a fork, but a **schema fork** (breaks JSONL bd-conformance).

**Cost verdict:** the state-machine + gate spine is **a config file (`policy.yaml`)**, not code — *if* we adopt
`br`'s model. Adding `plan`/`validation` as real columns is a small **fork**. The enforcement semantics
(deny-transition-until-gate-passes) we'd otherwise hand-roll are **already implemented and tested** (132 test
files; `gate.rs` has unit tests for required-gate computation).

## 3. Architecture & modularity (library-usable?)

**Crate layout:** single crate `beads_rust`, **`[lib]` + `[[bin]] br`** (`src/main.rs`). `lib.rs` exposes
**all modules `pub`**: `storage, model, close_policy, coordination, policy, config, sync, format, output, mcp
(feature-gated), error, util, cache, health, inheritance, validation`. So it **is** importable as a library
crate in principle. Module boundaries are clean by name (storage/model/sync/cli/mcp).

**But the storage+model layer is NOT cleanly severable:**
- **Storage is a concrete `SqliteStorage` struct, not a trait** (`storage/mod.rs` re-exports `SqliteStorage`,
  `IssueUpdate`, `ListFilters`, `ReadyFilters`…). `sqlite.rs` is **22,372 LoC in one file** — ready-cache,
  blocked propagation, gate-result I/O, JSONL dirty-tracking, fsqlite-workaround scar tissue all interleaved.
  You cannot lift "just the schema + model" without dragging `SqliteStorage` and therefore **`fsqlite`**.
- **The `fsqlite` dependency is the dealbreaker for vendoring.** It's a from-scratch pure-Rust SQLite
  (15 alpha-version `fsqlite-*` crates at 0.1.7) whose quirks are load-bearing in `br`'s code: no
  `execute_batch` (hand-rolled SQL splitter), **no reliable recursive CTEs** (hence the Rust BFS blocked-cache),
  unreliable unique autoindexes (hence PK-less KV tables), stale in-memory schema cache after ALTER
  (hence table rebuilds), WAL/page anomalies (hence `doctor --repair`). Adopting `br`'s storage = **adopting
  fsqlite and all its workarounds.** Our DR2 plan assumed boring, battle-tested `rusqlite`. This is a
  *downgrade* in storage maturity.
- **Edition 2024 + `rust-toolchain.toml` pinned to `nightly-2026-02-19`.** Vendoring forces nightly on our
  whole harness (or a toolchain fork). The KB-MCP is already Rust but presumably stable.
- Lints are `clippy::pedantic+nursery = deny` — a strict house style we'd inherit or fight.

**Feature flags:** `default = ["self_update"]`; `mcp = ["dep:fastmcp-rust"]` (only 13 `cfg(feature)` sites —
MCP is genuinely optional and cleanly gated). `self_update` pulls reqwest+rustls. Release profile is
`opt-level=z, lto, panic=abort, strip` (size-optimized single binary).

**Dependency weight:** clap, serde/serde_json/serde_norway, schemars, chrono, sha2, anyhow/thiserror, tracing,
indicatif/crossterm, **`rich_rust`/`toon_rust`/`tru`** (the author's sibling crates — more single-maintainer
alpha deps), regex, similar, semver, mimalloc, signal-hook. Plus fsqlite×15 and (optional) fastmcp-rust.
**Heavy, and laced with the author's own pre-1.0 sibling crates** (fsqlite, rich_rust, toon_rust, fastmcp-rust)
— a supply-chain profile that violates our "dependencies are liabilities" rule hard.

**Library-usable? Technically yes (it's a lib), practically no for VENDOR/depend-as-crate** — the storage core
is welded to a 22k-LoC fsqlite-coupled struct, nightly-pinned, edition-2024, with a single-maintainer alpha
SQLite at the bottom. You'd be taking the whole stack or nothing.

## 4. Agent/MCP surface

**MCP server** (`br serve`, feature `mcp`, via **`fastmcp-rust`**; not in default build). Surface
(`src/mcp/tools.rs`, `resources.rs`, `prompts.rs`):
- **7 tools:** `list_issues, show_issue, create_issue, update_issue, close_issue, manage_dependencies,
  project_overview` (with batch support). Stateful via an `Arc` shared `SqliteStorage`.
- **Resources:** `coordination_status` (read-only claim diagnosis), `project_info`, per-issue resources —
  cached JSON.
- **Prompts:** triage/standup/planning-style prompt handlers with validated args.
- CLI parity: every command supports `--json`/`--robot`; `br schema` (JSON schemas), `br capabilities
  --format json` (machine contract describing commands + safety guarantees), `br coordination status --json`.

**Coordination / stale-claim model** (`src/coordination.rs`, `cli/commands/coordination.rs`): a **pure,
advisory evidence classifier** — explicitly *"does not inspect Agent Mail, read the filesystem, or mutate
claims."* It takes issue metadata + **Agent Mail snapshots passed in as JSON** (`--reservations`, `--agents`
files: `AgentMailReservationSnapshot`, `AgentMailAgentSnapshot`) and classifies an in-progress claim as
fresh / stale-candidate / abandoned-likely by owner class (`SwarmAgent` 2h/8h, `Human`/`Unknown` 24h/72h
thresholds), matching reservations to issues by holder==assignee / issue-id-in-reason / comment-path. Output is
an **advisory** (`advisory_for_claim`), schema `br.coordination.v1`.

**Integration with mcp_agent_mail:** **loose and one-directional.** `br` is the *data plane*; Agent Mail
(separate MCP server by the same author) is the *reservation/locking plane*. `br` **reads** Agent Mail
snapshots to *diagnose* stale claims but never **writes** claims or talks to Agent Mail directly. So:
**we do NOT get coordinator↔worker plumbing for free.** What `br` gives is (a) a board with claim/assignee
state, (b) an advisory "is this claim stale?" classifier over externally-supplied evidence. The actual
mutex/lease/locking is mcp_agent_mail's job — a *second* external server, with its own integration cost — and
our DR5 coordination layer (git-worktree-isolated workers + single-writer coordinator) is a **different model**
than the multi-agent-mail-reservation pattern `br` assumes. The stale-claim advisory is a nice reference
design; it is not our dispatcher.

## 5. License rider

**MIT + "OpenAI/Anthropic Rider"** (`LICENSE`, © 2026 Jeffrey Emanuel). Standard MIT grant, then a rider that
**controls on conflict**:
- **"Restricted Parties" = OpenAI, Anthropic, their Affiliates (>50% control), and anyone acting on their
  behalf/benefit/direction** (employees, contractors, agents, service providers…). **No rights whatsoever are
  granted to any Restricted Party**; you **may not** provide/host/make-available the Software or Derivative
  Works to or for them, and "use" expressly includes incorporating it into any **training corpus, evaluation
  harness, or ML pipeline.** Breach auto-terminates the license; injunctive relief + fees clause.
- **Impact on us (personal harness):** Gary is an individual building a forever-personal dev harness — **not**
  OpenAI/Anthropic nor acting "on behalf of / for the benefit of / under the direction of" them. Forking,
  vendoring, modifying, and running `br` for personal use is **squarely permitted** by the MIT grant; the rider
  doesn't restrict *us*. **Three live cautions, none blocking:**
  1. **Don't feed it to a Restricted Party's models/pipelines.** Our harness drives **oMLX (local Qwen)** and
     optionally **Anthropic/OpenAI** APIs. If a worker model were to ingest `br` source into an *eval harness or
     training pipeline*, that's arguably "use … for the benefit of" a Restricted Party. Running `br` as a *tool*
     a Claude/GPT agent *calls* is normal use; **piping its source into an Anthropic/OpenAI eval/training
     pipeline is the line to avoid.** Since our design is local-first and we'd own a fork, easy to honor.
  2. **Derivative-work propagation:** any distribution of a fork must carry the rider unmodified. We don't
     distribute, so moot — but a vendored copy in our repo technically carries the obligation if ever shared.
  3. It's an **unusual, untested license** — a soft reason to prefer REFERENCE over a deep vendor entanglement.

---

## Decision — **REFERENCE (read, hand-roll anyway), with one ADOPT-the-design lift**

**Not ADOPT (run `br`/its MCP as our substrate):** it drags in **fsqlite** (single-maintainer alpha pure-Rust
SQLite with load-bearing quirks), **edition-2024 + pinned nightly**, and a stack of the author's pre-1.0 sibling
crates — a supply-chain and maturity profile that **violates "dependencies are liabilities"** harder than
hand-rolling. It's also a *full issue-tracker CLI* (labels/comments/epics/audit/doctor/sync/coordination/
capabilities/conformance, ~180k LoC) where our spine needs a fraction. **Wu Wei:** adopting it imports a huge
surface to use ~10%.

**Not VENDOR (lift `src/storage` + `src/model`):** not severable. `model/mod.rs` (1.9k LoC) *is* liftable in
isolation, but storage is one **22k-LoC `SqliteStorage` welded to fsqlite**; you cannot take the schema/ready/
blocked logic without taking fsqlite and its workaround scar tissue. The blocked/ready logic is fsqlite-shaped
(Rust BFS *because* CTEs don't work) — not the clean `rusqlite` recursive-CTE we want.

**Not FORK (own the whole source):** owning 180k LoC + fsqlite + nightly to delete 90% is negative leverage.

**REFERENCE is right — and `br` is a *better* reference than Go beads for our exact problem**, because it
already solved the two things DR2 thought were net-new:
1. **The state-machine + gate engine design is the headline lift.** `br`'s `close_policy.rs` (`Workflow`
   {`statuses`, `transitions` map, `gates`} + `GateSpec`/`GateRule`/`ConditionalGate` + `evaluate_gates` +
   `validate_transition`, backed by a `gate_results(issue_id, gate, provider, passed)` table and `br gate
   report/list`) is **almost exactly our Align/Verify/Review/Land allowed-transition-table + per-gate-artifact
   design** — and it's small, clean, and unit-tested. **Port this design 1:1 into our Rust+rusqlite board**
   (`close_policy.rs` is ~3k LoC but the load-bearing structs are ~250 lines). This is the single biggest
   time-save and it lands *as our own code on our own storage*, with none of the fsqlite/nightly tax.
2. **`agent_context` inheritance** (governing-JSON inherited by descendants on claim/in-progress) is a clean
   prior-art primitive for our P5 constitution/case-law propagation — port the *idea*.
Also reference: the `gate_results` provider-verdict shape (our mutation-score / land-proof become provider
reports), the `close_metadata` bypass-audit pattern, and the coordination stale-claim **thresholds** as a
default for our own (different, worktree-based) dispatcher.

Net: **hand-roll the lean board on rusqlite as DR2 already decided — but copy `br`'s `close_policy` Workflow+gate
model verbatim instead of inventing our state machine.** DR2's "we add the transition graph + gate artifacts
beads omits" becomes "we **port** `br`'s transition graph + gate engine," which is strictly less work and
de-risks the keystone WORK-plane feature.

## What changes in research/12 + research/13 board row

- **research/12 §2 / §1.4 correction:** the claim "beads has **no transition enforcement and no gate states**"
  is **true of Go beads but FALSE of `br`.** `br` ships a config-driven allowed-status set + allowed-transition
  map + per-transition gate engine (`close_policy::Workflow`, `gate_results` table, `validate_transition`,
  `evaluate_gates`). Annotate §2's delta table: our spine/gates are now a **port target**, not pure net-new.
- **research/12 ready-query correction:** "port beads' recursive-CTE `ready_issues` view" stands for *our*
  rusqlite build, but note `br` itself **abandoned the CTE** (fsqlite can't run them) for a Rust-side
  materialized `blocked_issues_cache` + BFS. We keep the rusqlite CTE (rusqlite *can* run them); `br`'s BFS is
  a fallback design we don't need.
- **research/12 edge-table note:** `br` dropped the Go tagged-union target + surrogate edge id; PK
  `(issue_id, depends_on_id)` is the natural merge-safe key. **Adopt `br`'s simpler edge table** over DR2's
  tagged-union — one less moving part, and we have no wisp/external-id case.
- **research/13 board row** ("Board: SQLite; ticket/edge/snapshot/memory; skip Dolt; JSONL export"): **largely
  confirmed** — `br` independently validates SQLite+JSONL-no-Dolt as the right architecture. **Add:** "state
  machine + gate engine ported from `br`'s `close_policy.rs` (Workflow{statuses,transitions,gates} +
  gate_results table); storage stays **rusqlite**, NOT fsqlite." Add a one-line **Rejected: adopt/vendor/fork
  `br`** (fsqlite alpha + nightly + 180k-LoC issue-tracker surface; reference-only).
- **wiki/decisions.md:** add "`br` (beads_rust) evaluated as board substrate → **REFERENCE only**; port its
  `close_policy` Workflow+gate design onto our rusqlite board." Add Rejected Approaches: "adopt/vendor `br`
  (fsqlite dependency, pinned nightly, edition-2024, single-maintainer sibling-crate supply chain)."

## Sources (paths read)

- `/Users/GaryT/Documents/Work/Skills/beads_rust/Cargo.toml` — fsqlite×15 + fastmcp/self_update features, edition 2024, lints
- `…/rust-toolchain.toml` — pinned `nightly-2026-02-19`
- `…/LICENSE` — MIT + OpenAI/Anthropic Rider (full read)
- `…/src/lib.rs`, `…/src/storage/mod.rs` — crate is lib+bin; `SqliteStorage` concrete struct (no trait)
- `…/src/storage/schema.rs` (3193 LoC, read 1–1343) — `SCHEMA_SQL`: issues/dependencies/labels/comments/events/
  config/metadata/dirty_issues/export_hashes/blocked_issues_cache/child_counters/close_metadata/**gate_results**;
  fsqlite workarounds (SQL splitter, table rebuilds, PK-less KV)
- `…/src/storage/sqlite.rs` (22,372 LoC; read ready/blocked paths) — `compute_blocked_issues_map_impl:5668`,
  `rebuild_blocked_cache`, `get_ready_issues:4815`; comments "embedded SQLite mishandles recursive CTEs",
  "Iterative BFS … replaces recursive CTE"
- `…/src/model/mod.rs` (1919 LoC) — `Status` (free-set + Custom), `Priority`, `IssueType`, `DependencyType`
  (11 kinds + Custom; `affects_ready_work`/`is_blocking` = 4-kind subset), `Issue`/`Dependency`/`Comment`/`Event`,
  `agent_context` field
- `…/src/close_policy.rs` (3072 LoC; read 1–468) — **the gate engine:** `Workflow{strict,statuses,transitions,
  gates}`, `GateRule`/`GateSpec`/`ConditionalGate`, `validate_transition:683`, `evaluate_gates:455`,
  `required_gates_for:385`, `GateResult`; `ClosePolicy` required-field/actor gates
- `…/src/cli/commands/gate.rs` (482 LoC) — `br gate report/list`, `compute_gated_transitions`, unit tests
- `…/src/policy.rs` (head) — separate `AdaptivePolicy` evidence-rules contract (future swarm; pure evaluator)
- `…/src/coordination.rs` (head) + `…/src/cli/commands/coordination.rs` — pure stale-claim classifier over
  Agent Mail snapshots; `ClaimOwnerKind` thresholds; `br.coordination.v1`; does NOT mutate claims / call Agent Mail
- `…/src/mcp/{mod,tools,resources,prompts}.rs` (8208 LoC) — 7 MCP tools via fastmcp-rust, coordination_status
  resource, prompts; feature-gated, not in default build
- `…/src/util/id.rs`, `…/src/util/hash.rs` — base36 adaptive-length hash IDs + nonce collision, sha256 content_hash
- `…/README.md` — `br serve` optional, `--json/--robot`, `capabilities`, `coordination status`; author lineage
  (Jeffrey Emanuel; beads ⨯ mcp_agent_mail ⨯ beads_viewer)
- KB grounding: not consulted — this is a source-teardown of a specific repo, not a literature question;
  no textbook claim is load-bearing here.
