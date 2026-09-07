//! Spawns the built `agent` binary to pin the `pi prepare` CLI contract (unit
//! D4a): the four stderr step lines, the `--json` stdout object, the exit codes
//! (0 all-ok/skipped, 9 root guard, 6 any other failure, 2 usage), the checkout
//! case (`--agent-dir` equal to `--template` skips seeding), the key fallback
//! through `$HOME/.omlx/settings.json`, and that the verb is dispatched BEFORE
//! the board opens (no harness-board.db side-files in cwd). Same pattern as
//! cli_dispatch.rs / board_extension.rs.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

/// Fresh empty dir per case, namespaced by pid + test name so parallel test
/// binaries and repeated runs never collide.
fn fresh_dir(name: &str) -> PathBuf {
	let dir = std::env::temp_dir().join(format!("agent-pi-prepare-{}-{name}", std::process::id()));
	let _ = std::fs::remove_dir_all(&dir);
	std::fs::create_dir_all(&dir).expect("create fresh temp dir");
	dir
}

fn write(path: &Path, content: &str) {
	if let Some(parent) = path.parent() {
		std::fs::create_dir_all(parent).expect("mkdir");
	}
	std::fs::write(path, content).expect("write");
}

/// The managed template, small but covering files and nested dirs.
fn template(name: &str) -> PathBuf {
	let root = fresh_dir(name);
	write(&root.join("AGENTS.md"), "# agent\n");
	write(&root.join("settings.json"), "{}\n");
	write(&root.join("settings.README.md"), "docs\n");
	write(
		&root.join("models.json.tmpl"),
		r#"{"baseUrl": "http://__BPPC_HOST__:8080/v1", "apiKey": "__OMLX_KEY__"}"#,
	);
	write(&root.join("agents/worker.md"), "# worker\n");
	write(&root.join("skills/dev/SKILL.md"), "# dev\n");
	root
}

/// Run `agent pi prepare` with a controlled environment: HARNESS_DB and
/// OMLX_API_KEY stripped, HOME/USERPROFILE pointed at a temp home so neither
/// the developer's nor CI's real oMLX settings can leak into a result. `tag`
/// namespaces this call's run cwd; tests run in parallel, so shared temp names
/// would race (one spawn's cwd removed under another).
fn run_prepare(tag: &str, template: &Path, agent_dir: &Path, cwd: &Path, extra: &[&str], env: &[(&str, &str)]) -> Output {
	let home = fresh_dir("home");
	let mut cmd = Command::new(env!("CARGO_BIN_EXE_agent"));
	cmd.args(["pi", "prepare"])
		.arg("--template").arg(template)
		.arg("--agent-dir").arg(agent_dir)
		.arg("--cwd").arg(cwd)
		.args(extra)
		.current_dir(fresh_dir(&format!("run-{tag}")))
		.env_remove("HARNESS_DB")
		.env_remove("OMLX_API_KEY")
		.env("HOME", &home)
		.env("USERPROFILE", &home);
	for (k, v) in env {
		cmd.env(k, v);
	}
	cmd.output().expect("spawn agent binary")
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

/// Any board side-file (`*.db`, `*.db-shm`, `*.db-wal`, `*.db-journal`) in `dir`.
fn db_files(dir: &Path) -> Vec<String> {
	std::fs::read_dir(dir)
		.expect("read dir")
		.map(|e| e.expect("dir entry").file_name().to_string_lossy().into_owned())
		.filter(|n| n.contains(".db"))
		.collect()
}

#[test]
fn json_happy_path_seeds_renders_guards_and_inits_wiki() {
	let tmpl = template("happy-tmpl");
	let agent_dir = fresh_dir("happy-agent");
	let project = fresh_dir("happy-proj");
	write(&project.join("CLAUDE.md"), "x\n");
	let out = run_prepare(
		"happy",
		&tmpl,
		&agent_dir,
		&project,
		&["--bppc-host", "203.0.113.10", "--json"],
		&[("OMLX_API_KEY", "happy-key-1")],
	);
	assert_eq!(out.status.code(), Some(0), "exit 0, stderr: {}", stderr(&out));

	// the four step lines on stderr, in launcher order
	let err = stderr(&out);
	assert!(err.contains("[1/4] seed ... OK ("), "stderr: {err}");
	assert!(err.contains("[2/4] render ... OK ("), "stderr: {err}");
	assert!(err.contains("[3/4] root ... OK ("), "stderr: {err}");
	assert!(err.contains("[4/4] wiki ... OK ("), "stderr: {err}");

	// one JSON object on stdout: the exact keys of the contract
	let v = json_out(&out);
	let steps = v["steps"].as_array().expect("steps is a list");
	assert_eq!(steps.len(), 4);
	assert_eq!(steps[0]["name"].as_str(), Some("seed"));
	assert_eq!(steps[0]["status"].as_str(), Some("OK"));
	assert_eq!(steps[3]["name"].as_str(), Some("wiki"));
	assert!(steps[0]["detail"].is_string(), "every step carries a detail");
	assert_eq!(
		v["agentDir"].as_str(),
		Some(agent_dir.to_str().expect("utf8")),
		"agentDir is the --agent-dir given"
	);
	assert_eq!(
		v["modelsJson"].as_str(),
		Some(agent_dir.join("models.json").to_str().expect("utf8")),
		"modelsJson is the path, never the rendered content (the key rides there)"
	);
	assert_eq!(
		v["env"]["PI_CODING_AGENT_DIR"].as_str(),
		Some(agent_dir.to_str().expect("utf8"))
	);

	// the disk holds the truth: the rendered file, the stamp, the wiki
	let models = std::fs::read_to_string(agent_dir.join("models.json")).expect("models.json");
	assert_eq!(
		models,
		r#"{"baseUrl": "http://203.0.113.10:8080/v1", "apiKey": "happy-key-1"}"#
	);
	assert!(agent_dir.join(".terax-seed").is_file());
	assert!(agent_dir.join("skills/dev/SKILL.md").is_file());
	assert!(project.join("wiki/index.md").is_file());
}

#[test]
fn exit_9_when_the_cwd_is_not_a_project_root() {
	let tmpl = template("root9-tmpl");
	let agent_dir = fresh_dir("root9-agent");
	let project = fresh_dir("root9-proj"); // no marker of any kind
	let out = run_prepare(
		"root9",
		&tmpl,
		&agent_dir,
		&project,
		&["--json"],
		&[("OMLX_API_KEY", "k")],
	);
	assert_eq!(out.status.code(), Some(9), "the root guard exits 9, stderr: {}", stderr(&out));
	let v = json_out(&out);
	let steps = v["steps"].as_array().expect("steps");
	let root = steps.iter().find(|s| s["name"] == "root").expect("root step");
	assert_eq!(root["status"].as_str(), Some("FAIL"));
	assert!(root["detail"].as_str().expect("detail").contains("no .git"));
	// a failed step never aborts the rest: wiki init still ran
	assert_eq!(
		steps.iter().find(|s| s["name"] == "wiki").expect("wiki step")["status"].as_str(),
		Some("OK")
	);
}

#[test]
fn exit_6_when_the_seed_fails_on_a_missing_template() {
	// A path that does NOT exist: under a created base dir, so fresh_dir cannot
	// accidentally make it real.
	let missing = fresh_dir("seed6-base").join("no-such-template");
	let agent_dir = fresh_dir("seed6-agent");
	let project = fresh_dir("seed6-proj");
	write(&project.join("CLAUDE.md"), "x\n");
	let out = run_prepare(
		"seed6",
		&missing,
		&agent_dir,
		&project,
		&["--json"],
		&[("OMLX_API_KEY", "k")],
	);
	assert_eq!(out.status.code(), Some(6), "a non-root failure exits 6, stderr: {}", stderr(&out));
	let v = json_out(&out);
	let steps = v["steps"].as_array().expect("steps");
	assert_eq!(steps.iter().find(|s| s["name"] == "seed").expect("seed")["status"].as_str(), Some("FAIL"));
	// render fails too (no template to read), root passes, wiki still runs
	assert_eq!(steps.iter().find(|s| s["name"] == "render").expect("render")["status"].as_str(), Some("FAIL"));
	assert_eq!(steps.iter().find(|s| s["name"] == "root").expect("root")["status"].as_str(), Some("OK"));
}

#[test]
fn missing_key_fails_render_with_the_var_name_and_exits_6() {
	let tmpl = template("nokey-tmpl");
	let agent_dir = fresh_dir("nokey-agent");
	let project = fresh_dir("nokey-proj");
	write(&project.join("CLAUDE.md"), "x\n");
	// PI_TEST_KEY unset, HOME is a bare temp dir with no .omlx/settings.json.
	let out = run_prepare(
		"nokey",
		&tmpl,
		&agent_dir,
		&project,
		&["--omlx-key-env", "PI_TEST_KEY", "--json"],
		&[],
	);
	assert_eq!(out.status.code(), Some(6), "stderr: {}", stderr(&out));
	let err = stderr(&out);
	assert!(
		err.contains("PI_TEST_KEY not set") && err.contains("cannot render"),
		"the failure names the env var, never a key value: {err}"
	);
	assert!(!err.contains("happy-key"), "no key value may appear in any report");
	assert!(!agent_dir.join("models.json").exists(), "a failed render writes nothing");
}

#[test]
fn key_falls_back_to_home_omlx_settings_json() {
	let tmpl = template("fallback-tmpl");
	let agent_dir = fresh_dir("fallback-agent");
	let project = fresh_dir("fallback-proj");
	write(&project.join("CLAUDE.md"), "x\n");
	// A settings file in the (temp) home: the fallback the bash launcher uses.
	let home = fresh_dir("fallback-home");
	write(
		&home.join(".omlx").join("settings.json"),
		r#"{"auth": {"api_key": "settings-key-1"}}"#,
	);
	// HOME is usually random per run; pass the prepared one explicitly.
	let mut cmd = Command::new(env!("CARGO_BIN_EXE_agent"));
	cmd.args(["pi", "prepare", "--template"]).arg(&tmpl)
		.arg("--agent-dir").arg(&agent_dir)
		.arg("--cwd").arg(&project)
		.arg("--json")
		.current_dir(fresh_dir("fallback-cwd"))
		.env_remove("HARNESS_DB")
		.env_remove("OMLX_API_KEY")
		.env("HOME", &home)
		.env("USERPROFILE", &home);
	let out = cmd.output().expect("spawn agent binary");
	assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
	let models = std::fs::read_to_string(agent_dir.join("models.json")).expect("models.json");
	assert_eq!(
		models,
		r#"{"baseUrl": "http://127.0.0.1:8080/v1", "apiKey": "settings-key-1"}"#,
		"blank host defaults to 127.0.0.1 and the key came from the settings file"
	);
}

#[test]
fn checkout_case_skips_seed_and_renders_in_place() {
	let tmpl = template("checkout-tmpl");
	let project = fresh_dir("checkout-proj");
	write(&project.join("AGENTS.md"), "project instructions\n");
	let out = run_prepare(
		"checkout",
		&tmpl, // agent_dir == template, the checkout case
		&tmpl,
		&project,
		&["--json"],
		&[("OMLX_API_KEY", "checkout-key")],
	);
	assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
	let v = json_out(&out);
	let seed = v["steps"].as_array().expect("steps").iter().find(|s| s["name"] == "seed").expect("seed");
	assert_eq!(seed["status"].as_str(), Some("SKIPPED"));
	assert!(seed["detail"].as_str().expect("detail").contains("used in place"));
	assert!(!tmpl.join(".terax-seed").exists(), "no stamp is written into a checkout");
	assert!(
		std::fs::read_to_string(tmpl.join("models.json")).expect("rendered in place").contains("checkout-key"),
		"render targets the dir in place"
	);
}

#[test]
fn allow_any_dir_and_no_wiki_skip_their_steps() {
	let tmpl = template("skip-tmpl");
	let agent_dir = fresh_dir("skip-agent");
	let project = fresh_dir("skip-proj"); // no marker: the guard would fail
	let out = run_prepare(
		"skip",
		&tmpl,
		&agent_dir,
		&project,
		&["--allow-any-dir", "--no-wiki", "--json"],
		&[("OMLX_API_KEY", "k")],
	);
	assert_eq!(out.status.code(), Some(0), "SKIPPED is success, stderr: {}", stderr(&out));
	let err = stderr(&out);
	assert!(err.contains("[3/4] root ... SKIPPED ("), "stderr: {err}");
	assert!(err.contains("[4/4] wiki ... SKIPPED ("), "stderr: {err}");
	assert!(!project.join("wiki").exists(), "--no-wiki creates no wiki");
}

#[test]
fn stamp_and_version_flags_land_in_the_seed_stamp() {
	let tmpl = template("stamp-tmpl");
	let project = fresh_dir("stamp-proj");
	write(&project.join("CLAUDE.md"), "x\n");

	let agent_dir = fresh_dir("stamp-explicit");
	let out = run_prepare("stamp-a", &tmpl, &agent_dir, &project, &["--stamp", "operator-v7"], &[("OMLX_API_KEY", "k")]);
	assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
	assert_eq!(
		std::fs::read_to_string(agent_dir.join(".terax-seed")).expect("stamp"),
		"operator-v7",
		"an explicit --stamp is stored verbatim"
	);

	let agent_dir = fresh_dir("stamp-version");
	let out = run_prepare("stamp-b", &tmpl, &agent_dir, &project, &["--version", "1.2.3"], &[("OMLX_API_KEY", "k")]);
	assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
	let stamp = std::fs::read_to_string(agent_dir.join(".terax-seed")).expect("stamp");
	assert!(stamp.starts_with("1.2.3+") && stamp.len() == 22, "version-prefixed hash, got {stamp}");

	// no flags at all: the bare 16-hex template hash
	let agent_dir = fresh_dir("stamp-bare");
	let out = run_prepare("stamp-c", &tmpl, &agent_dir, &project, &[], &[("OMLX_API_KEY", "k")]);
	assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
	let stamp = std::fs::read_to_string(agent_dir.join(".terax-seed")).expect("stamp");
	assert_eq!(stamp.len(), 16, "bare hash, got {stamp}");
}

#[test]
fn pi_prepare_never_opens_the_board() {
	let tmpl = template("noboard-tmpl");
	let agent_dir = fresh_dir("noboard-agent");
	let project = fresh_dir("noboard-proj");
	write(&project.join("CLAUDE.md"), "x\n");
	// A bare run cwd and no HARNESS_DB: if the dispatch went through
	// Board::open, harness-board.db{,-shm,-wal} would appear in that cwd.
	let run_cwd = fresh_dir("noboard-runcwd");
	let out = Command::new(env!("CARGO_BIN_EXE_agent"))
		.args(["pi", "prepare", "--template"])
		.arg(&tmpl)
		.arg("--agent-dir")
		.arg(&agent_dir)
		.arg("--cwd")
		.arg(&project)
		.arg("--json")
		.current_dir(&run_cwd)
		.env_remove("HARNESS_DB")
		.env_remove("OMLX_API_KEY")
		.env("OMLX_API_KEY", "noboard-key")
		.env("HOME", fresh_dir("noboard-home"))
		.env("USERPROFILE", fresh_dir("noboard-home2"))
		.output()
		.expect("spawn agent binary");
	assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
	assert_eq!(db_files(&run_cwd), Vec::<String>::new(), "the run cwd must stay clean");
	assert_eq!(db_files(&agent_dir), Vec::<String>::new());
	assert_eq!(db_files(&project), Vec::<String>::new());
}

#[test]
fn pi_usage_errors_exit_2() {
	let dir = fresh_dir("usage");
	let run = |args: &[&str]| {
		Command::new(env!("CARGO_BIN_EXE_agent"))
			.args(args)
			.current_dir(&dir)
			.env_remove("HARNESS_DB")
			.output()
			.expect("spawn agent binary")
	};

	// unknown pi subverb
	let out = run(&["pi", "frobnicate"]);
	assert_eq!(out.status.code(), Some(2), "unknown subverb must exit 2");
	assert!(stderr(&out).contains("usage: agent pi prepare"), "stderr: {}", stderr(&out));

	// missing required flags
	let out = run(&["pi", "prepare"]);
	assert_eq!(out.status.code(), Some(2), "missing flags must exit 2");
	assert!(stderr(&out).contains("--template"), "usage names the flags: {}", stderr(&out));

	// unknown flag
	let tmpl = template("usage-tmpl");
	let out = run_prepare("usage-flag", &tmpl, &fresh_dir("usage-a"), &fresh_dir("usage-p"), &["--frob"], &[]);
	assert_eq!(out.status.code(), Some(2), "unknown flag must exit 2");
	assert!(stderr(&out).contains("usage: agent pi prepare"));
}
