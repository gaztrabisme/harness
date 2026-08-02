# 31 — t5 priming-floor eval results (Slice A / S1)

> Companion to `31-rag-memory-researcher.md` (design + pre-registration §4). This is the
> point-in-time **result**: what the eval measured, what shipped, and what was refuted.
> Reproduce: `cargo test -p board -- --ignored --nocapture t5_floor_eval_v2`.

## TL;DR

**Shipped:** strip stopwords from the `recall_primed` overlap floor and keep `need≥1`
(arm **a\*1**). Noise leak 18→13/24, paraphrase recovery held at 32/33, **no tuned constant**.

**Refuted (do NOT ship):**
- The original t5 hypothesis — "the `need≥2` floor is too strict, drops single-content-token
  paraphrases." On a realistic corpus the shipped floor recovers **33/33**; the premise was an
  artifact of the 14-row v1 probe.
- The BM25-score floor (arms **b′**, **c**). It only reshuffles the same recall/precision frontier
  the count knob already controls, and pays a corpus-calibrated magic constant (θ) the module doc
  defers to Slice B. It does not earn its keep.
- The strict content floor (arm **a\***, `need≥2` on stopword-stripped tokens). It buys precision
  (1/24 noise) by **dropping the entire t5 regime** (0/10 single-content-token paraphrases) — it
  regresses the ticket's own axis.

## Setup

- **Corpus:** 36 atomic lessons mined from project history (14 v1 rows + 22 new). Real titles/bodies,
  `remember()`'d into an in-memory board, indexed by the production FTS5/BM25 path.
- **Queries:** 57, labelled by provenance — 12 **clean** (near-verbatim title), 21 **para**
  (symptom-framed, the single-token regime), 24 **noise** (topic absent → correct answer is EMPTY).
- **Metric:** per-axis **recovery** (target survives the floor within top-K=5) and **noise-leak**
  (a noise query keeps ≥1 row). Significance by exact two-sided binomial **McNemar**, decomposed
  per-axis (mixed-set McNemar cancels a precision gain against a recall cost ~1:1 → p≈1.0, wrong frame).
- **Discipline:** even/odd split per provenance class → calibration (n=29) / holdout (n=28); θ for the
  BM25 arms selected **only** on calibration (budget: noise ≤ a\*'s), pre-registered before reading results.

## Arms

| arm | floor | tokens |
|-----|-------|--------|
| `a` (shipped pre-fix) | count ≥ min(2,n) | raw (`overlap_tokens`) |
| `b` | count ≥ 1 | raw |
| `b′` | count ≥ 1 **and** bm25 ≤ θ | raw |
| `a*` | count ≥ min(2,n) | **content** (`content_tokens`, stopwords stripped) |
| **`a*1` (SHIPPED)** | **count ≥ 1** | **content** |
| `c` | count ≥ 1 **and** bm25 ≤ θ | content |

## Results (FULL, n=57)

```
arm                                   recovery   noise-leak
a:  need≥2 raw          (SHIPPED-pre) 33/33      18/24
b:  need≥1 raw          (blind relax) 33/33      23/24
b′: need≥1 raw & bm25≤-7             22/33      0/24
a*: need≥2 content                    22/33      1/24
a*1:need≥1 content      (SHIPPED)     32/33      13/24
c:  content need≥1 & bm25≤-7         22/33      0/24
```

**Decision metric** (gate by a number, per the harness rule). For *ambient* priming a missed
relevant lesson costs more than a mildly-irrelevant injected one (the model ignores the latter),
so recall-weighting (β≥1) is the right frame:

| arm | F1 | F2 | F0.5 |
|-----|-----|-----|------|
| `a` | 0.786 | 0.90 | 0.694 |
| `a*` | 0.786 | 0.713 | **0.881** |
| **`a*1`** | **0.819** | **0.908** | 0.751 |

`a*1` wins on F1 (neutral) and F2 (recall-weighted). `a*` wins only on F0.5, which over-penalizes
the cheap false-positive cost of ambient priming.

## The decisive panel — content-overlap==1 (the true t5 regime), n=10

```
arm                     recovery
a   (need≥2 raw)        10/10   ← recovered via stopword inflation, not content
a*  (need≥2 content)     0/10   ← DROPS every single-content-token paraphrase
a*1 (need≥1 content)    10/10   ← recovers them on one genuine content token
b′ / c (bm25 floor)      2/10   ← also drop the regime
```

This disqualifies `a*`, `b′`, `c` as "the t5 fix": all three drop the exact paraphrases t5 was
filed to recover. Only `a*1` both recovers the regime and improves precision.

## McNemar — `a` vs `a*1` (the ship), FULL

```
RECALL : a✓a*1✗=1  a✗a*1✓=0   p=1.0000   recall held
NOISE  : a✓a*1✗=3  a✗a*1✓=8   p=0.2266   precision gain (directional, no constant)
```

The single recovery `a*1` "loses" is `mint_id_collision` — query *"two records written the same
instant clobbered each other"* shared only the stopwords "two"/"same" with the target (content-
overlap **0**). That recovery under `a` was itself stopword inflation; dropping it is correct.
So `a*1` loses **zero genuine recall**. The noise gain (net +5 blocked) is directional, not
significant at n=24 — but it is free (no tuned constant), so it ships.

## Why the BM25 floor was refuted (b′ / c)

θ-swept on the full set, the bm25 floor traces the *same* recall/precision frontier the count knob
spans (e.g. b′@-7 = 22/33,0/24 ≈ a\* = 22/33,1/24; b′@-3 = 33/33,16/24 ≈ a = 33/33,18/24). It adds
nothing the count lever doesn't already give — at the cost of a corpus-calibrated θ. The module doc
explicitly defers a tuned relevance constant to Slice B; a redundant knob that needs one fails the
"earns its keep" bar. Honest null on its pre-registered gate ("b′ beats a on recovery"): a recovers
33/33, so the gate is unreachable → no-ship, per integrity constraint (criteria not moved to fit).

## Residual & the Slice-B handoff

`a*1` still leaks 13/24 on this corpus — coincidental single-content-token matches ("version",
"memory", "search" shared with an unrelated task). No lexical floor can separate a *topical*
single-token match from a *coincidental* one; only semantics can. That is precisely the job of
**Slice B (embed-on-write + RRF + rerank)** — the residual is a Slice-B problem, not a lexical-floor
one. The 36-row corpus also *overstates* the leak: a weak single-token match rarely survives top-K
once the store holds hundreds of rows, so real-world precision is better than 13/24.

## What shipped (code)

- `crates/board/src/memory.rs`: `STOPWORDS` const + `content_tokens()`; `recall_primed` floor now
  keeps a hit iff it shares ≥1 content token with the query (was: ≥min(2,n) raw tokens).
- Locked regression test `recall_primed_floor_keeps_content_drops_stopword_only` pins a\*1 between
  the old floor (drops the kept row) and naive relaxation (leaks the dropped row).
- Eval retained as `#[ignore]` `t5_floor_eval_v2` (reproducible measurement, not a CI gate).
