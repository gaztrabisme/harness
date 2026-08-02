//! Delegated Claude worker — the **pure core** (active-work "Next slice"; decisions.md
//! "Strategic posture" + "Claude integration has TWO shapes"). The subscription can't
//! back single-turn completions, so Claude enters the harness as a WHOLE-TASK worker:
//! `claude -p <task> --output-format stream-json` spawned inside the ticket's git
//! worktree, running its own agentic loop to completion. Everything the harness owns
//! stays outside and unchanged — Align before, verify/harden/review/land after — so
//! worker quality becomes a swappable dial while integrity stays structural.
//!
//! What does NOT apply to this worker (deliberate, documented): the native loop's
//! diagnostic spine (refeed/compaction/loopgate/planexec — CC manages its own
//! context), the per-tool gate (CC's tools can't be intercepted; the worker only ever
//! spawns post-Align and the land gate stays human), and the `confine()` jail (the
//! worktree boundary is prompt-level here — accepted at align 2026-07-14, revisit on
//! an observed out-of-tree write). Permission posture: `--dangerously-skip-permissions`
//! (Gary's align ruling — same trust as interactive CC).
//!
//! This module holds the pure parts — the task-prompt builder, the lenient result-JSON
//! parser, and the outcome mapping — no process, no board, no git, so the logic is
//! unit-tested in isolation (the `review.rs`/`researcher.rs` pattern). The spawn +
//! telemetry glue (`run_claude_ticket`) lives in `main.rs`.

/// Cap on the worker's internal turns (`--max-turns`). CC's own loop is competent but
/// unbounded delegation is not — this is the delegated analogue of the native loop's
/// `max_iters`. Generous: a real ticket needs room to read, edit, and run validation.
pub const WORKER_MAX_TURNS: u32 = 40;

/// Wall-clock backstop in seconds (the delegated analogue of the researcher's
/// timeout). A hung/looping worker is killed and the run labelled `timeout` — bounded
/// and loud, never a hang. 15 min default; override via `HARNESS_CLAUDE_TIMEOUT`.
pub const WORKER_TIMEOUT_SECS: u64 = 900;

/// The final `type:"result"` event of a `claude -p` stream, reduced to what the
/// harness records. Parsed leniently (CLI schema may drift between versions): only
/// `is_error` and `result` are load-bearing for the outcome; the rest is telemetry.
/// Shape verified live against CLI 2.1.208 (2026-07-14 probe).
#[derive(Debug, Clone, PartialEq)]
pub struct WorkerResult {
	pub is_error: bool,
	pub result: String,
	pub num_turns: Option<i64>,
	pub total_cost_usd: Option<f64>,
	pub session_id: Option<String>,
}

/// Parse one stream-json line iff it is the `type:"result"` event. Any other event
/// type, or a non-JSON line, is `None` (the stream interleaves system/assistant/tool
/// events; only the terminal result summarises the run).
pub fn parse_result_line(line: &str) -> Option<WorkerResult> {
	let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
	if v.get("type").and_then(serde_json::Value::as_str) != Some("result") {
		return None;
	}
	Some(WorkerResult {
		// a result event missing `is_error` is treated as an error: the field is the
		// success signal, and "absent" must not default to "succeeded".
		is_error: v.get("is_error").and_then(serde_json::Value::as_bool).unwrap_or(true),
		result: v.get("result").and_then(serde_json::Value::as_str).unwrap_or("").to_owned(),
		num_turns: v.get("num_turns").and_then(serde_json::Value::as_i64),
		total_cost_usd: v.get("total_cost_usd").and_then(serde_json::Value::as_f64),
		session_id: v.get("session_id").and_then(serde_json::Value::as_str).map(str::to_owned),
	})
}

/// Find the terminal result event in a captured stream. Scanned from the END — the
/// result is the last event by contract, and a rogue `type:"result"` string embedded
/// earlier (e.g. quoted inside an assistant message) must not shadow the real one.
pub fn find_result(stream: &str) -> Option<WorkerResult> {
	stream.lines().rev().find_map(parse_result_line)
}

/// Map a finished (or killed) worker to a run-outcome label. Precedence: a timeout
/// wins (the process was killed — its output is untrustworthy), then a non-zero exit,
/// then a missing/unparseable result event, then the event's own `is_error`. Labels
/// share the open vocabulary of the native loop's rows (`completed` is the same
/// signal verify/land key off; `timeout`/`error` are new but the column is free-form
/// by design — the `stalled` precedent).
pub fn outcome(exit_ok: bool, timed_out: bool, res: Option<&WorkerResult>) -> &'static str {
	if timed_out {
		return "timeout";
	}
	if !exit_ok {
		return "error";
	}
	match res {
		None => "error", // exited zero but never emitted a result event — not evidence of success
		Some(r) if r.is_error => "error",
		Some(_) => "completed",
	}
}

/// Build the delegated task prompt: harness framing + the rendered workpad (the same
/// single-source-of-truth `board::render` the native loop and `agent show` use) +
/// past lessons (the compounding channel, same `recall_primed` feed as `run_ticket`).
/// The rules encode the contract the harness can't enforce inside CC: stay in the
/// worktree, don't touch git (the harness owns commit/land), don't fake success.
pub fn build_task_prompt(workpad: &str, lessons: &[(String, String)]) -> String {
	let mut p = String::from(
		"You are a delegated worker for a gated coding harness, executing ONE ticket \
		 inside a dedicated git worktree (your current directory).\n\n",
	);
	p.push_str(workpad);
	if !lessons.is_empty() {
		p.push_str("\n\n### Relevant past experience\n");
		for (title, body) in lessons {
			p.push_str(&format!("- {title}: {body}\n"));
		}
	}
	p.push_str(
		"\n\nRules:\n\
		 - Work ONLY inside the current directory. Never read or modify files outside it.\n\
		 - Follow the Plan; satisfy every Acceptance Criterion.\n\
		 - Run the Validation command yourself before finishing; if it fails, keep fixing \
		 until it passes or you are genuinely blocked.\n\
		 - Do NOT commit, branch, push, or otherwise touch git — the harness owns git. \
		 Leave your changes uncommitted in the working tree.\n\
		 - If you are genuinely blocked, say so plainly in your final message. Honest \
		 failure beats fabricated success — the harness verifies everything downstream.",
	);
	p
}

#[cfg(test)]
mod tests {
	use super::*;

	const RESULT_OK: &str = r#"{"type":"result","subtype":"success","is_error":false,"num_turns":7,"result":"done","session_id":"abc","total_cost_usd":0.42}"#;
	const RESULT_ERR: &str = r#"{"type":"result","subtype":"error_during_execution","is_error":true,"num_turns":3,"result":"blocked"}"#;

	#[test]
	fn parse_result_ok() {
		let r = parse_result_line(RESULT_OK).unwrap();
		assert!(!r.is_error);
		assert_eq!(r.result, "done");
		assert_eq!(r.num_turns, Some(7));
		assert_eq!(r.total_cost_usd, Some(0.42));
		assert_eq!(r.session_id.as_deref(), Some("abc"));
	}

	#[test]
	fn parse_non_result_events_are_none() {
		assert!(parse_result_line(r#"{"type":"assistant","message":{}}"#).is_none());
		assert!(parse_result_line(r#"{"type":"system","subtype":"init"}"#).is_none());
		assert!(parse_result_line("not json").is_none());
		assert!(parse_result_line("").is_none());
	}

	#[test]
	fn parse_missing_is_error_defaults_to_error() {
		// absent success signal must NOT read as success
		let r = parse_result_line(r#"{"type":"result","result":"???"}"#).unwrap();
		assert!(r.is_error);
	}

	#[test]
	fn find_result_takes_the_last_event_and_skips_embedded_fakes() {
		// an assistant message QUOTING a result-shaped string must not shadow the real
		// terminal event; scanning from the end also picks the later of two results.
		let stream = format!(
			"{}\n{}\n{}\n",
			r#"{"type":"assistant","message":"look: {\"type\":\"result\",\"is_error\":false}"}"#,
			RESULT_ERR,
			RESULT_OK
		);
		let r = find_result(&stream).unwrap();
		assert!(!r.is_error, "the LAST result event wins");
		assert!(find_result("no events here\n").is_none());
	}

	#[test]
	fn outcome_matrix() {
		let ok = parse_result_line(RESULT_OK).unwrap();
		let err = parse_result_line(RESULT_ERR).unwrap();
		assert_eq!(outcome(true, false, Some(&ok)), "completed");
		assert_eq!(outcome(true, false, Some(&err)), "error", "is_error wins over clean exit");
		assert_eq!(outcome(true, false, None), "error", "no result event is not success");
		assert_eq!(outcome(false, false, Some(&ok)), "error", "non-zero exit wins over parsed ok");
		assert_eq!(outcome(true, true, Some(&ok)), "timeout", "timeout wins over everything");
	}

	#[test]
	fn task_prompt_carries_workpad_rules_and_lessons() {
		let lessons = vec![("past miss".to_string(), "validate before finishing".to_string())];
		let p = build_task_prompt("## Workpad — t1\n### Plan\n1. x", &lessons);
		assert!(p.contains("## Workpad — t1"));
		assert!(p.contains("Never read or modify files outside"));
		assert!(p.contains("Do NOT commit"));
		assert!(p.contains("past experience"));
		assert!(p.contains("past miss"));
		let bare = build_task_prompt("wp", &[]);
		assert!(!bare.contains("past experience"), "no lessons → no section");
	}
}
