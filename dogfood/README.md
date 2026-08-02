# dogfood/ — throwaway harness shakedown scaffolding

Test oracles for end-to-end harness shakedowns. Each subdir holds a fixed
acceptance test (written before the agent runs); the harness's agent loop writes
the implementation beside it in an isolated worktree. Only these oracles live on
`main` — agent-produced implementations are never landed from oMLX exercise runs.

- `lru-cache/` — shakedown #1 (happy path): `LRUCache(capacity)` kata. Oracle:
  `python3 -m unittest discover -s dogfood/lru-cache -t dogfood/lru-cache -v`
- `merge-intervals/` — shakedown #2 (failure/iteration probe): `merge_intervals`,
  with a touching-vs-gap tripwire. Oracle:
  `python3 -m unittest discover -s dogfood/merge-intervals -t dogfood/merge-intervals -v`
- `regex-match/` — shakedown #4 (explore-fanout stress / failure-trajectory probe):
  `is_match(s, p)` full-string matcher over `. * + ?`. An above-one-shot-capability
  kata (the `+`/`?` + backtracking interactions are where workers break) run through
  `agent explore` to force a PASS/kill spread and live-fire reflection-on-kill. The 49
  expected booleans were generated from `re.fullmatch` (a strict superset of the kata's
  syntax), so the frozen oracle is provably correct. Oracle:
  `python3 -m unittest discover -s dogfood/regex-match -t dogfood/regex-match -v`
- `calc-modulo/` — shakedown #5 (**multi-file / edit-existing-code stress**): an
  EXISTING 3-file recursive-descent calculator (`lexer.py` → `grammar.py` →
  `evaluator.py`, supporting `+ - * /`, parens, unary minus) ships on `main` with the
  oracle. The task: add the `%` modulo operator at multiplicative precedence — a
  coordinated change spanning all three files (lexer emits the token, grammar gives it
  precedence, evaluator computes it). Unlike the from-scratch katas, this measures
  whether the model can read existing code and extend it WITHOUT clobbering it: the
  oracle's 21 `regression` tests (existing ops) must stay green while the 14 `feature`
  tests (`%`, incl. paren/precedence combos like `10%3*2`, `(2+3)%4`) go from
  error→pass. Expecteds generated from Python's own `eval()` (the op set is a strict
  subset of Python arithmetic). Baseline on `main`: 21 pass / 14 error. Oracle:
  `python3 -m unittest discover -s dogfood/calc-modulo -t dogfood/calc-modulo -v`
- `big-edit/` — shakedown #6 (**big-file editing under the refeed offload cap**): a
  single ~5 KB `library.py` of ~30 independent pure functions ships on `main` with
  exactly ONE deliberate bug — `is_prime()` lacks its `n < 2` guard and reports 0 and 1
  as prime. The catch: at >4096 B the tool-output offload preview shows only the file's
  head `[0,2730)` + tail `[3796,end)`, and `is_prime` sits at byte ~3446 — in the
  **elided middle**. A worker reading the file the default way literally cannot see the
  bug; it must navigate the offload (re-read a slice via `read_file` offset/limit, or
  grep). A naive full-overwrite from the truncated preview clobbers the elided block
  (`fib` + `is_prime`), which the `fib` regression test catches. The task: fix `is_prime`
  WITHOUT breaking any other function. Expecteds come from independent references
  (builtins / `math` / a correct `is_prime`), not from importing the buggy code. Baseline
  on `main`: 56 pass / 2 fail (`is_prime(0)`, `is_prime(1)`). Oracle:
  `python3 -m unittest discover -s dogfood/big-edit -t dogfood/big-edit -v`
- `sig-ripple/` — shakedown #7 (**cross-file signature ripple**): an EXISTING 4-file pricing
  package ships on `main` — `pricing.py` holds the one helper `line_total(qty, unit_price)`,
  called from **7 textually-distinct sites** across `cart.py` / `invoice.py` / `report.py`,
  each with its discount stated in a per-call comment. The task: change `line_total` to a
  REQUIRED-parameter form `line_total(qty, unit_price, discount_pct)` returning
  `qty*unit_price*(1-discount_pct/100)`, then thread each caller's discount. Because every
  call site has distinct args, each needs its own unique `edit_file` anchor (no `replace_all`
  shortcut) — the multi-anchor regime. The oracle can't be faked: two `TestSignature` cases
  force the change to live *in* `line_total` (block the optional-default and hardcode-in-caller
  bypasses); seven value tests catch a missed/lazy caller; `basket_b` + `rush_fee` are 0%-discount
  tripwires that pass on `main` but crash if a model skips them as "no change needed" once the
  signature becomes 3 required args. Expecteds generated from plain Python arithmetic (op set ⊂
  Python), baked as literals, `assertAlmostEqual` for float safety. Baseline on `main`: 2 pass /
  7 fail. Oracle:
  `python3 -m unittest discover -s dogfood/sig-ripple -t dogfood/sig-ripple -v`
- `test-author/` — shakedown #8 (**test authorship under the mutation gate** — the flip): every prior
  kata froze *my* oracle and had the agent write the impl; this one has the agent write **both**
  `grades.py` (impl) AND `test_grades.py` (its own tests), gated by the **Harden gate (§7)**. The dir
  ships on `main` with ONLY its README (no `.py`), so both agent files land in the diff — cosmic-ray
  mutates `grades.py` (the test file is auto-excluded as an oracle) and the agent's OWN tests must kill
  the mutants. A happy-path suite passes `tests_green` (GREEN on unmutated code) but **FAILS harden**
  (boundary/validation mutants survive); only a thorough suite clears 0.70. Proven discriminator
  (independent correct impl, before any run): **lazy 0.545 FAIL / thorough 0.955 PASS**, 88 mutants.
  Honest claim (narrowed by adv review): not oracle-agnostic authorship — it proves the 35B can write a
  correct impl AND tests thorough enough to catch faults in its own code. Operator integrity (by hand,
  post-run, not harness gates): hidden reference oracle checks impl correctness + the agent's tests are
  GREEN against that independent impl (generalization) + a lazy suite still scores <0.70 on the agent's
  actual impl. No frozen oracle on `main` (only the README); agent output never landed. Gates:
  `tests_green` (`python3 -m unittest discover -s dogfood/test-author -t dogfood/test-author`) then
  `harden` (cosmic-ray mutation score ≥ 0.70 on the diff).
- `terax-keymap/` — shakedown #9 (**rebuild a real module from spec** — first dogfood on real-world code):
  three pure functions lifted from Terax's `keymap.ts` (keyboard event → readline escape sequence) are rebuilt
  from a prose spec, gated by a frozen oracle ported from Terax's own `keymap.test.ts`. The dir ships on `main`
  with a **blanked `keymap.mjs` stub** (signatures + names + readline-action docstrings, bodies `return null`)
  and a README spec; the agent fills the bodies in its worktree. The frozen oracle lives **outside the worktree**
  (`/tmp/ta-keymap/oracle.mjs`, hidden like #8's `/tmp/ta-proof`) and dynamic-imports the agent's `keymap.mjs`
  from cwd, so the 22 expected (input → escape) pairs never leak into the worktree (no overfit-to-inputs bypass);
  it pins each case by identity and exits 0 only if all 22 pass (tamper-evident without the Python-only vacuous
  guard). 22 cases = Terax's 13 + 9 added guard-boundary negatives; the 11 `→ null` cases are the discriminator.
  Calibration (before any run): reference 22/22 PASS · lazy (no guards) 11/22 FAIL · stub baseline 13/22.
  `.ts`→`.mjs` is a documented narrowing (no vitest/tsc toolchain dep; TS types here are trivial). Exercise-only,
  never landed into Terax. Gate: `tests_green` (`node /tmp/ta-keymap/oracle.mjs`); harden vacuous on `.mjs`.
