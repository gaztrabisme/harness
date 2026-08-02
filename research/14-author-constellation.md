# Jeffrey Emanuel (Dicklesworthstone) — Resource Constellation

> Surfaced by Gary pre-sign-off via `beads_rust`. The author of `beads_viewer` + `mcp_agent_mail` has built a
> **parallel Rust-native "Agent Flywheel" ecosystem** that maps onto nearly every harness pillar. This is a
> map, not a work order. **Discipline (Wu Wei): only `beads_rust` is foundation-gating now** (it refines DR2);
> everything else is "investigate when that pillar comes up." Cataloged so we don't re-discover it.

## Foundation-gating (now)

- **`beads_rust` (`br`)** ★939, Rust, MIT+rider — Rust port of beads frozen at the **SQLite + JSONL (no Dolt)**
  architecture = exactly DR2's call. Agent-first (`--json`/`--robot`, MCP `br serve`, `capabilities`/`schema`/
  `coordination status`), dependency-aware `ready`/`blocked`/`dep cycles`, non-invasive. **No gate states /
  state machine** → our Align/Verify/Review/Land spine remains net-new. → **board candidate; teardown =
  `research/15`.** Likely shifts DR2 from "hand-roll" → "fork/vendor `br` + add our spine."

## P3 — Memory & retrieval (investigate when building the Memory plane)

- **`frankensearch`** ★57, Rust — two-tier hybrid search: Tantivy BM25 + MiniLM-L6-v2 vector + RRF, progressive
  iterator, f16 SIMD. → candidate **retrieval engine** for the sidecar (vs hand-rolling). High fit.
- **`fast_vector_similarity`** ★430, Rust+Py — Spearman/Kendall/distance-corr/Jensen-Shannon/Hoeffding's D +
  bootstrapped CIs. → retrieval scoring / reranking primitives.
- **`cass_memory_system`** ★376, TS — "procedural memory for coding agents": scattered session history →
  persistent cross-agent memory. → direct P3 prior art (TS, so reference the design).
- **`coding_agent_session_search`** ★869, Rust — index/search local session history across 11+ agent providers.
  → episodic-capture source for the sidecar.
- **`cross_agent_session_resumer`** ★83, Rust — cross-provider session resume via a canonical IR. → session/memory adjacent.

## Execution / coordination / safety (P2/P4)

- **`destructive_command_guard` (dcg)** ★1103, Rust — blocks dangerous git/shell commands from agents. →
  **Execution-plane safety**, complements the worktree isolation + the gate. STEAL/INTEGRATE. High value.
- **`claude_code_agent_farm`** ★841, Shell — 20+ parallel Claude Code agents, lock-based coordination, tmux
  monitoring. → P4 / coordinator REFERENCE.
- **`frankenterm`** ★83, Rust — terminal hypervisor for agent swarms: pane capture, state-machine pattern
  detection, JSON API. → coordinator/observability REFERENCE.
- **`mcp_agent_mail`** (in `research/09`) — lease/ack/thread coordination; pairs with `br` via shared IDs.

## Rust runtime / infra primitives

- **`asupersync`** ★189, Rust — structured-correctness async runtime: region-owned tasks, cancel-correct
  protocols, capability-gated effects, deterministic replay testing. → advanced reference for the Execution
  runtime + deterministic worker replay. (Heavy; reference, don't adopt early.)
- **`fastmcp_rust`** ★25, Rust — build MCP servers/clients in Rust (cancel-correct async, zero-copy). → if/when
  the harness speaks MCP natively.
- **`frankensqlite`** ★182, Rust — ground-up SQLite reimpl with concurrent writers. → **likely NOT needed**
  (we're single-writer; `rusqlite` suffices). Note and skip unless concurrency demands it.
- **`frankentui`** ★246, Rust — minimal TUI kernel, diff-based rendering, RAII cleanup. → TUI option.
- **`charmed_rust`** ★23, Rust — Charm/Bubble Tea/Lip Gloss port (Elm-arch TUIs). → TUI option (vs `ratatui`/frankentui).

## Whole-system reference

- **`agentic_coding_flywheel_setup`** ★1505, Shell — bootstraps a full multi-agent dev env in 30 min. → study
  the *architecture* of his Agent Flywheel; it's the closest existing parallel to what we're building.
- **`flywheel_connectors`** ★80, Rust — mesh protocol + connectors (Linear/Stripe/Discord/Gmail/GitHub). → external integrations, situational.

## Note
License across his repos is typically "MIT + OpenAI/Anthropic Rider" (verify the rider per repo before
vendoring). Author does **not** accept outside PRs (reviews via Claude/Codex) — so for anything we use, plan
to **fork/vendor and own it**, not contribute upstream.
