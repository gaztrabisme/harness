//! `pi prepare`: the four local launcher steps (seed the agent dir, render
//! models.json, the project-root guard, wiki init) as ONE implementation, so a
//! pi session can be prepared on macOS, Linux and Windows from this binary
//! alone. Ported from the Terax app's modules/pi/launcher.rs; the wiki bodies
//! stay byte-equal to bin/wiki-init (pinned against a fixture of the script's
//! real output). Health probes of bppc and oMLX stay out of this unit, exactly
//! as in launcher.rs; the caller composes them separately.
//!
//! CLI (dispatched in main.rs BEFORE the board opens, so a launcher call from
//! any cwd leaves no harness-board.db side-files behind):
//!
//!   agent pi prepare --template <dir> --agent-dir <dir> --cwd <dir>
//!                    [--stamp <string>] [--version <string>]
//!                    [--bppc-host <host>] [--omlx-key-env <VAR>]
//!                    [--allow-any-dir] [--no-wiki] [--json]
//!
//! Steps in launcher order: seed, render, root, wiki. Each prints
//! `[k/4] name ... OK|FAIL|SKIPPED (detail)` on stderr; `--json` adds one
//! object on stdout. Exit 0 when every step is OK or SKIPPED, 9 on the root
//! guard, 6 on any other failure, 2 on a usage error.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// Entries the seed owns in the agent dir; everything else (auth.json,
/// models.json, mcp.json, sessions/, logs/, wiki/, agent-hub/,
/// tool-output-artifacts/) belongs to the user or the runtime and is never
/// written by the seed.
const MANAGED_FILES: &[&str] = &[
    "AGENTS.md",
    "settings.json",
    "settings.README.md",
    "models.json.tmpl",
];
const MANAGED_DIRS: &[&str] = &["agents", "extensions", "prompts", "skills"];
/// Stamp file in the agent dir, same name the app uses today: keeping it lets
/// the app and the bash launcher become callers (unit D4b) without a spurious
/// re-seed of already-seeded agent dirs.
const SEED_STAMP_FILE: &str = ".terax-seed";

/// FNV-1a 64-bit: tiny, dependency-free, stable across platforms; good enough
/// to detect template drift between versions.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv1a(hash: &mut u64, bytes: &[u8]) {
    for &b in bytes {
        *hash ^= u64::from(b);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}

/// FNV-1a over the managed template entries (relative name, then bytes),
/// sorted so the hash is stable across directory-listing orders.
fn template_hash(template: &Path) -> u64 {
    let mut hash = FNV_OFFSET;
    for name in MANAGED_FILES {
        hash_entry(&mut hash, template, Path::new(name));
    }
    for name in MANAGED_DIRS {
        hash_entry(&mut hash, template, Path::new(name));
    }
    hash
}

fn hash_entry(hash: &mut u64, template: &Path, rel: &Path) {
    let full = template.join(rel);
    if full.is_dir() {
        let mut children: Vec<PathBuf> = match fs::read_dir(&full) {
            Ok(entries) => entries.filter_map(Result::ok).map(|e| e.path()).collect(),
            Err(_) => {
                fnv1a(hash, b"<missing-dir>\0");
                return;
            }
        };
        children.sort();
        for child in children {
            let child_rel = rel.join(child.file_name().unwrap_or_default());
            hash_entry(hash, template, &child_rel);
        }
    } else {
        fnv1a(hash, rel.to_string_lossy().as_bytes());
        fnv1a(hash, b"\0");
        match fs::read(&full) {
            Ok(bytes) => fnv1a(hash, &bytes),
            Err(_) => fnv1a(hash, b"<missing>"),
        }
    }
}

/// Stamp shaped `<version>+<hash:016x>` (launcher.rs's format).
pub fn template_stamp(app_version: &str, template: &Path) -> String {
    format!("{app_version}+{:016x}", template_hash(template))
}

/// The default seed stamp: the FNV hash of the template tree, prefixed by the
/// optional `--version` when one is given.
fn default_stamp(version: Option<&str>, template: &Path) -> String {
    match version.map(str::trim).filter(|v| !v.is_empty()) {
        Some(v) => template_stamp(v, template),
        None => format!("{:016x}", template_hash(template)),
    }
}

#[derive(Debug, Default, Clone)]
pub struct SeedReport {
    /// Managed entries that did not exist in dest and were copied.
    pub created: Vec<String>,
    /// Managed entries that existed but differed and were re-copied.
    pub updated: Vec<String>,
    /// Managed entries already identical to the template, left untouched.
    pub kept: Vec<String>,
}

impl SeedReport {
    fn is_empty(&self) -> bool {
        self.created.is_empty() && self.updated.is_empty() && self.kept.is_empty()
    }
}

/// First run copies the whole managed template; later runs re-copy only when
/// the stamp differs, and then only the managed set. User files are never
/// touched. The stamp is written last, so a crash mid-seed leaves the old
/// stamp behind and the next run re-seeds.
pub fn seed_agent_dir(template: &Path, dest: &Path, stamp: &str) -> io::Result<SeedReport> {
    if fs::read_to_string(dest.join(SEED_STAMP_FILE)).is_ok_and(|s| s == stamp) {
        return Ok(SeedReport::default());
    }
    if !template.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("agent dir template not found: {}", template.display()),
        ));
    }
    fs::create_dir_all(dest)?;
    let mut report = SeedReport::default();
    for name in MANAGED_FILES {
        seed_file(template, dest, name, &mut report)?;
    }
    for name in MANAGED_DIRS {
        seed_dir(template, dest, name, &mut report)?;
    }
    fs::write(dest.join(SEED_STAMP_FILE), stamp)?;
    Ok(report)
}

fn seed_file(template: &Path, dest: &Path, name: &str, report: &mut SeedReport) -> io::Result<()> {
    let src = template.join(name);
    if !src.is_file() {
        return Ok(());
    }
    let dst = dest.join(name);
    let content = fs::read(&src)?;
    let existed = dst.exists();
    if existed && fs::read(&dst).is_ok_and(|d| d == content) {
        report.kept.push(name.to_string());
        return Ok(());
    }
    copy_file(&src, &dst)?;
    if existed {
        report.updated.push(name.to_string());
    } else {
        report.created.push(name.to_string());
    }
    Ok(())
}

fn seed_dir(template: &Path, dest: &Path, name: &str, report: &mut SeedReport) -> io::Result<()> {
    let src = template.join(name);
    if !src.is_dir() {
        return Ok(());
    }
    let mut files = Vec::new();
    collect_files(&src, Path::new(""), &mut files)?;
    files.sort();
    let dst_root = dest.join(name);
    if !dst_root.is_dir() {
        fs::create_dir_all(&dst_root)?;
        for rel in &files {
            copy_file(&src.join(rel), &dst_root.join(rel))?;
        }
        report.created.push(format!("{name}/"));
        return Ok(());
    }
    let mut stale = Vec::new();
    for rel in &files {
        let src_path = src.join(rel);
        let dst_path = dst_root.join(rel);
        match (fs::read(&src_path), fs::read(&dst_path)) {
            (Ok(s), Ok(d)) if s == d => {}
            (Ok(s), _) => stale.push((src_path, dst_path, s)),
            (Err(e), _) => return Err(e),
        }
    }
    if stale.is_empty() {
        report.kept.push(format!("{name}/"));
        return Ok(());
    }
    for (src_path, dst_path, _) in stale {
        copy_file(&src_path, &dst_path)?;
    }
    report.updated.push(format!("{name}/"));
    Ok(())
}

/// Copies while preserving the source permissions (skills carry scripts).
fn copy_file(src: &Path, dst: &Path) -> io::Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(src, dst).map(|_| ())
}

fn collect_files(root: &Path, rel: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(root.join(rel))? {
        let entry = entry?;
        let child = rel.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            collect_files(root, &child, out)?;
        } else {
            out.push(child);
        }
    }
    Ok(())
}

/// Placeholders from bin/pi-render-models: the bppc LAN host varies by
/// network and the oMLX key is secret.
const BPPC_HOST_PLACEHOLDER: &str = "__BPPC_HOST__";
const OMLX_KEY_PLACEHOLDER: &str = "__OMLX_KEY__";
/// Blank host: the local machine, so a fresh install never points at another box.
const BPPC_HOST_LAN: &str = "127.0.0.1";

/// Same substitution as bin/pi-render-models: replaces every placeholder with
/// the given values, byte for byte (no sed escaping involved).
pub fn render_models_json(tmpl: &str, bppc_host: &str, omlx_key: &str) -> String {
    tmpl.replace(BPPC_HOST_PLACEHOLDER, bppc_host)
        .replace(OMLX_KEY_PLACEHOLDER, omlx_key)
}

/// Writes <agent_dir>/models.json only when the rendered content differs from
/// what is on disk; returns whether it wrote.
pub fn write_models_json(agent_dir: &Path, content: &str) -> io::Result<bool> {
    let path = agent_dir.join("models.json");
    if fs::read(&path).is_ok_and(|d| d.as_slice() == content.as_bytes()) {
        return Ok(false);
    }
    fs::create_dir_all(agent_dir)?;
    fs::write(&path, content)?;
    Ok(true)
}

/// Wiki skeleton files written by bin/wiki-init. Byte-equal to the script's
/// heredocs (the em-dashes are its output, spelled as escapes here); a test
/// pins the bytes against a fixture of the script's real output.
const WIKI_INDEX: &str = "# Wiki Index
- [active-work.md](active-work.md) \u{2014} current workstreams, status, next steps
- [decisions.md](decisions.md) \u{2014} choices made, rejected options, why
- [log.md](log.md) \u{2014} dated session journal (grep it, never read wholesale)
";
const WIKI_ACTIVE_WORK: &str = "# Active Work\n\n(no open workstreams)\n";
const WIKI_DECISIONS: &str = "# Decisions\n";
const WIKI_LOG: &str = "# Wiki Log\n";

const WIKI_FILES: &[(&str, &str)] = &[
    ("index.md", WIKI_INDEX),
    ("active-work.md", WIKI_ACTIVE_WORK),
    ("decisions.md", WIKI_DECISIONS),
    ("log.md", WIKI_LOG),
];

/// Port of bin/wiki-init: creates wiki/index.md, active-work.md, decisions.md
/// and log.md under the project root, each only when missing, and returns the
/// files it created.
pub fn wiki_init(project_root: &Path) -> io::Result<Vec<PathBuf>> {
    let wiki = project_root.join("wiki");
    fs::create_dir_all(&wiki)?;
    let mut created = Vec::new();
    for (name, body) in WIKI_FILES {
        let path = wiki.join(name);
        if path.exists() {
            continue;
        }
        fs::write(&path, body)?;
        created.push(path);
    }
    Ok(created)
}

/// The launcher's project-root guard: the cwd must look like a project (.git
/// of any kind, CLAUDE.md, AGENTS.md, or wiki/). The Err text mirrors the
/// launcher's refusal message.
pub fn project_root_check(cwd: &Path) -> Result<(), String> {
    let is_root = cwd.join(".git").exists()
        || cwd.join("CLAUDE.md").is_file()
        || cwd.join("AGENTS.md").is_file()
        || cwd.join("wiki").is_dir();
    if is_root {
        Ok(())
    } else {
        Err(format!(
            "{} has no .git, CLAUDE.md, AGENTS.md, or wiki/; run from inside a project directory or allow any dir",
            cwd.display()
        ))
    }
}

/// OMLX key default, the same fallback the bash launcher uses when the env
/// carries none: `auth.api_key` from `<home>/.omlx/settings.json`. None when
/// the file is missing, malformed or the key blank, so render_step's own
/// failure message reports the gap. The key never leaves this function except
/// as the return value; nothing prints it.
pub fn omlx_key_default(home: Option<&str>) -> Option<String> {
    let home = home.map(str::trim).filter(|s| !s.is_empty())?;
    let path = Path::new(home).join(".omlx").join("settings.json");
    let raw = fs::read_to_string(path).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let key = parsed
        .get("auth")?
        .get("api_key")?
        .as_str()?
        .trim()
        .to_string();
    (!key.is_empty()).then_some(key)
}

/// The CLI's key resolution: the env var named by `--omlx-key-env` first, then
/// the settings fallback. Pure on its arguments (the caller reads the process
/// environment) so both branches are unit-testable without env mutation.
fn resolve_omlx_key(explicit: Option<String>, home: Option<&str>) -> Option<String> {
    match explicit {
        Some(k) if !k.trim().is_empty() => Some(k),
        _ => omlx_key_default(home),
    }
}

/// True when the two paths name the same directory: textual equality first,
/// then canonicalization for the differing-spelling case (symlinks, `..`).
/// A missing dir only matches textually, which is the honest answer for the
/// checkout case (the template must exist for anything else to work).
fn same_dir(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    Ok,
    Fail,
    Skipped,
}

impl StepStatus {
    /// One vocabulary for the stderr lines and the JSON object.
    fn label(self) -> &'static str {
        match self {
            StepStatus::Ok => "OK",
            StepStatus::Fail => "FAIL",
            StepStatus::Skipped => "SKIPPED",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Step {
    pub name: &'static str,
    pub status: StepStatus,
    pub detail: String,
}

/// Everything prepare() needs; the CLI builds it, unit tests build it directly
/// (so the step logic is testable without process-env or stdout coupling).
pub struct PrepareOptions {
    pub template: PathBuf,
    pub agent_dir: PathBuf,
    pub cwd: PathBuf,
    /// Explicit seed stamp; wins over the default (hash, optionally
    /// `--version`-prefixed).
    pub stamp: Option<String>,
    pub version: Option<String>,
    pub bppc_host: String,
    /// The resolved oMLX key, or None to fail the render step (the key value
    /// is never included in any detail line).
    pub omlx_key: Option<String>,
    /// Env var name the key was resolved from, for the failure message only.
    pub omlx_key_var: String,
    pub allow_any_dir: bool,
    pub no_wiki: bool,
}

#[derive(Debug, Clone)]
pub struct PrepareOutcome {
    pub steps: Vec<Step>,
    pub agent_dir: PathBuf,
    pub models_json: PathBuf,
}

/// Composes seed, render, root guard and wiki init in launcher step order.
/// Every step reports its own outcome; a failed step never aborts the rest,
/// so the caller sees the full picture in one round-trip.
pub fn prepare(opts: PrepareOptions) -> PrepareOutcome {
    let mut steps = Vec::new();

    // Step 1: seed. Template equal to the agent dir is the checkout case: the
    // dir is used in place, nothing is copied and no stamp is written.
    if same_dir(&opts.template, &opts.agent_dir) {
        steps.push(Step {
            name: "seed",
            status: StepStatus::Skipped,
            detail: format!("agent dir used in place: {}", opts.agent_dir.display()),
        });
    } else {
        let stamp = opts
            .stamp
            .clone()
            .unwrap_or_else(|| default_stamp(opts.version.as_deref(), &opts.template));
        steps.push(match seed_agent_dir(&opts.template, &opts.agent_dir, &stamp) {
            Ok(report) if report.is_empty() => Step {
                name: "seed",
                status: StepStatus::Ok,
                detail: format!("agent dir ready at {}", opts.agent_dir.display()),
            },
            Ok(report) => Step {
                name: "seed",
                status: StepStatus::Ok,
                detail: format!(
                    "seeded {}: {} created, {} updated, {} kept",
                    opts.agent_dir.display(),
                    report.created.len(),
                    report.updated.len(),
                    report.kept.len()
                ),
            },
            Err(e) => Step {
                name: "seed",
                status: StepStatus::Fail,
                detail: e.to_string(),
            },
        });
    }

    // Step 2: render pi-home agent models.json from the seeded template.
    steps.push(render_step(&opts));

    // Step 3: the project-root guard.
    steps.push(if opts.allow_any_dir {
        Step {
            name: "root",
            status: StepStatus::Skipped,
            detail: format!("root guard skipped (--allow-any-dir): {}", opts.cwd.display()),
        }
    } else {
        match project_root_check(&opts.cwd) {
            Ok(()) => Step {
                name: "root",
                status: StepStatus::Ok,
                detail: format!("project root: {}", opts.cwd.display()),
            },
            Err(msg) => Step {
                name: "root",
                status: StepStatus::Fail,
                detail: msg,
            },
        }
    });

    // Step 4: wiki init.
    steps.push(if opts.no_wiki {
        Step {
            name: "wiki",
            status: StepStatus::Skipped,
            detail: "wiki init skipped (--no-wiki)".to_string(),
        }
    } else {
        match wiki_init(&opts.cwd) {
            Ok(created) if created.is_empty() => Step {
                name: "wiki",
                status: StepStatus::Ok,
                detail: "wiki files present".to_string(),
            },
            Ok(created) => {
                let names = created
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                Step {
                    name: "wiki",
                    status: StepStatus::Ok,
                    detail: format!("created {names}"),
                }
            }
            Err(e) => Step {
                name: "wiki",
                status: StepStatus::Fail,
                detail: e.to_string(),
            },
        }
    });

    PrepareOutcome {
        models_json: opts.agent_dir.join("models.json"),
        agent_dir: opts.agent_dir,
        steps,
    }
}

/// Render: models.json from <agent_dir>/models.json.tmpl. A missing key fails
/// like the bash launcher; a blank bppc host falls back to the LAN default.
fn render_step(opts: &PrepareOptions) -> Step {
    let name = "render";
    let tmpl_path = opts.agent_dir.join("models.json.tmpl");
    let tmpl = match fs::read_to_string(&tmpl_path) {
        Ok(t) => t,
        Err(e) => {
            return Step {
                name,
                status: StepStatus::Fail,
                detail: format!("cannot read {}: {e}", tmpl_path.display()),
            };
        }
    };
    let key = opts.omlx_key.as_deref().map(str::trim).filter(|k| !k.is_empty());
    let Some(key) = key else {
        return Step {
            name,
            status: StepStatus::Fail,
            detail: format!(
                "{} not set and no auth.api_key in $HOME/.omlx/settings.json; cannot render models.json",
                opts.omlx_key_var
            ),
        };
    };
    let host = if opts.bppc_host.trim().is_empty() {
        BPPC_HOST_LAN
    } else {
        opts.bppc_host.trim()
    };
    let rendered = render_models_json(&tmpl, host, key);
    match write_models_json(&opts.agent_dir, &rendered) {
        Ok(true) => Step {
            name,
            status: StepStatus::Ok,
            detail: format!("wrote models.json (bppc host {host})"),
        },
        Ok(false) => Step {
            name,
            status: StepStatus::Ok,
            detail: format!("models.json unchanged (bppc host {host})"),
        },
        Err(e) => Step {
            name,
            status: StepStatus::Fail,
            detail: e.to_string(),
        },
    }
}

/// Exit-code contract: 0 when every step is OK or SKIPPED, 9 on the root
/// guard, 6 on any other failure.
fn exit_code(steps: &[Step]) -> i32 {
    if steps.iter().any(|s| s.name == "root" && s.status == StepStatus::Fail) {
        9
    } else if steps.iter().any(|s| s.status == StepStatus::Fail) {
        6
    } else {
        0
    }
}

const USAGE: &str = "usage: agent pi prepare --template <dir> --agent-dir <dir> --cwd <dir> \
[--stamp <string>] [--version <string>] [--bppc-host <host>] [--omlx-key-env <VAR>] \
[--allow-any-dir] [--no-wiki] [--json]";

struct ParsedCli {
    template: PathBuf,
    agent_dir: PathBuf,
    cwd: PathBuf,
    stamp: Option<String>,
    version: Option<String>,
    bppc_host: String,
    omlx_key_env: String,
    allow_any_dir: bool,
    no_wiki: bool,
    json: bool,
}

fn next_value(args: &[String], i: usize) -> Option<String> {
    args.get(i + 1).filter(|s| !s.is_empty()).cloned()
}

fn parse_flags(args: &[String]) -> Option<ParsedCli> {
    let mut out = ParsedCli {
        template: PathBuf::new(),
        agent_dir: PathBuf::new(),
        cwd: PathBuf::new(),
        stamp: None,
        version: None,
        bppc_host: String::new(),
        omlx_key_env: "OMLX_API_KEY".to_string(),
        allow_any_dir: false,
        no_wiki: false,
        json: false,
    };
    let mut i = 3; // args: [0]=binary, [1]=pi, [2]=prepare
    while i < args.len() {
        let takes_value = !matches!(args[i].as_str(), "--allow-any-dir" | "--no-wiki" | "--json");
        match args[i].as_str() {
            "--template" => out.template = PathBuf::from(next_value(args, i)?),
            "--agent-dir" => out.agent_dir = PathBuf::from(next_value(args, i)?),
            "--cwd" => out.cwd = PathBuf::from(next_value(args, i)?),
            "--stamp" => out.stamp = Some(next_value(args, i)?),
            "--version" => out.version = Some(next_value(args, i)?),
            "--bppc-host" => out.bppc_host = next_value(args, i)?,
            "--omlx-key-env" => out.omlx_key_env = next_value(args, i)?,
            "--allow-any-dir" => out.allow_any_dir = true,
            "--no-wiki" => out.no_wiki = true,
            "--json" => out.json = true,
            _ => return None,
        }
        i += if takes_value { 2 } else { 1 };
    }
    if out.template.as_os_str().is_empty()
        || out.agent_dir.as_os_str().is_empty()
        || out.cwd.as_os_str().is_empty()
    {
        return None;
    }
    Some(out)
}

/// The `agent pi prepare` entry point, called from main()'s dispatch before
/// the board opens. Parses the flags, runs the four steps, prints the report
/// and returns the process exit code (0, 6, 9, or 2 on a usage error).
pub fn cli(args: &[String]) -> i32 {
    if args.get(2).map(String::as_str) != Some("prepare") {
        eprintln!("{USAGE}");
        return 2;
    }
    let Some(parsed) = parse_flags(args) else {
        eprintln!("{USAGE}");
        return 2;
    };
    // Key resolution is the CLI's job (it owns the process environment):
    // --omlx-key-env VAR first, then the oMLX settings fallback. The value is
    // carried in memory only and never printed, in any mode.
    let explicit = std::env::var(&parsed.omlx_key_env).ok();
    let home = std::env::var("HOME").ok().or_else(|| std::env::var("USERPROFILE").ok());
    let omlx_key = resolve_omlx_key(explicit, home.as_deref());
    let outcome = prepare(PrepareOptions {
        template: parsed.template,
        agent_dir: parsed.agent_dir,
        cwd: parsed.cwd,
        stamp: parsed.stamp,
        version: parsed.version,
        bppc_host: parsed.bppc_host,
        omlx_key_var: parsed.omlx_key_env,
        omlx_key,
        allow_any_dir: parsed.allow_any_dir,
        no_wiki: parsed.no_wiki,
    });

    for (i, step) in outcome.steps.iter().enumerate() {
        eprintln!("[{}/4] {} ... {} ({})", i + 1, step.name, step.status.label(), step.detail);
    }
    if parsed.json {
        // Paths and step outcomes only: models.json's rendered CONTENT carries
        // the key, so the JSON object reports its path, never its body.
        let steps = outcome
            .steps
            .iter()
            .map(|s| json!({ "name": s.name, "status": s.status.label(), "detail": s.detail }))
            .collect::<Vec<Value>>();
        let agent_dir = outcome.agent_dir.to_string_lossy().into_owned();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "steps": steps,
                "agentDir": agent_dir,
                "modelsJson": outcome.models_json.to_string_lossy(),
                "env": { "PI_CODING_AGENT_DIR": agent_dir },
            }))
            .expect("pi prepare report serializes")
        );
    }
    exit_code(&outcome.steps)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fresh dir per case, namespaced by pid + test name (cli_dispatch.rs's
    /// pattern) so parallel runs never collide.
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("agent-pi-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("mkdir");
        }
        fs::write(path, content).expect("write");
    }

    /// The managed template the seed tests share.
    fn template(name: &str) -> PathBuf {
        let root = temp_dir(name);
        write(&root.join("AGENTS.md"), "# agent\n");
        write(&root.join("settings.json"), "{}\n");
        write(&root.join("settings.README.md"), "docs\n");
        write(
            &root.join("models.json.tmpl"),
            r#"{"baseUrl": "http://__BPPC_HOST__:8080/v1", "apiKey": "__OMLX_KEY__"}"#,
        );
        write(&root.join("agents/worker.md"), "# worker\n");
        write(&root.join("extensions/board.mjs"), "export {};\n");
        write(&root.join("prompts/brief.md"), "# brief\n");
        write(&root.join("skills/dev/SKILL.md"), "# dev\n");
        root
    }

    fn managed_entry_names() -> Vec<String> {
        let mut names: Vec<String> = MANAGED_FILES
            .iter()
            .map(|s| s.to_string())
            .chain(MANAGED_DIRS.iter().map(|s| format!("{s}/")))
            .collect();
        names.sort();
        names
    }

    fn seed_report_names(report: &SeedReport) -> Vec<String> {
        let mut names = report.created.clone();
        names.extend(report.updated.iter().cloned());
        names.extend(report.kept.iter().cloned());
        names.sort();
        names
    }

    fn prepare_opts(template: &Path, agent_dir: &Path, cwd: &Path, omlx_key: Option<&str>) -> PrepareOptions {
        PrepareOptions {
            template: template.to_path_buf(),
            agent_dir: agent_dir.to_path_buf(),
            cwd: cwd.to_path_buf(),
            stamp: None,
            version: None,
            bppc_host: String::new(),
            omlx_key: omlx_key.map(str::to_owned),
            omlx_key_var: "OMLX_API_KEY".to_string(),
            allow_any_dir: false,
            no_wiki: false,
        }
    }

    fn step_named<'a>(outcome: &'a PrepareOutcome, name: &str) -> &'a Step {
        outcome
            .steps
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("step {name} missing"))
    }

    // --- seed ---

    #[test]
    fn seed_first_run_copies_everything_and_writes_the_stamp() {
        let tmpl = template("seed-first");
        let dest = temp_dir("seed-first-dest");
        let report = seed_agent_dir(&tmpl, &dest, "v1+abc").expect("seed");
        assert_eq!(seed_report_names(&report), managed_entry_names());
        assert!(report.kept.is_empty());
        assert_eq!(
            fs::read_to_string(dest.join(SEED_STAMP_FILE)).expect("stamp"),
            "v1+abc"
        );
        assert_eq!(fs::read_to_string(dest.join("AGENTS.md")).expect("agents.md"), "# agent\n");
        assert_eq!(
            fs::read_to_string(dest.join("skills/dev/SKILL.md")).expect("skill"),
            "# dev\n"
        );
    }

    #[test]
    fn seed_second_run_with_same_stamp_changes_nothing() {
        let tmpl = template("seed-second");
        let dest = temp_dir("seed-second-dest");
        let stamp = template_stamp("1.2.3", &tmpl);
        seed_agent_dir(&tmpl, &dest, &stamp).expect("first seed");
        let agents_before = fs::read(dest.join("AGENTS.md")).expect("read");
        let report = seed_agent_dir(&tmpl, &dest, &stamp).expect("second seed");
        assert!(report.is_empty(), "same stamp must be a no-op");
        assert_eq!(fs::read(dest.join("AGENTS.md")).expect("read"), agents_before);
    }

    #[test]
    fn changed_stamp_recopies_modified_prompt_and_keeps_auth_json() {
        let tmpl = template("seed-changed");
        let dest = temp_dir("seed-changed-dest");
        seed_agent_dir(&tmpl, &dest, "v1").expect("first seed");
        write(&dest.join("auth.json"), r#"{"bppc":"secret"}"#);
        write(&tmpl.join("prompts/brief.md"), "# brief v2\n");
        let report = seed_agent_dir(&tmpl, &dest, "v2").expect("re-seed");
        assert_eq!(report.updated, vec!["prompts/".to_string()]);
        assert!(report.created.is_empty(), "nothing new on re-seed");
        assert_eq!(
            fs::read_to_string(dest.join("prompts/brief.md")).expect("prompt"),
            "# brief v2\n"
        );
        assert_eq!(
            fs::read_to_string(dest.join("auth.json")).expect("auth"),
            r#"{"bppc":"secret"}"#,
            "user files are never touched by a re-seed"
        );
        assert_eq!(fs::read_to_string(dest.join(SEED_STAMP_FILE)).expect("stamp"), "v2");
    }

    #[test]
    fn template_stamp_tracks_version_and_template_content() {
        let tmpl = template("stamp");
        let a = template_stamp("1.0.0", &tmpl);
        let b = template_stamp("1.0.1", &tmpl);
        let c = {
            write(&tmpl.join("prompts/ticket.md"), "# ticket\n");
            template_stamp("1.0.0", &tmpl)
        };
        assert_ne!(a, b, "version must be part of the stamp");
        assert_ne!(a, c, "template drift must change the stamp");
        assert!(a.starts_with("1.0.0+"));
    }

    #[test]
    fn default_stamp_prefixes_the_hash_only_when_a_version_is_given() {
        let tmpl = template("default-stamp");
        let bare = default_stamp(None, &tmpl);
        assert_eq!(bare.len(), 16, "no version: the bare 16-hex hash, got {bare}");
        assert!(bare.chars().all(|c| c.is_ascii_hexdigit()));
        let versioned = default_stamp(Some("0.7.3", ), &tmpl);
        assert!(versioned.starts_with("0.7.3+") && versioned.ends_with(&bare));
        let blank = default_stamp(Some("  "), &tmpl);
        assert_eq!(blank, bare, "a blank version is no version");
    }

    // --- render ---

    #[test]
    fn render_substitutes_both_placeholders() {
        let out = render_models_json(
            r#"{"baseUrl": "http://__BPPC_HOST__:8080/v1", "apiKey": "__OMLX_KEY__"}"#,
            "100.1.2.3",
            "test-key",
        );
        assert_eq!(
            out,
            r#"{"baseUrl": "http://100.1.2.3:8080/v1", "apiKey": "test-key"}"#
        );
    }

    #[test]
    fn write_models_json_skips_identical_content() {
        let dir = temp_dir("write-models");
        assert!(write_models_json(&dir, "{}").expect("write"), "first write lands");
        let first = fs::read(dir.join("models.json")).expect("read");
        assert!(
            !write_models_json(&dir, "{}").expect("second write"),
            "identical content must not rewrite"
        );
        assert_eq!(fs::read(dir.join("models.json")).expect("read"), first);
        assert!(write_models_json(&dir, "[]").expect("third write"), "changed content rewrites");
    }

    // --- wiki init ---

    #[test]
    fn wiki_init_creates_only_missing_files() {
        let project = temp_dir("wiki-missing");
        let first = wiki_init(&project).expect("wiki init");
        assert_eq!(first.len(), 4, "all four files created on a fresh project");
        for path in &first {
            assert!(path.exists(), "{} must exist", path.display());
        }
        assert_eq!(
            fs::read_to_string(project.join("wiki/index.md")).expect("index"),
            "# Wiki Index\n- [active-work.md](active-work.md) \u{2014} current workstreams, status, next steps\n- [decisions.md](decisions.md) \u{2014} choices made, rejected options, why\n- [log.md](log.md) \u{2014} dated session journal (grep it, never read wholesale)\n"
        );
        write(&project.join("wiki/decisions.md"), "user edits\n");
        let second = wiki_init(&project).expect("wiki init again");
        assert!(second.is_empty(), "no file may be overwritten once it exists");
        assert_eq!(
            fs::read_to_string(project.join("wiki/decisions.md")).expect("decisions"),
            "user edits\n"
        );
    }

    /// The fixture under tests/fixtures/wiki-init is a copy of bin/wiki-init's
    /// real output (generated by running the script), so this test fails the
    /// day either side drifts by one byte.
    #[test]
    fn wiki_init_output_is_byte_equal_to_the_bin_wiki_init_fixture() {
        let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("wiki-init");
        let project = temp_dir("wiki-fixture");
        let created = wiki_init(&project).expect("wiki init");
        assert_eq!(created.len(), 4);
        for (name, _) in WIKI_FILES {
            let written = fs::read(project.join("wiki").join(name)).expect("written file");
            let fixture = fs::read(fixture_root.join(name)).expect("fixture file");
            assert_eq!(written, fixture, "{name} must be byte-equal to bin/wiki-init's output");
        }
    }

    // --- root guard ---

    #[test]
    fn project_root_check_passes_on_markers_and_fails_when_empty() {
        let project = temp_dir("root-markers");
        write(&project.join("CLAUDE.md"), "x\n");
        assert!(project_root_check(&project).is_ok());
        let git_project = temp_dir("root-git");
        fs::create_dir(git_project.join(".git")).expect("gitdir");
        assert!(project_root_check(&git_project).is_ok());
        let agents_project = temp_dir("root-agents");
        write(&agents_project.join("AGENTS.md"), "x\n");
        assert!(project_root_check(&agents_project).is_ok());
        let wiki_project = temp_dir("root-wiki");
        fs::create_dir(wiki_project.join("wiki")).expect("wikidir");
        assert!(project_root_check(&wiki_project).is_ok());
        let empty = temp_dir("root-empty");
        let err = project_root_check(&empty).expect_err("empty dir must fail");
        assert!(err.contains(".git"));
        assert!(err.contains(empty.to_str().expect("utf8")));
    }

    // --- key resolution ---

    #[test]
    fn omlx_key_default_reads_the_same_settings_file_as_the_bash_launcher() {
        let home = temp_dir("key-home");
        fs::create_dir_all(home.join(".omlx")).expect("mkdir");
        fs::write(
            home.join(".omlx").join("settings.json"),
            r#"{"auth": {"api_key": "settings-key"}}"#,
        )
        .expect("write");
        let home_str = home.to_str().expect("utf8");
        assert_eq!(omlx_key_default(Some(home_str)).as_deref(), Some("settings-key"));
        // Whitespace-only keys count as unset, as do blank homes and missing
        // or malformed files: render_step then reports the gap itself.
        fs::write(
            home.join(".omlx").join("settings.json"),
            r#"{"auth": {"api_key": "  "}}"#,
        )
        .expect("rewrite");
        assert_eq!(omlx_key_default(Some(home_str)), None);
        let empty = temp_dir("key-empty-home");
        assert_eq!(omlx_key_default(empty.to_str()), None, "missing file must yield None");
        assert_eq!(omlx_key_default(Some("   ")), None);
        assert_eq!(omlx_key_default(None), None);
    }

    #[test]
    fn resolve_omlx_key_prefers_the_env_and_falls_back_to_settings() {
        let home = temp_dir("resolve-home");
        fs::create_dir_all(home.join(".omlx")).expect("mkdir");
        fs::write(
            home.join(".omlx").join("settings.json"),
            r#"{"auth": {"api_key": "settings-key"}}"#,
        )
        .expect("write");
        let home_str = home.to_str().expect("utf8");
        assert_eq!(
            resolve_omlx_key(Some("env-key".to_string()), Some(home_str)).as_deref(),
            Some("env-key"),
            "a non-blank env key wins over the settings file"
        );
        assert_eq!(
            resolve_omlx_key(Some("  ".to_string()), Some(home_str)).as_deref(),
            Some("settings-key"),
            "a blank env key falls back to the settings file"
        );
        assert_eq!(resolve_omlx_key(None, Some(home_str)).as_deref(), Some("settings-key"));
        assert_eq!(resolve_omlx_key(None, None), None, "nothing anywhere: None");
    }

    // --- same_dir (the checkout case) ---

    #[test]
    fn same_dir_matches_textually_and_through_symlinks() {
        let a = temp_dir("same-a");
        assert!(same_dir(&a, &a), "identical paths are the same dir");
        let b = temp_dir("same-b");
        assert!(!same_dir(&a, &b), "different dirs are different");
        #[cfg(unix)]
        {
            let link = temp_dir("same-link").join("linked");
            std::os::unix::fs::symlink(&a, &link).expect("symlink");
            assert!(same_dir(&a, &link), "a symlink to the dir is the same dir");
        }
    }

    // --- prepare (the composition) ---

    #[test]
    fn prepare_runs_end_to_end_on_a_temp_project() {
        let tmpl = template("prep-e2e");
        let agent_dir = temp_dir("prep-e2e-agent");
        let project = temp_dir("prep-e2e-proj");
        fs::create_dir(project.join(".git")).expect("gitdir");
        let outcome = prepare(PrepareOptions {
            bppc_host: "203.0.113.10".to_string(),
            omlx_key: Some("e2e-key".to_string()),
            ..prepare_opts(&tmpl, &agent_dir, &project, None)
        });
        assert_eq!(
            outcome.steps.iter().map(|s| s.name).collect::<Vec<_>>(),
            vec!["seed", "render", "root", "wiki"]
        );
        for step in &outcome.steps {
            assert_eq!(step.status, StepStatus::Ok, "step {} failed: {}", step.name, step.detail);
        }
        assert_eq!(outcome.agent_dir, agent_dir);
        assert_eq!(outcome.models_json, agent_dir.join("models.json"));
        assert_eq!(
            fs::read_to_string(outcome.models_json).expect("models.json"),
            r#"{"baseUrl": "http://203.0.113.10:8080/v1", "apiKey": "e2e-key"}"#,
            "the rendered models.json carries the host and key; neither is ever printed"
        );
        assert!(agent_dir.join("AGENTS.md").is_file());
        assert!(agent_dir.join(SEED_STAMP_FILE).is_file(), "the default stamp is written");
        assert!(project.join("wiki/index.md").is_file());
    }

    #[test]
    fn prepare_reports_failures_per_step_without_aborting() {
        let base = temp_dir("prep-fail");
        let project = temp_dir("prep-fail-proj");
        let outcome = prepare(PrepareOptions {
            template: base.join("no-such-template"),
            ..prepare_opts(&base, &base, &project, None)
        });
        assert_eq!(
            step_named(&outcome, "seed").status,
            StepStatus::Fail,
            "missing template must fail seed"
        );
        assert_eq!(
            step_named(&outcome, "render").status,
            StepStatus::Fail,
            "missing template must fail render"
        );
        assert_eq!(
            step_named(&outcome, "root").status,
            StepStatus::Fail,
            "empty project must fail the root guard"
        );
        assert_eq!(step_named(&outcome, "wiki").status, StepStatus::Ok, "wiki init still runs");
        assert_eq!(exit_code(&outcome.steps), 9, "the root guard failed here");
    }

    #[test]
    fn prepare_blank_omlx_key_fails_render_and_allow_any_dir_passes_root() {
        let tmpl = template("prep-blank-key");
        let agent_dir = temp_dir("prep-blank-key-agent");
        let project = temp_dir("prep-blank-key-proj");
        let mut opts = prepare_opts(&tmpl, &agent_dir, &project, None);
        opts.omlx_key_var = "PI_TEST_KEY".to_string();
        opts.allow_any_dir = true;
        let outcome = prepare(opts);
        let render = step_named(&outcome, "render");
        assert_eq!(render.status, StepStatus::Fail, "blank key must fail render");
        assert!(
            render.detail.contains("PI_TEST_KEY") && render.detail.contains("cannot render"),
            "the failure names the env var, never the key: {}",
            render.detail
        );
        let root = step_named(&outcome, "root");
        assert_eq!(root.status, StepStatus::Skipped, "allow_any_dir skips the guard");
        assert!(root.detail.contains("allow-any-dir"));
        assert_eq!(exit_code(&outcome.steps), 6, "a render failure exits 6, not 9");
    }

    #[test]
    fn prepare_render_is_idempotent() {
        let tmpl = template("prep-idem");
        let agent_dir = temp_dir("prep-idem-agent");
        let project = temp_dir("prep-idem-proj");
        fs::create_dir(project.join(".git")).expect("gitdir");
        let make_opts = || {
            PrepareOptions {
                bppc_host: "10.0.0.9".to_string(),
                omlx_key: Some("idem-key".to_string()),
                ..prepare_opts(&tmpl, &agent_dir, &project, None)
            }
        };
        let first = prepare(make_opts());
        let second = prepare(make_opts());
        assert!(
            step_named(&first, "render").detail.starts_with("wrote"),
            "first run writes models.json"
        );
        assert!(
            step_named(&second, "render").detail.contains("unchanged"),
            "second run must not rewrite models.json: {}",
            step_named(&second, "render").detail
        );
        assert!(
            step_named(&second, "seed").detail.starts_with("agent dir ready"),
            "the same default stamp must be a no-op seed: {}",
            step_named(&second, "seed").detail
        );
        assert_eq!(exit_code(&second.steps), 0);
    }

    #[test]
    fn prepare_checkout_case_skips_seed_and_renders_in_place() {
        let tmpl = template("prep-checkout");
        let project = temp_dir("prep-checkout-proj");
        write(&project.join("CLAUDE.md"), "x\n");
        let outcome = prepare(PrepareOptions {
            omlx_key: Some("checkout-key".to_string()),
            ..prepare_opts(&tmpl, &tmpl, &project, None)
        });
        let seed = step_named(&outcome, "seed");
        assert_eq!(seed.status, StepStatus::Skipped, "the checkout case skips seeding");
        assert!(seed.detail.contains("used in place"));
        assert!(!tmpl.join(SEED_STAMP_FILE).exists(), "no stamp is written into a checkout");
        assert_eq!(step_named(&outcome, "render").status, StepStatus::Ok);
        assert!(
            fs::read_to_string(tmpl.join("models.json"))
                .expect("models.json rendered in place")
                .contains("checkout-key"),
            "render still targets the agent dir, which is the template"
        );
        assert_eq!(exit_code(&outcome.steps), 0);
    }

    #[test]
    fn prepare_default_stamp_is_hash_plus_optional_version_and_explicit_stamp_wins() {
        let tmpl = template("prep-stamps");
        let a = temp_dir("prep-stamps-a");
        let project = temp_dir("prep-stamps-proj");
        write(&project.join("CLAUDE.md"), "x\n");
        prepare(prepare_opts(&tmpl, &a, &project, Some("k")));
        let bare = fs::read_to_string(a.join(SEED_STAMP_FILE)).expect("stamp");
        assert_eq!(bare.len(), 16, "no --version: the bare hash, got {bare}");

        let b = temp_dir("prep-stamps-b");
        prepare(PrepareOptions {
            version: Some("9.9.9".to_string()),
            ..prepare_opts(&tmpl, &b, &project, Some("k"))
        });
        let versioned = fs::read_to_string(b.join(SEED_STAMP_FILE)).expect("stamp");
        assert!(versioned.starts_with("9.9.9+"));

        let c = temp_dir("prep-stamps-c");
        prepare(PrepareOptions {
            stamp: Some("operator-stamp".to_string()),
            version: Some("9.9.9".to_string()),
            ..prepare_opts(&tmpl, &c, &project, Some("k"))
        });
        assert_eq!(
            fs::read_to_string(c.join(SEED_STAMP_FILE)).expect("stamp"),
            "operator-stamp",
            "an explicit --stamp wins over --version and the hash"
        );
    }

    #[test]
    fn exit_code_root_guard_failure_beats_other_failures() {
        let steps = vec![
            Step { name: "seed", status: StepStatus::Fail, detail: String::new() },
            Step { name: "render", status: StepStatus::Fail, detail: String::new() },
            Step { name: "root", status: StepStatus::Fail, detail: String::new() },
            Step { name: "wiki", status: StepStatus::Ok, detail: String::new() },
        ];
        assert_eq!(exit_code(&steps), 9, "the root guard dominates");
        let no_root_fail = vec![
            Step { name: "seed", status: StepStatus::Ok, detail: String::new() },
            Step { name: "render", status: StepStatus::Fail, detail: String::new() },
            Step { name: "root", status: StepStatus::Ok, detail: String::new() },
            Step { name: "wiki", status: StepStatus::Skipped, detail: String::new() },
        ];
        assert_eq!(exit_code(&no_root_fail), 6);
        let clean = vec![
            Step { name: "seed", status: StepStatus::Skipped, detail: String::new() },
            Step { name: "render", status: StepStatus::Ok, detail: String::new() },
            Step { name: "root", status: StepStatus::Skipped, detail: String::new() },
            Step { name: "wiki", status: StepStatus::Skipped, detail: String::new() },
        ];
        assert_eq!(exit_code(&clean), 0, "SKIPPED is success");
    }
}
