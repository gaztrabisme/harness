//! Git worktree lifecycle for ticket isolation (slice 5). A ticket's work happens
//! on branch `harness/<id>` in a worktree under `.harness/worktrees/<id>`, forked
//! from the repo's current branch. `land` squash-merges that branch into the base
//! and the squash commit IS the operator's approval (the §10 keystone). Churn
//! lives and dies in the worktree; only the squashed result reaches the base.
//!
//! Plain `git` (not pi-iso): land = squash-merge needs branch/base control and a
//! shared object store so merge-back is trivial. pi-iso stays the CoW content-
//! isolation + diff primitive for later, non-git workloads.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

const BRANCH_PREFIX: &str = "harness/";

/// Run `git` in `dir`, returning trimmed stdout; error on a non-zero exit.
fn git(dir: &Path, args: &[&str]) -> Result<String> {
	let out = Command::new("git")
		.args(args)
		.current_dir(dir)
		.output()
		.with_context(|| format!("running git {args:?}"))?;
	if !out.status.success() {
		bail!("git {:?} failed: {}", args, String::from_utf8_lossy(&out.stderr).trim());
	}
	Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Run `git` for its exit status only (existence/clean checks).
fn git_ok(dir: &Path, args: &[&str]) -> bool {
	Command::new("git").args(args).current_dir(dir).output().map(|o| o.status.success()).unwrap_or(false)
}

/// The ticket's branch name.
pub fn branch_for(id: &str) -> String {
	format!("{BRANCH_PREFIX}{id}")
}

/// Repo top-level containing `cwd`.
pub fn repo_root(cwd: &Path) -> Result<PathBuf> {
	Ok(PathBuf::from(git(cwd, &["rev-parse", "--show-toplevel"])?))
}

/// The (gitignored) worktree path for a ticket — a pure function of the id, so
/// nothing needs storing on the board.
pub fn worktree_path(root: &Path, id: &str) -> PathBuf {
	root.join(".harness").join("worktrees").join(id)
}

/// The (gitignored) trajectory path for a run — root-relative, NOT worktree-
/// relative (research/17 N1). `land` removes the worktree on success, so a
/// worktree-local trajectory would vanish exactly when the run *succeeds*; rooting
/// it under `.harness/runs/` (a sibling of `worktrees/`) keeps the record. Pure
/// function of root + ids, mirroring `worktree_path`. `run_id` already encodes
/// ticket+attempt+time, so it sorts chronologically within the ticket dir.
pub fn runs_path(root: &Path, ticket: &str, run_id: &str) -> PathBuf {
	root.join(".harness").join("runs").join(ticket).join(format!("{run_id}.jsonl"))
}

/// Short HEAD sha in `dir`, for the workpad header. A repo with no commits (or
/// no git) degrades to `0000000` rather than failing the render.
pub fn short_sha(dir: &Path) -> String {
	git(dir, &["rev-parse", "--short", "HEAD"]).unwrap_or_else(|_| "0000000".into())
}

/// Every tracked, repo-relative path under `dir` — the repo's own account of what
/// exists. `draft` injects this so the drafter names real files instead of
/// plausible ones. Tracked-only is deliberate: an untracked file is churn (build
/// output, a scratch note, another ticket's worktree), and `.gitignore` already
/// encodes which is which. Errors (no git, not a repo) are the caller's to
/// degrade on — drafting proceeds with an empty tree.
pub fn ls_files(dir: &Path) -> Result<Vec<String>> {
	let out = git(dir, &["ls-files"])?;
	Ok(out.lines().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
}

/// Escape hatch for the fork-point guard: `HARNESS_ALLOW_STALE_FORK=1` downgrades
/// the stale-fork refusal to a loud warning. Exists for the legit additive-retry
/// case where base advanced under a live branch (e.g. wiki commits landed on main
/// mid-sprint) and the operator decides the divergence is benign.
const ALLOW_STALE_ENV: &str = "HARNESS_ALLOW_STALE_FORK";

/// Fork-point guard for branch/worktree REUSE: refuse a surviving `harness/<id>`
/// branch whose fork point (merge-base with the root's current HEAD) is behind
/// that HEAD. Attaching a worker to such a branch silently bases its work on an
/// old base — the stale-worktree failure mode a leftover branch from an older
/// board generation produces. `allow_stale` is a parameter (not an env read) so
/// tests exercise both sides hermetically without mutating process env.
fn guard_fork_point(root: &Path, branch: &str, allow_stale: bool) -> Result<()> {
	let base_head = git(root, &["rev-parse", "HEAD"])?;
	let fork = git(root, &["merge-base", branch, "HEAD"])?;
	if fork == base_head {
		return Ok(());
	}
	let behind =
		git(root, &["rev-list", "--count", &format!("{fork}..HEAD")]).unwrap_or_else(|_| "?".into());
	if allow_stale {
		eprintln!(
			"WARNING: reusing STALE branch {branch} — forked at {fork}, {behind} commit(s) behind \
			 base HEAD {base_head} ({ALLOW_STALE_ENV}=1: proceeding on the old fork)"
		);
		return Ok(());
	}
	let id = branch.strip_prefix(BRANCH_PREFIX).unwrap_or(branch);
	bail!(
		"branch {branch} is STALE: forked at {fork}, {behind} commit(s) behind base HEAD {base_head} \
		 — refusing to attach work to an old base. Remedies:\n\
		 - preserve its unlanded work out of the way: git branch -m {branch} stale/{id}\n\
		 - start over from current HEAD: agent rework {id} (DESTROYS the branch's unlanded work)\n\
		 - proceed deliberately on the old fork: {ALLOW_STALE_ENV}=1"
	)
}

/// Ensure the ticket's worktree + branch exist; return the worktree path.
/// Idempotent: an existing worktree dir is reused (resume a run). Reads the
/// `HARNESS_ALLOW_STALE_FORK` escape hatch once and delegates to
/// `ensure_worktree_with`, the testable core.
pub fn ensure_worktree(root: &Path, id: &str) -> Result<PathBuf> {
	let allow_stale = std::env::var(ALLOW_STALE_ENV).is_ok_and(|v| v == "1");
	ensure_worktree_with(root, id, allow_stale)
}

/// Core of `ensure_worktree` with the stale-fork hatch as a parameter. BOTH reuse
/// paths — an existing worktree dir, and attaching a fresh worktree to a surviving
/// branch — pass the fork-point guard before anything is created or attached; a
/// fresh branch (no worktree, no branch) forks off current HEAD as always and
/// needs no guard.
pub fn ensure_worktree_with(root: &Path, id: &str, allow_stale: bool) -> Result<PathBuf> {
	let wt = worktree_path(root, id);
	let branch = branch_for(id);
	let branch_exists = git_ok(root, &["rev-parse", "--verify", &branch]);
	if branch_exists {
		guard_fork_point(root, &branch, allow_stale)?;
	}
	if wt.exists() {
		return Ok(wt);
	}
	if let Some(parent) = wt.parent() {
		std::fs::create_dir_all(parent).context("creating worktree parent dir")?;
	}
	let wt_str = wt.to_string_lossy().to_string();
	if branch_exists {
		// branch survived a previous attempt — attach a fresh worktree to it
		git(root, &["worktree", "add", &wt_str, &branch])?;
	} else {
		git(root, &["worktree", "add", "-b", &branch, &wt_str])?; // off current HEAD
	}
	Ok(wt)
}

/// The base branch the ticket's work will land into — whatever the main repo
/// `root` currently has checked out (`squash_merge` merges into it). Used by
/// `harden` to scope the mutation diff to the ticket's own changes.
pub fn base_branch(root: &Path) -> Result<String> {
	git(root, &["rev-parse", "--abbrev-ref", "HEAD"])
}

/// A unified diff of the ticket's changes: everything on the worktree's HEAD
/// since it diverged from `base` (three-dot = changes introduced by the branch,
/// not base advances). This is the `--in-diff` scope cargo-mutants mutates.
pub fn diff_against(wt: &Path, base: &str) -> Result<String> {
	git(wt, &["diff", &format!("{base}...HEAD")])
}

/// As `diff_against`, but limited to `paths` (repo-relative) — the mutation
/// scope after `harden`'s provenance filter dropped template-stamped files.
/// `:(literal)` pathspec magic keeps a path a path (never a glob). Zero paths
/// means a zero-file scope → empty diff, NOT an unlimited one (a bare trailing
/// `--` would ask git for the full diff again).
pub fn diff_against_paths(wt: &Path, base: &str, paths: &[String]) -> Result<String> {
	if paths.is_empty() {
		return Ok(String::new());
	}
	let range = format!("{base}...HEAD");
	let mut args = vec!["diff".to_string(), range, "--".to_string()];
	args.extend(paths.iter().map(|p| format!(":(literal){p}")));
	let refs: Vec<&str> = args.iter().map(String::as_str).collect();
	git(wt, &refs)
}

/// Total lines changed (insertions + deletions) on the worktree's branch since it
/// diverged from `base` — a coarse "did this branch do something, and how much"
/// signal for the `explore` report (research/19 §9). Binary files (numstat shows
/// `-`/`-`) contribute nothing. Same three-dot scope as `diff_against`.
pub fn lines_changed_against(wt: &Path, base: &str) -> Result<usize> {
	let stat = git(wt, &["diff", "--numstat", &format!("{base}...HEAD")])?;
	let mut total = 0usize;
	for line in stat.lines() {
		let mut cols = line.split_whitespace();
		let add = cols.next().and_then(|s| s.parse::<usize>().ok());
		let del = cols.next().and_then(|s| s.parse::<usize>().ok());
		if let (Some(a), Some(d)) = (add, del) {
			total += a + d;
		}
	}
	Ok(total)
}

/// The names (repo-relative paths) of files the branch changed since it diverged
/// from `base` — same three-dot scope as `diff_against`, but just the paths. Used
/// by the oracle-integrity check (finding #4) to spot a modified acceptance test.
pub fn changed_files_against(wt: &Path, base: &str) -> Result<Vec<String>> {
	let out = git(wt, &["diff", "--name-only", &format!("{base}...HEAD")])?;
	Ok(out.lines().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
}

/// As `changed_files_against`, but every path paired with its status letter —
/// `A`dded, `M`odified, `D`eleted, `R`enamed, … — from `git diff --name-status
/// <base>...HEAD`. Same three-dot scope, so the letter describes what the BRANCH
/// did, never what base advanced past it. harden's provenance filter turns on
/// this letter (finding #2, PDSI t6): under a template-stamped directory only a
/// file that already EXISTED on base is regenerated output; one the branch ADDED
/// there is the ticket's own handwriting, and nothing but the status tells them
/// apart.
pub fn changed_files_status_against(wt: &Path, base: &str) -> Result<Vec<(char, String)>> {
	Ok(parse_name_status(&git(wt, &["diff", "--name-status", &format!("{base}...HEAD")])?))
}

/// Parse `git diff --name-status` output into (status, path) rows. Rename/copy
/// lines carry TWO paths (`R100\told\tnew`); the LAST field is the path that
/// exists on the branch, which is also the single entry `--name-only` prints for
/// that rename — so the two helpers stay row-for-row comparable.
fn parse_name_status(out: &str) -> Vec<(char, String)> {
	let mut rows = Vec::new();
	for line in out.lines() {
		let mut cols = line.split('\t');
		let Some(status) = cols.next().and_then(|s| s.trim().chars().next()) else { continue };
		let Some(path) = cols.next_back().map(str::trim).filter(|p| !p.is_empty()) else { continue };
		rows.push((status, path.to_string()));
	}
	rows
}

/// True if `path` (repo-relative) existed on `base` — i.e. it's a committed file
/// the branch inherited, not one the branch newly created. The oracle-integrity
/// check uses this to tell a *modified* acceptance test (present on base → must
/// not change) from the agent's own *added* tests (absent on base → allowed).
pub fn path_exists_on_base(wt: &Path, base: &str, path: &str) -> bool {
	git_ok(wt, &["cat-file", "-e", &format!("{base}:{path}")])
}

/// The provenance stamp template instantiation drops in every directory it
/// generates. Shared by harden's mutation-scope filter (worktree-fs walk, its
/// intentional semantics) and verify's oracle-freeze exemption (base-resolved,
/// `stamped_on_base` below) — one name, two deliberately different resolvers.
pub const TEMPLATE_STAMP: &str = ".template-stamp.json";

/// True when some ancestor directory of `path` (repo-relative), from the file's
/// own directory up to and including the repo root, carries a committed
/// `.template-stamp.json` ON `base`. Resolved with `git cat-file -e
/// <base>:<dir>/<stamp>` — NEVER the worktree filesystem — because verify's
/// oracle-freeze exemption trusts base history only: a worker could drop a
/// stamp file into its own worktree (or commit one on its branch) to unfreeze
/// a protected test, and neither may count. Contrast harden's
/// `is_template_stamped`, whose worktree-fs walk is intentional there.
pub fn stamped_on_base(wt: &Path, base: &str, path: &str) -> bool {
	let mut dir = path;
	loop {
		dir = dir.rsplit_once('/').map_or("", |(parent, _)| parent);
		let spec = if dir.is_empty() {
			format!("{base}:{TEMPLATE_STAMP}") // repo root
		} else {
			format!("{base}:{dir}/{TEMPLATE_STAMP}")
		};
		if git_ok(wt, &["cat-file", "-e", &spec]) {
			return true;
		}
		if dir.is_empty() {
			return false;
		}
	}
}

/// True if `dir` has uncommitted changes — tracked *or* untracked — relative to
/// HEAD. `git status --porcelain` is empty iff the tree is clean, and unlike
/// `git diff --quiet` it counts brand-new untracked files (exactly what a fresh
/// `write_file` produces). This is the tool-agnostic "did this run change
/// anything" signal for the plan→execute gate (research/24): a worktree that
/// starts clean (forked from base) and is dirty at a natural stop means the run
/// wrote something — regardless of whether `write_file` or a `bash sed` did it,
/// and a net-no-op (write-then-revert) correctly reads clean. Best-effort: a
/// non-repo dir or a git error reads as "not dirty" (the gate then exempts it).
pub fn is_dirty(dir: &Path) -> bool {
	git(dir, &["status", "--porcelain"]).map(|s| !s.trim().is_empty()).unwrap_or(false)
}

/// Commit all changes in the worktree to its branch (a WIP commit). Returns
/// `false` (no commit) when the worktree is clean.
pub fn commit_worktree(wt: &Path, id: &str) -> Result<bool> {
	git(wt, &["add", "-A"])?;
	if git_ok(wt, &["diff", "--cached", "--quiet"]) {
		return Ok(false); // nothing staged → nothing to commit
	}
	git(wt, &["commit", "-q", "-m", &format!("{id}: work in progress")])?;
	Ok(true)
}

/// Squash-merge the ticket branch into the base branch in the main repo `root`,
/// committing one squashed commit. Returns its sha. Refuses a dirty base tree;
/// on conflict, aborts and restores the base, then errors for manual resolution.
pub fn squash_merge(root: &Path, id: &str, title: &str) -> Result<String> {
	let branch = branch_for(id);
	if !git_ok(root, &["diff", "--quiet"]) || !git_ok(root, &["diff", "--cached", "--quiet"]) {
		bail!("base working tree is dirty — commit or stash before landing {id}");
	}
	let out = Command::new("git")
		.args(["merge", "--squash", &branch])
		.current_dir(root)
		.output()
		.context("git merge --squash")?;
	if !out.status.success() {
		let _ = git(root, &["merge", "--abort"]);
		let _ = git(root, &["reset", "--hard"]);
		bail!(
			"squash-merge of {branch} conflicts with base — resolve manually:\n{}",
			String::from_utf8_lossy(&out.stdout).trim()
		);
	}
	if git_ok(root, &["diff", "--cached", "--quiet"]) {
		// merge --squash staged nothing: the branch added no changes over base
		let _ = git(root, &["reset", "--hard"]);
		bail!("nothing to land for {id} — branch {branch} has no changes over base");
	}
	git(root, &["commit", "-q", "-m", &format!("{id}: {title}")])?;
	git(root, &["rev-parse", "HEAD"])
}

/// Remove the ticket's worktree and delete its branch (cleanup on land/rework).
/// Best-effort: a missing worktree or branch is not an error. The worktree is
/// removed first so the branch is no longer checked out and can be deleted.
pub fn remove_worktree(root: &Path, id: &str) -> Result<()> {
	let wt = worktree_path(root, id);
	if wt.exists() {
		let _ = git(root, &["worktree", "remove", "--force", &wt.to_string_lossy()]);
	}
	let branch = branch_for(id);
	if git_ok(root, &["rev-parse", "--verify", &branch]) {
		let _ = git(root, &["branch", "-D", &branch]);
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	fn sh(dir: &Path, args: &[&str]) {
		let out = Command::new("git").args(args).current_dir(dir).output().unwrap();
		assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
	}

	fn init_repo(name: &str) -> PathBuf {
		let dir = std::env::temp_dir().join(name);
		let _ = std::fs::remove_dir_all(&dir);
		std::fs::create_dir_all(&dir).unwrap();
		sh(&dir, &["init", "-q", "-b", "main"]);
		sh(&dir, &["config", "user.email", "t@t"]);
		sh(&dir, &["config", "user.name", "t"]);
		sh(&dir, &["config", "commit.gpgsign", "false"]); // tests must not depend on the dev's GPG
		std::fs::write(dir.join(".gitignore"), ".harness/\n").unwrap();
		std::fs::write(dir.join("README.md"), "seed\n").unwrap();
		sh(&dir, &["add", "-A"]);
		sh(&dir, &["commit", "-q", "-m", "init"]);
		dir
	}

	fn head(dir: &Path) -> String {
		git(dir, &["rev-parse", "HEAD"]).unwrap()
	}

	// The whole lifecycle: worktree isolates writes from the base tree, and
	// land squash-merges them as exactly one commit, then cleans up.
	#[test]
	fn worktree_isolates_then_squash_lands() {
		let root = init_repo("harness-git-lifecycle");
		let base_before = head(&root);

		// create the worktree, write a file in it
		let wt = ensure_worktree(&root, "t1").unwrap();
		assert!(wt.exists(), "worktree materialized");
		std::fs::write(wt.join("feature.txt"), "from the worktree\n").unwrap();

		// ISOLATION: the file is NOT visible in the base working tree
		assert!(!root.join("feature.txt").exists(), "base tree must not see worktree writes");

		// commit the work to the branch, then land it
		assert!(commit_worktree(&wt, "t1").unwrap(), "made a WIP commit");
		assert_eq!(head(&root), base_before, "base HEAD unchanged before land");
		let sha = squash_merge(&root, "t1", "add feature").unwrap();

		// LANDED: file now on base, as exactly one new commit, with our message
		assert!(root.join("feature.txt").exists(), "squash-merge brought the file to base");
		assert_eq!(head(&root), sha, "land returned the new base HEAD");
		assert_ne!(sha, base_before, "base advanced");
		let count: i64 =
			git(&root, &["rev-list", "--count", &format!("{base_before}..HEAD")]).unwrap().parse().unwrap();
		assert_eq!(count, 1, "exactly one squashed commit");
		let subject = git(&root, &["log", "-1", "--format=%s"]).unwrap();
		assert_eq!(subject, "t1: add feature");

		// CLEANUP
		remove_worktree(&root, "t1").unwrap();
		assert!(!wt.exists(), "worktree removed");
		assert!(!git_ok(&root, &["rev-parse", "--verify", "harness/t1"]), "branch deleted");

		let _ = std::fs::remove_dir_all(&root);
	}

	// rework removes the worktree + branch so a fresh attempt re-branches clean.
	#[test]
	fn rework_cleanup_removes_worktree_and_branch() {
		let root = init_repo("harness-git-rework");
		let wt = ensure_worktree(&root, "t2").unwrap();
		std::fs::write(wt.join("scratch.txt"), "wip\n").unwrap();
		commit_worktree(&wt, "t2").unwrap();
		assert!(git_ok(&root, &["rev-parse", "--verify", "harness/t2"]));

		remove_worktree(&root, "t2").unwrap();
		assert!(!wt.exists());
		assert!(!git_ok(&root, &["rev-parse", "--verify", "harness/t2"]));

		// re-creating after cleanup starts from base, not the old branch
		let wt2 = ensure_worktree(&root, "t2").unwrap();
		assert!(!wt2.join("scratch.txt").exists(), "fresh worktree has no prior-attempt files");

		let _ = std::fs::remove_dir_all(&root);
	}

	// the trajectory path is rooted under .harness/runs (a sibling of the worktree),
	// so removing the worktree can never touch it — the N1 invariant, structurally.
	#[test]
	fn runs_path_is_root_relative_not_under_worktree() {
		let root = Path::new("/repo");
		let wt = worktree_path(root, "t9");
		let traj = runs_path(root, "t9", "t9-000-123");
		assert!(traj.starts_with(root.join(".harness").join("runs")), "trajectory under runs/");
		assert!(!traj.starts_with(&wt), "trajectory must NOT live inside the worktree (N1)");
		assert!(traj.to_string_lossy().ends_with("t9/t9-000-123.jsonl"));
	}

	// lines_changed_against counts insertions+deletions on the branch vs base — the
	// coarse "did this branch do something" signal explore ranks/reports on. A clean
	// branch is 0; adding lines bumps it; it ignores base advances (three-dot scope).
	#[test]
	fn lines_changed_counts_branch_diff_over_base() {
		let root = init_repo("harness-git-lineschanged");
		let wt = ensure_worktree(&root, "lc").unwrap();
		// no work yet → zero lines over base
		assert_eq!(lines_changed_against(&wt, "main").unwrap(), 0, "clean branch is 0");

		// add two lines, commit → 2 changed
		std::fs::write(wt.join("a.txt"), "one\ntwo\n").unwrap();
		commit_worktree(&wt, "lc").unwrap();
		assert_eq!(lines_changed_against(&wt, "main").unwrap(), 2, "two inserted lines");

		let _ = std::fs::remove_dir_all(&root);
	}

	// name-status tells ADDED from MODIFIED against base — the granularity harden's
	// provenance filter turns on (finding #2), and it must stay row-for-row with
	// changed_files_against so the two views of the same diff never disagree.
	#[test]
	fn changed_files_status_tells_added_from_modified() {
		let root = init_repo("harness-git-namestatus");
		std::fs::write(root.join("keep.txt"), "one\n").unwrap();
		std::fs::write(root.join("gone.txt"), "bye\n").unwrap();
		sh(&root, &["add", "-A"]);
		sh(&root, &["commit", "-q", "-m", "base files"]);

		let wt = ensure_worktree(&root, "ns").unwrap();
		assert!(changed_files_status_against(&wt, "main").unwrap().is_empty(), "clean branch → no rows");

		std::fs::write(wt.join("keep.txt"), "two\n").unwrap();
		std::fs::write(wt.join("new.txt"), "fresh\n").unwrap();
		std::fs::remove_file(wt.join("gone.txt")).unwrap();
		commit_worktree(&wt, "ns").unwrap();

		let rows = changed_files_status_against(&wt, "main").unwrap();
		let status_of = |p: &str| rows.iter().find(|(_, path)| path == p).map(|(s, _)| *s);
		assert_eq!(status_of("new.txt"), Some('A'), "branch-created file is Added");
		assert_eq!(status_of("keep.txt"), Some('M'), "base file edited on the branch is Modified");
		assert_eq!(status_of("gone.txt"), Some('D'), "deletion keeps its path");
		let paths: Vec<String> = rows.iter().map(|(_, p)| p.clone()).collect();
		assert_eq!(paths, changed_files_against(&wt, "main").unwrap(), "parity with --name-only");

		let _ = std::fs::remove_dir_all(&root);
	}

	// The rows are parsed from raw porcelain, so pin the two shapes git actually
	// emits — one-path lines, and rename/copy lines whose SECOND path is the one
	// that exists on the branch (taking the first would scope the mutation diff to
	// a path that no longer exists).
	#[test]
	fn parse_name_status_takes_the_branch_side_path() {
		let rows = parse_name_status("A\tnew.rs\nM\tsrc/old.rs\nD\tdead.rs\nR100\twas.rs\tis.rs\nC75\tsrc/a.rs\tsrc/b.rs\n");
		assert_eq!(
			rows,
			vec![
				('A', "new.rs".to_string()),
				('M', "src/old.rs".to_string()),
				('D', "dead.rs".to_string()),
				('R', "is.rs".to_string()),
				('C', "src/b.rs".to_string()),
			],
		);
		// junk in, nothing out — never a phantom row with an empty path
		assert!(parse_name_status("").is_empty(), "empty output → no rows");
		assert!(parse_name_status("\n\nM\t\n").is_empty(), "blank lines and empty paths dropped");
	}

	// stamped_on_base resolves the provenance stamp AGAINST BASE HISTORY, never
	// the worktree filesystem — the load-bearing security property of verify's
	// oracle-freeze exemption: a stamp the worker drops in its own worktree (or
	// even commits on its branch) must NOT unfreeze anything.
	#[test]
	fn stamped_on_base_trusts_base_history_only() {
		let root = init_repo("harness-git-stampbase");
		// base: gen/ carries a stamp (files at two depths under it); naked/ does not
		std::fs::create_dir_all(root.join("gen/deep")).unwrap();
		std::fs::write(root.join("gen").join(TEMPLATE_STAMP), "{}").unwrap();
		std::fs::write(root.join("gen/test_gen.py"), "def test_g(): pass\n").unwrap();
		std::fs::write(root.join("gen/deep/test_leaf.py"), "def test_l(): pass\n").unwrap();
		std::fs::create_dir_all(root.join("naked")).unwrap();
		std::fs::write(root.join("naked/test_plain.py"), "def test_p(): pass\n").unwrap();
		sh(&root, &["add", "-A"]);
		sh(&root, &["commit", "-q", "-m", "stamped base"]);

		let wt = ensure_worktree(&root, "sb").unwrap();
		// stamp in an ancestor dir on base → true, at any depth below the stamp
		assert!(stamped_on_base(&wt, "main", "gen/test_gen.py"), "stamped parent dir on base");
		assert!(stamped_on_base(&wt, "main", "gen/deep/test_leaf.py"), "stamp found at ANY ancestor depth");
		// no stamp anywhere on base → false (dir'd file and root file alike)
		assert!(!stamped_on_base(&wt, "main", "naked/test_plain.py"), "unstamped dir");
		assert!(!stamped_on_base(&wt, "main", "README.md"), "unstamped root file");

		// a stamp that exists ONLY on the worktree branch (fs + branch commit,
		// absent from base) must NOT unfreeze — false both ways
		std::fs::write(wt.join("naked").join(TEMPLATE_STAMP), "{}").unwrap();
		assert!(!stamped_on_base(&wt, "main", "naked/test_plain.py"), "worktree-fs stamp ignored");
		commit_worktree(&wt, "sb").unwrap();
		assert!(!stamped_on_base(&wt, "main", "naked/test_plain.py"), "branch-committed stamp ignored");

		let _ = std::fs::remove_dir_all(&root);
	}

	// the root case of the ancestor walk: a stamp committed at the REPO ROOT on
	// base covers every file in the tree (spec is `<base>:.template-stamp.json`,
	// with no leading dir component).
	#[test]
	fn stamped_on_base_finds_repo_root_stamp() {
		let root = init_repo("harness-git-stamproot");
		std::fs::write(root.join(TEMPLATE_STAMP), "{}").unwrap();
		std::fs::write(root.join("test_top.py"), "def test_t(): pass\n").unwrap();
		std::fs::create_dir_all(root.join("pkg")).unwrap();
		std::fs::write(root.join("pkg/test_sub.py"), "def test_s(): pass\n").unwrap();
		sh(&root, &["add", "-A"]);
		sh(&root, &["commit", "-q", "-m", "root stamp"]);

		let wt = ensure_worktree(&root, "sr").unwrap();
		assert!(stamped_on_base(&wt, "main", "test_top.py"), "root stamp covers root files");
		assert!(stamped_on_base(&wt, "main", "pkg/test_sub.py"), "root stamp covers the whole tree");

		let _ = std::fs::remove_dir_all(&root);
	}

	// ls_files reports tracked paths repo-relative (what draft grounds against) and
	// ignores untracked churn; outside a repo it errors so the caller can degrade.
	#[test]
	fn ls_files_lists_tracked_paths_only() {
		let root = init_repo("harness-git-lsfiles");
		std::fs::create_dir_all(root.join("src")).unwrap();
		std::fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();
		sh(&root, &["add", "-A"]);
		sh(&root, &["commit", "-q", "-m", "src"]);
		std::fs::write(root.join("untracked.txt"), "churn\n").unwrap();

		let files = ls_files(&root).unwrap();
		assert!(files.contains(&"src/main.rs".to_string()), "repo-relative, not absolute");
		assert!(files.contains(&"README.md".to_string()));
		assert!(!files.contains(&"untracked.txt".to_string()), "untracked churn excluded");

		let outside = std::env::temp_dir().join("harness-git-lsfiles-outside");
		let _ = std::fs::remove_dir_all(&outside);
		std::fs::create_dir_all(&outside).unwrap();
		// a non-repo dir errors → run_draft degrades to an empty tree
		assert!(ls_files(&outside).is_err() || ls_files(&outside).unwrap().is_empty());

		let _ = std::fs::remove_dir_all(&outside);
		let _ = std::fs::remove_dir_all(&root);
	}

	// The draft-grounding data path exactly as run_draft composes it, on a REAL repo:
	// ls_files → prompt (tree section carries the real paths) → ungrounded flag (an
	// invented sibling is caught; the real files and their dirs are not). Covers the
	// glue end-to-end minus the provider call and the println.
	#[test]
	fn ls_files_grounds_the_draft_prompt_and_flags_invented_paths() {
		use crate::draft;
		let root = init_repo("harness-git-draftgrounding");
		std::fs::create_dir_all(root.join("src")).unwrap();
		std::fs::write(root.join("src/lexer.rs"), "pub fn lex() {}\n").unwrap();
		sh(&root, &["add", "-A"]);
		sh(&root, &["commit", "-q", "-m", "lexer"]);

		let tree = ls_files(&root).unwrap();
		let seed = draft::Seed {
			kind: "build",
			title: "add an lru cache to the lexer",
			notes: None,
			plan: None,
			acceptance_criteria: None,
			validation: None,
		};
		let (_, user) = draft::build_draft_messages(&seed, &[], &tree);
		assert!(user.contains("Repository tree (repo-relative, 3 of 3 files):"));
		// title-relevant file leads the section; every real path is offered
		assert!(user.contains("src/lexer.rs") && user.contains("README.md"));

		// a draft naming a real file + a real dir + an invented one flags only the invented
		let flagged = draft::ungrounded_paths(
			&["1. Edit `src/lexer.rs` under src/; add the cache in src/cache/lru.rs"],
			&tree,
		);
		assert_eq!(flagged, vec!["src/cache/lru.rs"], "invented path flagged; real file + dir grounded");

		let _ = std::fs::remove_dir_all(&root);
	}

	// a dirty base tree refuses to land (don't squash onto uncommitted base work).
	#[test]
	fn squash_merge_refuses_dirty_base() {
		let root = init_repo("harness-git-dirtybase");
		let wt = ensure_worktree(&root, "t3").unwrap();
		std::fs::write(wt.join("f.txt"), "x\n").unwrap();
		commit_worktree(&wt, "t3").unwrap();
		// dirty the base (tracked file)
		std::fs::write(root.join("README.md"), "dirtied\n").unwrap();
		assert!(squash_merge(&root, "t3", "t").is_err(), "dirty base must refuse");

		let _ = std::fs::remove_dir_all(&root);
	}

	// advance base by one commit (the "wiki commit landed mid-sprint" shape).
	fn advance_base(root: &Path) {
		std::fs::write(root.join("README.md"), "base advanced\n").unwrap();
		sh(root, &["add", "-A"]);
		sh(root, &["commit", "-q", "-m", "base advance"]);
	}

	// t9 fork-point guard, surviving-branch path: a harness/<id> branch left behind
	// by a previous attempt, forked BEHIND base HEAD, is REFUSED — the error names
	// the branch, the fork sha, the commits-behind count, and the remedies, and no
	// worktree is created or attached.
	#[test]
	fn stale_fork_surviving_branch_is_refused_with_fork_sha() {
		let root = init_repo("harness-git-stalefork");
		let fork_sha = head(&root);

		// attempt 1: branch + worktree off base HEAD, with unlanded work; then the
		// worktree dir goes away but the branch survives (crash / manual cleanup)
		let wt = ensure_worktree(&root, "sf").unwrap();
		std::fs::write(wt.join("w.txt"), "unlanded\n").unwrap();
		commit_worktree(&wt, "sf").unwrap();
		sh(&root, &["worktree", "remove", "--force", &wt.to_string_lossy()]);
		assert!(git_ok(&root, &["rev-parse", "--verify", "harness/sf"]), "branch survives");

		advance_base(&root);

		let err = ensure_worktree_with(&root, "sf", false).unwrap_err().to_string();
		assert!(err.contains("harness/sf"), "names the branch: {err}");
		assert!(err.contains(&fork_sha), "names the fork sha: {err}");
		assert!(err.contains("1 commit(s) behind"), "names the behind count: {err}");
		assert!(err.contains("stale/sf") && err.contains("rework sf"), "names the remedies: {err}");
		assert!(!wt.exists(), "refusal must not create or attach a worktree");

		let _ = std::fs::remove_dir_all(&root);
	}

	// t9 fork-point guard, escape hatch: allow_stale=true (the testable side of
	// HARNESS_ALLOW_STALE_FORK=1) downgrades the same refusal to a warning and
	// attaches the worktree to the stale branch — its unlanded work rides along.
	#[test]
	fn stale_fork_escape_hatch_attaches_the_old_branch() {
		let root = init_repo("harness-git-stalehatch");
		let wt = ensure_worktree(&root, "sh").unwrap();
		std::fs::write(wt.join("w.txt"), "unlanded\n").unwrap();
		commit_worktree(&wt, "sh").unwrap();
		sh(&root, &["worktree", "remove", "--force", &wt.to_string_lossy()]);
		advance_base(&root);

		let wt2 = ensure_worktree_with(&root, "sh", true).unwrap();
		assert!(wt2.exists(), "escape hatch proceeds");
		assert!(wt2.join("w.txt").exists(), "the branch's unlanded work is back");

		let _ = std::fs::remove_dir_all(&root);
	}

	// t9 fork-point guard, existing-worktree path: a worktree dir that already sits
	// there is ALSO guarded — base advancing under a live worktree refuses the
	// resume unless the hatch is set (which then reuses the same dir).
	#[test]
	fn stale_fork_existing_worktree_is_refused_too() {
		let root = init_repo("harness-git-stalewt");
		let wt = ensure_worktree(&root, "sw").unwrap();
		advance_base(&root);

		assert!(ensure_worktree_with(&root, "sw", false).is_err(), "existing-worktree reuse guarded");
		assert!(wt.exists(), "the refused worktree itself is left untouched");
		assert_eq!(ensure_worktree_with(&root, "sw", true).unwrap(), wt, "hatch reuses the same dir");

		let _ = std::fs::remove_dir_all(&root);
	}

	// t9 fork-point guard, the unchanged paths: fresh create forks off current base
	// HEAD, and reuse with fork point == base HEAD (normal resume / additive-retry
	// reattach) proceeds exactly as before — no hatch needed.
	#[test]
	fn fresh_create_and_up_to_date_reuse_pass_the_guard() {
		let root = init_repo("harness-git-freshfork");
		let base = head(&root);

		// fresh create: branch forks off current HEAD
		let wt = ensure_worktree_with(&root, "ff", false).unwrap();
		assert_eq!(git(&root, &["merge-base", "harness/ff", "HEAD"]).unwrap(), base, "forked off HEAD");

		// existing-worktree resume, fork == base HEAD → reused (even with work on it)
		std::fs::write(wt.join("w.txt"), "work\n").unwrap();
		commit_worktree(&wt, "ff").unwrap();
		assert_eq!(ensure_worktree_with(&root, "ff", false).unwrap(), wt, "up-to-date resume reused");

		// surviving-branch reattach, fork == base HEAD → attached as before
		sh(&root, &["worktree", "remove", "--force", &wt.to_string_lossy()]);
		let wt2 = ensure_worktree_with(&root, "ff", false).unwrap();
		assert!(wt2.join("w.txt").exists(), "up-to-date surviving branch reattached with its work");

		let _ = std::fs::remove_dir_all(&root);
	}
}
