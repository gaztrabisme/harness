# Track 3: Test Hardening & Mutation

## Scope & what it grounds

Problem we're grounding: the user stopped reviewing AI-written tests, so test quality silently collapsed — tests now "agree with" whatever the implementer model produced rather than constraining it. A downstream "adversarial review" agent is theater: it's another LLM opinion, not a mechanical check, and it runs *after* the code is written (too late, and easily gamed).

Goal: make test quality a **number the harness computes**, not a judgment a human (or a judge-LLM) renders. The number is **mutation score** — generate broken implementations (mutants), and if the tests don't kill them, the tests are demonstrably weak and must be regenerated **before** real implementation begins. This converts "are these tests good?" from an opinion into a falsifiable measurement.

This track answers: (1) what code gets mutated vs a cheaper gate, (2) how mutation folds into Test→Harden→Implement→Verify automatically, (3) what replaces mutation for non-deterministic/ML work, (4) how author-vs-mutator model diversity decorrelates blind spots.

Boundary with other tracks: the *orchestration* of these gates (when they run, retry budgets) belongs to the spine track; here we specify the test-quality mechanics themselves.

---

## Landscape

### Mutation testing tooling (rule-based)

Mutation testing measures test-suite *adequacy*: inject a small change (a "mutant") into source, run the tests, and check whether some test fails ("kills" the mutant). Coverage asks "did this line run?"; mutation asks "would a bug here be caught?" — a test can have 100% line coverage while asserting nothing. `Mutation Score = killed / total_non-equivalent × 100`. Mutants bucket into killed / survived / timeout / suspicious.

- **mutmut** (Python) — simplest, most actively maintained. ~6 operators (arithmetic `+`→`-`, comparison `>`→`>=`, logical `and`→`or`, constant tweaks, statement deletion). Auto-discovers source/tests, caches in `.mutmut-cache`, diff-style `mutmut show <id>` / `mutmut apply <id>`. Whitelisting is line-by-line via `# pragma: no mutate`. https://github.com/boxed/mutmut · intro: https://opensource.com/article/20/7/mutmut-python
- **cosmic-ray** (Python) — ~9 operators, fine-grained `exclude-operators`, **native distributed/concurrent execution** (set `distributor` from `local` to `http`, fan workers across URLs; mutation is "embarrassingly parallel"). Heavier setup (SQLite DB of mutants), advised to run *infrequently* after changes accumulate. https://cosmic-ray.readthedocs.io/
- **Stryker** (JS/TS, also C#/Scala) — best-in-class scoping. `--incremental` persists `reports/stryker-incremental.json`, diffs code/tests (Google diff-match-patch) and reuses stable mutant results → "30 min to under 2 min on typical PRs." Line-range targeting: `stryker run --mutate bar.js:20-40`; `--only-undetected` reruns just survivors. Caveat: incremental drifts over time → periodic full `--force` run. https://stryker-mutator.io/docs/stryker-js/incremental/
- **PIT / pitest** (JVM) — pioneered incremental mutation (`withHistory` hashes classes to skip unchanged). `scmMutationCoverage` with `-Dinclude=ADDED,MODIFIED` mutates only changed code on PRs; persist `pit-history.xml` as a CI artifact. Consensus on thresholds: **start low, ratchet up** — "coach, not gatekeeper" initially. https://pitest.org/ · https://github.com/hcoles/pitest

Universal caveat across all four: **equivalent mutants** (a mutant that's semantically identical, e.g. `age >= 18` → `age > 17`) can never be killed, so 100% is impossible — aim 80–90%.

### LLM-as-mutator research (~2023–2026)

Motivation: rule-based operators are cheap but unrealistic; LLMs generate mutants that resemble *real* bugs (the "near-neighborhood" property — syntactically and semantically close to the original).

- **"On the Use of LLMs in Mutation Testing"** (TOSEM, arXiv 2406.09843) — the comprehensive study. Honest trade-off: LLM mutants have **worse compilability, more useless mutants, and more equivalent mutants** than rule-based, BUT higher behavioral similarity to real bugs and AST diversity. https://arxiv.org/html/2406.09843v2
- **LLMorpheus** (Tip et al., IEEE TSE 2025) — inserts placeholders at AST nodes and prompts an LLM to fill them. https://arxiv.org/abs/2406.09843 (cited therein)
- **µBERT** — masks one token, predicts replacement (masked-LM mutation).
- **Meta ACH — "Mutation-Guided LLM-based Test Generation"** (arXiv 2501.12862) — **the most directly transferable design.** Reverses mutation testing: instead of many mutants to *score* tests, generate *few targeted* mutants representing undetected faults, then generate tests that **kill them → harden the code**. Three Llama-3.1-70B agents: (1) make-a-fault, (2) **equivalence detector** (LLM-as-judge: "will mutant and original always do exactly the same thing?"), (3) make-a-test-that-fails-on-mutant-passes-on-original. Guarantees per test: buildable, non-flaky, hardening. Result: 73% engineer acceptance; **49% of accepted tests added zero line coverage yet caught faults** — i.e. coverage-only gating would have discarded half the valuable tests. Equivalence detector hit precision 0.95 / recall 0.96 after stripping inserted comments. https://arxiv.org/html/2501.12862v1
- **PRIMG** (arXiv 2505.05584) — mutant *prioritization* to cut LLM test-gen cost. https://arxiv.org/pdf/2505.05584

### Property-based testing (PBT)

Define *invariants* that hold for all inputs; the library generates hundreds of cases, and on failure **shrinks** to a minimal counterexample.

- **Hypothesis** (Python) — "strategies" generate data; integrates with pytest/unittest. Classic property patterns: round-trip (`decode(encode(x)) == x`), idempotence, **differential** (compare against a known-good reference impl), and **metamorphic** (relate two runs). `RuleBasedStateMachine` with `@rule`/`@invariant` does **stateful testing** — generates operation *sequences* and shrinks both inputs and order. https://hypothesis.readthedocs.io/
- **fast-check** (JS/TS) — same model for the JS ecosystem.
- Track record: QuickCheck-style PBT found 200+ bugs in Erlang telecom systems that example tests missed.

### Metamorphic testing (for systems with no oracle)

When you can't compute the correct output (ML, search, simulations, LLMs), you may still know **relations between outputs of related inputs** — Metamorphic Relations (MRs). Violations = bugs, no ground truth needed.

- Classic: `sin(x) == sin(π−x)`; `f(x)==f(−x)` for `x²`.
- ML/CV: rotate/recolor/blur an image → label should be invariant (DeepTest collapsed autonomous-driving vision under rain/tilt MRs). Clustering: shuffle input order → same clusters (METTLE found ~5% of k-means runs had 20% error under shuffling). https://www.lakera.ai/blog/metamorphic-relations-guide
- LLMs: MRs capture semantic invariants across paraphrases; self-consistency (inverted query → inverted answer). METAL, Drowzee (hallucination), MTF framework. https://arxiv.org/html/2511.02108v1
- Key practitioner finding: **some MRs transfer across tasks** — a small reusable MR library is viable.

---

## Transferable techniques (→ how each folds into our Verify gate)

1. **Mutation score as the test-quality gate (rule-based, scoped).**
   What: run mutmut/cosmic-ray (Py) or Stryker (JS) on the just-written code under test; compute killed/total.
   Folds in: this is the *hardening* step between Test-authoring and Implement. Run mutation against a **reference/stub or the first implementation** of the unit; surviving mutants = concrete proof of a missing assertion. The harness feeds each survivor's diff back to the test author: "this broken version passed your tests — add an assertion that catches it." Loop until score ≥ threshold, *then* unlock real implementation.

2. **Mutation-guided hardening, LLM flavor (Meta ACH pattern).**
   What: a separate **mutator agent** writes a handful of realistic broken implementations; the **test author** must produce tests that fail-on-mutant / pass-on-original.
   Folds in: this is the decorrelation lever — mutator and author on **different providers**. The ACH loop *is* our harden gate, minus Meta's privacy framing: make-fault → equivalence-check → make-killing-test → terminate when mutant is killed. Critically, ACH proves **49% of fault-catching tests add no coverage** → our gate must be mutation-driven, not coverage-driven.

3. **LLM equivalence detector (so the gate doesn't chase un-killable mutants).**
   What: before demanding a mutant be killed, an LLM-judge asks "is this mutant behaviorally identical to the original?" Strip inserted comments first.
   Folds in: prevents infinite harden loops on equivalent mutants (the thing that makes 100% impossible). If flagged equivalent → discard the mutant, don't penalize the tests. Precision/recall 0.95/0.96 is good enough to automate.

4. **Property-based tests as a force-multiplier on assertions.**
   What: where an invariant exists (round-trip, idempotence, ordering, conservation), write a Hypothesis/fast-check property instead of (or alongside) examples.
   Folds in: properties are *much* harder to "shape to pass" than example tests — the model can't hardcode the expected output because inputs are generated. The harden gate should **prefer a property when a known invariant pattern is detectable**, and mutation score on property-backed code tends to be high "for free." Stateful machines cover API/CRUD/ledger logic.

5. **Metamorphic relations as the oracle-free fallback.**
   What: assert relations across paired runs (transform input → predicted output relation), not absolute outputs.
   Folds in: this is the **ML/non-deterministic substitute** for mutation. A small reusable MR library (invariance under benign transforms, monotonicity, symmetry, self-consistency) becomes the Verify gate where mutation can't run.

6. **Incremental / changed-lines scoping (cost control).**
   What: Stryker `--incremental` + line-range `--mutate file:start-end`; PIT `scmMutationCoverage` ADDED,MODIFIED + history artifact.
   Folds in: the gate mutates **only the diff under test**, not the repo. Persist the incremental report between harness runs so re-verification is seconds, not minutes.

---

## Anti-patterns / what to avoid

- **Adversarial-review theater.** A downstream judge-LLM "reviewing" tests is an opinion, runs too late, and is gameable. Replace with a *number* (mutation score) computed *before* implementation. This is the whole point of the track.
- **Coverage as a quality gate.** 100% line coverage with zero meaningful assertions is trivially achievable and is exactly what a lazy/agreeable test suite produces. ACH data: half the fault-catching tests add no coverage — coverage-gating discards them. Coverage is at best a cheap *pre-filter*, never the bar.
- **Same model writes tests + code + mutants.** Correlated blind spots: the model misses the same edge case in all three, so the mutant is one it wouldn't write, the test is one it wouldn't think to add, and "all green" proves only internal consistency. Decorrelation (different providers for author vs mutator) is mandatory, not optional.
- **Tests shaped to pass.** When the implementer can see/influence the tests, it writes the minimum to go green. Mitigate: freeze tests before implementation (test-first), and harden them against mutants the *implementer didn't generate*.
- **Chasing 100% mutation score.** Equivalent mutants make it impossible; you'll burn budget and erode trust. Gate at 80–90% on scoped critical code; use the equivalence detector to exclude un-killable mutants from the denominator.
- **Whole-repo mutation every run.** 10–100× slower than the test suite; minutes-to-hours. Always scope to changed lines + incremental cache. Reserve full runs for periodic drift-correction.
- **Mutating trivial code.** Getters/setters, config, generated code, boilerplate produce noise. Scope to logic that handles money/security/business rules.
- **Flaky tests under mutation.** Non-determinism wreaks havoc (a "survivor" may be a flake). Stabilize or quarantine flaky tests before they enter the mutation gate; for genuinely non-deterministic units, switch to the metamorphic/property fallback instead.

---

## Recommendation for our design

### The integrated test-hardening loop

Insert a **Harden** stage between Test and Implement, gated by a number:

```
1. SPEC        → author understands the unit's contract
2. TEST        → Test-author model (Provider A) writes tests FIRST
                 (examples + properties where an invariant pattern is detectable)
3. HARDEN      ← the new gate, runs BEFORE real implementation:
   a. Generate a reference/stub or trivial first impl of the unit
   b. MUTATE the unit (scoped to changed lines):
        - rule-based (mutmut/Stryker) for cheap broad coverage of operators
        - + LLM-mutator (Provider B, ACH-style) for realistic semantic faults
   c. Equivalence-check each mutant (LLM-judge, comments stripped) → drop equivalents
   d. Run the frozen tests against each surviving mutant
   e. mutation_score = killed / (total − equivalent)
   f. IF score < threshold: feed survivor diffs back to Provider A
        → "this broken version passed; add assertions to kill it" → regenerate tests → goto (d)
        (cap retries; on repeated failure, escalate to human — this is the ONE place human review earns its keep)
   g. ELSE: freeze tests, proceed
4. IMPLEMENT   → Implementer model writes real code against frozen, hardened tests
5. VERIFY      → run hardened tests; optionally re-mutate the real impl to confirm score held
```

Why this kills the theater: the gate is mechanical, runs *before* implementation (so tests can't be shaped to the code), and the bar is a number, not a vote.

### Mutation scope + threshold

- **Tiered scope** (cost control):
  - **Full mutation** (rule-based + LLM-mutator) on: core business logic, money/security/auth, data-integrity code, anything in the diff flagged "critical."
  - **Lighter gate** elsewhere: coverage-gap check + rule-based mutation on changed lines only; skip getters/config/boilerplate.
  - **Always scope to the diff**: Stryker `--incremental` + line ranges; PIT `scmMutationCoverage ADDED,MODIFIED`; mutmut on changed files. Persist the incremental report across runs.
- **Threshold**: gate at **≥ 85% mutation score** on critical code (after excluding LLM-judged equivalents), **≥ 70%** as a soft warn elsewhere. Ratchet up over time ("coach not gatekeeper" at first). Never require 100%.
- **Timeouts**: kill any mutant run that exceeds ~10× baseline test time (infinite-loop guard, mutmut's default behavior).

### The ML / non-deterministic fallback

Mutation testing assumes fast, deterministic code. When the unit is an experiment, a model, or otherwise non-deterministic, **swap the Harden gate's oracle**:

- **Metamorphic relations** become the gate. Maintain a small reusable MR library: invariance under benign transforms (reorder, rescale, paraphrase, rotate/recolor for vision), monotonicity, symmetry, conservation, self-consistency (inverted query → inverted answer). The gate passes if MRs hold across paired runs within tolerance.
- **Property invariants** where they exist (bounds, shape, type, conservation laws, "loss decreases," "probabilities sum to 1").
- **Criteria-coverage adversary**: instead of mutating code, the mutator model perturbs *inputs/conditions* to find MR violations or invariant breaks — the non-deterministic analog of a surviving mutant.
- **Flakiness budget**: because even temperature-0 LLMs are non-deterministic, re-run failing metamorphic groups N times and gate on violation *rate*, not a single failure.

### Model-diversity setup (decorrelating blind spots)

The mechanism: correlated errors come from shared training data + architecture. Running more of the *same* model produces artificial consensus, not independent checking. Concretely:

- **Test author = Provider A** (e.g. Claude/Anthropic). **Mutator + equivalence-judge = Provider B** (e.g. a GPT/Gemini/Llama family — genuinely different pretraining). **Implementer = a third assignment** (can reuse A or a third provider, but must NOT be the test author's twin on the same prompt).
- Why it works here specifically: a mutant is a fault the **mutator** thought of. If author and mutator share blind spots, the mutator generates only faults the author already tested for → easy 100%, false confidence. Different-provider mutator generates faults *outside* the author's blind spot → survivors expose genuinely missing assertions. This is the decorrelation that makes the number trustworthy.
- Practical: keep providers swappable via config; rotate the mutator provider periodically so the test suite is hardened against *multiple* models' blind spots, not just one.
- The LLM-mutator pairs with rule-based mutation: rule-based catches the mechanical operator gaps cheaply and deterministically; the cross-provider LLM-mutator catches the realistic-semantic-bug gaps. Use both; they're complementary.

---

## Deep-dive questions

1. **Mutant-set sizing for the LLM-mutator.** ACH used *few targeted* mutants per class. How many mutants per unit balances signal vs cost/latency in an interactive harness? Is there a prioritization (PRIMG-style) that lets us run 3–5 high-value mutants instead of dozens?
2. **Reference impl for pre-implementation mutation.** Step 3a needs *something* to mutate before the real code exists. Options: a trivial stub, an LLM-generated "obvious" impl, or run Harden against the first implementation and treat any post-implement test change as suspicious. Which gives the cleanest "tests can't be shaped to code" guarantee?
3. **Equivalence-judge cost/reliability at our scale.** 0.95/0.96 was with comment-stripping pre-processing on Meta's corpus. Does it hold on small Python units, and what's the per-mutant token cost? Cheaper heuristic pre-filters (AST-identical, lexical) first?
4. **MR library bootstrapping.** What's the minimal reusable set of metamorphic relations that covers most ML/experiment cases, and can the harness *auto-suggest* applicable MRs from the unit's signature/docstring?
5. **Threshold calibration.** Is a flat 85% right, or should the gate be relative ("mutation score must not drop vs baseline," PIT scm-style) to avoid penalizing inherently hard-to-test units?
6. **Incremental drift management.** How often must we force a full mutation run to correct incremental-cache drift, and can that be a scheduled background job rather than inline?
7. **Provider-pair selection.** Which concrete provider pairings maximize blind-spot decorrelation (measurable as: mutator-B survivors that author-A's own mutants missed)? Worth an offline experiment.

---

## Sources

- mutmut — https://github.com/boxed/mutmut · https://opensource.com/article/20/7/mutmut-python · https://johal.in/mutation-testing-with-mutmut-python-for-code-reliability-2026/
- cosmic-ray — https://cosmic-ray.readthedocs.io/ · distributed tutorial: https://cosmic-ray.readthedocs.io/en/latest/tutorials/distributed/index.html
- Stryker incremental — https://stryker-mutator.io/docs/stryker-js/incremental/ · https://stryker-mutator.io/blog/announcing-incremental-mode/ · changed-lines issue: https://github.com/stryker-mutator/stryker-js/issues/2843
- PIT / pitest — https://pitest.org/ · https://github.com/hcoles/pitest · https://javapro.io/2026/01/21/test-your-tests-mutation-testing-in-java-with-pit/
- Comparison of Python mutation tools — https://par.nsf.gov/servlets/purl/10573281 · https://dl.acm.org/doi/10.1145/3701625.3701659
- LLMs in Mutation Testing (TOSEM) — https://arxiv.org/html/2406.09843v2
- Meta ACH, Mutation-Guided LLM Test Generation — https://arxiv.org/html/2501.12862v1
- PRIMG (mutant prioritization for LLM test-gen) — https://arxiv.org/pdf/2505.05584
- Hypothesis / property-based testing — https://hypothesis.readthedocs.io/ · stateful+metamorphic guide: https://www.marktechpost.com/2026/04/18/a-coding-guide-for-property-based-testing-using-hypothesis-with-stateful-differential-and-metamorphic-test-design/
- Metamorphic testing (ML, no oracle) — https://www.lakera.ai/blog/metamorphic-relations-guide · https://www.hillelwayne.com/post/metamorphic-testing/ · METTLE: https://arxiv.org/pdf/1807.10453
- Metamorphic testing of LLMs — https://arxiv.org/html/2511.02108v1 · METAMON: https://arxiv.org/pdf/2502.02794 · survey: https://arxiv.org/html/2605.13898v1
- TDD-with-LLM blind spots — https://blog.yfzhou.fyi/posts/tdd-llm/ · https://medium.com/vibe-coding/stop-using-tdd-with-ai-agents-heres-what-i-use-f76d086ac56d · LLM4TDD: https://arxiv.org/pdf/2312.04687 · multi-LLM blind-spot loop: https://sosuke.com/models-have-blind-spots-debugging-unfamiliar-code-with-a-multi-llm-loop/
