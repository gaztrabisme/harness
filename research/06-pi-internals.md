# Track 6: Pi Internals & Extension Surface

## Scope & what it grounds

This track is a code/docs teardown of **badlogic/pi-mono** (npm scope `@earendil-works/*`) to decide the adoption layer for our AI-engineering agent harness and to confirm we can implement control-flow **gates** (e.g. an "Align gate" that blocks all execution tools until a plan is confirmed) on top of Pi rather than building a runtime from scratch.

Packages mapped:
- **`@earendil-works/pi-ai`** ("ai") — unified multi-provider LLM API (model discovery, streaming, tool-calling, cross-provider handoff).
- **`@earendil-works/pi-agent-core`** ("agent") — stateful `Agent` runtime, low-level `agentLoop()`, `AgentTool`, event stream, config-level `beforeToolCall`/`afterToolCall`, plus the newer `AgentHarness` + typed hook event system.
- **`@earendil-works/pi-coding-agent`** ("coding-agent") — the CLI/TUI, SKILL.md, the `ExtensionAPI` plugin system, subagents (markdown agents + workflow prompts), JSONL tree sessions, MCP, custom providers.
- **`@earendil-works/pi-tui`** — terminal UI primitives (only relevant for custom rendering).

**Headline:** Pi is a near-perfect substrate. The `tool_call` extension event can deny tool calls (`{ block: true, reason }`) — our gates map directly onto it. Model-per-role is first-class (`getModel(provider, id)` + per-agent `model:` frontmatter + runtime `pi.setModel()`). Local vLLM/MLX endpoints are first-class via `openai-completions`. What Pi does **not** give us is a higher-order orchestration plane (board / state-machine / workpad / cross-session memory) or per-subagent workspace isolation — those we build.

---

## pi-agent-core (Agent runtime, agentLoop, AgentTool, event stream) — real signatures

Two layers exist in this package:
1. The **`Agent` class** + low-level **`agentLoop()`/`agentLoopContinue()`** (stable, documented in `packages/agent/README.md`).
2. **`AgentHarness`** — a newer orchestration layer above the loop (session persistence, phases, save points, typed hook events). Per `agent-harness.md` it is partially implemented / "migration-in-progress"; the coding-agent still runs on its own runner today. **Treat `AgentHarness` as forward-looking, not the thing coding-agent currently uses.**

### Agent class

```typescript
const agent = new Agent({
  initialState: {
    systemPrompt: string,
    model: Model<any>,                 // from pi-ai getModel(...)
    thinkingLevel: "off"|"minimal"|"low"|"medium"|"high"|"xhigh",
    tools: AgentTool<any>[],
    messages: AgentMessage[],
  },
  convertToLlm: (messages) => Message[],          // required for custom msg types
  transformContext: async (messages, signal) => AgentMessage[],  // prune/compact
  steeringMode: "one-at-a-time" | "all",
  followUpMode: "one-at-a-time" | "all",
  streamFn: streamProxy,                          // browser/proxy backends
  sessionId: string,
  getApiKey: async (provider) => string,          // dynamic/expiring OAuth
  toolExecution: "parallel" | "sequential",       // default "parallel"
  beforeToolCall: async ({ toolCall, args, context }) => { block?: boolean; reason?: string } | void,
  afterToolCall:  async ({ toolCall, result, isError, context }) => { terminate?: boolean; content?; details?; isError? } | void,
  thinkingBudgets: { minimal, low, medium, high },
});
```

`AgentState` (read via `agent.state`):
```typescript
interface AgentState {
  systemPrompt: string; model: Model<any>; thinkingLevel: ThinkingLevel;
  tools: AgentTool<any>[]; messages: AgentMessage[];
  readonly isStreaming: boolean; readonly streamingMessage?: AgentMessage;
  readonly pendingToolCalls: ReadonlySet<string>; readonly errorMessage?: string;
}
```
Methods: `prompt(text|msg, images?)`, `continue()` (resume, last msg must be `user`/`toolResult`), `abort()`, `waitForIdle()`, `subscribe(fn)`, `reset()`, `steer(msg)`, `followUp(msg)`, `clearSteeringQueue/clearFollowUpQueue/clearAllQueues()`. State setters are live: `agent.state.model = getModel(...)`, `agent.state.tools = [...]`, `agent.beforeToolCall = ...`.

### AgentTool interface

```typescript
const tool: AgentTool = {
  name: "read_file",
  label: "Read File",                 // UI display
  description: "...",                  // shown to LLM
  parameters: Type.Object({ path: Type.String() }),  // TypeBox schema
  executionMode: "sequential" | "parallel",          // optional per-tool override
  execute: async (toolCallId, params, signal, onUpdate) => {
    onUpdate?.({ content:[{type:"text",text:"..."}], details:{} });  // stream progress
    return { content:[{type:"text",text:content}], details:{...}, terminate?: true };
  },
};
```
Error contract: **throw** on failure (agent catches → reports as toolResult with `isError:true`). Don't return error strings as content.

### Tool execution modes
- **`parallel` (default):** preflight tool calls sequentially, execute allowed tools concurrently, emit `tool_execution_end` per-tool as finalized; persisted `toolResult` messages keep assistant source order.
- **`sequential`:** one-by-one (historical behavior). If **any** tool in a batch has `executionMode:"sequential"`, the whole batch is sequential.
- **`terminate:true`** (from `execute()` or `afterToolCall`) hints "skip the automatic follow-up LLM call". Only takes effect when **every** finalized tool result in the batch is terminating; mixed batches continue.
- **`shouldStopAfterTurn`** (low-level loop only): `async ({message,toolResults,context,newMessages}) => boolean` — graceful stop after `turn_end`, before next LLM call; does not abort streams or cancel tools.

### Event stream
`prompt()` emits: `agent_start → turn_start → message_start/update/end → tool_execution_start/update/end → turn_end → agent_end`. `message_update` is assistant-only and carries `assistantMessageEvent` deltas. `Agent.subscribe()` listeners are awaited in registration order; `agent_end` listeners still gate `waitForIdle()`/`prompt()` settlement.

### Low-level API
```typescript
for await (const event of agentLoop([userMessage], context: AgentContext, config: AgentLoopConfig)) { ... }
for await (const event of agentLoopContinue(context, config)) { ... }
```
`agentLoop`/`agentLoopContinue` are **observational** — they preserve event order but do not await your async handlers before continuing to tool preflight. For a barrier (process assistant message before tool preflight) you must use the `Agent` class. Custom `AgentMessage` types are added via declaration merging (`CustomAgentMessages`) and filtered in `convertToLlm`.

---

## Hooks — each hook: signature, what it mutates, can it block?

**Critical clarification: there are THREE distinct hook surfaces.** Pick the right one.

### (A) Agent config callbacks (pi-agent-core, lowest level)
Set on the `Agent`/`AgentLoopConfig`. Two callbacks:
- `beforeToolCall({ toolCall, args, context }) → { block?: true, reason?: string } | void` — runs after `tool_execution_start` + validated arg parsing. **CAN BLOCK execution.**
- `afterToolCall({ toolCall, result, isError, context }) → { terminate?, content?, details?, isError? } | void` — postprocess result, can force `terminate`, can patch result.

These are single-callback slots (not a registry) and are the rawest gate point.

### (B) AgentHarness typed hook events (pi-agent-core, `hooks.md`)
A typed event bus where each event declares its own result type via a phantom symbol. Registration:
```typescript
interface AgentHarnessHooks<E, Ctx> {
  context: Ctx; setContext(ctx): void;
  observe(handler): () => void;                 // read-only, sees ALL events
  on<TType>(type, handler): () => void;         // participates in that event's semantics
  emit<TEvent>(event, signal?): Promise<ResultOf<TEvent>|undefined>;
  addCleanup(cleanup): () => void; clear(): Promise<void>; dispose(): Promise<void>;
}
```
Event reducer semantics (from `hooks.md`):
- **`context`** `{messages?}` — transform chain; each handler sees current messages; returns replacement.
- **`before_provider_request` / `before_provider_payload`** `{payload?}` — sequential transform; each sees previous output.
- **`before_agent_start`** `{messages?, systemPrompt?}` — chain system prompt + collect injected messages.
- **`tool_call`** `{block?, reason?}` — **sequential, early-exit on `block`. THIS IS THE BLOCKING GATE.** (`if (result?.block) return result;`)
- **`tool_result`** `{content?, details?, isError?}` — sequential patch accumulation (middleware).
- **`session_before_compact` / `session_before_tree`** `{cancel?}` — sequential, early-exit on cancel.
Registries (tools, commands, shortcuts, flags, renderers, providers, OAuth) are **not** hooks — they live on the hooks object/extension host as registrations, not `emit()` events.

### (C) ExtensionAPI events (pi-coding-agent — what extensions actually use)
This is the surface our harness consumes. `pi.on(event, handler)`. The full set (from `extensions.md` lifecycle):

| Event | Can mutate / block? |
|---|---|
| `project_trust`, `session_start`, `session_shutdown` | lifecycle |
| `session_before_switch/fork/compact/tree` | **can cancel/customize** |
| `resources_discover` | aggregate resource paths |
| `before_agent_start` | inject message + **modify systemPrompt** (chained) |
| `agent_start`/`agent_end`, `turn_start`/`turn_end` | observe |
| `message_start`/`update`/`end` | `message_end` can **replace** finalized message (same role) |
| `tool_execution_start`/`update`/`end` | observe |
| **`tool_call`** | **CAN BLOCK** + mutate `event.input` in place |
| `tool_result` | **can modify** result (middleware chain) |
| `context` | **can modify** messages before each LLM call |
| `before_provider_request` | inspect/**replace** provider payload |
| `after_provider_response` | observe status+headers |
| `model_select`, `thinking_level_select` | observe model/thinking changes |
| `user_bash` | **can intercept** `!`/`!!` commands (custom ops or full result) |
| `input` | **transform / handle** raw input before skill/template expansion |

### THE GATE MECHANISM (our Align gate)
`tool_call` is exactly our enforcement point. Real signature/behavior:
```typescript
import { isToolCallEventType } from "@earendil-works/pi-coding-agent";

pi.on("tool_call", async (event, ctx) => {
  // event.toolName, event.toolCallId, event.input (MUTABLE)
  if (!aligned && isExecutionTool(event.toolName)) {
    return { block: true, reason: "Align gate: confirm the plan first." };
  }
});
```
Guarantees from docs: mutations to `event.input` affect real execution and are visible to later handlers; **no re-validation after mutation**; the only return that matters is `{ block: true, reason? }`. Handlers run in extension load order, **first `block` wins (early exit)**. The Quick Start ships a literal example that blocks `rm -rf` after a `ctx.ui.confirm()` dialog — meaning a gate can be *interactive* (prompt the user, block until they confirm). For a stateful Align gate we hold an `aligned` flag in extension closure (or read it from session entries), flip it when the user confirms a plan (e.g. via a `/align` command or an `input` handler), and `block` all execution tools until then. `setActiveTools()` is a coarser complementary lever (swap to a read-only tool set during planning).

---

## pi-ai (multi-provider, model-per-role, local-endpoint support)

Unified API, **tool-calling models only** (by design). Providers supported (partial list): OpenAI, Anthropic, Google, Vertex AI, xAI, Groq, Cerebras, Mistral, DeepSeek, NVIDIA NIM, OpenRouter, Vercel AI Gateway, Together, Fireworks, Amazon Bedrock, GitHub Copilot (OAuth), OpenAI Codex (OAuth), ZAI, MiniMax, Kimi, Xiaomi MiMo, **"Any OpenAI-compatible API: Ollama, vLLM, LM Studio, etc."**

### Model selection (the model-per-role primitive)
```typescript
import { getModel, getModels, getProviders, stream, complete, streamSimple, completeSimple } from '@earendil-works/pi-ai';
const model = getModel('anthropic', 'claude-sonnet-4-20250514');  // provider + id, IDE-autocompleted
getProviders();           // ['openai','anthropic','google',...]
getModels('anthropic');   // typed Model[] with .api .contextWindow .reasoning .input
```
`Model<TApi>` is a plain object: `{ id, name, api, provider, baseUrl?, reasoning, input:["text"|"image"], cost, contextWindow, maxTokens }`. So **model-per-role = hold a `Model` per role and pass the right one**: recon→cheap/local, build→Sonnet, adversarial→Opus. In coding-agent terms, each subagent markdown sets `model:` in frontmatter; at runtime `pi.setModel(model)` (returns `false` if no API key). Thinking is unified: `streamSimple/completeSimple(model, ctx, { reasoning: 'minimal'|'low'|'medium'|'high'|'xhigh' })`. **Cross-provider handoff is built in** — switching model mid-conversation preserves context; foreign-provider thinking blocks are downgraded to `<thinking>`-tagged text automatically. This is what makes per-role model swapping safe inside one session.

### Local endpoints (vLLM / MLX / Ollama)
Fully supported as a custom `Model<'openai-completions'>` with a `baseUrl`, or declaratively via coding-agent `~/.pi/agent/models.json`:
```json
{ "providers": { "local": {
  "baseUrl": "http://localhost:8000/v1", "api": "openai-completions", "apiKey": "x",
  "compat": { "supportsDeveloperRole": false, "supportsReasoningEffort": false },
  "models": [ { "id": "Qwen3.6-35B-A3B-8bit" } ] } } }
```
`compat` flags handle partial-OpenAI servers (vLLM/SGLang/MLX): `supportsDeveloperRole`, `supportsReasoningEffort`, `supportsUsageInStreaming`, `maxTokensField`, `thinkingFormat` (`qwen-chat-template` etc.), `cacheControlFormat`. So our local oMLX (`127.0.0.1:8000/v1`) and the PC's llama.cpp/vLLM drop straight in as recon/cheap-tier models. `apiKey`/`headers` support `$ENV`, `${VAR}`, and `!command` (shell) resolution at request time.

---

## pi-coding-agent (SKILL.md, subagents, workflows, sessions, MCP, extension mechanism, worktree isolation)

### SKILL.md — yes, Claude-Code-compatible
Pi implements the **Agent Skills standard** (agentskills.io). Same `SKILL.md` + YAML frontmatter (`name`, `description`, optional `license`/`compatibility`/`metadata`/`allowed-tools`/`disable-model-invocation`). Progressive disclosure: only descriptions in system prompt, full body `read` on demand. Skills register as `/skill:name` commands. **It can directly load Claude Code / Codex skills** — add `"skills": ["~/.claude/skills", "~/.codex/skills"]` to settings. Locations: `~/.pi/agent/skills/`, `~/.agents/skills/`, project `.pi/skills/` (after trust), package `skills/`, `--skill <path>`. (Our existing `~/.claude/skills` — incl. this `dev` skill — would be reusable as-is.)

### Subagents (an EXAMPLE extension, not core)
`packages/coding-agent/examples/extensions/subagent/` — a ~ reference implementation we'd adopt/fork. Agents are markdown + frontmatter:
```markdown
---
name: scout
description: Fast codebase recon
tools: read, grep, find, ls, bash
model: claude-haiku-4-5
---
System prompt body...
```
Discovery (`agents.ts`): user `~/.pi/agent/agents/*.md` (always), project `.pi/agents/*.md` (only with `agentScope:"project"|"both"`, prompts confirmation). Tool modes: **single** `{agent,task}`, **parallel** `{tasks:[...]}` (max 8, **4 concurrent**), **chain** `{chain:[...]}` with `{previous}` placeholder. Each subagent runs as a **separate `pi` subprocess** → isolated context window, streaming output, usage tracking, Ctrl+C propagation. Parallel model-visible output **capped at 50 KB/task** (full result kept in details). Workflow **prompts** are prompt-template `.md` files (`/implement` = scout→planner→worker, etc.) in `~/.pi/agent/prompts/`.

### Sessions — JSONL, tree-structured
`~/.pi/agent/sessions/--<path>--/<ts>_<uuid>.jsonl`. Each line is a typed entry; entries form a **tree** via `id`/`parentId` (v2+; v3 current). Entry types: `session` (header), `message`, `model_change`, `thinking_level_change`, `compaction`, `branch_summary`, `custom` (extension state, NOT in LLM context), `custom_message` (extension msg, IN context), `label`, `session_info`. Branching is in-place (no new file). `buildSessionContext()` walks leaf→root, splices compaction summaries + branch summaries. Rich `SessionManager` API: `getBranch()`, `getTree()`, `branch()`, `branchWithSummary()`, `createBranchedSession()`, `appendCustomEntry()`, `appendCustomMessageEntry()`, `setLabel()`. **This tree+label+custom-entry model is a usable substrate for our board/checkpoints** (see next section).

### Extension / plugin mechanism (the adoption surface)
TS extensions, loaded via **jiti** (no build step). Default-export factory `(pi: ExtensionAPI) => void | Promise<void>` (async factory awaited before `session_start` — used for remote model discovery, etc.).
```typescript
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
export default function (pi: ExtensionAPI) { pi.on(...); pi.registerTool(...); pi.registerCommand(...); }
```
Locations: `~/.pi/agent/extensions/*.ts` or `*/index.ts` (global), `.pi/extensions/...` (project, after trust), `settings.json` `extensions:[...]` paths, and npm/git **packages** (`"packages": ["npm:@foo/bar","git:github.com/u/r@v1"]`, `pi install`). Imports available: `@earendil-works/pi-coding-agent`, `typebox`, `@earendil-works/pi-ai`, `@earendil-works/pi-tui`, plus arbitrary npm + node builtins.

**ExtensionAPI methods** (the whole control surface):
`pi.on`, `pi.registerTool` (live, callable immediately, no reload), `pi.sendMessage` / `pi.sendUserMessage` (with `deliverAs:"steer"|"followUp"|"nextTurn"`, `triggerTurn`), `pi.appendEntry` (persist extension state), `pi.setSessionName`/`getSessionName`, `pi.setLabel`, `pi.registerCommand` (+ arg autocompletion), `pi.getCommands`, `pi.registerMessageRenderer`, `pi.registerShortcut`, `pi.registerFlag`/`getFlag`, `pi.exec`, `pi.getActiveTools`/`getAllTools`/`setActiveTools`, `pi.setModel`, `pi.getThinkingLevel`/`setThinkingLevel`, `pi.events` (cross-extension bus), `pi.registerProvider`/`unregisterProvider`.
`ExtensionContext` (handler arg): `ctx.ui` (notify/confirm/select/input/editor/setStatus/setWidget/custom), `ctx.mode` (`tui`/`rpc`/`json`/`print`), `ctx.hasUI`, `ctx.cwd`, `ctx.sessionManager`, `ctx.modelRegistry`/`ctx.model`, `ctx.signal`, `ctx.isIdle/abort/hasPendingMessages`, `ctx.shutdown`, `ctx.getContextUsage`, `ctx.compact`, `ctx.getSystemPrompt`; command-context adds `newSession/fork/navigateTree/switchSession/reload/waitForIdle`.

This is a **very wide** extension API — tools, commands, shortcuts, flags, providers, message renderers, custom UI, session manipulation (fork/branch/navigate/label), model & thinking control, and all the lifecycle/tool/context/provider hooks. It is the richest part of Pi for us.

### MCP support
MCP is supported by coding-agent (there are dedicated docs: providers/models/rpc/sdk). The extension model can also wrap external tools. (Did not deep-read the MCP doc this pass — flagged for deep phase to confirm exact config surface and whether MCP servers are exposed to subagents.)

### Worktree / workspace isolation — DOES NOT EXIST
There is **no git-worktree / filesystem-sandbox per subagent**. Subagent isolation is **context-window only** (separate `pi` process, separate session), and the child runs in the **same `cwd`** as the parent. Containerization docs exist (`docs/containerization.md`) for running pi *itself* in a container, but that is whole-process, not per-subagent worktrees. **Per-subagent workspace/worktree isolation is something we must build** (e.g. spawn each subagent with a distinct cwd / git worktree and pass it through, or wrap `pi.exec`/the subagent launcher).

---

## What Pi gives us vs what we must build

**Pi gives us (adopt as-is):**
- Multi-provider model layer + per-role model selection + cross-provider handoff + local-endpoint (vLLM/MLX) support. (pi-ai)
- Agent runtime: streaming event loop, parallel/sequential tool exec, steering/follow-up queues, abort, thinking levels. (pi-agent-core)
- **Tool-call gating** via `tool_call` `{block}` (interactive or flag-driven). (extension `tool_call` / config `beforeToolCall`)
- Context transform / compaction hooks (`context`, `transformContext`, `session_before_compact`).
- SKILL.md (Claude-Code-compatible, can load `~/.claude/skills`).
- Subagents (markdown agents, single/parallel/chain, concurrency+size caps) and workflow prompts — as an example extension to adopt/fork.
- JSONL **tree** sessions with branching, labels, custom entries, branch/compaction summaries, full `SessionManager`.
- A wide extension API: tools, commands, shortcuts, flags, providers, renderers, custom UI, model/thinking control, session navigation.

**We must build (not in Pi):**
- **Board / state-machine** (the orchestration spine: gate phases like Align→Build→Verify, task/issue states, transitions). Pi has tool-level `block` and session tree but **no first-class workflow/phase state machine**. Build as an extension holding phase state, gating tools per phase, using `appendEntry`/`custom` session entries for durability. (The `AgentHarness` has an internal `phase` enum but it's `idle|turn|compaction|...`, not a user workflow FSM.)
- **Workpad** (shared scratch/artifact space across subagents/turns). Approximate with session `custom` entries + a filesystem convention; no built-in workpad abstraction.
- **Memory plane** (cross-session learning/retrieval). Pi sessions are per-cwd JSONL files with no semantic index; cross-session memory is entirely ours.
- **Per-subagent worktree/workspace isolation** — must wrap the subagent launcher to assign isolated cwd/git worktrees.
- **Multi-agent coordination beyond chain/parallel** (a true team/board with shared task list) — Pi's subagent tool is fire-and-collect, not a persistent coordinating team.

---

## Recommendation: adoption layer (extend / fork / bespoke)

**Recommendation: (A) EXTEND pi-coding-agent via the `ExtensionAPI`, plus depend directly on pi-ai and pi-agent-core where we need lower-level control. Fork ONLY the subagent example extension.**

Rationale:
- The extension API is wide enough to implement gates (`tool_call` block), phase/board state (closure + `custom` entries), model-per-role (`setModel`/frontmatter), workflows (prompt templates), and custom tools/commands/UI **without forking the host**. Forking the whole coding-agent means owning the TUI, provider plumbing, session format, compaction, MCP, and OAuth — all of which Pi maintains well and we don't want to.
- (C) bespoke-on-pi-agent-core is viable for a *headless/server* product (you get the loop, tools, hooks, handoff and skip the TUI), but you re-implement sessions, skills discovery, extension loading, providers config, MCP — a lot. Reserve this only if we need an embedded/RPC engine with no terminal. Note coding-agent also exposes an **SDK** (`createAgentSession({ customTools })`) and **RPC** docs — likely the right seam for a headless build without going all the way to raw `agentLoop`.
- The subagent capability is shipped as an *example* extension (not core), so we **fork that one file/dir** and extend it (add worktree isolation, board integration, richer coordination) — low cost, high control.

**Concrete adoption shape:** one or more Pi extensions in `.pi/extensions/` (or an installable package) that: (1) register an Align/phase gate on `tool_call`; (2) register `/align`, `/build`, board/phase commands; (3) hold phase + board state in closure, persist via `pi.appendEntry`; (4) register custom tools (workpad, memory query); (5) fork+extend the subagent extension for worktree-isolated, model-per-role subagents; (6) register our local oMLX/vLLM providers via `pi.registerProvider` (or `models.json`).

### How gates map to hooks (mechanism, concretely)
```typescript
// align-gate extension (sketch)
export default function (pi: ExtensionAPI) {
  let phase: "align" | "build" = "align";
  const EXEC_TOOLS = new Set(["bash","write","edit"]);  // execution tools gated in align phase

  // restore phase from session on reload
  pi.on("session_start", (_e, ctx) => {
    for (const e of ctx.sessionManager.getBranch())
      if (e.type === "custom" && e.customType === "phase") phase = e.data.phase;
  });

  // THE GATE
  pi.on("tool_call", async (event) => {
    if (phase === "align" && EXEC_TOOLS.has(event.toolName))
      return { block: true, reason: "Align gate: confirm the plan before executing." };
  });

  // flip the flag when the user/agent confirms the plan
  pi.registerCommand("align", { description: "Approve plan; unlock execution",
    handler: async (_args, ctx) => { phase = "build"; pi.appendEntry("phase", { phase }); ctx.ui.notify("Aligned. Execution unlocked.","info"); }
  });
}
```
Because `tool_call` runs **first-block-wins, early-exit**, and `event.input` is mutable, the same hook can also *rewrite* args (sandbox paths) or *interactively* `await ctx.ui.confirm(...)` before allowing. For a hard guarantee even against new/unknown tools, combine with `setActiveTools([...read-only...])` during the align phase so execution tools aren't even offered to the model.

---

## Deep-dive questions (verify by cloning in the deep phase)

1. **`tool_call` ordering vs parallel preflight:** docs say sibling tool calls are preflighted sequentially then run concurrently, and `tool_call` is "not guaranteed to see sibling tool results." Confirm a `block` on one sibling doesn't race others in the same batch — important for an all-or-nothing gate. (Read `packages/coding-agent/src/...` runner + `packages/agent/src/agent-loop.ts`.)
2. **Which runner is live today:** confirm coding-agent uses its own runner vs `AgentHarness`, and how stable the extension `tool_call`/`context` contract is across that migration (`agent-harness.md` item #7 "later coding-agent migration").
3. **SDK / RPC seam for headless:** read `docs/sdk.md` + `docs/rpc.md` — is `createAgentSession({ customTools, ... })` enough to run pi as an embedded engine with our gates, skipping the TUI? This decides whether "extend" can also serve a server build.
4. **MCP exact surface:** how MCP servers are configured, whether MCP tools pass through `tool_call` gating, and whether subagents inherit MCP tools. (No dedicated mcp.md in the file list — likely under providers/settings.)
5. **Subagent launch internals:** read `examples/extensions/subagent/index.ts` (not fetched) — exact subprocess spawn, how `model`/`tools` frontmatter is enforced on the child, abort propagation, and the cleanest injection point for per-subagent **cwd/worktree** isolation and shared-board handoff.
6. **`setActiveTools` + dynamic tools timing:** confirm a model mid-turn can't call a tool we just disabled (race between `setActiveTools` and an in-flight assistant message).
7. **Session `custom` entries as board store:** confirm branching semantics for our phase/board state (does `branch()`/fork correctly carry or reset `custom` entries for checkpoint/rollback of the board?).
8. **`AgentHarness` readiness:** if we want its typed hook reducers + save-point model, gauge how far from production it is (the todo list shows hooks item #4 "designed, not implemented").

---

## Sources (file paths read)

GitHub repo `badlogic/pi-mono@main` (npm scope `@earendil-works`, also published as `earendil-works/pi-mono`):

- `packages/agent/README.md` — Agent class, agentLoop, AgentTool, event stream, beforeToolCall/afterToolCall, tool exec modes, terminate, shouldStopAfterTurn, steering/follow-up.
- `packages/agent/docs/hooks.md` — AgentHarness typed hook event system, reducer semantics, `tool_call` block early-exit.
- `packages/agent/docs/agent-harness.md` — harness lifecycle, phases, save points, state model, implementation TODO (migration status).
- `packages/coding-agent/docs/extensions.md` (~101 KB; read in chunks: Quick Start, Writing/Async factory, full Events lifecycle, `tool_call`, `tool_result`, `input`, `user_bash`, ExtensionAPI methods, State Management, Custom Tools, registerProvider).
- `packages/coding-agent/docs/models.md` — custom providers/models.json, local endpoints (Ollama/vLLM/LM Studio), `compat` flags, thinkingLevelMap, value resolution.
- `packages/coding-agent/docs/skills.md` — Agent Skills standard, SKILL.md frontmatter, locations, loading Claude Code/Codex skills.
- `packages/coding-agent/docs/session-format.md` — JSONL tree session format, entry types, SessionManager API, buildSessionContext.
- `packages/coding-agent/examples/extensions/subagent/README.md` — subagent modes (single/parallel/chain), concurrency (max 8, 4 concurrent), 50 KB cap, agent frontmatter, workflow prompts, security model.
- `packages/coding-agent/examples/extensions/subagent/agents.ts` — agent discovery/frontmatter parsing, scope handling (source code).
- `packages/ai/README.md` (~55 KB; read: providers list, getModel/getModels/getProviders, streamSimple/completeSimple reasoning, custom Model objects, cross-provider handoffs).
- Repo file listing (81 files) — confirmed presence of docs: rpc.md, sdk.md, providers.md, custom-provider.md, containerization.md, compaction.md, packages.md (not deep-read this pass).

NOT cloned this pass (flagged for deep phase): `packages/agent/src/agent-loop.ts`, `packages/coding-agent/src/core/session-manager.ts`, subagent `index.ts`, `docs/sdk.md`, `docs/rpc.md`, MCP config surface.
