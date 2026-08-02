# Eval Scope — Foundation Lock (pre-Phase-0)

> Purpose: resolve the foundation decisions that gate Phase 0, scored against Gary's stated direction.
> Dogfoods the Align gate — we fix the rubric and success/failure criteria *before* running the eval, so
> it can't optimize for the wrong thing. Two bounded teardowns + one synthesis decision. **Time-boxed:
> map boundaries and extract designs, do not survey every line (Wu Wei).**

## Direction (from Gary, locked as the rubric)

- **Forever-personal, local-first, hardware-specific** harness; eventually **self-building**.
- **Rust-native** is the working hypothesis — *to be pressure-tested, not rubber-stamped.*
- Base decision: "pure merits, weigh it." Board (beads): "earn its keep — but eval rigorously + extract schema."

**Consequence — verdict tags flip.** If the core is owned Rust, we don't run a Go binary (beads) or a TS
extension host (Pi) as our engine. Most references move `INTEGRATE → STEAL`. **oh-my-pi reframes from
"base to extend" → "Rust reference architecture."** beads reframes to "schema to extract + reimplement."

## The decision this eval must output

A single **language/runtime strategy**, one of:
- **(A) Fully Rust-native, own-core** — we build the agent loop / gate / board / TUI in Rust; mine pi-mono,
  omp, beads, symphony for *designs*, depend on none as runtime.
- **(B) Hybrid — Rust core + thin TS orchestration** (the omp model) — Rust for perf-critical engine (I/O,
  worktree iso, search), TS for providers + extension/orchestration where the ecosystem and model-fluency help.
- **(C) Stay TS-on-Pi** (the spike default) — extend pi-coding-agent; revisit Rust only if perf demands.

Plus the two sub-decisions (base reuse list; board adopt/steal/hand-roll). The honest costs to weigh:
upfront build (A > B > C), lock-in (C/B couple to others' code; A owns it), self-building friction (Rust
harder for models, but its compiler is a safety asset for self-modification), and provider-surface cost
(small, because local-first).

---

## DR1 — Base & language strategy (omp as Rust reference)

**Clone `can1357/oh-my-pi`. Bounded teardown.** Decision output: A / B / C + a concrete reuse-vs-reimplement list.

Probes:
1. **Map the Rust/TS boundary.** What lives in the ~27k-LoC Rust core (ripgrep/shell/AST/PTY/pi-iso) vs the
   TS layer (providers, ExtensionAPI, orchestration)? Where is the **agent loop, tool execution, and the
   gate hook** — Rust or TS? (This decides how much of A is already done for us.)
2. **Severability.** Does the Rust core stand alone, or is it welded to the TS host? Could we lift pieces
   (esp. **pi-iso** worktree/APFS-clone isolation) as standalone Rust crates?
3. **Provider reality-check.** Confirm we need only ~3 providers (oMLX local + 1–2 cloud tiers). Estimate the
   cost of a minimal Rust provider layer (OpenAI-compatible + Anthropic streaming/tool-calls) vs reusing
   pi-ai (TS). Note any Rust crates that already cover it.
4. **Gate re-validation (sketch, not full build).** Express the align-gate concept (deny exec tools until
   `aligned`) in the chosen runtime's agent loop. For (A): does a Rust loop make this *cleaner* than the TS
   hook? Confirm the spike's primitive ports.
5. **Self-building friction, honest read.** Rust vs TS for the eventual self-modifying endgame — model
   fluency, compile-loop latency, and the compiler-as-guardrail upside. One paragraph, no hand-waving.

- **Success:** a defensible A/B/C recommendation + the reuse list (which omp/pi crates or designs we lift) +
  a ported gate sketch on the chosen runtime.
- **Stop/fallback:** if the Rust core is too entangled to assess in one bounded session, recommend from its
  architecture docs and flag a deeper spike — don't read 27k LoC line-by-line.
- **Effort bound:** one focused teardown. Map the boundary, extract the pattern, decide.

## DR2 — Board: beads eval + schema extraction

**Clone `steveyegge/beads`. Eval rigorously AND extract the schema** (Gary's explicit ask). Decision output:
adopt (unlikely under Rust-native) / **steal-the-schema** (likely) / hand-roll-minimal — plus a Rust-ready
board schema.

Probes:
1. **Extract the full data model:** issue fields; ID scheme (hash + hierarchical `bd-a3f8.1.1`); dependency
   edge types (`blocks`/`relates_to`/`duplicates`/`supersedes`/`replies_to`); status model; **compaction/
   decay** semantics; the **`bd ready`** unblocked-query logic; `bd prime`/`bd remember`.
2. **State-model fit.** Does beads' status model map onto our `Todo→Align→In Progress→Verify→Review→Land→Done
   +Rework`, or fight it? Document the delta we'd need.
3. **Dolt's role.** Is git-branchable, mergeable *data* a feature we want (board history/merge for the
   self-building loop) or weight we skip with plain `rusqlite` + a JSONL export? 
4. **Ecosystem coupling.** How do agent_mail (shared IDs) and beads_viewer (PageRank/critical-path/cycle)
   bind to the schema — do we inherit those ideas *for free* by matching the schema in our Rust impl?

- **Success:** a **Rust-ready board schema** (tables + edge types + ready-query + decay rule) derived from
  beads, with a documented adopt/steal/hand-roll decision and the state-model delta.
- **Stop/fallback:** standard 3-attempt rule. Effort bound: one session; the schema extraction is the
  priority deliverable even if the full eval runs long.

---

## Synthesis → unblocks Phase 0

Output `research/11-foundation-decision.md`: the **language/runtime strategy (A/B/C)** + base reuse list +
board decision + schema. Updates `00-design-v2.md` §2/§10 (the "EXTEND-pi via TS" adoption layer) to reflect
the Rust direction. Only after this is Phase 0's foundation real.

**Method:** hands-on clones (as in spikes 06–08); two tracks run in parallel (independent); each writes a
findings doc ending in a decision. **Guard:** bounded teardowns to make ONE decision each — not exhaustive
surveys.
