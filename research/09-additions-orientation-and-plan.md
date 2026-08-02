# Additions — Orientation + Deep-Research Plan

> 13 references Gary surfaced before Phase 0. README-level orientation pass complete (4 parallel readers).
> **Two of these can be our foundation rather than something we build** — they re-sequence Phase 0: a focused
> evaluation must resolve them *before* we lay trunk code. Verdict tags: INTEGRATE (use as tool/dep), STEAL
> (copy the design, don't depend), REFERENCE (read for ideas), SKIP. ⭐ = gates Phase 0.

## Orientation (one line each + verdict)

| # | Project | What it is | Maps to | Verdict |
|---|---------|-----------|---------|---------|
| 1 | **steveyegge/beads** ⭐ | Dependency-aware **graph issue tracker** as "structured memory for coding agents" — Dolt-backed, `bd ready` surfaces unblocked work, `bd prime`/`remember`/compaction. | BOARD substrate, P0, P2, P3 | **INTEGRATE** (candidate to *be* the board) |
| 2 | **can1357/oh-my-pi** ⭐ | Maintained **fork/superset of pi-mono** — already ships `task` subagents in **git-worktree isolation** (typed returns), **Hindsight** memory (retain/recall/reflect), time-travel **stream rules**, checkpoint/rewind, RPC/ACP. | EXECUTION, P2, P3, P4, P5 | **EVALUATE AS BASE** (could replace vanilla pi-mono) |
| 3 | **abhigyanpatwari/GitNexus** | Codebase → **knowledge graph** (Leiden clusters, process tracing, hybrid search) over MCP; PreToolUse hook injects graph context. Already wired as our MCP. | P0, P2 | **INTEGRATE** (Noncommercial license — fine for us) |
| 4 | **repowise-dev/repowise** | 5-layer repo intelligence: graph + git-hotspots + docs + mined **decisions** + deterministic **code-health** (1–10/file); `get_risk` PR directive; `distill` output compression. | P0, P1 (risk gate), P3 | **STEAL** (AGPL — patterns only) |
| 5 | **Fission-AI/OpenSpec** | Spec-driven workflow: `propose → apply → archive`; per-change folder (`proposal.md`/`specs/`/`design.md`/`tasks.md`); deliberately gate-*less*. | P1 Align, WORKPAD | **STEAL** (add the hard gate they omit) |
| 6 | **gsd-build/get-shit-done** (→ open-gsd/gsd-core) | Context-engineering framework: **Discuss→Plan→Execute→Verify→Ship** loop, parallel fresh-context **waves**, `STATE.md`/`CONTEXT.md` artifacts. Closest end-to-end analog to our whole WORK+EXEC design. | State machine, P0, P1, P4 | **STEAL** |
| 7 | **karpathy/autoresearch** | Overnight LLM-training research loop: agent edits **one bounded file**, fixed **5-min budget**, single metric (`val_bpb`), **keep/discard**, ~100 exps/night; human edits `program.md`. | P4 (experiment ticket), P5 | **STEAL** (the per-branch judging discipline) |
| 8 | **Dicklesworthstone/mcp_agent_mail** | FastMCP coordination layer: agent identities, threaded inbox, **advisory file leases** + pre-commit conflict guard. Pairs with beads via shared IDs. | EXECUTION (coord↔worker), P2 | **STEAL** (the lease/ack/thread protocol) |
| 9 | **Dicklesworthstone/beads_viewer** | Go TUI over beads: kanban + dependency DAG + **graph analytics** (PageRank, critical path, cycle detection) + `--robot-triage` JSON for agents. | P4 (prioritization), BOARD observability | **INTEGRATE if beads** / REFERENCE |
| 10 | **patoles/agent-flow** | Live **node-graph visualizer** of Claude Code/Codex via hook events → SSE; web (Next.js) + VS Code, read-only. | P4, observability | **REFERENCE** (steal the JSONL event contract; web not TUI) |
| 11 | **claude.com skills blog** | Anthropic's concrete lessons on using Skills: description=trigger, one category, gotcha-mining, ship scripts, memory=re-read data, verification skills = highest ROI, instrument triggering. | Skill layer, P1, P3, P5 | **STEAL** (near-direct spec for our skill system) |
| 12 | **badlogic/pi-mono** | The canonical Pi base (validated in spike 07). | Runtime/base | **BASE** (already known) |
| 13 | **Rose22/openlumara** | Local-first toggleable-module personal agent fw; memory is flat MessagePack save/recall — **no embeddings/top-k**. | P3 (weakly) | **SKIP** (below our P3 bar; reference module ergonomics at most) |

## The two findings that re-sequence Phase 0

**(A) oh-my-pi may be the base, not pi-mono.** omp already ships ~60% of our Execution + Memory planes:
worktree-isolated `task` subagents returning typed objects, Hindsight memory, stream rules (self-evolution
enforcement), pi-iso workspace isolation, RPC/ACP. If we build on omp, Phase 0 collapses to mostly the WORK
plane (board + state machine + land + Align gate). But it's a solo-maintained fork — lock-in, divergence from
upstream, and license/maintenance risk are real. **This is the single most important decision and it gates
everything**: our `align-gate.ts` and the whole trunk sit on whichever base we pick.

**(B) beads may be the board, not hand-rolled markdown.** beads is purpose-built for exactly our BOARD +
P0 + P3 needs (dependency graph, `bd ready`, `prime`/`remember`, compaction-decay), and it's the shared
substrate that mcp_agent_mail and beads_viewer already plug into. Adopting it could collapse the board +
coordination + memory-priming sub-problems into one ecosystem. Risk: Dolt dependency weight, and whether its
state model bends to our 7-state machine + LAND gate.

Net: **don't lay trunk code until A and B are decided.** Building the wrong base/board is the expensive mistake.

---

## Deep-Research / Evaluation Plan

Method = clone-and-teardown (as in spikes 06–08), each track writes a findings doc and ends in a **decision**.
Tier 0 is **blocking** (resolve before Phase 0). Tiers 1–2 inform the build and can run in parallel / fold in.

### Tier 0 — Foundation decisions (BLOCK Phase 0)

**DR1 — Base platform: pi-mono vs oh-my-pi vs cherry-pick.** Clone omp; take it apart.
- Q: Is omp's `ExtensionAPI` the same surface (does our `align-gate.ts` port unchanged)? What exactly do
  `task`/pi-iso, Hindsight, stream rules give us vs the cost of building them on vanilla pi-mono?
- Q: Divergence/lock-in risk — how far has omp forked from upstream, how active, license, can we track upstream?
- Q: Does omp's built-in memory (Hindsight) satisfy or conflict with our P3 design (wiki + decaying sidecar)?
- **Decision:** build on omp (inherit ~60%, accept fork risk) vs pi-mono (canonical, build more) vs
  cherry-pick omp patterns onto pi-mono. Re-run the spike's Align-gate test on the chosen base.

**DR2 — Board substrate: beads vs hand-rolled.** Clone beads (+ skim beads_viewer).
- Q: Does beads' state/status model bend to our `Todo→Align→…→Land→Done +Rework`, or fight it?
- Q: Dolt dependency weight acceptable for a personal harness? Embedded mode enough (no server)?
- Q: Do `bd ready`/`prime`/`remember`/compaction cover P0-priming + P3-memory, or overlap awkwardly with our
  sidecar? Is `.beads/issues.jsonl` the clean integration seam?
- Q: Does the beads + agent_mail + beads_viewer stack cohere (shared IDs)?
- **Decision:** adopt beads as the board (INTEGRATE) vs steal its schema onto markdown vs hand-roll. Resolves
  v2 open-question #1.

### Tier 1 — Pattern sources (inform the build)

**DR3 — Align + state-machine discipline: OpenSpec + GSD.** Extract OpenSpec's `proposal/specs/design/tasks`
artifact set and propose→apply→archive shape → our **workpad + Align transition**. Read GSD's
Discuss→Plan→Execute→Verify→Ship loop, `STATE.md`/`CONTEXT.md`, and fresh-context **waves** → our state
machine + P4. Output: the concrete workpad template + transition contract to copy. (Pin versions — both mid-migration.)

**DR4 — P0 codebase intelligence: GitNexus (integrate) + repowise (steal).** Confirm GitNexus' hook-based
auto-injection (PreToolUse graph context, PostToolUse staleness) as our P0 priming mechanism. Steal from
repowise: **code-health biomarkers**, `get_risk` directive (a land-gate signal for P1), `distill` output
compression, the `_meta` stale-warning envelope. **Decision:** one graph backend (GitNexus), repowise patterns
reimplemented.

**DR5 — Coordination protocol: mcp_agent_mail.** Lift the **lease/ack/thread-per-ticket** protocol + the
pre-commit conflict guard for coordinator↔worker. Decide: do we need it if workers are worktree-isolated (omp/
DR1), or is it the softer layer for same-tree parallel edits? Synergy with DR2 (shared beads IDs).

**DR6 — Experiment-ticket loop: autoresearch.** Steal the **fixed-budget → single comparable metric →
keep/discard** discipline and the "agent edits one bounded surface" constraint (≈ our workpad) for P4 research
tickets. We supply parallel breadth; it supplies per-branch judging rigor.

### Tier 2 — UX & skill design (lighter, late)

**DR7 — Observability: agent-flow.** Adopt its **hooks → JSONL event-stream → tree-graph** contract: emit the
same JSONL from our harness so agent-flow is a free dev-time inspector now, and we have a render schema for
native Pi TUI widgets later. REFERENCE only (web, not TUI).

**DR8 — Skill-layer design: Claude skills blog.** Apply directly: description-as-trigger, one-category skills,
gotcha-mining (feeds P5), ship-scripts-not-prose, memory-as-reread-data (P3), over-invest in verification
skills (P1), and **PreToolUse trigger instrumentation** (P5 self-evolution measures which skills fire). Audit
our existing `references/*` judgment skills against these.

### Sequencing

```
DR1 (base) ─┐
DR2 (board)─┴─▶ FOUNDATION LOCKED ─▶ Phase 0 (thin trunk on the right base/board)
                                        │  steals from, in parallel:
                                        ├─ DR3 (align/state)  ├─ DR4 (P0 intel)
                                        ├─ DR5 (coordination) ├─ DR6 (experiment loop)
                                        └─ DR7/DR8 (obs + skills) fold in late
```

**Recommendation:** run DR1 + DR2 next (clone omp + beads, take them apart, decide). Everything else is
steal-as-you-build and need not block. SKIP openlumara.
