//! Intake drafting — the **pure core** of `agent draft` (readiness-assessment
//! "Proposed next slice"; decisions.md "Task intake"). Given a ticket seed at any
//! fidelity (one line or a full spec), a strong provider proposes the workpad's
//! Plan / Acceptance Criteria / Validation, written through the existing board
//! chokepoints. The ticket stays **pre-Align**: a draft is a proposal, not a gate
//! pass — `criteria_confirmed` remains human-only and the keystone is untouched.
//!
//! Adversarial-review trigger check (assessment §"Proposed next slice"): drafting
//! is reversible AND loud (the operator reads the draft at Align before anything
//! executes; a bad draft costs nothing) → review-exempt. The two properties that
//! keep it that way are enforced in the glue and MUST NOT erode:
//!   1. `run_draft` never touches gates or status — write chokepoints only.
//!   2. It refuses to overwrite operator-authored fields without `--force`.
//!
//! Like `review.rs`/`researcher.rs`, this module holds the value types, the
//! contract prompt, the message builder, and the tolerant parser — no board, no
//! model — so the logic is unit-tested in isolation. The provider + board glue
//! (`run_draft`) lives in `main.rs`.

/// Output cap for the draft call. Sized like `REVIEW_MAX_TOKENS`: the default
/// drafting provider (DeepSeek) is a reasoning model whose `reasoning_content`
/// spends from this same budget before `content` — a low cap yields empty content.
pub const DRAFT_MAX_TOKENS: u32 = 8192;

/// Reasoning budget for the draft call (medium rung of the preset ladder).
/// Drafting is a judgment task; reasoning stays ON-but-bounded (Decision 3).
pub const DRAFT_THINK_BUDGET: u32 = 2048;

/// Char cap on each injected past-lesson body — priming is a nudge, not a dump.
pub const LESSON_BODY_CAP: usize = 1200;

/// Entry cap on the injected repository tree. A repo-relative source path runs
/// ~40 chars, so 100 entries ≈ 4 KB — enough to ground a ticket's neighbourhood
/// without drowning the seed (this repo is ~60 tracked files; a big one is 10k+,
/// where the relevance ordering in `clamp_tree` is what earns the cap its keep).
pub const TREE_MAX_ENTRIES: usize = 100;

/// Shortest title token used for tree relevance. Below this, tokens ("a", "to",
/// "in") match nearly every path and the ordering collapses to lexicographic.
const TITLE_TOKEN_MIN: usize = 3;

/// A parsed draft proposal. `questions` carries what the model judged genuinely
/// undecidable from the seed — surfaced to the operator so Align starts at the
/// real gaps instead of rediscovering them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Draft {
	pub plan: String,
	pub acceptance_criteria: String,
	pub validation: String,
	pub questions: Vec<String>,
}

impl Draft {
	/// A draft is usable iff it proposes BOTH a plan and acceptance criteria —
	/// criteria are the whole point (they are what Align confirms and what every
	/// downstream gate reviews against). `validation` may be empty (not every kind
	/// has a one-command check); the glue warns rather than rejects.
	pub fn is_usable(&self) -> bool {
		!self.plan.trim().is_empty() && !self.acceptance_criteria.trim().is_empty()
	}
}

/// The drafter's contract. The hard rules mirror what makes hand-authored
/// workpads gate well: enumerated, testable criteria (the AC-thin guard in
/// `review.rs` rejects vague ones); a single non-interactive validation command;
/// uncertainty surfaced as questions instead of invented requirements.
pub const DRAFT_SYSTEM: &str = "\
You draft the workpad for a ticket in a gated coding harness. From the seed \
(title, kind, any operator-provided fields, past lessons), propose the three \
workpad fields the operator will review at the Align gate.

Rules:
- acceptance_criteria: 3-7 enumerated, concrete, independently testable behaviours \
(one per line, `- ` bullets). No vague adjectives (\"good\", \"robust\", \"clean\"). \
Each criterion must be checkable by running code or reading an artifact.
- plan: numbered implementation steps, specific to the seed. Short — steps, not prose.
- validation: ONE non-interactive shell command that exits 0 iff the criteria hold \
(e.g. a test command). Empty string if no single command can check this kind of work.
- questions: anything genuinely undecidable from the seed that would change the plan \
or criteria. Do NOT invent requirements to fill gaps — ask instead. Empty if none.
- Paths: name ONLY files that appear in the repository tree given below. A file the \
plan must CREATE is stated as \"create <path>\". If a file you need is not listed in \
the tree, ask a question instead of inventing a path.
- Stay inside the seed's clear intent. A draft that quietly widens scope is wrong.

Reply with a single JSON object and nothing else:
{\"plan\": \"...\", \"acceptance_criteria\": \"...\", \"validation\": \"...\", \
\"questions\": [\"...\"]}";

/// The seed handed to the drafter — the ticket's identity plus whatever the
/// operator already wrote. Borrowed views; the glue passes the ticket's fields.
pub struct Seed<'a> {
	pub kind: &'a str,
	pub title: &'a str,
	pub notes: Option<&'a str>,
	pub plan: Option<&'a str>,
	pub acceptance_criteria: Option<&'a str>,
	pub validation: Option<&'a str>,
}

/// The lowercase title tokens tree relevance ranks on.
fn title_tokens(title: &str) -> Vec<String> {
	title
		.split(|c: char| !c.is_alphanumeric())
		.filter(|t| t.len() >= TITLE_TOKEN_MIN)
		.map(str::to_lowercase)
		.collect()
}

/// Order `paths` by relevance to `title` and clamp to `max_entries`, returning
/// the kept list and how many were dropped. Relevance is the length of the
/// longest title token the path contains (case-insensitive), longest first —
/// a ticket about "draft grounding" surfaces `draft.rs` above `board.rs`, and a
/// longer token match is a stronger signal than a short one. Ties (including the
/// no-match bulk, all rank 0) break lexicographically, so the section is stable
/// across runs and reads like a tree rather than a shuffle.
pub fn clamp_tree(paths: &[String], title: &str, max_entries: usize) -> (Vec<String>, usize) {
	let tokens = title_tokens(title);
	let mut ranked: Vec<(usize, &String)> = paths
		.iter()
		.map(|p| {
			let lower = p.to_lowercase();
			let best = tokens.iter().filter(|t| lower.contains(t.as_str())).map(String::len).max().unwrap_or(0);
			(best, p)
		})
		.collect();
	ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
	let dropped = ranked.len().saturating_sub(max_entries);
	(ranked.into_iter().take(max_entries).map(|(_, p)| p.clone()).collect(), dropped)
}

/// Build the (system, user) message pair. Operator-provided fields are labelled
/// as such — the drafter refines them rather than ignoring them (fidelity is a
/// dial: a rich seed should yield a near-verbatim draft). `lessons` are past
/// `(title, body)` memories from `recall_primed` — the compounding channel: the
/// more the store has seen, the less the seed has to say. `tree` is the repo's
/// tracked, repo-relative paths (the grounding channel: the drafter cannot see
/// the repo, so it invents plausible filenames — this is the reality it names
/// against). An empty tree renders no section at all: the glue passes one when
/// drafting outside a git repo, and a header over nothing would read to the model
/// as "this repo has no files".
pub fn build_draft_messages(seed: &Seed, lessons: &[(String, String)], tree: &[String]) -> (String, String) {
	let mut user = format!("Ticket kind: {}\nTitle: {}\n", seed.kind, seed.title);
	let mut section = |label: &str, v: Option<&str>| {
		if let Some(text) = v.map(str::trim).filter(|s| !s.is_empty()) {
			user.push_str(&format!("\nOperator-provided {label} (refine, don't discard):\n{text}\n"));
		}
	};
	section("notes", seed.notes);
	section("plan", seed.plan);
	section("acceptance criteria", seed.acceptance_criteria);
	section("validation", seed.validation);
	if !tree.is_empty() {
		let (shown, dropped) = clamp_tree(tree, seed.title, TREE_MAX_ENTRIES);
		user.push_str(&format!(
			"\nRepository tree (repo-relative, {} of {} files):\n",
			shown.len(),
			tree.len()
		));
		for p in &shown {
			user.push_str(p);
			user.push('\n');
		}
		if dropped > 0 {
			user.push_str(&format!("… +{dropped} more files not shown\n"));
		}
	}
	if !lessons.is_empty() {
		user.push_str("\nRelevant past experience (use to sharpen criteria and pre-empt known failure modes):\n");
		for (title, body) in lessons {
			let clamped: String = body.chars().take(LESSON_BODY_CAP).collect();
			user.push_str(&format!("- {title}: {clamped}\n"));
		}
	}
	user.push_str("\nDraft the workpad now.");
	(DRAFT_SYSTEM.to_string(), user)
}

/// Parse the drafter's reply. Tolerant like `researcher::parse_packet`: the
/// trimmed message as JSON, else the first `{` … last `}` slice (fenced blocks
/// and stray prose survive). `None` only when no JSON object is recoverable;
/// missing fields default to empty, and usability is judged separately by
/// `Draft::is_usable` so the glue can report *which* failure happened.
pub fn parse_draft(s: &str) -> Option<Draft> {
	let v: serde_json::Value = serde_json::from_str(s.trim()).ok().or_else(|| {
		let start = s.find('{')?;
		let end = s.rfind('}')?;
		if end <= start {
			return None;
		}
		serde_json::from_str(&s[start..=end]).ok()
	})?;
	let field = |k: &str| v.get(k).and_then(serde_json::Value::as_str).unwrap_or("").trim().to_owned();
	let questions = v
		.get("questions")
		.and_then(serde_json::Value::as_array)
		.map(|a| {
			a.iter()
				.filter_map(|e| e.as_str())
				.map(str::trim)
				.filter(|s| !s.is_empty())
				.map(str::to_owned)
				.collect()
		})
		.unwrap_or_default();
	Some(Draft {
		plan: field("plan"),
		acceptance_criteria: field("acceptance_criteria"),
		validation: field("validation"),
		questions,
	})
}

/// Extensions that make a bare, slash-free token read as a file rather than a
/// word — `Cargo.toml` and `main.rs` are paths; "e.g." and "3.5" are not.
const CODE_EXTENSIONS: &[&str] = &[
	"rs", "toml", "md", "py", "ts", "tsx", "js", "jsx", "json", "yaml", "yml", "sh", "txt", "sql", "lock",
	"cfg", "ini", "html", "css", "rb", "go", "c", "h", "cpp", "java",
];

/// Strip the wrapping the model puts around a path in prose — backticks, quotes,
/// brackets/parens — plus trailing sentence punctuation. The two ends use
/// different sets on purpose: a trailing `.` is a full stop (`draft.rs".` →
/// `draft.rs`), but a LEADING `.` is part of the name (`.gitignore`), and one
/// shared set would strip through only until the first non-member — leaving the
/// quote behind the full stop attached. Trailing `/` goes too, so `crates/agent/`
/// and `crates/agent` ground identically.
fn unwrap_token(tok: &str) -> &str {
	tok.trim_end_matches(|c: char| "`\"')]}>,;:.*/".contains(c))
		.trim_start_matches(|c: char| "`\"'([{<*".contains(c))
		.trim_start_matches("./")
}

/// Pull the path-shaped tokens out of prose, in order, deduped. A token counts
/// when it contains `/` or ends in a known code/doc extension. URLs (`://`) are
/// excluded — they are not repo paths and would always read as ungrounded.
pub fn extract_pathish(text: &str) -> Vec<String> {
	let mut out: Vec<String> = Vec::new();
	for raw in text.split_whitespace() {
		let tok = unwrap_token(raw);
		if tok.is_empty() || tok.contains("://") {
			continue;
		}
		let has_ext = tok
			.rsplit_once('.')
			.is_some_and(|(stem, ext)| !stem.is_empty() && CODE_EXTENSIONS.contains(&ext.to_lowercase().as_str()));
		if (tok.contains('/') || has_ext) && !out.iter().any(|p| p == tok) {
			out.push(tok.to_string());
		}
	}
	out
}

/// The path-shaped tokens across `drafted` that the repo tree does not back —
/// neither an exact file match nor a directory prefix of some tracked file. This
/// is a FLAG, not a gate: a draft legitimately names files it intends to create,
/// so the operator adjudicates at Align (see the module header — drafting stays
/// reversible-and-loud). An empty tree grounds nothing, so it yields nothing:
/// outside a git repo the check is silent rather than screaming at every path.
pub fn ungrounded_paths(drafted: &[&str], tree: &[String]) -> Vec<String> {
	if tree.is_empty() {
		return Vec::new();
	}
	let mut out: Vec<String> = Vec::new();
	for text in drafted {
		for p in extract_pathish(text) {
			let dir = format!("{p}/");
			let grounded = tree.iter().any(|e| e == &p || e.starts_with(&dir));
			if !grounded && !out.contains(&p) {
				out.push(p);
			}
		}
	}
	out
}

#[cfg(test)]
mod tests {
	use super::*;

	fn seed<'a>(title: &'a str) -> Seed<'a> {
		Seed { kind: "build", title, notes: None, plan: None, acceptance_criteria: None, validation: None }
	}

	#[test]
	fn parse_clean_draft() {
		let d = parse_draft(
			r#"{"plan":"1. do x","acceptance_criteria":"- a\n- b","validation":"cargo test","questions":["scope?"]}"#,
		)
		.unwrap();
		assert_eq!(d.plan, "1. do x");
		assert_eq!(d.acceptance_criteria, "- a\n- b");
		assert_eq!(d.validation, "cargo test");
		assert_eq!(d.questions, vec!["scope?"]);
		assert!(d.is_usable());
	}

	#[test]
	fn parse_tolerates_prose_and_fences() {
		let s = "Here you go:\n```json\n{\"plan\":\"p\",\"acceptance_criteria\":\"c\",\"validation\":\"\"}\n```";
		let d = parse_draft(s).unwrap();
		assert_eq!(d.plan, "p");
		assert_eq!(d.acceptance_criteria, "c");
		assert!(d.validation.is_empty());
		assert!(d.questions.is_empty());
		assert!(d.is_usable());
	}

	#[test]
	fn parse_garbage_is_none() {
		assert!(parse_draft("no json at all").is_none());
		assert!(parse_draft("").is_none());
		assert!(parse_draft("}{").is_none());
	}

	#[test]
	fn parse_missing_fields_default_empty_and_unusable() {
		let d = parse_draft(r#"{"plan":"only a plan"}"#).unwrap();
		assert_eq!(d.plan, "only a plan");
		assert!(d.acceptance_criteria.is_empty());
		assert!(!d.is_usable(), "no criteria → not usable");
	}

	#[test]
	fn parse_blank_questions_dropped() {
		let d = parse_draft(r#"{"plan":"p","acceptance_criteria":"c","validation":"v","questions":["", "  ", "real?"]}"#)
			.unwrap();
		assert_eq!(d.questions, vec!["real?"]);
	}

	#[test]
	fn usable_requires_plan_and_criteria() {
		assert!(!Draft::default().is_usable());
		assert!(!Draft { plan: "p".into(), ..Default::default() }.is_usable());
		assert!(!Draft { acceptance_criteria: "c".into(), ..Default::default() }.is_usable());
		assert!(Draft { plan: "p".into(), acceptance_criteria: "c".into(), ..Default::default() }.is_usable());
	}

	#[test]
	fn messages_carry_kind_title_and_contract() {
		let (system, user) = build_draft_messages(&seed("add lru cache"), &[], &[]);
		assert!(system.contains("acceptance_criteria"));
		assert!(system.contains("questions"));
		assert!(user.contains("build"));
		assert!(user.contains("add lru cache"));
		assert!(!user.contains("Operator-provided"), "no pre-seeded fields → no refinement sections");
		assert!(!user.contains("past experience"), "no lessons → no lessons section");
	}

	#[test]
	fn messages_label_operator_fields() {
		let s = Seed {
			kind: "bugfix",
			title: "t",
			notes: Some("repro: run x"),
			plan: None,
			acceptance_criteria: Some("- must keep old API"),
			validation: Some("   "), // whitespace-only → treated as absent
		};
		let (_, user) = build_draft_messages(&s, &[], &[]);
		assert!(user.contains("Operator-provided notes"));
		assert!(user.contains("repro: run x"));
		assert!(user.contains("Operator-provided acceptance criteria"));
		assert!(user.contains("- must keep old API"));
		assert!(!user.contains("Operator-provided validation"), "blank field must not add a section");
		assert!(!user.contains("Operator-provided plan"));
	}

	#[test]
	fn messages_inject_and_clamp_lessons() {
		let long_body = "x".repeat(LESSON_BODY_CAP + 500);
		let lessons = vec![("past failure".to_string(), long_body)];
		let (_, user) = build_draft_messages(&seed("t"), &lessons, &[]);
		assert!(user.contains("past experience"));
		assert!(user.contains("past failure"));
		// the body is clamped: the full 1700-char run must not appear
		assert!(!user.contains(&"x".repeat(LESSON_BODY_CAP + 1)));
		assert!(user.contains(&"x".repeat(LESSON_BODY_CAP)));
	}

	fn tree(paths: &[&str]) -> Vec<String> {
		paths.iter().map(|s| s.to_string()).collect()
	}

	// relevance ordering: the LONGEST title-token match wins, then lexicographic —
	// the property that makes a 100-entry window land on the ticket's neighbourhood
	// in a repo far larger than the window.
	#[test]
	fn clamp_tree_orders_by_longest_title_token_then_lexicographically() {
		let t = tree(&[
			"crates/agent/src/board.rs",
			"crates/agent/src/draft.rs",
			"README.md",
			"crates/agent/src/grounding_helper.rs",
		]);
		let (shown, dropped) = clamp_tree(&t, "draft grounding of the tree", 10);
		assert_eq!(dropped, 0, "under the cap → nothing dropped");
		assert_eq!(
			shown,
			vec![
				"crates/agent/src/grounding_helper.rs", // "grounding" (9) — longest match
				"crates/agent/src/draft.rs",            // "draft" (5)
				"README.md",                            // rank 0, lexicographic: 'R' < 'c'
				"crates/agent/src/board.rs",
			]
		);
	}

	// short title tokens ("of", "a") must not rank — they match nearly every path
	// and would collapse the ordering back to lexicographic.
	#[test]
	fn clamp_tree_ignores_sub_three_char_title_tokens() {
		let t = tree(&["zz/of_a.rs", "aa/draft.rs"]);
		let (shown, _) = clamp_tree(&t, "a draft of it", 10);
		assert_eq!(shown[0], "aa/draft.rs", "\"draft\" ranks; \"of\"/\"it\"/\"a\" must not");
	}

	// the cap keeps the top-N and reports the rest as dropped (never silently).
	#[test]
	fn clamp_tree_clamps_and_counts_the_tail() {
		let t = tree(&["a.rs", "b.rs", "c.rs", "d.rs", "e.rs"]);
		let (shown, dropped) = clamp_tree(&t, "unrelated title", 2);
		assert_eq!(shown, vec!["a.rs", "b.rs"]);
		assert_eq!(dropped, 3);
	}

	#[test]
	fn messages_render_tree_section_when_passed() {
		let t = tree(&["crates/agent/src/draft.rs", "README.md"]);
		let (_, user) = build_draft_messages(&seed("draft grounding"), &[], &t);
		assert!(user.contains("Repository tree (repo-relative, 2 of 2 files):"));
		assert!(user.contains("crates/agent/src/draft.rs"));
		assert!(user.contains("README.md"));
		assert!(!user.contains("more files not shown"), "nothing clamped → no tail");
	}

	#[test]
	fn messages_omit_tree_section_when_empty() {
		let (_, user) = build_draft_messages(&seed("t"), &[], &[]);
		assert!(!user.contains("Repository tree"), "empty tree → no header at all");
	}

	// a repo bigger than the window renders exactly TREE_MAX_ENTRIES lines plus a
	// loud tail — the drafter must know the list it sees is partial.
	#[test]
	fn messages_clamp_tree_with_loud_tail() {
		let t: Vec<String> = (0..TREE_MAX_ENTRIES + 20).map(|i| format!("src/f{i:03}.rs")).collect();
		let (_, user) = build_draft_messages(&seed("t"), &[], &t);
		assert!(user.contains(&format!("Repository tree (repo-relative, {TREE_MAX_ENTRIES} of {} files):", t.len())));
		assert!(user.contains("… +20 more files not shown"));
	}

	// the system contract must carry the grounding rule — the whole point of the tree.
	#[test]
	fn system_contract_forbids_inventing_paths() {
		assert!(DRAFT_SYSTEM.contains("repository tree"));
		assert!(DRAFT_SYSTEM.contains("create <path>"));
		assert!(DRAFT_SYSTEM.contains("ask a question instead of inventing a path"));
	}

	#[test]
	fn extract_pathish_strips_wrapping_and_dedupes() {
		let got = extract_pathish(
			"Edit `crates/agent/src/draft.rs`, add Cargo.toml and \"README.md\". \
			 Then crates/agent/src/draft.rs again (see docs/x.md). Version 3.5, e.g. not a path.",
		);
		assert_eq!(got, vec!["crates/agent/src/draft.rs", "Cargo.toml", "README.md", "docs/x.md"]);
	}

	#[test]
	fn extract_pathish_rejects_prose_and_urls() {
		assert!(extract_pathish("run cargo test --workspace until it is green").is_empty());
		assert!(extract_pathish("see https://example.com/docs/x.md for context").is_empty());
	}

	// exact match and directory-prefix match both ground; a plausible-but-absent
	// path does not — that is the hallucination the flag exists to surface.
	#[test]
	fn ungrounded_paths_flags_only_unbacked_paths() {
		let t = tree(&["crates/agent/src/draft.rs", "crates/agent/src/git.rs", "README.md"]);
		let got = ungrounded_paths(
			&[
				"1. Edit `crates/agent/src/draft.rs`; create crates/agent/src/tree.rs",
				"- README.md stays accurate; crates/agent/src/ is the home",
				"cargo test -- docs/guide.md",
			],
			&t,
		);
		assert_eq!(got, vec!["crates/agent/src/tree.rs", "docs/guide.md"]);
	}

	// no tree (drafting outside a git repo) → the check is silent, not a screaming
	// list of every path the draft named.
	#[test]
	fn ungrounded_paths_is_silent_without_a_tree() {
		assert!(ungrounded_paths(&["edit crates/agent/src/anything.rs"], &[]).is_empty());
	}
}
