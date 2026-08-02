//! The §5 workpad contract — one canonical render of a ticket's workpad, used
//! both for the human view (`agent show`) and the agent loop's system prompt, so
//! the two never drift. Pure: markdown from ticket data + an injected header.
//! The header (`<host>:<abs-path>@<short-sha>`) is host/VCS-derived, so the
//! caller computes it and the board stays VCS-agnostic.

use crate::model::Ticket;

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

#[cfg(test)]
mod tests {
	use super::*;
	use crate::model::Status;

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
}
