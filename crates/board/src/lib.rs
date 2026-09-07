//! The board — slice 1 of the trunk. A rusqlite ticket store + the work-spine
//! state machine + gate engine, ported in design from `br`'s `close_policy.rs`
//! (research/15) with the adversarial-review corrections (C1/C2/C3) baked in.
//!
//! What this crate enforces:
//!  - legal transitions only (`spine::validate_transition`);
//!  - gates checked at the ticket's current attempt, so rework invalidates
//!    prior passes (no stale-pass bypass);
//!  - the Align gate is human-cleared (a machine source can't satisfy it);
//!  - `ready` = Todo with terminal direct blockers (cycle-safe by construction).
//!
//! Fenced OUT (later slices): per-kind gate profiles, transitive blocked-cache,
//! reconcile-first-on-entry, agent-raised confusions.

mod board;
mod memory;
mod model;
mod schema;
mod spine;
mod workpad;

pub use board::Board;
pub use memory::{NewMemory, clamp_primed_body};
pub use model::{
	GateReport, GateSource, MemoryHit, PrimedHit, Run, Scope, Status, Ticket, edge_kind_blocks,
};
pub use spine::{
	GATE_CRITERIA_CONFIRMED, GATE_LANDED, GATE_MUTATION, GATE_ORACLE_INTACT, GATE_RESOLVED,
	GATE_TESTS_GREEN, gate_source_required, is_operator_checker, is_protected_oracle, kind_is_code,
	required_gates_for,
	validate_transition,
};
pub use workpad::{render, render_with_gates};

#[cfg(test)]
mod tests {
	use super::*;

	fn mem() -> Board {
		Board::open(":memory:").unwrap()
	}

	/// Drive an already-created Todo ticket all the way to Done, clearing every
	/// gate along the spine. Used where a test just needs a terminal ticket.
	fn drive_to_done(b: &Board, id: &str) {
		b.set_status(id, Status::Align).unwrap();
		b.report_gate(id, GATE_CRITERIA_CONFIRMED, "gary", GateSource::Human, true, None).unwrap();
		b.set_status(id, Status::InProgress).unwrap();
		b.set_status(id, Status::Verify).unwrap();
		b.report_gate(id, GATE_TESTS_GREEN, "bash", GateSource::Machine, true, None).unwrap();
		// code kinds also need the Harden gate to leave Verify (§7); clearing a
		// gate a non-code kind doesn't require is harmless (just an unused row).
		b.report_gate(id, GATE_MUTATION, "cargo-mutants", GateSource::Machine, true, None).unwrap();
		b.set_status(id, Status::Review).unwrap();
		b.set_status(id, Status::Land).unwrap();
		b.report_gate(id, GATE_LANDED, "gary", GateSource::Human, true, None).unwrap();
		b.set_status(id, Status::Done).unwrap();
	}

	// Criterion #1 — schema creates; ticket + edge CRUD round-trip.
	#[test]
	fn schema_and_crud() {
		let b = mem();
		b.create_ticket("t1", "build", "first ticket", 1).unwrap();
		let t = b.get("t1").unwrap();
		assert_eq!(t.status, Status::Todo);
		assert_eq!(t.kind, "build");
		assert_eq!(t.attempt, 0);
		// edge insert is idempotent on the (issue, dep, kind) key
		b.add_edge("t1", "t2", "blocks").unwrap();
		b.add_edge("t1", "t2", "blocks").unwrap();
		// same pair, DIFFERENT kind must coexist (C1 #1 — kind in the PK)
		b.add_edge("t1", "t2", "parent-child").unwrap();
	}

	// Criterion #2 — validate_transition rejects an illegal hop, allows a legal one.
	#[test]
	fn transition_legality() {
		assert!(validate_transition(Status::Todo, Status::Done).is_err()); // skip the spine
		assert!(validate_transition(Status::Todo, Status::Align).is_ok());
		assert!(validate_transition(Status::Align, Status::InProgress).is_ok());
		// back-edges the design needs (C2 #2)
		assert!(validate_transition(Status::Verify, Status::InProgress).is_ok());
		assert!(validate_transition(Status::InProgress, Status::Align).is_ok());
		// rework is legal from non-terminal, illegal from Done (C2 #1)
		assert!(validate_transition(Status::InProgress, Status::Rework).is_ok());
		assert!(validate_transition(Status::Done, Status::Rework).is_err());
	}

	// Criterion #3 — the Align gate blocks align->in_progress until criteria_confirmed,
	// then allows it. And a MACHINE source cannot satisfy the human gate (C3 #1).
	#[test]
	fn align_gate_blocks_then_allows() {
		let b = mem();
		b.create_ticket("g1", "build", "gated", 2).unwrap();
		b.set_status("g1", Status::Align).unwrap();

		// blocked: no verdict yet
		assert!(b.set_status("g1", Status::InProgress).is_err());

		// a machine source is REJECTED for the human gate
		assert!(
			b.report_gate("g1", GATE_CRITERIA_CONFIRMED, "oMLX", GateSource::Machine, true, None)
				.is_err()
		);
		// still blocked
		assert!(b.set_status("g1", Status::InProgress).is_err());

		// human clears it → transition allowed
		b.report_gate("g1", GATE_CRITERIA_CONFIRMED, "gary", GateSource::Human, true, None)
			.unwrap();
		b.set_status("g1", Status::InProgress).unwrap();
		assert_eq!(b.get("g1").unwrap().status, Status::InProgress);
	}

	// Criterion #4 — ready excludes a Todo with an open blocker, includes it once
	// the blocker is Done; a 2-cycle yields neither (and does not hang).
	#[test]
	fn ready_respects_blockers_and_cycles() {
		let b = mem();
		b.create_ticket("a", "build", "blocked", 1).unwrap();
		b.create_ticket("c", "build", "blocker", 1).unwrap();
		b.add_edge("a", "c", "blocks").unwrap();

		// a is blocked by open c; c has no blockers
		assert_eq!(b.ready().unwrap(), vec!["c".to_string()]);

		// drive c to Done (clearing every spine gate) → a becomes ready
		drive_to_done(&b, "c");
		assert_eq!(b.ready().unwrap(), vec!["a".to_string()]);

		// a mutual cycle: neither is ready, and this returns (no infinite recursion)
		b.create_ticket("x", "build", "x", 1).unwrap();
		b.create_ticket("y", "build", "y", 1).unwrap();
		b.add_edge("x", "y", "blocks").unwrap();
		b.add_edge("y", "x", "blocks").unwrap();
		let ready = b.ready().unwrap();
		assert!(!ready.contains(&"x".to_string()));
		assert!(!ready.contains(&"y".to_string()));
	}

	// runnable = the post-Align sibling of ready: in_progress + unblocked only.
	// A ticket aligned before its blocker landed must PARK (running it would build
	// on a base missing the dependency's work), and Todo tickets never dispatch.
	#[test]
	fn runnable_is_in_progress_and_unblocked_only() {
		let b = mem();
		let align = |id: &str| {
			b.set_status(id, Status::Align).unwrap();
			b.report_gate(id, GATE_CRITERIA_CONFIRMED, "gary", GateSource::Human, true, None).unwrap();
			b.set_status(id, Status::InProgress).unwrap();
		};
		b.create_ticket("r1", "build", "free", 1).unwrap();
		b.create_ticket("r2", "build", "aligned but blocked", 1).unwrap();
		b.create_ticket("r3", "build", "open blocker", 2).unwrap();
		b.create_ticket("r4", "build", "todo never dispatches", 1).unwrap();
		b.add_edge("r2", "r3", "blocks").unwrap();
		align("r1");
		align("r2");

		assert_eq!(b.runnable().unwrap(), vec!["r1".to_string()], "r2 parks behind open r3; r4 is todo");

		drive_to_done(&b, "r3");
		assert_eq!(b.runnable().unwrap(), vec!["r1".to_string(), "r2".to_string()], "landed blocker frees r2");
		assert_eq!(b.ready().unwrap(), vec!["r4".to_string()], "ready still owns the todo side");
	}

	// Criterion #6 — entering Rework bumps attempt, so the prior attempt's
	// criteria_confirmed pass no longer satisfies the re-entered Align gate
	// (C1+C3 cross-confirmed blocker: stale-pass bypass on rework).
	#[test]
	fn rework_invalidates_prior_gate_pass() {
		let b = mem();
		b.create_ticket("r1", "build", "reworked", 2).unwrap();
		b.set_status("r1", Status::Align).unwrap();
		b.report_gate("r1", GATE_CRITERIA_CONFIRMED, "gary", GateSource::Human, true, None)
			.unwrap();
		b.set_status("r1", Status::InProgress).unwrap();
		assert_eq!(b.get("r1").unwrap().attempt, 0);

		// send it back: in_progress -> rework (bumps attempt) -> align
		b.set_status("r1", Status::Rework).unwrap();
		assert_eq!(b.get("r1").unwrap().attempt, 1, "rework bumps the attempt counter");
		b.set_status("r1", Status::Align).unwrap();

		// the attempt-0 pass must NOT satisfy the attempt-1 gate
		assert!(
			b.set_status("r1", Status::InProgress).is_err(),
			"stale pass from attempt 0 must not clear the re-entered gate"
		);

		// re-confirm at the current attempt → unblocked again
		b.report_gate("r1", GATE_CRITERIA_CONFIRMED, "gary", GateSource::Human, true, None)
			.unwrap();
		b.set_status("r1", Status::InProgress).unwrap();
		assert_eq!(b.get("r1").unwrap().status, Status::InProgress);
	}

	// Slice 4 — the back-half gates are real: verify->review needs a MACHINE
	// tests_green; land->done needs a HUMAN landed (an agent can't self-land);
	// and rework re-locks both via the attempt-epoch.
	#[test]
	fn verify_and_land_gates_enforced() {
		let b = mem();
		b.create_ticket("v1", "build", "back half", 2).unwrap();
		b.set_status("v1", Status::Align).unwrap();
		b.report_gate("v1", GATE_CRITERIA_CONFIRMED, "gary", GateSource::Human, true, None).unwrap();
		b.set_status("v1", Status::InProgress).unwrap();
		b.set_status("v1", Status::Verify).unwrap();

		// verify->review blocked until tests_green; a HUMAN source can't satisfy a machine gate
		assert!(b.set_status("v1", Status::Review).is_err(), "no tests_green yet");
		assert!(
			b.report_gate("v1", GATE_TESTS_GREEN, "gary", GateSource::Human, true, None).is_ok(),
			"a human-sourced row is recordable but won't satisfy a machine gate"
		);
		assert!(b.set_status("v1", Status::Review).is_err(), "human row doesn't satisfy machine gate");
		b.report_gate("v1", GATE_TESTS_GREEN, "bash", GateSource::Machine, true, None).unwrap();
		// a code ticket ("build") ALSO needs the Harden gate (§7) — tests_green alone won't pass.
		assert!(
			b.set_status("v1", Status::Review).is_err(),
			"a code ticket needs mutation_score too, not just tests_green"
		);
		b.report_gate("v1", GATE_MUTATION, "cargo-mutants", GateSource::Machine, true, None).unwrap();
		b.set_status("v1", Status::Review).unwrap();

		// land->done blocked until landed; a MACHINE source is REJECTED (agent can't self-land)
		b.set_status("v1", Status::Land).unwrap();
		assert!(b.set_status("v1", Status::Done).is_err(), "no landed yet");
		assert!(
			b.report_gate("v1", GATE_LANDED, "oMLX", GateSource::Machine, true, None).is_err(),
			"the land gate is human-only — an agent provider cannot clear it"
		);
		b.report_gate("v1", GATE_LANDED, "gary", GateSource::Human, true, Some("deadbeef")).unwrap();
		b.set_status("v1", Status::Done).unwrap();
		assert_eq!(b.get("v1").unwrap().status, Status::Done);

		// rework re-locks: a fresh ticket reworked from verify loses its attempt-0 tests_green
		let b2 = mem();
		b2.create_ticket("v2", "build", "relock", 2).unwrap();
		b2.set_status("v2", Status::Align).unwrap();
		b2.report_gate("v2", GATE_CRITERIA_CONFIRMED, "gary", GateSource::Human, true, None).unwrap();
		b2.set_status("v2", Status::InProgress).unwrap();
		b2.set_status("v2", Status::Verify).unwrap();
		b2.report_gate("v2", GATE_TESTS_GREEN, "bash", GateSource::Machine, true, None).unwrap();
		b2.report_gate("v2", GATE_MUTATION, "cargo-mutants", GateSource::Machine, true, None).unwrap();
		b2.set_status("v2", Status::Rework).unwrap(); // bumps attempt to 1
		b2.set_status("v2", Status::Align).unwrap();
		b2.report_gate("v2", GATE_CRITERIA_CONFIRMED, "gary", GateSource::Human, true, None).unwrap();
		b2.set_status("v2", Status::InProgress).unwrap();
		b2.set_status("v2", Status::Verify).unwrap();
		// re-record only the Harden gate at attempt 1 → the stale attempt-0
		// tests_green is now the SOLE gap, isolating the attempt-epoch re-lock.
		b2.report_gate("v2", GATE_MUTATION, "cargo-mutants", GateSource::Machine, true, None).unwrap();
		assert!(
			b2.set_status("v2", Status::Review).is_err(),
			"attempt-0 tests_green must not satisfy the attempt-1 verify gate"
		);
	}

	// research/32 — the non-landing `close` path: `Review -> Done` is gated by a HUMAN
	// `resolved` note (an agent can't self-close, exactly like land), and the verdict is
	// attempt-scoped so a stale pass can't survive a rework. Mirrors the land-gate test
	// for the new terminal edge.
	#[test]
	fn close_gate_enforced_and_attempt_scoped() {
		let b = mem();
		// a non-code kind reaches Review on tests_green alone (no Harden gate).
		b.create_ticket("k1", "research", "deep-research spike", 2).unwrap();
		b.set_status("k1", Status::Align).unwrap();
		b.report_gate("k1", GATE_CRITERIA_CONFIRMED, "gary", GateSource::Human, true, None).unwrap();
		b.set_status("k1", Status::InProgress).unwrap();
		b.set_status("k1", Status::Verify).unwrap();
		b.report_gate("k1", GATE_TESTS_GREEN, "bash", GateSource::Machine, true, None).unwrap();
		b.set_status("k1", Status::Review).unwrap();

		// review->done blocked until resolved; a MACHINE source is REJECTED (agent can't self-close)
		assert!(b.set_status("k1", Status::Done).is_err(), "no resolved note yet");
		assert!(
			b.report_gate("k1", GATE_RESOLVED, "oMLX", GateSource::Machine, true, Some("auto")).is_err(),
			"the close gate is human-only — an agent provider cannot clear it",
		);
		assert!(b.set_status("k1", Status::Done).is_err(), "still blocked after a rejected machine row");
		// human resolves with a note (the artifact) → closed to Done without landing
		b.report_gate("k1", GATE_RESOLVED, "gary", GateSource::Human, true, Some("shipped externally; nothing to land"))
			.unwrap();
		b.set_status("k1", Status::Done).unwrap();
		assert_eq!(b.get("k1").unwrap().status, Status::Done);
		// auditable: a closed ticket carries NO landed row (distinguishes resolved-close from landed-done)
		assert!(!b.gate_satisfied("k1", GATE_LANDED).unwrap(), "a resolved-close never writes a landed pass");

		// attempt-epoch: a resolved pass at attempt 0 must not survive a rework (mirrors the
		// land/verify re-lock; this is the board half of research/32's stale-gate fix).
		b.create_ticket("k2", "research", "re-locked close", 2).unwrap();
		b.set_status("k2", Status::Align).unwrap();
		b.report_gate("k2", GATE_CRITERIA_CONFIRMED, "gary", GateSource::Human, true, None).unwrap();
		b.set_status("k2", Status::InProgress).unwrap();
		b.set_status("k2", Status::Verify).unwrap();
		b.report_gate("k2", GATE_TESTS_GREEN, "bash", GateSource::Machine, true, None).unwrap();
		b.set_status("k2", Status::Review).unwrap();
		b.report_gate("k2", GATE_RESOLVED, "gary", GateSource::Human, true, Some("note")).unwrap();
		assert!(b.gate_satisfied("k2", GATE_RESOLVED).unwrap(), "resolved holds at attempt 0");
		b.set_status("k2", Status::Rework).unwrap(); // legal from Review (wildcard); bumps attempt
		assert!(
			!b.gate_satisfied("k2", GATE_RESOLVED).unwrap(),
			"a stale attempt-0 resolved must not satisfy the re-entered close gate",
		);
	}

	// Slice 2 — notes/confusions persist and emit a workpad_edited event each.
	#[test]
	fn notes_and_confusions_persist_and_audit() {
		let b = mem();
		b.create_ticket("w1", "build", "pad", 2).unwrap();
		let before = b.event_count("w1").unwrap();
		b.set_notes("w1", "reproduced on main").unwrap();
		b.set_confusions("w1", "which config path?").unwrap();
		let t = b.get("w1").unwrap();
		assert_eq!(t.notes.as_deref(), Some("reproduced on main"));
		assert_eq!(t.confusions.as_deref(), Some("which config path?"));
		assert_eq!(b.event_count("w1").unwrap(), before + 2, "two workpad edits audited");
	}

	// Supporting: the no-op self-transition writes no event (C2 #4), and
	// dangling blockers are linted, not silently resolved (C1 #2).
	#[test]
	fn noop_self_transition_and_dangling_lint() {
		let b = mem();
		b.create_ticket("n1", "build", "noop", 2).unwrap();
		let before = b.event_count("n1").unwrap(); // 1 (created)
		b.set_status("n1", Status::Todo).unwrap(); // from == to
		assert_eq!(b.event_count("n1").unwrap(), before, "no-op writes no event");

		b.add_edge("n1", "ghost", "blocks").unwrap();
		// ghost isn't on the board: n1 stays ready (typo can't wedge it)…
		assert!(b.ready().unwrap().contains(&"n1".to_string()));
		// …but the dangling edge is surfaced as a lint
		assert_eq!(b.dangling_edges().unwrap(), vec![("n1".to_string(), "ghost".to_string())]);
	}

	// Unit A (research/17 §4) — a run opens at entry with NULL outcome fields (the
	// queryable crash label), then `finish_run` closes it; finishing an unknown
	// run is rejected (gate by an artifact). `runs_for` round-trips every column.
	#[test]
	fn run_record_entry_then_finish_and_crash_label() {
		let b = mem();
		b.create_ticket("rn", "build", "telemetry", 2).unwrap();
		b.start_run("rn-000-100", "rn", 0, "m", "oMLX", Some("{\"temperature\":0.3}"), Some("harness"))
			.unwrap();

		let runs = b.runs_for("rn").unwrap();
		assert_eq!(runs.len(), 1);
		let r = &runs[0];
		assert_eq!(r.run_id, "rn-000-100");
		assert_eq!(r.attempt, 0);
		assert_eq!(r.provider, "oMLX");
		assert_eq!(r.sampling.as_deref(), Some("{\"temperature\":0.3}"));
		assert_eq!(r.project.as_deref(), Some("harness"));
		// the crash label: an open run has no ended_at / iters / stop_reason
		assert!(r.ended_at.is_none(), "an open run is the queryable crash label");
		assert!(r.iters.is_none());
		assert!(r.stop_reason.is_none());

		// close it → outcome fields populate
		b.finish_run("rn-000-100", "completed", 5).unwrap();
		let r = &b.runs_for("rn").unwrap()[0];
		assert!(r.ended_at.is_some(), "finished run has ended_at");
		assert_eq!(r.stop_reason.as_deref(), Some("completed"));
		assert_eq!(r.iters, Some(5));

		// finishing a run that was never opened is an error, not a silent no-op
		assert!(b.finish_run("ghost", "completed", 1).is_err());
	}

	/// The gate reports recorded on a real board, in the order they were reported.
	///
	/// PROVENANCE, stated plainly: this is a RECONSTRUCTION, not a byte-copy. The
	/// board it describes (a cv-mapper project board) lives outside this worktree
	/// and this ticket may not read it, so the sequence is rebuilt from the counts
	/// measured off it and recorded in the ticket plan: 15 gate reports, 10
	/// surviving `gate_results` rows, `t1`/`oracle_intact` reported three times,
	/// `t1`/`mutation_score` twice, and ZERO surviving failures. The shape — which
	/// keys repeat, and how often — is the part under test; the exact wording of
	/// the notes is not.
	const RECORDED_GATE_REPORTS: &[(&str, &str, &str, GateSource, bool)] = &[
		// t1 — a build ticket that bounced three times off the oracle guard and
		// twice off the mutation threshold before it landed.
		("t1", GATE_CRITERIA_CONFIRMED, "gary", GateSource::Human, true),
		("t1", GATE_MUTATION, "cargo-mutants", GateSource::Machine, false),
		("t1", GATE_ORACLE_INTACT, "git", GateSource::Machine, false),
		("t1", GATE_ORACLE_INTACT, "git", GateSource::Machine, false),
		("t1", GATE_ORACLE_INTACT, "git", GateSource::Machine, true),
		("t1", GATE_MUTATION, "cargo-mutants", GateSource::Machine, true),
		("t1", GATE_TESTS_GREEN, "bash", GateSource::Machine, false),
		("t1", GATE_TESTS_GREEN, "bash", GateSource::Machine, true),
		("t1", GATE_LANDED, "gary", GateSource::Human, true),
		// t2 — one verify bounce, otherwise clean.
		("t2", GATE_CRITERIA_CONFIRMED, "gary", GateSource::Human, true),
		("t2", GATE_MUTATION, "cargo-mutants", GateSource::Machine, true),
		("t2", GATE_ORACLE_INTACT, "git", GateSource::Machine, true),
		("t2", GATE_TESTS_GREEN, "bash", GateSource::Machine, false),
		("t2", GATE_TESTS_GREEN, "bash", GateSource::Machine, true),
		("t2", GATE_LANDED, "gary", GateSource::Human, true),
	];

	// Replay a recorded board's gate reports through the store. The old key kept
	// one row per (issue, gate, provider, attempt), so this 15-report sequence
	// collapsed to 10 rows with EVERY failure erased by the re-run that fixed it —
	// which is exactly what the live boards look like: 112 reports, zero surviving
	// `passed = 0`. Append-only keeps all 15, failures included.
	#[test]
	fn replaying_a_recorded_board_keeps_every_report_including_the_failures() {
		let b = mem();
		for id in ["t1", "t2"] {
			b.create_ticket(id, "build", "recorded", 2).unwrap();
		}
		for (id, gate, provider, source, passed) in RECORDED_GATE_REPORTS {
			b.report_gate(id, gate, provider, *source, *passed, None).unwrap();
		}

		let all: Vec<_> =
			["t1", "t2"].iter().flat_map(|id| b.gate_reports(id).unwrap()).collect();
		assert_eq!(all.len(), RECORDED_GATE_REPORTS.len(), "every report is a row: 15 in, 15 stored");

		// what the OLD key would have kept: one row per (gate, provider, attempt)
		// per ticket. 10 — the count actually observed on that board.
		let mut keys: Vec<String> = RECORDED_GATE_REPORTS
			.iter()
			.map(|(id, gate, provider, _, _)| format!("{id}/{gate}/{provider}/0"))
			.collect();
		keys.sort();
		keys.dedup();
		assert_eq!(keys.len(), 10, "the old key would have collapsed these 15 reports to 10 rows");

		let failures: Vec<_> = all.iter().filter(|r| !r.passed).collect();
		assert!(!failures.is_empty(), "failures survive (the old store had ZERO across 112 reports)");
		assert_eq!(
			failures.iter().filter(|r| r.gate == GATE_ORACLE_INTACT).count(),
			2,
			"both t1 oracle_intact refusals are on the board",
		);
		// report order is preserved, and the reds sit before the green that fixed them
		let t1 = b.gate_reports("t1").unwrap();
		assert!(t1.windows(2).all(|w| w[0].seq < w[1].seq), "seq order is report order");
		let oracle: Vec<bool> =
			t1.iter().filter(|r| r.gate == GATE_ORACLE_INTACT).map(|r| r.passed).collect();
		assert_eq!(oracle, vec![false, false, true], "the three oracle reports, in order");
		// and the gate itself still reads clear — history costs nothing at the gate
		assert!(b.gate_satisfied("t1", GATE_ORACLE_INTACT).unwrap());
	}

	// Migration v3 on a POPULATED v2 board: the old table is rebuilt around the
	// new key, and every banked row comes across with its `created_at` intact
	// (including two rows that differ only by provider — the shape the old key
	// allowed). Fabricates the real v2 table, since that is what boards on disk
	// have.
	#[test]
	fn migration_v3_rebuilds_a_populated_v2_gate_table() {
		let path = std::env::temp_dir().join("harness-board-migrate-v3-test.db");
		let clean = || {
			let _ = std::fs::remove_file(&path);
			let _ = std::fs::remove_file(path.with_extension("db-wal"));
			let _ = std::fs::remove_file(path.with_extension("db-shm"));
		};
		clean();
		let p = path.to_str().unwrap();

		{
			let b = Board::open(p).unwrap();
			b.create_ticket("v1", "build", "banked", 2).unwrap();
		}
		// fabricate the v2 board: the pre-v3 gate table, three banked rows with
		// known timestamps, user_version knocked back to 2.
		{
			let c = rusqlite::Connection::open(&path).unwrap();
			c.execute_batch(
				"DROP TABLE gate_results;
				 CREATE TABLE gate_results (
				     issue_id  TEXT NOT NULL,
				     gate      TEXT NOT NULL,
				     provider  TEXT NOT NULL,
				     source    TEXT NOT NULL CHECK (source IN ('human','machine')),
				     attempt   INTEGER NOT NULL,
				     passed    INTEGER NOT NULL CHECK (passed IN (0, 1)),
				     note      TEXT,
				     created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
				     PRIMARY KEY (issue_id, gate, provider, attempt)
				 );
				 INSERT INTO gate_results VALUES
				   ('v1','criteria_confirmed','gary','human',0,1,'aligned','2026-08-06 16:26:42'),
				   ('v1','mutation_score','cargo-mutants','machine',0,1,'score=0.812','2026-08-07 15:45:43'),
				   ('v1','mutation_score','none','machine',0,1,'nothing to mutate','2026-08-07 15:46:01');
				 PRAGMA user_version=2;",
			)
			.unwrap();
		}
		// reopen → v3 rebuilds the table under the new key
		{
			let b = Board::open(p).unwrap();
			let rows = b.gate_reports("v1").unwrap();
			assert_eq!(rows.len(), 3, "every banked row survived the rebuild");
			assert_eq!(rows[0].gate, "criteria_confirmed", "in their original order");
			assert_eq!(rows[0].created_at, "2026-08-06 16:26:42", "with their original stamps");
			assert_eq!(rows[1].created_at, "2026-08-07 15:45:43");
			assert_eq!(rows[2].provider, "none", "including the two rows that differ only by provider");
			assert!(rows[0].seq < rows[1].seq && rows[1].seq < rows[2].seq, "seq follows the copy order");

			// and the rebuilt table is append-only: the key that used to overwrite now appends
			b.report_gate("v1", GATE_MUTATION, "cargo-mutants", GateSource::Machine, false, Some("regressed"))
				.unwrap();
			assert_eq!(b.gate_reports("v1").unwrap().len(), 4);

			let c = rusqlite::Connection::open(&path).unwrap();
			let v: i64 = c.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
			assert_eq!(v, 3, "board is at v3");
		}
		clean();
	}

	// Unit A — the user_version migrator (research/17 §8 M1) actually upgrades a
	// PRE-EXISTING v0 database: drop the run table + reset the version to simulate
	// a board created before the migration, reopen, and confirm the migrator
	// re-creates the table and advances the version (idempotently).
	#[test]
	fn migrator_upgrades_preexisting_v0_db() {
		let path = std::env::temp_dir().join("harness-board-migrate-test.db");
		let _ = std::fs::remove_file(&path);
		let _ = std::fs::remove_file(path.with_extension("db-wal"));
		let _ = std::fs::remove_file(path.with_extension("db-shm"));
		let p = path.to_str().unwrap();

		// fresh open: migrates to v1, the run table works
		{
			let b = Board::open(p).unwrap();
			b.create_ticket("m1", "build", "t", 2).unwrap();
			b.start_run("m1-000-1", "m1", 0, "x", "oMLX", None, None).unwrap();
			assert_eq!(b.runs_for("m1").unwrap().len(), 1);
			// bank a gate verdict too: the v3 rebuild must carry banked history
			// across, not just re-create an empty table.
			b.report_gate("m1", GATE_TESTS_GREEN, "bash", GateSource::Machine, false, Some("exit=1"))
				.unwrap();
		}
		// fabricate a v0 db: drop the run table, knock user_version back to 0
		{
			let c = rusqlite::Connection::open(&path).unwrap();
			c.execute_batch("DROP TABLE run; PRAGMA user_version=0;").unwrap();
			let v: i64 = c.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
			assert_eq!(v, 0, "fabricated a v0 db (pre-migration)");
		}
		// reopen → the migrator re-applies every pending step: run table restored,
		// version advanced to the latest (v2 once the memory migration landed).
		{
			let b = Board::open(p).unwrap();
			b.start_run("m1-000-2", "m1", 0, "x", "oMLX", None, None).unwrap();
			assert_eq!(b.runs_for("m1").unwrap().len(), 1, "migrator re-created the run table");
			// the banked gate report came through the v3 table rebuild intact
			let reports = b.gate_reports("m1").unwrap();
			assert_eq!(reports.len(), 1, "the pre-existing gate row survived the rebuild");
			assert_eq!(reports[0].gate, GATE_TESTS_GREEN);
			assert!(!reports[0].passed, "and kept its verdict");
			assert_eq!(reports[0].note.as_deref(), Some("exit=1"), "and its evidence note");
			let c = rusqlite::Connection::open(&path).unwrap();
			let v: i64 = c.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
			assert_eq!(v, 3, "migrator advanced user_version 0 -> latest (v3)");
		}

		let _ = std::fs::remove_file(&path);
		let _ = std::fs::remove_file(path.with_extension("db-wal"));
		let _ = std::fs::remove_file(path.with_extension("db-shm"));
	}
}
