//! Self-evolution — Half A, the read side only (research/20 §9).
//!
//! The compounding loop's load-bearing half: the agent loop reads the committed,
//! human-approved `wiki/case-law.md` and injects its lessons into every worker's
//! system prompt. That single read is the whole irreversible payoff — a lesson,
//! once approved into the file, changes every future run's contract.
//!
//! Half B (the `agent reflect` propose instrument, oMLX distiller, critic panel,
//! proposals dir) is **deferred** until a measured trigger (§9.1): the pond is
//! empty (the only `lesson` writer emits ticket-scoped post-mortems, zero general
//! lessons to distill), and the deferred machinery was unsafe/incorrect as
//! specified (§9.2). Claude-in-the-loop already distills better than an oMLX pass.
//!
//! This module is the *tiny* pure core the read path needs — extract lessons from
//! the raw markdown, drop any that trip the gate-weakening screen, apply a size
//! budget. No git, no model, no clock, so the logic is unit-tested in isolation
//! (the explore.rs "metrics are just code" style).
//!
//! **Safety note (honest scoping, §9.3.4):** human approval at commit time is the
//! PRIMARY boundary. `mentions_gate_weakening` is defense-in-depth only — it runs
//! on already-approved text, drops on a match, and NEVER certifies anything
//! "safe". It has false negatives by construction (any paraphrase escapes a
//! keyword screen), so it is a flag/drop last resort, not the gate.

/// Verbs that signal an intent to lower, skip, or fake a gate. Matched against a
/// lowercased bullet alongside a gate reference — neither alone trips the screen.
const WEAKENING_VERBS: &[&str] = &[
	"skip",
	"bypass",
	"ignore",
	"disable",
	"weaken",
	"lower",
	"relax",
	"loosen",
	"circumvent",
	"sidestep",
	"work around",
	"workaround",
	"turn off",
	"fake",
	"fabricate",
	"falsify",
];

/// Result of preparing raw case-law markdown for injection.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Prepared {
	/// Lesson bullets to inject, in file order, post-screen and post-budget.
	pub bullets: Vec<String>,
	/// Count dropped by the gate-weakening screen (defense-in-depth).
	pub dropped_unsafe: usize,
	/// The actual text of each screen-dropped bullet, so the caller can surface
	/// *what* was screened (the screen over-drops legitimate gate-describing
	/// lessons — pre-land review #1; making the drop visible lets a curator reword
	/// rather than wonder why a committed lesson vanished). Length == `dropped_unsafe`.
	pub dropped_unsafe_texts: Vec<String>,
	/// Count dropped by the size budget (overflow past `max_bullets`).
	pub dropped_budget: usize,
}

/// Flag a bullet that *appears* to advise weakening a named gate. Defense-in-depth
/// only (§9.3.4): runs on human-approved text, returns `true` to drop on a match,
/// and never certifies "safe". A match requires BOTH a gate reference (a gate
/// const value, one of its underscore-split tokens ≥4 chars, or the generic word
/// "gate") AND a weakening verb — so an ordinary lesson that merely mentions
/// "tests" is not dropped. Has false negatives by construction; the real boundary
/// is the human approving the file.
pub fn mentions_gate_weakening(text: &str, gate_names: &[&str]) -> bool {
	let t = text.to_lowercase();

	// Build the set of gate references: the generic word, each gate name, and each
	// of its underscore-split tokens (≥4 chars, so "score"/"green"/"tests" count
	// but a stray short token can't).
	let mut refs: Vec<String> = vec!["gate".to_string()];
	for g in gate_names {
		let g = g.to_lowercase();
		for tok in g.split('_') {
			if tok.len() >= 4 {
				refs.push(tok.to_string());
			}
		}
		refs.push(g);
	}

	let names_a_gate = refs.iter().any(|r| t.contains(r.as_str()));
	if !names_a_gate {
		return false;
	}
	WEAKENING_VERBS.iter().any(|v| t.contains(v))
}

/// Extract lesson bullets from raw case-law markdown, drop any that trip the
/// gate-weakening screen, then apply the size budget. Pure + deterministic.
///
/// **Section-aware:** only markdown list items (`- ` / `* `) under a `## Lessons`
/// heading are harvested. The human-facing editing contract (the prose + bullets
/// above `## Lessons`) is meta-instruction for the curator, not guidance for the
/// worker, so it never reaches the prompt — a heading toggles harvesting on at a
/// `Lessons` heading and off at any other heading. `max_bullets == 0` injects
/// nothing (a kill switch). The budget is applied AFTER the unsafe screen so an
/// unsafe bullet can't consume a slot and silently push a good lesson over the
/// budget edge.
pub fn prepare_caselaw(raw: &str, gate_names: &[&str], max_bullets: usize) -> Prepared {
	let mut kept: Vec<String> = Vec::new();
	let mut dropped_unsafe_texts: Vec<String> = Vec::new();
	let mut in_lessons = false;

	for line in raw.lines() {
		let trimmed = line.trim();

		// A heading toggles the lessons section: harvest only under `## Lessons`
		// (case-insensitive), so the contract prose above it stays out of the prompt.
		if let Some(rest) = trimmed.strip_prefix('#') {
			in_lessons = rest.trim_start_matches('#').trim().to_lowercase().starts_with("lessons");
			continue;
		}
		if !in_lessons {
			continue;
		}

		let bullet = trimmed
			.strip_prefix("- ")
			.or_else(|| trimmed.strip_prefix("* "))
			.map(str::trim);
		let Some(b) = bullet else { continue };
		if b.is_empty() {
			continue;
		}
		if mentions_gate_weakening(b, gate_names) {
			dropped_unsafe_texts.push(b.to_string());
			continue;
		}
		kept.push(b.to_string());
	}

	let dropped_budget = kept.len().saturating_sub(max_bullets);
	kept.truncate(max_bullets);
	Prepared { bullets: kept, dropped_unsafe: dropped_unsafe_texts.len(), dropped_unsafe_texts, dropped_budget }
}

#[cfg(test)]
mod tests {
	use super::*;

	// The real gate names the glue passes (board::spine const values).
	const GATES: &[&str] = &["criteria_confirmed", "tests_green", "mutation_score", "landed"];

	#[test]
	fn empty_input_yields_nothing() {
		let p = prepare_caselaw("", GATES, 20);
		assert!(p.bullets.is_empty());
		assert_eq!(p.dropped_unsafe, 0);
		assert_eq!(p.dropped_budget, 0);
	}

	#[test]
	fn prose_and_headers_are_ignored_only_bullets_kept() {
		let raw = "\
# Case-law

This file is human-approved. Prose like this is NOT a lesson.

## Lessons
- Prefer standard-library primitives over a wrapper dependency.
- State success criteria as a number or an artifact before starting.

Trailing prose, also ignored.";
		let p = prepare_caselaw(raw, GATES, 20);
		assert_eq!(p.bullets.len(), 2);
		assert!(p.bullets[0].starts_with("Prefer standard-library"));
		assert_eq!(p.dropped_unsafe, 0);
		assert_eq!(p.dropped_budget, 0);
	}

	#[test]
	fn star_bullets_and_indentation_are_handled() {
		let raw = "## Lessons\n  * indented star bullet\n\t- tab-indented dash bullet";
		let p = prepare_caselaw(raw, GATES, 20);
		assert_eq!(p.bullets, vec!["indented star bullet", "tab-indented dash bullet"]);
	}

	#[test]
	fn only_bullets_under_the_lessons_heading_are_harvested() {
		// the contract bullets above `## Lessons` are curator meta-instruction and
		// must NOT reach the prompt; a later non-lessons heading turns harvest off.
		let raw = "\
## The contract
- This bullet is editing guidance, not a worker lesson.
- Another contract note.

## Lessons
- A genuine lesson.

## Appendix
- Not a lesson either.";
		let p = prepare_caselaw(raw, GATES, 20);
		assert_eq!(p.bullets, vec!["A genuine lesson."]);
		assert_eq!(p.dropped_unsafe, 0);
		assert_eq!(p.dropped_budget, 0);
	}

	#[test]
	fn budget_drops_overflow_and_reports_count() {
		let raw = "## Lessons\n- one\n- two\n- three\n- four";
		let p = prepare_caselaw(raw, GATES, 2);
		assert_eq!(p.bullets, vec!["one", "two"]);
		assert_eq!(p.dropped_budget, 2);
		assert_eq!(p.dropped_unsafe, 0);
	}

	#[test]
	fn max_zero_is_a_kill_switch() {
		let raw = "## Lessons\n- a real lesson";
		let p = prepare_caselaw(raw, GATES, 0);
		assert!(p.bullets.is_empty());
		assert_eq!(p.dropped_budget, 1);
	}

	#[test]
	fn gate_weakening_bullet_is_dropped_and_counted() {
		let raw = "\
## Lessons
- Prefer trusting a green suite for everyday changes.
- When time is short, skip the tests_green gate to land faster.
- Keep functions small and focused.";
		let p = prepare_caselaw(raw, GATES, 20);
		assert_eq!(p.bullets.len(), 2, "the gate-weakening bullet is dropped");
		assert_eq!(p.dropped_unsafe, 1);
		assert!(p.bullets.iter().all(|b| !b.contains("skip the tests_green")));
		// the dropped text is surfaced so a curator can see/reword it (review #1)
		assert_eq!(p.dropped_unsafe_texts.len(), 1);
		assert!(p.dropped_unsafe_texts[0].contains("skip the tests_green"));
	}

	#[test]
	fn multiple_lessons_headings_accumulate_and_crlf_is_handled() {
		// two Lessons sections both harvest; CRLF line endings parse (the \r is
		// trimmed before the bullet prefix). Regression pins for silent breakage.
		let raw = "## Lessons\r\n- first\r\n## Notes\r\n- not a lesson\r\n## Lessons\r\n- second\r\n";
		let p = prepare_caselaw(raw, GATES, 20);
		assert_eq!(p.bullets, vec!["first", "second"]);
	}

	#[test]
	fn content_without_a_lessons_heading_yields_nothing() {
		// bullets present, but no `## Lessons` heading → nothing harvested.
		let raw = "# Title\nsome prose\n- an orphan bullet with no Lessons heading";
		let p = prepare_caselaw(raw, GATES, 20);
		assert!(p.bullets.is_empty());
		assert_eq!(p.dropped_unsafe, 0);
		assert_eq!(p.dropped_budget, 0);
	}

	#[test]
	fn unsafe_drop_does_not_consume_a_budget_slot() {
		// screen runs before budget: the unsafe line must not push "good3" out.
		let raw = "## Lessons\n- good1\n- bypass the mutation_score gate\n- good2\n- good3";
		let p = prepare_caselaw(raw, GATES, 3);
		assert_eq!(p.bullets, vec!["good1", "good2", "good3"]);
		assert_eq!(p.dropped_unsafe, 1);
		assert_eq!(p.dropped_budget, 0);
	}

	#[test]
	fn screen_requires_both_a_gate_ref_and_a_weakening_verb() {
		// gate ref, no verb → kept
		assert!(!mentions_gate_weakening("Run the tests_green gate every iteration", GATES));
		// weakening verb, no gate ref → kept
		assert!(!mentions_gate_weakening("Skip redundant logging in hot loops", GATES));
		// both → dropped
		assert!(mentions_gate_weakening("disable the mutation_score gate when slow", GATES));
		assert!(mentions_gate_weakening("just lower the criteria_confirmed bar", GATES));
	}

	#[test]
	fn screen_matches_human_friendly_gate_words() {
		// underscore-split tokens (≥4 chars) are matched, so prose phrasing trips it
		assert!(mentions_gate_weakening("ignore failing tests to move on", GATES));
		assert!(mentions_gate_weakening("fake the mutation results", GATES));
	}

	#[test]
	fn ordinary_lessons_survive_the_screen() {
		let raw = "\
## Lessons
- Write tests before the implementation; let them fail first.
- Mention the criteria in the workpad so the operator and agent agree.
- Gate by a number or an artifact, never a vibe.";
		let p = prepare_caselaw(raw, GATES, 20);
		assert_eq!(p.bullets.len(), 3, "none of these advise weakening a gate");
		assert_eq!(p.dropped_unsafe, 0);
	}
}
