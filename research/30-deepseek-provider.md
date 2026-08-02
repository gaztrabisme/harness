# research/30 — DeepSeek V4 as a second cloud backend

**Status:** doc-verified (official docs only) + live-confirmed (smoke against the real API, prior session).
**Outcome:** wired into `OpenAiProvider` as a second dialect (`Dialect::DeepSeek`). See `crates/provider/src/openai.rs`.

> **Secret handling.** The API key is a real provider secret. It lives ONLY in the `DEEPSEEK_API_KEY`
> environment variable, read at construction by `OpenAiProvider::deepseek()`. It is never hardcoded, never
> written to any file (including this one), and must be kept out of the trajectory. The key shared in chat
> should be rotated.

## Why a second backend at all

The harness is local-first (oMLX is the default and the only landing backend until now). DeepSeek V4 is a
cheap, long-context cloud model that's useful when a run needs more capability or a 1M-token window than the
local MoE gives. It's opt-in per run via `HARNESS_PROVIDER=deepseek` — oMLX stays the default, and `explore`
(a throwaway probe that never lands) stays oMLX-pinned so a discard-run never reaches for paid compute.

## Wire facts (official docs, cross-checked + live)

- **Base URL:** `https://api.deepseek.com` — OpenAI-compatible `/chat/completions`. Also exposes `/anthropic`
  and `/beta`. We use the OpenAI-completions path (same dialect the oMLX provider already speaks).
- **Models:** `deepseek-v4-pro` (driven by the harness) and `deepseek-v4-flash` (cheaper/faster).
- **Context / output:** 1M context, up to 384K max output.
- **Reasoning knob:** a `thinking` object — `{"type": "enabled"|"disabled", "reasoning_effort": "high"|"max"}`.
  - There is **no numeric budget** (unlike oMLX's `thinking_budget`). The only dial is `reasoning_effort`.
  - Doc mapping: low/medium → `high`; xhigh → `max`.
  - `max_tokens` bounds **CoT + answer together** (not CoT alone).
  - Reasoning text comes back in the `reasoning_content` field (the OpenAiProvider already parses this).
  - **Must strip `reasoning_content` before re-sending** prior assistant turns, or the API 400s.
- **Tools:** up to 128; `tool_choice` ∈ none/auto/required/named; `strict` available (beta).
- **`response_format`:** `text` | `json_object`.
- **Deprecated:** `frequency_penalty` / `presence_penalty` (ignored). Legacy `deepseek-chat` / `deepseek-reasoner`
  deprecate **2026-07-24** — do not build on them.
- **Prompt caching:** automatic; `usage` reports `prompt_cache_hit_tokens`.

### Stale-doc trap (recorded so we don't re-hit it)

The `/guides/reasoning_model` page describes the **legacy** `deepseek-reasoner` ("always reasons, no toggle"),
which is **wrong for V4**. The authoritative V4 `thinking` object is on `/api/create-chat-completion`. Caught by
cross-checking the two pages + a live smoke.

## Pricing (per 1M tokens, recorded for cost-awareness)

| model            | input (cache hit) | input (miss) | output |
|------------------|-------------------|--------------|--------|
| deepseek-v4-pro  | $0.003625         | $0.435       | $0.87  |
| deepseek-v4-flash| $0.0028           | $0.14        | $0.28  |

## How the harness maps to it (`Dialect::DeepSeek`)

The normalized `Request.think_budget: Option<u32>` maps:
- `None` → `thinking: {type: "disabled"}`.
- `Some(n)` → `thinking: {type: "enabled", reasoning_effort: n >= 4096 ? "max" : "high"}`.

`think_budget` is the harness's single reasoning dial; since DeepSeek has no numeric budget, we collapse it to
the two-rung effort knob (modest → high, large → max). `max_tokens` still travels and bounds the total.

Response parsing is **unchanged** from the oMLX path — both dialects return `reasoning_content`, `tool_calls`,
and `finish_reason`. The only per-dialect divergence is the reasoning encoding in `request_to_body`.

## Verification

- Unit: `body_carries_deepseek_thinking` — asserts the `thinking` object is present with the right effort and
  that the oMLX keys (`chat_template_kwargs`, flat `thinking_budget`) are **absent** on the DeepSeek path; the
  oMLX unit test asserts the converse. (`cargo test -p provider`.)
- Live (operator-run, `#[ignore]`): `deepseek_emits_tool_call` — reads `DEEPSEEK_API_KEY`, skips cleanly if
  unset. Run with `DEEPSEEK_API_KEY=… cargo test -p provider -- --ignored deepseek`.
  Not run inline by the agent (would leak the key into the trajectory) — fire it yourself when you want a fresh
  end-to-end confirmation.
- Shape was live-confirmed against the real API during research (prior session); this wiring reproduces that
  exact shape.

## Selection

Backends are chosen by the **provider registry** (`crates/agent/src/config.rs::select`), not
hardcoded. `HARNESS_PROVIDER` (default `omlx`) names a section; `HARNESS_MODEL` optionally overrides
the model. Built-in defaults need no file:
- `deepseek` → (`OpenAiProvider::deepseek()`, model `deepseek-v4-pro`).
- `omlx` / unset → (`OpenAiProvider::omlx()`, the local MoE) — the default.

A `providers.toml` (gitignored; example `providers.example.toml`) can add endpoints or override
defaults — config-only for the `omlx`/`deepseek`/`anthropic` dialects, zero recompile. `key_env` names
an env var, never an inline key. With no file present, behaviour is identical to the old
`provider_and_model` (regression-asserted by the `no_file_*` tests). Only the landing `run` path
consults this; `explore` is oMLX-pinned by design.
