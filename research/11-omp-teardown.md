# oh-my-pi Teardown — Base & Language Strategy (DR1)

> Bounded teardown of `can1357/oh-my-pi` (cloned to `Skills/oh-my-pi`, shallow). omp is a maintained
> **superset/fork of pi-mono** (npm scope renamed `@earendil-works/*` → `@oh-my-pi/*`). It adds a
> ~27k-LoC owned-Rust native core under the *same* TS agent runtime pi-mono ships. The headline finding
> decides the language strategy: **the Rust core is leaf systems-primitives only; the agent loop, tool
> execution, and the gate hook are 100% TypeScript — byte-identical to pi-mono.**

Measured LoC (this clone): Rust `crates/` = **67.8k** total, of which **~34k is vendored brush-shell**
(`brush-core-vendored` 25.5k + `brush-builtins-vendored` 8.3k) and **~33k is omp-owned** (`pi-natives`
12.4k, `pi-shell` 14.8k, `pi-ast` 2.8k, `pi-iso` 4.0k). TS `packages/` = **615k** LoC
(`coding-agent` 252k, `ai` 75k, `tui` 23k, `mnemopi` 18k, `agent` 10.5k, swarm/utils/etc.). The "~27k
Rust core" in the README counts owned non-vendored modules minus glue/tests.

---

## 1. Rust/TS boundary

**The boundary is a napi-rs `cdylib`.** `crates/pi-natives` is `crate-type = ["cdylib"]` depending on
`napi` + `napi-derive` (`crates/pi-natives/Cargo.toml`). Every Rust capability is a `#[napi]`
function/class consumed from TS as `@oh-my-pi/pi-natives` (loader: `packages/natives/native/index.js`,
generated types: `native/index.d.ts`). `crates/pi-natives/src/lib.rs` declares the entire export surface:
`grep, fd, glob, ast, block, summary, shell, pty, ps, keys, text, highlight, html, sixel, clipboard,
tokens, iso, appearance, power, prof, fs_cache, task, workspace`.

**What lives in Rust** (per README crate table, `crates/pi-natives/src/*.rs`): regex search
(`grep`, grep-regex/searcher, 1.9k), embedded bash + PTY + process trees (`shell` 3.7k via vendored
`brush-*`, `pty` 455, `ps` 195), tree-sitter AST summarize/rewrite (`ast`+`summary` ~2k via
ast-grep-core), syntax highlight (syntect), ANSI-aware text width, BPE token counting (tiktoken-rs),
image decode, HTML→MD, clipboard, mtime fs-cache, and workspace isolation (`iso`, delegating to the
`pi-iso` PAL crate). **These are all stateless or self-contained leaf operations** — "the work other
harnesses shell out for" (README §"~27,000 lines of Rust").

**What lives in TypeScript** (the *entire* agent brain): the provider layer (`packages/ai`, ~20
providers), the `Agent` runtime + low-level `agentLoop()`/`agentLoopContinue()` (`packages/agent/src/
agent-loop.ts`, 1490 LoC), the coding-agent host (`packages/coding-agent`, 252k — TUI, sessions, MCP,
LSP/DAP, skills, ExtensionAPI, subagents), and all orchestration extensions (`packages/swarm-extension`
is a DAG multi-agent pipeline registered as a `/swarm` command — same extension pattern as our align-gate).

**CRITICAL — where the agent loop / tool exec / gate hook live:** all three are **TS, not Rust.**
`grep -rln "agentLoop|tool_call|reqwest|chat/completions" crates/` returns **nothing** — there is no LLM
client, no HTTP, no agent loop, and no tool-call gate anywhere in the Rust workspace. The gate is the
same `beforeToolCall` block in `packages/agent/src/agent-loop.ts:1278`:
```ts
if (beforeToolCall) {
  const beforeResult = await beforeToolCall({ assistantMessage, toolCall, args: effectiveArgs, context }, toolSignal);
  if (beforeResult?.block) { throw new ToolCallBlockedError(beforeResult.reason); }
}
```
This is the *exact* primitive the spike (`research/07`) validated and the extension `tool_call`
event funnels into. **Strategy-A consequence:** omp gives us **zero** of strategy A's hard parts — none
of the agent loop, provider layer, or gate exist in Rust. omp is strategy **B in the flesh** (Rust
primitives under a TS brain), and its Rust is only the *primitives* layer, not the loop.

## 2. Severability

**Mixed, but the one piece we want is cleanly severable.** The napi-rs boundary means each Rust module
is callable in isolation *from JS*, but as Rust crates they vary:

- **`pi-iso` — fully severable, lift it.** `crates/pi-iso/Cargo.toml` deps are only `async-trait`,
  `similar`, `tokio`, and platform `libc`/`windows-sys`. **Zero napi, zero TS coupling**
  (`grep -i napi crates/pi-iso/Cargo.toml` → none). It is a self-contained cross-platform isolation PAL:
  a `trait IsolationBackend { kind; probe; start(lower,merged); stop(merged); async diff() }`
  (`lib.rs:225`) with backends for APFS `clonefile`, btrfs/zfs snapshot, Linux reflink/overlayfs,
  Windows block-clone/ProjFS, and an `Rcopy` fallback that does **`git worktree add --detach`**
  (`rcopy.rs:4,40`). `resolve(preferred) -> Resolution` (`lib.rs:338`) auto-picks the best backend per
  host. This is *precisely* our P2 branch/promote worktree-isolation pillar, already written, tested,
  cross-platform, and dependency-clean. The napi shim `crates/pi-natives/src/iso.rs` is just ~245 LoC of
  thin `iso_start/iso_stop/iso_diff/iso_resolve` wrappers — proof pi-iso is the real unit and the host
  binding is trivial.
- **`pi-ast`, `pi-shell`** — severable in principle (own crates) but heavier: `pi-shell` wraps two
  vendored brush crates (~34k LoC) for an embedded bash; `pi-ast` pulls tree-sitter + 50 grammars.
  Liftable if we want them, but they're "nice to have," not load-bearing for the spine.
- **`pi-natives` itself** — *not* severable; it IS the napi host glue (it's the cdylib). You don't lift
  it, you lift the crates beneath it.

**Boundary mechanism = napi-rs N-API addon** (`.node` cdylib, per-platform leaf packages via
`optionalDependencies`, loader in `packages/natives/native/index.js`). Not wasm, not subprocess, not
NDJSON. For a Rust-native harness we don't need the napi shim at all — we call `pi-iso` (and friends) as
**direct Rust crate deps**, skipping the entire `.node`/loader/embed machinery.

## 3. Provider surface

omp implements providers in TS (`packages/ai`, ~75k LoC, ~20 providers) keyed by an `api` discriminant:
`openai-completions`, `anthropic-messages`, `openai-responses`, `bedrock-converse-stream`,
google/gemini, etc. (`packages/ai/src/providers/*.ts`). Local endpoints are first-class via
`openai-completions` + `baseUrl` + `compat` flags (the spike already ran oMLX through this).

**Confirmed: we need only ~3.** For model-per-role local-first: **(1) `openai-completions`** covers
oMLX (`127.0.0.1:8000/v1`) *and* the PC's llama.cpp/vLLM — one client, N models. **(2) `anthropic-
messages`** for the cloud adversarial/build tier (Opus/Sonnet). Optionally **(3)** a second OpenAI-
compatible cloud tier (OpenRouter/Groq) — but that reuses client #1's wire format. So the real surface is
**two wire protocols**: OpenAI-completions SSE and Anthropic Messages SSE.

**Cost of a minimal Rust provider layer vs reusing pi-ai (TS):** small and bounded. Two streaming
clients over `reqwest` + `eventsource-stream` (or `reqwest-eventsource`) + `serde`:
- OpenAI-completions: POST `/chat/completions` `stream:true`, parse `data:` SSE deltas, assemble
  `tool_calls` from incremental `function.arguments` fragments. ~300–500 LoC.
- Anthropic Messages: POST `/v1/messages` `stream:true`, handle the typed event stream
  (`message_start`/`content_block_start`/`content_block_delta`/`message_delta`), assemble `tool_use`
  blocks. ~400–600 LoC.
- Shared `Model`/`Message`/`ToolCall` types + a tiny role→model registry: ~200 LoC.

Total **~1–1.5k LoC of owned Rust** for the full provider need — a weekend, not a quarter. **Rust crates
that already cover it:** `async-openai` (mature OpenAI-compatible streaming + tool-calls — works against
any `baseUrl`, so oMLX/vLLM/llama.cpp included) and `anthropic-sdk`/`anthropic-rs` (community Anthropic
clients), or `genai`/`llm` (multi-provider Rust crates). Given the "dependencies are liabilities" rule,
the pragmatic path: **own the two thin clients** (the wire formats are stable and small) and optionally
crib `async-openai`'s streaming-assembly logic for the SSE delta accumulation. Reusing pi-ai (75k LoC TS)
to get two protocols we can write in ~1k LoC of Rust is a bad trade for a forever-personal core — it
re-imports the whole TS toolchain to avoid a fortnight of owned code.

## 4. Gate re-validation sketch

The align-gate concept — *deny exec tools until an `aligned` flag flips, persisted across reload* — ports
to a pure-Rust loop **more cleanly** than the TS hook, because in our own loop the gate is just a branch
in the dispatch path rather than a callback contract we don't control.

**(A) Pure-Rust agent loop (strategy A / the target):**
```rust
enum Phase { Align, Build }
struct Gate { phase: Phase, exec_tools: HashSet<&'static str> } // {"bash","write","edit",...}

impl Gate {
    // called in the tool-dispatch loop, before execute()
    fn check(&self, tool: &str) -> Result<(), Blocked> {
        if matches!(self.phase, Phase::Align) && self.exec_tools.contains(tool) {
            return Err(Blocked { reason: "Align gate: confirm the plan before executing.".into() });
        }
        Ok(())
    }
}
// in the loop: for call in tool_calls { gate.check(&call.name)?; /* else */ execute(call).await }
// /align flips phase and appends a board/session record; on boot, replay the record to restore phase.
```
The `Blocked.reason` becomes the tool-result content the model relays to the user — same teaching-surface
UX the spike proved (`research/07` §1). Persistence is a row in our (rusqlite) board/session, replayed on
startup; this is *cleaner* than pi's "scan session JSONL custom entries on `session_start`" because the
board is our durable source of truth, not a side-channel. The `enum Phase` makes the state machine
exhaustive and compiler-checked — illegal phase transitions fail to compile.

**(B) omp's existing TS hook (strategy B/C):** unchanged from the spike — `beforeToolCall` returns
`{block:true,reason}` (`agent-loop.ts:1288`), or the extension `tool_call` event with first-block-wins
early-exit. Proven, ~55-line extension. Works today, but the gate is advisory glue *on top of* a loop we
don't own; the phase flag lives in a closure + JSONL custom entries.

**Verdict:** Rust makes the gate **cleaner, not harder** — it collapses from "hook callback + closure
state + JSONL replay" to "a typed branch in our own dispatch + a board row." The spike's four primitives
(block-with-reason, batch atomicity via per-call preflight, `/align` unlock, phase persistence) all port
1:1, and batch atomicity is *free* in a Rust loop because we control the iteration order.

## 5. Self-building friction (honest read)

For an eventually self-modifying harness, Rust and TS trade off in a way that, for *this* project, favors
Rust despite the friction. Honest costs: models are demonstrably more fluent at TS than Rust — more
training data, fewer borrow-checker/lifetime stumbles, faster first-draft success — and TS's edit-run
loop is near-instant (tsx, no build) versus Rust's compile cycle, which even with a warm incremental
build (omp's `profile.dev` is tuned: `opt-level=0`, `codegen-units=256`, `incremental=true`,
`split-debuginfo`) is seconds-to-tens-of-seconds and grows with the crate; a self-modifying agent pays
that latency on every iteration of its own evolve loop. **But** the compiler is the decisive asset for
*safe* self-modification: a forever-personal harness that rewrites its own code needs a guardrail that
catches the agent's mistakes mechanically, and Rust's type system + borrow checker + exhaustive
`match` + `#[must_use]` reject whole classes of self-inflicted breakage (null/aliasing/unhandled-variant
/use-after-move) **at compile time, before the bad version ever runs** — exactly the "gate by a number
or an artifact, never a vibe" principle applied to the agent editing itself. TS's structural types catch
far less and `any` leaks erode even that. The compile-loop tax is real but bounded (keep crates small,
the evolve loop touches one module at a time); the model-fluency gap is closing and is mitigable by using
the strong cloud tier for Rust authorship and reserving local models for review/recon. Net: Rust trades a
slower, harder *write* loop for a vastly stronger *correctness oracle* — and for a harness whose endgame
is editing itself, the oracle is worth more than the fluency.

---

## Recommendation — **A (full Rust-native own-core), with pi-iso lifted as a crate**

Not B, not C. The teardown's headline kills the case for B/C: **omp's Rust core contains none of the
agent loop, provider layer, or gate** — those are 100% TS and identical to pi-mono. So adopting omp's
*model* (B = Rust primitives + TS brain) means inheriting a 615k-LoC TS host (coding-agent + ai + tui) to
get search/shell/AST we can either lift as standalone crates or write thin. The only thing in omp's Rust
worth depending on at runtime — **`pi-iso`** — is *already* a clean, napi-free, dependency-light crate we
can lift directly into a Rust-native build. That removes B's whole reason to exist (we don't need the TS
host to get the good Rust). C (stay TS-on-Pi) remains the *fast* path and a valid fallback, but it
permanently couples our forever-personal core to two upstreams and forfeits the compiler-as-guardrail
that the self-building endgame wants.

**Confidence: high on the boundary mapping** (the Rust/TS split, the napi mechanism, the gate location,
pi-iso's severability are all directly verified in code). **Medium on the build-cost estimates** (provider
layer ~1.5k LoC, agent loop) — these are reasoned from the surface, not yet spiked. **Flagged deeper
spike (Phase 0, not blocking):** a minimal end-to-end Rust slice = own `agentLoop` + the two streaming
provider clients + the Rust gate of §4 + `pi-iso` for worktree iso, driving oMLX, to confirm the loop +
provider estimates before committing the trunk. The architecture decision (A) does **not** need that
spike; the *sizing* does.

## Reuse vs reimplement list

**LIFT directly (Rust crate dep, runtime):**
- **`crates/pi-iso`** — the cross-platform isolation PAL (APFS clone / btrfs-zfs reflink / overlayfs /
  ProjFS / git-worktree rcopy + `IsolationBackend` trait + `resolve()` auto-select + `diff()`). This IS
  our P2 worktree-isolation pillar. Clean deps, no napi, no TS. **Top reuse.**

**STEAL the design (reimplement in our Rust, don't depend):**
- **The `beforeToolCall`-block gate shape** (`packages/agent/src/agent-loop.ts:1278-1291`) → our §4
  Rust `Gate::check` branch. Port the block-with-reason + per-call preflight (batch atomicity) semantics.
- **pi-ai's provider `api`-discriminant model** (`packages/ai/src/providers/`) → our two thin Rust
  clients (`openai-completions` SSE, `anthropic-messages` SSE). Crib `async-openai`'s SSE delta/tool-call
  assembly. ~1–1.5k LoC owned.
- **The swarm-extension DAG orchestration pattern** (`packages/swarm-extension/src/swarm/{dag,pipeline,
  state}.ts` — waves, `reports_to`/`waits_for`, cycle detect) → reference for our coordinator/board
  multi-worker dispatch (P4). Design only.
- **pi-iso's `Rcopy` git-worktree lifecycle** (`crates/pi-iso/src/rcopy.rs` — `worktree add --detach
  HEAD` / `worktree remove --force`) — already inside the lifted crate; this is the branch/promote churn
  mechanism for free.

**OPTIONAL lift (only if we want native bash/AST later):**
- `crates/pi-shell` (embedded brush bash + PTY) and `crates/pi-ast` (tree-sitter summarize) — severable
  but heavy; defer. Until then a Rust `tokio::process` shell + `tree-sitter` direct dep cover the need.

**DO NOT take:** `pi-natives` (it's the napi cdylib glue — irrelevant to a non-Node host), the
`packages/natives/native` loader/embed machinery, and the 615k-LoC TS host (`coding-agent`/`ai`/`tui`) as
a runtime dependency. Mine them for designs (compaction, session-tree, mnemopi memory) per Wu Wei, but
don't couple to them.

## Sources (paths read)

- `Skills/oh-my-pi/Cargo.toml`, `crates/*/Cargo.toml` — workspace, crate deps, build profiles, toolchain.
- `crates/pi-natives/Cargo.toml`, `crates/pi-natives/src/lib.rs` — napi cdylib + module export list.
- `crates/pi-natives/src/iso.rs` — the ~245-LoC napi shim over pi-iso (proves pi-iso is the unit).
- `crates/pi-iso/Cargo.toml`, `crates/pi-iso/src/lib.rs`, `crates/pi-iso/src/rcopy.rs` — IsolationBackend
  trait, backend resolver, git-worktree fallback; confirmed napi-free + dependency-light (severable).
- `packages/agent/src/agent-loop.ts` (1490 LoC) — the TS agent loop + `beforeToolCall` block at :1278.
- `packages/swarm-extension/src/extension.ts`, `src/swarm/schema.ts` — TS DAG orchestration extension.
- `packages/ai/src/providers/` (file listing), `packages/ai/src/api-registry.ts` — provider `api`
  discriminants (openai-completions / anthropic-messages / …); ~20 providers, all TS.
- `docs/natives-architecture.md`, `docs/natives-binding-contract.md` — napi-rs boundary, loader model,
  JS↔native export mapping, sync/async/constructor contract.
- `README.md` §"~27,000 lines of Rust" + crate/module table + Rust-crates table — module LoC breakdown,
  crate responsibilities, omp↔pi-mono framing.
- `grep` over `crates/` for `reqwest|agentLoop|tool_call|chat/completions` — confirmed **no** LLM/agent/
  HTTP code in any Rust crate (the decisive negative result).
- Prior context: `research/06-pi-internals.md`, `research/07-pi-spike.md`, `research/10-eval-scope.md`,
  `wiki/architecture.md`.
