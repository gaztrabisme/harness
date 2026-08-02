//! Sprint coordinator — the **pure core** (critical-path #3; readiness assessment
//! gap "no multi-ticket orchestration"). One pass over the board's `runnable()` set
//! (in_progress + unblocked, priority-ordered): for each ticket, drive the machine
//! phases the harness already owns — worker run → harden → verify — then PARK at the
//! `review` status. The two human keystones stay human: a sprint never aligns and never
//! lands. Consequence: the runnable set cannot grow mid-sprint (only a human land
//! makes a blocker `done`), so a single snapshot is complete by construction and the
//! rhythm is sprint → human lands/aligns → sprint.
//!
//! Failure posture: park-and-continue. One ticket's failure (worker error, gate
//! refusal, bounced verify) is recorded in its entry and the sprint moves on — a
//! sprint exists to advance the whole queue, not to halt on the first red. Nothing
//! is retried automatically: a bounce or rework is a human decision (auto-retry
//! would burn budget re-running a worker the gates just rejected).
//!
//! This module holds the pure parts — the per-ticket entry and the operator summary
//! (what happened, and exactly what awaits the human next) — no board, no process.
//! The phase-sequencing glue (`run_sprint`) lives in `main.rs`.

/// What one sprint pass did to one ticket. `notes` are phase breadcrumbs in
/// execution order (`run=completed`, `verify=pass`, …); `final_status` is read back
/// from the BOARD after the phases ran — the artifact, not the intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
	pub id: String,
	pub notes: Vec<String>,
	pub final_status: String,
}

/// Render the operator summary: one line per dispatched ticket, then the two human
/// queues — the land queue (tickets the sprint parked at `review`) and the align
/// queue (ready Todo tickets the sprint may NOT touch). The queues are the point:
/// a sprint ends by telling the human exactly where their two keystones are needed.
pub fn render_summary(entries: &[Entry], awaiting_align: &[String]) -> String {
	let mut s = String::from("[sprint] ===== summary =====\n");
	if entries.is_empty() {
		s.push_str("  (nothing dispatched)\n");
	}
	for e in entries {
		s.push_str(&format!("  {}  → {}   [{}]\n", e.id, e.final_status, e.notes.join("  ")));
	}
	let land: Vec<&str> = entries.iter().filter(|e| e.final_status == "review").map(|e| e.id.as_str()).collect();
	if !land.is_empty() {
		s.push_str(&format!("\n  land queue (review + land are yours): {}\n", land.join(", ")));
	}
	let stuck: Vec<&str> = entries.iter().filter(|e| e.final_status != "review").map(|e| e.id.as_str()).collect();
	if !stuck.is_empty() {
		s.push_str(&format!("  needs attention (did not reach review): {}\n", stuck.join(", ")));
	}
	if !awaiting_align.is_empty() {
		s.push_str(&format!("  align queue (todo, unblocked): {}\n", awaiting_align.join(", ")));
	}
	s
}

#[cfg(test)]
mod tests {
	use super::*;

	fn entry(id: &str, status: &str, notes: &[&str]) -> Entry {
		Entry {
			id: id.into(),
			notes: notes.iter().map(|s| s.to_string()).collect(),
			final_status: status.into(),
		}
	}

	#[test]
	fn summary_splits_land_queue_from_needs_attention() {
		let entries = vec![
			entry("t1", "review", &["run=completed", "verify=pass"]),
			entry("t2", "in_progress", &["run=completed", "verify=FAIL(bounced)"]),
			entry("t3", "review", &["run=completed", "verify=pass"]),
		];
		let s = render_summary(&entries, &["t9".into()]);
		assert!(s.contains("land queue (review + land are yours): t1, t3"));
		assert!(s.contains("needs attention (did not reach review): t2"));
		assert!(s.contains("align queue (todo, unblocked): t9"));
		assert!(s.contains("verify=FAIL(bounced)"), "phase notes surface in the ticket line");
	}

	#[test]
	fn summary_handles_empty_dispatch_and_empty_queues() {
		let s = render_summary(&[], &[]);
		assert!(s.contains("(nothing dispatched)"));
		assert!(!s.contains("land queue"));
		assert!(!s.contains("align queue"));
	}
}
