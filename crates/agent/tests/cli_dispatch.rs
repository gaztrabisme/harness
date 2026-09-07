//! Spawns the built `agent` binary to pin main()'s dispatch glue, which unit tests
//! can't reach: the pre-open usage gate (no/unknown verb → exit 2, usage on stderr,
//! and NO harness-board.db{,-shm,-wal} side-files created in cwd) and the fact that
//! a known verb actually reaches its dispatch arm instead of the usage path.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const USAGE_SNIPPET: &str = "usage: agent <new|draft";

/// Fresh empty cwd per case, namespaced by pid + test name so parallel test
/// binaries and repeated runs never collide.
fn fresh_dir(name: &str) -> PathBuf {
	let dir = std::env::temp_dir().join(format!("agent-cli-dispatch-{}-{name}", std::process::id()));
	// Stale leftovers from a previous run of the same pid-namespace are removed
	// so the "no files created" assertions start from a truly empty dir.
	let _ = std::fs::remove_dir_all(&dir);
	std::fs::create_dir_all(&dir).expect("create fresh temp cwd");
	dir
}

/// Run the agent binary in `cwd` with `args`. HARNESS_DB is stripped from the
/// environment unless explicitly provided, so the default `harness-board.db`
/// path (relative to cwd) is what a stray board-open would create.
fn run_agent(cwd: &Path, args: &[&str], harness_db: Option<&Path>) -> Output {
	let mut cmd = Command::new(env!("CARGO_BIN_EXE_agent"));
	cmd.args(args).current_dir(cwd).env_remove("HARNESS_DB");
	if let Some(db) = harness_db {
		cmd.env("HARNESS_DB", db);
	}
	cmd.output().expect("spawn agent binary")
}

/// `git init` a fresh temp dir. `sprint` resolves the repo root from cwd before it
/// touches the board, so its dispatch arm can only be reached inside a repo.
fn git_init(dir: &Path) {
	let out = Command::new("git")
		.args(["init", "-q", "-b", "main"])
		.current_dir(dir)
		.output()
		.expect("spawn git init");
	assert!(out.status.success(), "git init failed: {}", String::from_utf8_lossy(&out.stderr));
}

/// Any board side-file (`*.db`, `*.db-shm`, `*.db-wal`, `*.db-journal`) in `dir`.
fn db_files(dir: &Path) -> Vec<String> {
	std::fs::read_dir(dir)
		.expect("read temp cwd")
		.map(|e| e.expect("dir entry").file_name().to_string_lossy().into_owned())
		.filter(|n| n.contains(".db"))
		.collect()
}

#[test]
fn no_verb_exits_usage_without_opening_board() {
	let dir = fresh_dir("no-verb");
	let out = run_agent(&dir, &[], None);
	let stderr = String::from_utf8_lossy(&out.stderr);
	assert_eq!(out.status.code(), Some(2), "bare `agent` must exit 2, stderr: {stderr}");
	assert!(stderr.contains(USAGE_SNIPPET), "bare `agent` must print usage, got: {stderr}");
	assert_eq!(db_files(&dir), Vec::<String>::new(), "usage path must not create board side-files");
}

#[test]
fn unknown_verb_exits_usage_without_opening_board() {
	let dir = fresh_dir("unknown-verb");
	let out = run_agent(&dir, &["frobnicate"], None);
	let stderr = String::from_utf8_lossy(&out.stderr);
	assert_eq!(out.status.code(), Some(2), "`agent frobnicate` must exit 2, stderr: {stderr}");
	assert!(stderr.contains(USAGE_SNIPPET), "unknown verb must print usage, got: {stderr}");
	assert_eq!(db_files(&dir), Vec::<String>::new(), "usage path must not create board side-files");
}

#[test]
fn known_verb_reaches_its_dispatch_arm_not_usage() {
	let dir = fresh_dir("known-verb");
	let db = dir.join("board.db");
	let out = run_agent(&dir, &["recall-body", "m0"], Some(&db));
	let stderr = String::from_utf8_lossy(&out.stderr);
	// recall-body on a fresh board: the arm runs, finds no live memory m0, and
	// exits 1 — NOT the usage path (exit 2) and NOT a fallthrough panic (101).
	assert_eq!(out.status.code(), Some(1), "recall-body arm must run and exit 1, stderr: {stderr}");
	assert!(stderr.contains("no live memory m0"), "recall-body arm must report the missing id, got: {stderr}");
	assert!(!stderr.contains(USAGE_SNIPPET), "known verb must not hit the usage path, got: {stderr}");
	assert!(db.exists(), "known verb opens the board at HARNESS_DB");
}

#[test]
fn sprint_verb_reaches_its_dispatch_arm_not_usage() {
	let dir = fresh_dir("sprint");
	git_init(&dir);
	let db = dir.join("board.db");
	let out = run_agent(&dir, &["sprint"], Some(&db));
	let stdout = String::from_utf8_lossy(&out.stdout);
	let stderr = String::from_utf8_lossy(&out.stderr);
	// An empty board has nothing runnable, so the arm runs the coordinator to its
	// early return and exits 0 — NOT the usage path (exit 2) and NOT the
	// `unreachable!` fallthrough (101) a missing dispatch arm would take. A sprint
	// over an empty board makes no worker and no provider call, so this is hermetic.
	assert_eq!(out.status.code(), Some(0), "sprint arm must run and exit 0, stderr: {stderr}");
	assert!(stdout.contains("[sprint] nothing runnable"), "sprint arm must report an empty board, got: {stdout}");
	assert!(!stderr.contains(USAGE_SNIPPET), "sprint must not hit the usage path, got: {stderr}");
	assert!(db.exists(), "sprint opens the board at HARNESS_DB");
}

// The --json machine-readable face of the read verbs (board/show/status): the
// same reads the text renderers use, emitted as one JSON object per invocation,
// with the flag accepted anywhere after the verb. Drives a real board through
// new → criteria → align (the path `dogfood/board-json-check.sh` walks in
// bash+python) and pins the contract — including that the TEXT line is
// byte-for-byte unchanged when the flag is absent (existing scripts parse it).
#[test]
fn json_flags_on_board_show_status_emit_the_machine_contract() {
	use serde_json::Value;

	let dir = fresh_dir("json-verbs");
	let db = dir.join("board.db");
	let run = |args: &[&str]| {
		let out = run_agent(&dir, args, Some(&db));
		assert!(
			out.status.success(),
			"`agent {}` must succeed, stderr: {}",
			args.join(" "),
			String::from_utf8_lossy(&out.stderr)
		);
		String::from_utf8_lossy(&out.stdout).into_owned()
	};

	let new_out = run(&["new", "json check", "--kind", "question"]);
	let id = new_out.split_whitespace().next().expect("new prints the minted id").to_string();

	run(&["criteria", &id, "states/counts/tickets round-trip as JSON"]);
	run(&["validation", &id, "true"]);
	run(&["note", &id, "seeded by the json test"]);
	run(&["align", &id]);

	// board --json: the 8 spine states in declaration order, zero-filled counts,
	// exactly one ticket — in_progress past the human align gate.
	let board: Value = serde_json::from_str(&run(&["board", "--json"])).expect("board --json is one JSON object");
	assert_eq!(board["states"].as_array().map(|a| a.len()), Some(8), "states: {}", board["states"]);
	assert_eq!(board["states"][0].as_str(), Some("todo"));
	let tickets = board["tickets"].as_array().expect("tickets is a list");
	assert_eq!(tickets.len(), 1, "one ticket on the fresh board");
	assert_eq!(tickets[0]["status"].as_str(), Some("in_progress"));
	assert_eq!(tickets[0]["kind"].as_str(), Some("question"));
	assert_eq!(tickets[0]["attempt"].as_i64(), Some(0));
	assert_eq!(
		tickets[0]["workpad"]["criteria"].as_str(),
		Some("states/counts/tickets round-trip as JSON"),
		"workpad.criteria is the text that was set"
	);
	assert!(tickets[0]["gates"].is_array(), "gates is a list");
	assert_eq!(tickets[0]["red_gates"].as_i64(), Some(0));
	// never invent what the read shape doesn't carry:
	assert!(tickets[0]["created_at"].is_null(), "unstamped created_at stays null");
	assert!(tickets[0]["updated_at"].is_null(), "unstamped updated_at stays null");

	// show <id> --json: the same ticket-object shape, standalone. The align pass
	// is one row of the gate history (a list), red_gates counts latest-per-gate.
	let show: Value = serde_json::from_str(&run(&["show", &id, "--json"])).expect("show --json is one JSON object");
	assert_eq!(show["id"].as_str(), Some(id.as_str()));
	assert_eq!(show["status"].as_str(), Some("in_progress"));
	assert_eq!(show["workpad"]["criteria"].as_str(), Some("states/counts/tickets round-trip as JSON"));
	assert!(show["gates"].is_array(), "gates is a list");
	assert_eq!(show["gates"].as_array().map(|a| a.len()), Some(1), "the human align pass is in the history");
	assert_eq!(show["gates"][0]["passed"].as_bool(), Some(true));
	assert_eq!(show["red_gates"].as_i64(), Some(0));

	// status <id> --json: exactly the three keys, in a single object. (Key ORDER
	// is not part of the contract — serde_json's map sorts keys alphabetically.)
	let status: Value = serde_json::from_str(&run(&["status", &id, "--json"])).expect("status --json is one JSON object");
	let mut keys: Vec<&str> =
		status.as_object().expect("status is an object").keys().map(String::as_str).collect();
	keys.sort_unstable();
	assert_eq!(keys, vec!["attempt", "id", "status"], "exactly the three keys");
	assert_eq!(status["status"].as_str(), Some("in_progress"));

	// the flag is accepted anywhere after the verb — before the id too.
	let flipped: Value =
		serde_json::from_str(&run(&["status", "--json", &id])).expect("--json before the id still parses");
	assert_eq!(flipped["status"].as_str(), Some("in_progress"));

	// with the flag absent, the text line is byte-for-byte what it always was.
	let text = run(&["status", &id]);
	assert_eq!(
		text,
		format!("{id}  [question] in_progress  attempt=0  \"json check\"\n"),
		"text output must stay byte-identical when --json is absent"
	);
}
