//! Memory researcher — the S3 **pure selection core** (research/31 §3/§5).
//!
//! A deep-research sub-agent that explores the memory store on its own — it runs a
//! bounded `search_memory → recall_body → return` loop on a local (oMLX) model and
//! hands the main working agent only the lessons it judged relevant (verbatim
//! bodies + the ids it used). It is the read-side upgrade to `recall_primed`'s
//! single lexical shot: selection by a model that can read snippets, not a floor.
//!
//! This module holds the pure parts — value types, the model's contract prompt, the
//! final-message parser, and the selection function — with **no board and no model**,
//! so the logic is unit-tested in isolation (the explore.rs "pure core" pattern). The
//! board-coupled tool dispatch + the oMLX loop glue (`run_memory_researcher`) live in
//! `main.rs`, beside `run_explore`.
//!
//! The live oMLX behaviour probe (research/31 breadcrumb) confirmed the served 35B
//! reliably drives this loop (searches first, fetches the right body, returns a clean
//! packet; fabricates nothing on a topic-absent query) — but it sometimes **loops on
//! redundant calls and fails to terminate**. Every such failure is the *safe* kind
//! (it runs out of turns, never emits wrong content), so the design is: cap the
//! round-trips, dedup repeated fetches, and on a cap-hit **synthesize** the packet
//! from whatever real bodies were already recalled. All three are exercised here or
//! in the glue; the caller falls back to `recall_primed` on an empty/None result.

/// What the researcher hands back to the main loop. `used_ids` are the lessons it
/// selected (traceable; also the key the glue re-materialises bodies from); `context`
/// is the model's verbatim assembly (bodies, possibly with connective notes). An
/// empty packet is a *valid* "nothing relevant" answer — the caller treats it as no
/// research result and falls back to the lexical floor.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContextPacket {
	pub used_ids: Vec<String>,
	pub context: String,
}

impl ContextPacket {
	/// A packet contributes nothing iff it cites no ids AND carries no prose.
	pub fn is_empty(&self) -> bool {
		self.used_ids.is_empty() && self.context.trim().is_empty()
	}
}

/// Hard cap on tool-call round-trips before the glue force-terminates and falls back
/// to synthesis. The probe showed the 35B sometimes re-issues the same call; the cap
/// turns a non-termination into a bounded, safe outcome rather than a hang.
pub const TOOL_CALL_CAP: usize = 12;

/// The researcher's contract. Mirrors the probe prompt that worked, plus the two
/// guards the probe's failures demanded: do NOT re-fetch an id you already have, and
/// STOP as soon as you have the relevant bodies (don't keep searching). The final
/// message must be a bare JSON object so `parse_packet` can read the selection.
pub const RESEARCHER_SYSTEM: &str = "\
You are a memory-research sub-agent for a coding agent. Given a task query, find the \
stored lessons most relevant to it and return their verbatim bodies to the main agent.

Workflow:
1. Call search_memory with focused keywords from the query. It returns candidate \
lessons as (id, title, snippet) — NOT full bodies.
2. Call recall_body(id) ONLY for the candidates that look genuinely relevant, to read \
the full text. Do NOT call recall_body twice for the same id — you already have it.
3. As soon as you have the bodies you need, STOP calling tools and reply with a single \
JSON object and nothing else:
   {\"used_ids\": [\"<id>\", ...], \"context\": \"<the relevant verbatim bodies, joined>\"}

Be selective: include only lessons that actually bear on the query. If after searching \
nothing is relevant, reply with {\"used_ids\": [], \"context\": \"\"} — an empty result is \
correct and better than forcing an irrelevant lesson. Do not invent lessons or ids.";

/// Parse the researcher's final assistant message into a packet. Tolerant: tries the
/// trimmed message as JSON, then falls back to the first `{` … last `}` slice (so a
/// fenced block or a stray sentence around the object still parses). Returns `None`
/// only when no JSON object is recoverable; a well-formed *empty* packet returns
/// `Some` (a deliberate "nothing relevant", distinct from a parse failure).
pub fn parse_packet(s: &str) -> Option<ContextPacket> {
	let v: serde_json::Value = serde_json::from_str(s.trim()).ok().or_else(|| {
		let start = s.find('{')?;
		let end = s.rfind('}')?;
		if end <= start {
			return None;
		}
		serde_json::from_str(&s[start..=end]).ok()
	})?;
	let used_ids = v
		.get("used_ids")
		.and_then(serde_json::Value::as_array)
		.map(|a| a.iter().filter_map(|e| e.as_str().map(str::to_owned)).collect())
		.unwrap_or_default();
	let context = v.get("context").and_then(serde_json::Value::as_str).unwrap_or("").trim().to_owned();
	Some(ContextPacket { used_ids, context })
}

/// Pick the lessons to inject from what was actually recalled. When the model cited
/// ids, keep those (in the model's order, deduped, dropping any it hallucinated that
/// were never fetched); when it cited none, or cited only unknown ids, fall back to
/// everything recalled (fetch order). Always capped to `max`.
///
/// This is the single code path for BOTH outcomes: natural termination passes the
/// packet's `used_ids`; a cap-hit/parse-failure passes `&[]` (→ synthesis from all
/// recalled bodies). `recalled` is the deduped `(id, body)` list the loop fetched.
pub fn select_lessons(recalled: &[(String, String)], used_ids: &[String], max: usize) -> Vec<(String, String)> {
	let mut chosen: Vec<(String, String)> = Vec::new();
	for id in used_ids {
		if chosen.iter().any(|(cid, _)| cid == id) {
			continue; // dedup a doubly-cited id
		}
		if let Some(pair) = recalled.iter().find(|(rid, _)| rid == id) {
			chosen.push(pair.clone());
		}
	}
	// No usable citation (none given, or all hallucinated) → synthesize from all recalled.
	if chosen.is_empty() {
		chosen = recalled.to_vec();
	}
	chosen.truncate(max);
	chosen
}

#[cfg(test)]
mod tests {
	use super::*;

	/// The lesson cap the glue applies (it passes `MEMORY_RECALL_K`); fixed here so
	/// the selection tests assert against a stable ceiling.
	const RESEARCHER_MAX_LESSONS: usize = 5;

	#[test]
	fn parse_clean_packet() {
		let p = parse_packet(r#"{"used_ids": ["m1", "m2"], "context": "body one. body two."}"#).unwrap();
		assert_eq!(p.used_ids, vec!["m1", "m2"]);
		assert_eq!(p.context, "body one. body two.");
		assert!(!p.is_empty());
	}

	#[test]
	fn parse_empty_packet_is_some_but_empty() {
		// a deliberate "nothing relevant" is a valid parse, not a failure
		let p = parse_packet(r#"{"used_ids": [], "context": ""}"#).unwrap();
		assert!(p.is_empty());
	}

	#[test]
	fn parse_tolerates_prose_and_fences_around_the_object() {
		let s = "Here is the result:\n```json\n{\"used_ids\":[\"m9\"],\"context\":\"x\"}\n```\nDone.";
		let p = parse_packet(s).unwrap();
		assert_eq!(p.used_ids, vec!["m9"]);
		assert_eq!(p.context, "x");
	}

	#[test]
	fn parse_garbage_is_none() {
		assert!(parse_packet("no json here at all").is_none());
		assert!(parse_packet("").is_none());
		assert!(parse_packet("}{").is_none(), "end-before-start brace span is not an object");
	}

	#[test]
	fn parse_missing_fields_default_empty() {
		// a malformed-but-parseable object must not panic; absent fields → empty
		let p = parse_packet(r#"{"foo": 1}"#).unwrap();
		assert!(p.used_ids.is_empty());
		assert_eq!(p.context, "");
		assert!(p.is_empty());
	}

	fn recalled() -> Vec<(String, String)> {
		vec![
			("m1".into(), "body 1".into()),
			("m2".into(), "body 2".into()),
			("m3".into(), "body 3".into()),
		]
	}

	#[test]
	fn select_keeps_cited_in_model_order() {
		let sel = select_lessons(&recalled(), &["m3".into(), "m1".into()], RESEARCHER_MAX_LESSONS);
		assert_eq!(sel, vec![("m3".into(), "body 3".into()), ("m1".into(), "body 1".into())]);
	}

	#[test]
	fn select_drops_hallucinated_ids() {
		// model cites one real + one it never fetched → only the real one survives
		let sel = select_lessons(&recalled(), &["m2".into(), "m99".into()], RESEARCHER_MAX_LESSONS);
		assert_eq!(sel, vec![("m2".into(), "body 2".into())]);
	}

	#[test]
	fn select_dedups_doubly_cited() {
		let sel = select_lessons(&recalled(), &["m1".into(), "m1".into()], RESEARCHER_MAX_LESSONS);
		assert_eq!(sel, vec![("m1".into(), "body 1".into())]);
	}

	#[test]
	fn select_no_citation_synthesizes_from_all_recalled() {
		// the cap-hit / parse-failure path: empty used_ids → everything recalled
		let sel = select_lessons(&recalled(), &[], RESEARCHER_MAX_LESSONS);
		assert_eq!(sel.len(), 3);
	}

	#[test]
	fn select_all_hallucinated_falls_back_to_recalled() {
		let sel = select_lessons(&recalled(), &["zzz".into()], RESEARCHER_MAX_LESSONS);
		assert_eq!(sel.len(), 3, "no usable citation → synthesize from recalled, not empty");
	}

	#[test]
	fn select_caps_at_max() {
		let big: Vec<(String, String)> = (0..10).map(|i| (format!("m{i}"), format!("b{i}"))).collect();
		assert_eq!(select_lessons(&big, &[], 5).len(), 5);
	}

	#[test]
	fn select_empty_recalled_is_empty() {
		assert!(select_lessons(&[], &["m1".into()], 5).is_empty());
		assert!(select_lessons(&[], &[], 5).is_empty());
	}
}
