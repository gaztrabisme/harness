# Case-law — learned heuristics injected into every worker

> **This file is the agent's amendable case-law (self-evolution Half A, research/20 §9).**
> Every `- ` / `* ` bullet under a `## Lessons` heading (case-insensitive) is read by the agent
> loop at the start of each run, screened, size-budgeted, and injected into the worker's system
> prompt as `### Learned heuristics (case-law)`. The ticket workpad always wins on conflict;
> these are cross-ticket defaults, not the contract. Bullets outside a `## Lessons` heading (like
> the contract bullets below) are curator-only and never injected.

## The contract (read before editing)

- **Human-approved only.** A lesson is law *because a human committed it here.* The agent never
  writes this file in the loop — Half B (the `reflect` propose-only instrument) is deferred
  (§9.1). Until then, lessons are distilled by Claude-in-the-loop and added by a human edit +
  commit. The commit *is* the approval.
- **General, not bound-specific.** A good lesson reads true on the next ticket too. No worker
  ids, no digit-counts, no literal one-off commands. If it names `t5 w2` or "9 lines changed",
  it belongs in a post-mortem memory, not here.
- **Never weakens a gate.** The read path drops any bullet that pairs a gate reference with a
  weakening verb (defense-in-depth — *not* the safety boundary; you are). A lesson that lowers,
  skips, or fakes a gate is the one thing this file must never carry.
- **Budgeted.** Only the first `CASE_LAW_MAX_BULLETS` lessons inject (the run logs
  `injected N/M`). Keep the list short and high-signal; prune before you append.

## Lessons

- Prefer standard-library primitives over a wrapper dependency; the bar for a new dependency is "saves more debugging time than it will cost."
- State success criteria as a number or an artifact before starting, and never change them to fit the result.
- Write the failing test first, then the implementation; a green suite with no red proves nothing.
- Keep one file to one concern; if you must scroll to see the core logic, it is overscoped — split by responsibility.
- Do not add an abstraction (trait, factory, registry) until a second real implementation exists; extract it from working code, not upfront.
- Report outcomes faithfully: if a step was skipped or a test failed, say so with the output — honest failure beats fabricated success.
- When stuck after three real attempts, stop and report the blocker rather than working around it silently.
- Ask whether a change actually fixes a problem that is blocking work, causing bugs, or adding maintenance burden; if not, drop it.
