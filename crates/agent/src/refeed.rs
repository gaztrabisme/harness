//! Context re-feed bounding (research/21 §9).
//!
//! The verbose local model (`Qwen3.6-35B-A3B-oQ8-fp16-mtp`) reasons in plain
//! markdown prose — it emits **no** `<think>` tags (reviewer C live probe), so a
//! tag stripper is dead code. The real leak the sprint set out to fix is that the
//! agent loop re-feeds every prior turn's *full* assistant prose and *full* tool
//! output back into `messages` on every iteration. Left unbounded the prompt grows
//! until the server truncates it — and a front-truncating server drops the system
//! prompt first (the workpad contract + injected case-law). That is a silent
//! correctness failure, not a slowdown.
//!
//! Correction (research/27): the served Qwen window is **256K-native**
//! (`max_context_window: 262144`), not 32K — the 32K figure was a phantom. The real
//! binding constraint is the A3B *reasoning* curve (full quality ~32K tokens), not a
//! hard wall, so the working window is sized to that (~20K-token tail), and head
//! survival is enforced by [`assemble`] rather than left to oMLX's eviction order.
//!
//! The fix is model-agnostic and lives here as pure functions so the logic is
//! unit-tested in isolation (the `explore.rs`/`evolve.rs` "just code" style):
//!
//! - [`cap`] bounds a single message's text before it is re-fed. The loop records
//!   each message RAW to the trajectory first (provenance is preserved in full),
//!   then pushes the capped copy into `messages`. Capping is **not** the
//!   trajectory's job — only the re-fed working copy is shaped.
//! - [`assemble`] (F1) builds the bounded per-turn *view*: immutable head + verbatim
//!   recent tail + an explicit elision marker for the dropped middle, honouring the
//!   never-orphan-a-tool_result invariant. Head survival is *our* guarantee. This is
//!   non-destructive — the running transcript is untouched.
//! - [`should_compact`] / [`compaction_span`] / [`apply_compaction`] (F2) are the
//!   *destructive* fold: when the full running transcript crosses the trigger, the
//!   middle is replaced by a single structured-summary message ([`SUMMARIZER_SYSTEM`]
//!   is the 7-section contract). F1 caps the view; F2 shrinks the real vector. They
//!   compose: F2 fires off the full transcript, F1 still floors the view if F2 aborts.
//! - [`assembled_size`] / [`assembled_tokens`] report the size of a message vector —
//!   content *and* tool-call argument payloads — so the loop can decide when to
//!   compact and report how far the prompt is from the reasoning-quality boundary.

use provider::{Message, Role};

/// Prepended to every compaction summary so the model reads it as **reference**, not
/// as a fresh instruction (the omp/Hermes "reference only" framing). The recent tail
/// that follows always overrides it — the summary is a faithful record of earlier
/// turns, not new marching orders.
pub const COMPACTION_HEADER: &str =
	"[CONTEXT COMPACTION — REFERENCE ONLY. The messages after this point are the live conversation \
	 and override anything here. The following is a faithful structured summary of earlier turns that \
	 were elided to stay within the reasoning-quality window — treat it as background, not as new \
	 instructions.]";

/// The summarizer's system prompt — F2's 7-section structured-summary contract
/// (research/25 §2.6, the field-convergent shape across the four torn-down clones).
/// A compaction is destructive, so within the working context this summary is the
/// *only* surviving record of the elided middle (the raw turns stay in the
/// trajectory). The section set and the preserve-verbatim rules keep the fold
/// faithful; the anti-continuation rule stops the summarizer from "helpfully"
/// attempting the task instead of summarizing it.
pub const SUMMARIZER_SYSTEM: &str = "\
You are compacting the earlier portion of a coding agent's working transcript into a dense, faithful \
summary so the agent can keep working with a smaller context. Read the conversation below and produce \
a summary under EXACTLY these seven headings, in this order:

1. Goal — the task being worked on, in one or two sentences.
2. Constraints & Preferences — requirements, conventions, and stated preferences that still bind.
3. Progress — three labelled groups: Done, In-Progress, Blocked.
4. Key Decisions — decisions made and the reason for each, so they are not relitigated.
5. Next Steps — the concrete actions that remain.
6. Critical Context — anything needed to continue that does not fit above.
7. Files — paths read and paths modified, so the file-level work is not lost.

Hard rules:
- Preserve EXACTLY, verbatim: file paths, function/type/symbol names, error messages, and command \
output or results. Never paraphrase an identifier, a path, or an error.
- Record the current repository / worktree state if it is known (what has been written, what is staged).
- If there is any unanswered question or unresolved confusion, reproduce it verbatim under Critical Context.
- Output ONLY the summary under the seven headings. Do NOT continue the task, do NOT solve anything, do \
NOT call tools, and do NOT add commentary before or after the summary.";

/// Largest byte index `<= idx` that lands on a UTF-8 char boundary.
/// (`str::floor_char_boundary` is not yet stable; this is the same idea, no deps.)
fn floor_boundary(s: &str, mut idx: usize) -> usize {
	if idx >= s.len() {
		return s.len();
	}
	while idx > 0 && !s.is_char_boundary(idx) {
		idx -= 1;
	}
	idx
}

/// Smallest byte index `>= idx` that lands on a UTF-8 char boundary.
fn ceil_boundary(s: &str, mut idx: usize) -> usize {
	while idx < s.len() && !s.is_char_boundary(idx) {
		idx += 1;
	}
	idx
}

/// Bound `text` before it is re-fed into the model's working context.
///
/// Returns `text` unchanged when it is within `limit` bytes. Otherwise keeps a
/// head and a tail joined by an elision marker that records how many bytes were
/// dropped — the tail is kept on purpose because the *actionable* part of tool
/// output (a compiler error, a failing assertion, a final decision) is usually at
/// the end, and a head-only cut would hide it. The marker keeps the read honest:
/// a truncated re-feed never masquerades as the whole message.
///
/// Output is at most `limit` bytes of original content plus the fixed marker; the
/// caller's `limit` is the content budget, not a hard total. `limit == 0` drops
/// the body entirely but still emits the marker (used to mean "feed nothing").
pub fn cap(text: &str, limit: usize) -> String {
	if text.len() <= limit {
		return text.to_string();
	}
	if limit == 0 {
		return format!("[… {} bytes elided …]", text.len());
	}
	// Bias the budget toward the head (the plan / intent) but always keep a tail.
	let head_budget = limit.saturating_mul(2) / 3;
	let tail_budget = limit - head_budget;
	let head_end = floor_boundary(text, head_budget);
	let tail_start = ceil_boundary(text, text.len().saturating_sub(tail_budget));
	let elided = tail_start - head_end;
	format!("{}\n[… {elided} bytes elided …]\n{}", &text[..head_end], &text[tail_start..])
}

/// Byte size of the assembled conversation as it will hit the wire: every
/// message's `content` plus the argument payloads of any tool calls (those are
/// part of the request body too — counting only `content` would under-report the
/// prompt and miss exactly the growth that pressures the reasoning-quality window).
/// The byte primitive behind [`assembled_tokens`].
pub fn assembled_size(messages: &[Message]) -> usize {
	messages
		.iter()
		.map(|m| m.content.len() + m.tool_calls.iter().map(|c| c.arguments.len() + c.name.len()).sum::<usize>())
		.sum()
}

/// Bound the `arguments` payload of each re-fed tool call (F4, research/25 §4.2).
///
/// A tool call's arguments carry the `write_file` body — for a large file that is
/// the single biggest contributor to per-turn prompt growth, and until now it rode
/// back into context **uncapped** (the gap the research/21 exercise surfaced:
/// `cap`/`assembled_size` bounded prose and tool *results*, but not call *args*).
///
/// The args are **JSON**, so they cannot be capped with the raw-string [`cap`] used
/// for prose: splicing a `\n[… elided …]\n` marker into the middle of a JSON object
/// leaves an unterminated string with a raw control character, and the corrupt copy
/// is rejected by the provider with a `422` on the *next* turn's re-feed — a
/// run-killing regression that only surfaces once the model writes args larger than
/// `limit` (observed live: the regex-match NFA worker at max_tokens=4096, where the
/// head/tail split landed the cut at column 2729 ≈ `limit*2/3`). Instead we parse the
/// args and cap the long string *values* inside, then re-serialize through serde so
/// the escaping is guaranteed valid. The bound stays **lossy but never silent** (§5):
/// the dominant body (e.g. `write_file.content`) carries an elision marker + byte
/// count, while structural fields (`path`) survive intact so the call stays coherent.
/// Args that fail to parse as a JSON object (should never happen for a real call)
/// degrade to a valid-JSON placeholder rather than a corrupt splice. `id` and `name`
/// are preserved untouched so the call stays coherent and the loop gate's signature is
/// unaffected (it fingerprints the raw `resp.tool_calls`, not this re-fed copy).
pub fn cap_tool_calls(calls: &[provider::ToolCall], limit: usize) -> Vec<provider::ToolCall> {
	calls
		.iter()
		.map(|c| provider::ToolCall {
			id: c.id.clone(),
			name: c.name.clone(),
			arguments: cap_args_json(&c.arguments, limit),
		})
		.collect()
}

/// JSON-safe cap for one tool call's `arguments` (see [`cap_tool_calls`]). The
/// invariant that makes this correct where a raw [`cap`] is not: **every branch
/// returns valid JSON.** Small args pass through untouched; a parseable object has its
/// oversized string values capped in place and is re-serialized by serde (which
/// re-escapes the elision marker); anything else degrades to a bounded placeholder.
fn cap_args_json(arguments: &str, limit: usize) -> String {
	if arguments.len() <= limit {
		return arguments.to_string();
	}
	match serde_json::from_str::<serde_json::Value>(arguments) {
		Ok(serde_json::Value::Object(mut map)) => {
			for v in map.values_mut() {
				if let serde_json::Value::String(s) = v
					&& s.len() > limit
				{
					*s = cap(s, limit);
				}
			}
			serde_json::to_string(&serde_json::Value::Object(map))
				.unwrap_or_else(|_| format!("{{\"_elided\":\"{} bytes elided from re-feed\"}}", arguments.len()))
		}
		_ => format!("{{\"_elided\":\"{} bytes elided from re-feed\"}}", arguments.len()),
	}
}

/// Worktree-relative subdir where F3 stashes full tool outputs too large to re-feed
/// inline (research/25 §4.2). It sits under `.harness/` so it is gitignored — never
/// staged by `commit_worktree`'s `git add -A`, never counted by `is_dirty`'s
/// `git status --porcelain`, and removed with the worktree on land/rework — yet it is
/// *inside* the worktree, so the confined `read_file` can fetch it back on demand.
pub const ARTIFACT_DIR: &str = ".harness/artifacts";

/// True when a tool result is large enough to offload to disk rather than re-feed
/// inline. Mirrors [`cap`]'s passthrough boundary exactly (`> limit` bytes), so the
/// loop offloads precisely the outputs `cap` would otherwise have truncated.
pub fn should_offload(content: &str, limit: usize) -> bool {
	content.len() > limit
}

/// Build the in-context view of an offloaded tool output: a head+tail preview (via
/// [`cap`], which keeps both ends and its own byte-elided marker) plus a MANDATORY
/// re-read hint naming the exact worktree-relative `rel_path`, the full byte count,
/// and how to fetch the rest. The hint is F3's §5 no-silent-loss guarantee — the
/// elided middle lives losslessly on disk and the model is told exactly where it is
/// and how to read it (paginated `read_file`, or `grep` for execution tickets).
pub fn offload_preview(content: &str, limit: usize, rel_path: &str) -> String {
	let preview = cap(content, limit);
	format!(
		"{preview}\n\n[harness: this output was {} bytes — the full copy is saved to `{rel_path}`. \
		 Re-read a slice with read_file using `offset`/`limit`, or search it with bash, e.g. \
		 `grep -n PATTERN {rel_path}`.]",
		content.len()
	)
}

/// Rough token count of one message: chars/4 over `content` + tool-call payloads.
/// (research/27 §6 — the field-standard proxy used by every clone we tore down;
/// conservative enough to drive a budget, not a tokenizer.)
fn msg_tokens(m: &Message) -> usize {
	(m.content.len() + m.tool_calls.iter().map(|c| c.arguments.len() + c.name.len()).sum::<usize>()) / 4
}

/// Token estimate (chars/4, research/27 §6) for the whole assembled prompt — the
/// number the loop warns against the reasoning-quality boundary with. Derived from
/// [`assembled_size`] so there is one byte-counting source of truth.
pub fn assembled_tokens(messages: &[Message]) -> usize {
	assembled_size(messages) / 4
}

/// Assemble the bounded working context the model actually sees this turn — F1 of
/// the context-engineering build (research/25 §4.2, sized by research/27 §6).
///
/// The loop accumulates the **full** transcript in `messages`; sending it verbatim
/// on an over-budget run lets oMLX silently truncate the *front* — and the front
/// is the system prompt (workpad contract + injected case-law). This function makes
/// head-survival a property of *our* code, not a hope about the server's eviction
/// order.
///
/// Shape (the field-convergent mechanism, research/25 §3): keep an immutable
/// **head** (the first `head_keep` messages — system prompt + task spec), keep a
/// verbatim recent **tail** sized to `keep_recent_tokens`, drop the middle with an
/// explicit marker. Sizing follows the A3B *reasoning* curve, not the 256K hard
/// window (research/27): `keep_recent_tokens ≈ 20K`, kept on top of the head.
///
/// Invariants (research/25 §5 — "lossy is allowed, *silently* lossy is not"):
/// - **Head is never dropped**, even when `keep_recent_tokens` is tiny.
/// - **No orphan tool_result**: the tail never *begins* with a `Tool`-role message
///   (its matching assistant `tool_call` would be in the dropped middle → a
///   provider 400). The boundary is walked **backward** to include the call —
///   omp `findCutPoint` / Hermes `_align_boundary_backward`.
/// - **No silent loss**: when anything is dropped, a marker message records how
///   many messages / ~tokens were elided.
///
/// When the transcript already fits, returns an unchanged clone — byte-identical
/// to the input, so short runs keep a stable prefix (KV-cache friendly) and behave
/// exactly as before this slice.
pub fn assemble(messages: &[Message], head_keep: usize, keep_recent_tokens: usize) -> Vec<Message> {
	let head_keep = head_keep.min(messages.len());
	if head_keep == messages.len() {
		return messages.to_vec();
	}

	let tail_start = tail_boundary(messages, head_keep, keep_recent_tokens);

	// Nothing between head and tail → no curation needed; return the stable clone.
	if tail_start <= head_keep {
		return messages.to_vec();
	}

	let dropped = tail_start - head_keep;
	let dropped_tokens: usize = messages[head_keep..tail_start].iter().map(msg_tokens).sum();
	let marker = Message::system(format!(
		"[CONTEXT ASSEMBLY — {dropped} older message(s) (~{dropped_tokens} tokens) elided from this turn's \
		 view to stay within the reasoning-quality window; the system prompt and the most recent turns are \
		 intact. F2 compaction folds this middle into a structured summary once the full transcript crosses \
		 the trigger; this elision is the non-destructive per-turn floor (research/25).]"
	));
	let mut out = Vec::with_capacity(head_keep + 1 + (messages.len() - tail_start));
	out.extend_from_slice(&messages[..head_keep]);
	out.push(marker);
	out.extend_from_slice(&messages[tail_start..]);
	out
}

/// Index where the verbatim recent tail begins — the shared boundary used by both
/// the per-turn [`assemble`] view (F1) and F2's [`compaction_span`], so the view and
/// the destructive fold never disagree about where the tail starts.
///
/// Walks backward from the end accumulating `keep_recent_tokens` of tail (the most
/// recent message is always kept, even if it alone exceeds the budget), then steps
/// the boundary back past any leading `Tool` message so the tail never *begins* on an
/// orphan tool_result whose matching call sits in the dropped middle (research/25 §5;
/// omp `findCutPoint` / Hermes `_align_boundary_backward`). The walk is bounded by
/// `head_keep` (system+user — never an assistant-with-calls), so it always terminates
/// on a safe boundary. Precondition: `head_keep < messages.len()` (both callers guard).
fn tail_boundary(messages: &[Message], head_keep: usize, keep_recent_tokens: usize) -> usize {
	let last = messages.len() - 1;
	let mut tail_start = messages.len();
	let mut tail_tokens = 0usize;
	for i in (head_keep..messages.len()).rev() {
		let t = msg_tokens(&messages[i]);
		if i != last && tail_tokens + t > keep_recent_tokens {
			break;
		}
		tail_tokens += t;
		tail_start = i;
	}
	while tail_start > head_keep && messages[tail_start].role == Role::Tool {
		tail_start -= 1;
	}
	tail_start
}

/// Whether the FULL running transcript has crossed the compaction trigger (F2,
/// research/25 §4.2, research/27 §6). Measured on the real `messages` vector — NOT
/// the assembled view, which is already capped to head+tail and so never reaches the
/// trigger. Strict `>` so a transcript sitting exactly at the boundary is left alone.
pub fn should_compact(messages: &[Message], trigger_tokens: usize) -> bool {
	assembled_tokens(messages) > trigger_tokens
}

/// The half-open span `[head_keep, tail_start)` of the transcript middle that F2
/// folds into a summary — the same middle [`assemble`] elides, but here it will be
/// *destructively* replaced. `None` when there is nothing to compact: no middle
/// between the protected head and the recent tail, or `head_keep` already covers the
/// whole transcript. Shares [`tail_boundary`] with `assemble`.
pub fn compaction_span(
	messages: &[Message],
	head_keep: usize,
	keep_recent_tokens: usize,
) -> Option<(usize, usize)> {
	let head_keep = head_keep.min(messages.len());
	if head_keep >= messages.len() {
		return None;
	}
	let tail_start = tail_boundary(messages, head_keep, keep_recent_tokens);
	if tail_start <= head_keep {
		return None;
	}
	Some((head_keep, tail_start))
}

/// Rebuild the transcript with the middle `span` replaced by a single summary
/// message (F2, research/25 §2.6/§2.8). The summary is a `System` message placed at
/// the span's start — immediately after the protected head — carrying
/// [`COMPACTION_HEADER`] + `summary`. Placing it *there* makes the next compaction's
/// span naturally include it, so successive folds are **cumulative** (§2.8) with no
/// separate boundary bookkeeping: each summary folds the previous summary plus the
/// turns since. Head and tail are preserved verbatim; only the middle is lost — and
/// only after the call site's no-progress guard confirms the rebuild is smaller.
pub fn apply_compaction(messages: &[Message], span: (usize, usize), summary: &str) -> Vec<Message> {
	let (start, end) = span;
	let folded = Message::system(format!("{COMPACTION_HEADER}\n\n{summary}"));
	let mut out = Vec::with_capacity(start + 1 + messages.len().saturating_sub(end));
	out.extend_from_slice(&messages[..start]);
	out.push(folded);
	out.extend_from_slice(&messages[end..]);
	out
}

#[cfg(test)]
mod tests {
	use super::*;
	use provider::ToolCall;

	#[test]
	fn under_limit_is_unchanged() {
		assert_eq!(cap("hello", 1024), "hello");
		assert_eq!(cap("", 10), "");
		// exactly at the limit is still unchanged (<=)
		assert_eq!(cap("abcde", 5), "abcde");
	}

	#[test]
	fn over_limit_keeps_head_and_tail_with_marker() {
		let text = "HEAD".to_string() + &"x".repeat(1000) + "TAIL";
		let out = cap(&text, 60);
		assert!(out.starts_with("HEAD"), "head preserved: {out:?}");
		assert!(out.ends_with("TAIL"), "tail preserved: {out:?}");
		assert!(out.contains("bytes elided"), "elision marker present: {out:?}");
		// the capped copy is far smaller than the raw
		assert!(out.len() < text.len() / 2);
	}

	#[test]
	fn elided_count_is_accurate() {
		// 30 bytes, limit 9 → head 6, tail 3, 21 elided.
		let text = "0123456789abcdefghijklmnopqrst"; // 30 ascii bytes
		let out = cap(text, 9);
		assert!(out.contains("[… 21 bytes elided …]"), "got: {out:?}");
		assert!(out.starts_with("012345"));
		assert!(out.ends_with("rst"));
	}

	#[test]
	fn limit_zero_drops_body_but_stays_honest() {
		let out = cap("some reasoning prose", 0);
		assert_eq!(out, "[… 20 bytes elided …]");
	}

	// F3: offload fires strictly above the cap (same boundary as `cap` passthrough).
	#[test]
	fn should_offload_is_strict_over_the_cap() {
		assert!(!should_offload("abcde", 5), "exactly at cap is inline");
		assert!(!should_offload("", 5));
		assert!(should_offload("abcdef", 5), "over cap offloads");
	}

	// F3: the offloaded preview keeps both ends AND names the exact artifact path +
	// byte count + a working re-read instruction — the §5 no-silent-loss guarantee.
	#[test]
	fn offload_preview_keeps_ends_and_names_the_artifact() {
		let text = "HEAD".to_string() + &"x".repeat(50_000) + "TAIL";
		let rel = ".harness/artifacts/tool-3.txt";
		let out = offload_preview(&text, 4096, rel);
		assert!(out.starts_with("HEAD"), "head preview present: {out:?}");
		assert!(out.contains("TAIL"), "tail preview present");
		assert!(out.contains(rel), "exact artifact path named");
		assert!(out.contains(&format!("{} bytes", text.len())), "full byte count stated");
		assert!(out.contains("read_file"), "re-read tool named");
		assert!(out.contains("offset"), "pagination hint present");
		assert!(out.len() < text.len() / 2, "preview is far smaller than the raw output");
	}

	#[test]
	fn never_splits_a_utf8_char() {
		// multibyte chars straddling the head/tail cut points must not panic and
		// must yield valid UTF-8 (the test would panic on a bad slice).
		let text = "α".repeat(100); // each 'α' is 2 bytes → 200 bytes
		let out = cap(&text, 15);
		assert!(out.is_char_boundary(0));
		assert!(out.contains("elided"));
		// round-trips as valid UTF-8 by construction (String), and keeps real αs
		assert!(out.starts_with('α'));
		assert!(out.ends_with('α'));
	}

	#[test]
	fn assembled_size_counts_content_and_tool_call_payloads() {
		let msgs = vec![
			Message::system("system"),                 // 6
			Message::user("hi"),                        // 2
			Message::assistant(
				"plan", // 4
				vec![ToolCall { id: "1".into(), name: "edit".into(), arguments: "{\"x\":1}".into() }], // name 4 + args 7
			),
			Message::tool_result("1", "ok"), // 2
		];
		// 6 + 2 + 4 + (4 + 7) + 2 = 25
		assert_eq!(assembled_size(&msgs), 25);
	}

	#[test]
	fn assembled_size_empty_is_zero() {
		assert_eq!(assembled_size(&[]), 0);
	}

	// --- F1: assemble (research/25 §4.2, research/27 §6) ---

	/// A message whose token estimate is ~`tokens` (chars/4 → 4 bytes/token).
	fn sized(role_text: &str, tokens: usize) -> Message {
		Message::user(role_text.repeat(tokens.max(1) * 4 / role_text.len().max(1)))
	}

	#[test]
	fn assemble_under_budget_is_unchanged() {
		let msgs = vec![Message::system("sys"), Message::user("task"), Message::assistant("ok", vec![])];
		let out = assemble(&msgs, 2, 20_000);
		assert_eq!(out.len(), msgs.len(), "nothing dropped when it fits");
		assert!(out.iter().all(|m| !m.content.contains("elided")), "no marker when under budget");
		assert_eq!(out[0].content, "sys");
		assert_eq!(out[2].content, "ok");
	}

	#[test]
	fn assemble_protects_head_when_over_budget() {
		let mut msgs = vec![Message::system("SYSTEM-PROMPT"), Message::user("THE-TASK")];
		// a fat middle that blows the budget, then a small recent tail
		for i in 0..20 {
			msgs.push(sized(&format!("mid{i} "), 50)); // ~50 tok each → ~1000 tok middle
		}
		msgs.push(Message::user("recent"));
		let out = assemble(&msgs, 2, 100); // 100-tok tail budget << middle
		// head survives verbatim
		assert_eq!(out[0].content, "SYSTEM-PROMPT");
		assert_eq!(out[1].content, "THE-TASK");
		// something was dropped, and it was announced (no silent loss)
		assert!(out.len() < msgs.len(), "middle dropped");
		assert!(out.iter().any(|m| m.content.contains("elided")), "marker present");
		// the most recent message is still there
		assert_eq!(out.last().unwrap().content, "recent");
	}

	#[test]
	fn assemble_never_leads_tail_with_orphan_tool_result() {
		// head(2) + [assistant(call C), tool_result C] as the only affordable tail.
		let msgs = vec![
			Message::system("sys"),
			Message::user("task"),
			Message::assistant("call A", vec![ToolCall { id: "a".into(), name: "bash".into(), arguments: "{}".into() }]),
			Message::tool_result("a", "x".repeat(400)), // ~100 tok
			Message::assistant("call C", vec![ToolCall { id: "c".into(), name: "bash".into(), arguments: "{}".into() }]),
			Message::tool_result("c", "done"),
		];
		// Tiny budget: the backward walk first lands on the last tool_result, then
		// must step back to include its assistant call.
		let out = assemble(&msgs, 2, 1);
		// first message after head + marker must NOT be a bare tool_result
		assert_eq!(out[2].role, Role::System, "marker sits right after the head");
		assert_ne!(out[3].role, Role::Tool, "tail does not begin with an orphan tool_result");
		assert_eq!(out[3].content, "call C", "the assistant call that owns the kept result is included");
		assert_eq!(out.last().unwrap().tool_call_id.as_deref(), Some("c"));
	}

	#[test]
	fn assemble_keeps_head_even_with_zero_budget() {
		let msgs = vec![
			Message::system("sys"),
			Message::user("task"),
			Message::assistant("a", vec![]),
			Message::assistant("b", vec![]),
			Message::user("last"),
		];
		let out = assemble(&msgs, 2, 0);
		assert_eq!(out[0].content, "sys");
		assert_eq!(out[1].content, "task");
		assert!(out.iter().any(|m| m.content.contains("elided")));
		assert_eq!(out.last().unwrap().content, "last", "at least the most recent turn is kept");
	}

	// --- F4: cap_tool_calls (research/25 §4.2) ---

	#[test]
	fn cap_tool_calls_bounds_large_args_keeps_id_and_name() {
		let big = format!("{{\"path\":\"a.rs\",\"content\":\"{}\"}}", "x".repeat(10_000));
		let calls = vec![ToolCall { id: "1".into(), name: "write_file".into(), arguments: big.clone() }];
		let out = cap_tool_calls(&calls, 4096);
		assert_eq!(out[0].id, "1");
		assert_eq!(out[0].name, "write_file");
		assert!(out[0].arguments.len() < big.len(), "args bounded");
		// The invariant the old splice-cap violated: the re-fed args MUST be valid JSON
		// (a corrupt object is exactly what triggered the provider 422 that killed the run).
		let v: serde_json::Value =
			serde_json::from_str(&out[0].arguments).expect("capped arguments must remain valid JSON");
		assert_eq!(v["path"], "a.rs", "structural field (path) survives intact");
		assert!(
			v["content"].as_str().unwrap().contains("bytes elided"),
			"oversized content elided in place — not silently dropped"
		);
	}

	#[test]
	fn cap_tool_calls_newline_content_stays_valid_json() {
		// Regression for the live 422: a `write_file` body full of real newlines, once
		// over `limit`, must re-serialize to valid JSON. The old head/tail splice cut the
		// JSON string mid-value and left a raw newline → "Invalid control character".
		let body = "def f():\n    return 1\n".repeat(800); // ~17 KB, newline-dense
		let args = serde_json::json!({ "path": "x.py", "content": body }).to_string();
		let calls = vec![ToolCall { id: "1".into(), name: "write_file".into(), arguments: args.clone() }];
		let out = cap_tool_calls(&calls, 4096);
		assert!(out[0].arguments.len() < args.len(), "args bounded");
		let v: serde_json::Value = serde_json::from_str(&out[0].arguments)
			.expect("newline-laden content must still produce valid JSON after capping");
		assert_eq!(v["path"], "x.py", "path survives");
	}

	#[test]
	fn cap_tool_calls_unparseable_args_degrade_to_valid_json() {
		// A pathological arg that is over-limit but not a JSON object must not be spliced
		// into a corrupt blob — it degrades to a bounded, valid-JSON placeholder.
		let junk = "x".repeat(9000); // > limit, not JSON
		let calls = vec![ToolCall { id: "1".into(), name: "bash".into(), arguments: junk.clone() }];
		let out = cap_tool_calls(&calls, 4096);
		let v: serde_json::Value =
			serde_json::from_str(&out[0].arguments).expect("placeholder must be valid JSON");
		assert!(v.get("_elided").is_some(), "unparseable args become an _elided placeholder");
	}

	#[test]
	fn cap_tool_calls_leaves_small_args_untouched() {
		let calls = vec![ToolCall { id: "1".into(), name: "read_file".into(), arguments: "{\"path\":\"a.rs\"}".into() }];
		let out = cap_tool_calls(&calls, 4096);
		assert_eq!(out[0].arguments, "{\"path\":\"a.rs\"}", "small args pass through unchanged");
	}

	#[test]
	fn assemble_head_keep_larger_than_len_is_safe() {
		let msgs = vec![Message::system("sys")];
		let out = assemble(&msgs, 2, 20_000);
		assert_eq!(out.len(), 1);
		assert_eq!(out[0].content, "sys");
	}

	// --- F2: should_compact / compaction_span / apply_compaction (research/25 §2.6/§2.8) ---

	#[test]
	fn should_compact_is_strict_over_the_trigger() {
		// assembled_tokens = assembled_size/4. Build ~25 tokens (100 bytes of content).
		let msgs = vec![Message::user("x".repeat(100))]; // 100 bytes → 25 tokens
		assert_eq!(assembled_tokens(&msgs), 25);
		assert!(should_compact(&msgs, 24), "above trigger compacts");
		assert!(!should_compact(&msgs, 25), "exactly at the trigger does not (strict >)");
		assert!(!should_compact(&msgs, 26), "below trigger does not");
	}

	#[test]
	fn compaction_span_picks_the_middle_and_is_none_when_empty() {
		let mut msgs = vec![Message::system("SYS"), Message::user("TASK")];
		for i in 0..20 {
			msgs.push(sized(&format!("mid{i} "), 50)); // ~1000-tok middle
		}
		msgs.push(Message::user("recent"));
		// fat middle, small tail budget → a real span starting at the head boundary.
		let (start, end) = compaction_span(&msgs, 2, 100).expect("a middle to fold");
		assert_eq!(start, 2, "span begins right after the protected head");
		assert!(end > start && end < msgs.len(), "span is the interior middle, not the tail");

		// huge tail budget swallows everything → nothing to compact.
		assert!(compaction_span(&msgs, 2, 1_000_000).is_none(), "no middle when the tail fits it all");
		// head_keep covering the whole transcript → None (no panic).
		assert!(compaction_span(&msgs, msgs.len(), 100).is_none());
		assert!(compaction_span(&msgs, msgs.len() + 5, 100).is_none());
	}

	#[test]
	fn apply_compaction_rebuilds_head_summary_tail_and_shrinks() {
		let mut msgs = vec![Message::system("SYS"), Message::user("TASK")];
		for i in 0..20 {
			msgs.push(sized(&format!("mid{i} "), 50));
		}
		msgs.push(Message::user("recent"));
		let span = compaction_span(&msgs, 2, 100).unwrap();
		let before = assembled_tokens(&msgs);
		let out = apply_compaction(&msgs, span, "SUMMARY-BODY");

		// head verbatim, summary at the head boundary, tail verbatim
		assert_eq!(out[0].content, "SYS");
		assert_eq!(out[1].content, "TASK");
		assert_eq!(out[2].role, Role::System, "fold is a system message");
		assert!(out[2].content.contains(COMPACTION_HEADER), "reference-only header present");
		assert!(out[2].content.contains("SUMMARY-BODY"), "summary body present");
		assert_eq!(out.last().unwrap().content, "recent", "the recent tail survives the fold");
		// the whole point: the rebuilt transcript is smaller.
		assert!(assembled_tokens(&out) < before, "compaction shrinks the transcript");
	}

	#[test]
	fn apply_compaction_is_cumulative_next_span_includes_prior_summary() {
		// First fold leaves [SYS, TASK, SUMMARY#1, ...tail]. A second fold's span must
		// start at the same head boundary and so re-include SUMMARY#1 → cumulative.
		let mut msgs = vec![Message::system("SYS"), Message::user("TASK")];
		for i in 0..20 {
			msgs.push(sized(&format!("a{i} "), 50));
		}
		msgs.push(Message::user("recent-1"));
		let span1 = compaction_span(&msgs, 2, 100).unwrap();
		let folded1 = apply_compaction(&msgs, span1, "SUMMARY#1");
		assert!(folded1[2].content.contains("SUMMARY#1"));

		// grow again past the tail budget, then fold a second time.
		let mut grown = folded1;
		for i in 0..20 {
			grown.push(sized(&format!("b{i} "), 50));
		}
		grown.push(Message::user("recent-2"));
		let span2 = compaction_span(&grown, 2, 100).unwrap();
		assert_eq!(span2.0, 2, "second span starts at the same head boundary");
		// the prior summary sits at index 2, inside [span2.0, span2.1) → folded again.
		assert!(span2.0 <= 2 && 2 < span2.1, "prior summary is inside the new span (cumulative fold)");
	}
}
