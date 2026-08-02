# research/32 — Board close-path: terminal reachability for non-landing tickets (Fix B design)

**Status:** design, pre-adversarial-review. 2026-06-11.
**Scope:** the board FSM (`crates/board/src/spine.rs`, `board.rs`, `model.rs`) + a `close` verb in `crates/agent/src/main.rs`.

## Problem

`Done` is reachable **only** via `Land → Done`, gated by `GATE_LANDED` (human, requires a committed
tree — the §10 "git commit = human approval" keystone). Two ticket situations therefore have **no path
to a terminal state**:

- **P1 — non-code completion (latent).** The spine first-classes non-code kinds: `kind_is_code` enumerates
  `business-grounding`, `research`, `docs`, `spike` as *non-code* (spine.rs:144), and `required_gates_for`
  deliberately lets them leave `Verify` on `tests_green` alone (no mutation gate). But a non-code ticket
  produces **nothing to land** — so after `Review` it is stranded. No live ticket today, but a designed-in
  dead end the first research/docs/spike ticket will hit.
- **P2 — abandonment (live).** A ticket that will **not** be completed-and-landed has no disposal path.
  Live instances on `harness-board.db`:
  - `t3` build/review — throwaway oMLX exercise ("oMLX runs are throwaway, never landed"); has `tests_green`
    but will never get `mutation_score`/`landed`. Stuck in `review`.
  - `t5` refactor/todo — the S1 recall-floor fix, **already shipped in code outside the board**; stale/superseded,
    sitting in `todo`.

## Invariant to preserve (the keystone)

An **agent** provider (read/write/bash only) must never self-promote work to a terminal. Today that holds
because `GATE_LANDED` and `GATE_CRITERIA_CONFIRMED` are **human-only** (`gate_source_required`); the agent
cannot forge them. Any new terminal path must stay human-gated.

A second, weaker invariant the suite encodes (`lib.rs` criterion #2): `validate_transition(Todo, Done)` is
`Err` — **you cannot structurally skip straight to Done.** A good design keeps the "no-skip" protection
*structural*, not merely gate-deferred.

## Recommended package (to be stress-tested)

**Single terminal `Done`. One new edge `Review → Done`. One new human gate `GATE_RESOLVED`. A `close` verb.**

1. **`spine.rs`**
   - `pub const GATE_RESOLVED: &str = "resolved";` — human source in `gate_source_required`.
   - `forward_targets(Review)` gains `Done`: `&[Land, InProgress, Done]`.
   - `required_gates_for` adds `(Review, Done) => vec![GATE_RESOLVED]`. `(Land, Done) => [GATE_LANDED]` unchanged.
   - **No `kind` threading.** The edge is legal for all kinds; the gate is kind-blind. Zero signature churn.
2. **`board.rs`** — no change to `set_status` (already generic): the operator path is
   `report_gate(GATE_RESOLVED, "gary", Human, true, Some(note))` then `set_status(Review→Done)`, symmetric with Land.
3. **`main.rs`** — a thin `close(id, note)` verb: requires a non-empty resolution note (the artifact),
   reports `GATE_RESOLVED`, transitions `Review → Done`. (Mirrors `align`/`land`.)

### Why this shape

- **Covers both problems from one edge.** Non-code tickets reach `Review` (they pass a possibly-trivial
  `tests_green`), then `close` to `Done` (P1). A throwaway code ticket in `Review` (`t3`) closes the same
  way with `note = "abandoned: oMLX throwaway, never landed"` (P2).
- **No-skip invariant HELD.** Only `Review → Done` is added. `Todo → Done`, `InProgress → Done`, etc. stay
  **structurally illegal**. The protection remains in the transition map, not deferred to a gate.
- **Keystone HELD.** `GATE_RESOLVED` is human-only → the agent loop cannot self-close (just as it cannot
  self-land). Both terminal approaches reachable by the agent (`Review→Land`, `Review→Done`) need a human gate.
- **Code-success path unchanged.** A code ticket that *should* land still goes `Review → Land → Done`
  (`GATE_LANDED`). `Review → Done` is the explicit "won't land" disposition; the **absence** of a `landed`
  gate row makes "resolved-closed" auditable vs "landed-done".
- **No schema migration.** `GATE_RESOLVED` is just a new gate-name string; the `gate_results` table has no
  CHECK on gate name. Contrast a `Cancelled` terminal (rejected below): the `ticket.status` CHECK constraint
  (schema.rs:22) would force a table-rebuild migration **and** a `ready()` blocker-semantics fix
  (board.rs:323 `b.status != 'done'` would strand dependents of a cancelled blocker).

## Forks for the adversarial reviewer to adjudicate

1. **Single `Done` vs a distinct `Cancelled` terminal.** Recommend single (migration + `ready()` cost not
   earned for a personal harness; disposition lives in the note + auditable `landed`-gate absence). Risk:
   conflating "succeeded" and "abandoned" under one status. Defensible to defer `Cancelled` until a real
   reporting need splits them?
2. **`Review → Done` for *all kinds* vs *non-code only*.** Recommend all-kinds (covers `t3` code abandonment;
   does **not** widen the structural map beyond `Review`). Does letting a **code** ticket reach `Done` without
   landing erode the "code must land" keystone? Argument it does not: human-gated, success-path unchanged,
   auditable. **Find the hole.**
3. **Is `Review` the right/only close point?** `t5` is in `Todo` (stale, already-shipped-elsewhere) — not
   reachable by a `Review`-only close without walking it through the whole spine. Recommend handling `t5` as
   a **one-off** disposition in cleanup (not a new FSM edge — don't design the FSM around one stale ticket).
   Is that acceptable, or does abandonment-from-any-state warrant first-class support (which *would* reopen
   the `Cancelled`-wildcard / no-skip-erosion tradeoff)?

## Rejected (pre-review)

- **`Cancelled` terminal now** — table-rebuild migration (status CHECK) + `ready()` blocker fix; the
  Done/Cancelled distinction isn't worth that carrying cost yet. Revisit if reporting needs the split.
- **Wildcard `any-non-terminal → Done` gated `GATE_RESOLVED`** — smallest code, but moves the "no-skip-to-Done"
  guarantee from *structure* to *gate* (breaks the `lib.rs` `Todo→Done is Err` invariant in spirit). Rejected:
  keep the protection structural.
- **`close` refuses code kinds** — would leave `t3` (code abandonment) with no FSM disposal. Rejected:
  abandonment is kind-agnostic; the success-vs-abandon distinction is the note + `landed`-row, not the kind.
