# Foundation Decision — synthesis of DR1 + DR2

> **Status: FINAL — pending Gary's sign-off.** Synthesizes DR1 (`research/11`), DR2 (`research/12`), and
> DR2-prime (`research/15`, the `beads_rust`/`br` teardown). All three converge on one posture: **own a Rust
> core on `rusqlite`; mine the references for designs, depend on none.** This locks the language/runtime
> strategy, the base reuse list, and the board — the gate on Phase 0.

## The decision (one line)

**Strategy A — full Rust-native own-core.** Build the agent loop / gate / board / providers / TUI in Rust;
**lift `crates/pi-iso`** from oh-my-pi as the one severable Rust asset; **steal the beads schema** and
hand-roll the board in Rust + SQLite. Mine pi-mono / omp / symphony / beads for designs; runtime-depend on none.

## Why — the two teardowns reinforce each other

**DR1 (omp): strategy B collapses, A wins.** omp's ~33k-LoC Rust core is **leaf systems-primitives only**
(grep/shell/AST/PTY/iso via napi-rs `cdylib`). The agent loop, tool execution, provider layer, and the
`beforeToolCall` gate are **100% TypeScript, byte-identical to pi-mono** (`agent-loop.ts:1278`). So:
- B ("Rust core + TS brain") *is* what omp already is — and it hands us **none** of A's hard parts (they're
  all TS). The earlier "omp ships ~60% of our planes for free" optimism dies here: those features are TS;
  under a Rust core we reimplement them anyway.
- The reimplementation is **bounded**: providers = ~2 wire formats (~1–1.5k LoC Rust) vs importing 75k LoC
  of TS pi-ai; the **gate ports *cleaner* in Rust** (typed `enum Phase` + a board row, compiler-checked, vs
  a TS hook + closure + JSONL replay).
- `crates/pi-iso` (worktree/sandbox isolation) **severs cleanly** (napi-free, deps = `async-trait`/`similar`/
  `tokio`) → lift it directly as our **P2** isolation pillar, including its `git worktree add --detach` rcopy lifecycle.
- C (stay TS-on-Pi) remains a valid fast fallback but couples us to two upstreams forever and forfeits the
  compiler-as-guardrail the self-building endgame wants.

**DR2 (beads): steal the schema, skip the engine.** beads is Go + embedded **Dolt** (versioned MySQL).
Read at the source (50 SQL migrations, idgen, compaction):
- Its Dolt branch/merge/federation exists for **multi-clone, multi-agent board reconciliation** — which a
  **single-writer local harness does not need**. Skip it: plain SQLite + a git-committed JSONL export gives
  history for free.
- The valuable part is **small design, not runtime**: the edge model (deterministic edge `id` from
  `(issue, target)`, a CHECK-enforced tagged-union target — both hard-won *merge-safety* lessons), the
  `ready` recursive-CTE, and tiered decay/compaction.
- beads has **no transition enforcement and no gate states** — our Align/Verify/Review/Land have no beads
  equivalent. That's exactly our net-new WORK-plane value (state enum + allowed-transition table + per-gate artifacts).
- Free inheritance: the **agent-mail thread pattern** (`thread_id` + `message` + `replies-to`). `beads_viewer`
  is already broken against Dolt-era beads (#121); its analytics are generic graph algos over `(nodes, typed
  edges)` — inherited by *shape*, not by matching columns.

## The locked foundation (concrete)

| Layer | Decision | Source |
|---|---|---|
| **Language/runtime** | Full Rust-native own-core; own agent loop + tool dispatch | DR1 → A |
| **Align gate** | Typed `enum Phase` + a board-row flag, checked in our dispatch loop (`Gate::check`) | DR1 (ports cleaner than TS) |
| **Isolation (P2)** | **Lift `crates/pi-iso`** as-is | DR1 (severable) |
| **Providers** | Minimal Rust layer, 2 wire formats: `openai-completions` (oMLX local + cloud) + `anthropic-messages` SSE | DR1 (~1.5k LoC) |
| **Board** | Hand-roll on **`rusqlite`** (NOT fsqlite/Dolt); `ticket` (workpad fields inline + `state` enum) · `edge` (simple PK `(issue, depends_on, kind)`, no tagged-union — br's simpler model) · `ticket_snapshot` · `memory` · **`gate_results`** (ported from br). JSONL export for git history | DR2 + DR2-prime |
| **State machine + gates** | **Port br's `close_policy.rs` (~250 lines): allowed-transition map + per-transition gate engine + `gate_results(issue,gate,provider,passed)` ledger.** Adapt to Align/Verify/Review/Land | DR2-prime (already built+tested) |
| **Ready-query** | recursive CTE: `state='todo'`, not in transitively-blocked set (hard-blocker edges inherited down parent-child), `defer_until` passed | DR2 (verified beads CTE) |
| **Decay** | `done` ∧ `closed_at ≤ now−N` ∧ `level<tier` → snapshot original, summarize via **local oMLX**, bump level | DR2 + research/01 |
| **Edge kinds** | collapse beads' 20 → ready-affecting `Blocks·ParentChild·ConditionalBlocks·WaitsFor` + knowledge `RelatesTo·Duplicates·Supersedes·RepliesTo·DiscoveredFrom` + `Other(String)` | DR2 |
| **TUI** | `ratatui` (when we get there) | — |

Memory plane (research/01) is unchanged and reinforced: beads' `prime` (session-start context assembler) and
`remember` (flat KV) patterns inform P0/P3; the sidecar+wiki split stands.

## DR2-prime — beads_rust (`br`): REFERENCE, and lift its gate engine

`br` (Jeffrey Emanuel's Rust port of beads) froze the **SQLite + JSONL, no-Dolt** architecture — empirically
confirming DR2's call. But its storage depends on **`fsqlite`** (a single-maintainer *alpha* pure-Rust SQLite)
+ pinned nightly + ~180k LoC of full issue-tracker we'd use ~10% of → **adopt/vendor/fork is a Wu-Wei loss**.
Verdict: **REFERENCE — hand-roll on `rusqlite`**, with high-value design lifts:
- **Correction to DR2:** br *does* have a state machine. `src/close_policy.rs` ships a config-driven
  allowed-transition map + a **per-transition gate engine** (`GateRule`/`GateSpec`/`evaluate_gates`) backed by a
  `gate_results(issue_id, gate, provider, passed)` table. That is almost exactly our Align/Verify/Review/Land
  "transition-table + per-gate-artifact" design — **already built and unit-tested.** We **port `close_policy.rs`
  (~250 lines)** and adapt it, instead of inventing it. (Its `gate_results.provider` column even matches our
  "test author vs mutator on different providers" idea.)
- **Simpler edge table:** drop Go-beads' deterministic-id / tagged-union target (that was Dolt merge-safety scar
  tissue); br's plain `(issue, depends_on, kind)` PK is enough for a single-writer SQLite board.
- **Bonus:** because we use *real* SQLite (`rusqlite`), our `ready` query can be the clean **recursive CTE** DR2
  designed — br only fell back to a Rust-side BFS + `blocked_issues_cache` because *fsqlite can't run recursive
  CTEs*. We don't inherit that constraint.
- **Later lift:** br's `agent_context` (governing-JSON inherited by descendant tickets) — useful P5/context prior art.

License: MIT + a rider blocking OpenAI/Anthropic & affiliates — fine for a personal fork/reference; just don't
feed `br` source into an Anthropic/OpenAI training/eval pipeline.

## Open risk + the first Phase-0 step (DR1's hedge)

Confidence is **high on the architecture** (all code-verified) but **medium on Rust build-cost sizing**.
So Phase 0 opens with a **bounded sizing spike**, not the full trunk: own agent loop + 2 provider clients +
the Rust gate (`enum Phase`) + lifted `pi-iso`, driving **oMLX** end-to-end — re-validating the spike-07
gate result on the Rust runtime. If the sizing holds, proceed to the trunk (board + state machine + workpad +
land) on this foundation. If it balloons, C (TS-on-Pi) is the documented fallback.

## Grounding

`KB: searched "recursive CTE / transitive closure" (grep_books) → nothing relevant; semantic search
unavailable (Qdrant single-client lock). Board schema grounded instead in beads' verified production schema
(50 migrations, incl. merge-safety lessons 0041/0043/0050) — stronger prior art than textbook theory.`

## On sign-off — updates to make

- `wiki/decisions.md`: resolve the OPEN foundation decision (→ A + steal-beads-schema); promote
  "beads/repowise as runtime deps" from *leaning rejected* to **rejected** (confirmed by DR2).
- `00-design-v2.md` §2 (adoption layer) + §10 (Pi mapping): rewrite from "EXTEND pi-coding-agent via TS" to
  "Rust-native own-core; lift pi-iso; steal-schema board." The planes/pillars/spine/memory/gates are unchanged.
- `wiki/active-work.md`: Foundation Eval → done; next = Phase 0 sizing spike.
