//! The worker tools. `read_file` is read-only (always allowed); `write_file`,
//! `edit_file`, and `bash` mutate (gated in Align). `edit_file` is the surgical,
//! anchor-by-exact-text editor — preferred over a full `write_file` overwrite
//! (which clobbers offloaded/elided regions) or `bash sed` line-arithmetic.
//! Deliberately tiny — enough to exercise the gate, no more.
//!
//! Path confinement (t4): the file tools resolve the model's path against the
//! worktree `cwd` and REJECT anything that escapes it — `cwd.join(abs)` discards
//! the base, so without this an absolute path writes anywhere on disk (the
//! dogfood-exercise sandbox escape). The *status* gate decides WHETHER a tool
//! runs; `confine` decides WHERE it may touch. `bash` is cwd-scoped but not a
//! hard sandbox (it can `cd`/use absolute paths) — see the note on its arm.

use std::path::{Component, Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use provider::ToolDef;
use serde_json::{Value, json};

/// Tool schemas advertised to the model.
pub fn tool_defs() -> Vec<ToolDef> {
	vec![
		ToolDef {
			name: "read_file".into(),
			description: "Read a UTF-8 text file and return its contents. Optional `offset` \
			              (1-based start line) and `limit` (max lines) read just a slice — use them to \
			              re-read a large offloaded output around a specific line."
				.into(),
			parameters: json!({
				"type": "object",
				"properties": {
					"path": { "type": "string", "description": "path relative to the worktree" },
					"offset": { "type": "integer", "description": "1-based first line to return (default 1)" },
					"limit": { "type": "integer", "description": "max lines to return (default: to end of file)" },
				},
				"required": ["path"],
			}),
		},
		ToolDef {
			name: "write_file".into(),
			description: "Write (create or overwrite) a UTF-8 text file.".into(),
			parameters: json!({
				"type": "object",
				"properties": {
					"path": { "type": "string" },
					"content": { "type": "string" },
				},
				"required": ["path", "content"],
			}),
		},
		ToolDef {
			name: "edit_file".into(),
			description: "Change part of an existing file by exact-text match. Replaces `old_string` \
			              with `new_string`. `old_string` must match the file byte-for-byte INCLUDING \
			              indentation and must be unique — copy it from a `read_file` of the region. \
			              Prefer this over rewriting a file with `write_file` (which can clobber code \
			              you cannot see in an offloaded/elided region) or computing line numbers for \
			              `bash sed`. Pass `replace_all: true` to replace every occurrence."
				.into(),
			parameters: json!({
				"type": "object",
				"properties": {
					"path": { "type": "string", "description": "path relative to the worktree" },
					"old_string": { "type": "string", "description": "exact text to find (incl. whitespace); must be unique unless replace_all" },
					"new_string": { "type": "string", "description": "text to replace it with" },
					"replace_all": { "type": "boolean", "description": "replace every occurrence (default false)" },
				},
				"required": ["path", "old_string", "new_string"],
			}),
		},
		ToolDef {
			name: "bash".into(),
			description: "Run a bash command and return combined stdout+stderr.".into(),
			parameters: json!({
				"type": "object",
				"properties": { "command": { "type": "string" } },
				"required": ["command"],
			}),
		},
	]
}

/// Execute a tool by name. `args` is the parsed JSON-object the model emitted.
/// Returns the string fed back to the model as the tool result.
pub fn execute(name: &str, args: &Value, cwd: &std::path::Path) -> Result<String> {
	match name {
		"read_file" => {
			let path = arg_str(args, "path")?;
			let full = confine(cwd, &path)?;
			let body = std::fs::read_to_string(&full)
				.with_context(|| format!("read_file {}", full.display()))?;
			// Optional line-window: `offset` (1-based) / `limit`. Absent → whole file,
			// byte-identical to a plain read (the F3 re-read hint relies on this to fetch
			// a slice of a large offloaded artifact without re-injecting the whole thing).
			let offset = args.get("offset").and_then(Value::as_u64);
			let limit = args.get("limit").and_then(Value::as_u64);
			if offset.is_none() && limit.is_none() {
				return Ok(body);
			}
			Ok(slice_lines(&body, offset, limit))
		}
		"write_file" => {
			let path = arg_str(args, "path")?;
			let content = arg_str(args, "content")?;
			let full = confine(cwd, &path)?;
			if let Some(parent) = full.parent() {
				std::fs::create_dir_all(parent).ok();
			}
			std::fs::write(&full, content.as_bytes())
				.with_context(|| format!("write_file {}", full.display()))?;
			Ok(format!("wrote {} bytes to {}", content.len(), full.display()))
		}
		"edit_file" => {
			let path = arg_str(args, "path")?;
			let old = arg_str(args, "old_string")?;
			let new = arg_str(args, "new_string")?;
			let replace_all = args.get("replace_all").and_then(Value::as_bool).unwrap_or(false);
			let full = confine(cwd, &path)?;
			// edit_file changes EXISTING files only — creating is write_file's job.
			let body = std::fs::read_to_string(&full)
				.with_context(|| format!("edit_file {} (does the file exist?)", full.display()))?;
			if old.is_empty() {
				bail!("edit_file: `old_string` is empty — give the exact text to replace");
			}
			if old == new {
				bail!("edit_file: `old_string` equals `new_string` — no change requested");
			}
			let count = body.matches(old.as_str()).count();
			if count == 0 {
				bail!("{}", no_match_hint(&body, &old, &path));
			}
			if count > 1 && !replace_all {
				bail!(
					"edit_file: `old_string` is ambiguous — {count} matches in `{path}`. Add \
					 surrounding lines to make it unique, or pass `replace_all: true`."
				);
			}
			let edited = if replace_all {
				body.replace(old.as_str(), &new)
			} else {
				body.replacen(old.as_str(), &new, 1)
			};
			std::fs::write(&full, edited.as_bytes())
				.with_context(|| format!("edit_file {}", full.display()))?;
			Ok(format!("edit_file: replaced {count} occurrence(s) in {path}"))
		}
		"bash" => {
			let command = arg_str(args, "command")?;
			// NOTE (t4): bash runs with cwd = the worktree but is NOT a hard
			// sandbox — a command can still `cd` or use absolute paths to escape.
			// The file tools are confined (see `confine`); fully confining bash
			// needs OS-level isolation (container/chroot) — deferred.
			let out = Command::new("bash")
				.arg("-c")
				.arg(&command)
				.current_dir(cwd)
				.output()
				.with_context(|| format!("bash: {command}"))?;
			let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
			s.push_str(&String::from_utf8_lossy(&out.stderr));
			if !out.status.success() {
				s.push_str(&format!("\n[exit: {}]", out.status.code().unwrap_or(-1)));
			}
			Ok(s)
		}
		other => anyhow::bail!("unknown tool: {other}"),
	}
}

fn arg_str(args: &Value, key: &str) -> Result<String> {
	args.get(key)
		.and_then(Value::as_str)
		.map(ToOwned::to_owned)
		.with_context(|| format!("missing string arg `{key}`"))
}

/// Diagnose an `edit_file` zero-match: a bare "not found" sends the model back to
/// guess the same way. The two failures that dominate for an LLM are (1) wrong
/// indentation/whitespace and (2) invisible CRLF line endings — both render
/// identically, so we probe for them and name the cause instead.
fn no_match_hint(body: &str, old: &str, path: &str) -> String {
	let mut hint = format!("edit_file: `old_string` not found in `{path}`");
	// (1) whitespace/indentation: the trimmed text exists, so only surrounding
	// whitespace differs — the single most common LLM edit miss.
	let trimmed = old.trim();
	if !trimmed.is_empty() && trimmed != old && body.contains(trimmed) {
		hint.push_str(
			" — but its whitespace-trimmed form IS present, so your indentation or surrounding \
			 blank space is off. Re-read the region with `read_file` and copy the line(s) exactly.",
		);
		return hint;
	}
	// (2) CRLF on disk vs LF in old_string: invisible, undebuggable otherwise.
	if body.contains('\r') && !old.contains('\r') {
		hint.push_str(
			" — the file has CRLF (\\r\\n) line endings but `old_string` uses LF, so they don't \
			 match. Match a single line with no newline, or include the \\r.",
		);
		return hint;
	}
	hint.push_str(
		". Copy the exact text (including indentation) from a `read_file` of that region — \
		 do not retype it from memory.",
	);
	hint
}

/// Return the lines `[offset, offset+limit)` of `body` (offset 1-based, both
/// optional), prefixed with a `[lines X-Y of Z]` marker so a sliced read never
/// masquerades as the whole file. Out-of-range offset yields just the marker. The
/// targeted-re-read primitive behind F3's offloaded-output hint (research/25 §4.2).
fn slice_lines(body: &str, offset: Option<u64>, limit: Option<u64>) -> String {
	let lines: Vec<&str> = body.lines().collect();
	let total = lines.len();
	let start = offset.unwrap_or(1).max(1) as usize - 1; // to 0-based
	let end = match limit {
		Some(n) => start.saturating_add(n as usize).min(total),
		None => total,
	};
	if start >= total {
		return format!("[lines {}-{} of {total}: out of range — file has {total} lines]", start + 1, end);
	}
	let shown = lines[start..end].join("\n");
	format!("[lines {}-{} of {total}]\n{shown}", start + 1, end)
}

/// Resolve a model-supplied `path` against the worktree `cwd`, rejecting any
/// path that escapes it (t4). Absolute paths are refused outright; relative paths
/// are joined under the canonical `cwd` and lexically normalized, then required to
/// stay within it — so `../` traversal that climbs above the worktree is caught.
/// Normalization is lexical (not `fs::canonicalize`) so a not-yet-existing target
/// (write_file creating a new file) resolves the same as an existing one.
fn confine(cwd: &Path, path: &str) -> Result<PathBuf> {
	let p = Path::new(path);
	if p.is_absolute() {
		bail!("path `{path}` is absolute — tools may only touch files inside the worktree");
	}
	// canonicalize the (existing) worktree root so the prefix check is symlink-safe.
	let base = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
	let full = normalize_lexical(&base.join(p));
	if !full.starts_with(&base) {
		bail!("path `{path}` escapes the worktree sandbox");
	}
	Ok(full)
}

/// Lexically resolve `.`/`..` without touching the filesystem. `..` pops the
/// previous component (and cannot climb above what's accumulated), so joined with
/// an absolute base this yields an absolute, normalized path.
fn normalize_lexical(p: &Path) -> PathBuf {
	let mut out = PathBuf::new();
	for comp in p.components() {
		match comp {
			Component::ParentDir => {
				out.pop();
			}
			Component::CurDir => {}
			other => out.push(other.as_os_str()),
		}
	}
	out
}

#[cfg(test)]
mod tests {
	use super::*;

	fn tmp(name: &str) -> PathBuf {
		let d = std::env::temp_dir().join(name);
		let _ = std::fs::remove_dir_all(&d);
		std::fs::create_dir_all(&d).unwrap();
		d
	}

	// confine: in-worktree relative paths resolve; absolute and ../-escapes are refused.
	#[test]
	fn confine_allows_inworktree_rejects_escape() {
		let cwd = tmp("harness-confine-unit");
		assert!(confine(&cwd, "a.txt").is_ok());
		assert!(confine(&cwd, "sub/dir/b.txt").is_ok());
		assert!(confine(&cwd, "x/../c.txt").is_ok(), "interior .. that stays inside is fine");
		assert!(confine(&cwd, "/etc/passwd").is_err(), "absolute rejected");
		assert!(confine(&cwd, "../escape.txt").is_err(), "parent traversal rejected");
		assert!(confine(&cwd, "../../etc/passwd").is_err(), "deep traversal rejected");
		let _ = std::fs::remove_dir_all(&cwd);
	}

	// write_file lands inside the worktree but refuses to write outside it — the
	// regression guard for the dogfood sandbox escape (nothing written elsewhere).
	#[test]
	fn write_file_confined_to_worktree() {
		let cwd = tmp("harness-confine-write");
		let ok = execute("write_file", &json!({"path": "note.txt", "content": "hi"}), &cwd).unwrap();
		assert!(ok.contains("wrote"));
		assert!(cwd.join("note.txt").exists());

		// absolute path → refused, and nothing is written outside the worktree
		let evil = std::env::temp_dir().join("harness-confine-EVIL.txt");
		let _ = std::fs::remove_file(&evil);
		let r = execute(
			"write_file",
			&json!({"path": evil.to_string_lossy().to_string(), "content": "pwned"}),
			&cwd,
		);
		assert!(r.is_err(), "absolute write must be refused");
		assert!(!evil.exists(), "no file written outside the worktree");

		// ../ escape → refused
		let parent_file = cwd.parent().unwrap().join("harness-confine-ESCAPE.txt");
		let _ = std::fs::remove_file(&parent_file);
		let r = execute(
			"write_file",
			&json!({"path": "../harness-confine-ESCAPE.txt", "content": "x"}),
			&cwd,
		);
		assert!(r.is_err(), "../ write must be refused");
		assert!(!parent_file.exists(), "no file written above the worktree");

		let _ = std::fs::remove_dir_all(&cwd);
	}

	// read_file is confined too: an in-worktree read works, an absolute read of a
	// real outside file is refused (no exfil of arbitrary host files into the loop).
	#[test]
	fn read_file_confined_to_worktree() {
		let cwd = tmp("harness-confine-read");
		std::fs::write(cwd.join("inside.txt"), "mine").unwrap();
		assert_eq!(execute("read_file", &json!({"path": "inside.txt"}), &cwd).unwrap(), "mine");
		assert!(
			execute("read_file", &json!({"path": "/etc/hosts"}), &cwd).is_err(),
			"absolute read must be refused"
		);
		let _ = std::fs::remove_dir_all(&cwd);
	}

	// read_file pagination (F3 re-read primitive): no offset/limit → whole file,
	// byte-identical (regression); a window returns just those lines with a marker;
	// an out-of-range offset is reported, not silently empty.
	#[test]
	fn read_file_offset_limit_windows() {
		let cwd = tmp("harness-read-window");
		std::fs::write(cwd.join("big.txt"), "l1\nl2\nl3\nl4\nl5\n").unwrap();

		// whole-file read is unchanged by the new params being absent
		assert_eq!(
			execute("read_file", &json!({"path": "big.txt"}), &cwd).unwrap(),
			"l1\nl2\nl3\nl4\nl5\n"
		);

		// a window: lines 2..=3
		let w = execute("read_file", &json!({"path": "big.txt", "offset": 2, "limit": 2}), &cwd).unwrap();
		assert_eq!(w, "[lines 2-3 of 5]\nl2\nl3");

		// limit only → from line 1
		let h = execute("read_file", &json!({"path": "big.txt", "limit": 2}), &cwd).unwrap();
		assert_eq!(h, "[lines 1-2 of 5]\nl1\nl2");

		// offset past EOF → marker, no panic
		let oob = execute("read_file", &json!({"path": "big.txt", "offset": 99}), &cwd).unwrap();
		assert!(oob.contains("out of range"), "got: {oob}");

		let _ = std::fs::remove_dir_all(&cwd);
	}

	// edit_file replaces a unique, possibly multi-line anchor in place and leaves
	// the rest of the file byte-identical (the is_prime-style surgical fix that the
	// 35B kept fumbling with sed line-arithmetic).
	#[test]
	fn edit_file_replaces_unique_multiline_match() {
		let cwd = tmp("harness-edit-unique");
		let src = "def is_prime(n):\n    for d in range(2, n):\n        if n % d == 0:\n            return False\n    return True\n";
		std::fs::write(cwd.join("library.py"), src).unwrap();

		let r = execute(
			"edit_file",
			&json!({
				"path": "library.py",
				"old_string": "def is_prime(n):\n    for d in range(2, n):",
				"new_string": "def is_prime(n):\n    if n < 2:\n        return False\n    for d in range(2, n):",
			}),
			&cwd,
		)
		.unwrap();
		assert!(r.contains("replaced 1 occurrence"), "got: {r}");

		let after = std::fs::read_to_string(cwd.join("library.py")).unwrap();
		assert!(after.contains("    if n < 2:\n        return False\n    for d in range"));
		assert!(after.ends_with("    return True\n"), "tail untouched: {after:?}");
		let _ = std::fs::remove_dir_all(&cwd);
	}

	// zero match → actionable error, file untouched.
	#[test]
	fn edit_file_rejects_zero_match() {
		let cwd = tmp("harness-edit-zero");
		std::fs::write(cwd.join("a.txt"), "hello world\n").unwrap();
		let r = execute(
			"edit_file",
			&json!({"path": "a.txt", "old_string": "goodbye", "new_string": "hi"}),
			&cwd,
		);
		let e = format!("{:#}", r.unwrap_err());
		assert!(e.contains("not found"), "got: {e}");
		assert_eq!(std::fs::read_to_string(cwd.join("a.txt")).unwrap(), "hello world\n");
		let _ = std::fs::remove_dir_all(&cwd);
	}

	// whitespace miss: trimmed form present → the hint names the cause (highest-
	// leverage diagnostic for an LLM editor).
	#[test]
	fn edit_file_zero_match_hints_whitespace() {
		let cwd = tmp("harness-edit-ws");
		std::fs::write(cwd.join("a.py"), "x = 1\n").unwrap();
		// model over-indented: "    x = 1" is not a substring (file has no leading
		// space), but its trimmed form "x = 1" is — the indentation-guess miss.
		let r = execute(
			"edit_file",
			&json!({"path": "a.py", "old_string": "    x = 1", "new_string": "    x = 2"}),
			&cwd,
		);
		let e = format!("{:#}", r.unwrap_err());
		assert!(e.contains("whitespace-trimmed form IS present"), "got: {e}");
		let _ = std::fs::remove_dir_all(&cwd);
	}

	// CRLF on disk vs LF in old_string → the hint names CRLF (otherwise invisible).
	#[test]
	fn edit_file_zero_match_hints_crlf() {
		let cwd = tmp("harness-edit-crlf");
		std::fs::write(cwd.join("a.txt"), "alpha\r\nbeta\r\n").unwrap();
		let r = execute(
			"edit_file",
			&json!({"path": "a.txt", "old_string": "alpha\nbeta", "new_string": "x"}),
			&cwd,
		);
		let e = format!("{:#}", r.unwrap_err());
		assert!(e.contains("CRLF"), "got: {e}");
		let _ = std::fs::remove_dir_all(&cwd);
	}

	// ambiguous (overlapping-aware) without replace_all → refused; with it → all
	// replaced, and count matches str::replace's non-overlapping semantics.
	#[test]
	fn edit_file_ambiguous_needs_replace_all() {
		let cwd = tmp("harness-edit-ambig");
		std::fs::write(cwd.join("a.txt"), "aaaa").unwrap(); // "aa" matches twice (non-overlapping)

		let r = execute(
			"edit_file",
			&json!({"path": "a.txt", "old_string": "aa", "new_string": "b"}),
			&cwd,
		);
		let e = format!("{:#}", r.unwrap_err());
		assert!(e.contains("ambiguous") && e.contains("2 matches"), "got: {e}");
		assert_eq!(std::fs::read_to_string(cwd.join("a.txt")).unwrap(), "aaaa", "untouched on refusal");

		let ok = execute(
			"edit_file",
			&json!({"path": "a.txt", "old_string": "aa", "new_string": "b", "replace_all": true}),
			&cwd,
		)
		.unwrap();
		assert!(ok.contains("replaced 2 occurrence"), "got: {ok}");
		assert_eq!(std::fs::read_to_string(cwd.join("a.txt")).unwrap(), "bb");
		let _ = std::fs::remove_dir_all(&cwd);
	}

	// empty old_string and no-op (old == new) are refused before any write.
	#[test]
	fn edit_file_rejects_empty_and_noop() {
		let cwd = tmp("harness-edit-bad-args");
		std::fs::write(cwd.join("a.txt"), "data\n").unwrap();

		let empty = execute(
			"edit_file",
			&json!({"path": "a.txt", "old_string": "", "new_string": "x"}),
			&cwd,
		);
		assert!(format!("{:#}", empty.unwrap_err()).contains("empty"));

		let noop = execute(
			"edit_file",
			&json!({"path": "a.txt", "old_string": "data", "new_string": "data"}),
			&cwd,
		);
		assert!(format!("{:#}", noop.unwrap_err()).contains("no change requested"));

		assert_eq!(std::fs::read_to_string(cwd.join("a.txt")).unwrap(), "data\n");
		let _ = std::fs::remove_dir_all(&cwd);
	}

	// edit_file is confined like the other file tools: absolute path and ../-escape
	// are refused, and nothing is written outside the worktree.
	#[test]
	fn edit_file_confined_to_worktree() {
		let cwd = tmp("harness-edit-confine");
		let evil = std::env::temp_dir().join("harness-edit-EVIL.txt");
		std::fs::write(&evil, "secret\n").unwrap();
		let r = execute(
			"edit_file",
			&json!({
				"path": evil.to_string_lossy().to_string(),
				"old_string": "secret",
				"new_string": "pwned",
			}),
			&cwd,
		);
		assert!(r.is_err(), "absolute edit must be refused");
		assert_eq!(std::fs::read_to_string(&evil).unwrap(), "secret\n", "outside file untouched");
		let _ = std::fs::remove_file(&evil);
		let _ = std::fs::remove_dir_all(&cwd);
	}

	// editing a nonexistent (but in-worktree) file errors cleanly — edit_file does
	// not create files; that's write_file's job.
	#[test]
	fn edit_file_missing_file_errors() {
		let cwd = tmp("harness-edit-missing");
		let r = execute(
			"edit_file",
			&json!({"path": "nope.txt", "old_string": "a", "new_string": "b"}),
			&cwd,
		);
		assert!(r.is_err(), "editing a missing file must error, not create it");
		assert!(!cwd.join("nope.txt").exists(), "no file created");
		let _ = std::fs::remove_dir_all(&cwd);
	}
}
