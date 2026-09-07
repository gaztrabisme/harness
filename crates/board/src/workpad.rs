//! The §5 workpad contract — one canonical render of a ticket's workpad, used
//! both for the human view (`agent show`) and the agent loop's system prompt, so
//! the two never drift. Pure: markdown from ticket data + an injected header.
//! The header (`<host>:<abs-path>@<short-sha>`) is host/VCS-derived, so the
//! caller computes it and the board stays VCS-agnostic.

use crate::model::{GateReport, Ticket};

/// Placeholder for an empty field — every section is always rendered so the
/// contract shape is stable and a reader always sees all five headings.
const NONE: &str = "_(none)_";

/// Render the workpad in the fixed §5 shape: a fenced header line, then the five
/// sections (Plan / Acceptance Criteria / Validation / Notes / Confusions),
/// always all present.
pub fn render(t: &Ticket, header: &str) -> String {
	let field = |v: &Option<String>| match v.as_deref().map(str::trim) {
		Some(s) if !s.is_empty() => s.to_string(),
		_ => NONE.to_string(),
	};
	format!(
		"## Workpad — {id}\n\
		 ```text\n{header}\n```\n\
		 **{title}**  ·  [{kind}] {status}  ·  attempt {attempt}\n\n\
		 ### Plan\n{plan}\n\n\
		 ### Acceptance Criteria\n{ac}\n\n\
		 ### Validation\n{val}\n\n\
		 ### Notes\n{notes}\n\n\
		 ### Confusions\n{conf}\n",
		id = t.id,
		title = t.title,
		kind = t.kind,
		status = t.status.as_str(),
		attempt = t.attempt,
		plan = field(&t.plan),
		ac = field(&t.acceptance_criteria),
		val = field(&t.validation),
		notes = field(&t.notes),
		conf = field(&t.confusions),
	)
}

/// The workpad plus a sixth `### Gates` section: every gate report ever recorded
/// for the ticket, in `seq` (report) order — the append-only history, INCLUDING
/// the reds a later green superseded. `agent show` is the per-ticket verb the
/// operator already types, so the history lives here rather than behind a new
/// one. The five-section pad the agent loop is prompted with is unchanged (see
/// `render`): this is the human view.
pub fn render_with_gates(t: &Ticket, header: &str, reports: &[GateReport]) -> String {
	format!("{}\n### Gates\n{}\n", render(t, header), gates_section(reports))
}

/// The Gates body: one line per report, `seq` order. Verdict first (it is what
/// the eye is looking for), then source/provider, the attempt epoch, and the
/// note that carries the evidence.
fn gates_section(reports: &[GateReport]) -> String {
	if reports.is_empty() {
		return NONE.to_string();
	}
	reports
		.iter()
		.map(|r| {
			let verdict = if r.passed { "PASS" } else { "FAIL" };
			let note = match r.note.as_deref().map(str::trim) {
				Some(n) if !n.is_empty() => format!(" — {n}"),
				_ => String::new(),
			};
			format!(
				"- #{seq}  {gate}  {verdict}  [{source}/{provider}]  attempt {attempt}  {at}{note}",
				seq = r.seq,
				gate = r.gate,
				source = r.source.as_str(),
				provider = r.provider,
				attempt = r.attempt,
				at = r.created_at,
			)
		})
		.collect::<Vec<_>>()
		.join("\n")
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::model::{GateSource, Status};

	fn ticket() -> Ticket {
		Ticket {
			id: "t1".into(),
			kind: "build".into(),
			status: Status::InProgress,
			title: "do the thing".into(),
			plan: Some("step 1".into()),
			acceptance_criteria: None,
			validation: Some("cargo test".into()),
			notes: None,
			confusions: None,
			priority: 2,
			attempt: 0,
		}
	}

	// All five sections, in order, with the header; set fields show their value,
	// empty fields show the placeholder (so the shape never collapses).
	#[test]
	fn renders_all_five_sections_in_order_with_header() {
		let out = render(&ticket(), "host:/ws@abc1234");
		assert!(out.contains("host:/ws@abc1234"), "header present");

		let order =
			["### Plan", "### Acceptance Criteria", "### Validation", "### Notes", "### Confusions"];
		let mut last = 0;
		for h in order {
			let at = out.find(h).unwrap_or_else(|| panic!("missing section {h}"));
			assert!(at >= last, "section {h} out of order");
			last = at;
		}

		assert!(out.contains("step 1"), "set plan rendered");
		assert!(out.contains("cargo test"), "set validation rendered");
		// acceptance + notes + confusions are empty → three placeholders
		assert_eq!(out.matches(NONE).count(), 3, "empty fields render the placeholder");
	}

	fn report(seq: i64, gate: &str, provider: &str, passed: bool, note: &str) -> GateReport {
		GateReport {
			seq,
			gate: gate.into(),
			provider: provider.into(),
			source: GateSource::Machine,
			attempt: 0,
			passed,
			note: Some(note.into()),
			created_at: "2026-08-17 09:00:00".into(),
		}
	}

	// The sixth section is the whole point of the append-only store: the RED that
	// a later green superseded is still on the page, in report order, with its
	// note. An empty history renders the placeholder, not a missing heading.
	#[test]
	fn gates_section_lists_every_report_in_seq_order_including_the_red() {
		let reports = vec![
			report(1, "mutation_score", "cargo-mutants", false, "score=0.400 thr=0.70"),
			report(2, "mutation_score", "cargo-mutants", true, "score=0.812 thr=0.70"),
		];
		let out = render_with_gates(&ticket(), "host:/ws@abc1234", &reports);

		let gates_at = out.find("### Gates").expect("Gates section present");
		assert!(gates_at > out.find("### Confusions").unwrap(), "Gates is the SIXTH section");

		let red = out.find("#1  mutation_score  FAIL").expect("the red report survives");
		let green = out.find("#2  mutation_score  PASS").expect("the green re-run is there too");
		assert!(red < green, "reports render in seq order");
		assert!(out.contains("score=0.400 thr=0.70"), "the red's evidence note is carried");
		assert!(out.contains("[machine/cargo-mutants]"), "source and provider are shown");
		assert!(out.contains("attempt 0"), "the attempt epoch is shown");

		// no history → the placeholder, so the section shape never collapses
		let empty = render_with_gates(&ticket(), "h", &[]);
		assert!(empty.contains("### Gates"), "the heading is unconditional");
		assert_eq!(empty.matches(NONE).count(), 4, "Gates adds a fourth placeholder when empty");
	}
}
