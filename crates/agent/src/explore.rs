//! Parallel exploration — the PROBE (research/19 §9). A coordinator fans out N
//! *diverse* workers across isolated worktrees (`<ticket>-w<k>`), runs each
//! worker's validation in its own tree, and reports a ranked pass/fail table for
//! a human to inspect + land. It **reaps nothing, consolidates nothing, never
//! lands** — `explore` is an oMLX exercise that produces evidence, not a spine
//! mutation (the review fold-in cut the auto-consolidation/judge/synthesis
//! cathedral down to this instrument; see research/19 §9).
//!
//! This module holds the **pure selection core** — value types + the fan-out and
//! ranking functions — with no git and no model, so the search logic is unit-
//! tested in isolation (the "metrics are just code" style). The git + oMLX glue
//! (`run_explore`) lives in `main.rs` beside `run_ticket`/`run_validation`.

/// Hard ceiling on fan-out — a mis-set stakes field or a fat `--fanout` can't
/// spawn a swarm (research/19 §3.1 / risk-4). Also the reap-before-fanout bound:
/// the coordinator reaps `<ticket>-w0..w{CEIL-1}` so a crashed prior run can't
/// leave a contaminated worktree behind (review fold-in §9.3).
pub const FANOUT_CEILING: usize = 5;

/// How much breadth a ticket warrants — derived from its priority (the MVP stakes
/// proxy; research/19 §3.1). Distinct from the loop budget: stakes set *how many*
/// approaches to try, not how long each runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stakes {
	Low,
	Default,
	High,
}

/// Map a ticket `priority` to stakes. Convention: priority 1 = high (try more
/// approaches), 2 = default, ≥3 = low. Conservative at the edges so an unset/odd
/// priority lands on Default rather than spawning the max.
pub fn stakes_from_priority(priority: i64) -> Stakes {
	match priority {
		i if i <= 1 => Stakes::High,
		2 => Stakes::Default,
		_ => Stakes::Low,
	}
}

/// Fan-out width: an explicit `--fanout` override wins (clamped to `[1, CEIL]`),
/// otherwise stakes choose. Never returns 0 (a zero-worker explore is a no-op
/// that would silently "succeed") and never exceeds the ceiling.
pub fn fanout_for(stakes: Stakes, override_n: Option<usize>) -> usize {
	match override_n {
		Some(n) => n.clamp(1, FANOUT_CEILING),
		None => match stakes {
			Stakes::Low => 2,
			Stakes::Default => 3,
			Stakes::High => 4,
		},
	}
}

/// The fixed default strategy directives — diversity by construction, no planner
/// oMLX call (review fold-in §9.7: the planner-trait + a "distinct-string" gate
/// were premature; whether the strategies actually diverged is read off the
/// report's diff column, real evidence, not a string check). Each is injected
/// into one worker's system prompt. `n` is already `≤ FANOUT_CEILING`, so `take`
/// never pads; the cycle-guard is defensive only.
pub fn default_strategies(n: usize) -> Vec<String> {
	const DEFAULTS: [&str; FANOUT_CEILING] = [
		"Favor the simplest change that satisfies the criteria; minimize new code.",
		"Favor robustness: explicit error handling and edge-case coverage, even at the cost of more code.",
		"Favor reusing existing helpers and patterns already in the codebase over introducing new ones.",
		"Favor a test-first approach: write or extend tests to pin the behavior, then make them pass.",
		"Favor a direct, brute-force implementation; optimize only if the criteria demand it.",
	];
	(0..n).map(|i| DEFAULTS[i % DEFAULTS.len()].to_string()).collect()
}

/// One worker branch's outcome — the value type the ranking is computed over.
/// Carries only objective signal (no LLM judgment): did its validation pass, how
/// much did it change, how many loop iters, how did the loop stop. The human
/// reads the strategy + diff to judge whether the branches actually diverged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchOutcome {
	pub k: usize,
	pub strategy: String,
	pub passed: bool,
	pub lines_changed: usize,
	pub iters: i64,
	pub stop: String,
}

/// Rank survivors by **objective signal only** (review fold-in §9.6: the pairwise
/// judge was over-built before any evidence the objective gate is insufficient).
/// Order: validation-passers first; among equals, branches that produced a diff
/// first (a "successful" no-op that passed a weak validation sorts below real
/// work — risk-6); then worker index for a deterministic, stable tiebreak. The
/// caller treats `ranked.first()` (if it passed) as the winner *candidate* — a
/// human verifies and lands, explore never does.
pub fn rank_outcomes(mut outcomes: Vec<BranchOutcome>) -> Vec<BranchOutcome> {
	outcomes.sort_by_key(|o| {
		(
			u8::from(!o.passed),              // passers (false→0) first
			u8::from(o.lines_changed == 0),   // diff-producers (>0→0) first
			o.k,                              // deterministic tiebreak
		)
	});
	outcomes
}

#[cfg(test)]
mod tests {
	use super::*;

	fn outcome(k: usize, passed: bool, lines: usize) -> BranchOutcome {
		BranchOutcome { k, strategy: format!("s{k}"), passed, lines_changed: lines, iters: 1, stop: "completed".into() }
	}

	#[test]
	fn stakes_map_from_priority() {
		assert_eq!(stakes_from_priority(1), Stakes::High);
		assert_eq!(stakes_from_priority(0), Stakes::High, "≤1 is high");
		assert_eq!(stakes_from_priority(2), Stakes::Default);
		assert_eq!(stakes_from_priority(3), Stakes::Low);
		assert_eq!(stakes_from_priority(9), Stakes::Low);
	}

	#[test]
	fn fanout_by_stakes_and_override() {
		assert_eq!(fanout_for(Stakes::Low, None), 2);
		assert_eq!(fanout_for(Stakes::Default, None), 3);
		assert_eq!(fanout_for(Stakes::High, None), 4);
		// override wins, clamped to [1, CEIL] — a swarm or a no-op are both refused
		assert_eq!(fanout_for(Stakes::Low, Some(3)), 3);
		assert_eq!(fanout_for(Stakes::High, Some(99)), FANOUT_CEILING, "ceiling caps a fat override");
		assert_eq!(fanout_for(Stakes::Default, Some(0)), 1, "zero clamps up to one");
	}

	#[test]
	fn default_strategies_are_distinct_and_nonempty() {
		let s = default_strategies(3);
		assert_eq!(s.len(), 3);
		assert!(s.iter().all(|x| !x.trim().is_empty()), "no empty directive");
		// the three are pairwise distinct (diversity by construction, for n ≤ ceiling)
		assert_ne!(s[0], s[1]);
		assert_ne!(s[1], s[2]);
		assert_ne!(s[0], s[2]);
		assert_eq!(default_strategies(FANOUT_CEILING).len(), FANOUT_CEILING);
	}

	#[test]
	fn rank_passers_first_then_diff_then_index() {
		// a failing branch always sorts below a passing one, regardless of diff size
		let ranked = rank_outcomes(vec![outcome(0, false, 999), outcome(1, true, 1)]);
		assert_eq!(ranked[0].k, 1, "passer beats a bigger-diff failure");

		// among passers, a real diff beats a no-op (the weak-validation guard, risk-6)
		let ranked = rank_outcomes(vec![outcome(0, true, 0), outcome(1, true, 5)]);
		assert_eq!(ranked[0].k, 1, "diff-producer beats a passing no-op");

		// full equality → lower index wins (deterministic, stable)
		let ranked = rank_outcomes(vec![outcome(2, true, 5), outcome(0, true, 5), outcome(1, true, 5)]);
		assert_eq!(ranked.iter().map(|o| o.k).collect::<Vec<_>>(), vec![0, 1, 2]);
	}

	#[test]
	fn rank_empty_is_empty() {
		assert!(rank_outcomes(vec![]).is_empty(), "no branches → no ranking, no panic");
	}
}
