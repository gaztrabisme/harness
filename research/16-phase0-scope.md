# Phase 0 — Rust Sizing Spike (scope)

> The first build step of Strategy A. Its **only** job: answer one question with evidence — *is the
> Rust build-cost of the own-core acceptable, or do we fall back to C (TS-on-Pi)?* Architecture confidence
> is high (all code-verified in DR1/DR2/DR2-prime); **Rust sizing is the one medium-confidence risk** and
> this spike retires it. Go/no-go gate, deliberately small. Aligned with Gary 2026-06-09.

## Locked decisions (this scoping)

- **Home:** new sibling Cargo workspace `../harness-rs/` (NOT tracked in `Skills/dev/`), mirroring the
  `harness-spike/` pattern. Stays a *spike*; **graduates** into its own product repo (+ the wiki migrates)
  only if the sizing criteria go green. Defers repo-structure commitment until Rust is proven.
- **Providers:** **both** wire formats — `openai-completions` (oMLX local) + `anthropic-messages` SSE. The
  second format is the point: one provider can't prove the provider *abstraction*. Stress the normalization.
- **Kill-box:** **~1 week** of focused work. No green end-to-end run by then ⇒ that is the signal Rust sizing
  is too heavy ⇒ fall back to **C** (extend pi-coding-agent in TS, the spike-07 base). Also kill early on a
  hard blocker (below).

## The slice (minimal end-to-end exercising all four hard parts)

1. **Agent loop + tool dispatch** driving **oMLX** end-to-end, multi-turn with tool calls.
2. **Three tools:** `read_file`, `write_file`, `bash` — enough to exercise the gate (one read-only, two mutating).
3. **The Align gate:** `enum Phase { Align, Execute }`; the loop checks `Phase` before dispatching any
   *mutating* tool (`write_file`/`bash`); a `/align` command flips `Align → Execute`; the phase is
   **persisted to a `rusqlite` row** so it survives a process restart. (Doubles as a rusqlite smoke-test —
   the board substrate later.)
4. **Lift `crates/pi-iso`** from oh-my-pi: spawn the loop with `cwd` inside a pi-iso-created git worktree;
   **prove** DR1's "cleanly severable" claim by compiling it standalone (strip napi), not assuming it.

### Proposed crate layout (`../harness-rs/`)
```
Cargo.toml                 # workspace
crates/
  pi-iso/                  # lifted from oh-my-pi, napi stripped (deps: async-trait, similar, tokio)
  provider/                # lib: trait Provider + openai_completions + anthropic_messages (normalization)
  agent/                   # bin: loop, gate (enum Phase), 3 tools, rusqlite phase store
```
Dep budget (liabilities, each justified): `tokio` (async runtime), `reqwest` (HTTP + streaming SSE),
`serde`/`serde_json` (wire), `rusqlite` (phase store / future board), plus pi-iso's existing three. No
agent-framework, no provider-SDK wrapper — that's the whole point.

**Provider trait under test** (the normalization the second wire format stresses):
```rust
trait Provider { async fn complete(&self, req: Request) -> Result<EventStream>; }
```
`Request`/`Event` normalized across openai-completions (oMLX) and anthropic-messages (SSE). If this trait
needs a redesign to fit the second format, that's a sizing finding.

## Success criteria (binary — gate by number/artifact, not vibe)

- [ ] Loop completes a multi-turn **oMLX** tool-calling conversation.
- [ ] Gate **denies** `write_file`/`bash` in `Align`, **allows** them after `/align` — reproduces spike-07 on Rust.
- [ ] Phase **persists across a process restart** (rusqlite row reloaded).
- [ ] **Anthropic-messages** provider drives the same loop through the *same trait* (abstraction holds, no fork).
- [ ] `pi-iso` lifted + **compiles standalone**, driving a real worktree the loop runs inside.
- [ ] First-party LoC within the DR1 estimate (~1.5–2k) with **no surprise blocker**.

## Kill / fallback triggers (→ C, TS-on-Pi)

- `pi-iso` won't sever without dragging the napi host back in.
- Provider normalization can't cover both wire formats without a heavy redesign or per-provider forks.
- The loop forces in a heavy async/agent framework we can't keep minimal (dependencies-are-liabilities breach).
- ~1 week elapses with no green end-to-end run.

## Out of scope (this is the TRUNK — must not creep in)

The 7-state spine (only `Align→Execute` here), the board schema beyond the one phase row, the state-machine
gate engine (`close_policy.rs` port), ticket `kind`/gate-profiles, the memory plane, workpad, land procedure,
TUI, parallel exploration, self-evolution. Any of these starting to pull in = scope leak → stop, note, defer.

## KB grounding gate

Phase 0 is systems/glue (Rust agent loop, provider HTTP, rusqlite usage, worktree spawn) — **not a
KB-covered domain** (ML / DB-theory / security / distributed / crypto / RAG). Gate honestly skipped.

## On green → graduation

All six criteria green within the box ⇒ `harness-rs` graduates to its own git repo, the wiki travels with it,
and we proceed to the **trunk** (board + state-machine/gate-engine + workpad + land) per design-v2 §12. On
red ⇒ document the blocker and fall back to C; the spike-07 TS base is the documented landing spot.
