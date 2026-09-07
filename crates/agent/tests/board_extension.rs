//! The pi board extension verbs — `gate`, `close-check`, `wiki check` — the
//! machine-facing surface `bin/board` drives on macOS/Linux/Windows without
//! bash or the sqlite3 CLI. Spawned through the built binary (same pattern as
//! cli_dispatch.rs) because these pin main()'s dispatch glue: exit codes, the
//! text/JSON output contracts, and the gate rows they leave behind.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

/// Fresh empty dir per case, namespaced by pid + test name (cli_dispatch.rs's
/// pattern) so parallel test binaries never collide.
fn fresh_dir(name: &str) -> PathBuf {
	let dir = std::env::temp_dir().join(format!("agent-board-ext-{}-{name}", std::process::id()));
	let _ = std::fs::remove_dir_all(&dir);
	std::fs::create_dir_all(&dir).expect("create fresh temp dir");
	dir
}

fn run_agent(cwd: &Path, args: &[&str], harness_db: &Path) -> Output {
	Command::new(env!("CARGO_BIN_EXE_agent"))
		.args(args)
		.current_dir(cwd)
		.env("HARNESS_DB", harness_db)
		.output()
		.expect("spawn agent binary")
}

fn stdout(out: &Output) -> String {
	String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
	String::from_utf8_lossy(&out.stderr).into_owned()
}

fn json_out(out: &Output) -> Value {
	serde_json::from_str(&stdout(out)).expect("stdout is one JSON object")
}

/// Mint a ticket and return its id (the first token of `agent new`'s line).
fn seed_ticket(dir: &Path, db: &Path, title: &str) -> String {
	let out = run_agent(dir, &["new", title, "--kind", "question"], db);
	assert!(out.status.success(), "new must succeed: {}", stderr(&out));
	stdout(&out).split_whitespace().next().expect("new prints the minted id").to_string()
}

/// A minimal compliant wiki fixture. No bin/pi under the root, so the token
/// bound exercises the bytes/4 estimator — deterministic, no child process.
fn compliant_wiki(root: &Path) {
	let wiki = root.join("wiki");
	std::fs::create_dir_all(&wiki).unwrap();
	std::fs::write(wiki.join("active-work.md"), "# Active work\n\n- one thing, small.\n").unwrap();
	std::fs::write(wiki.join("log.md"), "# log\n- an entry\n").unwrap();
	std::fs::write(
		wiki.join("index.md"),
		"# Index\n\n- [active](active-work.md) current work\n- [log](log.md)\n- [facts](facts.md)\n",
	)
	.unwrap();
	// one dated fact (runs), one undated and one malformed-date line (skipped —
	// `false` would fail the whole report if the date guard let them through)
	std::fs::write(
		wiki.join("facts.md"),
		"- [2026-09-06] the log exists — check: test -f wiki/log.md\n\
		 not a fact — check: false\n\
		 - [notadate] wrong slot — check: false\n",
	)
	.unwrap();
}

#[test]
fn gate_records_board_machine_rows_and_honors_the_json_contract() {
	let dir = fresh_dir("gate");
	let db = dir.join("board.db");
	let id = seed_ticket(&dir, &db, "gate row test");

	// pass with a note — human line names id, gate, verdict
	let out = run_agent(&dir, &["gate", &id, "wiki-close", "pass", "--note", "log updated"], &db);
	assert!(out.status.success(), "gate pass must succeed: {}", stderr(&out));
	assert_eq!(stdout(&out), format!("{id}  gate wiki-close=pass recorded\n"));

	// fail with --json — the machine contract: exactly id/gate/passed/attempt
	let out = run_agent(&dir, &["gate", &id, "wiki-close", "fail", "--json", "--note", "not today"], &db);
	assert!(out.status.success(), "gate fail must succeed (a red is a recorded result): {}", stderr(&out));
	let v = json_out(&out);
	let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
	keys.sort_unstable();
	assert_eq!(keys, vec!["attempt", "gate", "id", "passed"], "exactly the four keys");
	assert_eq!(v["id"].as_str(), Some(id.as_str()));
	assert_eq!(v["gate"].as_str(), Some("wiki-close"));
	assert_eq!(v["passed"].as_bool(), Some(false));
	assert_eq!(v["attempt"].as_i64(), Some(0));

	// append-only history: both rows survive in show --json as provider=board,
	// source=machine — a red survives the green re-run that fixed it (t15).
	let out = run_agent(&dir, &["show", &id, "--json"], &db);
	let v = json_out(&out);
	let gates = v["gates"].as_array().expect("gates is a list");
	assert_eq!(gates.len(), 2, "both reports must survive");
	assert_eq!(gates[0]["passed"].as_bool(), Some(true));
	assert_eq!(gates[0]["provider"].as_str(), Some("board"));
	assert_eq!(gates[0]["source"].as_str(), Some("machine"));
	assert_eq!(gates[0]["note"].as_str(), Some("log updated"));
	assert_eq!(gates[1]["passed"].as_bool(), Some(false));
	assert_eq!(gates[1]["note"].as_str(), Some("not today"));
}

#[test]
fn gate_refuses_a_missing_ticket_with_exit_2() {
	let dir = fresh_dir("gate-missing");
	let db = dir.join("board.db");
	let out = run_agent(&dir, &["gate", "nosuch", "wiki-close", "pass"], &db);
	assert_eq!(out.status.code(), Some(2), "the shim refuses with exit 2");
	assert!(stderr(&out).contains("nosuch"), "the message must name the missing id: {}", stderr(&out));
}

#[test]
fn gate_refuses_a_verdict_that_is_not_pass_or_fail() {
	let dir = fresh_dir("gate-verdict");
	let db = dir.join("board.db");
	let id = seed_ticket(&dir, &db, "verdict test");
	let out = run_agent(&dir, &["gate", &id, "wiki-close", "sideways"], &db);
	assert!(!out.status.success(), "a bad verdict must refuse");
	assert!(stderr(&out).contains("pass|fail"), "the refusal must say why: {}", stderr(&out));
	// and nothing was recorded
	let out = run_agent(&dir, &["show", &id, "--json"], &db);
	let v = json_out(&out);
	assert_eq!(v["gates"].as_array().map(|a| a.len()), Some(0), "no gate row for a refused verdict");
}

#[test]
fn close_check_demands_todays_wiki_close_pass_on_every_open_ticket() {
	let dir = fresh_dir("close-check");
	let db = dir.join("board.db");
	let id = seed_ticket(&dir, &db, "close check test");
	let id2 = seed_ticket(&dir, &db, "second open ticket");

	// red board: no wiki-close gate today — FAIL naming the ids, exit 1
	let out = run_agent(&dir, &["close-check"], &db);
	assert_eq!(out.status.code(), Some(1));
	assert!(stdout(&out).contains(&id) && stdout(&out).contains(&id2), "{}", stdout(&out));

	// --json: {"ok":false,"missing":[...both ids...]}
	let out = run_agent(&dir, &["close-check", "--json"], &db);
	assert_eq!(out.status.code(), Some(1));
	let v = json_out(&out);
	assert_eq!(v["ok"].as_bool(), Some(false));
	let missing = v["missing"].as_array().expect("missing is a list");
	assert_eq!(missing.len(), 2);
	assert!(missing.iter().any(|m| m.as_str() == Some(id.as_str())));

	// a FAIL gate does not satisfy — only a pass does
	let out = run_agent(&dir, &["gate", &id, "wiki-close", "fail"], &db);
	assert!(out.status.success(), "recording the red must succeed: {}", stderr(&out));
	let out = run_agent(&dir, &["close-check", "--json"], &db);
	assert_eq!(out.status.code(), Some(1), "a red wiki-close gate is not a pass");

	// pass on one ticket → still failing on the other open ticket
	let out = run_agent(&dir, &["gate", &id, "wiki-close", "pass"], &db);
	assert!(out.status.success(), "{}", stderr(&out));
	let out = run_agent(&dir, &["close-check", "--json"], &db);
	assert_eq!(out.status.code(), Some(1), "the second ticket still lacks its pass");
	let v = json_out(&out);
	assert_eq!(v["missing"].as_array().map(|a| a.len()), Some(1));
	assert_eq!(v["missing"][0].as_str(), Some(id2.as_str()));

	// pass on the last open ticket → OK, exit 0, empty missing list
	let out = run_agent(&dir, &["gate", &id2, "wiki-close", "pass"], &db);
	assert!(out.status.success(), "{}", stderr(&out));
	let out = run_agent(&dir, &["close-check", "--json"], &db);
	assert!(out.status.success(), "close-check must pass now");
	let v = json_out(&out);
	assert_eq!(v["ok"].as_bool(), Some(true));
	assert_eq!(v["missing"].as_array().map(|a| a.len()), Some(0));
	let out = run_agent(&dir, &["close-check"], &db);
	assert!(out.status.success());
	assert_eq!(stdout(&out), "close-check OK\n");
}

#[test]
fn wiki_check_passes_a_compliant_fixture_and_skips_undated_fact_lines() {
	let dir = fresh_dir("wiki-ok");
	compliant_wiki(&dir);
	let root = dir.to_str().unwrap().to_string();

	let out = run_agent(&dir, &["wiki", "check", "--root", &root, "--json"], &dir.join("board.db"));
	assert!(out.status.success(), "compliant fixture must pass: {}{}", stdout(&out), stderr(&out));
	let v = json_out(&out);
	assert_eq!(v["ok"].as_bool(), Some(true));
	let checks = v["checks"].as_array().expect("checks is a list");
	assert!(
		checks.len() >= 8,
		"3 file-presence + 3 active-work + index + log + index-disk + facts"
	);
	for c in checks {
		let mut keys: Vec<&str> = c.as_object().expect("check is an object").keys().map(String::as_str).collect();
		keys.sort_unstable();
		assert_eq!(keys, vec!["limit", "measured", "name", "ok"], "exactly the four keys per check");
		assert_eq!(c["ok"].as_bool(), Some(true), "check {} must pass: {c}", c["name"]);
	}
	// exactly the dated fact ran — the undated/malformed `false` lines were
	// skipped, else the report would be red
	let fact_names: Vec<&str> =
		checks.iter().filter_map(|c| c["name"].as_str()).filter(|n| n.starts_with("fact ")).collect();
	assert_eq!(fact_names.len(), 1, "only the dated fact line runs: {fact_names:?}");
	assert!(fact_names[0].contains("the log exists"));

	// human mode: per-check lines then the summary line, still exit 0
	let out = run_agent(&dir, &["wiki", "check", "--root", &root], &dir.join("board.db"));
	let text = stdout(&out);
	assert!(out.status.success(), "human mode must pass too: {text}");
	assert!(text.contains("ok active-work-lines 3 400"), "per-check line shape: {text}");
	assert!(text.contains("bin/pi absent — bytes/4 estimate"), "the estimator must say so: {text}");
	assert_eq!(text.lines().last(), Some("HOUSEKEEPING-PASS"));
}

#[test]
fn wiki_check_fails_loud_on_oversized_active_work_and_red_facts() {
	let dir = fresh_dir("wiki-bad");
	let wiki = dir.join("wiki");
	std::fs::create_dir_all(&wiki).unwrap();
	// 500 lines × ~49 bytes ≈ 24.5 KB → over the 400-line and 4000-token
	// (bytes/4) bounds; bytes land just over the 24000 bound too.
	let big: String =
		(0..500).map(|i| format!("line {i:03} - active work filler padding bytes here\n")).collect();
	std::fs::write(wiki.join("active-work.md"), big).unwrap();
	std::fs::write(wiki.join("log.md"), "# log\n").unwrap();
	std::fs::write(
		wiki.join("index.md"),
		"# Index\n\n- [active](active-work.md)\n- [log](log.md)\n- [facts](facts.md)\n",
	)
	.unwrap();
	std::fs::write(wiki.join("extra.md"), "unindexed stray\n").unwrap();
	std::fs::write(wiki.join("facts.md"), "- [2026-09-06] the log is gone — check: false\n").unwrap();
	let root = dir.to_str().unwrap().to_string();

	// --json: ok:false, exit 1, and each failure is its own named check
	let out = run_agent(&dir, &["wiki", "check", "--root", &root, "--json"], &dir.join("board.db"));
	assert_eq!(out.status.code(), Some(1), "the same pass/fail exit code as the script");
	let v = json_out(&out);
	assert_eq!(v["ok"].as_bool(), Some(false));
	let by_name = |name: &str| {
		v["checks"]
			.as_array()
			.unwrap()
			.iter()
			.find(|c| c["name"].as_str() == Some(name))
			.unwrap_or_else(|| panic!("check {name} missing from: {}", v["checks"]))
			.clone()
	};
	assert_eq!(by_name("active-work-lines")["ok"].as_bool(), Some(false), "500 > 400");
	assert_eq!(by_name("active-work-lines")["measured"].as_i64(), Some(500));
	assert_eq!(by_name("active-work-tokens")["ok"].as_bool(), Some(false), "bytes/4 ≈ 6100 > 4000");
	assert!(by_name("active-work-tokens")["measured"].is_number());
	assert_eq!(by_name("index-disk")["ok"].as_bool(), Some(false), "extra.md is unindexed");
	assert_eq!(by_name("index-disk")["measured"].as_i64(), Some(1));
	let fact = v["checks"].as_array().unwrap().iter().find(|c| c["name"].as_str().map(|n| n.contains("the log is gone")).unwrap_or(false)).expect("the failing fact is a check");
	assert_eq!(fact["ok"].as_bool(), Some(false), "`false` exits 1");
	// a compliant check inside a failing report stays green
	assert_eq!(by_name("log-lines")["ok"].as_bool(), Some(true));

	// human mode: FAIL lines, violation detail, HOUSEKEEPING-FAIL last, exit 1
	let out = run_agent(&dir, &["wiki", "check", "--root", &root], &dir.join("board.db"));
	assert_eq!(out.status.code(), Some(1));
	let text = stdout(&out);
	assert!(text.contains("FAIL active-work-lines 500 400"), "{text}");
	assert!(text.contains("UNINDEXED: extra"), "per-file violation detail: {text}");
	assert_eq!(text.lines().last(), Some("HOUSEKEEPING-FAIL"));
}

#[test]
fn wiki_check_fails_when_the_wiki_dir_is_missing() {
	let dir = fresh_dir("wiki-none");
	let root = dir.to_str().unwrap().to_string();
	let out = run_agent(&dir, &["wiki", "check", "--root", &root, "--json"], &dir.join("board.db"));
	assert_eq!(out.status.code(), Some(1));
	let v = json_out(&out);
	assert_eq!(v["ok"].as_bool(), Some(false));
	assert_eq!(v["checks"].as_array().map(|a| a.len()), Some(1), "one wiki-dir check, no fake reads");
	assert_eq!(v["checks"][0]["name"].as_str(), Some("wiki-dir"));
}
