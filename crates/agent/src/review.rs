//! Decorrelated acceptance review — the POINTWISE acceptance reviewer (research/19
//! review fold-in; calibrated 2026-06-11). Given ONE branch's diff and the ticket's
//! acceptance criteria, a *decorrelated* model (DeepSeek — a different lineage from
//! the local-35B workers) judges whether the change ACTUALLY and COMPLETELY satisfies
//! the criteria. It catches the passer that should not have passed: an impl overfit to
//! the visible test inputs, a hard-coded lookup table, or a no-op that slipped a weak
//! validation. This reproduces, automatically, the by-hand generalization-oracle
//! integrity check the operator ran across shakedowns #5–#9.
//!
//! **What this is NOT.** This is a POINTWISE check — one branch vs its own AC. It is
//! NOT the deferred §9.6/§9.10 *pairwise tiebreak ranker* (which would rank survivors
//! against each other, and is gated on objective ranking becoming ambiguous in
//! practice — a trigger that has not fired). This module never ranks branches and must
//! never mutate `explore::rank_outcomes`. It is **advisory only**: it does not gate,
//! land, or change the board.
//!
//! Like `explore`, this is the **pure core** — value types + the AC-thinness check,
//! prompt builder, and verdict parser, with no model and no git, so the logic is
//! unit-tested in isolation. The DeepSeek + git glue (`run_review`) lives in `main.rs`.
//!
//! Four adversarial-review findings are baked in (see `run_review`/`print_review` for
//! the two glue-side ones): (1) AC-thin guard — refuse to review against a thin AC, a
//! review is only as strong as its criteria; (2) truncation → distinct verdict, never
//! parse a length-truncated body; (3) a `confidence` field + a false-negative nudge —
//! the asymmetric danger is a false SATISFIES on subtly-wrong code; (4) experimental
//! labelling until the false-negative rate is measured on real tickets.
//!
//! That measurement ran (5-ticket trial window, 2026-08-02): 0 true positives, 0 false
//! VIOLATES — the reviewer gated nothing, so the AUTOMATIC call in `agent sprint` was
//! deleted (decisions.md "Reviewer fix-or-drop: RESOLVED → DELETE"). The module and the
//! `agent review <id>` verb remain for DELIBERATE manual use; nothing invokes them
//! automatically, so no harness phase meters a DeepSeek call on its own.

/// The decorrelated reviewer model. Hard-coded to a DIFFERENT lineage than the oMLX
/// workers on purpose — local-35B judging local-35B is self-preference circularity.
pub const REVIEW_MODEL: &str = "deepseek-v4-pro";

/// Hard per-call output cap (CoT + answer together). deepseek-v4-pro is a reasoning
/// model: `reasoning_content` is spent from this budget *before* `content`, so a low
/// cap yields empty content. 8192 is the validated floor from calibration (1200 gave
/// empty content → PARSE_FAIL; 8000 gave clean JSON + separate reasoning).
pub const REVIEW_MAX_TOKENS: u32 = 8192;

/// Reasoning budget → `reasoning_effort: "high"` (the dialect maps <4096 to "high").
/// Calibration ran with reasoning ON (DeepSeek default) and discriminated correctly;
/// this reproduces that. `None` would *disable* reasoning, so it is the wrong default.
pub const REVIEW_THINK_BUDGET: u32 = 2048;

/// Char cap on the diff fed into the prompt — keep the request bounded. A larger diff
/// is truncated with a marker (the operator reviews oversized diffs by hand anyway).
pub const DIFF_CAP: usize = 24_000;

/// Word floor below which an AC is "thin" unless it enumerates ≥`MIN_CRITERIA`
/// distinct criteria (a terse-but-structured 3-bullet AC is not thin).
const THIN_WORD_FLOOR: usize = 25;
/// Distinct enumerated criteria (bullets or clauses) that rescue a short AC.
const MIN_CRITERIA: usize = 2;

/// The verdict space. `Satisfies`/`Suspect`/`Violates` come from the model; the other
/// three are produced by the harness, never the model: `Truncated` (length-capped
/// response, finding 2), `AcThin` (refused before the call, finding 1), `ParseFail`
/// (the model returned no parseable JSON).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
	Satisfies,
	Suspect,
	Violates,
	Truncated,
	AcThin,
	ParseFail,
}

impl Verdict {
	pub fn as_str(self) -> &'static str {
		match self {
			Verdict::Satisfies => "SATISFIES",
			Verdict::Suspect => "SUSPECT",
			Verdict::Violates => "VIOLATES",
			Verdict::Truncated => "TRUNCATED",
			Verdict::AcThin => "AC-THIN",
			Verdict::ParseFail => "PARSE-FAIL",
		}
	}
}

/// The model's stated confidence in its verdict. Missing/unknown → `Medium` (a model
/// that omits the field gets the benefit of the doubt, not a false `High`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
	High,
	Medium,
	Low,
}

impl Confidence {
	pub fn as_str(self) -> &'static str {
		match self {
			Confidence::High => "high",
			Confidence::Medium => "medium",
			Confidence::Low => "low",
		}
	}
}

/// A parsed (or harness-constructed) review result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewVerdict {
	pub verdict: Verdict,
	pub confidence: Confidence,
	pub concerns: Vec<String>,
	pub summary: String,
}

impl ReviewVerdict {
	fn bare(verdict: Verdict, summary: &str) -> Self {
		ReviewVerdict { verdict, confidence: Confidence::Medium, concerns: Vec::new(), summary: summary.into() }
	}
	/// Finding 2: a length-truncated response that partially parses → false SATISFIES is
	/// the worst case. The glue checks `StopReason::Length` and constructs this BEFORE
	/// any parse — a truncated body is never handed to `parse_verdict`.
	pub fn truncated() -> Self {
		Self::bare(Verdict::Truncated, "response hit the output cap — re-run with a higher max_tokens; not parsed")
	}
	fn parse_fail(raw: &str) -> Self {
		// keep a short tail of what came back, to debug a prompt/model regression.
		let tail: String = raw.trim().chars().rev().take(160).collect::<String>().chars().rev().collect();
		ReviewVerdict { verdict: Verdict::ParseFail, confidence: Confidence::Medium, concerns: Vec::new(),
			summary: format!("no parseable JSON verdict in the response (tail: …{tail})") }
	}
}

/// Finding 1: is the AC too thin to review against? A review is only as strong as its
/// criteria — a SATISFIES on a one-line AC is a mirror (the model invents criteria from
/// the diff, which is circular). Thin = short overall AND lacking enumerated structure.
///
/// This is a **structural** heuristic, not a semantic one, and it deliberately errs
/// conservative (toward flagging thin): a false-thin just asks the human to enrich the
/// AC and re-run, whereas a false-pass would let a circular review proceed — so for a
/// circularity *guard*, biasing toward thin is the safe direction. Two known limits the
/// guard does NOT close (cold review 2026-06-11): (a) a genuinely rich AC written as one
/// run-on sentence with no bullets/periods can be flagged thin (annoyance, conservative
/// direction); (b) deliberately verbose-but-vacuous padding ("make it good and make it
/// nice and …") clears the word floor and passes — a word/clause count cannot tell
/// enumerated requirements from filler. The backstop for (b) is the whole design:
/// advisory-only + human-in-loop + the experimental label, so a padded-AC review is
/// never load-bearing. If padded ACs ever slip through in practice, that is the trigger
/// to add a semantic check — not before (Wu Wei).
pub fn ac_is_thin(ac: &str) -> bool {
	let ac = ac.trim();
	if ac.is_empty() {
		return true;
	}
	let words = ac.split_whitespace().count();
	words < THIN_WORD_FLOOR && enumerated_criteria(ac) < MIN_CRITERIA
}

/// Count distinct enumerated criteria: the larger of (bulleted lines) and (clause-like
/// segments of ≥3 words). A 3-bullet AC and a 3-sentence AC both read as 3 criteria.
fn enumerated_criteria(ac: &str) -> usize {
	let bullets = ac
		.lines()
		.filter(|l| {
			let l = l.trim_start();
			l.starts_with('-') || l.starts_with('*') || l.starts_with(|c: char| c.is_ascii_digit())
		})
		.count();
	let clauses = ac.split(['.', ';', '\n']).filter(|s| s.split_whitespace().count() >= 3).count();
	bullets.max(clauses)
}

/// Cap the diff fed into the prompt. Returns the diff unchanged if within `cap`, else a
/// head slice with a visible truncation marker (so the model knows it saw a partial diff).
pub fn truncate_diff(diff: &str, cap: usize) -> String {
	if diff.len() <= cap {
		return diff.to_string();
	}
	let mut end = cap;
	while !diff.is_char_boundary(end) {
		end -= 1;
	}
	format!("{}\n…[diff truncated at {cap} chars for review]…", &diff[..end])
}

/// Build the (system, user) messages for the decorrelated reviewer. The system prompt
/// pins the skeptic stance + the strict-JSON schema (with `confidence`, finding 3); the
/// user message carries the AC and the diff under review.
pub fn build_review_messages(ac: &str, diff: &str, label: &str) -> (String, String) {
	let system = "You are a decorrelated code reviewer. You did NOT write this code and have no \
stake in it passing. Given a task's acceptance criteria and a diff, judge whether the code \
ACTUALLY and COMPLETELY satisfies the criteria. Be skeptical: code can pass a weak test suite \
while silently omitting required logic, overfitting to the visible test inputs, hard-coding \
expected outputs, or no-op'ing. Look specifically for required behaviour that is described but \
not implemented.\n\n\
The dangerous error is a FALSE SATISFIES — passing code that is subtly wrong. When you cannot \
confirm a requirement from the diff, prefer SUSPECT over SATISFIES and lower your confidence.\n\n\
Respond with STRICT JSON only, no prose around it:\n\
{\"verdict\": \"SATISFIES\" | \"SUSPECT\" | \"VIOLATES\", \"confidence\": \"high\" | \"medium\" | \
\"low\", \"concerns\": [\"specific concern\", ...], \"summary\": \"one line\"}\n\
- SATISFIES: implements every stated requirement correctly.\n\
- SUSPECT: probably works but you see a real risk or cannot confirm a requirement.\n\
- VIOLATES: a stated requirement is provably missing or wrong.\n\
- confidence: how sure you are of the verdict given only the diff you can see."
		.to_string();
	let user = format!(
		"## Acceptance criteria\n{ac}\n\n## Diff under review ({label})\n```diff\n{diff}\n```"
	);
	(system, user)
}

/// Parse the model's reply into a `ReviewVerdict`. Robust to markdown fences and prose
/// around the JSON (extracts the first `{`…last `}` span). Operates on `content` only —
/// the glue must pass `response.text`, never the separate `reasoning_content`. Any
/// failure → a `ParseFail` verdict (never a guessed Satisfies).
pub fn parse_verdict(content: &str) -> ReviewVerdict {
	let Some(json) = extract_json(content) else {
		return ReviewVerdict::parse_fail(content);
	};
	let Ok(v) = serde_json::from_str::<serde_json::Value>(&json) else {
		return ReviewVerdict::parse_fail(content);
	};
	let verdict = match v.get("verdict").and_then(|x| x.as_str()).unwrap_or("").to_ascii_uppercase().as_str() {
		"SATISFIES" => Verdict::Satisfies,
		"SUSPECT" => Verdict::Suspect,
		"VIOLATES" => Verdict::Violates,
		_ => return ReviewVerdict::parse_fail(content),
	};
	let confidence = match v.get("confidence").and_then(|x| x.as_str()).unwrap_or("").to_ascii_lowercase().as_str() {
		"high" => Confidence::High,
		"low" => Confidence::Low,
		_ => Confidence::Medium, // missing/unknown → medium
	};
	let concerns = v
		.get("concerns")
		.and_then(|x| x.as_array())
		.map(|a| a.iter().filter_map(|c| c.as_str().map(str::to_string)).collect())
		.unwrap_or_default();
	let summary = v.get("summary").and_then(|x| x.as_str()).unwrap_or("").to_string();
	ReviewVerdict { verdict, confidence, concerns, summary }
}

/// Extract the first balanced-ish JSON object span — first `{` through last `}`. Cheap
/// and good enough: the schema is a flat object, and this tolerates code fences/prose.
fn extract_json(s: &str) -> Option<String> {
	let start = s.find('{')?;
	let end = s.rfind('}')?;
	(end > start).then(|| s[start..=end].to_string())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn thin_ac_flagged() {
		assert!(ac_is_thin(""));
		assert!(ac_is_thin("   "));
		assert!(ac_is_thin("make it work"));
		assert!(ac_is_thin("Fix the bug."));
		// short but enumerated → rescued (3 bullets = 3 criteria ≥ MIN).
		assert!(!ac_is_thin("- left\n- right\n- delete"));
	}

	#[test]
	fn substantive_ac_passes() {
		let ac = "Implement three exported pure functions. terminalWordNavigationSequence returns \
		          word-left or word-right. terminalLineNavigationSequence is macOS only. \
		          terminalDeleteSequence handles backspace. Every binding fires only on its sole modifier.";
		assert!(!ac_is_thin(ac));
		// the keymap kata's bulleted contract.
		let bulleted = "Implement keymap.mjs:\n- word nav on Alt-sole\n- line nav on Meta-sole (mac)\n- delete on Cmd/Opt/Ctrl-sole";
		assert!(!ac_is_thin(bulleted));
	}

	#[test]
	fn parse_clean_json() {
		let v = parse_verdict(r#"{"verdict":"SATISFIES","confidence":"high","concerns":[],"summary":"all good"}"#);
		assert_eq!(v.verdict, Verdict::Satisfies);
		assert_eq!(v.confidence, Confidence::High);
		assert!(v.concerns.is_empty());
		assert_eq!(v.summary, "all good");
	}

	#[test]
	fn parse_fenced_with_prose() {
		let raw = "Here is my verdict:\n```json\n{\"verdict\": \"VIOLATES\", \"confidence\": \"high\", \
		           \"concerns\": [\"missing sole-modifier guard\", \"no macOS check\"], \"summary\": \"incomplete\"}\n```\nDone.";
		let v = parse_verdict(raw);
		assert_eq!(v.verdict, Verdict::Violates);
		assert_eq!(v.concerns.len(), 2);
		assert_eq!(v.concerns[0], "missing sole-modifier guard");
	}

	#[test]
	fn missing_confidence_defaults_medium() {
		let v = parse_verdict(r#"{"verdict":"SUSPECT","concerns":["unconfirmed"],"summary":"risk"}"#);
		assert_eq!(v.verdict, Verdict::Suspect);
		assert_eq!(v.confidence, Confidence::Medium);
	}

	#[test]
	fn garbage_is_parse_fail_not_satisfies() {
		assert_eq!(parse_verdict("I think this looks fine to me!").verdict, Verdict::ParseFail);
		assert_eq!(parse_verdict("").verdict, Verdict::ParseFail);
		// an unknown verdict string is a parse failure, never a silent pass.
		assert_eq!(parse_verdict(r#"{"verdict":"LGTM"}"#).verdict, Verdict::ParseFail);
	}

	#[test]
	fn two_json_objects_fail_safe_to_parse_fail() {
		// extract_json spans first `{`…last `}`, so a scratch blob before the real verdict
		// yields an invalid combined span → ParseFail. Fail-safe: never a guessed pass even
		// if the SECOND object would have been a SATISFIES.
		let raw = r#"{"scratch":"thinking"} then {"verdict":"SATISFIES","confidence":"high"}"#;
		assert_eq!(parse_verdict(raw).verdict, Verdict::ParseFail);
	}

	#[test]
	fn brace_inside_concern_string_parses() {
		// the outermost `{`…`}` are the real object delimiters; a `}` *inside* a string is
		// serde's problem, not ours, and serde handles it. Verdict + concern survive intact.
		let raw = r#"{"verdict":"VIOLATES","confidence":"high","concerns":["missing } guard on Alt"],"summary":"x"}"#;
		let v = parse_verdict(raw);
		assert_eq!(v.verdict, Verdict::Violates);
		assert_eq!(v.concerns, vec!["missing } guard on Alt".to_string()]);
	}

	#[test]
	fn garbage_confidence_defaults_medium() {
		// a non-{high,low} confidence value is not a parse failure — the verdict still
		// stands, confidence falls back to medium (same arm as a missing field).
		let v = parse_verdict(r#"{"verdict":"SATISFIES","confidence":"banana","summary":"x"}"#);
		assert_eq!(v.verdict, Verdict::Satisfies);
		assert_eq!(v.confidence, Confidence::Medium);
	}

	#[test]
	fn truncated_constructor_is_distinct() {
		assert_eq!(ReviewVerdict::truncated().verdict, Verdict::Truncated);
	}

	#[test]
	fn diff_truncation_marks_and_bounds() {
		let small = "abc";
		assert_eq!(truncate_diff(small, 100), "abc");
		let big = "x".repeat(50);
		let out = truncate_diff(&big, 10);
		assert!(out.starts_with(&"x".repeat(10)));
		assert!(out.contains("truncated"));
	}

	#[test]
	fn messages_carry_ac_and_diff() {
		let (sys, user) = build_review_messages("the criteria here", "the diff body", "t99-w0");
		assert!(sys.contains("decorrelated"));
		assert!(sys.contains("FALSE SATISFIES"));
		assert!(user.contains("the criteria here"));
		assert!(user.contains("the diff body"));
		assert!(user.contains("t99-w0"));
	}
}
