//! `Board` — the rusqlite handle and the operations that enforce the spine.
//! `set_status` is the chokepoint: it validates the transition, checks gates at
//! the ticket's *current* attempt, bumps the attempt on entry to `Rework`, and
//! appends an audit event. Nothing mutates status except through here.

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};

use crate::model::{GateSource, Run, Status, Ticket, edge_kind_blocks};
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
				 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
				 ON CONFLICT(issue_id, gate, provider, attempt)
				 DO UPDATE SET source = excluded.source, passed = excluded.passed, note = excluded.note",
				params![id, gate, provider, source.as_str(), attempt, passed as i64, note],
			)
			.context("recording gate verdict")?;
		self.append_event(id, "gate_reported", None, None, Some(provider), Some(gate))?;
		Ok(())
	}

	/// Is `gate` satisfied for `id` at its current attempt? A satisfying row is
	/// `passed = 1`, at this attempt, with the source class the gate demands.
	/// Public + read-only so a caller can *require a prerequisite gate* (e.g.
	/// `agent verify` refusing a code ticket that hasn't been hardened) without
	/// attempting a transition and parsing the error string — gate by an
	/// artifact, not by a caught error.
	pub fn gate_satisfied(&self, id: &str, gate: &str) -> Result<bool> {
		let attempt = self.get(id)?.attempt;
		let need = spine::gate_source_required(gate);
		let ok = self
			.conn
			.query_row(
				"SELECT 1 FROM gate_results
				  WHERE issue_id = ?1 AND gate = ?2 AND attempt = ?3
				        AND passed = 1 AND source = ?4 LIMIT 1",
				params![id, gate, attempt, need.as_str()],
				|_| Ok(true),
			)
			.optional()?
			.unwrap_or(false);
		Ok(ok)
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
