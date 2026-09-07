# beads — Board Schema Extraction (DR2)

> Source: `git clone --depth 1 https://github.com/steveyegge/beads` → `$HOME/Documents/Work/Skills/beads`.
> Read the actual schema (`internal/storage/schema/migrations/*.up.sql`, 50 migrations), the Go type model
> (`internal/types/types.go`, `id_generator.go`), idgen (`internal/idgen/hash.go`), compaction
> (`internal/storage/issueops/compaction.go`, `internal/compact/`), and the prime/remember commands
> (`cmd/bd/prime.go`, `cmd/bd/memory.go`). beads is **Go + embedded Dolt** (a versioned MySQL-compatible DB).
> Verdict up front: **STEAL-the-schema, hand-roll-minimal in Rust+SQLite.** Reasoning in the final section.

---

## 1. Full data model

### 1.1 `issues` (the work item) — the one fat table

beads puts everything in one wide `issues` table (migration `0001`, grown by ~20 later migrations). The
table is heavily denormalized and carries multiple *overlaid* feature sets (issue tracker + agent mailbox +
"wisp"/molecule swarm-coordination + gate/await primitives). The **core** columns that matter for a board:

| Column | Type | Meaning |
|---|---|---|
| `id` | `VARCHAR(255)` PK | the bead ID, e.g. `bd-a3f8` (see ID scheme below) |
| `content_hash` | `VARCHAR(64)` | sha256 of content (drift/dedup) |
| `title` | `VARCHAR(500)` NOT NULL | |
| `description` | `TEXT` | |
| `design` | `TEXT` | **separate field** — design notes |
| `acceptance_criteria` | `TEXT` | **separate field** — maps directly to our workpad Acceptance |
| `notes` | `TEXT` | free-form |
| `status` | `VARCHAR(32)` default `'open'` | see status model |
| `priority` | `INT` default 2 | 0–4 (0 = P0/critical) |
| `issue_type` | `VARCHAR(32)` default `'task'` | bug/feature/task/epic/chore/decision/spike/story/milestone/… |
| `assignee` / `owner` / `created_by` | `VARCHAR(255)` | actors |
| `estimated_minutes` | `INT` | |
| `created_at` / `updated_at` | `DATETIME` | `updated_at` has `ON UPDATE CURRENT_TIMESTAMP` |
| `started_at` | `DATETIME` | added `0027` — when work began (in_progress transition) |
| `closed_at` | `DATETIME` | set on close; **drives compaction clock** |
| `close_reason` | `TEXT` | free text; scanned for failure keywords (conditional-blocks) |
| `closed_by_session` | `VARCHAR(255)` | |
| `external_ref` | `VARCHAR(255)` | link to GitHub/Jira/Linear/etc. |
| `spec_id` | `VARCHAR(1024)` | spec linkage (added `0034`) |
| `is_blocked` | `TINYINT(1)` default 0 | **materialized** blocked flag (added `0046`, recomputed `0047`) |
| `defer_until` / `due_at` | `DATETIME` | scheduling |
| `pinned` / `is_template` / `ephemeral` | `TINYINT(1)` | lifecycle flags |
| `compaction_level` / `compacted_at` / `compacted_at_commit` / `original_size` | | compaction bookkeeping |
| `metadata` | `JSON` default `JSON_OBJECT()` | escape hatch |
| `source_repo` / `source_system` | | multi-repo / import provenance |

The rest of the columns (`wisp_type`, `mol_type`, `work_type`, `hook_bead`, `role_bead`, `agent_state`,
`await_type`, `await_id`, `timeout_ns`, `waiters`, `rig`, `sender`, `event_kind`, `actor`, `target`,
`payload`…) are **swarm/orchestration/messaging overlays** bolted onto the issue row. For our board these
are **noise — do not port them.** They are exactly the "earn its keep" weight the eval warned about.

Side tables: `dependencies` (the graph; below), `labels` (issue_id, label), `comments`, `events` (audit
trail: created/updated/status_changed/closed/reopened/dependency_added…), `config` (KV — also stores
memories), `metadata`/`local_metadata`, `child_counters` (parent_id → last_child for hierarchical IDs),
`issue_counter` (prefix → last_id for sequential IDs), `issue_snapshots` + `compaction_snapshots` (pre-compaction
content archive), plus `wisps`/`wisp_*` (dolt-ignored swarm tables we skip), `routes`, `interactions`,
`federation_peers`. Two **VIEWS** carry the core query logic: `ready_issues` and `blocked_issues`.

### 1.2 ID scheme — hash-based, collision-extended, hierarchically suffixed

Two layers, both in `internal/types/id_generator.go` + `internal/idgen/hash.go`:

- **Root ID** = `prefix-<shorthash>`. The hash is `sha256(title ‖ description ‖ created_RFC3339Nano ‖
  workspaceID)`. The newer `idgen.GenerateHashID` encodes the leading bytes as **base36** (denser than hex),
  length 3–8 chars, with a **nonce** appended to the content on collision. The older path takes
  `hash[:6]` and **progressively extends to 7, then 8 chars on collision** (97% stay at 6). `workspaceID`
  in the hash is what makes IDs **collision-free across clones without coordination** — the property Dolt
  merge needs. Prefix is per-project (`bd-`, `sh-`, etc.); a per-prefix `issue_counter` also supports a
  sequential mode.
- **Hierarchical child ID** = `parent.N` → `bd-a3f8.1`, `bd-a3f8.1.2`. `GenerateChildID` just appends
  `.N`; `child_counters` hands out `N`. **Max depth 3** (`MaxHierarchyDepth`, enforced by
  `CheckHierarchyDepth`), unlimited breadth. `ParseHierarchicalID` returns `(rootID, parentID, depth)` by
  splitting on dots. NOTE: the hierarchical suffix is **string convention only** — the parent/child *edge*
  is still a row in `dependencies` with `type='parent-child'`. The dotted ID is a human/sort affordance,
  not the source of truth for the tree.

### 1.3 Dependency edges — `dependencies` table + a 20-type taxonomy

Edge row (migrations `0002`, reshaped by `0041`/`0043`/`0050`):

```
dependencies(
  id            -- deterministic surrogate (0050): derived from (issue_id, target), NOT random UUID — merge-safe
  issue_id      -- the dependent (FK → issues, ON DELETE CASCADE)
  depends_on_issue_id  / depends_on_wisp_id / depends_on_external  -- 0041 split the single target into 3 typed nullable columns
  depends_on_id -- STORED generated column = COALESCE(issue, wisp, external); part of PK
  type          -- VARCHAR(32), default 'blocks'
  thread_id     -- groups conversation edges (replies-to threading)
  metadata      -- JSON: per-type payload (waits-for gate, attests skill/level, similarity score…)
  created_at, created_by
  PRIMARY KEY (issue_id, depends_on_id)
  CHECK: exactly one of the three target columns is non-null (ck_dep_one_target)
  UNIQUE (issue_id, depends_on_issue_id) etc. -- natural identity; `type` deliberately NOT in the key
)
```

Two design points worth stealing: (a) the **target is a tagged union** (internal issue / wisp / external
ref) with a CHECK enforcing exactly-one — clean way to point edges at things outside the board; (b) the
edge **id is derived deterministically** from `(issue_id, target)` so two clones that independently add the
same logical edge converge to byte-identical rows (the whole point of `0050`; `0043`'s random `UUID()`
default broke Dolt merges — `#4259`). `type` is intentionally **not** part of edge identity (one edge per
pair; its kind can change).

**The full edge taxonomy** (`internal/types/types.go`), with exact semantics and the three behavioral
predicates beads attaches to each:

| Type (string) | Semantics | `AffectsReadyWork` | `IsBlockingEdge` | `IsWellKnown` |
|---|---|:--:|:--:|:--:|
| `blocks` | A blocks B: B not ready until A closed | ✅ | ✅ | ✅ |
| `parent-child` | structural epic→subtask; transitively gates ready | ✅ | ❌ | ✅ |
| `conditional-blocks` | "B runs only if A **fails**" — unblocks when A closes with a failure close_reason (keyword scan: failed/rejected/wontfix/cancelled/error/timeout/aborted…) | ✅ | ✅ | ✅ |
| `waits-for` | fanout gate: wait for dynamic children (`all-children`/`any-children` in metadata) | ✅ | ✅ | ✅ |
| `related` / `discovered-from` | association (non-blocking) | ❌ | ❌ | ✅ |
| `replies-to` | conversation threading (uses `thread_id`) | ❌ | ❌ | ✅ |
| `relates-to` | loose knowledge-graph edge | ❌ | ❌ | ✅ |
| `duplicates` | dedup link | ❌ | ❌ | ✅ |
| `supersedes` | version-chain link | ❌ | ❌ | ✅ |
| `authored-by` / `assigned-to` / `approved-by` / `attests` | entity/HOP edges (attests carries skill+level JSON) | ❌ | ❌ | ✅ |
| `tracks` | convoy → issue, non-blocking cross-project | ❌ | ❌ | ✅ |
| `until` / `caused-by` / `validates` / `delegated-from` | reference / delegation edges | ❌ | ❌ | ✅ |

Custom types: **any non-empty string ≤50 chars is a valid edge type** (`IsValid`); only the list above is
"well-known". The single load-bearing distinction is `AffectsReadyWork()` = `{blocks, parent-child,
conditional-blocks, waits-for}` — **only those four gate `bd ready`.** Everything else is a knowledge graph.

### 1.4 Status / state model

Built-in statuses (`internal/types/types.go`): `open`, `in_progress`, `blocked`, `deferred`, `closed`,
`pinned` (persistent bead, never auto-closes), `hooked` (work actively claimed by a worker). Plus
**custom statuses** via `bd config set status.custom` — each tagged with a **category** that controls view
behavior: `active` (shows in `bd ready` + list), `wip` (in list, **not** ready), `done`, `frozen`,
`unspecified`. Built-in mapping (`BuiltInStatusCategory`): open→active; in_progress/blocked/hooked→wip;
closed→done; deferred/pinned→frozen. So the category system, not the status string, is what `bd ready`
keys on. Priority is a separate `INT` 0–4. There is **no enforced transition graph** — status is a free
field validated only for membership; beads does not gate `open → closed` through intermediate states.

### 1.5 `bd ready` — the unblocked-query (a VIEW, recursive CTE)

`ready_issues` view (final form, migration `0044`). An issue is **ready** iff:

1. status is `open` **or** a custom status in category `active`; AND
2. not `ephemeral`; AND
3. **not transitively blocked** — computed by a recursive CTE: seed = issues with a `blocks` edge to a
   blocker whose status is NOT in (`closed`,`pinned`); then walk **up the `parent-child` tree** (a child
   inherits its parent's blocked-ness), depth-capped at 50; AND
4. `defer_until` is null or in the past, and no `parent-child` parent is itself deferred-into-the-future.

`blocked_issues` is the inverse view, adding a `blocked_by_count`. Note migration `0046` *also*
materializes an `is_blocked` TINYINT on the row (recomputed by the same CTE) as an index-friendly cache —
so beads has **both** a live view and a denormalized flag. Cycle detection is a separate in-tree path
(`internal/storage/issueops/cycles.go`, `bd dep cycles`), run on add (`--no-cycle-check` to skip).

### 1.6 Compaction / decay — closed issues forgotten on a clock

This is beads' "structured memory decays" mechanism (`internal/storage/issueops/compaction.go`,
`internal/compact/`). Two tiers, time-gated off `closed_at`:

- **Tier 1** eligible when: status=`closed` AND `closed_at` ≥ **30 days** ago (config `compact_tier1_days`)
  AND `compaction_level < 1`.
- **Tier 2** eligible when: same, ≥ **90 days** (`compact_tier2_days`) AND already Tier-1 (`level == 1`).
- Open/recently-closed issues are **never** eligible (fail-fast in `CheckEligibilityInTx`).
- On compaction: the full pre-compaction content (description+design+notes+acceptance) is archived to
  `issue_snapshots` / `compaction_snapshots`, then an **AI summarizer (Claude Haiku, `internal/compact/haiku.go`)**
  rewrites the issue to a terse summary; `compaction_level`, `compacted_at`, `original_size` are stamped.
  So decay = **lossy AI summarization of long-closed issues, with the original recoverable from a snapshot
  table.** It is *not* deletion and *not* automatic — it's a `bd compact` pass over eligible candidates.

### 1.7 `bd prime` / `bd remember` — session priming + persistent memory

- **`bd remember "<insight>" [--key k]`** (`cmd/bd/memory.go`): stores a string memory in the `config` KV
  table under key `kv.memory.<slug>` (slug = first ~8 words of the insight, ≤60 chars, or explicit `--key`).
  Plain key→value; no embeddings, no decay. This is a flat agent scratchpad, not our episodic sidecar.
- **`bd prime`** (`cmd/bd/prime.go`): the session-start context dump. Injects (a) stored memories, (b) a
  ready-work snapshot, optionally (c) a global `~/.config/beads/PRIME.md`, in several modes (`--full`,
  `--mcp`, `--stealth`, `--memories-only`, `--hook-json` for agent hooks). It is a **read assembler** over
  the board + memories — the analog of our P0 auto-context priming, but hand-assembled rather than top-k
  retrieved.

---

## 2. State-model fit vs our spine (the delta)

Our spine: `Todo → Align → In Progress → Verify → Review → Land → Done` (+ Rework: hard reset → re-enter
Align). beads' built-ins: `open, in_progress, blocked, deferred, closed, pinned, hooked`.

| Our state | beads built-in | Fit |
|---|---|---|
| Todo | `open` | clean |
| **Align** | — | **gap** — beads has no "planning/criteria-gating" state |
| In Progress | `in_progress` (+ `started_at`) | clean; `hooked` ≈ "claimed by worker" |
| **Verify** | — | **gap** — no verify/harden state |
| **Review** | — | **gap** |
| **Land** | — | **gap** — beads closes; no squash-merge gate concept |
| Done | `closed` (+ `close_reason`, `closed_at`) | clean |
| Rework | `reopened` event + back to `open` | partial — event exists, no "re-enter Align" semantics |
| (orthogonal) | `blocked` | **derived in our model** (from edges), not a manual status |
| (orthogonal) | `deferred` / `pinned` | map to our `defer_until` / pinned flag |

**Where it maps cleanly:** the two endpoints (Todo=open, Done=closed) and In Progress. `started_at`/
`closed_at`/`close_reason` are exactly the timestamps our state machine wants. `blocked` should be
**derived from the dependency graph** (as beads' own `ready_issues` view does), not a hand-set status —
keep it computed.

**Where it fights:** beads has **no transition enforcement** and **no gate states**. Our four keystone
gates — **Align, Verify, Review, Land** — have no beads equivalent; beads is status-as-free-label.
The custom-status `category` system (`active/wip/done/frozen`) is the seam: it's *enough* to express our
extra states as customs without schema change (e.g. `align:wip`, `verify:wip`, `review:wip`, `landing:wip`
all category-`wip` so they're excluded from `ready` but visible). **But the gating logic — "deny exec tools
until Align confirmed", "Verify needs a mutation-score number", "Land needs proof-of-work" — lives in our
harness, not the board.** beads would only *record* the state; our align-gate enforces the transition.

**The exact delta we implement:** (1) a real **state enum with an allowed-transition table** (beads has
neither); (2) the four gate states as first-class, each with a **gate-condition artifact** the transition
checks (criteria-confirmed / mutation-score / review-approval / land-proof); (3) `blocked` stays **derived**
from edges, never stored as a primary status; (4) Rework = explicit transition that hard-resets to Align
(beads only emits a `reopened` event). Net: we keep beads' *vocabulary at the endpoints* and its
*category-driven ready filter*, and add the **transition graph + gate artifacts** beads deliberately omits.

---

## 3. Dolt's role — branchable data: feature or weight?

**What Dolt is to beads:** an embedded, MySQL-wire, **version-controlled** SQL engine. Every write is a
Dolt commit; it offers **cell-level (not line-level) diff/merge**, native branches independent of git,
multi-writer server mode, and `bd dolt push/pull` federation between clones (peer-to-peer, no central hub).

**What beads concretely uses it for** (from source, not marketing):
- **Schema migrations are committed** (`CALL DOLT_COMMIT('-m','schema: apply migrations')` in `schema.go`)
  and migrations carefully stage/unstage **dirty tables** so a migration never silently merges a user's
  uncommitted work — substantial machinery (`MigrateUp`, `dirtyTableSignature`, `dolt_ignore`).
- **Merge-safety is a first-class, hard-won concern**: the entire `0041`/`0043`/`0050` saga exists to make
  IDs and dependency-edge keys **deterministic across clones** so `bd dolt pull` can merge two boards
  without primary-key conflicts. `wisp_*` tables are `dolt_ignore`'d precisely because they can't be made
  merge-safe. This is real engineering spent on **distributed multi-clone board sync.**
- **History/audit**: every change is queryable as a Dolt commit; `bd backup` is a full Dolt backup (branches
  + history + working set), explicitly **richer than the JSONL export** (`bd export` only dumps the `issues`
  rows — "they do not capture Dolt branches, full commit history, working-set state, or non-issue tables").

**Do *we* want it?** For a **forever-personal, local-first, single-operator** harness: **no — skip it.**
The branch/merge/federation value is for *multiple humans/agents independently mutating clones of the same
board and reconciling.* Our self-building loop is **git-worktree-isolated workers promoting a squashed
result to one mainline** — the *code* lives in git; the **board does not need to be independently branched
and merged.** A worker reads the board, does work in a worktree, and the coordinator writes the result back
to the single board. We get board *history* for free from **SQLite + git-committing a JSONL export**
(`.beads/issues.jsonl`-style append-only log), which is diffable/blameable in the repo we already version.
We lose cell-level auto-merge of concurrent board edits — which we don't need under a single-writer
coordinator. **Verdict: plain `rusqlite` (SQLite) as the live store + a deterministic JSONL export
committed to git for history/portability. Dolt is weight we skip.** (If we ever want multi-machine board
sync, revisit — but YAGNI now.)

---

## 4. Ecosystem coupling — what we inherit by matching the schema

- **mcp_agent_mail**: beads' own messaging is *issues with `type='message'`*, threaded via `replies-to`
  edges and the `dependencies.thread_id` column, with `sender`/`assignee` on the issue row and an
  `ephemeral` lifecycle flag (`docs/messaging.md`). The coupling is **shared IDs**: a bead ID *is* the
  thread/message ID, and `bd mail` delegates routing to an external provider (`gt mail`) while beads is just
  the data plane. **What we inherit by matching:** if our board carries `thread_id` on edges + a `message`
  item type + `replies-to`, the agent-mail *pattern* (thread-per-ticket, replies as graph edges) drops onto
  our schema for free — we do **not** need beads' binary, just the two columns and the convention. This is
  the DR5 coordination layer's data seam.
- **beads_viewer**: a Go TUI doing kanban + dependency-DAG + **graph analytics (PageRank / critical path /
  cycle detection)** + `--robot-triage` JSON. **Critical finding:** `docs/COMMUNITY_TOOLS.md` states
  beads_viewer is **NOT compatible with Dolt-based beads (v0.50+)** — it was built against the *old JSONL*
  format and is now broken (its issue #121). So there is **no live schema coupling to inherit** — the
  viewer is a point-in-time reference, not a plug-in we get free. The *analytics themselves* are standard
  graph algorithms over `(issues, dependencies)`: **PageRank** = eigenvector centrality on the edge graph;
  **critical path** = longest weighted path over `blocks`/`parent-child` DAG; **cycle detection** is already
  in beads in-tree (`internal/storage/issueops/cycles.go`, `bd dep cycles`). **We inherit these as
  algorithms-over-our-own-tables, not as a dependency** — any `(node, directed-edge, edge-type)` schema
  (ours included) supports them. Matching beads' column *names* buys us nothing here; matching the *shape*
  (nodes + typed directed edges + a `blocks`/`parent-child` subset that forms a DAG) is what enables the
  analytics, and our Rust schema below already has that shape.

**Bottom line on coupling:** the only *real* free inheritance is the **agent-mail thread pattern** (two
columns + a convention). The viewer's analytics are generic graph algorithms we'd reimplement regardless;
matching beads' exact schema is **not** required to get them.

---

## Rust-ready board schema (Rust + SQLite, what we'd actually build)

Minimal, single-writer, the swarm/wisp/await overlays dropped. SQLite tables + the Rust enums.

```sql
-- ticket: one row per work item. The four big text fields are the WORKPAD.
CREATE TABLE ticket (
    id              TEXT PRIMARY KEY,         -- 'bd-a3f8' or hierarchical 'bd-a3f8.1.2'
    root_id         TEXT NOT NULL,            -- denormalized: everything before first '.'
    parent_id       TEXT,                     -- NULL for roots; FK ticket(id)
    content_hash    TEXT NOT NULL,            -- sha256(title|description|created|workspace) — collision-free across clones
    title           TEXT NOT NULL,
    description      TEXT NOT NULL DEFAULT '',
    plan            TEXT NOT NULL DEFAULT '',  -- workpad: Plan
    acceptance      TEXT NOT NULL DEFAULT '',  -- workpad: Acceptance criteria (the Verify oracle)
    validation      TEXT NOT NULL DEFAULT '',  -- workpad: Validation evidence / proof-of-work
    notes           TEXT NOT NULL DEFAULT '',  -- workpad: Notes / Confusions
    state           TEXT NOT NULL DEFAULT 'todo',  -- see State enum; transitions enforced in code
    priority        INTEGER NOT NULL DEFAULT 2 CHECK(priority BETWEEN 0 AND 4),
    item_type       TEXT NOT NULL DEFAULT 'task',  -- task|bug|feature|epic|spike|decision|message|...
    assignee        TEXT, owner TEXT, created_by TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    started_at      TEXT,                     -- set on → in_progress
    closed_at       TEXT,                     -- set on → done; drives decay clock
    close_reason    TEXT,                     -- scanned for failure (conditional edges)
    defer_until     TEXT, due_at TEXT,
    pinned          INTEGER NOT NULL DEFAULT 0,
    ephemeral       INTEGER NOT NULL DEFAULT 0,
    external_ref    TEXT, spec_id TEXT,
    thread_id       TEXT,                     -- agent-mail thread root (inherit the pattern)
    compaction_level INTEGER NOT NULL DEFAULT 0,
    original_size   INTEGER,
    metadata        TEXT NOT NULL DEFAULT '{}', -- JSON escape hatch
    FOREIGN KEY(parent_id) REFERENCES ticket(id) ON DELETE CASCADE
);
CREATE INDEX idx_ticket_state ON ticket(state);
CREATE INDEX idx_ticket_root ON ticket(root_id);

-- edge: typed directed dependency graph. target is a tagged union (steal beads' CHECK).
CREATE TABLE edge (
    id              TEXT PRIMARY KEY,         -- DETERMINISTIC: blake3(from_id|target|'') — merge/dedup-safe
    from_id         TEXT NOT NULL,            -- the dependent ticket (FK, ON DELETE CASCADE)
    to_ticket       TEXT,                     -- internal target  \
    to_external     TEXT,                     -- external ref       } exactly one non-null
    kind            TEXT NOT NULL DEFAULT 'blocks',
    thread_id       TEXT,                     -- conversation grouping (replies-to)
    metadata        TEXT NOT NULL DEFAULT '{}',
    created_at      TEXT NOT NULL DEFAULT (datetime('now')), created_by TEXT,
    CHECK ((to_ticket IS NOT NULL) + (to_external IS NOT NULL) = 1),
    UNIQUE (from_id, to_ticket),
    UNIQUE (from_id, to_external),
    FOREIGN KEY(from_id)   REFERENCES ticket(id) ON DELETE CASCADE,
    FOREIGN KEY(to_ticket) REFERENCES ticket(id) ON DELETE CASCADE
);
CREATE INDEX idx_edge_to ON edge(to_ticket);
CREATE INDEX idx_edge_kind ON edge(kind, to_ticket);

-- snapshot: pre-compaction archive (decay is lossy-but-recoverable)
CREATE TABLE ticket_snapshot (
    id INTEGER PRIMARY KEY, ticket_id TEXT NOT NULL, level INTEGER NOT NULL,
    snapshot_json TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY(ticket_id) REFERENCES ticket(id) ON DELETE CASCADE
);

-- memory: flat KV for `remember`/prime (episodic sidecar is separate, top-k/embedded)
CREATE TABLE memory (key TEXT PRIMARY KEY, value TEXT NOT NULL, created_at TEXT);
```

```rust
// Edge kinds — collapse beads' 20 to the few that earn their keep + an Other escape hatch.
enum EdgeKind {
    Blocks,             // hard blocker — gates ready
    ParentChild,        // structural epic→subtask — transitively gates ready
    ConditionalBlocks,  // unblocks only if target closed with a failure reason
    WaitsFor,           // fanout gate (metadata: all|any children)
    RelatesTo,          // knowledge-graph (non-blocking)
    Duplicates, Supersedes, RepliesTo, DiscoveredFrom,  // non-blocking links
    Other(String),      // any ≤50-char custom string
}
impl EdgeKind {
    /// THE load-bearing predicate: only these four gate `ready`.
    fn affects_ready(&self) -> bool {
        matches!(self, Blocks | ParentChild | ConditionalBlocks | WaitsFor)
    }
    fn is_hard_blocker(&self) -> bool { matches!(self, Blocks | ConditionalBlocks | WaitsFor) }
}

// State — a REAL enum with an enforced transition table (beads' missing piece).
enum State { Todo, Align, InProgress, Verify, Review, Land, Done, Rework }
// allowed: Todo→Align→InProgress→Verify→Review→Land→Done;  any→Rework→Align (hard reset).
// `blocked` is NOT a state — it is derived from the edge graph (see ready-query).
```

**The ready-query** (recursive CTE, ported & simplified from beads' `ready_issues` view — drop the
wisp/ephemeral/custom-status machinery; keep transitive-blocked-up-the-parent-tree):

```sql
WITH RECURSIVE
  blocked_seed AS (   -- directly blocked: a ready-affecting edge to a non-terminal blocker
    SELECT DISTINCT e.from_id AS id
    FROM edge e JOIN ticket b ON b.id = e.to_ticket
    WHERE e.kind IN ('blocks','conditional-blocks','waits-for')
      AND b.state != 'done'
  ),
  blocked AS (        -- inherit blockedness down the parent→child tree
    SELECT id FROM blocked_seed
    UNION
    SELECT e.from_id FROM blocked b
    JOIN edge e ON e.to_ticket = b.id AND e.kind = 'parent-child'
  )
SELECT t.* FROM ticket t
WHERE t.state = 'todo'                         -- or any state you treat as "schedulable"
  AND t.ephemeral = 0
  AND t.id NOT IN (SELECT id FROM blocked)
  AND (t.defer_until IS NULL OR t.defer_until <= datetime('now'));
```
*(`conditional-blocks` refinement: a target closed with a failure `close_reason` should NOT block — fold a
`close_reason`-keyword check into the seed if/when we use that edge kind.)*

**The decay rule** (port of `CheckEligibility`, no AI required for v1):

> A ticket is compaction-eligible iff `state='done'` AND `closed_at ≤ now − N days` AND `compaction_level <
> tier`. Tier 1 default N=30, Tier 2 N=90 (configurable). On compact: write full content JSON to
> `ticket_snapshot`, replace the live text fields with a short summary, bump `compaction_level`. Lossy but
> recoverable. v1 can use a deterministic/template summary or local oMLX (Gary's `Qwen3.6` endpoint) in
> place of beads' Claude-Haiku call — **no cloud dependency.**

---

## Decision — **STEAL-the-schema + hand-roll-minimal** (Rust + SQLite)

**Not adopt** (run the Go `bd` binary): it violates the locked Rust-native/own-core direction, drags in
**embedded Dolt** (a whole versioned MySQL engine) for a branch/merge/federation capability a single-operator
local harness does not need, and ships ~30 columns of swarm/wisp/await/messaging overlay that are pure weight
for us. beads is a *multi-agent, multi-clone* tracker; we are single-writer.

**Steal-the-schema, then hand-roll-minimal** is the right verdict because the *valuable* part of beads is
**design, not runtime**, and it's small: (1) the **content-hash + hierarchical-dotted ID** scheme
(collision-free across clones, human-sortable tree); (2) the **typed-directed-edge graph with a deterministic,
merge-safe edge key and a one-of-N tagged-union target** (the `0041`/`0050` lessons are worth inheriting for
free instead of re-learning); (3) the **single load-bearing predicate** `affects_ready ∈ {blocks,
parent-child, conditional-blocks, waits-for}` and the **recursive-CTE ready-query** that walks blockers down
the parent tree; (4) the **time-gated, snapshot-backed decay** rule. Everything else (Dolt, wisps, await,
20-type taxonomy, no-transition status) we drop or replace. The schema above is ~4 tables and two Rust enums
— well within hand-roll scope, and it leaves room for the thing beads *lacks* and we *need*: a **real state
machine with enforced transitions and gate artifacts** (Align/Verify/Review/Land).

This resolves design-v2 open-question #1 (board substrate) toward **own-it-in-Rust**, consistent with DR1's
expected Rust-native verdict.

---

## Sources (paths read)

- `$HOME/Documents/Work/Skills/beads/internal/storage/schema/schema.go` — migration engine, Dolt commit/stage machinery
- `…/internal/storage/schema/migrations/` — all 50 `*.up.sql`; read in full: `0001_create_issues`, `0002_create_dependencies`, `0008_create_child_counters`, `0009_create_issue_snapshots`, `0010_create_compaction_snapshots`, `0013_create_issue_counter`, `0017`/`0025`/`0044` (`ready_issues` view evolution), `0018` (`blocked_issues`), `0027_add_started_at`, `0041_split_dependencies_target`, `0046_add_is_blocked`, `0050_dependencies_deterministic_id`
- `…/internal/types/types.go` — Issue/Dependency structs, Status + category model, IssueType, full DependencyType taxonomy + `AffectsReadyWork`/`IsBlockingEdge`/`IsWellKnown`/`IsFailureClose`
- `…/internal/types/id_generator.go` + `…/internal/idgen/hash.go` — hash ID, base36, progressive collision, hierarchical child IDs, `MaxHierarchyDepth`
- `…/internal/storage/issueops/compaction.go` — `CheckEligibilityInTx`, tier thresholds (30/90 days), candidate queries
- `…/internal/compact/compactor.go` + `haiku.go` — AI-summarization compaction pipeline
- `…/cmd/bd/prime.go`, `…/cmd/bd/memory.go` — `bd prime` (context assembler) and `bd remember` (KV memory under `kv.memory.*`)
- `…/docs/DOLT.md`, `…/docs/messaging.md`, `…/docs/COMMUNITY_TOOLS.md` — Dolt rationale + backup-vs-export, agent-mail-as-issues thread model, beads_viewer **incompatible with v0.50+ Dolt** (#121)
- KB grounding (DAG ready-set scheduling): **attempted, not obtained** — `search(...)` returned a Qdrant
  "storage already accessed by another instance" lock error; recorded as a negative result rather than
  fabricated. The ready-query design rests on the directly-verified beads `ready_issues` CTE, not on the KB.
