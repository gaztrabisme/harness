//! `wiki check` — a Rust port of efficient-pi's `bin/wiki-check` (the numeric
//! wiki housekeeping gate, its unit B5), so the pi board extension can run the
//! check on macOS, Linux and Windows without bash. Same checks, same
//! R5-measured thresholds (the script says: do not tune), same exit contract
//! (0 pass / 1 fail):
//!
//!   wiki/active-work.md  <= 400 lines, <= 4000 o200k tokens, <= 24000 bytes
//!   wiki/index.md        <= 120 lines
//!   wiki/log.md          <= 2000 lines (the R5 filing trigger)
//!   index <-> disk       every wiki/*.md indexed, every index link on disk
//!   wiki/facts.md        every dated `check:` command exits 0 (run from root)
//!
//! Tokens: `<root>/bin/pi token @<file>` is consulted when that script exists
//! and answers with an `o200k:` line — the source the bash script greps. On
//! Windows (and whenever `bin/pi` is absent or fails) the count falls back to
//! the bytes/4 estimate the script itself uses, and the human check line says
//! so. Read-only: nothing here writes to the wiki it checks. All paths go
//! through `std::path` joins, never string concatenation, so native Windows
//! separators work.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

// R5's measured thresholds — bin/wiki-check carries them as constants of the
// practice ("Thresholds are R5's measured ones — do not tune here").
pub const ACTIVE_WORK_MAX_LINES: i64 = 400;
pub const ACTIVE_WORK_MAX_TOKENS: i64 = 4000;
pub const ACTIVE_WORK_MAX_BYTES: i64 = 24000;
pub const INDEX_MAX_LINES: i64 = 120;
pub const LOG_MAX_LINES: i64 = 2000;

/// One housekeeping check. `measured`/`limit` are JSON values so numbers stay
/// numbers on the wire (`--json` emits exactly name/ok/measured/limit).
pub struct Check {
	pub name: String,
	pub ok: bool,
	pub measured: Value,
	pub limit: Value,
	/// Text-only annotation for the human line (the token estimator source).
	/// Deliberately not part of the JSON contract.
	pub note: Option<String>,
	/// Text-only detail lines printed above the check line (the per-file
	/// UNINDEXED:/MISSING: violations the bash script prints).
	pub details: Vec<String>,
}

/// Everything one invocation measured: overall pass + the checks in script order.
pub struct Report {
	pub ok: bool,
	pub checks: Vec<Check>,
}

/// Run every housekeeping check against `root`'s `wiki/`. Mirrors the script's
/// control flow: a missing wiki dir is an immediate fail; missing required
/// files fail their own check and skip the checks that would read them.
pub fn run(root: &Path) -> Report {
	let mut checks: Vec<Check> = Vec::new();
	let wiki = root.join("wiki");
	if !wiki.is_dir() {
		checks.push(Check {
			name: "wiki-dir".into(),
			ok: false,
			measured: json!(wiki.display().to_string()),
			limit: json!("present"),
			note: Some("run bin/wiki-init".into()),
			details: Vec::new(),
		});
		return Report { ok: false, checks };
	}

	// Required files first (the script's MISSING pass): a missing file is one
	// failed check, and the checks that would read it are skipped, not faked.
	let mut have = std::collections::BTreeMap::new();
	for f in ["index.md", "active-work.md", "log.md"] {
		let present = wiki.join(f).is_file();
		have.insert(f, present);
		checks.push(ck(
			&format!("wiki/{f}"),
			present,
			json!(if present { "present" } else { "missing" }),
			json!("present"),
		));
	}

	// active-work.md: lines / tokens / bytes.
	if have["active-work.md"] {
		match read_bytes(&wiki.join("active-work.md")) {
			Some(bytes) => {
				let lines = newline_count(&bytes);
				checks.push(ck(
					"active-work-lines",
					lines <= ACTIVE_WORK_MAX_LINES,
					json!(lines),
					json!(ACTIVE_WORK_MAX_LINES),
				));
				let (tokens, note) = token_count(root, &wiki.join("active-work.md"));
				checks.push(Check {
					name: "active-work-tokens".into(),
					ok: tokens <= ACTIVE_WORK_MAX_TOKENS,
					measured: json!(tokens),
					limit: json!(ACTIVE_WORK_MAX_TOKENS),
					note,
					details: Vec::new(),
				});
				let size = bytes.len() as i64;
				checks.push(ck(
					"active-work-bytes",
					size <= ACTIVE_WORK_MAX_BYTES,
					json!(size),
					json!(ACTIVE_WORK_MAX_BYTES),
				));
			}
			None => checks.push(unreadable("active-work")),
		}
	}

	// index.md / log.md: lines.
	if have["index.md"] {
		match read_bytes(&wiki.join("index.md")) {
			Some(bytes) => {
				let lines = newline_count(&bytes);
				checks.push(ck("index-lines", lines <= INDEX_MAX_LINES, json!(lines), json!(INDEX_MAX_LINES)));
			}
			None => checks.push(unreadable("index")),
		}
	}
	if have["log.md"] {
		match read_bytes(&wiki.join("log.md")) {
			Some(bytes) => {
				let lines = newline_count(&bytes);
				checks.push(ck("log-lines", lines <= LOG_MAX_LINES, json!(lines), json!(LOG_MAX_LINES)));
			}
			None => checks.push(unreadable("log")),
		}
	}

	// index <-> disk, only when there is an index to check against.
	if have["index.md"] {
		match read_bytes(&wiki.join("index.md")) {
			Some(bytes) => {
				let index = String::from_utf8_lossy(&bytes);
				let mut details = Vec::new();
				let mut violations = 0i64;
				// every wiki/*.md except index.md must be linked as "(stem.md)"
				let mut on_disk: Vec<String> = std::fs::read_dir(&wiki)
					.map(|rd| {
						rd.filter_map(|e| e.ok())
							.map(|e| e.file_name().to_string_lossy().into_owned())
							.filter(|n| n.ends_with(".md") && n != "index.md")
							.map(|n| n.trim_end_matches(".md").to_string())
							.collect()
					})
					.unwrap_or_default();
				on_disk.sort();
				for stem in on_disk {
					if !index.contains(&format!("({stem}.md)")) {
						details.push(format!("UNINDEXED: {stem}"));
						violations += 1;
					}
				}
				// every "(name.md)" link in the index must exist on disk
				for name in md_link_names(&index) {
					if !wiki.join(&name).is_file() {
						details.push(format!("MISSING: wiki/{name}"));
						violations += 1;
					}
				}
				checks.push(Check {
					name: "index-disk".into(),
					ok: violations == 0,
					measured: json!(violations),
					limit: json!(0),
					note: None,
					details,
				});
			}
			None => checks.push(unreadable("index")),
		}
	}

	// wiki/facts.md: run every dated `check:` command from the project root.
	let facts = wiki.join("facts.md");
	if facts.is_file() {
		match read_bytes(&facts) {
			Some(bytes) => {
				for line in String::from_utf8_lossy(&bytes).lines() {
					let Some((claim, cmd)) = split_fact(line) else { continue };
					let code = run_shell(cmd, root).unwrap_or(-1);
					checks.push(ck(&format!("fact {claim}"), code == 0, json!(code), json!(0)));
				}
			}
			None => checks.push(unreadable("facts")),
		}
	}

	let ok = checks.iter().all(|c| c.ok);
	Report { ok, checks }
}

/// The human line for one check — `ok|FAIL <check> <measured> <limit>` — with
/// any detail lines above it and the text-only annotation appended.
pub fn line(check: &Check) -> String {
	let mut out = String::new();
	for d in &check.details {
		out.push_str(d);
		out.push('\n');
	}
	let verdict = if check.ok { "ok" } else { "FAIL" };
	out.push_str(&format!("{verdict} {} {} {}", check.name, value_to_text(&check.measured), value_to_text(&check.limit)));
	if let Some(note) = &check.note {
		out.push_str(&format!(" ({note})"));
	}
	out
}

fn ck(name: &str, ok: bool, measured: Value, limit: Value) -> Check {
	Check { name: name.to_string(), ok, measured, limit, note: None, details: Vec::new() }
}

fn unreadable(which: &str) -> Check {
	Check {
		name: which.to_string(),
		ok: false,
		measured: json!("unreadable"),
		limit: json!("readable"),
		note: None,
		details: Vec::new(),
	}
}

fn value_to_text(v: &Value) -> String {
	match v {
		Value::String(s) => s.clone(),
		other => other.to_string(),
	}
}

fn read_bytes(path: &Path) -> Option<Vec<u8>> {
	std::fs::read(path).ok()
}

/// `wc -l` semantics: count newlines, so a missing trailing newline doesn't
/// silently move a file across its bound.
fn newline_count(bytes: &[u8]) -> i64 {
	bytes.iter().filter(|&&b| b == b'\n').count() as i64
}

/// (tokens, Some(source-note) when the bytes/4 estimate was used). `bin/pi
/// token @file` prints `o200k: <n>` — the line the bash script greps with
/// `/^o200k:/{print $2}`. Absent/failed/unparseable → the estimate, annotated
/// with the script's own wording.
fn token_count(root: &Path, file: &Path) -> (i64, Option<String>) {
	let bytes = std::fs::read(file).map(|b| b.len() as i64).unwrap_or(0);
	let pi = root.join("bin").join("pi");
	if !pi.exists() {
		return (bytes / 4, Some("bin/pi absent — bytes/4 estimate".into()));
	}
	if let Ok(out) = Command::new(&pi)
		.arg("token")
		.arg(format!("@{}", file.display()))
		.stdin(Stdio::null())
		.output()
		&& out.status.success()
	{
		for line in String::from_utf8_lossy(&out.stdout).lines() {
			if let Some(rest) = line.strip_prefix("o200k:")
				&& let Some(n) = rest.split_whitespace().next().and_then(|t| t.parse::<i64>().ok())
			{
				return (n, None);
			}
		}
	}
	(bytes / 4, Some("bin/pi token failed — bytes/4 estimate".into()))
}

/// The `(name.md)` links inside an index — the script's
/// `grep -o '([A-Za-z0-9._-]*\.md)'`, hand-rolled (no regex dep): a `(`, a run
/// of `[A-Za-z0-9._-]`, ending `.md`, then `)`. Sorted + deduped like the
/// script's `sort -u`.
fn md_link_names(index: &str) -> Vec<String> {
	let b = index.as_bytes();
	let mut found = BTreeSet::new();
	let mut i = 0;
	while i < b.len() {
		if b[i] != b'(' {
			i += 1;
			continue;
		}
		let mut j = i + 1;
		while j < b.len() && (b[j].is_ascii_alphanumeric() || matches!(b[j], b'.' | b'_' | b'-')) {
			j += 1;
		}
		// run must be non-empty, end in ".md", and close right here
		if j < b.len() && b[j] == b')' && j >= i + 4 && &b[j - 3..j] == b".md" {
			found.insert(String::from_utf8_lossy(&b[i + 1..j]).into_owned());
		}
		i = j.max(i + 1);
	}
	found.into_iter().collect()
}

/// Split a facts.md line into (claim, command). Format:
/// `- [YYYY-MM-DD] <claim> — check: <cmd>` — exactly the script's guard:
/// the ` — check: ` separator, a `- [` prefix, and a real date at claim[3..13].
/// Undated lines and non-`- [` claims are skipped, never run.
fn split_fact(line: &str) -> Option<(&str, &str)> {
	let (claim, cmd) = line.split_once(" — check: ")?;
	if !dated_claim(claim) {
		return None;
	}
	Some((claim, cmd))
}

fn dated_claim(claim: &str) -> bool {
	let b = claim.as_bytes();
	b.len() >= 13
		&& b.starts_with(b"- [")
		&& b[3..7].iter().all(|c| c.is_ascii_digit())
		&& b[7] == b'-'
		&& b[8..10].iter().all(|c| c.is_ascii_digit())
		&& b[10] == b'-'
		&& b[11..13].iter().all(|c| c.is_ascii_digit())
}

/// Run one fact command from `cwd` with output suppressed (the script's
/// `>/dev/null 2>&1`), returning its exit code. `sh -c` on unix; `cmd /C` on
/// windows — fact commands are authored per-platform.
fn run_shell(cmd: &str, cwd: &Path) -> Option<i64> {
	let mut c = if cfg!(windows) {
		let mut c = Command::new("cmd");
		c.arg("/C");
		c
	} else {
		let mut c = Command::new("sh");
		c.arg("-c");
		c
	};
	c.arg(cmd).current_dir(cwd).output().ok().and_then(|o| o.status.code().map(i64::from))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn md_link_names_parses_paren_md_links_only() {
		let idx = "see [x](active-work.md) and (log.md); not (img.png), (a.markdown), (b .md) or [c](nested/deep.md); dup (log.md); bare (.md)";
		assert_eq!(md_link_names(idx), vec![".md", "active-work.md", "log.md"]);
	}

	#[test]
	fn split_fact_takes_only_dated_dash_lines() {
		let good = "- [2026-09-06] pi is pinned — check: test -f bin/pi";
		assert_eq!(split_fact(good), Some(("- [2026-09-06] pi is pinned", "test -f bin/pi")));
		// undated claim: skipped even though it carries a check command
		assert_eq!(split_fact("some claim — check: false"), None);
		// date in the wrong slot (not at claim[3..13]): skipped
		assert_eq!(split_fact("[2026-09-06] wrong slot — check: false"), None);
		// malformed date: skipped
		assert_eq!(split_fact("- [2026-9-6] short date — check: false"), None);
		// no separator at all
		assert_eq!(split_fact("- [2026-09-06] no command here"), None);
	}

	#[test]
	fn human_line_matches_the_ok_fail_measured_limit_shape() {
		let c = ck("index-lines", true, json!(31), json!(120));
		assert_eq!(line(&c), "ok index-lines 31 120");
		let bad = ck("active-work-lines", false, json!(512), json!(400));
		assert_eq!(line(&bad), "FAIL active-work-lines 512 400");
		let est = Check {
			note: Some("bin/pi absent — bytes/4 estimate".into()),
			..ck("active-work-tokens", true, json!(1360), json!(4000))
		};
		assert_eq!(line(&est), "ok active-work-tokens 1360 4000 (bin/pi absent — bytes/4 estimate)");
		let det = Check { details: vec!["UNINDEXED: stray".into()], ..ck("index-disk", false, json!(1), json!(0)) };
		assert_eq!(line(&det), "UNINDEXED: stray\nFAIL index-disk 1 0");
	}
}
