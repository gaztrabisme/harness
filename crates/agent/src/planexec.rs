//! Plan→execute gate (research/24) — the *no-action stall*, the disjoint sibling
//! of the loop gate (research/22).
//!
//! The loop gate catches "calling the same tool forever" (action without
//! progress, in the ToolCalls branch). This gate catches the opposite failure:
//! the model emits a plan or analysis in prose, calls **no tool**, and the loop's
//! natural-stop branch accepts that as `completed` — banking a do-nothing run.
//! The single most-cited local-model failure across the breadcrumbs (Unit B
//! wrote a correct plan and zero files).
//!
//! The whole design hinges on **not** nudging a model that legitimately finished.
//! The objective signal is *whether the worktree actually changed* (`git::is_dirty`
//! at the natural stop), not which tool ran — the system prompt itself orders a
//! `bash ls -R` first, so a tool-based `acted` proxy would be true on every run
//! and the gate would never fire (research/24 §10, the BLOCKER fix). A code kind
//! (`board::spine::kind_is_code`) that stops having changed nothing is incomplete
//! by definition; a research/docs kind may legitimately write nothing, so it is
//! exempt.
//!
//! One pure decision function, unit-tested in isolation (the `loopgate.rs` /
//! `refeed.rs` style). The single-nudge bound makes it monotone — after one nudge
//! the next stop can only `Accept` (it acted) or `Stall` (it still didn't), both
//! terminal — so termination is guaranteed; `max_iters` backstops regardless.

/// What to do on a *natural stop* (the no-tool-call branch).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
	/// Legitimate stop — it changed the tree, or it is a non-code kind. Break,
	/// label as usual (`completed`/`truncated`).
	Accept,
	/// First no-action stop on a code ticket — inject one plan→execute nudge and
	/// continue the loop, giving the model exactly one chance to act.
	Nudge,
	/// Already nudged and *still* changed nothing — accept the exit but label it
	/// `stalled` (an honest failure), never `completed`.
	Stall,
}

/// Decide the verdict for a natural stop.
///
/// - `acted`  — did the worktree change this run? (`git::is_dirty` at the stop)
/// - `code`   — `kind_is_code(ticket.kind)`? (non-code kinds are exempt)
/// - `nudged` — has the single plan→execute nudge already been spent this run?
pub fn verdict(acted: bool, code: bool, nudged: bool) -> Verdict {
	if acted || !code {
		Verdict::Accept // did work, or a kind that need not → legitimate stop
	} else if !nudged {
		Verdict::Nudge // first no-action stop on a code ticket → one chance
	} else {
		Verdict::Stall // already nudged, still nothing → honest failure
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// The full acted×code×nudged matrix (8 cases). `acted` and non-code both mean
	// a legitimate stop regardless of the other inputs; the nudge↔stall split only
	// matters in the one quadrant that is both !acted and code.
	#[test]
	fn acted_always_accepts() {
		for code in [false, true] {
			for nudged in [false, true] {
				assert_eq!(
					verdict(true, code, nudged),
					Verdict::Accept,
					"acted=true is always a legitimate stop (code={code}, nudged={nudged})"
				);
			}
		}
	}

	#[test]
	fn non_code_is_exempt_even_with_no_action() {
		// a research/docs ticket may legitimately write nothing.
		assert_eq!(verdict(false, false, false), Verdict::Accept);
		assert_eq!(verdict(false, false, true), Verdict::Accept);
	}

	#[test]
	fn code_no_action_first_time_nudges() {
		assert_eq!(verdict(false, true, false), Verdict::Nudge);
	}

	#[test]
	fn code_no_action_after_nudge_stalls() {
		assert_eq!(verdict(false, true, true), Verdict::Stall);
	}

	// Termination: the bound is monotone — once nudged, the verdict can never be
	// Nudge again, so the loop cannot re-arm the nudge and spin.
	#[test]
	fn one_nudge_bound_is_monotone() {
		assert_eq!(verdict(false, true, false), Verdict::Nudge, "first: one chance");
		assert_eq!(verdict(false, true, true), Verdict::Stall, "second: terminal, never Nudge again");
		assert_ne!(verdict(false, true, true), Verdict::Nudge);
	}
}
