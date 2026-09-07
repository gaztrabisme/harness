# Symphony Elixir Teardown (implementation mechanics)

Source clone: `$HOME/Documents/Work/Skills/symphony` (openai/symphony, shallow). All paths below are repo-relative under `elixir/lib/` unless noted. The orchestrator is a single 1951-line GenServer; that file is where ~90% of the spine lives.

## What this adds beyond 02-orchestration-spine.md

02 distilled the *contracts* (SPEC.md / WORKFLOW.md / land SKILL.md). This pass reads the *code* and pins the things the docs only gestured at:

- The tick is **not** one function — it's a GenServer message cascade (`{:tick}` → `:run_poll_cycle` → `maybe_dispatch`) with a deliberate 20ms render gap so the dashboard can paint "checking now…". Worth knowing because the "poll tick" is really three handlers.
- The "claim/lease layer" is concretely **a `MapSet` of issue ids (`claimed`) plus a `running` map plus a `blocked` map plus a `retry_attempts` map** — four in-memory structures, no lease object, no TTL. Double-dispatch is prevented by a 6-clause boolean guard, not by a lock.
- Stall detection runs **inside** `reconcile_running_issues` on every tick by diffing `now` against `last_codex_timestamp || started_at`; the "5 min" is `codex.stall_timeout_ms` default `300_000`. Backoff is `min(10_000 * 2^(n-1), max_retry_backoff_ms=300_000)` with a literal `<<<` bit-shift.
- There are **two distinct kinds of agent-exit handling**: a "blocked" path (Codex asked for human input/approval/elicitation → ticket parked in `blocked` map, *not retried*) and a "retry" path (crash/stall → exponential backoff). 02 didn't surface the `blocked` map at all — and it is the single most relevant existing mechanic for our gated-interactive model.
- Hooks run as **OS subprocesses** (`System.cmd("sh", ["-lc", cmd], cd: workspace)` locally, or `cd … && cmd` over SSH) with a `hooks.timeout_ms` default `60_000` and `Task.shutdown(:brutal_kill)` on timeout. `after_create`/`before_run` failures are fatal; `after_run`/`before_remove` failures are swallowed (`ignore_hook_failure/1`).
- cwd==workspace is enforced in **two** places with the same canonicalize-and-prefix check: `Workspace.validate_workspace_path/2` (before creation) **and** `AppServer.validate_workspace_cwd/2` (before the codex port is even opened). Symlink escape is a distinct rejection from outside-root.
- **Zero persistence.** Supervision tree is `:one_for_one` with no Repo/Ecto-SQL/postgrex/dets/ets. Restart recovery = "start with empty State, re-poll Linear, rebuild running/claimed by re-dispatching active issues, and clean workspaces for terminal issues." This is the gap we must close.

## The reconcile/dispatch tick (real code, module:function, file paths)

File: `symphony_elixir/orchestrator.ex`.

**Timer cascade (why it's 3 handlers):**
- `schedule_tick(state, delay_ms)` (l.1542) arms `Process.send_after(self(), {:tick, tick_token}, delay)` where `tick_token = make_ref()`. The token guards against stale timers firing after a manual refresh re-armed the tick.
- `handle_info({:tick, tick_token}, %{tick_token: tick_token})` (l.75) flips `poll_check_in_progress: true`, notifies dashboard, then `schedule_poll_cycle_start()` which does `:timer.send_after(20, self(), :run_poll_cycle)` (`@poll_transition_render_delay_ms = 20`, l.1558).
- `handle_info(:run_poll_cycle, ...)` (l.110) is the real body:
  ```elixir
  state = refresh_runtime_config(state)   # re-read poll_interval + max_concurrent every tick
  state = maybe_dispatch(state)
  state = schedule_tick(state, state.poll_interval_ms)   # re-arm
  %{state | poll_check_in_progress: false}
  ```

**The actual reconcile-then-dispatch — `maybe_dispatch/1` (l.248):**
```elixir
state =
  state
  |> reconcile_running_issues()     # refresh/kill running workers vs tracker truth
  |> reconcile_blocked_issues()     # re-check parked (human-input) tickets

with :ok <- Config.validate!(),
     {:ok, issues} <- Tracker.fetch_candidate_issues(),
     true <- available_slots(state) > 0 do
  choose_issues(issues, state)
else
  ... # every error branch just logs and returns state unchanged
  false -> state                    # no slots → do nothing this tick
end
```
So the order is **reconcile running → reconcile blocked → preflight (`Config.validate!`) → fetch candidates → global-slot gate → choose**. Reconciliation is *unconditional* every tick; candidate fetch is skipped if no global slots.

**`choose_issues/2` (l.769)** sorts then folds, dispatching while the per-issue guard passes:
```elixir
issues
|> sort_issues_for_dispatch()
|> Enum.reduce(state, fn issue, state_acc ->
     if should_dispatch_issue?(issue, state_acc, active_states, terminal_states),
       do: dispatch_issue(state_acc, issue), else: state_acc
   end)
```

**Sort — `sort_issues_for_dispatch/1` (l.784):** `Enum.sort_by` on the tuple
`{priority_rank(priority), issue_created_at_sort_key, identifier||id||""}`.
`priority_rank` maps Linear 1..4 to itself, everything else (incl. nil/0 "no priority") to **5** (l.794). `created_at` missing sorts to `max_int` (oldest-first, undated last). So: **priority asc → created_at oldest → identifier lexicographic**, exactly as 02 said, now with the nil-handling pinned.

**`should_dispatch_issue?/4` (l.804)** is the eligibility AND-gate — the 7 conjuncts:
```elixir
candidate_issue?(issue, active_states, terminal_states) and      # id+identifier+title+state present, routable, active∖terminal
  !todo_issue_blocked_by_non_terminal?(issue, terminal_states) and # blocker rule
  !MapSet.member?(claimed, issue.id) and                          # not already claimed
  !Map.has_key?(running, issue.id) and                            # not running
  !Map.has_key?(blocked, issue.id) and                            # not parked-on-human
  available_slots(state) > 0 and                                  # global cap
  state_slots_available?(issue, running) and                      # per-state cap
  worker_slots_available?(state)                                  # ssh-host cap
```

**Preflight per-dispatch — `dispatch_issue/4` (l.909) → `revalidate_issue_for_dispatch/3` (l.995):** before spawning, it **re-fetches that single issue from the tracker** (`Tracker.fetch_issue_states_by_ids([id])`) and re-checks `retry_candidate_issue?`. Result: `{:ok, fresh}` dispatch, `{:skip, …}` if it went stale/terminal between candidate-fetch and dispatch, `{:error,…}` on fetch failure. This is a TOCTOU guard — the candidate list can be seconds stale.

**Blocker rule — `todo_issue_blocked_by_non_terminal?/2` (l.864):** only applies when `normalize_issue_state(state)=="todo"`; then ANY blocker whose state isn't terminal (or any malformed blocker) blocks dispatch. Non-Todo active states ignore blockers.

## Claim/lease implementation

There is **no lease object and no TTL.** The "claim layer" is four fields on `Orchestrator.State` (l.24):
```elixir
running: %{},            # issue_id => running_entry (pid, ref, issue snapshot, codex telemetry, started_at, retry_attempt…)
completed: MapSet.new(), # issue_ids that exited normal (advisory only)
claimed: MapSet.new(),   # issue_ids "owned" right now — the actual mutex
blocked: %{},            # issue_id => blocked_entry (parked awaiting human input)
retry_attempts: %{}      # issue_id => %{attempt, timer_ref, retry_token, due_at_ms, …}
```

**Unclaimed→Claimed→Running is atomic in one reduce step.** `spawn_issue_on_worker_host/5` (l.942) starts the worker under `Task.Supervisor`, `Process.monitor`s it, then in a single struct update:
```elixir
%{state |
  running: Map.put(state.running, issue.id, %{pid: pid, ref: ref, … started_at: DateTime.utc_now()}),
  claimed: MapSet.put(state.claimed, issue.id),          # claim taken here
  retry_attempts: Map.delete(state.retry_attempts, issue.id)}
```
Because the GenServer is single-threaded, `should_dispatch_issue?` (which checks `claimed`/`running`) and the claim insert happen in the same message turn → **double-dispatch is structurally impossible within a node**; no lock needed. (Cross-node would need a real lease; Symphony is single-node.)

**Released** = `release_issue_claim/2` (l.1183), which deletes from `claimed`, `blocked`, and `retry_attempts` together. Called whenever a tracker re-poll shows the issue left active/became terminal/disappeared. `terminate_running_issue/3` (l.546) additionally stops the task (`Task.Supervisor.terminate_child` → fallback `Process.exit(:shutdown)`), `Process.demonitor(ref, [:flush])`, optionally cleans the workspace, and removes from `running` too.

**Worker death → `handle_info({:DOWN, ref, …})` (l.120):** finds the issue by ref (`find_issue_id_for_ref/2`), pops it from `running`, records token totals, then branches in `handle_agent_down/5`:
- `:normal` exit + **input-required blocker** → `block_input_required_agent_down` (park in `blocked`, no retry).
- `:normal` exit, not blocked → `complete_issue` + schedule a **continuation** retry at `@continuation_retry_delay_ms = 1_000` (l.13) — the ~1s "is the issue still active?" re-check.
- abnormal exit, not blocked → `retry_agent_down` (exponential backoff).

**Rebuild on restart:** there is none beyond re-polling. `init/1` (l.52) builds an empty `%State{}`, runs `run_terminal_workspace_cleanup/0` (deletes workspaces for issues currently in terminal states), and schedules the first tick at delay 0. Claimed/running are reconstructed implicitly: the first `maybe_dispatch` sees active issues with no claim and re-dispatches them. **A worker mid-flight when the orchestrator restarts is orphaned** (its OS process / remote codex keeps going) and the issue is simply re-dispatched fresh. This is the durability hole.

## Stall detection + retry (constants)

**Constants (module attrs, l.13-16 + config schema defaults):**
- `@continuation_retry_delay_ms = 1_000` — back-to-back turn re-check.
- `@failure_retry_base_ms = 10_000` — backoff base.
- `@poll_transition_render_delay_ms = 20` — tick→poll render gap.
- `codex.stall_timeout_ms` default **300_000** (5 min) — `config/schema.ex` l.182; `<= 0` disables.
- `agent.max_retry_backoff_ms` default **300_000** — backoff ceiling (schema l.139).
- `agent.max_turns` default **20**, `agent.max_concurrent_agents` default **10** (schema l.137-138).
- `polling.interval_ms` default 30_000 in schema (l.81), but **WORKFLOW.md overrides to `5000`** (5 s) — the shipped config polls every 5 s.
- `hooks.timeout_ms` default 60_000; `codex.turn_timeout_ms` 3_600_000 (1 h); `codex.read_timeout_ms` 5_000.

**Stall loop — `reconcile_stalled_running_issues/1` (l.574):** called first inside `reconcile_running_issues`. For each running entry (skipping any already in `blocked`):
```elixir
elapsed_ms = max(0, DateTime.diff(now, last_activity_timestamp(entry), :millisecond))
# last_activity_timestamp = entry.last_codex_timestamp || entry.started_at  (l.646)
if elapsed_ms > timeout_ms do
  if input_required_blocker?(entry) -> stop_and_block_issue(...)   # park, don't retry
  else -> terminate_running_issue(id, false) |> schedule_issue_retry(id, next_attempt, %{error: "stalled for #{elapsed_ms}ms…"})
```
So "no codex event for 5 min" = stall → kill (no workspace cleanup) → backoff retry. Note the **input-required-vs-stall fork**: a stalled agent that last emitted `turn_input_required`/`approval_required`/MCP `elicitation/request` is *parked*, not retried (`input_required_blocker?/1`, l.652).

**Backoff — `failure_retry_delay/1` (l.1200):**
```elixir
max_delay_power = min(attempt - 1, 10)
min(@failure_retry_base_ms * (1 <<< max_delay_power), max_retry_backoff_ms)
```
i.e. `min(10_000 · 2^(n-1), 300_000)` capped at attempt-1=10. `retry_delay/2` (l.1192) special-cases `delay_type: :continuation, attempt==1` → fixed 1_000ms.

**Retry scheduling — `schedule_issue_retry/4` (l.1023):** cancels any prior timer, mints a `retry_token = make_ref()`, `Process.send_after(self(), {:retry_issue, id, token}, delay)`, stores `%{attempt, timer_ref, retry_token, due_at_ms, identifier, issue_url, error, worker_host, workspace_path}` in `retry_attempts`. When `{:retry_issue,id,token}` fires (l.182), `pop_retry_attempt_state` validates the token matches (drops stale timers), then `handle_retry_issue` **re-polls candidates**, and only re-dispatches if the issue is still an active retry-candidate AND slots are free (`handle_active_retry`, l.1162) — else reschedules with attempt+1. Retries are therefore self-throttling against tracker truth and capacity.

## Workspace lifecycle hooks (the safety invariants in code)

File: `symphony_elixir/workspace.ex` + `symphony_elixir/path_safety.ex` + `symphony_elixir/codex/app_server.ex`.

**The lifecycle (who calls what):**
- `AgentRunner.run_on_worker_host/4` (`agent_runner.ex` l.37) is the orchestration:
  ```elixir
  {:ok, workspace} <- Workspace.create_for_issue(issue, worker_host)   # → after_create hook
  try do
    with :ok <- Workspace.run_before_run_hook(workspace, issue, worker_host) do  # fatal if fails
      run_codex_turns(workspace, issue, …)
    end
  after
    Workspace.run_after_run_hook(workspace, issue, worker_host)        # best-effort
  end
  ```
- `create_for_issue/2` (l.15) pipeline: `safe_identifier` → `workspace_path_for_issue` → `validate_workspace_path` → `ensure_workspace` (reuse dir if present, else mkdir, returns `created?`) → `maybe_run_after_create_hook` **only when `created?==true`** (so reuse across runs does NOT re-clone). `after_create` failure aborts creation.
- `before_remove` runs inside `remove/2` only when the dir exists; `Workspace.remove_issue_workspaces/2` is what the orchestrator calls on terminal/blocked cleanup.

**WORKFLOW.md hook bodies (shipped):** `after_create: git clone --depth 1 … && cd elixir && mise … mix deps.get`; `before_remove: cd elixir && mise exec -- mix workspace.before_remove`. (`elixir/WORKFLOW.md` l.22-28.)

**Hook execution** — `run_hook/5` (l.294): local = `System.cmd("sh", ["-lc", command], cd: workspace, stderr_to_stdout: true)` wrapped in `Task.async` + `Task.yield(timeout_ms)`; on timeout `Task.shutdown(task, :brutal_kill)` → `{:error, {:workspace_hook_timeout,…}}`. Remote = same command over `SSH.run` with `cd <esc> && cmd`. Non-zero exit → `{:error, {:workspace_hook_failed, hook, status, output}}` (output truncated to 2 KB in logs). `after_run`/`before_remove` pipe through `ignore_hook_failure/1` (l.291) → failures swallowed; `after_create`/`before_run` propagate.

**The three safety invariants, in code:**
1. **cwd == workspace, validated before launch.** Enforced *twice*. `Workspace.validate_workspace_path/2` (l.358) before create, and `AppServer.validate_workspace_cwd/2` (`app_server.ex` l.147) immediately before `start_port` opens the codex port with `cd: String.to_charlist(workspace)` (l.203) / `cd <esc> && exec codex` remotely (l.217). The codex process literally cannot start outside the validated dir.
2. **Workspace must be a prefix-child of root.** Both validators canonicalize (`PathSafety.canonicalize/1`) then require `String.starts_with?(canonical_workspace <> "/", canonical_root <> "/")`. `canonical_workspace == canonical_root` is a *distinct* rejection (`:workspace_equals_root` / `:workspace_root`) — you can't run *at* the root. A path that's a prefix-child only *before* canonicalization (i.e. via a symlink that escapes after resolution) is rejected as **`:workspace_symlink_escape`** — a separately named failure (l.374 / app_server l.163).
3. **Key sanitized.** `safe_identifier/1` (l.206): `String.replace(identifier || "issue", ~r/[^a-zA-Z0-9._-]/, "_")`. Workspace path = `Path.join(workspace.root, safe_id)`.

`PathSafety.canonicalize/1` (`path_safety.ex`) is a hand-rolled symlink-resolving realpath: walks each segment, `File.lstat`, and on `:symlink` reads the link and recurses from the link target — so it resolves chains, and `:enoent` (not-yet-created tail) is allowed (returns the joined path). This is what makes the symlink-escape detection real rather than string-prefix theatre.

## The land loop implementation

Files: `.codex/skills/land/SKILL.md` (the procedure the agent follows in the `Merging` state) + `.codex/skills/land/land_watch.py` (the async watcher it runs). There is **no Elixir land code** — landing is delegated to the coding agent running the skill inside its workspace. The orchestrator's only involvement: `Merging` is an active state, so a worker is dispatched and told (via prompt/WORKFLOW.md) to open the land skill; it never calls `gh pr merge` directly.

**`land_watch.py` is the mechanical gate** — an asyncio race of three tasks (`watch_pr/0`, l.575):
- `wait_for_checks(head_sha, …)` (l.547): polls `commits/{sha}/check-runs` every `POLL_SECONDS=10`; `dedupe_check_runs` keeps latest run per name; `summarize_checks` (l.201) → `(pending, failed, failures)`. Any conclusion not in `{success, skipped, neutral}` = failed → **exit 3**. No checks after `CHECKS_APPEAR_TIMEOUT_SECONDS=120` → exit 3. All complete + none failed → `checks_done.set()`.
- `wait_for_codex(pr_number, checks_done)` (l.514): polls comments; `raise_on_human_feedback` (l.488) → **exit 2** if any unaddressed human issue-comment, human review-comment, or `## Codex Review` issue-comment, or any blocking review. Unaddressed = not followed by a newer `[codex]`-prefixed reply in the same thread (the ack protocol). Returns only when `checks_done` is set with zero outstanding feedback.
- `head_monitor()` (l.588): if `mergeable=="CONFLICTING"` or `mergeStateStatus=="DIRTY"` → **exit 5**; if `head_sha` changed (e.g. CI autofix commit) → **exit 4**.

**Exit codes are the loop's control signal** (SKILL.md l.110): `2`=address review, `3`=CI failed, `4`=head moved (pull/amend/force-push to retrigger), `5`=conflicts. The agent reacts to the code (fix → `commit` skill → `push` skill → re-run watcher) and **loops**; it only squash-merges when the watcher exits 0 (`success_task` gathered cleanly). Merge = `gh pr merge --squash --subject "$pr_title" --body "$pr_body"`; remote branch auto-deletes.

**Loops vs yields:** the watcher itself does not loop forever — it *exits with a code* on the first actionable event; the **agent** is the loop (re-invoke watcher after each fix). The SKILL's stated rule: "Do not yield to the user until the PR is merged." Review-comment gating is **hard**: `raise_on_human_feedback` exits 2 before any merge, and Codex-review issue comments stay "unresolved" until a newer `[codex]` ack is posted. gh calls have their own retry/backoff (`run_gh`, l.39: 5 retries, base 2s, doubling, jitter) but only for HTTP 429 — other gh errors raise immediately.

## In-memory vs durable — what we must add for a durable file-backed board

**Everything in the orchestrator is in-memory and ephemeral.** Supervision tree (`symphony_elixir.ex` l.26): `Phoenix.PubSub`, `Task.Supervisor`, `WorkflowStore` (just caches WORKFLOW.md, l.2 "reloads when WORKFLOW.md changes" — config, not state), `Orchestrator`, `HttpServer`, `StatusDashboard`, strategy `:one_for_one`. **No Repo, no Ecto-SQL, no postgrex, no dets, no ets, no on-disk state file.** The only durable substrates are (a) **Linear** (the board/FSM + workpad comment) and (b) **git/GitHub** (branches, PRs). The orchestrator's `%State{}` is pure RAM.

**What "restart recovery" actually is:** `init/1` → empty `%State{}` → `run_terminal_workspace_cleanup` (GC workspaces for terminal issues) → first tick re-polls Linear → active-but-unclaimed issues get re-dispatched. **No retry timers survive, no in-flight session survives, no claim survives.** An issue mid-run during a crash is re-run from scratch (workspace is reused if it still exists, so the agent reconciles the workpad and continues — that's the only continuity, and it lives in Linear + the workspace dir, not in the orchestrator).

**For our durable file-backed board we must ADD (Symphony has none of this):**
1. **A persisted board** — the FSM state, ticket bodies, and workpads on disk (markdown/SQLite), not rented from Linear. This replaces `Tracker.fetch_candidate_issues` (the `Tracker` behaviour at `tracker.ex` is a clean 5-callback seam: `fetch_candidate_issues`, `fetch_issues_by_states`, `fetch_issue_states_by_ids`, `create_comment`, `update_issue_state` — implement it against a file store and the orchestrator is unchanged).
2. **Durable claim/lease with crash-recovery semantics.** Symphony's `claimed` MapSet is fine to keep ephemeral *only because* the board is the source of truth and re-dispatch is idempotent-ish. If our board is durable and authoritative, we still want a **lease with owner+expiry persisted on the ticket** (or a sidecar) so that after a coordinator crash we can tell "claimed-but-dead" from "actively running" rather than blindly re-dispatching. Symphony tolerates orphaned workers; we should fence them (lease epoch / PID-host stamp the worker writes into the workpad on start, checked on recovery).
3. **Durable retry schedule.** `retry_attempts` (timers + backoff counters) is RAM-only; a restart loses all pending backoffs and the attempt counter resets to 0. Persist `{attempt, due_at, error}` per ticket so backoff survives restart and a flapping ticket doesn't reset its exponential delay on every crash.
4. **Persisted "blocked/parked-on-human" set.** Symphony's `blocked` map is the closest thing to our human-gate park state and it's RAM-only — on restart a parked ticket is just re-evaluated from its tracker state. For us the park reason (which gate, what the human must answer, the checkpoint payload) must be durable on the ticket so the gate survives a restart and the human's eventual answer resumes the right thing.
5. **Checkpoint payloads for in-worker interrupts.** Symphony has no resume-mid-thread mechanism; our Align/Review interrupts need persisted checkpoints (LangGraph-style) — entirely additive.

Everything else (the tick, the reduce-dispatch, the sort, the slot math, the reconcile-before-dispatch discipline, the cwd/path invariants, the land watcher) ports as-is.

## Concurrency control

Three independent caps, all counted off the in-memory `running` map (no separate counters):

- **Global** — `available_slots/1` (l.1329): `max(max_concurrent_agents - map_size(running), 0)`. `max_concurrent_agents` default 10 (WORKFLOW.md sets 10). Gated both in `maybe_dispatch` (skip candidate fetch entirely if `<=0`) and per-issue.
- **Per-state** — `state_slots_available?/2` (l.822): `limit = Config.max_concurrent_agents_for_state(issue.state)` vs `running_issue_count_for_state(running, issue.state)` (counts running entries whose snapshot's state matches, normalized lowercase). Limits come from `agent.max_concurrent_agents_by_state` (schema l.140, default `%{}` = unlimited per state, falls back to global). State names normalized via `normalize_issue_state` (downcase+trim).
- **Per-SSH-worker-host** — `worker_host_slots_available?/2` (l.1293) / `select_worker_host/2` (l.1241): filters configured `worker.ssh_hosts` to those under `worker.max_concurrent_agents_per_host`, prefers the previously-used host (continuation affinity), else **least-loaded** (`least_loaded_worker_host`, l.1269, min by running count then list index). Returns `:no_worker_capacity` if all hosts full → dispatch skipped. With no ssh_hosts configured, runs local (host=nil, no per-host cap).

All three are AND-ed in `should_dispatch_issue?`. Because counting is derived from `running` (not incremented/decremented), it can't drift — every count is recomputed each tick. **Counting is O(n) per check** but n≤10 so it's free.

## Direct transfer list (concrete things to copy into our coordinator) vs things to change for gated-interactive

**Copy near-verbatim:**
- The **reconcile-before-dispatch tick order**: `reconcile_running → reconcile_blocked → preflight → fetch candidates → global gate → sort → fold-dispatch-while-slots`. (`maybe_dispatch/1`.)
- The **claim-in-the-same-message-turn** trick: check eligibility and insert the claim in one synchronous step so double-dispatch is impossible without locks. (`spawn_issue_on_worker_host/5`.)
- **Per-dispatch revalidation** (`revalidate_issue_for_dispatch/3`): re-read the single ticket from the durable board immediately before spawning, to close the TOCTOU between candidate snapshot and dispatch.
- **Sort key** `{priority, created_at, id}` with explicit nil-handling (no-priority → rank 5, no-date → last).
- **Stall = no-activity-timestamp-diff > timeout**, computed inside reconcile; kill + backoff. Backoff `min(base·2^(n-1), ceil)`; continuation re-check at a fixed short delay. Token-validated retry timers (`make_ref()` guard) to drop stale fires.
- The **three workspace safety invariants** and the **double cwd validation** (pre-create + pre-port-open), incl. canonicalize-then-prefix and the distinct symlink-escape rejection. Port `PathSafety.canonicalize/1` directly.
- **Hooks as timed subprocesses** with fatal `after_create`/`before_run` and best-effort `after_run`/`before_remove`; workspace reuse (only run `after_create` on actual creation).
- **The land watcher's exit-code contract** (2/3/4/5) and the **"agent loops, watcher signals"** split; squash-merge with PR title/body; hard refuse-to-merge-on-outstanding-review.
- The **`Tracker` behaviour seam** (5 callbacks) — implement it against our file board; coordinator code stays untouched.
- The **`blocked` map pattern** — a separate "parked, do not retry, re-evaluate against tracker each tick" set distinct from the retry queue. This is the existing mechanic closest to our human gates.

**Change for gated-interactive + durable board:**
- **Invert the input-required handling.** Symphony treats `turn_input_required` / `approval_required` / MCP elicitation as a *blocker that parks the ticket but is a degenerate/failure-ish state* (`input_required_blocker?`, with stall-while-blocked logged as a warning). For us this is the **first-class, non-failing Align/Review gate** — same park mechanism, opposite valence: the worker should *intentionally* yield to a human, persist a checkpoint payload, and the coordinator should surface it to the human, not treat it as a stall to eventually kill.
- **Make the gate park survive restart.** Persist the `blocked`/parked entry (which gate, payload, reason) on the durable ticket, since our `blocked` set must outlive a coordinator crash.
- **Add a durable lease/epoch** so re-dispatch after crash fences orphaned workers instead of blindly double-running (Symphony tolerates orphans; we shouldn't, because our board is durable and a stale worker could write to a workpad the new worker owns).
- **Persist retry backoff state** so attempt counters and `due_at` survive restart.
- **Don't auto-clean workspaces on terminal-by-human-rejection the way Symphony GCs terminal states** — our Rework should *archive* the workpad (feed the learning plane) rather than delete; keep the workspace until the fresh branch is cut.
- **Per-state caps become human-flow throttles.** Reuse `max_concurrent_agents_by_state` to cap how many tickets sit in `Align`/`Review` at once (avoid flooding the human) — Symphony has the mechanism, we repurpose the intent.
- **Two block mechanisms, by gate latency.** Symphony has only the coordinator-level "leave active set → reconcile kills the worker" coarse block. We add the in-worker checkpointed interrupt for fast gates (Review) and keep the coarse coordinator park for slow ones (Align). Symphony's reconcile-kill is exactly the coarse backstop; build the fine one on top.

## Sources (file paths read)

- `$HOME/Documents/Work/Skills/symphony/elixir/lib/symphony_elixir/orchestrator.ex` (full, 1951 lines — the tick, claim layer, stall/retry, reconcile, concurrency, dispatch)
- `$HOME/Documents/Work/Skills/symphony/elixir/lib/symphony_elixir/workspace.ex` (lifecycle hooks, path validation, remote-vs-local create/remove)
- `$HOME/Documents/Work/Skills/symphony/elixir/lib/symphony_elixir/path_safety.ex` (symlink-resolving canonicalize)
- `$HOME/Documents/Work/Skills/symphony/elixir/lib/symphony_elixir/agent_runner.ex` (worker lifecycle, continuation turns, hook ordering)
- `$HOME/Documents/Work/Skills/symphony/elixir/lib/symphony_elixir/codex/app_server.ex` (l.147-223: second cwd validation + codex port launch with cd)
- `$HOME/Documents/Work/Skills/symphony/elixir/lib/symphony_elixir/config/schema.ex` (all defaults/constants: concurrency, timeouts, stall, backoff, sandbox policy)
- `$HOME/Documents/Work/Skills/symphony/elixir/lib/symphony_elixir/tracker.ex` (the 5-callback tracker seam)
- `$HOME/Documents/Work/Skills/symphony/elixir/lib/symphony_elixir.ex` (supervision tree — confirms no persistence)
- `$HOME/Documents/Work/Skills/symphony/elixir/lib/symphony_elixir/workflow_store.ex` (config cache, not state)
- `$HOME/Documents/Work/Skills/symphony/.codex/skills/land/SKILL.md` + `.codex/skills/land/land_watch.py` (land loop + async watcher, exit-code contract)
- `$HOME/Documents/Work/Skills/symphony/elixir/WORKFLOW.md` (shipped work-model config: states, hooks, interval_ms=5000, max_concurrent=10, max_turns=20)
