# research/26 — Bounded Thinking (adaptive self-assessed budget): design + adversarial review

> Decision 3 (`decisions.md`, 2026-06-10): *thinking is assumed helpful, allow it BOUNDED; mechanism = a cheap
> **think-OFF, JSON-schema-enforced pre-step** where the model self-assesses how much thinking budget the task needs,
> then the real call runs `thinking_budget:N` at that estimate.* This doc designs that mechanism, confronts it honestly
> against research/23's *opposite* recommendation (reactive escalation, no predictive router), and routes it through an
> adversarial review before any build. **To be designed + adv-reviewed before build** (a bad budget estimate silently
> degrades quality → silent-failure class → Process-gate trigger).

## 1. Goal + grounding

**Goal:** let the local 35B *think* on turns that need it (Decision 3: the think block is computation — more
deliberation → better output), **bounded** so it can never run away.

**Hard facts this design must obey (research/23, all verified on our oMLX):**
- **F1 — `max_tokens` does NOT bound `reasoning_content`.** Only `thinking_budget`/wallclock bounds thinking. Naive
  think-ON ran to **8091 reasoning tokens / 830 s** and blew a 200 s client cap on 9/9 runs. → **a budget is mandatory**,
  not optional.
- **F2 — the verified wire knobs are per-request** on `/v1/chat/completions`: `chat_template_kwargs:{enable_thinking:
  bool}` (thinking fully OFF when false, ~1 tok), the flat `thinking_budget:N` (the real field — `reasoning.max_tokens`/
  `thinking.budget_tokens` are ignored), `structured_outputs` / `guided_grammar` (GBNF) / `response_format` for schema
  enforcement, full sampling. (oMLX server/profile config stays in the dashboard; these are inference controls.)
- **F3 — the token-elasticity trap is real.** A *fixed-tight* `thinking_budget` (e.g. 64) does not save tokens — it
  **relocates** the spiral into `content` (`content` ballooned to 1329 c / 502 tok ≈ baseline). Only the binary extremes
  were clean (budget=0/think-off ≈ 105–143 tok). → a budget must be **generous enough**, never tight-to-force-brevity.
- **F4 — think-OFF is strictly better for single-step structured dispatch** (3–15× fewer tokens, same/better tool
  calls, reasoning empty every time). → **the triage pre-step belongs in think-OFF** (it *is* single-step structured
  dispatch); this is the one place research/23's strongest positive result directly applies.
- **F5 — measured residual (research/23 §7):** on *generative code* think-OFF silently fails **~30%** (near-miss
  artifacts, no signal); naive think-ON is unusable (F1). The downstream `tests_green` artifact gate is the catch.

**Current code state** (the thing this design changes):
- `provider::Request.think: bool` (lib.rs:92) → `request_to_body` emits `chat_template_kwargs:{enable_thinking: req.think}`
  (openai.rs:76). **No `thinking_budget` is ever sent.** The loop hardcodes `think:false` every turn.
- anthropic.rs ignores `think` (its default ≈ no extended thinking).

---

## 2. The mechanism (Gary's design, concretized)

Two calls per "thinking-eligible" decision point:

**Phase A — budget triage (think-OFF, schema-enforced).**
- Wire: `enable_thinking:false` + `response_format`/`guided_grammar` pinning a tiny JSON schema. Cheap by F4.
- The model is asked to *estimate effort, not solve* (anti-continuation framing, borrowed from research/25 §2.5).
- **Schema choice (design decision, §6):** categorical, not raw-integer. The model estimates a **complexity class**,
  the harness maps class → budget via a fixed generous table. Categorical is far more reliable than asking a 35B for a
  raw token count, and the table keeps every rung *generous* (dodges F3 elasticity by construction).
  ```json
  { "complexity": "none" | "light" | "moderate" | "deep", "why": "<= 1 short sentence" }
  ```
- Map (env-tunable, generous on purpose):
  `none → 0` (collapses to think-OFF — the elegant degenerate case),
  `light → 1024`, `moderate → 2560`, `deep → 4096` (ceiling; below the 8091 runaway, leaves window for content).

**Phase B — bounded thinking call (think-ON unless `none`).**
- Wire: `enable_thinking: (N>0)` + (when N>0) `thinking_budget: N`, plus the normal request (tools, messages).
- **N is clamped to `[0, THINK_BUDGET_MAX]` at the harness, regardless of triage** — defense against a triage that
  returns garbage or a future table edit. The triage *advises*; the harness *bounds*.

Degenerate case is first-class: triage says `none` → N=0 → Phase B is exactly today's think-OFF call (no behavior
change, no extra round-trip cost beyond Phase A). The whole mechanism reduces to "today" when thinking isn't needed.

---

## 3. The honest tension — predictive router vs reactive escalation

**research/23's actual RECOMMENDATION was the OPPOSITE of this mechanism.** Verbatim from the wiki: *"reactive
escalation, NOT a predictive effort-router — default every turn to `enable_thinking=false`; escalate to think-ON only on
a detected failure (`stop_reason==Length` / loopgate strike / gate red); think-OFF is so cheap a wasted first attempt
beats a triage call."* And §7 measured: *"a predictive router can't see a content-level near-miss any better than
reactive → router stays shelved."*

Gary's Decision-3 mechanism **is a predictive effort-router** (the Phase-A triage predicts how hard the task is). So
this design must clear the bar research/23 set, not pretend the conflict isn't there. Three reconciling arguments — and
the one place each is *weak*:

1. **Objective changed.** research/23 optimized *minimize cost; the gate is the safety net*. Decision 3 optimizes
   *assume capable, maximize quality*. The "a wasted think-OFF attempt is cheaper than a triage call" argument is a
   **cost** argument; under a quality objective it no longer dominates. *Weak point:* the triage call still costs a
   round-trip every turn — cost didn't vanish, it just stopped being the tie-breaker. §4 F-e + §5 must measure it.
2. **Different question.** research/23's shelved router predicted *failure* ("will this turn fail?"); §7 showed a
   triage can't see a content-level near-miss. Phase-A predicts *effort* ("how much should I think?"), not failure.
   Estimating effort up-front is a genuinely different task from spotting a near-miss after the fact. *Weak point:* if
   effort-estimation is *also* unreliable on a 35B, the mechanism is theoretically motivated but empirically hollow —
   this is the **central adv-review question (§5).**
3. **Elasticity is dodged, not triggered.** research/23 rejected a *fixed graded `thinking_budget` ladder* because a
   blind tight rung triggers F3. Here the rungs are **self-selected** and **generous** — the model picks the rung, and
   even the top rung (4096) is below the runaway. *Weak point:* a self-selected rung that's too *low* (model
   under-estimates a hard task) re-triggers F3 silently — the **F-a failure mode (§4).**

**Design stance (integrity-preserving): the predictive pre-step must EARN its keep against a fixed-budget baseline.**
We do not assume Phase-A works. §5 defines the number it has to beat. If it doesn't beat a fixed generous bounded budget
+ reactive escalation, we **don't build the triage** — we ship the cheaper baseline. That honors Decision 3's *intent*
(bounded thinking, on) while refusing to build speculative complexity (Wu Wei + "gate by a number, not a vibe").

---

## 4. Failure modes (the adv-review surface)

| # | Failure | Class | Why it bites | Candidate mitigation |
|---|---------|-------|--------------|----------------------|
| **F-a** | Triage **under**-estimates → real call's thinking is cut at budget → F3 elasticity relocates the spiral into `content` → subtly-wrong artifact, **no signal** | **silent quality loss** (the Decision-3-flagged class) | a 35B mis-rating a `deep` task as `light` is plausible; the cut is invisible at the wire (content looks normal) | generous rungs; **detect budget-exhaustion** (open Q §5 — what does oMLX return when `thinking_budget` is hit?); on exhaustion escalate one rung + re-run; downstream `tests_green` backstop (F5) |
| **F-b** | Triage **over**-estimates → wasted thinking, slow | cost (not correctness) | tolerable under a quality objective; bounded by MAX | clamp; accept |
| **F-c** | Triage itself runs long / verbose despite think-OFF | cost | F4 says think-OFF dispatch is cheap, and schema bounds output — but F5 showed think-OFF can still be verbose in `content` on *non-dispatch* tasks | schema (`response_format`) hard-bounds triage output; triage prompt is dispatch-shaped |
| **F-d** | **Effort-estimation is no more reliable than failure-prediction** (research/23 §7) → the whole pre-step adds a round-trip for noise | validity (kills the mechanism) | this is the central risk; if true, reactive escalation wins | **§5 empirical gate** — pre-step must beat fixed-default baseline |
| **F-e** | Doubling round-trips per turn (triage + real) → ~2× latency at ~70 tok/s | cost / Wu-Wei | every turn pays Phase A even when `none` | cadence (§6); measure overhead; collapse-to-today when `none` keeps the *real* call single |
| **F-f** | Cadence mismatch — per-turn triage wastes calls on tool-ingestion turns; per-ticket triage misses turn-level variation | design | a read-result turn needs no thinking; a synthesis turn needs lots | §6 cadence decision; start simple, measure |
| **F-g** | Triage returns invalid/out-of-enum JSON | robustness | schema enforcement can still fail (model emits prose) | clamp + default-to-`none` (fail safe = today's behavior, never fail loud-stop on a triage hiccup) — **but log it** (silent fallback that hides a broken triage is itself the anti-pattern; emit a `[triage]` signal) |

**The silent-failure through-line (consistent with research/25 §5):** F-a and F-g are the dangerous ones because they
fail *silently*. Rule, same as the re-feed design: **the mechanism may be imperfect, but never silently imperfect** —
budget exhaustion and triage-parse-fallback both emit a signal (a `[think]`/`[triage]` line, like the existing `[ctx]`),
never a quiet degrade.

---

## 5. The empirical gate (what must be measured — gates the build)

Decision 3 says "designed + adv-reviewed before build." This section is the number the design must produce **before the
triage is built**. All probes are throwaway oMLX exercises (`/tmp`, never landed), extending research/23 §7's harness.

**Probe 1 — the open wire question (blocks F-a mitigation): what does oMLX return when `thinking_budget` is exhausted?**
Run a known-hard prompt with `enable_thinking:true, thinking_budget:256`. Inspect the response: does `stop_reason`
become `length`? Is `reasoning_content` truncated with a flag? Does it silently spill into `content` (F3)? **The
mitigation for F-a depends entirely on this answer** — if exhaustion is detectable, we can escalate-and-retry; if it's
silent, F-a has no in-band catch and we lean harder on the downstream gate.

**Probe 2 — the central validity gate (F-d): does a think-OFF 35B self-estimate beat a fixed default?**
On research/23 §7's battery (the subset-regex / Thompson-NFA task class, graded vs a hidden case battery), compare three
arms at equal MAX content budget:
- **(A) baseline:** fixed generous bounded budget every turn (`thinking_budget = 2560`, no triage).
- **(B) Gary's mechanism:** Phase-A triage → mapped budget.
- **(C) research/23's recommendation:** think-OFF default + reactive escalation (re-run think-ON-bounded on
  `Length`/loopgate/gate-red).
Metric: green-rate and silent-fail-rate (the F5 ~30% number is the bar). **Decision rule:** build the triage (B) **only
if** it beats (A) on green-rate by a margin that justifies the extra round-trip; otherwise ship (A) or (C). This is the
Wu-Wei / gate-by-a-number discipline applied to Decision 3 itself.

**Probe 3 — cadence (F-e/F-f):** measure Phase-A overhead per turn and the `none`-rate (how often triage says no
thinking) on a real ticket trajectory. A high `none`-rate cheaply justifies per-turn triage; a low one argues for
per-ticket or escalation-only.

---

## 6. Design decisions (provisional — confirmed/cut by §5 + §7 review)

- **Schema = categorical + 1-line why** (§2), not raw integer. Robustness over false precision.
- **Bounds:** `THINK_BUDGET_MAX = 4096` (env `HARNESS_THINK_BUDGET_MAX`), table `none/light/moderate/deep =
  0/1024/2560/4096`. All generous (F3). Clamp at the harness, triage only advises.
- **Cadence (v1): per-turn triage on *generative* turns only.** A turn whose previous step was a tool result that the
  model must now act on is generative; skip triage (use `none`/today) on pure ingestion if cheaply detectable. Start
  with the simplest correct thing and let Probe 3 refine. **Do not over-build cadence before the number.**
- **Request shape:** replace `think: bool` with **`think_budget: Option<u32>`** (`None` → `enable_thinking:false`;
  `Some(0)` → also off; `Some(n)` → `enable_thinking:true, thinking_budget:n`). Extend the already-quarantined
  `request_to_body` mapping (openai.rs:65) + its unit test; anthropic.rs keeps ignoring it. One field, format-agnostic.
- **Triage as a provider call, not a new abstraction:** Phase A is just another `complete()` with a schema and
  `think_budget:None` — no new trait, no router object (no second implementation yet → no ABC, per engineering style).
- **Fail-safe + loud:** triage parse-fail → default `none` **and** emit `[triage]` signal (F-g). Budget exhaustion (if
  Probe 1 says it's detectable) → `[think] budget hit` signal + one-rung escalation-retry.
- **Reactive escalation stays in the design as the fallback arm**, not deleted — if Probe 2 picks (C), it's already
  specced.

## 7. Adversarial review — folded in (4 cold, decorrelated lenses)

Four reviewers ran the four §6-named lenses cold. **The verdict is convergent and strong: cut the predictive triage.**
Two said CUT/ship-the-baseline outright; two said REVISE-with-blockers, and both REVISE paths *also* land on "ship the
fixed-budget baseline, hold the triage." No reviewer defended building Phase-A as designed.

**(i) silent-quality-loss → REVISE.** The triage as designed is "unproven and plausibly negative." Must-fix:
- **Probe 1 is a BLOCKER on F-a, not an open question.** If oMLX spills an exhausted thinking budget silently into
  `content` (F3), F-a has *no in-band catch* and the whole mechanism's central mitigation is gone. Run Probe 1 before
  anything.
- The F-g fail-safe (**default to `none` on triage parse-fail**) is **hardness-selective**: a parse failure correlates
  with task difficulty (harder prompt → flakier structured output), so the fail-safe systematically strips thinking from
  exactly the turns that needed it most. Flipping the default off `none` (to a generous rung) is safer, but that just
  re-exposes that the triage adds risk without proven upside.
- Probe 2 needs a **budget=0 / plain think-OFF arm** to isolate whether *any* of the gain is from thinking at all vs
  from the triage's routing.

**(ii) predictive-router-validity → CUT.** The sharpest verdict: the triage is **unsound under its own objective**.
- Under a *quality* objective (Decision 3: assume capable, maximize quality), routing a task *down* to a smaller budget
  is **pure downside risk with no upside** versus simply giving every turn the ceiling budget. The only thing triage can
  do that a fixed ceiling can't is *spend less* — which is a cost win, and cost is explicitly no longer the objective.
- §3.2's "effort-estimation is a different problem than failure-prediction" is graded a **rationalization**, not a
  reconciliation — both ask the model to predict its own future behavior, which research/23 §7 already measured it can't
  do reliably.
- Probe 2's decision rule ("beats A by a margin that justifies the round-trip") is **unfalsifiable as written** (no
  pre-registered margin) and the regex/Thompson-NFA battery is **not representative** of real ticket work.
- Ship: **arm A at the ceiling (4096) + arm C reactive escalation.**

**(iii) Wu-Wei / cost → REVISE → ship arm A.** Independently lands on the same place from the simplicity axis: the
triage is speculative complexity (a schema, a class table, a cadence rule, a second round-trip per turn) bolted on
*before the number that would justify it exists*. Recommendation:
- Ship a **fixed `think_budget = 2560` on every generative turn**, clamp `[0, 4096]`, lean on the downstream
  `tests_green` gate as the catch (exactly the F5 backstop already in place).
- **Cut** the triage, the JSON schema, the class table, and the cadence machinery.
- Hold **arm C (reactive escalation)** as the *only* sanctioned next step, and only *if* fixed-2560 is **observed** to
  fail (not speculatively).
- Keep Probe 1 (standalone, cheap) and keep the `think_budget: Option<u32>` field — those survive regardless.

**(iv) wire-correctness → REVISE (hard blockers before any code).** The design rests on wire facts we have *asserted but
never shown*:
- **BLOCKER — the `thinking_budget` JSON path is unverified.** §2/F2 assume a flat top-level `thinking_budget:N`, but it
  may need to be nested inside `chat_template_kwargs` (where `enable_thinking` lives). A wrong guess = the field is
  silently dropped = think-ON with no bound = the **8091-token / 830 s runaway (F1)**. Must be probed before the field is
  added.
- **No in-band exhaustion detection exists.** `StopReason` and `Response` (lib.rs) have no variant/field that can
  represent "thinking budget hit" — so even if Probe 1 shows oMLX flags it, the provider layer currently *can't carry
  the signal up*. F-a's escalate-on-exhaustion mitigation is un-implementable without a `Response`/`StopReason` change.
- **Triage must omit tools.** `request_to_body` force-sets `tool_choice:"auto"` whenever tools are non-empty — a triage
  call that inherits the ticket's tools would be pushed toward calling one instead of returning the schema. Phase-A
  would need a tool-less request path.
- **`enable_thinking:false` + `response_format` have never been probed together** — the triage assumes they compose.
- **Drop `Some(0)`; enforce `n ≥ 1`.** A `Some(0)` budget is an ambiguous third "off" encoding alongside `None`; collapse
  off-ness to `None` only.

**Convergence:** all four independently reduce to the same shape — *the irreducible, defensible core is a fixed generous
bounded budget; the triage is unproven complexity that must earn its keep and currently can't.* This is research/23's
"router stays shelved" reaching the same conclusion from the opposite (quality, not cost) objective.

## 8. Recommendation → wiki

**Cut the predictive triage. Ship the fixed-budget baseline. Reactive escalation is the only sanctioned escalation, and
only on observed failure.** Concretely, in build order:

1. **Probe 1 first — cheap throwaway oMLX exercise, BLOCKS all code.** Answer the wire questions before touching the
   crate: (a) flat `thinking_budget:N` vs nested in `chat_template_kwargs` — which one actually bounds reasoning? (b)
   what does the response look like when the budget is exhausted — `stop_reason==length`? a flag? silent spill into
   `content`? (c) does `enable_thinking:true` + `thinking_budget:N` compose with a real tool-calling request? Record the
   answers in research/23's verified-knobs ledger.
2. **Wire change (irreducible): `think: bool` → `think_budget: Option<u32>`** on `provider::Request` (`None` →
   `enable_thinking:false`; `Some(n≥1)` → `enable_thinking:true` + budget on the path Probe 1 proved). Drop `Some(0)`.
   Extend the quarantined `request_to_body` mapping (openai.rs:65) + its unit test; anthropic.rs keeps ignoring it.
3. **Default behavior: fixed `think_budget = Some(2560)` on generative turns**, clamp `[1, HARNESS_THINK_BUDGET_MAX=4096]`,
   `None` on pure tool-ingestion turns if cheaply detectable. Downstream `tests_green` gate is the catch (F5). No triage,
   no schema, no class table, no cadence machinery.
4. **Reactive escalation (arm C) — specced, built only on observed failure.** If fixed-2560 is *measured* to leave
   silent near-misses (the F5 ~30% bar), add escalation keyed on the **universal** signals already in hand
   (`stop_reason==Length` / loopgate strike / gate red), re-running the turn at a higher bounded budget. This reuses
   `classify_stop`/loopgate — no new predictor. Requires the `Response`/`StopReason` exhaustion-signal change from §7(iv)
   if Probe 1 says exhaustion is detectable.
5. **Triage (Gary's Phase-A) — REJECTED for build, recorded as a rejected approach.** It is downside-only under a quality
   objective, its fail-safe is hardness-selective, and effort-prediction is the same unreliable self-prediction
   research/23 already measured. Decision 3's *intent* (bounded thinking, on) is honored by step 3; the *specific
   mechanism* (predictive self-assessed budget) does not survive review.

**Integrity note:** this overturns the literal text of Decision 3 ("mechanism = a think-OFF JSON-schema pre-step"). That
is a deliberate, adv-review-driven mechanism change, not a silent drop — Decision 3's *goal* (let it think, bounded) is
preserved and strengthened; only the *router* is cut. This must go to Gary as an explicit decision change ("Wiki first,
then align on build"), not be enacted unilaterally.

**Owed wiki updates** (on Gary's sign-off): `decisions.md` — amend Decision 3 to the fixed-budget mechanism + add
"predictive thinking-budget triage — REJECTED" to Rejected Approaches; `active-work.md` breadcrumb; `log.md` session
entry; `index.md` research/26 line.

## 9. Probe 1 results — root cause is a server config flag, not the model (2026-06-10)

> **Correction (2026-06-10, after Gary's review).** An earlier draft of this section claimed the budget knob "does not
> exist on the current build" and invented a "model got swapped (`8bit`→`oQ8-fp16-mtp`) which broke the knob" story to
> explain the probe. **Both were wrong.** The model was not swapped. The probe *observations* (budget did nothing;
> `reasoning_content` empty) were real, but I had **not read oMLX's config or code** — I black-box-guessed the wire and
> over-generalized a misconfigured-server result into a property of the model. The real cause was two server-side
> feature flags in `~/.omlx/model_settings.json`. This is the corrected record.

**What actually gates the lever — `~/.omlx/model_settings.json`, per-model:**
```jsonc
"Qwen3.6-35B-A3B-oQ8-fp16-mtp": {
  "enable_thinking": true,
  "thinking_budget_enabled": false,   // ← THE smoking gun: thinking_budget:N is IGNORED until this is true
  "reasoning_parser": "qwen_3_5",     // ← controls whether thinking is split into reasoning_content
  ...
}
```
The per-request `thinking_budget:N` field is only enforced when **`thinking_budget_enabled: true`** for that model.
Reasoning is only split into the `reasoning_content` field when **`reasoning_parser`** is set. These are oMLX server
config (dashboard / `model_settings.json`), not per-request wire knobs — which is exactly why no amount of request-body
fiddling moved them.

**Probe timeline:**
- **Probe 1 (T0–T6), reasoning_parser was effectively inactive + `thinking_budget_enabled:false`:** budget did nothing
  (flat & nested, budget=64 ≡ no-budget, both ran the full 4000 tok) and `reasoning_content` came back empty (thinking
  landed inline in `content`). *Real observations — but explained by the two flags, not by the model.*
- **Probe 2, after Gary set `reasoning_parser: "qwen_3_5"`:** think-ON on "What is 17×23? think step by step" →
  `reasoning_content` = 1641 chars ("Here's a thinking process…"), `content` = 264 chars (clean answer), `finish:stop`,
  745 completion tokens. **The reasoning channel now separates correctly.** ✅
- **Still pending:** `thinking_budget_enabled` is **still `false`**, so `thinking_budget:N` enforcement is unverified.
  Flip it to `true` (dashboard), then re-probe to confirm a budget actually bounds `reasoning_content`.

**Corrected findings:**
1. **The budget lever is REAL and present** — it's gated behind `thinking_budget_enabled` (a config flag), not absent.
   My earlier "not implementable / lever doesn't exist" verdict is **retracted.**
2. **`reasoning_parser` controls the separated channel** — with it on, thinking goes to `reasoning_content` and `content`
   stays clean (so think-ON is *not* inherently unsafe in the loop, contrary to the earlier draft — the inline-ramble
   problem was the parser being off).
3. **No model swap happened.** The `…-8bit` label in CLAUDE.md is a stale doc alias; the served model is the same one.
   The verified-knobs lesson still stands but is re-framed: **the knobs depend on `model_settings.json` flags, and our
   notes didn't capture those flags** — read the config, don't guess the wire.

**Method lesson (the real takeaway):** I should have read `~/.omlx/settings.json` + `model_settings.json` (and ideally
the oMLX request-handling code) **before** probing. Black-box guessing produced a confident, wrong, fabricated-causality
conclusion. Config-first, then probe to confirm.

**Revised verdict (supersedes §7/§8's "cut + fixed-budget-is-also-dead"):**
- **Bounded-thinking IS implementable here.** Prereq: Gary sets `thinking_budget_enabled: true` for the model
  (dashboard). Then re-probe: does `thinking_budget:N` bound `reasoning_content`? what's `finish_reason`/usage on
  exhaustion? (the original Probe 1 questions, now answerable).
- **§7's review verdict still holds on its own terms** (the *predictive self-estimating triage* is downside-only under a
  quality objective — cut it). But the fixed-budget baseline is **not** dead; it was only "dead" under the
  flag-off misreading.
- **Gary's preset-ladder is now the live design to spec** (caller/harness-chosen `low/med/high = 1024/2048/4096`, not
  model-self-estimated). Distinct from the cut triage; dodges the "can a 35B predict its own effort" problem. **Spec
  together** (Gary: "maybe we should have spec this together more clearly") once `thinking_budget_enabled` is on and the
  re-probe confirms the bound.
- **Action items:** (a) Gary flips `thinking_budget_enabled: true`; (b) re-probe budget enforcement + exhaustion
  signal; (c) then spec the preset ladder + the `Request` wire change together; (d) fix the stale `…-8bit` alias in
  CLAUDE.md and record the two flags in research/23's ledger.

### 9.1 Budget verified after the flag flip (2026-06-10) — Probe 3 + Probe 4

Gary flipped `thinking_budget_enabled: true`; the running server picked it up **live (no restart)**. Re-probed flat
top-level `thinking_budget:N`:

- **Probe 3 (verbose domino prompt, max_tokens=4000):** reasoning scaled with budget — 256→622 chars (~256 tok),
  1024→2221 chars (~1024 tok), no-budget→0 (defaults to no-think). The cap bit; then the *answer* ran long and hit
  max_tokens (finish=length on content, not on reasoning).
- **Probe 4 (bat-and-ball, short required answer, max_tokens=8000):** budget=128 → ~720 reasoning tok (**ran *over*
  budget**), content "$0.05" ✓, finish=stop, 18s; budget=2048 → ~846 reasoning tok (finished *under* budget), "$0.05"
  ✓, finish=stop, 20s.

**Verified findings:**
1. **`thinking_budget:N` (flat top-level) now has a real effect** — confirmed against the flag-off baseline where it
   was inert. The wire path is **flat top-level**, not nested.
2. **It's a SOFT target, not a token-exact hard cap.** A verbose prompt is capped near N; a low budget on a
   naturally-longer reasoning (128 → ~720 tok) is **exceeded**, with the model wrapping up *gracefully* rather than
   hard-truncating mid-thought. Adequate to bias effort up/down per task; not a precise throttle.
3. **No explicit exhaustion signal.** Reasoning ends → model emits the answer → `finish_reason: stop`. **No new
   `StopReason` variant needed** — it degrades into a normal completion. (Resolves §7(iv)'s "no in-band exhaustion
   detection" concern: there's nothing to detect; it's graceful by construction.)
4. **Bounded think-ON is loop-safe** — reasoning then a clean answer, fast, natural stop. The earlier "think-ON is
   unsafe in the loop" verdict was an artifact of parser-off + no-budget, **retracted**.
5. **`max_tokens` is the hard backstop**; `thinking_budget` shapes only the reasoning portion. Total output ≤ max_tokens
   always.

**Design consequence — Gary's preset ladder is the live, simple build:** harness picks `low/med/high =
1024/2048/4096` as **soft** `thinking_budget` targets, `max_tokens` as the hard ceiling, **no exhaustion-signal
plumbing, no triage, no new StopReason.** Wire change reduces to `think: bool → think_budget: Option<u32>` (`None` →
`enable_thinking:false`; `Some(n)` → `enable_thinking:true` + flat `thinking_budget:n`). Open design question to spec
together: **who picks the rung** (fixed per-turn default vs caller/ticket-tagged vs reactive escalation) — that's the
real remaining decision, not the wire.

## 10. Implementation (2026-06-10) — wire change shipped

The wire change is in. Three edit sites, ~40 lines, all green (90 tests pass, clippy clean, live oMLX tool-call smoke
green). Subagent fan-out was Wu-Wei-unnecessary for a change this small and this well-specified (same precedent as the
Harden gate in decisions.md) — coordinated solo.

**What landed:**
- `provider::Request`: `think: bool` → `think_budget: Option<u32>` (`crates/provider/src/lib.rs`). Doc comment rewritten
  to the verified soft-budget semantics (no exhaustion signal, `max_tokens` backstop, requires the two server flags).
- `openai.rs request_to_body`: `None` → `chat_template_kwargs.enable_thinking:false`, no budget key; `Some(n)` →
  `enable_thinking:true` **+ flat top-level `thinking_budget:n`** (the §9.1-verified path). Unit test
  `body_carries_thinking_budget` asserts both mappings (absent key when off; flat `2048` when on).
- `anthropic.rs`: unchanged — it never referenced the field; compiles clean against the rename.
- `agent/src/main.rs`: new `DEFAULT_THINK_BUDGET: u32 = 2048` (medium rung); the loop reads
  `HARNESS_THINK_BUDGET` (via the existing `env_or`), maps `0 → None` else `Some(n)`, computed once per run and passed
  every turn. Recorded in the run's `sampling` telemetry JSON (`"think_budget":N|null`) for provenance.

**Resolved — "who picks the rung": fixed per-turn default, env-overridable.** The run picks one rung at the loop
boundary (`DEFAULT_THINK_BUDGET`, override `HARNESS_THINK_BUDGET`) and uses it for every turn. Rationale: it's the
simplest thing that ships the settled posture (reasoning ON-but-bounded, medium=2048), it's reconfigurable without a
recompile, and it commits to nothing that blocks the richer options. The two documented follow-ons stay open and are
*additive*, not rewrites:
  - **caller/ticket-tagged rung** — let a ticket carry its own budget (a `harden` ticket asks for high, an `ingest`
    ticket for 0/off). Needs a board-schema field; defer until a ticket kind actually wants a different rung.
  - **reactive escalation** — bump the rung on Length/loop/gate-red within a run. Deferred deliberately: it couples
    thinking to the loop/plan gates, and we want to observe fixed-rung behavior in telemetry first before adding a
    feedback loop. (This is the *escalation* trigger from research/23, now budget-shaped instead of bool-shaped.)

**Behavior change to flag to Gary:** the committed default flips from think-OFF (research/23) to bounded think-ON at
medium=2048. This is the intended posture shift (Decision 3), settled by the §7 adv review + §9.1 verification — not a
silent default change. `HARNESS_THINK_BUDGET=0` restores the old think-OFF behavior for any run.
