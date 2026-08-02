//! Tool policy — slice 3. The Align gate is no longer a standalone `enum Phase`:
//! a tool's permission is derived from the *board ticket's spine status*. Mutating
//! tools unlock only once the ticket is past the human Align gate (`in_progress`
//! onward), so the human-cleared `criteria_confirmed` transition transitively
//! gates mutation — the Phase 0 behaviour, now ticket-scoped and spine-native.
//!
//! Board stays tool-agnostic; this mapping (spine status → tool policy) is the
//! agent's concern, kept on its side of the protocol boundary.

use board::Status;

/// Whether a tool mutates state (and is therefore gated before execution).
/// Read-only tools are always allowed; everything else needs an execution status.
pub fn tool_is_mutating(name: &str) -> bool {
	!matches!(name, "read_file")
}

/// Do mutating tools run in this status? True only in the execution band
/// (`in_progress`..`land`). `todo`/`align`/`rework` are planning-or-reset
/// (read-only); `done` is finished (read-only).
pub fn mutating_allowed(status: Status) -> bool {
	matches!(status, Status::InProgress | Status::Verify | Status::Review | Status::Land)
}

/// The gate decision for a tool given the ticket's status — a rule, not a vibe.
pub fn gate_allows(status: Status, tool: &str) -> bool {
	!tool_is_mutating(tool) || mutating_allowed(status)
}

#[cfg(test)]
mod tests {
	use super::*;
	use board::{Board, GateSource};

	// The tool gate maps over the whole spine exactly as intended.
	#[test]
	fn gate_maps_status_to_tool_policy() {
		for s in [Status::Todo, Status::Align, Status::Rework, Status::Done] {
			assert!(gate_allows(s, "read_file"), "{s:?} allows reads");
			assert!(!gate_allows(s, "write_file"), "{s:?} blocks writes");
			assert!(!gate_allows(s, "edit_file"), "{s:?} blocks edits");
			assert!(!gate_allows(s, "bash"), "{s:?} blocks bash");
		}
		for s in [Status::InProgress, Status::Verify, Status::Review, Status::Land] {
			assert!(gate_allows(s, "read_file"));
			assert!(gate_allows(s, "write_file"), "{s:?} allows writes");
			assert!(gate_allows(s, "edit_file"), "{s:?} allows edits");
			assert!(gate_allows(s, "bash"), "{s:?} allows bash");
		}
	}

	// Success criterion #2 (no oMLX): the tool gate flips from DENY to ALLOW
	// exactly at the human-cleared align->in_progress transition, and rework
	// re-locks it. This ties the agent's tool policy to the board's enforced
	// spine — the slice-3 integrity property.
	#[test]
	fn human_gate_flips_tool_policy_and_rework_relocks() {
		let b = Board::open(":memory:").unwrap();
		b.create_ticket("w1", "build", "wire test", 2).unwrap();
		b.set_acceptance_criteria("w1", "write_file is permitted only post-gate").unwrap();

		// todo: writes denied
		assert!(!gate_allows(b.get("w1").unwrap().status, "write_file"));

		// move into align; still denied, and the gate blocks in_progress with no human pass
		b.set_status("w1", Status::Align).unwrap();
		assert!(!gate_allows(b.get("w1").unwrap().status, "write_file"));
		assert!(b.set_status("w1", Status::InProgress).is_err(), "no human pass yet");

		// human clears criteria_confirmed -> transition succeeds -> writes ALLOWED
		b.report_gate("w1", board::GATE_CRITERIA_CONFIRMED, "gary", GateSource::Human, true, None)
			.unwrap();
		b.set_status("w1", Status::InProgress).unwrap();
		assert!(gate_allows(b.get("w1").unwrap().status, "write_file"), "post-gate: writes unlock");

		// rework re-enters align (bumping attempt) -> writes DENIED again
		b.set_status("w1", Status::Rework).unwrap();
		b.set_status("w1", Status::Align).unwrap();
		assert!(!gate_allows(b.get("w1").unwrap().status, "write_file"), "rework re-locks writes");
		assert!(
			b.set_status("w1", Status::InProgress).is_err(),
			"prior attempt's pass must not satisfy the re-entered gate"
		);
	}
}
