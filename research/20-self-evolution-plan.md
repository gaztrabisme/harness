# research/20 — Self-evolution pillar: implementation plan (P5 / design-v2 §9)

> Planning doc for the **last** pillar. Grounded in `research/04-self-evolution.md` (landscape +
> recommendation) and design-v2 §9 (constitution vs case-law). Process gate: self-evolution requires
> **adversarial review at design AND pre-land** (highest-risk pillar — an agent editing its own behavior is
> the canonical gaming/drift surface). §9 below is the **review fold-in** (added after §1–§8 are reviewed).

## 0. The honest problem statement

The compounding loop is **open**. The harness already *writes* `lesson` episodes (explore's reflection-on-
kill at `main.rs:529`; the memory plane `remember`/`recall` on rusqlite+FTS5) — but **nothing reads them
back into behavior**. `build_system_prompt` (`main.rs:605`) is built purely from the ticket workpad; no
lesson, no heuristic, no case-law is ever injected. So lessons accrete in a table and die there.

Self-evolution = **closing that loop safely**. "Safely" is the whole problem: an agent that edits the
surface governing its own behavior is exactly where drift and metric-gaming come from (Gödel Agent's
"error accumulation"; the user's own stated complaint that the current skill over-fits its lessons).

## 1. What the design (research/04 + §9) actually asks for

- **Constitution (immutable, human-only):** the spine, integrity constraints, Wu-Wei filter, gate-
  enforcement logic, **and the eval metric / definition of "done"**. Litmus: *if the agent could weaken
  safety/integrity by editing it, it's constitution.* Written as explanation, not a rule-list.
- **Case-law (agent-proposes, human-approves):** heuristics, mode thresholds, subagent prompts, learned
  lessons — the markdown/config surface the loop reads.
- **Amendment pipeline:** reflection emits a case-law *diff* → **self-critique gate** (auto-reject if it
  contradicts the constitution or touches a gate) → **grounding gate** (cite ≥2 episodes; 1 = candidate)
  → **regression gate** (no regression on a frozen baseline) → **human approval = git commit** (`git
  revert` = rollback). Instrument a **gate-bypass counter**.
- **Generalization filter** (accept only if all four): (a) cross-context transfer; (b) no bound specifics
  (no file/repo/literal); (c) evidence floor ≥2 episodes; (d) methodology form (mindset, not task-instr).
  Implementation = **two-column rewrite** ("specific observation" → "general principle"); commit only the
  general column.
- **Decay/cadence:** token budget on active case-law; confidence × recency × usage; uncorroborated decays
  to candidate then retires (git retains). Per-task reflection + periodic constitutional review.
- **Watch:** the *vacuous-generalization* failure — lessons general enough to pass the filter but carrying
  no signal. Evidence floor is the guard; may need an explicit anti-vacuity check.

**KB grounding gate:** searched `"self-improving agents, reflection, distilling episodes into reusable
lessons, avoiding overfitting and metric gaming"` → **nothing relevant** (KB hits are all RL *training*
overfitting/distillation; self-modifying-**agent** architecture is research-paper territory, already
grounded in `research/04`: Constitutional AI / STOP / Gödel Agent / ADAS / DSPy / Voyager). The KB does
not cover this domain; grounding stands on research/04's literature review.

## 2. The discipline this pillar inherits (cut the cathedral)

The two prior reviewed pillars both got cut from a cathedral to an **evidence-producing, propose-only,
human-gated instrument**: memory shipped Slice A (lexical) not the vector/decay cathedral; parallel-
exploration shipped a **probe** that *reaps/consolidates/lands nothing*. Self-evolution gets the same
treatment — and it's the **most** important place to apply it, because the failure modes are the worst.

**The MVP is propose-only `agent reflect`, mirroring `explore`'s "reaps nothing":**

> `reflect` reads accumulated `lesson` episodes, runs them through a **pure** generalization filter + gate-
> bypass detector, and emits a **proposal artifact** (two-column, cited, verdict'd). It **writes no case-
> law, edits no constitution path, auto-commits nothing.** A human reads the proposal, moves accepted
> general-column lines into `wiki/case-law.md`, and commits. The loop injects the **committed** case-law.

This keeps the constitution/case-law boundary *trivially* honest: the only thing the agent can do is
*emit text for a human to read*. Adoption is a human `git commit`, exactly as the design demands. No new
trust surface, no auto-mutation of behavior.

## 3. The two halves (both small)

**Half A — close the read side (the payoff):** the loop injects the committed case-law file.
- New git-tracked file `wiki/case-law.md` — curated, human-approved **active** lessons (starts ~empty
  with a header explaining the contract). One bullet = one general principle.
- `build_system_prompt` gains a `case_law: &str` arg; if non-empty, append a `### Learned heuristics
  (case-law)` section after the workpad. Read once per run from `wiki/case-law.md` (best-effort: missing
  file → empty → no section, never a hard failure — same degrade-gracefully posture as `workpad_header`).
- That's it. The read side is ~15 lines + a test. **It is the entire point** — without it, lessons never
  change behavior. Build it first so the loop *can* compound the moment a human approves one lesson.

**Half B — the propose side (`agent reflect`, the instrument):**
- `agent reflect [--scope SC] [--project P] [--since N]` →
  1. **gather**: `recall`-style pull of `lesson` episodes from the memory plane (reuse the existing
     query path; no new storage).
  2. **filter (PURE, no oMLX):** each candidate through `evolve::screen` (§4) — the mechanically-checkable
     parts of the generalization filter + gate-bypass detector + anti-vacuity. Produces a `Verdict`
     (`Accept | Candidate | Reject`) + machine reasons.
  3. **distill (oMLX, the exercise):** for survivors, one oMLX call does the **two-column rewrite**
     (specific→general) — this is the part a local model is good at and the part that's *exercise-only*.
  4. **emit**: a proposal artifact to **stdout + `wiki/case-law-proposals/<runlabel>.md`** (a proposals
     dir, NOT case-law.md): two columns, citations, confidence, per-candidate verdict + reasons, and a
     **gate-bypass count** banner if any candidate tripped the detector. Human-readable, diff-able.
  5. **never**: write `wiki/case-law.md`, write any constitution path, or `git commit`. (The boundary
     check refuses a proposal whose target path classifies as Constitution — defense in depth even though
     reflect only ever writes to the proposals dir.)

## 4. The pure core — `crates/agent/src/evolve.rs` (the no-oMLX gate)

Same shape as `explore.rs`: value types + pure functions, no git, no model, unit-tested in isolation.

```rust
pub enum Tier { Constitution, CaseLaw }
/// Path → which tier owns it. UNKNOWN paths classify as Constitution (fail-closed:
/// an unrecognized target is treated as off-limits, never as freely-editable case-law).
pub fn classify_path(path: &str) -> Tier;

pub enum Verdict { Accept, Candidate, Reject }
pub struct Screen { pub verdict: Verdict, pub reasons: Vec<String>, pub gate_bypass: bool }

/// The MECHANICALLY-checkable subset of the generalization filter. HONEST about its
/// limits (the explore-style "no fake gate"): it enforces what code can actually
/// decide — (b) no-bound-specifics, (c) evidence-floor, anti-vacuity, gate-bypass —
/// and FLAGS (a cross-context / d methodology-form) as human/oMLX judgment, never
/// fakes a verdict on them.
pub fn screen(text: &str, citations: usize, gate_names: &[&str]) -> Screen;

/// (b) no bound specifics: rejects text naming a path-like token, a repo/literal,
/// an absolute path, a digit-bearing config value (`save_steps=200`), etc.
pub fn has_bound_specifics(text: &str) -> bool;

/// gate-bypass detector: lesson text references skipping/weakening/disabling/bypassing
/// a KNOWN gate (by const name or synonym) → the early gaming signal.
pub fn mentions_gate_weakening(text: &str, gate_names: &[&str]) -> bool;

/// anti-vacuity: too short, no concrete noun, or a pure platitude → carries no signal.
pub fn is_vacuous(text: &str) -> bool;

/// decay/ranking: confidence × recency × usage (integer-stable, deterministic).
pub fn rank_caselaw(items: Vec<CaseLawItem>) -> Vec<CaseLawItem>;
```

**Why these are the right seams:** they are the parts that must be *correct and tamper-proof*, and they're
exactly the parts that don't need a model. The grounding floor (≥2) and the gate-bypass detector are the
two load-bearing safety checks from research/04 — both are pure predicates. The honest line (mirroring
explore §9.7) is that **cross-context transfer (a) and methodology-form (d) are genuinely LLM judgment** —
the pure screen does not pretend to decide them; it surfaces them for the human/oMLX. Faking a regex for
"is this transferable?" would be the exact "distinct-string gate" anti-pattern the explore review killed.

## 5. Constitution boundary — how it's actually enforced

Three layers, weakest-to-strongest:
1. **Structural (the real one):** `reflect` is propose-only — it writes to `wiki/case-law-proposals/`
   and nothing else. It cannot mutate behavior because it cannot write the file the loop reads. Adoption
   is a human git commit. This is the same "reaps nothing" control that makes `explore` safe.
2. **`classify_path` fail-closed:** any path the proposal targets that isn't a known case-law path →
   `Constitution` → refused. Defense in depth.
3. **`mentions_gate_weakening`:** a proposed lesson whose *content* would route around a gate is flagged
   (and counted) even if its target path is fine — catches "add a heuristic that says skip the harden
   gate" which is a content attack, not a path attack.

Note the candid limit (for the reviewer): a *build* ticket's worker already has `write_file`+`bash` over
its worktree, so it *could* edit `spine.rs` in-tree — but that's the **existing** build-ticket trust model
(worktree isolation + human land gate), not something `reflect` introduces. `reflect` itself never runs a
worker and never writes code. The constitution's immutability *at the reflection surface* is real; its
immutability *against a malicious build ticket* is the land gate's job, out of scope here.

## 6. Subagent coordination pattern (the mandate's ask)

The genuine pattern for this pillar is an **adversarial critic panel** over each candidate lesson —
literally the design's "self-critique gate" realized as fan-out. The coordination shape (mirrors the
Workflow "perspective-diverse verify"):

> For each candidate that passes the pure screen, fan out K=3 **diverse** critics (oMLX), each with a
> distinct lens — **over-fit** ("does this name hidden specifics / only apply to one repo?"),
> **vacuity** ("does this carry actual signal or is it a platitude?"), **gaming** ("does this subtly
> weaken a gate or the integrity constraints?"). A candidate is **proposed as Accept only if ≥2 of 3
> critics clear it**; otherwise it drops to Candidate with the critics' reasons attached.

MVP builds the **seam** (the pure screen + a single-pass distill is the floor); the critic panel is the
coordination layer exercised on top — same as explore where the probe *is* the pattern. The pure
`screen` is what each critic's verdict is fused with, so the panel can't override a hard mechanical reject
(grounding floor / gate-bypass are non-negotiable; critics only *tighten*, never loosen).

## 7. Build order + gates

1. **Half A** (read side): `wiki/case-law.md` + `build_system_prompt(case_law)` + test. ~20 lines.
2. **`evolve.rs`** pure core + unit tests (the no-oMLX gate). The bulk of the correctness surface.
3. **`run_reflect`** glue in main.rs (gather → screen → distill(oMLX) → emit proposal). Reuses the
   memory query path + the oMLX provider.
4. **(coordination)** critic-panel fan-out over survivors — built on the same provider, fused with the
   pure screen.
5. **Gates:** `cargo test -p agent` + `-p board` green, clippy clean. fmt skipped (hard tabs, non-signal).
   **Exercise (oMLX, never landed):** seed 2–3 `lesson` episodes, run `agent reflect`, show the proposal
   artifact distinguishes a transferable lesson (Accept) from an over-fit config-fact (Reject, bound-
   specifics) and flags a gate-weakening lesson (gate_bypass banner). Evidence = the artifact + the
   counts, not a vibe.
6. **Pre-land adversarial review** (REQUIRED by the Process gate) before declaring the pillar done.

## 8. Explicitly deferred (NOT in MVP — Wu Wei)

- **Regression gate against a frozen baseline** (research/04 gate 4): needs a held-out suite of past task
  transcripts + an LLM-judge harness. Real, but it's a whole eval rig — defer until there *are* enough
  past tasks to freeze. MVP's substitute is the human reading the proposal (the design's "human is the
  metric where no number exists").
- **Automatic confidence calibration from usage** (usage-without-failure raises confidence): needs the
  case-law items to be *used* and *tracked* first. The schema carries the fields; the auto-bump is later.
- **Periodic constitutional review / promotion case-law→constitution:** a human ritual, not harness code.
- **Auto-application of approved case-law** (reflect writing case-law.md directly behind a gate): the MVP
  deliberately keeps the human git-commit in the loop. Auto-apply is the cathedral; revisit only if the
  propose-only flow proves too slow in practice (Wu-Wei: don't fix a problem we don't have).

---

## 9. Review fold-in (DECISION — supersedes §3–§6 scope)

Three adversarial reviewers (safety/gaming, Wu-Wei/scope, correctness/silent-failure), in parallel,
**converged** on one conclusion from three directions: **Half A is the real, load-bearing, irreversible
change; Half B (`agent reflect` + the pure-filter core + oMLX distill + critic panel) is a cathedral built
over an empty pond.** The decision is to **ship Half A only, hardened**, and **defer Half B entirely**
until a measured trigger. This mirrors memory (Slice A only) and explore (a probe) — the third time the
discipline has cut a pillar to its load-bearing core.

### 9.1 Why Half B is deferred (the Wu-Wei finding, empirically grounded)
- **The pond is empty *and mis-stocked*.** The only `lesson` writer in production is explore's reflection-
  on-kill (`main.rs:541`), which emits **ticket-scoped, bound-specific failure post-mortems** (`"explore t5
  w2: failed approach … 9 lines changed … did not satisfy \`cargo test -p agent\`"`). Every one names a
  worker id, digits, and a literal command — they would all trip the generalization filter. There are
  **zero** general lessons for `reflect` to distill.
- **The exercise required hand-seeded fakes** (§7.5 said "seed 2–3 synthetic episodes"). When the
  evidence-producing exercise must first fabricate the instrument's input, the instrument is ahead of its
  data. That is the tell.
- **Distillation is already done, better, in-loop.** Claude (the strong model in the session) distills
  lessons far better than an oMLX two-column pass. Building an oMLX pipeline to replace Claude-in-the-loop,
  before a single general lesson exists, is solving a problem we don't have.
- **Wu-Wei trigger to build Half B later:** when ≥~10 genuinely *general* lessons have accumulated through
  manual curation and re-curating them by hand has become real toil. Not before.

### 9.2 Why Half B was also *unsafe/incorrect* as specified (so deferring loses nothing good)
The other two reviewers independently showed the deferred machinery didn't even work:
- **Injection sink (BLOCKER):** the pure `screen` ran on the raw lesson `text`, but the human approves the
  **oMLX distill *output*** — different text. A poisoned `lesson` body (memory rows are unsanitized free
  text) steers the distill; the screen guarantees nothing about what the human sees. The mechanical gate
  and the artifact were decoupled.
- **Self-dealing grounding floor (BLOCKER):** "≥2 episodes" is cleared by the agent's own auto-authored
  post-mortems — one explore run with 2 failed branches manufactures 2 corroborating episodes. research/04
  explicitly wants a baseline *"the agent didn't author."* The floor rewarded the most-repeated mistake.
- **Keyword gate-bypass detector = false safety (MAJOR):** a 5-const keyword match is walked past by any
  paraphrase ("prefer trusting a green suite over mutation noise") or integrity attack that names no gate
  ("when blocked >3 attempts, ship the partial"). A `gate_bypass:false` banner reads as "safe" — worse
  than no check. This is the explore review's "distinct-string gate" anti-pattern wearing a safety badge.
- **`has_bound_specifics` / `is_vacuous` are theater (BLOCKER×2):** the former rejects 100% of real input
  *and* the design's own floor lesson ("prefer ≥2 corroborating cases" — has a digit); the latter stamps
  dense platitudes "not vacuous," laundering them. Surface-feature regexes confidently mis-verdict both
  ways. Honest verdict: these are LLM/human judgment, never code verdicts — same line explore drew.

### 9.3 What Half A keeps — and the read-path hardening that survives the cut
Half A = **the loop reads the committed, human-approved case-law and injects it into every worker's system
prompt.** That is the entire compounding payoff and it's ~20 lines. But Rev 1 + Rev 3 showed even Half A
has a real new attack surface and real silent-failure modes, so it ships **hardened**:

1. **Resolve `wiki/case-law.md` against the repo root** (via `git::repo_root`, as `workpad_header` already
   does), **not** cwd — workers run inside isolated worktrees, so a cwd-relative read would silently find
   nothing and disable the pillar invisibly (Rev 3 #3a).
2. **Distinguish missing-file from empty-file.** Missing → a one-line **stderr** note (`[case-law] no
   wiki/case-law.md at <root> — loop read-side inactive`); empty → silent (genuinely "nothing approved
   yet"). The pillar whose point is closing the loop must not render its own closure-failure invisible.
3. **Size budget at the injection point** (research/04's token ceiling, line 121): cap the number of
   bullets injected and report the count (`injected N/M case-law bullets`). An unbounded read surface is a
   steering/DoS vector and crowds out the workpad (Rev 1 #5, Rev 3 #3b).
4. **Read-path screen (defense in depth):** drop any case-law bullet that trips `mentions_gate_weakening`
   *before* injecting it, so a poisoned line that slipped into the file (via a bad approval or a build
   worker that edited the tracked file) can't steer every future worker. This is the **one** pure
   predicate that survives — and only in the **flag/drop, never clear** direction all three reviewers
   endorsed. Renamed honestly: it flags, it never certifies "safe."
5. **Tests pin the contract** (explore discipline): empty → no section; non-empty → section present;
   budget drops the overflow; a planted gate-weakening bullet is dropped.

### 9.4 The minimal build (the whole pillar MVP)
- **`crates/agent/src/evolve.rs`** — a *tiny* pure core, only what the read path needs (NOT the 6-function
  cathedral): `mentions_gate_weakening(text, gate_names) -> bool` (flag/drop-only) + `prepare_caselaw(raw,
  gate_names, max_bullets) -> Prepared { bullets, dropped_unsafe, dropped_budget }` (split into bullets,
  drop unsafe, apply budget — pure, deterministic, fully unit-tested). This is load-bearing *safety* code
  for the read path, not speculative filtering.
- **`build_system_prompt(t, header, case_law)`** — append a `### Learned heuristics (case-law)` section
  iff non-empty.
- **main.rs glue** — read case-law.md from repo root, `prepare_caselaw`, inject; missing-file stderr note.
- **`wiki/case-law.md`** — created with a header explaining the human-approve contract + a few genuinely-
  *general* hand-distilled lessons (Claude-in-loop distillation — the §9.1 point made concrete). This is
  the evidence: a committed lesson visibly changing the next run's prompt.
- **Gates:** `cargo test -p agent` + clippy clean; fmt skipped (hard tabs, non-signal). **Exercise:** show
  a committed case-law lesson appears in the next run's system prompt, and a planted gate-weakening line is
  dropped from it. Evidence = the rendered prompt + the drop count, not a vibe.
- **Pre-land adversarial review** (Process gate) before declaring the pillar done.

### 9.5 Deferred (was Half B; revisit only at the §9.1 trigger)
`agent reflect`, the oMLX two-column distiller, the K=3 critic panel, the proposals dir, and the full
6-function filter core (`classify_path`, `has_bound_specifics`, `is_vacuous`, `rank_caselaw`, the
input-side `screen`). When built, the §9.2 fixes are mandatory: screen the **oMLX output**, require
**distinct-provenance** citations (≥2 different tickets, excluding same-template auto-authored episodes,
emitted as verified episode-id lists per principle), make the gate-bypass detector **flag-only behind the
critic panel** with a **persisted cumulative counter** (the trend signal research/04 wants), and shape the
artifact to **lead with the raw cited episodes**, not a pre-stamped verdict. Also: read bodies via a
**non-mutating** path (`recall_body` bumps `usage_count` — a determinism trap), and pass `now` into any
ranker rather than reading the clock.
