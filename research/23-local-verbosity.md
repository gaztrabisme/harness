# research/23 — Local-model verbosity & the effort lever

> Scope: how to make a cheap local model (Qwen3.6-35B-A3B on oMLX) emit **bounded, structured, usable**
> output in the agent loop — bounding the *reasoning*, not just the answer. Follow-on from research/21
> (think-stripping & re-feed bloat) and research/22 (loop gate); those characterized the failure modes,
> this one finds the lever. Three axes: (A) online literature, (B) reference-framework prior art,
> (C) an empirical probe matrix on our actual server. **Verdict up front: default thinking OFF, escalate
> reactively — not a per-turn effort-router, and not a graded token budget.**

## 0. Why this exists (the thesis)

"oMLX needs to work to prove the harness can get a cheap local model to get shit done." Axis-B verbosity
(research/21 §8, research/22 §8) is the thing standing between the 35B and useful agentic work: it spirals
into multi-paragraph deliberation, burns the token budget, and truncates (`stop_reason==Length`) before
producing or validating an artifact. The question is the cheapest reliable lever to stop that.

## 1. The capability surface (what oMLX actually exposes) — VERIFIED

From `GET /openapi.json` (authoritative) + live probes. `/v1/chat/completions` accepts per-request,
**on the wire, no dashboard mutation needed**:

| Knob | Effect (measured) |
|------|-------------------|
| `chat_template_kwargs:{enable_thinking:false}` | thinking fully OFF — `reasoning_content` empty, ~1 token of overhead. **The clean lever.** |
| `thinking_budget: N` | caps `reasoning_content` length, but see §4 (elasticity trap at small N) |
| `guided_grammar` (GBNF) / `structured_outputs` / `response_format` | three forced-format mechanisms |
| `temperature/top_p/top_k/min_p/repetition_penalty/presence_penalty` | full sampling control |
| `seed`, `stop` | standard |
| `max_tokens` | caps **`content`** only — does **NOT** bound `reasoning_content` (§7: think-ON ran to 8091 tokens under `max_tokens=2048`). To bound reasoning, use `thinking_budget` or a wallclock/iter kill. |

Profiles are *also* programmatic (`POST /admin/api/models/{id}/profiles/{name}/apply`, `ModelSettingsRequest{enable_thinking, thinking_budget_tokens, reasoning_parser, ...}`, `GlobalSettingsRequest`) — but admin-authed and stateful. **We don't need them**: per-request wire knobs cover the design. Soft switches `/think`
`/no_think` in the prompt were tested and do **not** work on this build (only `chat_template_kwargs` does) —
consistent with the literature (§2).

Correction to an earlier in-session claim ("no reasoning-budget knob"): that was a wrong field-name guess.
The real field is the flat `thinking_budget`; it is honored.

**Correction 2 (research/26 §9, 2026-06-10) — `thinking_budget` is gated by two per-model flags, NOT pure wire.**
The table row above is wrong that `thinking_budget` needs "no dashboard mutation." It is silently inert unless the
served model has these set in `~/.omlx/model_settings.json` (server reads them **live**, no restart):
- **`thinking_budget_enabled: true`** — when `false` (the prior default), `thinking_budget:N` is **silently ignored**
  (no error, runs to `max_tokens` like the no-budget control). This flag, off, was the root cause of the earlier
  "budget is a no-op" mystery + a fabricated "model was swapped" story (owned in research/26 §9).
- **`reasoning_parser: "qwen_3_5"`** — splits reasoning into the separate `reasoning_content` field; when unset,
  thinking leaks inline into `content` (no clean field boundary for the loop to drop).
With both on, `thinking_budget:N` is a **soft** target (not a hard cap; no exhaustion signal; `max_tokens` is the hard
backstop) — see research/26 §9.1. The §4 "elasticity trap at small N" is the *same* soft-target behavior seen again.
`enable_thinking:false` (the OFF lever) needs **neither** flag — it is pure wire and always works.

## 2. Axis A — online literature & forums

**Disabling thinking.** The canonical, most-reliable method across deployments is
`enable_thinking=False` via `chat_template_kwargs` (OpenAI-compatible: pass in `extra_body`). Soft switches
(`/no_think`) are version-dependent and often ignored — matches our probe.

**Non-thinking sampling for terseness.** Qwen team / practitioners: balanced non-thinking ≈ **temp 0.7,
top_p 0.8** (the low top_p is the terseness lever — constrains to likely tokens, kills the wandering);
some add `presence_penalty ~1.5` / `repetition_penalty ~1.05` to break loops. Our oMLX dashboard is already
on this profile (temp 0.7 / top_p 0.8 / top_k 20 / min_p 0).

**Overthinking is worse in small models, and it's measurable.** Qwen3.5-4B emits ~2.5× the reasoning
tokens of the 27B for the same task — "cheaper per-token ≠ cheaper per-task." On LiveCodeBench, **17.4% of
outputs had truncated thinking** (no closing think tag before the cap), and **84% of those showed >30%
repetition** — the model repeats phrases until tokens run out, never answering. *This is exactly the
research/21 §21 collapse, documented in the wild.*

**The token-elasticity trap (key).** Budget caps are non-monotonic: a 50-token thinking budget → ~86 tokens
out, accuracy held; but a **10-token budget → output jumped to 157 tokens — the model "panicked."** Too-tight
a cap *increases* cost. Enforcement also matters: post-hoc truncation is insufficient (the model never gets
the "stop thinking" signal); forcing the closing-think token at decode level gives a clean transition.

**Function-calling overthinking is mechanistically different — and shorter CoT helps.** In agentic tool use,
over-reasoning shows up as *format erosion and function hallucination*, not abandoned reasoning paths.
Qwen3 reasoning models "develop sophisticated solution patterns via pure text" and **under-call tools**
(use them mainly to double-check their own prose). Direct quote of the practical takeaway: *shorter chains
of thought can improve tool-calling quality.* Papers on point: "Brief Is Better: Non-Monotonic CoT Budget
Effects in Function-Calling Agents" (arXiv:2604.02155); "BudgetThinker" control-token enforcement
(arXiv:2508.17196).

Sources:
- HF: Qwen3.5-9B "How to disable or reduce thinking"; unsloth/Qwen3.5-9B-GGUF reasoning-default discussion
- Zach Mueller, "Limiting Qwen 3's Thinking" (max_thinking_tokens logits processor)
- Moonglade/lyn.one, "Why Qwen3.5 Falls Into Infinite Thinking — and How to Fix It" (the elasticity + 17.4%/84% numbers)
- buildmvpfast, "Token Budget Control Patterns for LLMs / Stop Overthinking"
- Qwen blog, "Qwen3: Think Deeper, Act Faster" (hybrid thinking/non-thinking budget design)
- arXiv:2604.02155, arXiv:2508.17196

## 3. Axis B — reference-framework prior art (how the harnesses we mined handle it)

Searched pi-mono, oh-my-pi, symphony, beads, harness-spike.

- **Reasoning is controlled by a token budget tier, not a cap-and-pray.** pi-mono/oh-my-pi define
  `ThinkingLevel = minimal|low|medium|high|xhigh` → budgets ~1k/2k/8k/16k/32k, passed to the provider
  (Anthropic `budget_tokens`). `AUTO_THINKING` sentinel = per-session auto-detect of the level.
  (`oh-my-pi/packages/coding-agent/src/thinking.ts`, `pi-mono/packages/ai/src/types.ts`.)
- **Verbosity backstops are cheap and universal:** a `"Be concise in your responses"` system guideline
  always appended (`pi-mono/.../core/system-prompt.ts`), and `max_tokens = model.maxTokens / 3` as a hard
  output cap.
- **Structured output via native tool-calling, kept.** Both use native `tool_use`/function-calling with
  TypeBox→schema; **neither hand-rolls a JSON action protocol.** oh-my-pi uses `response_format:
  json_schema, strict:true` *only* for a non-agent task (prompt rewriting) with token-by-token validation +
  fallback. → Counter-signal to the earlier "move off native tool-calling" idea.
- **No anti-repetition / loop detection in either.** We already built that (`loopgate.rs`, research/22) —
  we're ahead of the references here.

Prior-art verdict: the references solve verbosity with **(reasoning budget tier) + (terse guideline) +
(max_tokens fraction)**, on native tool-calling. They do *not* use grammar-as-anti-verbosity. The budget
tier is the relevant idea — but our empirical matrix (§4) shows the *graded* version of it misbehaves on
this model, which is why our recommendation diverges (§6).

## 4. Axis C — empirical matrix on our server

Model: `Qwen3.6-35B-A3B-oQ8-fp16-mtp`, reasoning parser ON, dashboard sampling (temp 0.7/top_p 0.8).

**4a. `thinking_budget` sweep** (prompt: count r's in "strawberry"):

| budget | reasoning chars | content chars | completion tokens |
|--------|----------------|---------------|-------------------|
| none   | 1512 | 199  | 568 |
| **0**  | **0**    | 237  | **105** |
| 64     | 134  | **1329** | 502 |
| 256    | 503  | 1218 | 602 |
| `enable_thinking=false` | 0 | 338 | **143** |

**Finding: the elasticity trap reproduces exactly.** A tight-but-nonzero budget (64) doesn't save tokens —
it *relocates* the spiral into `content` (1329 chars, 502 tokens, barely under baseline 568). Only the
binary extremes are clean (budget 0 / think-off ≈ 105–143 tokens, short answer). The graded middle is the
worst of both worlds. Independent confirmation of the §2 literature.

**4b. `response_format` routing** (earlier probe): under `response_format: json_schema` the schema
constrains `content` to valid JSON, but the deliberation moves into `reasoning_content` (328 tokens for a
2-field object); `json_object` was 780. So structured output **relocates** verbosity rather than removing
it — and kills the "invalid/unclosed JSON = reasoning overran → retry" idea (the JSON stays valid no matter
how long the model thinks). That earlier design fork is dead.

**4c. Tool-calling: think-ON vs think-OFF** (3 tools: read_file/write_file/bash):

| task | think-ON | think-OFF | verdict |
|------|----------|-----------|---------|
| read src/main.rs | 122 tok, `read_file{path}` | **26 tok**, identical | OFF 4.7× cheaper, same |
| list dir | 289 tok, `bash{ls -la}` | **27 tok**, identical | OFF 10.7× cheaper, same |
| write notes.txt | 96 tok, `write_file{…}` | **40 tok**, identical | OFF 2.4× cheaper, same |
| "what is 2+2" (no-op) | 121 tok, no tool, "4" | **8 tok**, no tool, "4" | OFF 15× cheaper, same |
| find TODO + first match | 185 tok, `grep -rn …` (forgot "first") | **43 tok**, `grep … \| head -n 1` | **OFF cheaper AND more correct** |

**Finding: for single-step agentic dispatch, think-OFF is strictly better** — identical-or-better tool
calls at 3–15× fewer tokens, `reasoning_content` empty every time (zero Axis-B surface), correct
no-tool-call discrimination on the no-op. The one case where the answers differed, think-OFF was *more*
faithful to the instruction. This matches the §2 function-calling literature ("shorter CoT improves
tool-calling").

## 5. Synthesis — the three failure modes vs the lever

| Failure mode | Lever that kills it |
|--------------|---------------------|
| Re-feed bloat (research/21) | already fixed (refeed caps + no reasoning re-feed) |
| Axis A cross-turn tool loop (research/22) | already fixed (`loopgate.rs`) |
| Axis B intra-turn collapse — repetition & over-deliberation | **think-OFF by default** (this doc) |

`thinking_budget` is real but the graded version is a trap (elasticity). `response_format`/grammar only
*relocate* verbosity. The single clean, measured lever is **binary thinking control**, and on agentic
single-step work the model is *better* with thinking off.

## 6. Recommendation — reactive escalation, default think-OFF

**Decision: reactive escalation beats a predictive effort-router.**

- **Default every agent turn to `enable_thinking=false`.** It nails single-step dispatch at 3–15× lower
  cost with zero spiral risk, and discriminates no-op turns correctly.
- **Escalate to think-ON only on a detected failure** of that turn — the signals already exist:
  `stop_reason==Length` (Axis-B truncation, research/22 §8), loopgate strike, or a downstream gate
  (`tests_green`) failing. Retry *that turn* with thinking enabled.
- **Why not the per-turn triage call (Gary's first instinct):** think-OFF is so cheap (~8–43 tok) that a
  dedicated classifier call would *add* tokens + latency for a decision that is "OFF" ~always; a wasted
  cheap first attempt costs less than the triage call would. Reactive is simpler (no router to build/tune)
  and Wu-Wei-correct. **§7 settled the router's fate: think-OFF *does* fail silently ~30% on generative code,
  but a predictive triage call cannot see a content-level near-miss any better than a reactive one can — the
  failure isn't a detectable spiral. The router stays shelved; the downstream artifact gate is the catch.**
- **Do not use a graded `thinking_budget` ladder** (elasticity trap). The escalation is binary: OFF →
  (on failure) ON. **If you escalate, bound the ON turn with `thinking_budget` (and a wallclock/iter kill) —
  `max_tokens` does NOT cap reasoning (§7 finding 2): naive think-ON ran to 8091 tokens / 830 s.**
- **Keep the cheap backstops** the references use: a terse system guideline (already partly in
  `build_system_prompt`) and a sane `max_tokens`.

This stays keyed off **universal signals** (research/22 §3 discipline): the *trigger* to escalate is
`Length`/loop/gate-failure (model-agnostic); only the *mechanism* (`enable_thinking`) is Qwen-specific and
stays quarantined in the provider/request layer.

## 7. Residual measurement — silent-failure rate (RUN 2026-06-10)

The §6 gate: *how often does think-OFF fail **silently** on a genuinely multi-step task* (one that trips no
universal signal — not `Length`, not a loop — yet is wrong)? Measured it. Throwaway oMLX exercise (`/tmp`,
never landed): a real generative task — a Kernighan-style subset-regex matcher (`.`,`*`,`^`,`$`, literals) —
graded against a hidden 25-case battery. Classify each run: **GREEN** all pass; **SILENT_FAIL** wrong but
`fr=stop` & no loop (the dangerous case); **LOUD_FAIL** length-cut / no-code / exec-raised / client-timeout
(a signal exists). N=10 think-OFF, N=8(+1) think-ON.

| Condition | GREEN | SILENT_FAIL | LOUD_FAIL | completion tokens |
|-----------|-------|-------------|-----------|-------------------|
| **think-OFF** (n=10) | 6 (60%) | **3 (30%)** — 22/25, 23/25, 24/25 | 1 (exec raised) | 904–2409, all `content` |
| **think-ON** (n=8+1) | 0 | 0 | **9/9 client-timeout >200s** | 777–8091 server-side, 50–830 s/run |

**Three findings:**

1. **think-OFF silent-failure ≈ 30% on generative code.** Near-miss artifacts (off-by-one on 1–3 of 25 edge
   cases) that pass structurally, emit `fr=stop`, don't loop → **no universal signal catches them.** This is
   real and material — the predictive router cannot help here either (the failure is in the *content*, not a
   detectable spiral), so it does **not** resurrect the router idea (§6 stands).
2. **think-ON is operationally unusable on this task — and `max_tokens` does NOT bound reasoning.** Every
   think-ON run blew a 200 s client cap; server-side they ran 777–**8091** tokens / up to **830 s**, despite
   `max_tokens=2048` (the cap bounds `content`, not `reasoning_content`). So "escalate to naive think-ON for
   correctness" trades a 30% silent-fail for a ~100% runaway. **Escalation, if used, must pair think-ON with
   an explicit `thinking_budget` (or wallclock/iter kill)** — never unbounded.
3. **The catch is the artifact gate, not the model.** A 30% "looks-done-but-wrong" rate is precisely why the
   trunk gates code by `tests_green`/mutation downstream: a silently-wrong `write_file` fails `verify` and the
   loop retries — it is **not shipped**. The residual therefore *validates* the architecture (reactive
   escalation + a strong, model-independent downstream gate = defense in depth) rather than overturning it.
   Strongest possible restatement of "gate by a number, not the model's confidence."

**Confound resolved:** `enable_thinking:false` IS honored, not just "model writes verbose code anyway." On a
trivial prompt (`7*8`): think-OFF → `reasoning_content`=0 chars / 2 completion tokens; think-ON → 543 chars /
161 tokens (80× more for the same "56"). On the coding task, think-OFF's 900–2400 tokens are confirmed all
`content` (`reasoning_content`=0) — the model is verbose in its *answer*, with zero hidden reasoning. So the
§4c "think-OFF is 3–15× cheaper" result is **task-class-specific**: it holds for short *tool dispatch*, not
for open-ended *code generation* (where think-OFF is no cheaper — it just doesn't spiral the reasoning).

**Net effect on the recommendation:** §6 holds and is sharpened — (a) default think-OFF for dispatch turns is
confirmed safe & cheap; (b) any escalation to think-ON must be budget-bounded (finding 2); (c) the load-bearing
safety is the downstream artifact gate, not the LLM call (finding 3). Wiring is now unblocked.

## 7b. Other open questions / scope cuts (Wu Wei)
- **GBNF `guided_grammar`** is available and could later force the *action* shape (orthogonal to reasoning
  bounding) — deferred; native tool-calling already works (§4c) and the references kept it.
- **Token-elasticity-safe budgeting** (force closing-think at decode level) — oMLX may already do the clean
  thing at budget=0; not worth chasing partial budgets given binary is better.
- Nothing here is built yet. This doc is the grounding; the wiring (default-OFF + escalate-on-`Length`) is a
  small follow-on change to the agent loop, gated behind the residual measurement above.

## 8. Success criteria

- [x] Capability surface verified against `/openapi.json` + live probes (§1).
- [x] Literature grounding with the elasticity + function-calling findings (§2).
- [x] Framework prior art recalled (§3).
- [x] Empirical matrix run on our server: thinking_budget sweep, response_format routing, tool-calling
  think-on-vs-off (§4).
- [x] A single recommendation, gated by numbers not vibe (§6).
- [x] Residual silent-failure measurement on a multi-step task (§7, run 2026-06-10): think-OFF 30% silent-fail
  on generative code; think-ON unbounded/unusable (`max_tokens` ≠ reasoning bound); the downstream artifact
  gate is the catch. Recommendation validated & sharpened; wiring unblocked.
