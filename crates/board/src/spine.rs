//! The state machine — pure functions, no DB. Ported from `br`'s
//! `close_policy.rs` design (research/15 §2): an allowed-transition map +
//! `validate_transition` + the per-transition required-gate lookup.
//!
//! The map carries the non-destructive back-edges design-v2 §4 needs
//! (Confusions→Align, verify-bounce) so a minor miss doesn't force the
//! `Rework` hard-reset — C2's blocker #2.

use crate::model::{GateSource, Status};

/// Forward + back edges of the spine. `Rework` is handled separately (the
/// `any-non-terminal -> rework` wildcard), so it is not listed here.
fn forward_targets(from: Status) -> &'static [Status] {
	use Status::*;
	match from {
		Todo => &[Align],
		Align => &[InProgress],
		InProgress => &[Verify, Align],     // Align = Confusions back-edge (§4:144)
		Verify => &[Review, InProgress],    // InProgress = verify-miss bounce-back
		// InProgress = minor review fix; Done = the non-landing `close` path (research/32):
		// a non-code ticket that completes with nothing to land, or any kind abandoned.
		Review => &[Land, InProgress, Done],
		Land => &[Done],
		Rework => &[Align],                 // hard reset re-enters Align (§4:114)
		Done => &[],                        // terminal — absorbing
	}
}

/// Is `from -> to` a legal hop? `from == to` is a no-op (always legal, but the
/// board short-circuits it before side effects). `-> Rework` is legal from any
/// non-terminal state (the wildcard). Everything else must be in the map.
pub fn validate_transition(from: Status, to: Status) -> Result<(), String> {
	if from == to {
		return Ok(());
	}
	if to == Status::Rework {
		return if from.is_terminal() {
			Err(format!("cannot rework a terminal ticket ({})", from.as_str()))
		} else {
			Ok(())
		};
	}
	if forward_targets(from).contains(&to) {
		Ok(())
	} else {
		Err(format!("illegal transition {} -> {}", from.as_str(), to.as_str()))
	}
}

/// The gates a transition requires. Slice 1 enforces exactly one: the human
/// Align gate. `kind` is taken NOW (even though unused) so per-kind gate
/// profiles (§2.1 — `business-grounding` etc.) become a body change later, not
/// a callsite churn across the trunk — C3's seam fix #5.
pub fn required_gates_for(from: Status, to: Status, kind: &str) -> Vec<&'static str> {
	match (from, to) {
		// Align: human confirms the criteria before any execution (slice 1).
		(Status::Align, Status::InProgress) => vec![GATE_CRITERIA_CONFIRMED],
		// Verify: the validation command must pass (machine artifact) to leave
		// Verify (slice 4); code tickets ALSO need the Harden gate — a mutation
		// score on the diff, a number not a review (§7, the Harden pillar). The
		// gate keys off `kind`: mutation testing only applies to code work, so
		// non-code kinds (e.g. business-grounding) leave Verify on tests alone.
		(Status::Verify, Status::Review) if kind_is_code(kind) => {
			vec![GATE_TESTS_GREEN, GATE_MUTATION]
		}
		(Status::Verify, Status::Review) => vec![GATE_TESTS_GREEN],
		// Land: the keystone — only a human may land, and only a committed tree
		// (the §10 "git commit = human approval" gate; slice 4).
		(Status::Land, Status::Done) => vec![GATE_LANDED],
		// Close: the non-landing terminal path (research/32). A human resolution note
		// is the artifact — kind-blind here (the `close` verb adds the code-kind guard),
		// human-only (gate_source_required) so the agent loop cannot self-close.
		(Status::Review, Status::Done) => vec![GATE_RESOLVED],
		_ => vec![],
	}
}

/// Does this ticket kind produce source code the Harden gate can mutate?
/// Code kinds gate on a mutation score; non-code kinds (grounding, research,
/// docs) do not — mutation testing has nothing to mutate there. Conservative:
/// only kinds we KNOW are code count, so a novel kind doesn't silently acquire
/// an un-runnable gate that wedges its tickets in Verify.
pub fn kind_is_code(kind: &str) -> bool {
	matches!(kind, "build" | "bugfix" | "refactor")
}

/// Is `path` (repo-relative) a protected acceptance oracle — a test file the agent
/// must not edit to "pass" (finding #4, "never modify success criteria")? Three
/// conventions are covered: Python `test_*.py` / `*_test.py`, Rust integration
/// tests under a `tests/` directory, and validation checker scripts — a `check_*.py`
/// file whose immediate parent directory is literally named `scripts` (at any depth;
/// the PDSI t4 tamper window). The checker rule is deliberately narrow: the parent
/// must BE `scripts` and the name must start `check_` and end `.py`, so
/// `scripts/instantiate.py` and `check_`-prefixed files outside a `scripts/` dir
/// stay editable. NOT covered: Rust inline `#[cfg(test)]` units, which live in the
/// source the agent is supposed to edit — there's no way to protect them without
/// freezing the source, so a Rust ticket that wants a guarded oracle must put it in
/// `tests/`. The check is applied only to build|bugfix tickets (a refactor
/// legitimately rewrites tests).
pub fn is_protected_oracle(path: &str) -> bool {
	let file = path.rsplit('/').next().unwrap_or(path);
	let py = file.ends_with(".py") && (file.starts_with("test_") || file.ends_with("_test.py"));
	let rs_integration =
		path.ends_with(".rs") && path.split('/').any(|c| c == "tests");
	py || rs_integration || is_operator_checker(path)
}

/// Is `path` (repo-relative) an operator-authored validation checker — a
/// `check_*.py` file whose immediate parent directory is literally named
/// `scripts` (at any depth)? This is the checker arm of `is_protected_oracle`,
/// exposed on its own because checkers are the one oracle class NO provenance
/// exemption may ever unfreeze: a template-stamped `test_*.py` is template
/// output the ticket may legitimately regenerate, but `scripts/check_*.py` is
/// the operator's validation contract even when a template stamped its dir.
pub fn is_operator_checker(path: &str) -> bool {
	let mut components = path.rsplit('/');
	let file = components.next().unwrap_or(path);
	let parent = components.next();
	file.ends_with(".py") && file.starts_with("check_") && parent == Some("scripts")
}

/// The Align gate name — the spike's human `/align` confirmation, now a row.
pub const GATE_CRITERIA_CONFIRMED: &str = "criteria_confirmed";
/// The Verify gate — the ticket's validation command ran green (machine).
pub const GATE_TESTS_GREEN: &str = "tests_green";
/// The Land gate — the operator landed a committed tree (human; the §10 keystone).
pub const GATE_LANDED: &str = "landed";
/// The Harden gate — the diff's mutation score cleared threshold (machine; §7).
/// An agent provider can satisfy it (it's a computed number, not a human call).
pub const GATE_MUTATION: &str = "mutation_score";
/// The oracle-integrity guard (finding #4) — the acceptance test the run inherited
/// was not modified on the branch (machine; checked at verify for build|bugfix).
/// Recorded for evidence; NOT a transition gate in `required_gates_for` (a clean
/// run writes a pass row, a tampered run bails before reaching Review/Land).
pub const GATE_ORACLE_INTACT: &str = "oracle_intact";
/// The Close gate — a human resolution note closing a ticket to `Done` WITHOUT
/// landing (research/32): a non-code ticket that completes with nothing to land,
/// or any kind abandoned. Human-only — the note IS the artifact, and keeping it
/// human-sourced means the agent loop cannot self-close (the keystone holds for
/// `Review -> Done` exactly as it does for `Land -> Done`).
pub const GATE_RESOLVED: &str = "resolved";

/// Which source class a gate demands. Human gates cannot be machine-cleared
/// (an agent provider, which only has read/write/bash, can satisfy `tests_green`
/// by running tests but can NOT write `criteria_confirmed`/`landed`).
pub fn gate_source_required(gate: &str) -> GateSource {
	match gate {
		GATE_CRITERIA_CONFIRMED | GATE_LANDED | GATE_RESOLVED => GateSource::Human,
		_ => GateSource::Machine,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use Status::*;

	// The Harden gate is KIND-GATED (§7): a code kind needs both tests_green and
	// the mutation score to leave Verify→Review; a non-code kind needs tests_green
	// alone. This pins BOTH arms — the negative case is what stops `kind_is_code`
	// (or the match guard) from silently widening to "every kind is code", which is
	// exactly the survivor cargo-mutants flags when the negative case is untested.
	#[test]
	fn harden_gate_is_kind_gated() {
		// code kinds: tests_green + mutation_score
		for code in ["build", "bugfix", "refactor"] {
			assert!(kind_is_code(code), "{code} must be a code kind");
			assert_eq!(
				required_gates_for(Verify, Review, code),
				vec![GATE_TESTS_GREEN, GATE_MUTATION],
				"code kind {code} needs the Harden gate to leave Verify",
			);
		}
		// non-code kinds: tests_green ONLY — no un-runnable mutation gate wedging them
		for non in ["business-grounding", "research", "docs", "spike"] {
			assert!(!kind_is_code(non), "{non} must NOT be a code kind");
			assert_eq!(
				required_gates_for(Verify, Review, non),
				vec![GATE_TESTS_GREEN],
				"non-code kind {non} leaves Verify on tests alone",
			);
		}
	}

	// The non-landing `close` path (research/32): `Review -> Done` is a NEW legal edge,
	// gated by a HUMAN `resolved` note. Two assertions are load-bearing for the keystone:
	// (a) the edge exists ONLY from Review (no-skip held — InProgress->Done stays illegal),
	// (b) `resolved` is human-sourced, so the agent loop (machine) cannot self-close. If a
	// careless edit drops `resolved` into the `_ => Machine` arm, (b) fails here — the same
	// guard `harden_gate_is_kind_gated` gives the mutation gate.
	#[test]
	fn close_path_is_human_gated_and_review_only() {
		// (a) the new edge is legal, and only from Review — the no-skip invariant holds.
		assert!(validate_transition(Review, Done).is_ok(), "Review->Done is the close edge");
		assert_eq!(
			required_gates_for(Review, Done, "research"),
			vec![GATE_RESOLVED],
			"Review->Done is gated by the human resolution note",
		);
		// kind-blind at the spine (the code-kind guard lives in the `close` verb): a code
		// kind takes the SAME edge + gate (abandonment), not a different one.
		assert_eq!(required_gates_for(Review, Done, "build"), vec![GATE_RESOLVED]);
		for skip in [(Todo, Done), (InProgress, Done), (Verify, Done), (Align, Done)] {
			assert!(
				validate_transition(skip.0, skip.1).is_err(),
				"{:?}->Done must stay illegal (no structural skip to a terminal)",
				skip.0,
			);
		}
		// (b) the close gate is human-only — the agent provider cannot satisfy it.
		assert_eq!(gate_source_required(GATE_RESOLVED), GateSource::Human);
	}

	// The oracle convention (finding #4): Python test files and Rust integration
	// tests under `tests/` are protected; implementation source — including Rust
	// files that hold inline #[cfg(test)] units — is NOT (the agent must edit it).
	#[test]
	fn protected_oracle_convention() {
		// protected: python test files (both naming conventions) and rust tests/ dirs
		for p in [
			"dogfood/fix-merge/test_merge_intervals.py",
			"pkg/widget_test.py",
			"crates/agent/tests/loop_integration.rs",
			"tests/e2e.rs",
		] {
			assert!(is_protected_oracle(p), "{p} should be a protected oracle");
		}
		// NOT protected: implementation source the agent legitimately edits — including
		// Rust modules with inline unit tests, and non-test python impl.
		for p in [
			"dogfood/fix-merge/merge_intervals.py",
			"crates/agent/src/main.rs", // has #[cfg(test)] mod tests inline — still editable
			"crates/board/src/spine.rs",
			"src/contest.py", // "test" only as a substring, not a test file
			"README.md",
		] {
			assert!(!is_protected_oracle(p), "{p} must NOT be treated as an oracle");
		}
	}

	// The checker-script convention (PDSI t4's tamper window): `check_*.py` directly
	// under a `scripts/` dir is a validation oracle. Both halves matter — the parent
	// must be LITERALLY `scripts` and the name must start `check_`; the negative cases
	// pin the deliberate narrowness so scaffolding under scripts/ stays editable.
	#[test]
	fn checker_scripts_under_scripts_dir_are_protected() {
		// protected: check_*.py whose immediate parent dir is `scripts`, at any depth
		for p in ["scripts/check_ingest.py", "run/scripts/check_trace.py"] {
			assert!(is_protected_oracle(p), "{p} should be a protected oracle");
		}
		// NOT protected: wrong name prefix, or parent dir not literally `scripts`
		for p in [
			"scripts/instantiate.py", // under scripts/ but not a check_ script
			"scripts/checker.py",     // "check" prefix but not "check_"
			"app/check_util.py",      // check_ name but parent is not scripts
			"scripts/check_ingest",   // no .py extension
			"check_top.py",           // repo root — no parent dir at all
		] {
			assert!(!is_protected_oracle(p), "{p} must NOT be treated as an oracle");
		}
	}

	// `is_operator_checker` is the standalone checker arm: it must agree with
	// `is_protected_oracle` on every checker path, and must NOT claim the other
	// oracle classes (test files) — verify's stamp exemption relies on it to
	// carve checkers OUT of the exemption, so a false positive would freeze a
	// legitimately-editable file and a false negative would unfreeze a checker.
	#[test]
	fn operator_checker_predicate_matches_only_checkers() {
		for p in ["scripts/check_ingest.py", "run/scripts/check_trace.py"] {
			assert!(is_operator_checker(p), "{p} is an operator checker");
			assert!(is_protected_oracle(p), "every checker is also a protected oracle");
		}
		for p in [
			"scripts/instantiate.py",           // under scripts/ but not check_
			"app/check_util.py",                // check_ name, parent not scripts
			"scripts/check_ingest",             // no .py extension
			"check_top.py",                     // repo root — no parent dir
			"kata/test_kata.py",                // protected oracle, but NOT a checker
			"crates/agent/tests/loop_integration.rs", // rust oracle, NOT a checker
		] {
			assert!(!is_operator_checker(p), "{p} must NOT be an operator checker");
		}
	}
}
