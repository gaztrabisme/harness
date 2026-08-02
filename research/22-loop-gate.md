# research/22 — Loop gate (Tier-1 deterministic cycle stop)

> Scope + literature grounding + adversarial review (necessity-first) + design, with the review folded
> back in (§9). Sibling to research/21 (verbose-local-model sprint). Follow-on from the exercise in §21
> that surfaced a **third failure mode** and from the 2026-06-10 discussion ("are we tunnel-visioned on
> the local provider?"). Verdict up front: **build it, keyed strictly off universal signals.**

## 1. The trigger

The research/21 throwaway exercise (Thompson-NFA on oMLX) ended with the model in **reasoning-loop
collapse** — ~6 paragraphs repeated 20–22× verbatim until the token cap fired. Separately, the canonical
"agent stuck in a loop" is a model calling the *same tool with the same arguments* turn after turn,
making no progress, until `max_iters` crudely kills the run with a misleading `max_iters` label.

`max_iters` (default 8; the exercise used 40) already *bounds* a runaway, but bluntly:
- it wastes up to N turns before stopping (N=40 in the exercise),
- it gives the model **no chance to recover**, and
- it labels the result `max_iters` — indistinguishable from "made steady progress but ran out of room."

## 2. Literature grounding (the discussion's "latest techniques" ask)

The consensus across recent practitioner writing and the KB:

- **Root cause is decoding-level, not a model defect.** Greedy / low-temp / narrow top-k decoding
  maximizes local likelihood with no global anti-repetition objective; once a phrase is locally probable
  it self-reinforces (neural text degeneration — Holtzman et al., *The Curious Case of Neural Text
  Degeneration*, arXiv:1904.09751). KB: *Mastering PyTorch* (greedy→repetition; top-k/top-p as remedy).
  This is the root of the §21 intra-turn collapse.
- **Detection is cheap and deterministic.** Tool-call hashing (same tool+args 2–3× = loop); output
  n-gram / char-run repetition; progress signals (no new tool result / no state change across N turns).
  White-box activation monitoring (RecurrentDetector, 2025, ~95% acc) needs hidden states we don't get
  from an API — out of scope.
- **Prevention is a harness discipline.** Hard limits (max steps/turns/tokens) as the backstop, then
  *external* stop rules + a course-correction nudge: tell the model it's repeating, give it one chance,
  then stop. KB: Production RAG Guide "Agent Loop Safety" (max-step / turn-count / token-budget). The
  recurring line: **"loop prevention is fundamentally an engineering discipline, not a model-quality
  issue."**

> KB: searched "neural text degeneration repetition loops sampling decoding" → applied greedy→repetition,
> top-k/top-p remedy (*Mastering PyTorch*). KB: searched "agent loop safety max-step token budget" →
> applied hard-limit + external-stop pattern (Production RAG Guide, "Agent Loop Safety").

The Tier-1 gate below is exactly the endorsed shape: deterministic post-turn detection → telemetry record
→ next-turn nudge → 2-strikes stop.

## 3. The meta question, settled (why this is NOT tunnel vision)

The §21 think-tag investigation *was* a degree of provider-specific tunnel vision (chasing
`reasoning_content` / `<think>` semantics — provider internals). The loop gate is the opposite: agent
loops and autoregressive degeneration are **universal**. The discipline test we hold the design to:

> **Is the fix keyed off a universal signal, or a provider quirk?**

Universal signals — `stop_reason == Length`, output repetition, **identical consecutive tool-call
signatures**, no-progress — belong in the agent loop. Provider quirks — think tags, `reasoning_content`,
SSE shapes — stay quarantined in the provider layer. The gate reads only `resp.tool_calls` (name + args)
and `resp.stop_reason`, both already in the format-agnostic `provider::Response`. It would behave
identically against the anthropic provider. "Design for the 35B floor → works for everything" holds here
because the *mechanism* is model-agnostic; the 35B merely surfaced the failure earlier.

## 4. Signals & axes

Two independent repetition axes:

| Axis | Where it lives | Loop continues? | What stops it today |
|------|----------------|-----------------|---------------------|
| **A. Cross-turn tool-call cycle** | identical `(name,args)` across turns | **yes** (`stop_reason==ToolCalls` each turn) → runs to `max_iters` | only `max_iters` |
| **B. Intra-turn text collapse** | one `resp.text` repeating itself | no — `stop_reason==Length` already breaks the loop | already breaks; labeled `truncated` |

Axis A is the unbounded runaway and the textbook agent loop. Axis B already self-terminates (the §21
collapse broke the loop on `Length`); its only residual cost is a slightly imprecise telemetry label
(`truncated` vs a hypothetical `degenerate`).

## 5. Adversarial review — necessity first

**Q: Necessary at all, given `max_iters`?** Yes, but scoped honestly. Over `max_iters`, the gate adds:
earlier stop (2 turns vs N), a recovery nudge (`max_iters` has none), and an honest `looped` label. It is
the canonical fix and we have an observed degeneration. **It is NOT what caused the exercise's RED commit**
— that was axis-B collapse, and a broken artifact is already caught downstream by the `tests_green` /
mutation gates. Do not oversell the gate as "fixes the RED commit."

**Q: False positives — could it stop legitimate work?** The chosen signal is **byte-identical
consecutive** tool-call signatures = provably non-progress (same args → same deterministic effect, with
nothing changed in between). It is the lowest-false-positive signal available, and the nudge-first design
gives one recovery turn before stopping. The one theoretical exception — a tool whose result changes
between identical calls due to an *external* actor (polling) — does not apply: workers run **sequentially
in isolated worktrees** (research/19 §9.4), so the agent is the only actor; two identical calls are
non-progress here by construction.

**Q: Transcript validity?** On **Nudge** the loop executes the tools and emits `tool_result`s *before*
appending the nudge, preserving the call→result invariant (openai/oMLX reject a tool_call with no
matching result). On **Stop** the loop breaks and never sends another request, so the (already-recorded)
dangling tool_call is never transmitted — no corruption.

**Q: Label integrity?** A looped run is a **failure**, never `completed`. `classify_stop` is bypassed and
`finish_run` records `looped` (a new value in the existing TEXT column — no schema change). Downstream
`verify` still runs and will reflect the broken/again-incomplete work; the human sees `looped` in
`agent trajectory`. Consistent with the integrity constraints (never report success without evidence).

**Q: Interaction with the existing break?** The natural-completion break (`stop_reason != ToolCalls`)
runs *before* detection, so a turn that stops calling tools never trips the gate — only *continuing*
turns are gated. Correct.

## 6. Design — `crates/agent/src/loopgate.rs` (pure, model-agnostic)

Mirrors `refeed.rs` (pure functions, unit-tested in isolation, no I/O):

```rust
pub fn signature(calls: &[ToolCall]) -> String   // ordered name\0args\x01 … fingerprint; "" if no calls
pub enum Verdict { Proceed, Nudge, Stop }
pub struct LoopGate { prev: Option<String>, strikes: u32 }
impl LoopGate { fn observe(&mut self, sig: &str) -> Verdict; fn strikes(&self) -> u32 }
```

State machine (`STOP_AT_STRIKES = 2`):
- compute `repeat = (sig == prev)`; update `prev = sig`.
- not a repeat → `Proceed`.
- repeat → `strikes += 1`; `strikes >= 2 → Stop` else `Nudge`.

`strikes` is **run-level** (not reset on signature change) so a model that loops, gets nudged, recovers,
then loops on a *different* signature is stopped on that second cycle's first repeat (catches
cycle-switching for free). Consecutive-only detection deliberately does **not** catch period-2
alternation (A,B,A,B) — `max_iters` remains the backstop for that exotic case; we add a window only if we
observe it (start light, adapt).

## 7. Loop integration (`run_ticket`)

After recording the assistant tool-call turn (raw + capped re-feed), before dispatching tools:

```
let sig = loopgate::signature(&resp.tool_calls);
match gate_loop.observe(&sig) {
  Stop   => { print "[loop ] STOP …"; looped = true; hit_max = false; break; }   // do not re-execute
  Nudge  => { /* dispatch tools as normal, then append a [harness control] user nudge */ }
  Proceed=> { /* dispatch tools as normal */ }
}
```

The nudge is a short `user` message prefixed `[harness control]`, **recorded** to the trajectory and
pushed **uncapped** (it must be read whole; it is ~300 bytes). Outcome label:
`let reason = if looped { "looped" } else { classify_stop(last_stop, hit_max) };`.

## 8. Scope cuts (Wu Wei)

- **Intra-turn text-repetition detection (axis B) — DEFERRED.** It does not prevent a bug (the collapse
  already self-terminates on `Length`, and a broken artifact is caught downstream), and the only value —
  a `degenerate` label finer than `truncated` — has **no consumer yet** (no finetune pipeline reads it).
  Revisit when a label consumer exists or we see a collapse that *doesn't* hit `Length`.
  - **2026-06-10 refinement (live re-run, `/tmp/rx-exercise` t2).** Axis B has (at least) two sub-modes,
    not just the §21 verbatim-repetition one (§4 framed it as "one `resp.text` repeating itself"):
    1. **Repetition collapse** — a paragraph repeated 20–22× until `Length` (the §21 case).
    2. **One-shot over-deliberation** — a *single, non-repeating* analytical monologue in `content` that
       runs to `Length` with no tool call. Observed: the model wrote a complete artifact on turn 3, then
       spent its final turn re-analyzing instead of validating (8181 B of `content`, stop=`Length`).
    Both hit `Length` and are caught downstream by `tests_green`, so the defer still holds. **Note the
    reasoning parser does NOT mitigate either**: the deliberation lands in `content`, not `reasoning_content`,
    so neither the field-boundary strip nor an oMLX reasoning-token cap touches it. A future Axis-B mitigation
    must act on `content` length/no-progress, not on the reasoning field. The cheapest candidate first cut, if
    revisited: on `stop_reason==Length` with iterations remaining, inject one "you were cut off — stop
    analyzing and run the validation command" nudge and continue (a truncation-strike bounds it), rather than
    ending the run. Still gated behind a real need (Wu Wei) — not built.
- **Streaming in-flight abort (Tier 2) — DEFERRED.** Killing a turn *mid-generation* (before it burns the
  whole token budget on repetition) needs SSE streaming + an incremental detector. Out of scope until the
  provider exposes a token stream to the loop.
- **Period-2+ window / no-progress (state-change) detection — DEFERRED** until observed.

## 9. Review fold-in (what changed from the first draft)

1. **Necessity reframed (review Q1):** scoped as "stop runaway tool-call cycles + honest label," explicitly
   *not* "fix the §21 RED commit." Prevents overselling.
2. **Intra-turn detection cut (Q1 + Wu Wei):** moved from "core" to DEFERRED — no current consumer for the
   finer label, and it doesn't prevent a bug.
3. **Signal narrowed to consecutive-identical (Q2):** chose the lowest-false-positive signal over a
   recent-window; documented the period-2 blind spot + `max_iters` backstop rather than building for an
   unobserved case.
4. **Execute-before-nudge / break-before-execute (Q3):** nail the call→result invariant on Nudge and avoid
   re-running the repeated call on Stop.
5. **Run-level strikes (Q2/Q4):** robust to cycle-switching without a window; matches "2 strikes → stop."
6. **`looped` is a failure label, bypassing `classify_stop` (Q4):** queryable, distinct from
   `max_iters`/`truncated`/`completed`, no schema change.

## 10. Success criteria

- `loopgate.rs` is pure (no I/O), depends only on `provider::ToolCall`, and is fully unit-tested
  (no-repeat → Proceed; A,A → Nudge then Stop; alternating A,B,A → all Proceed; switching A,A,B,B →
  Proceed,Nudge,Proceed,Stop; `strikes()`).
- `run_ticket` stops a same-(name,args) cycle at strike 2 with a recorded `[harness control]` nudge at
  strike 1, and records `stop_reason = "looped"`.
- `cargo build`, `cargo clippy` clean; full workspace test suite green.
- Wiki updated (index/log/decisions/active-work). Nothing committed without Gary's go.
