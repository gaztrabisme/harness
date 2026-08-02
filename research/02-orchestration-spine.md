# Track 2: Orchestration Spine & Work Model

## Scope & what it grounds

This track grounds the **Work plane** of the harness: the BOARD of tickets, the per-ticket WORKPAD, the explicit STATE MACHINE whose transitions are un-skippable gates, and the LAND gate that promotes distilled output to mainline gated on proof-of-work. It also touches the seam between the Work plane and the Execution plane (coordinator + isolated workers), because the state machine is what the coordinator dispatches against and what workers report back into.

The closest prior art is **openai/symphony**, which is almost exactly our architecture: a tracker-as-state-machine, one workspace per issue, a single durable workpad comment, and a separate `land` loop that promotes to main only after proof. The key difference we must engineer around: Symphony is built for **full autonomy** (`approval_policy: never`, "never ask a human"), with the human reduced to two tracker-state gates. Our model is **gated-interactive** — the human is a first-class actor at Align and Review, and gates must be able to *block on a human* without the run failing. So we harvest Symphony's spine wholesale and re-introduce blocking human gates where Symphony deliberately removed them.

---

## Symphony SPEC.md — distilled

Read in full: SPEC.md (2,187 lines / 81KB), plus `elixir/WORKFLOW.md` (the actual work-model contract), `.codex/skills/land/SKILL.md` (the land loop). The SPEC is the *orchestrator service* spec (dispatch/concurrency/reconciliation); the **work model** lives in `WORKFLOW.md`.

### Two state machines, deliberately separated

Symphony keeps **tracker states** (the work model) distinct from the orchestrator's **internal claim states** (the scheduler). This separation is the single most important design move.

**Tracker states (the work model — what a ticket goes through):**
`Backlog → Todo → In Progress → Human Review → Merging → Done`, with `Rework` as a side-state that hard-resets back into the loop. `active_states` = {Todo, In Progress, Merging, Rework}; `terminal_states` = {Closed, Cancelled, Duplicate, Done}. The agent is *only* dispatched against active states; terminal states stop and clean up. `Human Review` is notably **not** an active state — it is the human gate (the agent polls, doesn't work).

**Orchestrator claim states (the scheduler — prevents duplicate dispatch):**
`Unclaimed → Claimed → {Running | RetryQueued} → Released`. This is in-memory, ephemeral, and tracker-driven on restart. It exists purely so two workers never grab the same ticket. *We need both layers too: the work-model FSM the human reasons about, and a claim/lease layer the coordinator uses.*

### Workspace-per-issue isolation (§9)

- Per-issue path: `<workspace.root>/<sanitized_issue_identifier>`. Workspace key sanitizes the identifier to `[A-Za-z0-9._-]`.
- **Safety invariants (the un-skippable part):** (1) the coding agent runs *only* with `cwd == workspace_path`, validated before launch; (2) workspace path MUST be a prefix-child of workspace root — any path outside is rejected; (3) key is sanitized. These three are checked mechanically, not by the agent.
- Workspaces are **reused** across runs for the same issue (continuation), **not** auto-deleted on success, and cleaned only when the issue reaches a terminal state (startup cleanup + reconciliation).
- Population (git clone, deps) is done by **hooks** (`after_create`, `before_run`, `after_run`, `before_remove`), not by the agent. `after_create` failure is fatal to workspace creation; `before_run` failure is fatal to the attempt. The actual `WORKFLOW.md` `after_create` does `git clone --depth 1 ... && mix deps.get`.

### The Workpad contract (the durable artifact)

**One persistent Linear comment per issue, marker header `## Codex Workpad`, edited in place** — never multiple "done"/summary comments. Treated as "the source of truth for progress." Exact template:

```md
## Codex Workpad

```text
<hostname>:<abs-path>@<short-sha>      # environment stamp, code-fenced
```

### Plan
- [ ] 1. Parent task
  - [ ] 1.1 Child task
- [ ] 2. Parent task

### Acceptance Criteria
- [ ] Criterion 1

### Validation
- [ ] targeted tests: `<command>`

### Notes
- <short progress note with timestamp>

### Confusions
- <only when something was confusing during execution>
```

Workpad lifecycle rules that matter:
- On entering a ticket, **reconcile the workpad first** (check off done items, fix the plan) before any new code.
- `Acceptance Criteria` + `Validation` must be kept current and **always make sense for the task**.
- If the ticket body carries a `Validation` / `Test Plan` / `Testing` section, **mirror it into the workpad as required (non-downgradeable) checkboxes** — this is "non-negotiable acceptance input."
- Do **not** edit the issue body for planning/progress — planning lives in the workpad comment only.
- `### Confusions` is a structured place to surface ambiguity for the human (a learning-loop / align signal).

### The Land loop (promote/distill to main)

`Merging` state → open and follow `.codex/skills/land/SKILL.md`; **never call `gh pr merge` directly.** The land loop:
1. Confirm the full gauntlet is green **locally** before any push.
2. Check mergeability vs main; if `CONFLICTING`, pull `origin/main`, resolve, push.
3. Acknowledge all review comments (human + Codex bot) before merge — **do not merge with outstanding review comments.**
4. Watch CI; on failure pull logs, fix, commit, push, re-watch (loop, don't yield).
5. When green and review addressed: **squash-merge** using PR title/body as subject/body.

So churn (many WIP commits on a branch) is collapsed into **one squashed commit** on main; the PR body + workpad are the distilled record. Branch auto-deletes.

### Proof-of-work (what must exist before promotion)

Proof is enforced at several un-skippable points, not asserted by the agent's say-so:
- **Reproduce-first:** capture a concrete reproduction signal (command/output, screenshot, deterministic UI behavior) into `Notes` *before* changing code, so the fix target is explicit.
- **`pull` skill evidence:** record merge source(s), result (`clean`/`conflicts resolved`), resulting HEAD short-SHA in Notes.
- **Validation gate:** execute every ticket-provided validation item; "treat unmet items as incomplete work." Prefer a **targeted proof that directly demonstrates the changed behavior.** Temporary proof edits allowed but **must be reverted before commit** and documented.
- **Green-before-push:** required validation must pass before *every* `git push`.
- **Completion bar before Human Review** (a checklist gate): plan complete + reflected in workpad; acceptance + ticket validation complete; tests green for latest commit; PR feedback sweep done with zero outstanding comments; CI green; branch pushed; PR linked; `symphony` label present. Only then may the agent move the ticket to `Human Review`.

### Scope discipline (out-of-scope → new ticket)

Hard rule, stated twice (default posture + guardrails): when meaningful out-of-scope improvements are discovered, **file a separate Linear issue instead of expanding scope.** The follow-up must include title + description + acceptance criteria, be placed in `Backlog`, assigned to the same project, `related`-linked to the current issue, and `blockedBy`-linked when dependent. The current ticket's scope is frozen to its plan.

### Rework = hard reset (not incremental patching)

`Rework` is explicitly "a full approach reset": re-read the whole issue + all human comments, identify what's done differently, **close the existing PR, delete the existing workpad comment, branch fresh from `origin/main`, rebuild plan from scratch.** This prevents accreting fixes on a flawed approach.

### Dispatch / concurrency / reconciliation (the coordinator mechanics, §7–8)

- **Poll tick:** reconcile running issues → preflight-validate → fetch candidates in active states → sort by dispatch priority → dispatch while slots remain.
- **Candidate eligibility:** has id/identifier/title/state; state ∈ active ∖ terminal; routed to this worker (assignee + required labels); not already running/claimed; global + per-state concurrency slots free; **blocker rule:** a `Todo` issue is not dispatched while any blocker is non-terminal.
- **Sort:** priority asc → created_at oldest → identifier lexicographic.
- **Concurrency:** global `max_concurrent_agents` (10 in WORKFLOW.md) + optional per-state caps.
- **Continuation:** a worker may run back-to-back agent turns (up to `max_turns`=20) on the same thread/workspace while the issue stays active; first turn = full prompt, continuation turns = short guidance only. After clean exit, a ~1s continuation retry re-checks whether the issue is still active.
- **Stall detection:** if no agent event for `stall_timeout_ms` (5m), kill + retry. Failure retries use exponential backoff `min(10000·2^(n-1), max)`.
- **Reconciliation every tick:** if tracker state went terminal → kill worker + clean workspace; still active → refresh snapshot; neither → kill without cleanup. **The tracker is authoritative** — a human moving a ticket state mid-run steers/aborts the worker. State is in-memory only; restart recovers by re-polling the tracker.
- **Approval posture (§10.5):** implementation-defined but a run **MUST NOT stall indefinitely** on approval/user-input. The example "high-trust" posture auto-approves all commands/file-changes and treats user-input-required as a *hard failure*. (This is exactly the knob we invert for gated-interactive.)

---

## Landscape

### LangGraph — state-machine orchestration with checkpointed interrupts
- **Technique:** Agents as an explicit graph: `State` (typed shared schema = the contract), `Nodes` (units of work), `Edges` (fixed or **conditional** routing functions that read state and return the next node), `Checkpointer` (persists state after every node → resume/retry/audit). HITL via `interrupt_before` / `interrupt_after` a node, or a dynamic `interrupt()` call inside a node that pauses the whole graph and surfaces a payload to a human, who can **approve / edit / reject / respond**; the persisted checkpoint is what makes pause-and-resume safe across hours or sessions.
- **Failure modes it names:** infinite conditional loops (fix: iteration counter + hard exit) and doubled accumulator fields on resume (fix: pass only the incremental update).
- Link: https://docs.langchain.com/oss/python/langchain/human-in-the-loop · https://www.langchain.com/langgraph

### SWE-agent / OpenHands — the observe→act→verify agent loop
- **Technique:** The **Agent-Computer Interface (ACI)** — give the agent a small set of *structured, purpose-built* commands (navigate/search, windowed file-view, lint-checked edit, run-tests) rather than raw shell. SWE-agent's loop settles into "edit → execute" with localization (`search_file`, `scroll`) interleaved in response to re-running the reproduction script. **Editing is the hardest step** (51.7% of trajectories had ≥1 failed edit; edits are flake8-checked *before* apply). OpenHands generalizes to an event-driven sandbox (filesystem + terminal + browser) with an **event log as memory**. Verification is external & mechanical: extract a `git_patch`, apply to a *fresh* environment, run the task's tests.
- **Counter-trend (Agentless):** a streamlined localize → repair → validate pipeline beats many heavy agent loops at lower cost — i.e., don't over-engineer the loop.
- Link: https://arxiv.org/pdf/2405.15793 (SWE-agent) · https://arxiv.org/html/2511.03690v1 (OpenHands SDK)

### OpenAI harness-engineering — the philosophy Symphony assumes
- **Technique:** Engineer everything *outside* the agent: per-worktree isolation (each task in its own sandboxed worktree with ephemeral, isolated logs/metrics torn down on completion); **mechanical invariants in CI** ("taste invariants") enforced as **hard failures, not warnings**; make error signals usable by the agent — **the lint/CI message itself becomes the next prompt** (structured feedback loop for autonomous self-correction); give the agent **a map, not a manual** (`AGENTS.md`); recurring "garbage collection" passes that scan for drift. Their stated conclusion: "Our most difficult challenges now center on designing environments, feedback loops, and control systems."
- Link: https://openai.com/index/harness-engineering/ (note: 403s to automated fetch) · https://martinfowler.com/articles/harness-engineering.html

### Plan-and-execute / plan-then-act
- **Technique:** Split a **planner** (writes a numbered plan up front) from an **executor** (walks each step), with a **replanner/joiner** that revises remaining steps from new observations and decides finish-vs-continue. Plan-then-act improves global coherence and cuts LLM calls vs ReAct's myopic step-by-step loop; the replanner restores adaptivity. Devin uses plan-first for long-horizon tasks with replanning when the executor stalls.
- Link: https://blog.langchain.com/planning-agents/

---

## Transferable techniques (technique → how it maps to our design)

1. **Two-layer FSM separation (Symphony tracker-vs-claim).** → Our BOARD is the human-facing work-model FSM (Todo…Done + Rework). Underneath, the coordinator keeps an ephemeral **claim/lease** layer (Unclaimed→Claimed→Running→Released) so isolated workers never double-grab a ticket. The board is durable + authoritative; the claim layer is rebuildable from the board on restart. *Never conflate them.*

2. **Tracker-as-authority + reconcile-before-dispatch.** → The board state is the source of truth; the coordinator reconciles every tick. A human moving a ticket (e.g. to Rework, or out of an active state) **steers or aborts the worker** without touching the worker directly. This is how the human gate physically blocks: an active worker on a ticket that leaves the active set is killed on the next reconcile.

3. **Workpad = single durable in-place artifact with a fixed contract.** → Adopt Symphony's template near-verbatim (Plan / Acceptance Criteria / Validation / Notes / Confusions) + the env stamp. Add our **Confusions → Align** wiring: confusions are the structured channel that re-opens a human gate. The workpad is what bridges branch-churn → mainline: branch commits are disposable, the workpad + squashed PR body are the distilled record that survives.

4. **Mirror ticket-authored validation as non-downgradeable acceptance.** → Our Verify gate must *ingest* any Validation/Test-Plan section from the ticket as **required** checkboxes the agent cannot weaken. This is the un-skippable core of the Verify gate.

5. **Reproduce-first + targeted proof + revert-temp-edits.** → Our proof-of-work contract: a reproduction signal recorded *before* the fix, a targeted proof that *directly* demonstrates the changed behavior, and any scaffolding reverted before commit and documented in Notes. Proof is captured artifacts, not the agent's assertion.

6. **Green-locally-before-push + completion-bar checklist as the gate.** → Verify→Review and Review→Land transitions are guarded by an explicit, mechanical checklist (tests green for *this* commit, acceptance met, CI green, no outstanding review comments). The checklist *is* the gate; an unchecked item blocks the transition.

7. **Land loop = squash + review-clear + CI-green (Symphony land skill).** → LAND collapses churn into one squashed commit; promotion requires conflict-free-with-main, all review comments acknowledged, CI green. Mainline only ever sees distilled output.

8. **Out-of-scope → new ticket, frozen scope.** → Encode Symphony's exact rule: discovered improvements become a new Backlog ticket (title + description + acceptance criteria, `related` link, `blockedBy` if dependent). The active ticket's scope is its plan; the plan is frozen at Align.

9. **Rework = hard reset.** → Our Rework transition closes the PR, archives/clears the workpad, branches fresh, rebuilds the plan. Prevents fix-on-flawed-approach accretion. (Soften vs Symphony: *archive* the old workpad rather than delete, to feed the learning loop.)

10. **LangGraph checkpointed `interrupt()` for human gates.** → This is the mechanism Symphony lacks. At Align and Review, the worker hits an interrupt that **persists state and surfaces a payload (plan / diff+proof) to the human**, who approves/edits/rejects. Resume is checkpoint-driven. Combined with #2, we get two complementary block mechanisms: tracker-state-leaves-active (coarse, coordinator-level) and in-worker checkpointed interrupt (fine, mid-run).

11. **ACI + lint-as-prompt (SWE-agent / harness-engineering).** → Workers get structured edit/test tools where edits are lint-checked before apply, and **CI/lint output is fed back as the next prompt**. Mechanical invariants are hard failures, not warnings — they make the Verify gate self-correcting without a human.

12. **Plan-then-act with a replanner.** → Align produces the frozen Plan; the executor walks it; a replanner revises *remaining* steps on new observations (but cannot expand scope — that routes to #8). Bounded iteration counter with a hard exit (LangGraph's loop fix) caps churn.

---

## Anti-patterns / what to avoid

- **Symphony's "never ask a human" posture.** `approval_policy: never` + "never ask a human to perform follow-up actions" + "treat user-input-required as a hard failure" is the *opposite* of our gated-interactive model. We invert this knob: user-input/approval is a first-class, blocking, *non-failing* state. Do **not** copy the full-autonomy default.
- **Human gate as a non-active poll state only.** Symphony's `Human Review` makes the agent merely poll. For us the human gate must also be able to (a) carry rich context to the human (plan diff, proof artifacts) and (b) resume the *same* worker mid-thread on approval — hence the checkpointed-interrupt mechanism, not just a state the agent watches.
- **In-memory-only scheduler state.** Symphony intentionally keeps scheduler state in memory and rebuilds from the tracker on restart (no durable retry timers/sessions). Fine for a polling service; risky for us if the board itself isn't durable. **Make the board + workpads durable; only the claim/lease layer may be ephemeral.**
- **Editing the ticket body for progress.** Symphony forbids this for good reason (keeps the audit trail in one comment). Keep planning/progress in the workpad, not the ticket description.
- **Unbounded agent loops.** Both Symphony (`max_turns`, stall timeout) and LangGraph (iteration counter) cap iteration. Always bound turns + add a stall/iteration kill-switch; an agent that "checks again" forever is a real failure mode.
- **Over-engineering the inner loop.** Agentless shows a lean localize→repair→validate pipeline beats heavy multi-agent planners at lower cost. Don't add planner/replanner/multi-agent machinery the task doesn't need; reserve it for genuinely long-horizon tickets.
- **Doc-only edits / proof by assertion.** Symphony explicitly bans "doc-only edits to appease review" and requires concrete validation for correctness feedback. Proof must be an artifact (test output, repro), never a claim.
- **Scope creep via "while I'm here" fixes.** The most common autonomous-agent failure. Freeze scope at Align; everything else is a new ticket.
- **interrupt_after when you meant interrupt_before.** LangGraph's named pitfall: pausing *after* a risky node means the action already happened. Approval gates must pause *before* the gated action (e.g., before push/merge), review-of-results gates *after*.

---

## Recommendation for our design

### The exact states + gate conditions

| State | Meaning | Entry gate (un-skippable condition to *enter*) |
|---|---|---|
| **Todo** | Queued, scoped enough to start | On board; blockers terminal (Symphony blocker rule); not claimed |
| **Align** | Plan + acceptance + validation drafted; **human approves scope** | Workpad exists with Plan + Acceptance Criteria + Validation populated; ticket-authored validation mirrored as required; **HUMAN approves the plan** (checkpointed interrupt). Blocks until approval. |
| **In Progress** | Executing the frozen plan | Came from Align with approved plan; workspace created in isolation (cwd==workspace, path inside root); reproduction signal recorded in Notes *before* code changes |
| **Verify** | Self-verification against acceptance + validation | Every Acceptance Criterion + every required Validation item checked with **captured proof artifact**; temp proof edits reverted; tests green for the current commit; lint/CI invariants pass |
| **Review** | Human (and/or bot) reviews diff + proof | Verify gate fully green; PR opened, branch pushed, CI green, workpad reconciled to match reality; **HUMAN review** (checkpointed interrupt). Outstanding review comments block. |
| **Land** | Promote distilled output to main | Review approved; conflict-free with main; all review comments acknowledged; CI green → **squash-merge** |
| **Done** | Terminal | Merge complete |
| **Rework** | Hard reset (from Review/Align rejection) | Reviewer/human rejects → close PR, archive workpad, branch fresh from main, rebuild plan; re-enters at Align |

**What makes each gate un-skippable** (mechanical, not honor-system):
- *Align/Review human gates* — implemented as **LangGraph-style checkpointed `interrupt()`** (pause *before* the gated action): the worker persists state and cannot transition until the human returns approve/edit/reject. Coarse backstop: the coordinator only dispatches workers against active states, so a ticket parked in a human gate has no active worker burning tokens.
- *Verify gate* — a **completion-bar checklist** evaluated mechanically: each acceptance/validation checkbox must be checked *and* backed by a proof artifact in the workpad; "green for this commit" is re-checked, not trusted from an earlier run.
- *Land gate* — guarded by the **land procedure** (below), which refuses to merge on conflicts, red CI, or outstanding review comments.
- *All transitions* — the board is authoritative; the coordinator **reconciles before dispatch every tick**, so illegal/concurrent transitions are impossible (claim/lease layer) and human steering takes effect within one tick.

### The workpad template (adopt Symphony's, with our additions)

```md
## Workpad — {{ticket.id}}

```text
<host>:<abs-workspace-path>@<short-sha>
```

### Plan            # frozen at Align; replanner may revise *remaining* steps, never expand scope
- [ ] 1. ...
  - [ ] 1.1 ...

### Acceptance Criteria   # what "done" means; human-approved at Align
- [ ] ...

### Validation       # required (non-downgradeable); mirrors any ticket-authored Test Plan
- [ ] targeted proof: `<command>`   → evidence: <captured output / artifact link>

### Notes            # reproduction signal (pre-change), pull/sync evidence, milestone log w/ timestamps
- ...

### Confusions       # structured ambiguity → re-opens an Align human gate
- ...
```

One workpad per ticket, edited in place; reconcile-first on every entry; never edit the ticket body for progress.

### The land procedure (distill + promote)

1. Confirm full gauntlet green **locally** before any push.
2. Check mergeability vs main; if conflicting → pull main, resolve, push.
3. Ensure all review comments (human + bot) acknowledged; **refuse to merge with any outstanding.**
4. Watch CI; on red → pull logs, fix, commit, push, re-watch (loop, don't yield).
5. When green + review clear → **squash-merge** (PR title/body as subject/body); branch auto-deletes.
6. Move ticket → Done.

Net effect: branch churn stays isolated in the per-ticket workspace; mainline receives exactly one squashed commit + a PR body distilled from the workpad + green CI as proof.

### Scope & rework rules (verbatim-ish from Symphony)
- Out-of-scope discovery → **new Backlog ticket** (title + description + acceptance criteria; `related` link; `blockedBy` if dependent). Active ticket's scope = its frozen plan.
- Rework = **hard reset**: close PR, archive workpad (don't silently delete — feed the learning plane), fresh branch, rebuild plan, re-enter at Align.

---

## Deep-dive questions

1. **Board substrate.** Symphony rents the FSM from Linear (tracker = board). Do we want an external tracker, a local file-backed board (markdown/SQLite), or the Pi framework's own task store? This decides durability, restart recovery, and how the human edits the board.
2. **Two block mechanisms — when each?** Coordinator-level (ticket leaves active set → worker killed) is coarse and loses in-thread context; checkpointed interrupt is fine but keeps a worker parked. For Align (long human latency) prefer coordinator-level park; for Review (likely fast) prefer interrupt-resume? Needs a policy.
3. **Continuation vs hard-stop at gates.** Symphony resumes the same agent thread/workspace across turns. After a human edits the plan at Align, do we resume the same thread (cheaper, retains context) or restart the turn with the edited plan (cleaner, avoids the LangGraph "edit-args causes re-evaluation" pitfall)?
4. **Proof-artifact storage.** Where do captured proof artifacts live so they survive squash-merge — workpad inline, PR body, or a sidecar in the Memory plane? Symphony keeps them in the workpad comment; that's lost if the comment is deleted on Rework.
5. **Replanner scope-guard.** How do we mechanically prevent the replanner from expanding scope (vs legitimately revising remaining steps)? A diff against the frozen Acceptance Criteria that routes additions to new-ticket creation?
6. **Lint/CI-as-prompt plumbing.** Concretely, how does structured CI/lint output get fed back as the next worker prompt in the Pi harness (the harness-engineering "the lint message becomes the prompt" trick)?
7. **Per-state concurrency for human gates.** Symphony has per-state concurrency caps. Do we want a cap on how many tickets sit in Review/Align at once (to avoid flooding the human)?

## Sources

- openai/symphony — `SPEC.md` (full, 2,187 lines), `elixir/WORKFLOW.md` (work model + workpad template), `.codex/skills/land/SKILL.md` (land loop). https://github.com/openai/symphony
- OpenAI, "Harness engineering: leveraging Codex in an agent-first world" — https://openai.com/index/harness-engineering/
- Martin Fowler, "Harness engineering for coding agent users" — https://martinfowler.com/articles/harness-engineering.html
- LangGraph / LangChain HITL docs — https://docs.langchain.com/oss/python/langchain/human-in-the-loop · https://www.langchain.com/langgraph
- SWE-agent (ACI) — https://arxiv.org/pdf/2405.15793
- OpenHands Agent SDK — https://arxiv.org/html/2511.03690v1
- Agentless (lean localize→repair→validate) — https://arxiv.org/pdf/2511.00872
- LangChain, "Plan-and-Execute Agents" — https://blog.langchain.com/planning-agents/
