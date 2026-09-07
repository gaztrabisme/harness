# research/25 — Context Engineering (survey + design input)

> Special research project (Gary, 2026-06-10): *"start a special research project for this problem, look up all
> the literatures, forums tricks and see how other frameworks do it (Hermes Agent, oh-my-pi, OpenCode, etc.)."*
> Working assumption now in force (`decisions.md`): **assume the local model is CAPABLE given proper context
> engineering** — the harness's job is to feed it the *right* context, not to route around an assumed capability
> gap. This doc is the survey that grounds the re-feed redesign. Method = **Hybrid**: Claude tears down the local
> clones directly (this doc's §2); three web workers survey the rest (§3); §4 synthesizes; §5 adversarial review.

## 1. The problem (why this project exists)

Our agent loop does **zero semantic context curation**. Grounded in `crates/agent/src/main.rs:300-499` +
`crates/agent/src/refeed.rs`:

- `messages` is built **once** (`Message::system(...)` + `Message::user(title)`) then is **push-only forever**.
  Every turn sends `messages.clone()` — the entire transcript, growing unbounded.
- The **only** interference is a per-message **byte cap** (`refeed::cap`, head ⅔ + tail ⅓ + elision marker):
  assistant prose → `REFEED_TEXT_CAP = 1024 B`, tool results → `REFEED_TOOL_CAP = 4096 B`.
- **tool_call arguments are UNCAPPED** (the `write_file` body rides there — §21 exercise gap).
- **reasoning is recorded but never re-fed** (`resp.reasoning` → trajectory only).
- `think: false` is **hardcoded** (main.rs:354 — being reopened, Decision 3 / research/23).

Net: a 32K-window survival hack (`cap` + `[ctx]` overflow signal) bolted onto a raw append log. There is **no
summarization, no cut-point logic, no file-operation tracking, no eviction, no prefix-cache discipline**. When
the transcript outgrows 32K, oMLX silently truncates — and the first thing it drops is the system prompt (the
workpad contract + injected case-law). That is a silent correctness failure, and it is exactly the THRASH-to-
max_iters failure mode the planexec evidence surfaced.

**What "context engineering" must deliver here:** keep the working window small *and* semantically intact across
a long task — so the model always sees (a) its instructions, (b) a faithful summary of what's already done, and
(c) the recent working set in full. The frameworks below have all solved some version of this.

---

## 2. Local teardown — oh-my-pi + pi-mono (Claude, direct read)

The two local clones implement a **complete** compaction subsystem. oh-my-pi is the Rust-core superset; pi-mono
is the earlier/parallel TS design with a written design doc. They share the same structured-summary format and
file-tracking. This is the single highest-signal source — it is the context engineering our harness lacks.

### 2.1 Trigger — token-threshold, reserve-aware
`packages/agent/src/compaction/compaction.ts`:
- `DEFAULT_COMPACTION_SETTINGS = { enabled:true, strategy:"context-full", thresholdPercent:-1, thresholdTokens:-1,
  reserveTokens:16384, keepRecentTokens:20000, autoContinue:true, remoteEnabled:true }`.
- `shouldCompact(contextTokens, contextWindow, settings)` fires when `contextTokens > thresholdTokens`, where the
  effective reserve is `max(floor(contextWindow * 0.15), reserveTokens)`. pi-mono doc states it equivalently:
  **`contextTokens > contextWindow - reserveTokens`**. Reserve leaves room for the LLM's own response.
- Manual `/compact [instructions]` path; optional instructions focus the summary.

### 2.2 Cut-point — walk back from newest, never split a tool result
`findCutPoint(entries, start, end, keepRecentTokens)`:
- Walk **backwards** from the newest message, accumulating `estimateTokens` until `>= keepRecentTokens` (default 20k).
- Land on a **valid cut point**: user / assistant / bash-execution / branch-summary / compaction-summary message.
  **Never cut at a tool result** — it must stay glued to its tool call (else the model sees an orphaned result).
- Everything **before** the cut → summarized; everything **after** → kept verbatim (the recent working set).

### 2.3 Split turns — when one turn alone blows the budget
If a single turn (user msg + all assistant/tool msgs until next user msg) exceeds `keepRecentTokens`, the cut
lands **mid-turn** at an assistant message (`isSplitTurn`). Then **two** summaries are generated and merged:
1. **history summary** (previous complete turns, if any), 2. **turn-prefix summary** (the early part of the split
turn). Prompt `compaction-turn-prefix.md` is explicit: *"This is the PREFIX of a turn that was too large to keep.
The SUFFIX (recent work) is retained… summarize the prefix to provide context for the retained suffix."*

### 2.4 Token estimation — real tokenizer, image-aware
`estimateTokens(message)`: `cl100k_base` native tokenizer (not a byte/4 heuristic). `IMAGE_TOKEN_ESTIMATE = 1200`
per image. Counts text + thinking + tool-call **name** + `JSON.stringify(arguments)`. → Note for us: they DO count
tool-call argument bytes (our `assembled_size` does too); but our `cap` leaves those **uncapped**.

### 2.5 Summary generation — three sizes, iterative update, anti-continuation
- `generateSummary` (maxTokens = `0.8 * reserveTokens`) — the full structured checkpoint.
- `generateShortSummary` (PR-style, `min(512, 0.2*reserveTokens)`) — 2-3 sentences, "what changed" not "what asked".
- `generateTurnPrefixSummary` (`0.5 * reserveTokens`) — the split-turn prefix.
- **Iterative UPDATE**: when a `previousSummary` exists, the update prompt *preserves all prior info and folds in
  new messages*, moving items In-Progress→Done. Compaction is **cumulative**, not a fresh summary each time.
- **Anti-continuation**: the conversation being summarized is wrapped in `<conversation>…</conversation>` tags and
  a system instruction (`summarization-system.md`): *"Do NOT continue the conversation. Do NOT respond to questions…
  Output ONLY the structured summary."* — stops the summarizer from trying to *solve* the task.

### 2.6 The structured-summary contract (the actual format)
`compaction-summary.md` mandates these sections (omit if N/A): **Goal · Constraints & Preferences · Progress
(Done/In-Progress/Blocked) · Key Decisions · Next Steps · Critical Context · Additional Notes**. Hard rules baked
into the prompt:
- *"preserve exact file paths, function names, error messages, and relevant tool outputs or command results."*
- *"include repository state changes (branch, uncommitted changes)."*
- *"If conversation ends with an unanswered question / request awaiting the user… preserve that exact question."*
This is a **handoff document for another LLM to resume** — the design treats every compaction as a context
checkpoint that a *fresh* model could pick up cold.

### 2.7 File-operation preservation — the cumulative touch-list
`extractFileOperations` → `upsertFileOperations(summary, readFiles, modifiedFiles)`. Every summary is appended with
the cumulative set of files read / modified (prompt `file-operations.md` renders `<read-files>` / `<modified-files>`
XML blocks). Tracking **accumulates across compactions** — files touched 5 compactions ago are still listed. This
is the single most reused-state-bearing artifact in a coding session and it is preserved by construction.

### 2.8 Reload / what the LLM sees after compaction
A `CompactionEntry { summary, firstKeptEntryId, tokensBefore, details:{readFiles,modifiedFiles} }` is appended.
The session reloads as: `system prompt · summary · messages from firstKeptEntryId onward`. On repeated compactions
the summarized span starts at the **previous** compaction's kept boundary (so survivors get re-summarized, not
lost), and `tokensBefore` is recomputed from the rebuilt context. Branch summarization (`/tree`) reuses the exact
same machinery on a different span (old-leaf → common-ancestor).

### 2.9 Prefix-cache discipline — append-only context
`packages/agent/src/append-only-context.ts`: `StablePrefix` freezes system prompt + tool defs (fingerprinted) for
max provider prefix-cache hits; `AppendOnlyLog` only grows (`replaceTail()` reserved for compaction);
`AppendOnlyContextManager.syncMessages` detects a compaction via array-shrink and rewrites in place via an FNV
rolling digest. Design comment: *"messages only grow; prior turns are never re-serialized… only the user's new
message delta is a cache miss each turn."* → KV-cache stability is a **first-class** design constraint, traded off
against compaction (which necessarily busts the cache once).

### 2.10 Tool-output MINIMIZATION — the uncapped-tool-result answer (`pi-shell/src/minimizer/`)
A separate, **opt-in** engine that compresses a command's stdout/stderr **before it ever enters the transcript**
(distinct from compaction, which acts later on the whole window). This directly addresses our uncapped-tool-result
gap — and it does it far more surgically than our blunt `cap`:
- **Per-program filters** (`git`, `cargo`, `gradle`, listing=`ls`/`tree`/`find`/`grep`/`rg`/`cat`, `ruby`, `cpp`).
  e.g. grep output → `"grep: N matches in M files"` + top-4-per-file + `"… K more"` tails.
- **Structural safety via `brush-parser` (`plan.rs`)**: classify the command. **Single simple command** → safe to
  minimize. **Piped** (`foo | bar`) → **passthrough** (user is parsing the output — rewriting it is a correctness
  bug). **Compound** (`a && b`, `;`, `&`) → passthrough. Parse-fail → passthrough. *The minimizer never touches
  output that something downstream consumes.*
- **Fail-safe**: filter panics are caught (`catch_unwind`) → passthrough. Over `max_capture_bytes` (4 MiB) → passthrough.
  Minimization can **never** be the reason a command loses output.
- **Stash-and-reference**: when a filter rewrites, the **original** is carried in `original_text`; the session layer
  persists it via an `ArtifactManager` and splices an **`artifact://<id>`** reference into the visible text. The
  agent sees the compressed view but can **retrieve the full original on demand**. (This is the key idea our `cap`
  lacks: lossy in the window, lossless on disk, addressable.)
- **Trust gate**: agent-controllable settings file is honored only if its `xxHash64` matches a supplied hash.

### 2.11 Local-teardown technique catalog (the menu)
| # | Technique | Source | Maps to our gap |
|---|-----------|--------|-----------------|
| L1 | Token-threshold trigger (reserve-aware) | omp/pi-mono compaction | we have none — only a byte `[ctx]` warning |
| L2 | Backward cut-point, never split tool result | findCutPoint | we keep *everything*, no cut |
| L3 | Split-turn → prefix+history merge | turn-prefix | n/a until single turns are huge |
| L4 | Real tokenizer estimate (cl100k, image-aware) | estimateTokens | we use raw bytes |
| L5 | Structured handoff summary (Goal/Progress/Decisions/NextSteps/CriticalCtx) | compaction-summary.md | **we have nothing — top candidate** |
| L6 | Iterative cumulative update (preserve+fold) | update-summary | — |
| L7 | Anti-continuation framing (`<conversation>` + system) | summarization-system | — |
| L8 | Cumulative file-op touch-list | upsertFileOperations | **directly fixes "model forgets what it touched"** |
| L9 | Short PR-style summary | short-summary | for land/log artifacts |
| L10 | Append-only + StablePrefix (KV-cache) | append-only-context | our `messages.clone()` already mostly append-only; formalize |
| L11 | Per-program tool-output minimizer | pi-shell/minimizer | **fixes uncapped tool_call + blunt `cap` on results** |
| L12 | Structural safety (don't minimize piped/compound) | plan.rs | our `cap` is blind to this — could truncate piped output mid-parse |
| L13 | Stash-and-reference (`artifact://id`, lossless on disk) | minimizer + ArtifactManager | **upgrade path for `cap`: addressable elision** |

---

## 3. Web survey — other frameworks + literature (background workers)

> Three workers dispatched in parallel (Hybrid method). Findings fold in here on completion.

### 3.1 OpenCode — *(worker a7b63ca29421473f2, COMPLETE)*
Source: `sst/opencode` (TS). Three distinct context layers + a subagent escape hatch.

**(a) Compaction = summary-anchor + protected-tail** (`session/{overflow,compaction}.ts`, `core/session/compaction.ts`).
Trigger: `total_tokens >= usable` where `usable = limit.input - reserved`, `reserved = min(COMPACTION_BUFFER=20k,
maxOutput)`; checked after every assistant turn + as a catch on a streaming `ContextOverflowError`; manual `/compact`.
On fire: summarize history up to a boundary, store as a `summary:true` message, and on later turns feed only
`[compaction-user · summary · …protected tail… · continue-user]` (`filterCompacted()`); everything before the
boundary (incl. *prior* summaries) is suppressed. **Protected tail** = last `tail_turns` (default 2) that fit
`preserve_recent_tokens = min(8000, max(2000, usable*0.25))`. **`SUMMARY_TEMPLATE` = Goal · Constraints&Prefs ·
Progress(done/in-prog/blocked) · Key Decisions · Next Steps · Critical Context · Relevant Files** — *the same
7-section handoff shape as oh-my-pi §2.6 and pi-mono*. The summarizer is a named `compaction` agent using its
configured model or **falling back to the same model that overflowed** (no cheap-model default — that's a knob we'd set).

**(b) Tool-result handling — THREE layers (the richest signal for our gaps):**
1. **Live truncate-to-disk-with-hint** (`tool/truncate.ts`): every big tool output (shell/grep/glob/read) over
   `MAX_LINES=2000` or `MAX_BYTES=50KB` is written to a temp file (kept 7 days); the result keeps a preview + a
   **hint**: *"Use Grep to search the full content or Read with offset/limit"* (or delegate to the explore agent).
   head by default; shell uses tail. `read` itself paginates (`offset`/`limit`, `MAX_LINE_LENGTH=2000`). → **This is
   OpenCode's version of oh-my-pi's `artifact://` (L13): lossy in-window, lossless on disk, model re-reads on demand.**
2. **Backward-scan pruner** (`compaction.prune`, opt-in) — *a cheaper-than-compaction pressure valve*: scan tool
   parts newest→oldest, skip last 2 user turns, stop at the compaction boundary, accumulate token estimates; once
   total > `PRUNE_PROTECT=40k`, mark older tool results compacted if the prunable amount > `PRUNE_MINIMUM=20k`.
   Compacted results are replaced with the literal `"[Old tool result content cleared]"` — **but the tool call's
   name+args are KEPT** (the model still knows *what* it ran, just not the bulky output). Protected tools: `["skill"]`.
   → **directly fixes our gap, and inverts it**: OpenCode prunes *results* but always keeps args; we leave *args*
   uncapped and bluntly `cap` results. The right design caps/prunes both, keyed off tokens not raw bytes.
3. **Compaction-time cap**: during summary generation each tool output is hard-capped to `TOOL_OUTPUT_MAX_CHARS=2000`.

**(c) On-demand instruction injection** (`session/instruction.ts`) — *the novel idea not in omp/pi-mono*: when the
agent **reads a file**, walk upward from that file's dir and attach nearby `AGENTS.md`/`CLAUDE.md` — **once per
message** (`claims: Map<MessageID,Set<path>>` dedupes). Context pulled in **lazily by relevance** as the working set
is touched. Session-start injects global `~/.claude/CLAUDE.md` + project `AGENTS.md` (findUp, first match wins).
**No repo map** — OpenCode injects no tree/ctags structure; the model explores via tools (or the explore subagent).

**(d) Reasoning** is re-fed **only same-provider** (Anthropic signed-thinking needs its signature), downgraded to
plain text cross-provider, stripped during compaction. → **validates our record-but-never-refeed** for local Qwen
(no signed-thinking chain to preserve; re-feeding it would just burn budget).

**(e) Subagents** (`tool/task.ts`): `task{description,prompt,subagent_type}` spawns an **isolated child session**
(own window, derived permissions, optional background, resumable). Built-ins: `build`/`plan`/`explore`/`compaction`.
The `explore` read-only subagent is what processes the disk offloaded truncation files — keeping the parent window clean.

**Honest finding (worker's gap analysis):** OpenCode *also* has **no semantic curation** — its working set is purely
recency-based (token count is the only signal). It hasn't "solved" context engineering; **compaction + truncate-to-disk
+ prune are release valves**, not curation. This matters for §4: the convergent industry answer is *bounded recency +
structured summary + offload*, NOT semantic selection. That tempers Decision 2's "curated re-feed" lever.

### 3.2 Hermes / Nous agentic — *(worker a31f0a91347ba1e41, COMPLETE)*
Two artifacts: the **reference repo** (`Hermes-Function-Calling/functioncall.py`) = naive growing prompt, **no
truncation**, only `max_depth=5` (a floor, like our raw append) — and the **production `hermes-agent`**, which has
the most sophisticated compressor of any source surveyed.

**Reasoning re-feed — Nous's own default is DROP** (the strongest external corroboration of our posture):
- Hermes 4 chat template ships `keep_cots=False` by **default** → `<think>…</think>` CoT is **stripped** before the
  turn enters history. `keep_cots=True` is flagged "to experiment with", **not** a production setting.
- The GOAP `<scratch_pad>` planning block (Goal/Actions/Observation/Reflection) is stripped-before-refeed in third-
  party integrations (LocalAI). Nous gives no re-feed guidance; the reference repo accumulates everything (demo only).
- → directly corroborates research/23 + our record-but-never-refeed. **But note the tension with Decision 3**: Nous
  drops CoT for *reliability*, while Gary's bounded-thinking bet is that more thinking *helps quality*. Reconcile in §4:
  "let the model think, bound the budget, **record but don't re-feed** the thinking" is consistent with *both* — the
  budget buys quality on the turn that produces it; dropping it from re-feed protects the window. (See §4.)

**`hermes-agent` ContextCompressor — 4-phase, fires at 50% of window** (`agent/context_compressor.py`):
- **Phase 1 (no LLM, cheap):** old tool results >200 chars outside the protected tail → **one-line summaries**
  (`[terminal] ran npm test -> exit 0, 47 lines`); duplicate outputs → back-reference placeholders; images →
  `[screenshot removed]`; **tool-call arguments JSON-truncated** to prevent provider 400s. → **this is the direct,
  named fix for our uncapped-`tool_call`-args gap, and it's smarter than a byte cap (semantic 1-liner, not head/tail).**
- **Phase 2 (boundary):** head (`protect_first_n=3`: system + first exchange) + tail (`protect_last_n=20` or token
  budget) + middle; `_align_boundary_backward()` **never splits a tool-call from its result** (== omp `findCutPoint`).
- **Phase 3 (LLM summarize middle):** single aux-model call, template = **Goal · Constraints&Prefs · Progress
  (Done/In-Prog/Blocked) · Key Decisions · Relevant Files · Next Steps · Critical Context** (the SAME 7-section shape
  — now confirmed across **four** independent codebases). Budget 20% of content, min 2000, ceiling 12K; iterative update.
- **Phase 4 (reassemble):** head + summary labeled **`[CONTEXT COMPACTION — REFERENCE ONLY]`** + verbatim tail; the
  model is told **the latest user message overrides anything in the summary** (summary is background, not authority).
- Defaults: `threshold:0.50`, `target_ratio:0.20`, `protect_first_n:3`, `protect_last_n:20`. **Anti-thrashing:** skip
  if the last two compressions each saved <10% (locks off until `/new`).
- **⚠ Failure mode (critical for §5):** if the auxiliary summarizer's window < main model's, `_generate_summary()`
  returns `None` and **the middle turns are silently dropped** — the docs name this the most common cause of degraded
  compaction. A summarizer that *eats* context unannounced is the exact silent-correctness class this project exists to kill.

**Tool-output truncation** (`hermes-agent` config, infra-layer not prompt): `max_bytes=50000` keeping **first 40% /
last 60%** (tail-biased — errors live at the end), `max_lines=2000`, `max_line_length=2000`, `file_read_max_chars=
100000` (over-limit reads **rejected** with offset/limit guidance, not silently cut), files **read-twice-unchanged →
stub**. → Note our `cap` is **2/3 head, 1/3 tail** — the *opposite* bias. Hermes' tail-bias matches our own rationale
in `refeed.rs` ("the actionable part is usually at the end"); we under-weight the tail. Worth flipping/measuring.

**Budget-pressure injection** (Issue #414): at 70%/90% of `max_turns=90`, inject an ephemeral "consolidate / wrap up
NOW" message into the **API copy only** (never persisted → prompt-cache structure preserved). → a softer cousin of our
planexec nudge + loopgate; the ephemeral-not-persisted discipline is the portable idea.

**Other (lower priority for us):** lazy tool-schema loading (proposed, P3 — names+1-liners then full schema on demand,
30-70% savings but +round-trip; not worth it at our 3-tool scale); skill-library procedure extraction to SQLite (our
memory/evolve pillar); subagent delegation via ThreadPoolExecutor returning summary-only ("zero-context-cost turns").

**`<tool_call>`/`<tool_response>` schema** is Hermes-tokenizer-specific (single-token tags, baked in training); not
portable to Qwen, but the system-prompt-`<tools>` + `tool`-role-result surface is the canonical shape we already match.

### 3.3 Claude Code + Aider + literature — *(worker afcd347f7a35ad031, COMPLETE)*

**Claude Code** (proprietary; from Anthropic docs + SDK + engineering posts):
- **A1 — Compaction** (`compact_20260112`): auto-trigger at **150K tokens** (SDK `compaction_control` lets you set
  it lower, e.g. 100K). Summarization uses **the request's own model** (no cheap-model option at the public API level;
  the SDK allows a `"model"` override). The summary is told to keep **"architectural decisions, unresolved bugs,
  implementation details + the 5 most recently accessed files"** and drop **"redundant tool outputs"**. Empirical:
  **58.6% token reduction** per firing. **3-failed-compactions circuit breaker** (stops retrying a compaction that
  doesn't shrink the window — the same anti-thrash instinct as Hermes' <10% skip).
- **A2 — Sub-agent isolation** (the Task tool): a child runs in a **separate context** and returns **only its final
  message** (~420 tokens) vs. the ~6,100 tokens it read to produce it — a **14:1** context saving. Children **cannot
  recurse** (no sub-sub-agents); the parent's only interface is the prompt string in / final message out. → identical
  shape to OpenCode §3.1(e) and our own `explore.rs` probe.
- **A3 — Just-in-time retrieval:** startup context is small (~7,850 tokens); the model pulls files on demand rather
  than pre-loading. In a naive baseline **96.3% of context was file-read output** — i.e. the dominant consumer is tool
  output, which is exactly what offload/clearing targets (corroborates our uncapped-result priority).
- **A4 — Tool-result clearing** (`clear_tool_uses_20250919`, `keep=3` default): once tool results age past the keep
  window they're cleared from the re-fed context — **and `clear_tool_inputs` clears the tool-call _arguments_ too**
  (the **direct, named fix for our uncapped-args gap**, same as Hermes Phase-1). No inference cost (pure deletion);
  **lossless-because-refetchable** (the model re-runs the tool if it needs the data). One firing freed **163,817 tokens**.
- **A5 — Structured note-taking:** a persistent memory tool the model writes to deliberately (external scratchpad that
  survives compaction) — our memory/evolve pillar's analogue.

**Aider** (open source; the edit-format + repo-map reference):
- **B1 — Repo map** (`repomap.py`): tree-sitter extracts symbol signatures across the repo, **PageRank** ranks them by
  reference graph, rendered as a **signatures-only skeleton** within a **1024-token default** budget. Chat-added files
  weighted **50×**, mentioned identifiers **10×**. → the one source with a real **structural** context primitive; but
  it's a *retrieval* aid, not a re-feed mechanism, and it costs a tree-sitter parse per language. (Wu Wei: out of scope
  for our re-feed fix; revisit if/when a repo-map pillar is justified.)
- **B2 — ChatSummary:** history compressed to a **1–2K-token** budget; **no** automatic threshold-triggered compaction
  (Aider's sessions are short by design).
- **B3 — Edit formats:** the **udiff** format lifted GPT-4-Turbo's edit success **20% → 61%** empirically and **cut
  "laziness" (elided code) ~3×**. → not context-engineering per se, but a reminder that *output* format is as load-bearing
  as input curation; our `write_file`-whole-body is the lazy-prone shape udiff exists to fix (note for a future tool pillar).

**Literature** (peer-reviewed + lab engineering notes — the grounding layer):
- **C1 — Lost-in-the-Middle** (Liu et al., *TACL* 2024): retrieval/QA accuracy is **U-shaped** in the position of the
  relevant token — models attend strongly to the **head and tail** and **degrade ~30% in the middle**. This is the
  single strongest literature signal for us: it **directly motivates position-aware assembly** (put instructions at the
  head, the live working set at the tail, the summary in the middle where degradation is *expected and acceptable*). A
  near-free win — it costs only ordering discipline. Validates the head+summary+tail shape every framework converged on.
- **C2 — Anthropic, "effective context engineering":** the goal is the **"smallest set of high-signal tokens"**; context
  is a finite budget with diminishing returns ("context rot"). Frames compaction + just-in-time retrieval + sub-agents as
  the three levers (exactly A1/A3/A2).
- **C3 — Anthropic, "building effective agents":** orchestrator-workers pattern (our explore/parallel pillar).
- **C4 — Cognition, "don't build multi-agents":** counter-weight to naive fan-out — **"share full traces, not just
  individual messages"**; sub-agents that see only a slice make incoherent decisions. They use a **fine-tuned compression
  model** for history. → tempers A2/explore: isolation saves tokens but costs coherence; the parent must pass enough trace.
- **C5 — MemGPT (Packer et al.):** three-tier memory (main-context / external recall / archival) with the LLM paging
  between them via tool calls — the theoretical frame under "external memory + just-in-time retrieval".
- **C6 — Atomic task decomposition:** break work into **3–5 sub-tasks each sized to fit one window** — the literature
  backing for **Gary's own reframe** ("we haven't broken tasks down atomically enough"). Decomposition *is* a context-
  engineering technique: a task that fits the window never needs compaction.

**Convergence check (now 4 codebases + literature):** the 7-section structured summary (Goal/Constraints/Progress/
Decisions/NextSteps/RelevantFiles/CriticalContext) appears in oh-my-pi, pi-mono, OpenCode, **and** Hermes; Claude Code's
"keep architectural decisions + unresolved bugs + recent files, drop redundant tool output" is the same shape stated as
a policy. Lost-in-the-Middle explains *why* the universal head+summary+tail layout works. **No source does semantic
curation of the working set** — all four use bounded recency + structured summary + offload/clearing. Aider's repo-map is
the lone structural primitive, and it's retrieval, not re-feed.

**Worker's tiering (apply-now → later):** Tier-1 (cheap, high-value, fixes our exact gaps) = **tool-result/arg clearing**
(A4) + **position-aware assembly** (C1); Tier-2 = sub-agent isolation (A2), compaction-with-custom-instructions (A1),
atomic task decomposition (C6); Tier-3 (defer, heavier) = repo-map (B1), external memory tool (A5/C5).

---

## 4. Synthesis

### 4.1 The convergent answer (universal across 4 codebases + literature)
Every surveyed framework — oh-my-pi, pi-mono, OpenCode, Hermes — and the literature converge on the **same three-part
mechanism**, with **no semantic curation anywhere**:

1. **Bounded recency, position-aware.** Protect the **head** (system prompt + first exchange) and the **tail** (recent
   working set, last ~2 turns / a token budget); summarize the **middle**. Liu et al. (C1, Lost-in-the-Middle) is the
   *why*: attention is U-shaped, the middle degrades ~30%, so the middle is exactly where a lossy summary belongs and the
   head/tail are exactly what must stay verbatim. The head+summary+tail layout is not taste — it's the shape the
   degradation curve dictates.
2. **Structured summary of the middle.** The **same 7-section handoff** (Goal · Constraints&Prefs · Progress[Done/
   InProg/Blocked] · Key Decisions · Next Steps · Relevant Files · Critical Context) in all four codebases; Claude Code
   states it as a keep/drop policy. Always: anti-continuation framing ("summarize, don't solve"), iterative cumulative
   update (preserve+fold, never fresh), cumulative file-op touch-list, "summary is REFERENCE-ONLY, latest message wins".
3. **Offload / clear tool output.** The dominant context consumer is tool output (Claude: 96.3% of a naive baseline).
   Three flavours, all "lossy in window, lossless on disk/refetchable": truncate-to-disk + re-read hint (OpenCode),
   `artifact://` stash (oh-my-pi), clear-and-refetch (Claude `clear_tool_uses`). All keep the tool-call **name** so the
   model knows *what* it ran.

**Framework-specific, NOT for us:** Hermes single-token `<tool_call>` tags (tokenizer-baked); Aider repo-map (tree-sitter
+PageRank — a *retrieval* primitive, not re-feed; defer); signed-thinking re-feed (Anthropic-only); lazy tool-schema
loading (only pays at many tools — we have 3). **The honest negative result (OpenCode + Hermes workers both flagged it):**
nobody has solved semantic working-set selection. This **tempers Decision 2** — the re-feed redesign should adopt the
mechanical convergent answer, **not** chase a semantic curator that the state of the art doesn't have.

### 4.2 Our gaps → the minimal subset that fixes them (impact ÷ effort)
Our re-feed path (§1) has six gaps. Mapped to the convergent answer, the minimal fixing subset, ordered:

| # | Fix | Kills which gap | Source | Effort |
|---|-----|-----------------|--------|--------|
| **F1** | **Protect the head + position-aware assembly** — guarantee the system prompt (workpad contract + case-law) is *never* the dropped item; assemble head · [middle] · tail in that order. | silent 32K truncation dropping the system prompt (the THRASH-to-max_iters mode) | C1 + all | **low** (ordering + a floor) |
| **F2** | **Token-threshold compaction → 7-section structured summary of the middle** (anti-continuation, iterative, cumulative file-list); replaces oMLX's silent truncation with an explicit checkpoint. | no summarization at all (L5 — the top candidate) | omp/pi-mono/OpenCode/Hermes | **med** |
| **F3** | **Tool-output offload** (truncate-to-disk + re-read hint, or `artifact://`); keep a head+tail preview, keep the tool-call name. | blunt byte-`cap` on results; the uncapped *result* path | OpenCode truncate.ts / omp L13 / Claude A4 | **med** |
| **F4** | **Cap/clear tool-call ARGS too** (not just results) — the `write_file` body rides here. Only clear what's **refetchable** (the file is on disk in the worktree). | uncapped tool_call arguments (§21 exercise gap) | Hermes Phase-1 / Claude `clear_tool_inputs` | **low** |
| **F5** | **Cumulative file-op touch-list** folded into every summary. | "model forgets what it touched" | omp L8 | **low** |

**Defer (Wu Wei):** real tokenizer (raw-byte is a working proxy; note the bias, don't block on it); split-turn merge
(only when a single turn > budget — not our regime yet); StablePrefix/KV-cache formalization (our `messages.clone()` is
already mostly append-only; an optimization, not a correctness fix); sub-agent isolation (we already have `explore.rs`);
repo-map + external-memory tool (separate pillars).

**Tail-bias correction (measure, don't assume):** our `cap` is 2/3-head/1/3-tail; Hermes is 40/60 **tail**-biased
(errors live at the end) and our own `refeed.rs` comment agrees the actionable part is the tail — yet the code is
head-biased. For a single tool **result**, flip toward the tail (or better: F3 offloads it and the preview keeps both
ends). For the **transcript**, F1 already protects both head and tail. This is a one-number change to measure, not a
redesign.

### 4.3 Reconciling re-feed with Decision 3 (bounded thinking)
Three independent corroborations say **drop reasoning before re-feed**: Hermes `keep_cots=False` (default), OpenCode
strips-on-compact + re-feeds reasoning same-provider-only, Claude Code's separate `clear_thinking` primitive. Decision 3
says **thinking helps quality**. These are **not** in tension once you separate the two axes:

- **Production axis (how much to think *this turn*):** Decision 3 — let the model think, **bound the budget** (the
  adaptive think-OFF JSON self-assessment → `thinking_budget:N`). The budget buys quality on the turn that produces the
  output. *Orthogonal to re-feed.*
- **Re-feed axis (does thinking re-enter context next turn):** **record but never re-feed** — corroborated 3×. Re-feeding
  CoT just burns the window (and for local Qwen there's no signed-thinking chain to preserve, per OpenCode §3.1d).

So the unified rule — **"let it think, bound the budget, record-but-don't-re-feed the thinking"** — satisfies both
decisions with no contradiction. The bounded-thinking mechanism (the next build) and this re-feed redesign compose
cleanly: thinking controls per-turn cost; re-feed controls cross-turn window; reasoning lives in neither re-fed context.

## 5. Adversarial review

**Class: silent correctness failure.** Per the Process-gate this triggers REVIEW, because *every mechanism in §4.2 is
itself a potential silent-correctness failure* — a summarizer that drops a constraint, an offload that loses an error, a
circuit-breaker that quietly gives up. This is the exact class the harness exists to kill, so the review centers on
**"where can each fix fail silently, and what makes it fail loud instead."**

| Fix | Silent-failure mode | Forced-loud mitigation (the design rule) |
|-----|---------------------|------------------------------------------|
| **F2 summarizer** | Drops a success-criterion / pending question / exact error → the model resumes against a lossy spec and produces subtly-wrong output (Hermes names this the #1 degradation cause). | (a) anti-continuation + "preserve exact pending question/paths/errors" framing (omp); (b) **the downstream artifact gate is the backstop** — a summary that dropped a requirement → wrong output → `tests_green`/gate **red** (same safety-net logic as research/23's escalation). The summary is never the sole authority: "[REFERENCE ONLY], latest message wins." |
| **F2 summarizer-window overflow** | The **Hermes centerpiece failure**: aux summarizer window < main → `_generate_summary()` returns partial/None → **middle turns silently vanish**. For us the summarizer *is* the same oMLX/window, so a long middle can overflow summarization itself. | **Hard rule: summary generation that errors or overflows ABORTS compaction and emits a `[ctx]`-class signal — never proceeds with a truncated summary.** Fail loud, never silent. (This is the single most important rule in the doc.) |
| **F3 offload** | Output goes to disk, model never re-reads, misses the real error living in it. | Preview keeps **both ends** (head+tail per C1; tail-biased since errors cluster late) + an explicit, imperative re-read hint with the exact `artifact://`/path + offset/limit. Never offload silently — the hint is mandatory. |
| **F3/F4 clearing** | Clearing args/results that are **not** refetchable = real data loss (a pure-compute result with no disk copy). | **Clear only what is provably recoverable** (a file in the worktree; a tool that's deterministic to re-run). Non-refetchable output is summarized (F2), not cleared. The `write_file` body is safe to clear *because the file is on disk*; a one-shot computation is not. |
| **F1 reorder** | Position-aware assembly orphans a `tool_result` from its `tool_call` → provider 400 or model confusion. | Honour the universal **never-split-call-from-result** rule (omp `findCutPoint`, Hermes `_align_boundary_backward`). Adjacency is an invariant the assembler asserts, not a best-effort. |
| **anti-thrash breaker** | After N low-yield compactions we lock off (Hermes <10%×2, Claude 3-strike) — but if the window keeps growing we're back to silent truncation. | The breaker must **escalate** (signal / stop the run with a clear outcome label), **not** silently continue. A breaker that hides an unrecoverable overflow is worse than no breaker. |

**Through-line (the one rule that covers all of them):** *a context mechanism may be lossy, but it may never be silently
lossy.* Any compaction that can't shrink, any summary that can't be generated within budget, any offload whose artifact
isn't recoverable → **emit a signal and/or stop with an explicit outcome**, exactly like the existing `[ctx]` warning and
the `looped`/`stalled` outcome labels. This is consistent with the whole harness spine (gate by artifact, explicit
labels, fail loud) and it is the acceptance criterion every §6 slice must meet.

**Residual risk accepted:** the summary is fundamentally lossy and no framework has solved that; we lean on the
downstream artifact gate as the catch (validated pattern from research/23). We are **not** claiming semantic fidelity —
we're claiming *bounded, signalled, recoverable* loss. That is the honest, evidence-backed posture.

## 6. Recommendation → wiki

**Verdict: adopt the convergent mechanism, build it as a scoped slice AFTER the bounded-thinking design (Gary's
sequencing), under one hard invariant.** The re-feed redesign is **not** a semantic curator (the field doesn't have one
and chasing it violates Wu Wei + tempered Decision 2). It is: **protect head+tail · summarize the middle · offload tool
output · cap both args and results · fail loud, never silent.**

**The invariant (non-negotiable, from §5):** *no silent loss.* Every lossy step emits a signal or an explicit outcome
label when it can't do its job.

**Proposed slices (smallest correctness-bearing first; each gates by a number/artifact):**
- **Slice 1 — F1 (protect the head).** Guarantee the system prompt is never the dropped item; assert tool_call↔result
  adjacency on assembly. *Gate:* a unit test that proves the system prompt survives an over-window transcript, and a
  reorder never orphans a result. **This alone kills the THRASH-to-max_iters silent failure** — highest impact/effort.
- **Slice 2 — F4 (cap/clear refetchable tool-call args).** Close the uncapped `write_file`-body gap; clear only what's
  on disk. *Gate:* unit test that an over-cap arg is elided in re-feed but the file still exists in the worktree.
- **Slice 3 — F2 (token-threshold compaction + 7-section structured summary).** With the §5 hard rule: summary overflow
  ⇒ abort + signal. *Gate:* a live oMLX run on a long task where the transcript crosses the threshold, a summary row is
  produced, the system prompt + pending question survive, and the task still reaches a green artifact gate.
- **Slice 4 — F3 + F5 (tool-output offload + cumulative file-list).** *Gate:* a big tool output is offloaded with a
  working re-read hint; the file-op touch-list accumulates across a compaction.

**Defer** per §4.2 (real tokenizer, split-turn, StablePrefix, repo-map, external-memory). **Measure** the tail-bias flip
as a one-number change inside Slice 2/3, not a redesign.

**Wiki updates owed (on Gary's go):** new `research/25` index entry; `decisions.md` — record (a) the convergent
mechanism as the chosen re-feed design, (b) the no-silent-loss invariant, (c) the re-feed/Decision-3 reconciliation,
(d) **Rejected:** semantic working-set curation (no SOTA exists; tempered Decision 2); `active-work.md` breadcrumb +
the slice plan as the next build after bounded-thinking.
