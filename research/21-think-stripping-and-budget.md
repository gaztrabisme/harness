# research/21 — `<think>`-stripping + budget room for the verbose local model

> Scope + adversarial review for a small, silent-failure-prone change: give the verbose Qwen reasoner enough
> budget to *finish* a hard task, and stop its reasoning traces from (a) silently filling the model's own
> context every turn and (b) overflowing Claude when reviewing a run. Sized as a **focused** review (3 lenses),
> not a pillar cathedral — the change is reversible code, but its failure modes are quiet (context corruption,
> lost provenance, eaten output).

## 1. Problem (grounded in the code, 2026-06-10)

Two failure modes seen repeatedly on local oMLX exercises (the 35B `Qwen3.6-35B-A3B-oQ8-fp16-mtp`):

- **plan-then-stop / read-only wander** — the model reasons at length, then ends its turn (or wanders reading
  files) without writing. Recorded as `completed` or exhausted `max_iters`, not a crash.
- **verbosity** — the model's reasoning is long. With the current defaults that reasoning is doubly costly.

What the code actually does today:

- `openai.rs:129` takes `choice.message.content` **verbatim** into `Response.text`. If the model emits
  `<think>…</think>`, it is preserved.
- `main.rs:332-333` pushes that raw assistant `content` back into `messages`, and `message_to_wire`
  (`openai.rs:32-53`) ships it whole on the next request. **So every prior turn's reasoning re-enters the
  model's context on every subsequent turn** — a quadratic context leak against the 32K oMLX window.
- Defaults are tiny: `DEFAULT_MAX_TOKENS=1024`, `DEFAULT_MAX_ITERS=8` (`main.rs:48-49`). 1024 *output*
  tokens/turn is smaller than a single verbose think block → the model can be `length`-truncated mid-thought
  before it ever emits a tool call. The env knobs `HARNESS_MAX_ITERS` / `HARNESS_MAX_TOKENS` already exist.
- `agent trajectory` prints only a 100-char preview (`main.rs:411`), so a *real* audit means catting raw JSONL
  → the reviewer (Claude) eats every think block.

**Important honesty caveat:** whether this model emits literal `<think>` tags is empirical (global context says
the oMLX sampling profile runs Qwen "thinking off"). The stripper is therefore designed to be a **no-op when no
tags are present** — it costs nothing if reasoning isn't delimited, and the real lever in that case is budget +
the (deferred) plan→execute nudge. We let the telemetry tell us which world we're in rather than guessing.

## 2. Diagnosis discipline — let telemetry adjudicate, don't pre-build the fix

The harness already distinguishes `truncated` (`StopReason::Length`) from `completed` (`classify_stop`,
`main.rs:375`). So the budget question is answerable with a number, not a guess:

- run with a bumped budget, read the per-run `stop_reason`.
- `truncated` dominates → budget was the binding constraint; the bump is the fix.
- `completed`-with-no-writes dominates → behavioral; **that** is when the force-plan→execute nudge earns its
  build. **Not before.** (Stays OUT of this scope deliberately.)

## 3. Deliverables

### 3a. Prod (lands in the harness repo; TDD; useful regardless of the exercise)

1. **`strip_think(content: &str) -> String`** — pure, tolerant, in `agent` (peer to the `evolve.rs`/`explore.rs`
   "just code, unit-tested in isolation" style). Rules:
   - remove well-formed `<think>…</think>` spans (case-insensitive tag match), including **multiple** spans;
   - an **unclosed** `<think>` running to end-of-message → strip from the tag to EOM (the lenient-reader
     posture: tolerate torn input rather than corrupt it);
   - **no-op when no tags present** (returns content unchanged, trimmed of the now-empty gap);
   - never touches `tool_calls` (those are structured, separate from `content`).
2. **Loop re-feed** (`main.rs` ~327/332-334): **record the assistant message raw** (trajectory keeps
   everything — provenance), **push the stripped copy into `messages`**. The natural-break branch (line 325-329)
   records raw and breaks — no re-feed there, so it stays whole in the trajectory by construction.
3. **`agent trajectory <id> --full [--think]`** — a full-content read (not the 100-char preview), think-
   **stripped by default**, `--think` to include raw reasoning. This is the "conscious choice to audit."

### 3b. Exercise (throwaway `/tmp/rx-exercise`, never in harness git history)

- Separate `git init` repo in `/tmp`; its own board (`HARNESS_DB` + cwd local to it), own worktrees +
  trajectories. The harness repo's git history is untouched; cleanup = delete the dir.
- Driven by the freshly-built `agent` binary from this repo.
- **Task: a regex engine (Thompson NFA)** — `pub fn is_match(pattern: &str, text: &str) -> Result<bool,
  RegexError>` over literals + escapes, `.`, `*`, `+`, `?`, `|`, `( )`, anchors `^`/`$`. std-only. A provided
  **failing ~25-case test suite is the acceptance artifact** (gate by an artifact). Error-prone enough
  (epsilon transitions, greedy quantifiers) to force write→test→fail→fix across many turns; classic enough a
  35B can plausibly finish (achievable, not unsatisfiable — the prior wander-failures were on *impossible*
  contracts).
- Full spine: `new` → `validation "cargo test"` → `run` (worktree) → `verify` (`tests_green` = literal
  pass/fail). **Stop at green; do NOT land** (oMLX output is exercise-only — prod is Claude's).

### 3c. Budget (env-only, per Gary)

`HARNESS_MAX_TOKENS=12288`, `HARNESS_MAX_ITERS=40` to start; retune between runs. Raising the oMLX
`max_context_window` past 32K is explicitly **out** (Gary's lever, later) — if context fills despite the strip,
that's the verdict to surface, not a thing to fix here.

## 4. Success criteria (gate by a number / artifact)

- **Harness:** a long, verbose, multi-iteration run completes end-to-end; `run` rows + trajectory intact;
  isolation holds (artifact present in the worktree, absent from the `/tmp` repo's main during the run); the
  strip **measurably** shrinks re-fed context vs the recorded raw (show byte counts).
- **strip_think:** unit tests cover well-formed / multiple / unclosed / absent / tool-call-adjacent; workspace
  green + clippy clean.
- **Model:** either `cargo test` goes green via the loop, **or** the telemetry says *why* (`truncated` vs
  `completed`-no-write) — both are acceptable outcomes; the second is a clean diagnosis, not a failure of the
  sprint.
- **Review:** the run is auditable think-stripped without overflow; `--think` opts back in.

## 5. Failure surface (why even a light review)

- **Over-strip:** a greedy or wrong-boundary match eats real output or a tool-call rationale → quietly worse
  runs. Mitigation: precise span matching + tests with adjacent content.
- **Provenance inversion:** if we strip *before* recording, the trajectory loses reasoning forever. Mitigation:
  record-raw-then-push-stripped, pinned by a test.
- **Tool-call coherence:** stripping reasoning from history while keeping the tool_call + result — is the model
  still coherent next turn? (Standard: most harnesses don't re-feed reasoning; Qwen's template drops prior
  `<think>`.) Flag for the review.
- **Context math:** does 12288 max_tokens + growing history actually stay under 32K, or does oMLX silently
  truncate the *prompt* (dropping the system prompt = silently disabling the workpad/case-law)? Flag.

## 9. Adversarial review — findings of record

Three lenses run in parallel against this scope (2026-06-10), **before any code written**. All three returned
non-SHIP. The load-bearing finding inverts the sprint's headline premise.

### Verdicts
- **A — strip_think correctness:** REWORK-FIRST. A naive span match over-strips; needs depth-counting, content-only,
  test-pinned with adjacent-content cases. (Moot given C, below.)
- **B — loop integration / context math:** REWORK-FIRST. The real leak is *uncapped re-fed assistant prose + tool
  output*, not tags. `FIELD_CAP=8192` is recorder-only — it does **not** bound what `messages` re-ships. 12288
  max_tokens × growing history can silently blow the 32K prompt window → oMLX drops the *system prompt* (workpad +
  case-law) silently. Cap re-fed content; add an overflow signal.
- **C — Wu-Wei / right-cut:** RE-SCOPE. **Live probe of `Qwen3.6-35B-A3B-oQ8-fp16-mtp` emits ZERO `<think>`
  tags** — it reasons in plain markdown prose, even with `/think` + `chat_template_kwargs:{enable_thinking:true}`.
  So `strip_think` is a **no-op on 100% of real output**: dead code by construction.

### Load-bearing fact
The model does not delimit reasoning. Every premise that assumed `<think>` tags (strip the span, `--think` to opt
back in, "no-op when absent" as a safety net) is therefore building a lever with nothing on the other end. The
verbose-context leak is **real**, but it is *untagged prose re-fed every turn* — a stripper cannot find it.

### Re-scope decision (RESOLVED 2026-06-10)
**Cut `strip_think` and `--think` entirely.** Pivot the prod deliverables to what the telemetry + code actually
support:

1. **Budget bump (env-only, zero code):** `HARNESS_MAX_TOKENS=12288`, `HARNESS_MAX_ITERS=40`. This is the first
   lever and it answers the diagnosis question (§2) with a `stop_reason` number.
2. **Stop re-feeding *prior* assistant prose (the real fix for the untagged leak):** the loop should re-feed the
   structured `tool_calls` (coherence) but **not** carry every prior turn's full reasoning narrative forward. This
   is a one-line loop policy at `main.rs:332-334`, pinned by a test on the re-fed `messages` vector — and it is
   model-agnostic (works whether or not tags ever appear). Bound any re-fed text by a cap (B's point) so history
   cannot silently evict the system prompt.
3. **`agent trajectory <id> --full`:** plain full-content read (drop `--think` — there is nothing to strip). The
   "conscious choice to audit" becomes `--full` (read everything) vs default (100-char preview). Decoupled from any
   stripping.
4. **Overflow signal (B):** when the assembled prompt approaches the 32K window, surface it in telemetry rather
   than letting oMLX silently truncate the system prompt. Minimal: record assembled-prompt byte/char size per turn.
5. **Exercise unchanged:** keep the throwaway `/tmp` Thompson-NFA regex task; seed a few already-green cases in the
   provided suite so a partial pass is visible (not all-or-nothing).
6. **plan→execute nudge stays deferred** (§2 discipline): build it only if `completed`-no-write dominates *after*
   the budget bump. The bumped run's `stop_reason` telemetry adjudicates.

Net: the sprint keeps its *intent* (let the verbose model finish a hard task; audit without drowning) but drops the
dead lever (`strip_think`) and redirects effort to the leak that actually exists (uncapped re-fed prose) + the
budget that lets the model finish + an honest audit path. §3a/§3b/§4 above are superseded by this list where they
conflict.
