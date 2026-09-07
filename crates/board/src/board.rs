//! `Board` — the rusqlite handle and the operations that enforce the spine.
//! `set_status` is the chokepoint: it validates the transition, checks gates at
//! the ticket's *current* attempt, bumps the attempt on entry to `Rework`, and
//! appends an audit event. Nothing mutates status except through here.

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, params};

use crate::model::{GateReport, GateSource, Run, Status, Ticket, edge_kind_blocks};
use crate::{schema, spine};

pub struct Board {
	conn: Connection,
}

impl Board {
	/// Open (or create) a board at `path`. `:memory:` is valid for tests.
	pub fn open(path: &str) -> Result<Self> {
		let conn = Connection::open(path).with_context(|| format!("opening board at {path}"))?;
		schema::init(&conn)?;
		Ok(Self { conn })
	}

	/// The underlying connection, for sibling op modules in this crate (e.g.
	/// `memory`) that build their own statements rather than re-housing every op
	/// in this file. Crate-private: the connection never escapes the board.
	pub(crate) fn conn(&self) -> &Connection {
		&self.conn
	}

	// ---- tickets ---------------------------------------------------------

	/// Create a ticket in `Todo`. Workpad fields fill in later.
	pub fn create_ticket(&self, id: &str, kind: &str, title: &str, priority: i64) -> Result<()> {
		self.conn
			.execute(
				"INSERT INTO ticket (id, kind, status, title, priority) VALUES (?1, ?2, 'todo', ?3, ?4)",
				params![id, kind, title, priority],
			)
			.with_context(|| format!("creating ticket {id}"))?;
		self.append_event(id, "created", None, Some(Status::Todo), None, None)?;
		Ok(())
	}

	pub fn get(&self, id: &str) -> Result<Ticket> {
		self.conn
			.query_row(
				"SELECT id, kind, status, title, plan, acceptance_criteria, validation,
				        notes, confusions, priority, attempt
				   FROM ticket WHERE id = ?1",
				params![id],
				Self::row_to_ticket,
			)
			.with_context(|| format!("ticket {id} not found"))
	}

	/// Map a full ticket row (the `get`/`all_tickets` column order) to a `Ticket`.
	fn row_to_ticket(r: &rusqlite::Row<'_>) -> rusqlite::Result<Ticket> {
		let status: String = r.get(2)?;
		Ok(Ticket {
			id: r.get(0)?,
			kind: r.get(1)?,
			status: status.parse().unwrap_or(Status::Todo),
			title: r.get(3)?,
			plan: r.get(4)?,
			acceptance_criteria: r.get(5)?,
			validation: r.get(6)?,
			notes: r.get(7)?,
			confusions: r.get(8)?,
			priority: r.get(9)?,
			attempt: r.get(10)?,
		})
	}

	/// Every ticket on the board, in overview order: spine position (todo → align →
	/// in_progress → verify → review → land → rework → done), then priority, then id.
	/// Read-only — the `agent board` overview; emits no events.
	pub fn all_tickets(&self) -> Result<Vec<Ticket>> {
		let mut stmt = self.conn.prepare(
			"SELECT id, kind, status, title, plan, acceptance_criteria, validation,
			        notes, confusions, priority, attempt
			   FROM ticket",
		)?;
		let mut rows =
			stmt.query_map([], Self::row_to_ticket)?.collect::<rusqlite::Result<Vec<_>>>()?;
		// spine order lives on `Status` (single source of the ordering); SQL can't
		// see it, so sort here rather than duplicating it in a CASE expression.
		rows.sort_by(|a, b| {
			(a.status.spine_pos(), a.priority, a.id.as_str())
				.cmp(&(b.status.spine_pos(), b.priority, b.id.as_str()))
		});
		Ok(rows)
	}

	/// Highest `t<N>` ticket sequence on the board, for id minting. `None` when
	/// no `t<N>`-shaped ids exist yet. (MAX returns one row — NULL maps to None.)
	pub fn max_ticket_seq(&self) -> Result<Option<i64>> {
		let n: Option<i64> = self.conn.query_row(
			"SELECT MAX(CAST(SUBSTR(id, 2) AS INTEGER)) FROM ticket WHERE id GLOB 't[0-9]*'",
			[],
			|r| r.get(0),
		)?;
		Ok(n)
	}

	/// Set the plan field (operator authoring; rich rendering is the workpad slice).
	pub fn set_plan(&self, id: &str, plan: &str) -> Result<()> {
		self.set_workpad_field(id, "plan", plan)
	}

	/// Set the acceptance-criteria field. `criteria_confirmed` confirms *these*.
	pub fn set_acceptance_criteria(&self, id: &str, criteria: &str) -> Result<()> {
		self.set_workpad_field(id, "acceptance_criteria", criteria)
	}

	/// Set the validation command — the shell command the Verify gate runs; its
	/// exit status is the `tests_green` artifact.
	pub fn set_validation(&self, id: &str, validation: &str) -> Result<()> {
		self.set_workpad_field(id, "validation", validation)
	}

	/// Set the Notes field (milestones, reproduction signal, sync evidence).
	pub fn set_notes(&self, id: &str, notes: &str) -> Result<()> {
		self.set_workpad_field(id, "notes", notes)
	}

	/// APPEND to the Notes field with a blank-line separator. Notes accumulate
	/// operator guidance across retries — a plain overwrite burned the operator
	/// (PDSI t5: retry guidance had to rebuild the whole pad). A NULL/empty pad
	/// takes the text as-is (no leading separator). `set_notes` stays the explicit
	/// overwrite (`agent note --replace`); confusions keep overwrite semantics BY
	/// DESIGN (the Align bounce reads one current confusion, not a history).
	pub fn append_notes(&self, id: &str, text: &str) -> Result<()> {
		let n = self
			.conn
			.execute(
				"UPDATE ticket
				    SET notes = CASE WHEN notes IS NULL OR notes = ''
				                     THEN ?1
				                     ELSE notes || char(10) || char(10) || ?1 END,
				        updated_at = CURRENT_TIMESTAMP
				  WHERE id = ?2",
				params![text, id],
			)
			.with_context(|| format!("appending notes on {id}"))?;
		if n == 0 {
			bail!("ticket {id} not found");
		}
		self.append_event(id, "workpad_edited", None, None, None, Some("notes"))?;
		Ok(())
	}

	/// Set the Confusions field — structured ambiguity. The agent CLI bounces an
	/// `in_progress` ticket back to Align when this is set (§5's Confusions→Align
	/// channel, re-opening the human gate).
	pub fn set_confusions(&self, id: &str, confusions: &str) -> Result<()> {
		self.set_workpad_field(id, "confusions", confusions)
	}

	/// Whitelisted single-column workpad update + audit event. The column name is
	/// a hard-coded literal at each caller — never user input — so the format! is
	/// not an injection surface.
	fn set_workpad_field(&self, id: &str, column: &str, value: &str) -> Result<()> {
		let n = self
			.conn
			.execute(
				&format!("UPDATE ticket SET {column} = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2"),
				params![value, id],
			)
			.with_context(|| format!("setting {column} on {id}"))?;
		if n == 0 {
			bail!("ticket {id} not found");
		}
		self.append_event(id, "workpad_edited", None, None, None, Some(column))?;
		Ok(())
	}

	/// The dispatch chokepoint. Validates the hop, enforces gates at the current
	/// attempt, bumps the attempt entering `Rework`, writes the audit event.
	pub fn set_status(&self, id: &str, to: Status) -> Result<()> {
		let t = self.get(id)?;
		let from = t.status;
		if from == to {
			return Ok(()); // typed no-op: no gate eval, no event, no timestamp churn (C2 #4)
		}
		if let Err(e) = spine::validate_transition(from, to) {
			bail!("{e}");
		}
		let missing = self.missing_gates(&t, to)?;
		if !missing.is_empty() {
			bail!(
				"transition {} -> {} blocked; unsatisfied gate(s): {}",
				from.as_str(),
				to.as_str(),
				missing.join(", ")
			);
		}
		let new_attempt = if to == Status::Rework { t.attempt + 1 } else { t.attempt };
		self.conn
			.execute(
				"UPDATE ticket SET status = ?1, attempt = ?2, updated_at = CURRENT_TIMESTAMP WHERE id = ?3",
				params![to.as_str(), new_attempt, id],
			)
			.with_context(|| format!("updating status of {id}"))?;
		self.append_event(id, "status_changed", Some(from), Some(to), None, None)?;
		Ok(())
	}

	// ---- edges -----------------------------------------------------------

	pub fn add_edge(&self, issue_id: &str, depends_on_id: &str, kind: &str) -> Result<()> {
		self.conn
			.execute(
				"INSERT OR IGNORE INTO edge (issue_id, depends_on_id, kind) VALUES (?1, ?2, ?3)",
				params![issue_id, depends_on_id, kind],
			)
			.context("adding edge")?;
		Ok(())
	}

	// ---- gates -----------------------------------------------------------

	/// Record a gate verdict at the ticket's current attempt. A human gate
	/// (`criteria_confirmed`) REJECTS a machine source — an agent provider
	/// cannot self-clear the Align gate (C3 #1).
	///
	/// APPEND-ONLY (schema v3): every report is its own row. The old key
	/// `(issue_id, gate, provider, attempt)` + `ON CONFLICT DO UPDATE` meant the
	/// re-run that fixed a gate overwrote the failure that motivated it, so a red
	/// verdict never survived the green that followed it — and `attempt` did not
	/// save it, because no red gate causes a rework. Nothing here overwrites now;
	/// `gate_satisfied` decides which row is in force.
	pub fn report_gate(
		&self,
		id: &str,
		gate: &str,
		provider: &str,
		source: GateSource,
		passed: bool,
		note: Option<&str>,
	) -> Result<()> {
		if spine::gate_source_required(gate) == GateSource::Human && source != GateSource::Human {
			bail!("gate '{gate}' is human-only; a {} source cannot satisfy it", source.as_str());
		}
		let attempt = self.get(id)?.attempt;
		self.conn
			.execute(
				"INSERT INTO gate_results (issue_id, gate, provider, source, attempt, passed, note)
				 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
				params![id, gate, provider, source.as_str(), attempt, passed as i64, note],
			)
			.context("recording gate verdict")?;
		self.append_event(id, "gate_reported", None, None, Some(provider), Some(gate))?;
		Ok(())
	}

	/// Is `gate` satisfied for `id` at its current attempt? LATEST REPORT WINS:
	/// among the rows at this attempt carrying the source class the gate demands,
	/// take the newest row *per provider* and require every one of them to pass
	/// (and at least one to exist).
	///
	/// Two things fall out of that, both deliberate:
	///  - a re-run supersedes its own earlier verdict (`seq` order is commit
	///    order), so the append-only history costs nothing at the gate: a red
	///    followed by a green from the same provider reads satisfied, a green
	///    followed by a red reads unsatisfied;
	///  - a red from ONE provider is not cleared by a pass from ANOTHER. That
	///    closes a live hole: `run_harden` reports `mutation_score` under the tool
	///    name when it measures something and under `none` when the diff has
	///    nothing to mutate, so a red cargo-mutants run followed by a vacuous
	///    stamped pass used to read satisfied with the failure sitting right
	///    beside it. Clearing it now takes a real re-run of the provider that
	///    failed, or a rework (which bumps the attempt epoch).
	///
	/// Public + read-only so a caller can *require a prerequisite gate* (e.g.
	/// `agent verify` refusing a code ticket that hasn't been hardened) without
	/// attempting a transition and parsing the error string — gate by an
	/// artifact, not by a caught error.
	pub fn gate_satisfied(&self, id: &str, gate: &str) -> Result<bool> {
		let attempt = self.get(id)?.attempt;
		let need = spine::gate_source_required(gate);
		// `seq` (autoincrement = commit order), never `created_at`: the timestamp
		// is second-resolution and same-second reports are routine.
		let (providers, passing): (i64, i64) = self.conn.query_row(
			"SELECT COUNT(*), COALESCE(SUM(passed), 0) FROM (
			     SELECT g.passed FROM gate_results g
			      WHERE g.issue_id = ?1 AND g.gate = ?2 AND g.attempt = ?3 AND g.source = ?4
			        AND g.seq = (SELECT MAX(h.seq) FROM gate_results h
			                      WHERE h.issue_id = g.issue_id AND h.gate = g.gate
			                        AND h.attempt = g.attempt AND h.source = g.source
			                        AND h.provider = g.provider)
			 )",
			params![id, gate, attempt, need.as_str()],
			|r| Ok((r.get(0)?, r.get(1)?)),
		)?;
		Ok(providers > 0 && passing == providers)
	}

	/// Every gate report ever recorded for `id`, oldest first. The append-only
	/// history `agent show` renders — including the reds that a later green
	/// superseded, which is the whole point of the store.
	pub fn gate_reports(&self, id: &str) -> Result<Vec<GateReport>> {
		let mut stmt = self.conn.prepare(
			"SELECT seq, gate, provider, source, attempt, passed, note, created_at
			   FROM gate_results WHERE issue_id = ?1 ORDER BY seq ASC",
		)?;
		let rows = stmt
			.query_map(params![id], |r| {
				let source: String = r.get(3)?;
				let passed: i64 = r.get(5)?;
				Ok(GateReport {
					seq: r.get(0)?,
					gate: r.get(1)?,
					provider: r.get(2)?,
					source: source.parse().unwrap_or(GateSource::Machine),
					attempt: r.get(4)?,
					passed: passed != 0,
					note: r.get(6)?,
					created_at: r.get(7)?,
				})
			})?
			.collect::<rusqlite::Result<Vec<_>>>()?;
		Ok(rows)
	}

	/// How many FAILING gate reports stand at the ticket's current attempt —
	/// the number the board/status one-liners carry so a red is visible without
	/// knowing the history exists. Counts reports, not gates: a gate reported red
	/// twice before it went green counts twice, because that is what happened.
	/// Earlier attempts are excluded (a bumped `attempt` already says the ticket
	/// was sent back).
	pub fn red_gate_count(&self, id: &str) -> Result<i64> {
		let attempt = self.get(id)?.attempt;
		Ok(self.conn.query_row(
			"SELECT COUNT(*) FROM gate_results
			  WHERE issue_id = ?1 AND attempt = ?2 AND passed = 0",
			params![id, attempt],
			|r| r.get(0),
		)?)
	}

	/// The pi board extension's close predicate, lifted verbatim from its bash
	/// shim: ids of every open (non-done) ticket lacking a PASSING `wiki-close`
	/// gate row dated today (UTC). A red does not satisfy — only today's pass
	/// does, regardless of attempt or latest-wins (`wiki-close` is a
	/// housekeeping gate, not a spine gate, so rework epochs don't apply; this
	/// is the shim's contract, kept byte-for-byte).
	pub fn close_check_missing(&self) -> Result<Vec<String>> {
		let mut stmt = self.conn.prepare(
			"SELECT id FROM ticket WHERE status <> 'done' AND id NOT IN (
			     SELECT issue_id FROM gate_results
			      WHERE gate = 'wiki-close' AND passed = 1 AND date(created_at) = date('now'))
			 ORDER BY id",
		)?;
		let rows = stmt.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
		Ok(rows)
	}

	/// The required gates for `t.status -> to` that are NOT satisfied at the
	/// ticket's current attempt (the gate verdicts `set_status` enforces).
	fn missing_gates(&self, t: &Ticket, to: Status) -> Result<Vec<String>> {
		let mut missing = Vec::new();
		for gate in spine::required_gates_for(t.status, to, &t.kind) {
			if !self.gate_satisfied(&t.id, gate)? {
				missing.push(gate.to_string());
			}
		}
		Ok(missing)
	}

	// ---- runs (telemetry) ------------------------------------------------

	/// Open a run record at loop entry — `ended_at`/`iters`/`stop_reason` stay
	/// NULL until `finish_run`, so an interrupted run is the queryable
	/// `ended_at IS NULL` rather than an orphan trajectory file (research/17 §8 M2).
	/// The caller mints `run_id` (it also derives the trajectory path from it).
	#[allow(clippy::too_many_arguments)]
	pub fn start_run(
		&self,
		run_id: &str,
		ticket_id: &str,
		attempt: i64,
		model: &str,
		provider: &str,
		sampling: Option<&str>,
		project: Option<&str>,
	) -> Result<()> {
		self.conn
			.execute(
				"INSERT INTO run (run_id, ticket_id, attempt, model, provider, sampling, project)
				 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
				params![run_id, ticket_id, attempt, model, provider, sampling, project],
			)
			.with_context(|| format!("opening run {run_id}"))?;
		Ok(())
	}

	/// Close a run at loop exit: stamp `ended_at` and record the outcome.
	/// `stop_reason` is derived from the observed loop exit (`completed` |
	/// `truncated` | `max_iters` | `error`); a crash that never reaches here leaves
	/// the row open (`ended_at IS NULL`).
	pub fn finish_run(&self, run_id: &str, stop_reason: &str, iters: i64) -> Result<()> {
		let n = self
			.conn
			.execute(
				"UPDATE run SET ended_at = CURRENT_TIMESTAMP, stop_reason = ?2, iters = ?3
				  WHERE run_id = ?1",
				params![run_id, stop_reason, iters],
			)
			.with_context(|| format!("finishing run {run_id}"))?;
		if n == 0 {
			bail!("run {run_id} not found");
		}
		Ok(())
	}

	/// Runs for a ticket, most-recent first (`run_id` is time-sortable).
	pub fn runs_for(&self, ticket_id: &str) -> Result<Vec<Run>> {
		let mut stmt = self.conn.prepare(
			"SELECT run_id, ticket_id, attempt, model, provider, sampling, project,
			        started_at, ended_at, iters, stop_reason
			   FROM run WHERE ticket_id = ?1 ORDER BY run_id DESC",
		)?;
		let rows = stmt
			.query_map(params![ticket_id], |r| {
				Ok(Run {
					run_id: r.get(0)?,
					ticket_id: r.get(1)?,
					attempt: r.get(2)?,
					model: r.get(3)?,
					provider: r.get(4)?,
					sampling: r.get(5)?,
					project: r.get(6)?,
					started_at: r.get(7)?,
					ended_at: r.get(8)?,
					iters: r.get(9)?,
					stop_reason: r.get(10)?,
				})
			})?
			.collect::<rusqlite::Result<Vec<_>>>()?;
		Ok(rows)
	}

	// ---- ready / lint ----------------------------------------------------

	/// Tickets ready to dispatch: status `Todo` with every *direct* blocker
	/// terminal. A blocker that isn't on the board is treated as non-blocking
	/// (see `dangling_edges` for the lint) — a typo cannot permanently wedge a
	/// ticket. No recursion: a blocked blocker is simply still non-terminal, so
	/// the direct check already excludes its dependents, and cycles can't hang.
	pub fn ready(&self) -> Result<Vec<String>> {
		self.unblocked_with_status("todo")
	}

	/// The sprint's dispatch set: `in_progress` tickets whose every direct blocker
	/// is terminal — the post-Align sibling of `ready` (same edge semantics, same
	/// priority ordering). A ticket aligned *before* its blocker landed parks here
	/// rather than dispatching: running it would build on a base that is missing
	/// its dependency's work.
	pub fn runnable(&self) -> Result<Vec<String>> {
		self.unblocked_with_status("in_progress")
	}

	fn unblocked_with_status(&self, status: &str) -> Result<Vec<String>> {
		let mut stmt = self.conn.prepare(
			"SELECT t.id FROM ticket t WHERE t.status = ?1
			   AND NOT EXISTS (
			       SELECT 1 FROM edge e JOIN ticket b ON b.id = e.depends_on_id
			        WHERE e.issue_id = t.id AND b.status != 'done'
			          AND e.kind IN ('blocks','parent-child','conditional-blocks','waits-for')
			   )
			 ORDER BY t.priority ASC, t.id ASC",
		)?;
		let ids = stmt
			.query_map(params![status], |r| r.get::<_, String>(0))?
			.collect::<rusqlite::Result<Vec<_>>>()?;
		Ok(ids)
	}

	/// Blocking edges whose target is absent from the board — the conscious
	/// dangling-blocker policy (C1 #2): surfaced as a lint, never silently
	/// resolved into "ready". Read-only; emits no events. Non-blocking edge
	/// kinds are excluded here (they never affect dispatch anyway).
	pub fn dangling_edges(&self) -> Result<Vec<(String, String)>> {
		let mut stmt = self.conn.prepare(
			"SELECT issue_id, depends_on_id, kind FROM edge
			  WHERE depends_on_id NOT IN (SELECT id FROM ticket)",
		)?;
		let rows = stmt
			.query_map([], |r| {
				Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
			})?
			.collect::<rusqlite::Result<Vec<_>>>()?
			.into_iter()
			.filter(|(_, _, kind)| edge_kind_blocks(kind))
			.map(|(issue, dep, _)| (issue, dep))
			.collect();
		Ok(rows)
	}

	// ---- audit -----------------------------------------------------------

	fn append_event(
		&self,
		issue_id: &str,
		kind: &str,
		from: Option<Status>,
		to: Option<Status>,
		provider: Option<&str>,
		note: Option<&str>,
	) -> Result<()> {
		self.conn
			.execute(
				"INSERT INTO event (issue_id, kind, from_status, to_status, provider, note)
				 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
				params![
					issue_id,
					kind,
					from.map(Status::as_str),
					to.map(Status::as_str),
					provider,
					note
				],
			)
			.context("appending event")?;
		Ok(())
	}

	/// Count of audit events for a ticket (used by tests to prove the no-op
	/// short-circuit writes nothing, and that transitions are logged).
	pub fn event_count(&self, issue_id: &str) -> Result<i64> {
		Ok(self
			.conn
			.query_row("SELECT COUNT(*) FROM event WHERE issue_id = ?1", params![issue_id], |r| {
				r.get(0)
			})?)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::spine::{GATE_CRITERIA_CONFIRMED, GATE_LANDED, GATE_MUTATION, GATE_TESTS_GREEN};

	fn mem() -> Board {
		Board::open(":memory:").unwrap()
	}

	/// Clear the human Align gate: todo → align → in_progress.
	fn clear_align(b: &Board, id: &str) {
		b.set_status(id, Status::Align).unwrap();
		b.report_gate(id, GATE_CRITERIA_CONFIRMED, "gary", GateSource::Human, true, None).unwrap();
		b.set_status(id, Status::InProgress).unwrap();
	}

	/// Drive a Todo ticket all the way to Done, clearing every spine gate.
	fn drive_to_done(b: &Board, id: &str) {
		clear_align(b, id);
		b.set_status(id, Status::Verify).unwrap();
		b.report_gate(id, GATE_TESTS_GREEN, "bash", GateSource::Machine, true, None).unwrap();
		b.report_gate(id, GATE_MUTATION, "cargo-mutants", GateSource::Machine, true, None).unwrap();
		b.set_status(id, Status::Review).unwrap();
		b.set_status(id, Status::Land).unwrap();
		b.report_gate(id, GATE_LANDED, "gary", GateSource::Human, true, None).unwrap();
		b.set_status(id, Status::Done).unwrap();
	}

	// append is the accumulating default: twice-append preserves order with a
	// blank-line separator, and `set_notes` (the --replace path) still overwrites.
	#[test]
	fn append_notes_accumulates_with_blank_line_separator() {
		let b = mem();
		b.create_ticket("n1", "build", "pad", 2).unwrap();
		// first append lands on a NULL pad: no leading separator
		b.append_notes("n1", "first note").unwrap();
		assert_eq!(b.get("n1").unwrap().notes.as_deref(), Some("first note"));
		// second append: order preserved, joined by exactly one blank line
		b.append_notes("n1", "second note").unwrap();
		assert_eq!(b.get("n1").unwrap().notes.as_deref(), Some("first note\n\nsecond note"));
		// set_notes remains a full overwrite
		b.set_notes("n1", "clean slate").unwrap();
		assert_eq!(b.get("n1").unwrap().notes.as_deref(), Some("clean slate"));
	}

	// an empty-STRING pad (distinct from NULL) also takes the first append with no
	// leading separator — the CASE arm covers both empties.
	#[test]
	fn append_notes_on_empty_string_has_no_leading_separator() {
		let b = mem();
		b.create_ticket("n2", "build", "pad", 2).unwrap();
		b.set_notes("n2", "").unwrap();
		b.append_notes("n2", "fresh").unwrap();
		assert_eq!(b.get("n2").unwrap().notes.as_deref(), Some("fresh"));
	}

	// append audits like every workpad edit, and an unknown id fails loudly
	// (parity with set_workpad_field — no silent 0-row UPDATE).
	#[test]
	fn append_notes_audits_and_rejects_unknown_ticket() {
		let b = mem();
		b.create_ticket("n3", "build", "pad", 2).unwrap();
		let before = b.event_count("n3").unwrap();
		b.append_notes("n3", "x").unwrap();
		assert_eq!(b.event_count("n3").unwrap(), before + 1, "workpad edit audited");
		assert!(b.append_notes("ghost", "x").is_err());
	}

	// THE DEFECT THIS SLICE EXISTS FOR (both directions). Under the old
	// (issue, gate, provider, attempt) key + ON CONFLICT DO UPDATE, the second
	// report of a gate LANDED ON THE SAME ROW and erased the first — so the re-run
	// that fixed a gate destroyed the evidence of the failure it fixed. Now every
	// report is a row and the LATEST one decides.
	#[test]
	fn a_red_gate_survives_the_green_re_run_that_fixed_it() {
		let b = mem();
		b.create_ticket("h1", "build", "harden twice", 2).unwrap();
		clear_align(&b, "h1");

		// red, then the green re-run of the SAME provider at the SAME attempt
		b.report_gate("h1", GATE_MUTATION, "cargo-mutants", GateSource::Machine, false, Some("score=0.400"))
			.unwrap();
		b.report_gate("h1", GATE_MUTATION, "cargo-mutants", GateSource::Machine, true, Some("score=0.812"))
			.unwrap();

		let reports = b.gate_reports("h1").unwrap();
		let mutation: Vec<_> = reports.iter().filter(|r| r.gate == GATE_MUTATION).collect();
		assert_eq!(mutation.len(), 2, "BOTH reports survive — nothing is overwritten");
		assert!(!mutation[0].passed, "the red is still on the board");
		assert_eq!(mutation[0].note.as_deref(), Some("score=0.400"), "with its evidence");
		assert!(mutation[1].passed, "followed by the green");
		assert!(mutation[0].seq < mutation[1].seq, "in report order");
		assert!(b.gate_satisfied("h1", GATE_MUTATION).unwrap(), "latest report wins: the gate is clear");
		assert_eq!(b.red_gate_count("h1").unwrap(), 1, "and the red is COUNTED, not hidden");

		// the other direction: a green that a later red supersedes does NOT hold
		let b = mem();
		b.create_ticket("h2", "build", "green then red", 2).unwrap();
		clear_align(&b, "h2");
		b.report_gate("h2", GATE_TESTS_GREEN, "bash", GateSource::Machine, true, Some("exit=0")).unwrap();
		assert!(b.gate_satisfied("h2", GATE_TESTS_GREEN).unwrap());
		b.report_gate("h2", GATE_TESTS_GREEN, "bash", GateSource::Machine, false, Some("exit=1")).unwrap();
		assert_eq!(b.gate_reports("h2").unwrap().iter().filter(|r| r.gate == GATE_TESTS_GREEN).count(), 2);
		assert!(
			!b.gate_satisfied("h2", GATE_TESTS_GREEN).unwrap(),
			"the stale pass does not outrank the failure that followed it"
		);
	}

	// The provider hole, closed. `run_harden` reports `mutation_score` under the
	// TOOL name when it measures something and under "none" when the diff has
	// nothing to mutate — different providers, so under the old key they were two
	// coexisting rows and "any passing row" read the vacuous pass as satisfaction
	// with the red sitting right beside it. A red is now cleared only by a re-run
	// of the provider that produced it.
	#[test]
	fn a_vacuous_pass_from_another_provider_does_not_clear_a_red() {
		let b = mem();
		b.create_ticket("p1", "build", "provider hole", 2).unwrap();
		clear_align(&b, "p1");

		b.report_gate("p1", GATE_MUTATION, "cargo-mutants", GateSource::Machine, false, Some("score=0.400"))
			.unwrap();
		b.report_gate("p1", GATE_MUTATION, "none", GateSource::Machine, true, Some("nothing to mutate"))
			.unwrap();
		assert!(
			!b.gate_satisfied("p1", GATE_MUTATION).unwrap(),
			"a pass from a DIFFERENT provider cannot clear another provider's failure"
		);
		// and the honest way out: re-run the provider that failed
		b.report_gate("p1", GATE_MUTATION, "cargo-mutants", GateSource::Machine, true, Some("score=0.900"))
			.unwrap();
		assert!(b.gate_satisfied("p1", GATE_MUTATION).unwrap(), "the failing provider went green");
		assert_eq!(b.gate_reports("p1").unwrap().iter().filter(|r| r.gate == GATE_MUTATION).count(), 3);
	}

	// created_at now means what it says. Under the upsert the row kept the FIRST
	// report's timestamp beside the LAST report's verdict (observed live: a gate
	// stamped 08-06 16:26 whose surviving verdict came from an 08-07 15:45 re-run).
	// Backdating the first row makes the difference observable despite
	// CURRENT_TIMESTAMP's one-second resolution.
	#[test]
	fn each_report_carries_its_own_timestamp() {
		let b = mem();
		b.create_ticket("ts", "build", "stamps", 2).unwrap();
		clear_align(&b, "ts");
		b.report_gate("ts", GATE_TESTS_GREEN, "bash", GateSource::Machine, false, Some("exit=1")).unwrap();
		b.conn
			.execute(
				"UPDATE gate_results SET created_at = '2000-01-01 00:00:00' WHERE gate = ?1",
				params![GATE_TESTS_GREEN],
			)
			.unwrap();

		let now: String =
			b.conn.query_row("SELECT CURRENT_TIMESTAMP", [], |r| r.get(0)).unwrap();
		b.report_gate("ts", GATE_TESTS_GREEN, "bash", GateSource::Machine, true, Some("exit=0")).unwrap();

		let rows: Vec<_> = b
			.gate_reports("ts")
			.unwrap()
			.into_iter()
			.filter(|r| r.gate == GATE_TESTS_GREEN)
			.collect();
		assert_eq!(rows.len(), 2);
		assert_eq!(rows[0].created_at, "2000-01-01 00:00:00", "the first report keeps its stamp");
		assert!(
			rows[1].created_at >= now,
			"the second report carries the SECOND report's time ({} < {now})",
			rows[1].created_at,
		);
	}

	// red_gate_count is the number the one-liners carry: reds at the CURRENT
	// attempt only — a rework epoch already announces itself through `attempt`.
	#[test]
	fn red_gate_count_is_scoped_to_the_current_attempt() {
		let b = mem();
		b.create_ticket("rc", "build", "counts", 2).unwrap();
		clear_align(&b, "rc");
		assert_eq!(b.red_gate_count("rc").unwrap(), 0, "a clean ticket carries no red");

		b.report_gate("rc", GATE_TESTS_GREEN, "bash", GateSource::Machine, false, None).unwrap();
		b.report_gate("rc", GATE_MUTATION, "cargo-mutants", GateSource::Machine, false, None).unwrap();
		assert_eq!(b.red_gate_count("rc").unwrap(), 2);

		b.set_status("rc", Status::Rework).unwrap(); // attempt 0 -> 1
		assert_eq!(b.red_gate_count("rc").unwrap(), 0, "a new epoch starts clean");
		assert_eq!(b.gate_reports("rc").unwrap().len(), 3, "but the history is still all there");
	}

	// the `agent board` listing: every ticket, ordered spine-position → priority
	// → id, regardless of insertion order.
	#[test]
	fn all_tickets_orders_by_spine_then_priority_then_id() {
		let b = mem();
		// seeded deliberately out of the expected order
		b.create_ticket("f", "build", "done, lower priority", 2).unwrap();
		b.create_ticket("b", "build", "todo tie on priority", 2).unwrap();
		b.create_ticket("e", "build", "in progress", 3).unwrap();
		b.create_ticket("a", "build", "todo tie on priority", 2).unwrap();
		b.create_ticket("d", "build", "done, higher priority", 1).unwrap();
		b.create_ticket("c", "build", "todo, highest priority", 1).unwrap();
		drive_to_done(&b, "f");
		drive_to_done(&b, "d");
		clear_align(&b, "e");

		let ids: Vec<String> = b.all_tickets().unwrap().into_iter().map(|t| t.id).collect();
		assert_eq!(
			ids,
			vec!["c", "a", "b", "e", "d", "f"],
			"todo by priority then id, then in_progress, then done by priority"
		);
	}
}
