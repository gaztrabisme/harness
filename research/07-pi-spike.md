# Pi Spike — verify-by-building (P0 de-risk)

> Goal: prove or kill the EXTEND-pi architecture by *running* Pi, not reading its docs. Answer the P0
> unknowns from `00-design-v2.md` §13 hands-on. **Verdict: architecture VALIDATED — go.** The core
> enforcement primitive (Align gate) works, model-per-role + local oMLX work, and worktree isolation is
> cheaper than assumed. The spike's extension is commit #1 of the trunk.

## Setup (reproducible)

- Repo: `badlogic/pi-mono` cloned to `Skills/pi-mono` (v0.78.1). Monorepo uses **npm workspaces**, node **≥22.19**.
- Run from source via `./pi-test.sh` (runs `packages/coding-agent/src/cli.ts` through `tsx` — **no build step**).
- Node 22 via nvm (`v22.22.3`); the tool-shell re-resolves `node` to v20, so commands pin
  `PATH="$HOME/.nvm/versions/node/v22.22.3/bin:$PATH"`.
- Isolation: `PI_CODING_AGENT_DIR=Skills/harness-spike/agent` redirects the whole agent config dir →
  spike never touches Gary's global `~/.pi`. (Env name derived from `APP_NAME` in `config.ts`.)
- Spike workspace: `Skills/harness-spike/` = `agent/` (models.json), `workspace/` (sandbox cwd),
  `align-gate.ts` (the extension), `run-pi.sh` + `drive-tmux.sh` (launcher + test driver).

## What was tested and what held

| Test | Method | Result |
|---|---|---|
| pi runs from source | `./pi-test.sh --version` | ✅ `0.78.1` |
| oMLX as a local provider | `models.json` (`openai-completions`+`baseUrl`+`compat`) → `--list-models omlx` | ✅ both models visible |
| End-to-end inference | `-p "Reply ok"` via `omlx/Qwen3.5-0.8B-8bit` | ✅ returned `ok` |
| Runtime model-per-role | `--provider omlx --model omlx/<id>` selected per call | ✅ works (cross-provider handoff is built in per T6) |
| **Align gate blocks** | `-p` ask 35B to write a file in align phase | ✅ `write` **and** `bash` both denied; **no file** |
| **Batch atomicity** | same run emitted two exec tools | ✅ both blocked independently (per-tool preflight) |
| **/align unlocks** | tmux: prompt(blocked) → `/align` → prompt | ✅ file `unlock-test.txt` = `hello` created only after `/align` |
| **Phase persists** | grep session JSONL after `/align` | ✅ `{"customType":"align-gate:phase","data":{"phase":"build"}}` written |

## P0 unknowns (v2 §13) — resolved

1. **Can `tool_call` block enforce the Align gate?** — **YES, proven at runtime.** `pi.on("tool_call", …)` returning
   `{block:true, reason}` denies the tool; the loop substitutes an error tool-result carrying `reason`
   (`agent-loop.ts:598`), which the model relays to the user ("run /align to unlock"). The reason string
   doubles as a *teaching* surface — the gate tells the user how to proceed. Exactly the designed UX.
2. **Block atomicity under parallel preflight?** — **Non-issue.** Parallel mode *preflights tool calls
   sequentially, then runs allowed ones concurrently* (`types.ts:248`). Each mutating tool is evaluated
   against the phase flag independently, so in align phase **all** exec tools in a batch are blocked and
   read-only tools pass. Confirmed by code and by the runtime double-block.
3. **Worktree injection point?** — **Trivial.** The subagent spawns a child `pi` via
   `spawn(cmd, args, { cwd: cwd ?? defaultCwd })` (`examples/extensions/subagent/index.ts:329`), and `cwd`
   is already a first-class tool parameter (schema lines 428/434/451). Worktree isolation = create a git
   worktree, pass its path as `cwd`. No launcher surgery; we fork the example only to add the worktree
   create/cleanup wrapper.
4. **Custom-entry durability across reload?** — **YES.** `pi.appendEntry("align-gate:phase", {phase})`
   writes a `type:"custom"` entry to the session JSONL; `session_start` restores it via
   `ctx.sessionManager.getBranch()`. Persisted entry observed on disk. (Branch/fork semantics of custom
   entries still worth confirming when we build checkpoint/rollback, but the basic durability holds.)

## The extension (commit #1 of the trunk)

`Skills/harness-spike/align-gate.ts` — ~55 lines. Holds `phase: "align"|"build"` in closure; blocks
`{bash,write,edit}` on `tool_call` while align; `/align` and `/realign` flip + persist via `appendEntry`;
`session_start` restores phase from the session. This is the literal seed of the spine's Align gate — the
harness grows by adding board/state-machine/workpad/memory around this primitive.

## Incidental findings (useful for the build)

- **Pi self-manages search binaries** — auto-downloaded `fd` into `agent/bin/` on first run (also `rg`).
  We get fast file/grep tools for free; no need to ship our own.
- **Extension loads cleanly via `-e <path>`** (no trust prompt for explicit `-e`); shows under `[Extensions]`
  at startup. For the real harness we'll install it as a package or project `.pi/extensions/`.
- **Default provider is `google`** — the harness must set a default model in `settings.json` so it boots to
  the intended model-per-role config without `--provider/--model` flags.
- **`tsx` from-source run is the right dev loop** — fast iteration, no build; `npm run build` only needed for
  release binaries. Matches pi-mono's own `pi-test.sh` workflow.
- Cosmetic: tmux warns about extended-keys; irrelevant to the harness.

## Verdict & next step

**Adoption layer = EXTEND pi-coding-agent — confirmed.** Pi gives us the runtime, gating, model-per-role,
local endpoints, skills (loads `~/.claude/skills`), and tree sessions; we build board/state-machine/workpad/
memory/worktree-wrapper on top via the extension API. Nothing in the spike contradicted v2; two unknowns
(atomicity, worktree) resolved *easier* than assumed.

Next: begin the **thin trunk slice** (v2 §14 fork 2) — grow the align-gate extension into the minimal spine:
a file-backed board + workpad + the Todo→Align→…→Done phase machine + the land step, dogfooded on its own
tickets. Symphony's concrete coordinator mechanics for this are now in `08-symphony-teardown.md`.
