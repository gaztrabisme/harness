//! Run trajectory recorder (research/17 §2a/§4) — appends one redacted JSON line
//! per provider `Message` at the loop boundary. The board `run` row is the
//! authoritative record (started/ended/stop_reason); this file is the replayable
//! detail that an audit or a finetune/RL pipeline consumes.
//!
//! Best-effort by design: telemetry must never block or fail the agent's work.
//! An open failure yields a disabled recorder; a later write failure disables it.
//! Either way the loop runs on. The reader is correspondingly lenient: a crash
//! mid-append leaves a torn final line, which is tolerated, not an error.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

use anyhow::{Result, anyhow};
use provider::{Message, Role};
use serde_json::{Value, json};

/// Cap any single recorded text field (bytes) so one giant tool dump can't bloat
/// the trajectory. Capping happens after redaction.
const FIELD_CAP: usize = 8192;

/// Appends redacted JSON-line records for a single run. Construct with `open`,
/// feed it `Message`s as the loop produces them. The oMLX local API key — a
/// local-only credential that must never land in a trajectory on disk
/// (research/17 §8 M8) — is resolved once at open and redacted at the write
/// boundary, whatever its per-machine value is.
pub struct Recorder {
	file: Option<File>,
	step: u64,
	secret: String,
}

impl Recorder {
	/// Open (append) the trajectory at `path`, creating parent dirs. Non-fatal:
	/// on any IO error this logs once to stderr and returns a *disabled* recorder
	/// (every `record` becomes a no-op that still advances the step counter).
	pub fn open(path: &Path) -> Self {
		let secret = provider::omlx_key();
		match Self::try_open(path) {
			Ok(f) => Self { file: Some(f), step: 0, secret },
			Err(e) => {
				eprintln!("[rec  ] telemetry disabled ({path:?}): {e:#}");
				Self { file: None, step: 0, secret }
			}
		}
	}

	fn try_open(path: &Path) -> Result<File> {
		if let Some(parent) = path.parent() {
			std::fs::create_dir_all(parent)?;
		}
		Ok(OpenOptions::new().create(true).append(true).open(path)?)
	}

	/// Record each message in order (used for the seed system+user turns).
	pub fn record_all(&mut self, messages: &[Message]) {
		for m in messages {
			self.record(m);
		}
	}

	/// Append one redacted JSON line for `m`, flushed so a crash preserves every
	/// complete write. Best-effort: a write failure disables the recorder but is
	/// never propagated — the run continues.
	pub fn record(&mut self, m: &Message) {
		let rec = to_record(self.step, m, &self.secret);
		self.step += 1;
		let Some(f) = self.file.as_mut() else {
			return;
		};
		if writeln!(f, "{rec}").and_then(|()| f.flush()).is_err() {
			self.file = None; // stop trying; the work is what matters
		}
	}

	/// Append the model's server-separated chain-of-thought (`reasoning_content`)
	/// as a `reasoning`-role line. This is captured for audit/finetune provenance
	/// ONLY — the loop never re-feeds it (it is the bulky deliberation; re-feeding
	/// it bloats the window — the research/21 finding). Redacted and capped like any
	/// other field; advances the step counter so the trajectory stays sequential.
	pub fn record_reasoning(&mut self, reasoning: &str) {
		let rec =
			json!({ "step": self.step, "role": "reasoning", "content": clean(reasoning, &self.secret) });
		self.step += 1;
		let Some(f) = self.file.as_mut() else {
			return;
		};
		if writeln!(f, "{rec}").and_then(|()| f.flush()).is_err() {
			self.file = None;
		}
	}
}

/// Build the §2a record for a message: `step` + `role` + redacted/capped
/// `content`, plus `tool_calls` / `tool_call_id` when present. Built via `json!`
/// because the provider types intentionally don't derive `Serialize`.
fn to_record(step: u64, m: &Message, secret: &str) -> Value {
	let mut obj = json!({
		"step": step,
		"role": role_str(m.role),
		"content": clean(&m.content, secret),
	});
	if !m.tool_calls.is_empty() {
		obj["tool_calls"] = Value::Array(
			m.tool_calls
				.iter()
				.map(|c| json!({ "id": c.id, "name": c.name, "arguments": clean(&c.arguments, secret) }))
				.collect(),
		);
	}
	if let Some(id) = &m.tool_call_id {
		obj["tool_call_id"] = json!(id);
	}
	obj
}

fn role_str(r: Role) -> &'static str {
	match r {
		Role::System => "system",
		Role::User => "user",
		Role::Assistant => "assistant",
		Role::Tool => "tool",
	}
}

/// Redact the local credential, then cap length at a UTF-8 char boundary so the
/// record stays valid and bounded (§8 M8 + bloat guard). An empty secret means
/// "nothing to redact" — `str::replace("")` would garble every record.
fn clean(s: &str, secret: &str) -> String {
	let red =
		if secret.is_empty() { s.to_string() } else { s.replace(secret, "[REDACTED]") };
	if red.len() <= FIELD_CAP {
		return red;
	}
	let mut end = FIELD_CAP;
	while end > 0 && !red.is_char_boundary(end) {
		end -= 1;
	}
	let mut t = red[..end].to_string();
	t.push_str("…[truncated]");
	t
}

/// Read a trajectory back as JSON records. Lenient by contract: a torn FINAL line
/// (a crash mid-append) is dropped, not an error; an unparseable INTERIOR line is
/// real corruption and errors.
pub fn read_trajectory(path: &Path) -> Result<Vec<Value>> {
	let body = std::fs::read_to_string(path)?;
	let lines: Vec<&str> = body.lines().collect();
	let mut out = Vec::with_capacity(lines.len());
	for (i, line) in lines.iter().enumerate() {
		if line.trim().is_empty() {
			continue;
		}
		match serde_json::from_str::<Value>(line) {
			Ok(v) => out.push(v),
			Err(e) => {
				if i == lines.len() - 1 {
					break; // torn final line — tolerate (the writer may have crashed)
				}
				return Err(anyhow!("trajectory {path:?}: corrupt interior line {i}: {e}"));
			}
		}
	}
	Ok(out)
}

#[cfg(test)]
mod tests {
	use super::*;
	use provider::ToolCall;

	fn tmp(name: &str) -> std::path::PathBuf {
		let p = std::env::temp_dir().join(name);
		let _ = std::fs::remove_file(&p);
		p
	}

	// One JSON record per message, in order, with the right shape — then it reads
	// back through the lenient reader unchanged.
	#[test]
	fn records_each_message_and_round_trips() {
		let path = tmp("rec-roundtrip.jsonl");
		{
			let mut r = Recorder::open(&path);
			r.record_all(&[Message::system("sys"), Message::user("hi")]);
			r.record(&Message::assistant(
				"calling",
				vec![ToolCall { id: "c1".into(), name: "read_file".into(), arguments: "{}".into() }],
			));
			r.record(&Message::tool_result("c1", "file body"));
		}
		let recs = read_trajectory(&path).unwrap();
		assert_eq!(recs.len(), 4, "one record per message");
		assert_eq!(recs[0]["step"], 0);
		assert_eq!(recs[0]["role"], "system");
		assert_eq!(recs[1]["role"], "user");
		assert_eq!(recs[2]["role"], "assistant");
		assert_eq!(recs[2]["tool_calls"][0]["name"], "read_file");
		assert_eq!(recs[3]["role"], "tool");
		assert_eq!(recs[3]["tool_call_id"], "c1");
		assert_eq!(recs[3]["step"], 3, "step counts every message");
		let _ = std::fs::remove_file(&path);
	}

	// The oMLX key never reaches disk (§8 M8) — neither in message content nor in
	// tool-call arguments. The secret is injected so the test is deterministic on
	// machines with no configured key.
	#[test]
	fn redacts_the_omlx_key() {
		const KEY: &str = "sekret-injected-test-key";
		let path = tmp("rec-redact.jsonl");
		{
			let mut r = Recorder::open(&path);
			r.secret = KEY.into();
			r.record(&Message::user(format!("token is {KEY} ok")));
			r.record(&Message::assistant(
				"",
				vec![ToolCall {
					id: "c".into(),
					name: "bash".into(),
					arguments: format!("{{\"k\":\"{KEY}\"}}"),
				}],
			));
		}
		let raw = std::fs::read_to_string(&path).unwrap();
		assert!(!raw.contains(KEY), "raw key must not appear anywhere on disk");
		assert!(raw.contains("[REDACTED]"), "redaction marker present");
		let _ = std::fs::remove_file(&path);
	}

	// An empty secret (unconfigured machine) must be a no-op, not a garbler —
	// `str::replace("")` inserts the marker between every character.
	#[test]
	fn empty_secret_leaves_text_untouched() {
		assert_eq!(clean("plain text", ""), "plain text");
	}

	// A torn final line (crash mid-append) is dropped; a corrupt interior line is
	// a hard error.
	#[test]
	fn reader_tolerates_torn_tail_but_not_interior_corruption() {
		let torn = tmp("rec-torn.jsonl");
		std::fs::write(&torn, "{\"step\":0,\"role\":\"user\"}\n{\"step\":1,\"rol").unwrap();
		let recs = read_trajectory(&torn).unwrap();
		assert_eq!(recs.len(), 1, "torn final line dropped, the good one kept");
		let _ = std::fs::remove_file(&torn);

		let bad = tmp("rec-interior.jsonl");
		std::fs::write(&bad, "{not json\n{\"step\":1}\n").unwrap();
		assert!(read_trajectory(&bad).is_err(), "interior corruption is an error");
		let _ = std::fs::remove_file(&bad);
	}

	// A disabled recorder (open failed) never panics and still advances steps.
	#[test]
	fn disabled_recorder_is_a_noop() {
		// parent is a file, so create_dir_all fails → disabled recorder
		let blocker = tmp("rec-blocker");
		std::fs::write(&blocker, "x").unwrap();
		let mut r = Recorder::open(&blocker.join("nested.jsonl"));
		r.record(&Message::user("a"));
		r.record(&Message::user("b"));
		assert_eq!(r.step, 2, "steps advance even when disabled");
		let _ = std::fs::remove_file(&blocker);
	}
}
