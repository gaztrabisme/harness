//! Loop gate (research/22) — deterministic detection of a non-progress tool-call
//! cycle, the textbook "agent stuck in a loop."
//!
//! The signal is model-agnostic and universal: an assistant turn whose tool calls
//! are **byte-identical** to the immediately preceding turn's (same names, same
//! arguments, same order) made no progress — identical arguments produce the same
//! deterministic effect, and (workers run sequentially in isolated worktrees,
//! research/19 §9.4) nothing external changed in between. That is the lowest
//! false-positive loop signal available; we key off it rather than any provider
//! quirk (think tags, `reasoning_content`) so the gate behaves identically across
//! providers. `max_iters` remains the crude backstop for exotic cycles this
//! consecutive-only detector deliberately does not chase (e.g. A,B,A,B).
//!
//! Two pure pieces, unit-tested in isolation (the `refeed.rs`/`explore.rs` style):
//!
//! - [`signature`] fingerprints a turn's tool calls into a comparable string.
//! - [`LoopGate`] is the per-run state machine: feed it each turn's signature and
//!   it returns whether to proceed, nudge the model once, or stop the run. Strikes
//!   are **run-level** (not reset when the signature changes), so a model that
//!   loops, is nudged, recovers, then loops on a *different* call is still stopped
//!   on that second cycle's first repeat — "2 strikes → stop," robust to cycle
//!   switching without needing a window.

use provider::ToolCall;

/// Stop the run once this many repeat strikes have accrued. The first strike
/// nudges (one recovery turn); the second stops.
const STOP_AT_STRIKES: u32 = 2;

/// Fingerprint a turn's tool calls: each call's `name` and raw `arguments` in
/// order, joined by control bytes that cannot appear in a tool name and are
/// vanishingly unlikely to straddle a JSON-argument boundary ambiguously. A turn
/// with no tool calls fingerprints to `""` — but the loop only consults the gate
/// on tool-call turns, so that case never drives a verdict.
pub fn signature(calls: &[ToolCall]) -> String {
	let mut s = String::new();
	for c in calls {
		s.push_str(&c.name);
		s.push('\u{0}'); // name/args separator
		s.push_str(&c.arguments);
		s.push('\u{1}'); // end-of-call separator
	}
	s
}

/// What the loop should do after observing a turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
	/// No repeat — dispatch tools and continue normally.
	Proceed,
	/// First repeat — dispatch tools, then inject a course-correction nudge so the
	/// next turn sees it. The model gets exactly one chance to change approach.
	Nudge,
	/// Repeat after the nudge (or a second distinct cycle) — stop the run. The loop
	/// breaks *before* re-executing the repeated call.
	Stop,
}

/// Per-run loop detector. Construct one per `run_ticket` invocation.
#[derive(Debug, Default)]
pub struct LoopGate {
	prev: Option<String>,
	strikes: u32,
}

impl LoopGate {
	pub fn new() -> Self {
		Self::default()
	}

	/// Observe this turn's tool-call signature and decide. A repeat (signature
	/// identical to the immediately preceding turn) is a strike; strike 1 nudges,
	/// strike 2 stops. Strikes accumulate across the whole run.
	pub fn observe(&mut self, sig: &str) -> Verdict {
		let repeat = self.prev.as_deref() == Some(sig);
		self.prev = Some(sig.to_string());
		if !repeat {
			return Verdict::Proceed;
		}
		self.strikes += 1;
		if self.strikes >= STOP_AT_STRIKES { Verdict::Stop } else { Verdict::Nudge }
	}

	/// How many repeat strikes have accrued (for the log line / telemetry).
	pub fn strikes(&self) -> u32 {
		self.strikes
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn call(name: &str, args: &str) -> ToolCall {
		ToolCall { id: "x".into(), name: name.into(), arguments: args.into() }
	}

	#[test]
	fn signature_is_stable_and_discriminating() {
		let a = vec![call("read_file", r#"{"path":"a"}"#)];
		let b = vec![call("read_file", r#"{"path":"a"}"#)];
		let c = vec![call("read_file", r#"{"path":"b"}"#)];
		assert_eq!(signature(&a), signature(&b), "same name+args → same signature");
		assert_ne!(signature(&a), signature(&c), "different args → different signature");
		assert_eq!(signature(&[]), "", "no calls → empty signature");
	}

	#[test]
	fn signature_is_order_sensitive() {
		let ab = vec![call("read_file", "{}"), call("bash", "{}")];
		let ba = vec![call("bash", "{}"), call("read_file", "{}")];
		assert_ne!(signature(&ab), signature(&ba), "call order is part of the turn");
	}

	#[test]
	fn signature_cannot_collide_across_call_boundaries() {
		// "ab"+"c" must not fingerprint the same as "a"+"bc" — the separators prevent it.
		let one = vec![call("ab", "c")];
		let two = vec![call("a", "bc")];
		assert_ne!(signature(&one), signature(&two));
	}

	#[test]
	fn no_repeat_always_proceeds() {
		let mut g = LoopGate::new();
		assert_eq!(g.observe("A"), Verdict::Proceed);
		assert_eq!(g.observe("B"), Verdict::Proceed);
		assert_eq!(g.observe("C"), Verdict::Proceed);
		assert_eq!(g.strikes(), 0);
	}

	#[test]
	fn identical_repeat_nudges_then_stops() {
		let mut g = LoopGate::new();
		assert_eq!(g.observe("A"), Verdict::Proceed, "first sight of A");
		assert_eq!(g.observe("A"), Verdict::Nudge, "first repeat → nudge");
		assert_eq!(g.observe("A"), Verdict::Stop, "second repeat → stop");
		assert_eq!(g.strikes(), 2);
	}

	#[test]
	fn alternating_is_not_flagged() {
		// Consecutive-only by design: A,B,A,B never repeats *consecutively*, so it
		// proceeds (max_iters is the backstop for this exotic case).
		let mut g = LoopGate::new();
		assert_eq!(g.observe("A"), Verdict::Proceed);
		assert_eq!(g.observe("B"), Verdict::Proceed);
		assert_eq!(g.observe("A"), Verdict::Proceed);
		assert_eq!(g.observe("B"), Verdict::Proceed);
		assert_eq!(g.strikes(), 0);
	}

	#[test]
	fn switching_cycles_still_stops_on_run_level_strikes() {
		// Loop on A (nudged), switch and loop on B — stopped on B's first repeat
		// because strikes are run-level, not reset when the signature changes.
		let mut g = LoopGate::new();
		assert_eq!(g.observe("A"), Verdict::Proceed);
		assert_eq!(g.observe("A"), Verdict::Nudge, "strike 1 on A");
		assert_eq!(g.observe("B"), Verdict::Proceed, "new signature resets the repeat, not the strikes");
		assert_eq!(g.observe("B"), Verdict::Stop, "strike 2 on B → stop");
		assert_eq!(g.strikes(), 2);
	}

	#[test]
	fn recovery_after_nudge_does_not_stop() {
		// Nudged on A, then genuinely makes progress (B, C, D distinct) → never stops.
		let mut g = LoopGate::new();
		assert_eq!(g.observe("A"), Verdict::Proceed);
		assert_eq!(g.observe("A"), Verdict::Nudge);
		assert_eq!(g.observe("B"), Verdict::Proceed);
		assert_eq!(g.observe("C"), Verdict::Proceed);
		assert_eq!(g.observe("D"), Verdict::Proceed);
		assert_eq!(g.strikes(), 1, "one strike, never escalated");
	}
}
