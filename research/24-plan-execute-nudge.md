# research/24 — plan→execute nudge (the no-action stall)

**Status:** BUILT (2026-06-10). Scope → adv review (§10) → built → live-validated on oMLX → see §11.
**Thesis tie-in:** oMLX must prove a cheap local model can get work done (binding since the verbosity sprint).
The single most-cited local-model failure across the breadcrumbs is **plan-then-stop**: the model emits a plan
or analysis in prose, calls **no tool**, and the loop accepts that as completion. This is the fix.

## 1. The failure mode (observed, not theoretical)

The loop's natural-stop branch (`main.rs:368`):

```rust
if resp.stop_reason != StopReason::ToolCalls {
    // record the assistant turn, break — treated as DONE
    hit_max = false;
    break;
}
```

A turn with no tool calls ends the run and `classify_stop` labels it `completed`. But a model that *described*
what it would do and stopped has done **nothing** — yet it banks a `completed` run. Observed instances:
- **Unit B exercise (2026-06-09):** 35B produced a correct plan, ended naturally, wrote **0 files**. Logged
  `completed`-shaped (the `truncated` fix caught the budget variant, not this behavioural one).
- **explore unsatisfiable-contract run (2026-06-10):** workers *wandered the repo reading files* rather than
  attempting work — read-only activity that never mutates, then stops.
- **t2 over-deliberation (2026-06-10):** wrote a complete artifact then burned the last turn re-analysing instead
  of running `cargo test`.

Earlier this was logged as "deferred — prod is Claude's job" (decisions.md, Telemetry Unit B known-hole). **That
stance is now reversed**: the oMLX-must-work thesis makes local-loop behaviour in-scope.

## 2. The core decision — how to tell "stalled" from "genuinely done"

The whole design hinges on **not** nudging a model that legitimately finished. A blanket "re-prompt on every
no-tool stop" punishes correct completions and risks a plan↔nudge ping-pong. We need an **objective** signal that
a no-action stop is illegitimate. Two facts the loop already owns make it clean:

1. **Did the run take any action?** Track `acted: bool` — set true the first time a **gate-allowed mutating tool
   actually executes** (`gate::tool_is_mutating(name)` ⇒ `write_file`/`bash`, never `read_file`). A run that
   reaches a natural stop with `acted == false` produced no change to the worktree.
2. **Is this a kind that *must* mutate to be done?** `board::spine::kind_is_code(kind)` (build/bugfix/refactor —
   the same predicate the Harden gate uses). A code ticket that ends having written nothing is **incomplete by
   definition**; a research/analysis ticket legitimately may write nothing, so it is exempt.

**Signal:** `natural stop (End, no tool calls) AND !acted AND kind_is_code(kind)` ⇒ plan-then-stop. Anything else
(it wrote something; or it's a non-code kind) ⇒ accept the stop. This is "gate by a number/artifact, not a vibe"
— the artifact is *whether a mutating tool ran*.

## 3. Behaviour

Mirror the loopgate's "one recovery turn, then stop" shape (research/22):

- **First** illegitimate no-action stop → **don't break.** Inject one `[harness control]` plan→execute nudge and
  continue the loop:
  > *"[harness control] You produced a plan/analysis but called no tool — describing work is not doing it. The
  > acceptance criteria are unmet and nothing has been written. Take a concrete action now (write_file / bash). If
  > you believe the work is genuinely complete, run the validation command to prove it before stopping."*
- **Second** time it stops with still `!acted` → **accept the exit but label it `stalled`** (a new run outcome),
  never `completed`. One nudge, then an honest failure label — no infinite ping-pong.
- A nudge that *works* (next turn calls a mutating tool) → `acted` flips true; a later natural stop is now a
  legitimate `completed`.

**Bound:** at most **1** plan-nudge per run (a `plan_nudged: bool`), exactly like loopgate's strike→nudge→stop.

## 4. Why this is independent of the loop gate (no interaction)

The loop gate (`gate_loop.observe`) is only reached **inside the ToolCalls branch** (`main.rs:389`) — it
fingerprints *tool calls*. The plan→execute nudge lives in the **no-tool-call branch** (`main.rs:368`), which the
loop gate never sees. They are disjoint by construction:
- loopgate = "calling the same tool forever" (action without progress).
- plan→execute = "never calling a tool at all" (no action).
Two different stalls, two independent counters, no shared state.

## 5. Shape (pure core + thin wiring — mirrors refeed.rs / loopgate.rs)

A pure decision function, unit-testable without a server or a loop:

```rust
// crates/agent/src/planexec.rs
pub enum Verdict { Accept, Nudge, Stall }

/// Decide what to do on a *natural stop* (the no-tool-call branch).
/// `acted`   — did any mutating tool run this run?
/// `code`    — kind_is_code(ticket.kind)?
/// `nudged`  — has the single plan-nudge already been spent?
pub fn verdict(acted: bool, code: bool, nudged: bool) -> Verdict {
    if acted || !code { Verdict::Accept }      // did work, or non-code kind → legitimate stop
    else if !nudged   { Verdict::Nudge }       // first no-action stop on a code ticket → one chance
    else              { Verdict::Stall }        // already nudged, still nothing → honest failure
}
```

Wiring in `run_ticket` (the `stop_reason != ToolCalls` branch): compute `verdict`; on `Nudge` record+push the
`[harness control]` message and `continue` instead of `break`; on `Stall` break and force the `stalled` label; on
`Accept` break as today. `acted` is set in the dispatch loop when a mutating tool executes gate-allowed. The run
outcome adds `stalled` alongside `completed|truncated|max_iters|looped|error`.

## 6. Forks for Gary (genuine decisions)

1. **Signal: mutation+code-kind (recommended) vs blanket one-nudge-on-every-natural-stop.** Recommend the
   objective signal — it never punishes a legitimate completion and reuses two existing predicates. Blanket is
   simpler but nudges correct stops and needs the same bound anyway.
2. **Terminal label `stalled` (recommended) vs leave `completed`.** Recommend a distinct label — research/17's
   entire point is honest outcome labels for telemetry/RL; a do-nothing run scored `completed` poisons the signal.
3. **Cap = 1 nudge (recommended) vs N.** Recommend 1, mirroring loopgate (one recovery turn, then an honest stop).
4. **Non-code kinds exempt via `kind_is_code` (recommended) vs nudge everything.** Recommend exempt — a
   research/analysis ticket may legitimately write nothing.

## 7. Adversarial review target (light, per the Process-gate rule)

This is **reversible** (a loop-behaviour tweak, no schema), **telemetry-labelled** (the new `stalled` outcome +
`run.log` make it loud), and **small** (mirrors loopgate). Per the rule that's a *light* focused review, not the
heavy multi-agent workflow — same treatment loopgate got. The review must attack the two-sided error surface:
- **False positive (nudging a real completion):** can `acted && code` still be a genuine stop that gets nudged?
  e.g. a refactor whose only change was via `bash` (counts as acted ✓) vs one that legitimately needed no edit
  (rare for a build kind). Is `kind_is_code` the right exemption boundary?
- **False negative (accepting a real stall):** a code ticket that `write_file`s *the wrong thing* then stops —
  `acted == true` so we Accept. Is that correct? (Argued yes: it *acted*; correctness is the downstream
  `tests_green` gate's job, not this nudge's — same division of labour as research/23.)
- **Ping-pong / non-termination:** does the 1-nudge bound truly guarantee termination under every interleaving
  with the loop gate and `max_iters`?
- **Necessity (Wu Wei):** does `max_iters` already bound the damage enough that the nudge earns its complexity?
  (Counter: `max_iters` mislabels the stall `completed`/`max_iters` and never gives a recovery turn.)

## 8. Success criteria (gate the build)

- `cargo build`, `cargo clippy --workspace --all-targets` clean.
- Full workspace suite green, plus new `planexec` unit tests covering the full `verdict` matrix
  (acted×code×nudged) and a `ScriptedProvider` integration test (mirror loopgate's): a no-action code run gets
  exactly one nudge then `stalled`; a run that acts after the nudge ends `completed`; a non-code no-action run is
  `Accept`ed unlabelled.
- Live oMLX exercise: a plan-then-stop reproduction gets nudged into acting (or honestly labelled `stalled`).

## 10. Adversarial review — fold-in (2026-06-10)

Two focused reviewers (false-positive/necessity; false-negative/termination), mirroring the loopgate
treatment. Both returned **ship-with-changes**. Termination proven sound (the 1-nudge bound makes `verdict`
monotone; loopgate lives in the disjoint ToolCalls branch; `max_iters` backstops). The forks (§6) are now
**resolved by the review**, not left open:

- **BLOCKER (reviewer B) — `acted` cannot be tool-based.** §2/§5 said `acted` = "a gate-allowed mutating tool
  ran", citing `gate::tool_is_mutating` (= `!read_file`, so `bash` counts). But the system prompt (`main.rs:731`)
  **orders the model to start with `bash ls -R`** — so `acted` would be true on essentially *every* run and the
  nudge would never fire. The feature was dead on its primary target. **Fix:** `acted` = **the worktree is dirty
  at the natural stop** (`git status --porcelain` non-empty), computed once at the branch — base-free,
  tool-agnostic (catches a `bash sed` edit, ignores `bash ls`, and a write-then-revert net-no-op correctly reads
  clean). The fresh-worktree invariant (every `agent run`/explore forks a clean tree; rework reaps first) makes
  dirty-at-stop ≡ "this run wrote something". New helper `git::is_dirty`. `lines_changed_against` is the *wrong*
  call mid-loop — it diffs `base...HEAD` and the work is still uncommitted, so it reads 0. The pure
  `verdict(acted, code, nudged)` core is unchanged; only the *source* of `acted` moves.
- **MAJOR (reviewer A) — split the feature, gate the nudge on a number.** The `stalled` *label* pays its way on
  telemetry honesty alone (Unit B's 0-file run must not bank `completed`). The *nudge* is the speculative part:
  of the three cited cases only Unit B is a clean hit, and its sole un-fixed harm is the mislabel (the label
  alone fixes it). **Decision:** ship the label unconditionally; build the nudge but make §8's live exercise a
  real Wu-Wei gate — report nudge→recovery conversion; if it's ~zero, cut the nudge and keep the honest label.
- **Label plumbing (both).** `stop_reason` is free-form `TEXT` (no enum, no DB `CHECK`) → **no migration**. But
  `classify_stop` can't see `acted`, so `stalled` is **forced at the call site** like `looped`:
  `if looped {…} else if stalled {"stalled"} else {classify_stop(…)}` (looped wins precedence). `rank_outcomes`
  sorts on `passed`+`lines_changed`, **not** the `stop` string, so a `stalled` branch (passed=false, lines=0)
  already sorts to the bottom — no ranking change needed.
- **Recovery path (reviewer A trap).** `stalled` is set only in the `Stall` arm, which fires on the *final*
  stop's verdict — so nudge→act→stop computes `acted=true` → Accept → `completed`. The label is keyed on final
  `acted`, never on "was ever nudged". Pinned by integration test (d).
- **Wiring (reviewer B MINOR).** The natural-stop branch records the assistant turn but (today) does not push it
  to `messages` because it breaks. On `Nudge` we now `continue`, so we must push a capped copy of the assistant
  plan turn *then* the `[harness control]` nudge — otherwise the next turn's context lacks the referent. Don't
  double-record the assistant turn.

## 11. BUILT — fold-in (2026-06-10)

Built exactly as §5/§10 specify; the pure `verdict(acted, code, nudged)` core is unchanged from §5.

- **`acted` = worktree-dirty at the natural stop** (the BLOCKER fix). New `git::is_dirty(dir)` = `git status
  --porcelain` non-empty (catches untracked new files; `git diff --quiet` would miss a fresh `write_file`,
  `lines_changed_against` reads 0 mid-loop because the work is uncommitted). Computed once at the no-tool-call
  branch.
- **`crates/agent/src/planexec.rs`** — pure core + 5 unit tests over the full acted×code×nudged matrix
  (acted_always_accepts, non_code_is_exempt, code_no_action→Nudge, after-nudge→Stall, monotone bound).
- **Wiring (`main.rs run_ticket`).** Natural-stop branch now: `let acted = git::is_dirty(cwd);` →
  `match planexec::verdict(acted, board::kind_is_code(&t.kind), plan_nudged)` — **Accept** breaks as before;
  **Nudge** pushes a *capped* copy of the assistant plan turn (`refeed::cap`, so the referent survives) + the
  `[harness control]` plan→execute message, sets `plan_nudged=true`, `continue`s; **Stall** sets `stalled=true`,
  `hit_max=false`, breaks. Label wiring keeps `looped` precedence:
  `if looped {…} else if stalled {"stalled"} else {classify_stop(…)}`. No migration (`stop_reason` is free-form
  TEXT); `rank_outcomes` already sinks a `stalled` row (passed=false, lines=0).
- **4 `ScriptedProvider` integration tests** over the real loop: `plan_then_stop_gets_one_nudge_then_stalls`,
  `writing_then_stopping_is_completed_no_nudge`, `non_code_no_action_is_exempt`,
  `nudge_then_act_recovers_to_completed` (pins the reviewer-A recovery trap — label keyed on *final* `acted`, not
  "was ever nudged"). Test helper makes cwd a git-init'd dir **distinct from root** so the trajectory write (under
  `root/.harness/`) never dirties cwd and defeats stall detection — mirrors production (worktree ≠ repo root).
- **Evidence:** `cargo build` + `cargo clippy --workspace --all-targets` clean; suite green (agent 67/1-ignored,
  board 20, pi-iso 2, provider 1-ignored) — +9 over the pre-slice baseline.
- **Live oMLX validation (3 throwaway exercises, never landed).** Distribution finding: on real refactor tickets
  the 35B fails via **action-without-progress** — run 1 → `max_iters` (read-wander), run 2 → `looped` (repeated
  `sed -i`) — both already owned by max_iters/loopgate, which fire *before* any natural stop. planexec's
  natural-stop signal only surfaces on an **already-satisfied** task (run 3): the model declared "No edits were
  needed" → plan nudge fired → it re-ran `cargo build` / re-declared done → **STALL** → row labelled `stalled`
  (not a faked `completed`). This is the live end-to-end proof of the wiring against the real provider.
- **Wu-Wei finding (reviewer A's number-gate, answered honestly): the nudge did NOT convert to action.** On these
  tasks the model insisted the work was complete rather than acting on the nudge — conversion ≈ 0 in the observed
  runs. Per reviewer A's split, the **`stalled` label is the value** (it pays its way on telemetry honesty alone:
  a do-nothing run must not bank `completed`); the **nudge is on probation** — keep it for now (it's one bounded
  turn, cheap), cut it if a larger sample confirms near-zero conversion. The label and the nudge are independent,
  exactly as the review required.

## 9. Deferred (not in this slice)

- Graded/multiple nudges or escalating wording (start with 1; widen only if a number demands it).
- Tying the nudge to *acceptance-criteria satisfaction* rather than mutation-occurred (criteria are prose;
  checking them mid-loop is an LLM call — out of scope, the mutation signal is the cheap objective proxy).
- Reactive think-escalation on `stalled` (the research/23 follow-on — a `stalled` outcome is a candidate trigger,
  but escalation stays deferred until a number shows the gate-catch is insufficient).
