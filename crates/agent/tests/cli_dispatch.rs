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
