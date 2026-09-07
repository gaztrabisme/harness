//! The board's value types: the 8-state spine `Status`, the `GateSource`
//! (human vs machine — the distinction that keeps the Align gate human-cleared),
//! and the edge-kind blocking predicate. Pure data; no DB here.

/// The work spine (design-v2 §4). `Done` is the only terminal state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
	Todo,
	Align,
	InProgress,
	Verify,
	Review,
	Land,
	Done,
	Rework,
}

impl Status {
	pub const fn as_str(self) -> &'static str {
		match self {
			Self::Todo => "todo",
			Self::Align => "align",
			Self::InProgress => "in_progress",
			Self::Verify => "verify",
			Self::Review => "review",
			Self::Land => "land",
			Self::Done => "done",
			Self::Rework => "rework",
		}
	}

	/// Terminal = absorbing. Only `Done`. (No transition may originate here,
	/// and the `any -> rework` wildcard excludes it — C2's finding #1.)
	pub const fn is_terminal(self) -> bool {
		matches!(self, Self::Done)
	}

	/// Position along the spine, for board-overview ordering: active statuses in
	/// spine order first, `rework` (sent back, awaiting re-align) next, terminal
	/// `done` last. Presentation ordering only — no transition semantics.
	pub const fn spine_pos(self) -> u8 {
		match self {
			Self::Todo => 0,
			Self::Align => 1,
			Self::InProgress => 2,
			Self::Verify => 3,
			Self::Review => 4,
			Self::Land => 5,
			Self::Rework => 6,
			Self::Done => 7,
		}
	}
}

impl std::str::FromStr for Status {
	type Err = ();
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(match s {
			"todo" => Self::Todo,
			"align" => Self::Align,
			"in_progress" => Self::InProgress,
			"verify" => Self::Verify,
			"review" => Self::Review,
			"land" => Self::Land,
			"done" => Self::Done,
			"rework" => Self::Rework,
			_ => return Err(()),
		})
	}
}

/// Who cleared a gate. The Align gate (`criteria_confirmed`) MUST be `Human`;
/// an agent provider writing a machine pass cannot satisfy it (C3's blocker #1
/// — the human-gate must not dissolve into "any provider self-certifies").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateSource {
	Human,
	Machine,
}

impl GateSource {
	pub const fn as_str(self) -> &'static str {
		match self {
			Self::Human => "human",
			Self::Machine => "machine",
		}
	}
}

impl std::str::FromStr for GateSource {
	type Err = ();
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(match s {
			"human" => Self::Human,
			"machine" => Self::Machine,
			_ => return Err(()),
		})
	}
}

/// One recorded gate verdict — a row of the append-only `gate_results` store
/// (schema v3). The store keeps EVERY report, so a red that a later green
/// superseded is still here: `seq` (autoincrement, i.e. commit order) is the
/// only correct sort, since `created_at` is second-resolution and same-second
/// reports are routine. `gate_satisfied` decides which of these rows is in
/// force; this shape is the history behind that decision.
#[derive(Debug, Clone)]
pub struct GateReport {
	/// Autoincrement primary key — report order, and the sort key for every reader.
	pub seq: i64,
	pub gate: String,
	pub provider: String,
	pub source: GateSource,
	/// The rework epoch the verdict was recorded at.
	pub attempt: i64,
	pub passed: bool,
	pub note: Option<String>,
	/// When THIS report was written (under the old upsert this carried the first
	/// report's timestamp beside the last report's verdict).
	pub created_at: String,
}

/// A ticket row (the read shape). Workpad fields are nullable — they fill in
/// across the lifecycle (the workpad-rendering slice owns their editing).
#[derive(Debug, Clone)]
pub struct Ticket {
	pub id: String,
	pub kind: String,
	pub status: Status,
	pub title: String,
	pub plan: Option<String>,
	pub acceptance_criteria: Option<String>,
	pub validation: Option<String>,
	pub notes: Option<String>,
	pub confusions: Option<String>,
	pub priority: i64,
	/// Bumped on every entry into `Rework`; scopes gate verdicts so a prior
	/// attempt's `pass` cannot satisfy a re-entered gate (C1+C3's cross-confirmed
	/// blocker — stale-pass bypass on rework).
	pub attempt: i64,
}

/// A memory's scope (research/18 §10.2) — a closed, stable set, so it keeps a SQL
/// CHECK (unlike `type`, which P5 case-law extends and is validated in Rust). A
/// `ticket`-scoped memory is episodic to one ticket; `project` spans the repo;
/// `global` is cross-project (Gary's standing preferences, hardware facts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
	Ticket,
	Project,
	Global,
}

impl Scope {
	pub const fn as_str(self) -> &'static str {
		match self {
			Self::Ticket => "ticket",
			Self::Project => "project",
			Self::Global => "global",
		}
	}
}

impl std::str::FromStr for Scope {
	type Err = ();
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(match s {
			"ticket" => Self::Ticket,
			"project" => Self::Project,
			"global" => Self::Global,
			_ => return Err(()),
		})
	}
}

/// A recall hit — the *progressive-disclosure index* shape (research/18 §10.5):
/// just enough to decide whether to spend tokens on the body. `recall` returns
/// these (id + title + type, never the body); `recall_body` fetches the full text
/// for a chosen id and is the only path that counts as retrieval use.
#[derive(Debug, Clone)]
pub struct MemoryHit {
	pub id: String,
	pub title: String,
	/// The memory `type` column (an open vocabulary; `type` is a Rust keyword).
	pub r#type: String,
}

/// A *primed* recall hit — the body-carrying shape used only by auto-context
/// priming (research/18 §3 "loop integration"). Unlike `MemoryHit` (the human
/// CLI index), priming injects the lesson inline because the auto-written
/// post-mortem titles are generic ("explore <t> w<k>: failed approach") — the
/// signal lives in the body. The body is head+tail clamped on read, and
/// producing this struct does NOT bump `usage_count` (ambient injection is not
/// deliberate retrieval use — see `recall_primed`).
#[derive(Debug, Clone)]
pub struct PrimedHit {
	/// The memory id — carried so a primed/researched lesson is traceable and so
	/// `used_ids` is first-class through the same shape the researcher (S3) emits.
	pub id: String,
	pub title: String,
	/// The memory `type` column (an open vocabulary; `type` is a Rust keyword).
	pub r#type: String,
	/// The clamped body (head+tail), already bounded for prompt injection.
	pub body: String,
}

/// Edge kinds that make a ticket *not ready* while their target is non-terminal
/// (`br`'s `affects_ready_work` set). All other kinds are knowledge edges.
/// `parent-child` is modelled as parent→child (the parent depends on the child),
/// so "a parent with open children is not ready" falls out of the same predicate.
pub fn edge_kind_blocks(kind: &str) -> bool {
	matches!(kind, "blocks" | "parent-child" | "conditional-blocks" | "waits-for")
}

/// A telemetry run record (research/17 §4 Unit A) — one per execution of the
/// agent loop. The row is the queryable audit index; the trajectory itself lives
/// in a referenced JSONL (Unit B). Opened at loop entry with `ended_at`/`iters`/
/// `stop_reason` NULL and closed on exit, so an interrupted run is the queryable
/// `ended_at IS NULL` rather than an orphan file. `attempt` joins `gate_results`
/// at the run's rework epoch; `model`/`provider`/`sampling` are the provenance the
/// RL/audit views need (e.g. to partition teacher- vs local-generated trajectories).
#[derive(Debug, Clone)]
pub struct Run {
	pub run_id: String,
	pub ticket_id: String,
	pub attempt: i64,
	pub model: String,
	pub provider: String,
	pub sampling: Option<String>,
	pub project: Option<String>,
	pub started_at: String,
	pub ended_at: Option<String>,
	pub iters: Option<i64>,
	pub stop_reason: Option<String>,
}
