# research/27 — Context Budget (how big should the working window be?)

> Special research project (Gary, 2026-06-10): triggered by *"Elaborate the 32K-tok? I thought coding harnesses
> allow up to 128k for effectiveness."* — a correct challenge to a stale assumption. Sibling of research/25:
> **25 = how to curate the middle (the mechanism); 27 = how big the working window should be (the number), and
> where quality/cost actually turn over.** This doc sizes the F1 budget constant + the compaction trigger
> threshold with evidence instead of a phantom 32K wall. Method = **Hybrid**: Claude local teardown + KB
> grounding (§2); parallel web workers survey the field (§3); §4 synthesis; §5 adversarial review; §6 verdict
> (the numbers the harness will use).

## 1. The problem (why this project exists)

The harness had a **32K-token working-window assumption baked in three places**, and it was wrong:

- `crates/agent/src/refeed.rs` / `main.rs`: `CONTEXT_WARN_BYTES = 100_000`, documented as "~80% of a 32K-token
  window (~128KB at 4 B/token)."
- `research/21`: attributed an observed **silent truncation** (oMLX dropping the system prompt first) to the 32K
  window.
- The proposed **F1 budget** (research/25) was floated at ~96KB, sized to that 32K wall.

**Verified live (2026-06-10):** `~/.omlx/settings.json` → `max_context_window: 262144`; server `/v1/models`
reports `262144` for `Qwen3.6-35B-A3B-oQ8-fp16-mtp`. The 32K cap is **gone** (only the jina embeddings model is
still 32768). The CLAUDE.md note ("capped at 32768 — raise it to 256K") is **stale**; the raise already happened.
The model is **256K-native** (no RoPE scaling).

So the binding constraint is no longer the hard window. It is the pair the field actually optimizes against:

1. **Quality** — long-context degradation (Lost-in-the-Middle, TACL 2024: ~30% U-shaped mid-context drop). A
   model can *attend* to 256K but reliably *uses* head + tail. Effectiveness turns over well below the hard window.
2. **Cost/latency** — KV cache on a shared-memory box. CLAUDE.md math: KV ≈ 80 KB/token → ~2.5 GB @32K but
   **~20 GB @256K fp16** on a 96 GB Mac that also holds the 35B weights. TurboQuant 4-bit ÷4s that; shape holds.

**The question this doc must answer with numbers:**
- **Q1.** What *effective working window* do real coding harnesses actually use, and **how do they pick it** —
  fixed constant, % of the hard window, or adaptive? (Claude Code, Cursor, Aider, OpenCode, Cline, Continue, Zed…)
- **Q2.** Where does long-context **quality** turn over empirically — for Qwen3-class / MoE models specifically?
  (Lost-in-the-Middle, RULER, NoLiMa, needle-in-haystack effective-length numbers.)
- **Q3.** What is the **KV-cache cost curve** on Apple-Silicon MLX for this model — memory & TTFT/decode vs context
  length, and what does TurboQuant 4-bit KV buy? Where does cost bind on a 96 GB box sharing the 35B weights?
- **Q4.** How does the budget feed research/25's **compaction trigger** (`contextWindow - reserveTokens`,
  `keepRecentTokens`)? The budget isn't one number — it's (working-window, reserve, keep-recent).
- **Q5.** Reconcile **research/21's truncation claim** with the 256K reality — was it a 32K-era artifact, or a
  different cause (e.g. server-side eviction order)?

## 2. Local teardown + KB grounding (Claude + Explore agent, direct)

**omp / pi-mono compaction constants** (identical across both clones; `coding-agent/src/core/compaction/compaction.ts`):
- `DEFAULT_COMPACTION_SETTINGS = { enabled:true, reserveTokens:16384, keepRecentTokens:20000 }` — **global policy
  defaults, NOT per-model**.
- `shouldCompact` fires when `contextTokens > contextWindow − reserveTokens` (≈ 80% of the window once reserve is
  carved off). Reserve = room for the summary + the model's own response.
- **`contextWindow` is per-model**, from an auto-generated registry (`models.generated.ts`, populated from provider
  APIs), **fallback `128000`** when a model isn't listed (`model-registry.ts:677`). Sample entries: Nova-Lite 128K,
  Nova-Pro 300K, Claude Sonnet-4.5 200K, Opus-4.x 1M. So the clones **trust the provider's advertised window** as
  the budget base — exactly the trap this doc exists to avoid.
- **Token estimate = `Math.ceil(chars/4)`** (`compaction.ts:250-290`) — a chars-per-4 heuristic, **not a real
  tokenizer**. Tool-call arguments ARE counted (`JSON.stringify(arguments).length`); images use a flat constant.

**research/21 truncation claim — evidence class = INFERENCE/ASSUMPTION, never a logged repro.** Exact wording
(research/21:116-117): *"max_tokens × growing history can silently blow the 32K prompt window → oMLX drops the
system prompt (workpad + case-law) silently. Cap re-fed content; add an overflow signal."* It is framed
throughout as a **flag to investigate** (":103 — *does* oMLX silently truncate… Flag"), not an observation with a
repro. **Both premises are now falsified:** (a) the window is 262144 not 32K (§1), and (b) no truncation was ever
reproduced. The defensive work it motivated (`cap` + `[ctx]` overflow signal in `refeed.rs`) is still *useful*
(bounded re-feed is good hygiene), but its stated rationale ("the 32K wall") is dead. **`CONTEXT_WARN_BYTES`'s
docstring is wrong, not its value** (see §6).

**KB grounding** — `search("long context degradation lost in the middle effective context length")`: the ML
texts cover attention mechanics, not long-context degradation empirics (out of corpus scope). The one on-point
hit is **Production RAG Guide §5**: *"longer context windows introduce their own problems (noise, cost, attention
degradation)"* and *"never remove the system prompt to save tokens"* — corroborates the thesis, no new numbers.
The quantitative grounding came from the web workers (§3), not the KB. → `KB: searched "long context
degradation" → corroborating principle (Production RAG Guide §5), no quantitative numbers (out of corpus).`

## 3. Web survey (parallel workers — full reports archived in session transcript)

### 3.1 W1 — how real harnesses pick the budget (Q1)
- **Dominant pattern = `window − fixed_reserve`.** OpenCode (source-verified, `overflow.ts`):
  `usable = context_limit − min(20_000, maxOutputTokens)`; compact at `count >= usable`. **Cline (hybrid floor):**
  `maxAllowed = max(contextWindow − 40_000, contextWindow × 0.8)` — fixed floor OR 80%, whichever leaves more room.
- **~80% trigger is the convergent fraction** (Claude Code ~78-84%, Cline 80%, Roo Code default 100% but users set
  80%, Zed warns 80%). Codex CLI: token-based `model_auto_compact_token_limit` ~180-244K + ~95% safety margin,
  preserves last ~20K of user msgs. Aider: repo-map `--map-tokens` **1024** fixed soft budget.
- **OpenCode two-phase** (verified): prune stale tool output first (`PRUNE_PROTECT=40_000`, keep last 2 turns),
  *then* LLM-summarize only if still over. Cheap wins before the lossy step.
- **"128K sweet spot" = folklore / marketing anchor.** No measured inflection *at* 128K; 128K is just the advertised
  GPT-4 window. The real, repeated finding is **effective ≈ ½ advertised** (GPT-4 128K→~64K effective). Chroma
  "Context Rot" (18 models incl. Claude 4 / Gemini 2.5 / Qwen3): reliability degrades **monotonically** with input
  length even on trivial tasks; degradation is non-uniform.

### 3.2 W2 — where quality turns over (Q2) — *the decisive section*
- **RULER (NVIDIA, effective length = longest window still ≥85.6%):** advertised-vs-effective gap ≈ **2×**.
  **`Qwen3-30B-A3B` → 64K effective** (96.5@4K, 92.4@32K, 89.1@64K, **79.2@128K**) — lands with the *small* dense
  models (8B, 4B all 64K), **below** dense 14B/32B (>128K). **Active-param count (~3B), not total, tracks
  long-context robustness.** This is the best public proxy for our 35B-**A3B**.
- **NoLiMa (ICML 2025) — retrieval ≠ reasoning, and the gap is huge.** Same model/length, Llama-3.3-70B:
  literal NIAH retrieval **flat ~98% to 32K**, but **one-hop latent reasoning 84→56% by 32K**, **two-hop 57→26%**.
  Restoring a literal lexical cue rescues two-hop 26→87%. NoLiMa's reasoning effective-lengths for "128K" models
  are **1K-8K** (GPT-4o best at 8K). **"256K-native" is a *retrieval* capacity claim; it says nothing about
  reasoning at depth.**
- **Lost-in-the-Middle (TACL 2024):** >30-point accuracy drop when the relevant item is mid-context vs at the
  head/tail. Position is a near-free lever — pin instructions + key files to the **edges**.
- **Qwen3 specifics:** base Qwen3 is **32K-native**, 128K via **YaRN** (4× extrapolation, costs short-context
  quality — enable only when needed). **Our `Qwen3.6-35B-A3B` is 256K-*native* (no YaRN)** — a genuinely better
  base than YaRN-to-128K, so its native curve should be *gentler* than the 30B-A3B proxy; but the ~3B-active
  reasoning constraint still applies.

### 3.3 W3 — cost curve on this box (Q3)
- **Per-token KV math confirmed exactly: 80 KB/token fp16** (2·40·2·256·2). Linear in length. fp16 KV: 2.5 GB@32K,
  10 GB@128K, 20 GB@256K; 4-bit ÷~3-4.
- **Memory is NOT the binding constraint.** Weights (8-bit 35B) ~35 GB; default wired cap ~72 GB (kernel 77.8 GB)
  → ~35 GB free → KV doesn't hit the wall until **~500K tokens**. The full 256K window fits with room to spare.
- **Latency is the real cost. Prefill (TTFT) is compute-bound, O(n²)-ish, quant-independent.** Same model on M1
  Max: **~49 s TTFT at 8.5K** (94% of turn time). M3 Max faster but still tens of seconds at 8K, **minutes at
  32K+** cold. **Prefix/SSD KV cache (oMLX has it) is the #1 lever: 49 s → 1.7 s at 8K on a cache hit.** Decode is
  bandwidth-bound, fp16 KV dominates at depth: ~−32% tok/s by 32K (MoE analog), worse beyond.
- **4-bit KV caveat for Qwen specifically:** KIVI-4bit on Qwen2.5 hid a worst-case per-head collapse (min cosine
  0.588) under good average PPL — a Qwen-family failure mode. TurboQuant (Hadamard+Lloyd-Max *scalar*, not VQ as
  CLAUDE.md says) reportedly matched fp16 on the 35B at temp 0, but **eval-gate before trusting it.**

## 4. Synthesis — three ceilings, at three very different places

The "context budget" is governed by **three independent ceilings**, and the binding one for a *coding* agent is
the tightest:

| Ceiling | Where it binds | For our model on this box |
|---|---|---|
| **Memory** (KV + weights vs wired cap) | ~**500K** tokens | non-binding — even full 256K fits |
| **Latency** (cold prefill TTFT) | ~**16-32K** interactive | mitigated by prefix cache (49s→1.7s on hit); cold tail still hurts |
| **Quality / reasoning** (A3B reasoning erosion) | **~32K full / ~64K wall** | **the binding constraint** |

The decisive reframe: **a coding agent *reasons over* its context (multi-hop: "this error ↔ that import ↔ this
signature"), it does not merely *retrieve* from it.** NoLiMa proves reasoning erodes far earlier than retrieval —
two-hop halves by 16-32K while NIAH stays 98%. So "256K-native" (a retrieval/capacity fact, verified only that the
*server accepts* 262144) is the **wrong number to size against**. The right number is the **reasoning** curve:
full quality ≤ **~32K** (the Qwen3 native window, where every benchmark holds strongest), hard wall ~**64K** (the
A3B RULER effective length). Gary's ~128K instinct is closer to the truth than our phantom 32K *hard wall* — but
128K is itself folklore; for an **A3B reasoning** workload the honest figure is **lower**, ~32-64K.

**Reconciliation with the baked-in 32K:** the old code constant (`CONTEXT_WARN_BYTES = 100_000` ≈ 25K tok) was
**numerically in the right zone by accident** — it was justified as "80% of the 32K *hard window*," which is dead,
but it lands right next to the real "approaching the ~32K *quality* boundary" line. We keep ~the value, **replace
the rationale**, and stop calling 32K a hard wall.

## 5. Adversarial review (self, 4 angles)

1. **"30B-A3B is a stale proxy; the 256K-native 35B may degrade later → 64K too conservative."** Partly fair —
   native-256K should beat YaRN-to-128K, so retrieval and maybe shallow reasoning hold longer. But NoLiMa's
   reasoning-erosion and the ~3B-active constraint are architecture-general; the proxy is conservative on
   retrieval, *roughly right on reasoning*. **Treat §6 numbers as PRIORS, not gospel** → §6 ships a cheap oMLX
   NoLiMa-lite probe as the empirical gate before hard-coding anything tighter.
2. **"chars/4 undercounts code tokens (code tokenizes denser) → compact later than intended."** Real. Undercount
   pushes us *toward* holding too much (quality-risky direction). Mitigation: chars/4 to start (matches clones,
   simple), but size the trigger with headroom so the error is absorbed; revisit to chars/3.5 for code if a probe
   shows drift. Wu Wei — don't build a tokenizer yet.
3. **"Prefix cache buys latency headroom — why not a bigger window?"** Because the cache moves the **latency**
   ceiling, not the **quality** one. Quality doesn't cache. The cap stays quality-driven; the cache just makes the
   chosen window cheaper to re-send. (This actually *sharpens* the verdict: don't let cheap prefill tempt a wider
   window.)
4. **"Wu Wei — is this even a problem yet?"** Yes, at the tail: F2 compaction is already planned, the transcript
   grows unbounded today (research/25 §1), and planexec surfaced THRASH-to-max_iters. Sizing the constant *now*
   (cheap, one doc) stops a wrong number (phantom 32K hard wall, or naive 256K) getting baked into F1/F2. But the
   *binding* deliverable is the constant + its rationale, **not** new machinery — the machinery is research/25's
   F1-F5.

**Bonus win surfaced by review:** at a quality-first ~32-48K working window, fp16 KV is only ~2.5-4 GB — **we do
not need 4-bit KV for memory at all**, so we sidestep the Qwen 4-bit-KV worst-case-collapse risk entirely. Keep
KV fp16 until/unless concurrency or much longer contexts force the question.

## 6. Verdict — the numbers the harness will use

**Size the working window to the reasoning curve, not the hard window. Token-based, model-tagged, not bytes.**

| Constant | Value | Rationale |
|---|---|---|
| **effective working window** | **48K tokens** (not 256K) | inside the A3B 64K RULER wall, honoring NoLiMa reasoning erosion; quality-first |
| **compaction trigger** | **32K tokens** (= window − reserve) | the Qwen3 native-window full-quality boundary; compact exactly when you'd leave the sharp zone |
| **reserveTokens** | **16K** | matches omp default; room for response + thinking_budget (≤4K) + summary |
| **keepRecentTokens** | **20K** | matches omp default; the verbatim recent working set F1 protects |
| **token estimate** | **chars/4** | matches clones; conservative enough for a trigger; revisit for code if a probe shows drift |
| **KV precision** | **fp16** (no 4-bit) | at ≤48K, KV is ~2.5-4 GB — quant unnecessary; avoids Qwen 4-bit per-head-collapse risk |

**Formula (Cline-style hybrid floor, in tokens):** `trigger = max(window − reserveTokens, floor(window × 0.8))`.
At window=48K, reserve=16K → `max(32K, 38.4K)` = **38.4K**. *Choose the stricter quality-first reading:* set the
**hard window to 40K** so `window − reserve = 24K` and `0.8×window = 32K` → trigger **32K**. Net: live transcript
oscillates ~20K (post-compact) ↔ 32K (trigger) — squarely in the full-quality band.

**Free levers (independent of the budget):**
- **Pin instructions + key files to the edges** (head/tail), never bury the spec mid-context (Lost-in-the-Middle,
  >30% mid drop). Our system prompt is already at head[0]; ensure the *active task spec* rides head or recent tail.
- **Give lexical anchors** — name exact files/symbols rather than relying on latent multi-hop (NoLiMa rescue
  26→87%). Affects how the summary (research/25 §2.6) and case-law injection are phrased.
- **Lean on the prefix cache** — keep the head stable (system prompt + case-law unchanged across turns) to
  maximize cache hits and kill cold-prefill TTFT.

**Impact on the build:**
- **research/25 F1 (`refeed::assemble`)** budget = `keepRecentTokens ≈ 20K tok` for the tail (head always kept),
  **not** the ~96 KB I floated. In bytes that's ~80 KB at chars/4 — close to the old value, now principled.
- **`CONTEXT_WARN_BYTES`**: keep ~100 KB *or* convert to a 32K-token warn; **fix the docstring** ("approaching the
  ~32K reasoning-quality boundary," not "80% of a 32K hard window"). The byte→token migration can ride F1.
- **research/25 F2 (compaction trigger)** uses the §6 table directly: window 40K, reserve 16K, keepRecent 20K,
  trigger 32K.
- **Empirical gate (cheap, deferred until a number actually bites):** a **NoLiMa-lite probe on oMLX** — plant a
  2-hop fact at varying depths in a synthetic transcript, measure recall@8K/16K/32K/48K. Confirms (or tightens)
  the 32K/48K priors on *our* model. Throwaway `/tmp` exercise; don't land it.

**Decision to record:** *Effective context budget is sized to the A3B reasoning curve (~32K full / ~48K cap), not
the 256K hard window or the 128K folklore. Selection policy = `window − fixed_reserve` (window=40K, reserve=16K,
keepRecent=20K), token-estimated at chars/4, KV fp16.* → `decisions.md`.
