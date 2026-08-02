# terax-keymap/ — shakedown #9 (rebuild a real module from spec)

**The first dogfood on real-world code.** Every prior shakedown (#1–#8) used a synthetic kata. This one
takes a pure module lifted from a real repo (Terax, a Tauri terminal emulator) — `keymap.ts`, three pure
functions that map a keyboard event to the escape sequence a readline-style line editor expects — and asks
the 35B to **rebuild it from a prose spec**, gated by a frozen oracle ported from Terax's own `keymap.test.ts`.

**What it measures (one capability):** can the model translate a behavioral spec into correct code for the
two things that are genuinely hard here — (a) **readline escape constants** ("Ctrl+A" → `\x01`, "Ctrl+U" →
`\x15`, word-motion → `\x1bb`/`\x1bf`), and (b) **modifier-exclusion guards** (each binding fires only when
its modifier is the *sole* modifier; every other combination → `null`). The spec names the readline *action*
(matching Terax's own docstrings) but never the byte values and never the guard logic — those are the work.

## The task (given to the agent as ticket criteria)

Implement the three exported functions in `dogfood/terax-keymap/keymap.mjs` (the stub ships with signatures,
names, and a prose contract per function; bodies are `return null`). Keep names/signatures exact — the oracle
imports them by name. It is plain JavaScript: a key event is `{ altKey, ctrlKey, metaKey: boolean, key, code:
string }`, platform is `{ isMac: boolean }`.

- `terminalWordNavigationSequence(event)` — Alt-sole + arrow → word-left `\x1bb` / word-right `\x1bf`, else null.
- `terminalLineNavigationSequence(event, opts)` — macOS only; Meta-sole + arrow → line-start `\x01` / line-end `\x05`, else null.
- `terminalDeleteSequence(event, opts)` — Backspace; mac Cmd-sole → `\x15`, mac Opt-sole → `\x17`, other Ctrl-sole → `\x17`, else null.

(The byte values above are stated *here in the operator's README for audit* — they are NOT in the stub the
agent sees. The agent gets only the readline-action prose, like the real Terax docstrings.)

## Why the gate can't be cheated

- **`tests_green` (validation):** `node /tmp/ta-keymap/oracle.mjs`. The frozen oracle lives **outside the
  worktree** (hidden, like #8's `/tmp/ta-proof`) and dynamic-imports the agent's `keymap.mjs` from the
  worktree cwd. So the 22 expected (input → escape) pairs **never enter the worktree** — the agent writes
  from the spec, not from a visible lookup table, which kills the overfit-to-the-test-inputs bypass.
- The oracle pins **each case by identity** and exits 0 **only if all 22 pass** (`passed === count`) — an
  emptied or deleted stub fails to import or fails cases, never a silent zero-collection pass. This makes the
  kata tamper-evident on its own, independent of the harness's Python-only vacuous-pass guard. The oracle is
  also not on the agent's branch, so it cannot be edited (the `.mjs` it can write is the impl, not the test).
- **22 cases = Terax's 13 + 9 added guard-boundary negatives.** The 11 negative (`→ null`) cases are the
  discriminator: a happy-path-only impl passes the positives and fails the guards.

**Calibration (proven before any run, gate-by-artifact):** reference (faithful JS port of `keymap.ts`) →
**22/22 PASS**; lazy impl (correct escapes, *no* exclusion guards) → **11/22 FAIL** (fails all 11 guard
negatives). So `tests_green` cannot be reached without the guards. Stub baseline on `main`: 13/22.

## Honest claims / scope

- **`.ts` → `.mjs` is a deliberate, documented narrowing.** Terax's `keymap.ts` is TypeScript; this kata runs
  plain JS. Reason: Terax's `node_modules` is absent and local Node is v20 (no native type-strip), so real
  `vitest`/`tsc` would mean installing a ~200-package toolchain into a sandboxed worktree — rejected under
  *dependencies are liabilities*. For *this* module the TS "types" are `Pick<KeyboardEvent, …>` and
  `{ isMac: boolean }` — reading fields off an object, zero inference challenge — so the measured difficulty
  (escape constants + guards) is byte-identical in JS. Weaker on type-discipline (~nil here), identical on
  what's measured.
- **Exercise-only, never landed.** Run via `agent explore` fan-out on oMLX. Worktrees are reaped; agent
  output is **never landed into Terax** (Terax is a read-only spec source — only `keymap.ts`/`keymap.test.ts`
  were read to author this scaffold). Only this stub + README live on `main`.
- **Operator integrity (post-run, by hand — NOT harness gates):** per passing worker, (1) re-confirm the
  hidden oracle is green, (2) re-run the lazy impl to confirm the discriminator still holds (<22), (3) read
  the agent's `keymap.mjs` to confirm it is a real keymap, not a lookup table keyed on the case inputs,
  (4) diff against Terax's real `keymap.ts` to see how close the spec-driven rebuild landed.

Validation command: `node /tmp/ta-keymap/oracle.mjs`  ·  Harden: vacuous 1.0 (the `.mjs` diff is neither
`.rs` nor `.py`), so the frozen oracle's exact-string pins are the sole discriminator — stated plainly.
