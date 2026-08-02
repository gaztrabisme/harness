# test-author/ — shakedown #8 (test authorship under the mutation gate)

**The flip.** Every prior shakedown (#1–#7) froze *my* oracle and had the agent write the
implementation; the mutation score then measured *my* test quality. This one flips it: the agent
writes **both** the implementation **and its own test suite**, and the harness's **Harden gate (§7,
"kill test theater")** measures whether the agent's tests are good enough to catch real faults.

**Honest claim (narrowed after adv review).** This does *not* measure oracle-agnostic test
authorship — the agent writes the impl too, so the harden gate scores the agent's tests against the
agent's *own* code. What it proves is: **can the 35B write a correct implementation AND a test suite
thorough enough to catch faults injected into it?** That is the realistic engineering loop ("ship the
function and tests that actually constrain it"), and it is exactly what the §7 gate exists to enforce.

## The task (given to the agent as ticket criteria)

Implement two pure functions in `grades.py` and write `test_grades.py` proving them:

- `letter_grade(score)` — `ValueError` if `score < 0` or `score > 100`; then `>=90`→`"A"`,
  `>=80`→`"B"`, `>=70`→`"C"`, `>=60`→`"D"`, else `"F"`.
- `gpa(letters)` — `ValueError` on an empty list or any unknown letter; points `A=4..F=0`;
  return `round(mean, 2)`.

`dogfood/test-author/` ships on `main` with **only this README — no `.py`**. The agent creates both
files in its worktree, so both land in the diff: cosmic-ray mutates `grades.py` (the `test_grades.py`
file is auto-excluded as an oracle), and the agent's own tests must kill the mutants.

## Why the two gates separate the lazy from the thorough

- `tests_green` (validation): `python3 -m unittest discover -s dogfood/test-author -t dogfood/test-author`.
  A happy-path-only suite is **GREEN here** — passing tests_green does *not* prove the tests are good.
- `harden` (§7): cosmic-ray mutation score on the diff, threshold **0.70**. A lazy suite leaves the
  boundary and validation mutants alive and **fails the number**; only a thorough suite (every band
  boundary exactly, the `<0`/`>100` validation, gpa empty/unknown/rounding) clears it.

**Calibration (proven before any run, with an independent correct impl).** Both suites GREEN on
unmutated code; mutation score: **lazy 0.545 (FAIL) · thorough 0.955 (PASS)**, 88 mutants, margin 0.41.
So tests_green cannot distinguish them — only harden does. (If the impl style or cosmic-ray version
changes, re-run the proof and recalibrate; mutable surface is impl-shape-dependent.)

## Integrity (operator-side, done by hand after the run — these are NOT harness gates)

The agent writes its own tests, so the harness alone cannot tell a *correct* impl from a *self-consistent
wrong* one. After a run I verify, against the agent's actual `grades.py`/`test_grades.py`:

1. **Correctness** — a hidden independent reference oracle (kept out of the repo and the worktree) is run
   against the agent's `grades.py`. The impl must be right *per spec*, not merely self-agreeing.
2. **Generalization** — the agent's `test_grades.py` is run against that hidden reference impl. It must
   be GREEN there too, proving the tests capture spec *intent*, not just the agent's own code shape.
3. **Discriminator-still-holds** — a lazy suite is run against the agent's *own* impl; it must still
   score <0.70, confirming the gate discriminates for this impl's mutable surface (not just the proof's).
4. **Mutant count** — the live cosmic-ray session's mutant count is compared to the proof's 88.

Agent-produced implementations are **never landed** from these oMLX exercise runs; only this README lives
on `main`. (Note: if a future variant ships a frozen impl on `main` and asks the agent to write *only*
tests, the current diff-scoped harden gate would have no changed impl to mutate — that needs a harness
change and is out of scope here.)
