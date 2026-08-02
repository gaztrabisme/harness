//! The memory sidecar — Slice A (research/18 §10.5): a lexical episodic/semantic
//! store distinct from the wiki (rows decay; the wiki doesn't). Three ops on
//! `Board`, mirroring board.rs house style (`params!`, validation at the chokepoint):
//!  - `remember` — insert a row (type validated in Rust so P5 case-law can extend
//!    the vocabulary without a table rebuild; salience range-checked for a clear error).
//!  - `recall` — FTS5 BM25 over (title, body, entities), project/scope **pre-filter**,
//!    soft-delete aware, returning the progressive-disclosure index (id, title, type),
//!    BM25 order with salience as a pure tiebreak.
//!  - `recall_body` — the body for a chosen id; the ONLY path that bumps
//!    `usage_count`/`last_used_ts` (retrieval-use is the real signal).
//!
//! Fenced OUT of Slice A (gated by measurement in §10.4): embeddings / the vector
//! leg / rank-fusion / dedup-merge (Slice B, gated on a golden-set eval); decay +
//! reflection (Slice C, gated on the sidecar earning its keep). The v2 schema
//! already carries every column those slices need, so neither requires a rebuild.
//!
//! The numeric relevance floor (§10.1) is the FTS5 MATCH itself plus `k` for
//! Slice A: a row appears only if it lexically matches, and an empty store returns
//! empty (empty beats noise). An absolute BM25 cutoff is corpus-calibrated against
//! the golden set in Slice B — inventing the number now would violate gate-by-a-number.

use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, bail};
use rusqlite::{OptionalExtension, ToSql, params};

use crate::Board;
use crate::model::{MemoryHit, PrimedHit, Scope};

/// Internal row shape shared by `recall` and `recall_primed` (`body` is `None`
/// when the caller didn't ask for it). Keeps the SQL in one place — the two public
/// ops differ only in projection + post-processing, never in the query.
struct RawHit {
	id: String,
	title: String,
	r#type: String,
	body: Option<String>,
}

/// Max chars of a primed body injected into a prompt (head+tail clamped). Sized so
/// an explore post-mortem's `Strategy:` head AND its trailing `did not satisfy
/// \`<validation>\`` tail both survive a clamp — a naive head-only cut would drop
/// the actionable half (adv-review §A-d).
const PRIMED_BODY_CAP: usize = 240;

/// The Slice A memory-type vocabulary (research/18 §2). Validated in Rust at this
/// chokepoint — NOT a SQL CHECK — so P5 self-evolution case-law can add types
/// without an `ALTER`/table rebuild (§10.2).
const MEMORY_TYPES: &[&str] = &[
	"decision", "bugfix", "feature", "refactor", "discovery", "lesson", "constraint",
];

/// A per-process monotonic counter so two memories minted in the same millisecond
/// get distinct, still-sortable ids (research/18 §10.2 — no same-tick collision).
static MEM_COUNTER: AtomicU64 = AtomicU64::new(0);

/// The write-input for `remember`. Borrowed fields — the row is built and inserted
/// in one call, so nothing needs to own the strings. `salience` defaults to 0.5 at
/// the CLI; `scope` is required (the closed enum).
pub struct NewMemory<'a> {
	pub r#type: &'a str,
	pub title: &'a str,
	pub body: Option<&'a str>,
	pub salience: f64,
	pub scope: Scope,
	pub entities: Option<&'a str>,
	pub files: Option<&'a str>,
	pub project: Option<&'a str>,
	pub ticket_id: Option<&'a str>,
}

impl Board {
	/// Insert a memory, returning its minted id. Validates `type` against the
	/// known vocabulary and `salience` against [0,1] here (the chokepoint) for
	/// clear errors; the FTS index syncs via the schema's AFTER INSERT trigger.
	pub fn remember(&self, m: &NewMemory) -> Result<String> {
		if !MEMORY_TYPES.contains(&m.r#type) {
			bail!("unknown memory type '{}'; known: {}", m.r#type, MEMORY_TYPES.join(", "));
		}
		if !(0.0..=1.0).contains(&m.salience) {
			bail!("salience {} out of range [0,1]", m.salience);
		}
		let id = mint_id();
		self.conn()
			.execute(
				"INSERT INTO memory
				   (id, type, title, body, salience, scope, entities, files, project, ticket_id)
				 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
				params![
					id,
					m.r#type,
					m.title,
					m.body,
					m.salience,
					m.scope.as_str(),
					m.entities,
					m.files,
					m.project,
					m.ticket_id
				],
			)
			.with_context(|| format!("inserting memory {id}"))?;
		Ok(id)
	}

	/// Recall up to `k` live memories matching `query`, ranked by BM25 then salience
	/// (a pure tiebreak — no weighted blend in Slice A). `project`/`scope` are
	/// **pre-filters** (applied in the WHERE, before ranking — never post-filtered).
	/// Returns the index shape only (id, title, type); call `recall_body` for the
	/// text. A query with no usable tokens, or an empty/all-evicted store, returns
	/// an empty Vec (empty beats noise injected into the loop).
	pub fn recall(
		&self,
		query: &str,
		k: usize,
		project: Option<&str>,
		scope: Option<Scope>,
	) -> Result<Vec<MemoryHit>> {
		let rows = self.recall_rows(query, k, project, scope, false)?;
		Ok(rows
			.into_iter()
			.map(|r| MemoryHit { id: r.id, title: r.title, r#type: r.r#type })
			.collect())
	}

	/// Auto-context priming (research/18 §3 "loop integration"): recall up to `k`
	/// project-scoped memories for `query` and return them **with bodies**, ready to
	/// inject into a worker's prompt. Distinct from `recall` in three deliberate ways:
	///
	/// 1. **Carries the body** (head+tail clamped here, not trusting the writer) —
	///    the auto-written post-mortem titles are generic, so the lesson is in the body.
	/// 2. **Does NOT bump `usage_count`/`last_used_ts`.** Ambient injection is not
	///    deliberate retrieval use; bumping the same top-k on *every* run would make
	///    them immortal under any future decay rule. This is defensible only because
	///    no decay consumer exists yet (Slice C) — it is a no-consumer default, not a
	///    principle. `recall_body` (the human CLI drill-down) stays the only use-bump.
	/// 3. **Applies a token-overlap relevance floor** (not a numeric BM25 cutoff,
	///    which the module doc defers to Slice B): a hit must share at least
	///    `min(2, n_query_tokens)` distinct ≥3-char tokens with `query`. In a cold,
	///    near-empty store BM25's OR-of-tokens otherwise surfaces an unrelated
	///    post-mortem on one shared boilerplate word; the floor turns that noise into
	///    an empty result (the section is then omitted entirely). Empty beats noise.
	///
	/// Scope is intentionally unfiltered (all of ticket/project/global within the
	/// project) so a ticket-scoped post-mortem from one ticket can surface on related
	/// work — the compounding payoff. The floor is what makes cross-scope safe.
	pub fn recall_primed(&self, query: &str, k: usize, project: Option<&str>) -> Result<Vec<PrimedHit>> {
		let rows = self.recall_rows(query, k, project, None, true)?;
		// Floor: keep a hit only if it shares at least one CONTENT token (stopwords
		// stripped) with the query. The earlier raw floor (need≥2 over `overlap_tokens`)
		// leaked badly because stopwords like "the"/"add"/"fix" inflated overlap — a
		// topically-unrelated task that merely shared a filler word plus one incidental
		// term cleared a 2-token bar on stopwords alone. Requiring one genuine content
		// token in common cuts that leak (eval v2: noise 18→13/24) while holding
		// paraphrase recovery (32/33; the single drop was itself a stopword artifact),
		// with no tuned constant. The residual lexical-coincidence leak ("version",
		// "memory" shared with an unrelated task) is a Slice-B semantic-rerank problem,
		// not a lexical-floor one. See research/31-eval-results.md.
		let q_tokens = content_tokens(query);
		Ok(rows
			.into_iter()
			.filter(|r| {
				let hay: std::collections::HashSet<String> =
					content_tokens(&format!("{} {}", r.title, r.body.as_deref().unwrap_or(""))).into_iter().collect();
				q_tokens.iter().any(|t| hay.contains(t))
			})
			.map(|r| PrimedHit {
				id: r.id,
				title: r.title,
				r#type: r.r#type,
				body: clamp_body(r.body.as_deref().unwrap_or(""), PRIMED_BODY_CAP),
			})
			.collect())
	}

	/// The shared recall query — the single source of truth for the pre-filter +
	/// BM25 ordering so `recall` (index) and `recall_primed` (body) can never drift.
	/// `include_body` only widens the projection; everything else is identical. The
	/// FTS table is referenced unaliased so `bm25(memory_fts)` resolves; only
	/// `memory` is aliased.
	fn recall_rows(
		&self,
		query: &str,
		k: usize,
		project: Option<&str>,
		scope: Option<Scope>,
		include_body: bool,
	) -> Result<Vec<RawHit>> {
		let Some(match_expr) = fts_query(query) else {
			return Ok(Vec::new());
		};
		let cols = if include_body { "m.id, m.title, m.type, m.body" } else { "m.id, m.title, m.type, NULL" };
		let mut sql = format!(
			"SELECT {cols}
			   FROM memory_fts JOIN memory m ON m.rowid = memory_fts.rowid
			  WHERE memory_fts MATCH ?1 AND m.evicted_at IS NULL"
		);
		let mut binds: Vec<Box<dyn ToSql>> = vec![Box::new(match_expr)];
		if let Some(p) = project {
			binds.push(Box::new(p.to_string()));
			sql.push_str(&format!(" AND m.project = ?{}", binds.len()));
		}
		if let Some(s) = scope {
			binds.push(Box::new(s.as_str().to_string()));
			sql.push_str(&format!(" AND m.scope = ?{}", binds.len()));
		}
		binds.push(Box::new(k as i64));
		sql.push_str(&format!(" ORDER BY bm25(memory_fts) ASC, m.salience DESC LIMIT ?{}", binds.len()));

		let mut stmt = self.conn().prepare(&sql).context("preparing recall query")?;
		let refs: Vec<&dyn ToSql> = binds.iter().map(AsRef::as_ref).collect();
		let hits = stmt
			.query_map(refs.as_slice(), |r| {
				Ok(RawHit { id: r.get(0)?, title: r.get(1)?, r#type: r.get(2)?, body: r.get(3)? })
			})
			.context("running recall query")?
			.collect::<rusqlite::Result<Vec<_>>>()?;
		Ok(hits)
	}

	/// Fetch the body for a chosen memory and record the retrieval use: bump
	/// `usage_count` and stamp `last_used_ts` (the ONLY op that does — recall of
	/// the index alone is not "use"). Returns `None` for an unknown or evicted id;
	/// a found row with a NULL body returns `Some("")`, so `Some` always means
	/// "found and counted".
	pub fn recall_body(&self, id: &str) -> Result<Option<String>> {
		// Bump only a live row; the WHERE keeps the count honest for evicted ids.
		// This UPDATE doesn't touch FTS-indexed columns, but the AFTER UPDATE
		// trigger still re-syncs the (unchanged) row — correct, just idempotent.
		self.conn()
			.execute(
				"UPDATE memory SET usage_count = usage_count + 1, last_used_ts = CURRENT_TIMESTAMP
				  WHERE id = ?1 AND evicted_at IS NULL",
				params![id],
			)
			.with_context(|| format!("bumping usage for memory {id}"))?;
		let row: Option<Option<String>> = self
			.conn()
			.query_row(
				"SELECT body FROM memory WHERE id = ?1 AND evicted_at IS NULL",
				params![id],
				|r| r.get(0),
			)
			.optional()
			.with_context(|| format!("reading memory body {id}"))?;
		Ok(row.map(Option::unwrap_or_default))
	}
}

/// Mint a sortable id: `m<unix_millis:013><counter:09>` — lexical order is
/// chronological, and the per-process counter breaks same-millisecond ties. Clock
/// failure degrades to millis 0 rather than panicking (mirrors `mint_run_id`).
fn mint_id() -> String {
	let millis = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.map(|d| d.as_millis())
		.unwrap_or(0);
	let c = MEM_COUNTER.fetch_add(1, Ordering::Relaxed);
	format!("m{millis:013}-{c:09}")
}

/// Turn a raw query into a safe FTS5 MATCH expression: split on whitespace, drop
/// embedded quotes and any token without an alphanumeric char, wrap each surviving
/// token as a quoted phrase, and OR them together (recall-friendly; BM25 then
/// ranks). Quoting neutralises FTS5 operator characters so arbitrary user input
/// (paths, `::`, error strings) can't be a syntax error. `None` when nothing usable
/// remains — the caller returns an empty recall.
fn fts_query(raw: &str) -> Option<String> {
	let terms: Vec<String> = raw
		.split_whitespace()
		.map(|t| t.replace('"', ""))
		.filter(|t| t.chars().any(char::is_alphanumeric))
		.map(|t| format!("\"{t}\""))
		.collect();
	(!terms.is_empty()).then(|| terms.join(" OR "))
}

/// Distinct lower-cased ≥3-char alphanumeric tokens of `text`, for the priming
/// relevance floor. The ≥3-char rule drops the short boilerplate ("to", "of",
/// "w", "a") that would otherwise inflate overlap with a stored post-mortem.
/// Order-stable, de-duplicated.
fn overlap_tokens(text: &str) -> Vec<String> {
	let mut seen = std::collections::HashSet::new();
	let mut out = Vec::new();
	for raw in text.split(|c: char| !c.is_alphanumeric()) {
		if raw.len() < 3 {
			continue;
		}
		let t = raw.to_lowercase();
		if seen.insert(t.clone()) {
			out.push(t);
		}
	}
	out
}

/// English + ticket-boilerplate stopwords that pollute the ≥3-char overlap floor.
/// `overlap_tokens` keeps any ≥3-char alphanumeric token, so function words ("the",
/// "and", "for") and ubiquitous ticket verbs ("add", "fix") otherwise count as
/// "shared content" — inflating overlap so the floor never binds on recovery yet
/// leaks noise (measured: t5 eval v2). Stripping these is the actual t5 fix.
const STOPWORDS: &[&str] = &[
	"the", "and", "for", "are", "was", "were", "but", "not", "you", "your", "our",
	"its", "out", "into", "with", "from", "this", "that", "these", "those", "than",
	"then", "them", "they", "their", "have", "has", "had", "can", "could", "would",
	"should", "will", "shall", "may", "might", "must", "does", "did", "done", "doing",
	"how", "what", "why", "when", "where", "which", "who", "whom", "whose",
	"two", "one", "all", "any", "some", "each", "between", "without", "within",
	"add", "fix", "use", "using", "make", "made", "get", "got", "set", "let",
	"new", "old", "via", "per", "off", "now", "still", "just", "only", "also",
	"about", "after", "before", "under", "over", "again", "same", "other",
	"keep", "kept", "stop", "run", "runs", "work", "works", "thing", "things",
];

/// Distinct lower-cased ≥3-char alphanumeric **content** tokens (overlap_tokens minus
/// `STOPWORDS`). This is what the priming relevance floor should compare on.
fn content_tokens(text: &str) -> Vec<String> {
	overlap_tokens(text).into_iter().filter(|t| !STOPWORDS.contains(&t.as_str())).collect()
}

/// Clamp a body to `cap` chars keeping the HEAD and the TAIL (a naive head-only
/// cut drops the actionable tail; adv-review §A-d). Char-boundary safe. A body at
/// or under the cap is returned unchanged.
fn clamp_body(body: &str, cap: usize) -> String {
	let n = body.chars().count();
	if n <= cap {
		return body.to_string();
	}
	// Reserve room for the elision marker; split the budget head-heavy (2:1).
	let marker = " … ";
	let budget = cap.saturating_sub(marker.chars().count());
	let head = (budget * 2) / 3;
	let tail = budget - head;
	let head_s: String = body.chars().take(head).collect();
	let tail_s: String = body.chars().skip(n - tail).collect();
	format!("{head_s}{marker}{tail_s}")
}

/// The body clamp used for prompt-injected primed lessons, exposed so the S3
/// researcher glue (in the agent crate) bounds a recalled body to the SAME cap
/// `recall_primed` applies — one ceiling for every injected lesson, whether it
/// arrived via the lexical floor or the researcher sub-agent.
pub fn clamp_primed_body(body: &str) -> String {
	clamp_body(body, PRIMED_BODY_CAP)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::Board;

	fn mem() -> Board {
		Board::open(":memory:").unwrap()
	}

	/// Minimal builder for a project-scoped memory (the common test shape).
	fn nm<'a>(ty: &'a str, title: &'a str, body: Option<&'a str>, sal: f64) -> NewMemory<'a> {
		NewMemory {
			r#type: ty,
			title,
			body,
			salience: sal,
			scope: Scope::Project,
			entities: None,
			files: None,
			project: None,
			ticket_id: None,
		}
	}

	// remember → recall round-trips; recall returns the index shape (id/title/type)
	// and NOT the body; only the lexically relevant row matches; recall_body fetches
	// the text. (Progressive disclosure — §10.5.)
	#[test]
	fn remember_then_recall_round_trips() {
		let b = mem();
		let id = b.remember(&nm("lesson", "WAL avoids dropped rows", Some("set busy_timeout"), 0.5)).unwrap();
		b.remember(&nm("bugfix", "unrelated coffee machine", Some("descale it"), 0.5)).unwrap();

		let hits = b.recall("WAL busy_timeout", 8, None, None).unwrap();
		assert_eq!(hits.len(), 1, "only the relevant memory matches");
		assert_eq!(hits[0].id, id);
		assert_eq!(hits[0].r#type, "lesson");
		assert_eq!(hits[0].title, "WAL avoids dropped rows");
		assert_eq!(b.recall_body(&id).unwrap().as_deref(), Some("set busy_timeout"));
	}

	// Two rows with identical indexed text → identical BM25 → the higher-salience
	// row wins the tiebreak (BM25 order, salience as a pure tiebreak — §10.4).
	#[test]
	fn recall_orders_by_relevance_then_salience() {
		let b = mem();
		let lo = b.remember(&nm("discovery", "sqlite locking note", Some("same text"), 0.2)).unwrap();
		let hi = b.remember(&nm("discovery", "sqlite locking note", Some("same text"), 0.9)).unwrap();
		let hits = b.recall("sqlite locking", 8, None, None).unwrap();
		assert_eq!(hits.len(), 2);
		assert_eq!(hits[0].id, hi, "equal BM25 → higher salience wins");
		assert_eq!(hits[1].id, lo);
	}

	// project and scope are PRE-filters: a matching query is narrowed by each.
	#[test]
	fn recall_prefilters_project_and_scope() {
		let b = mem();
		b.remember(&NewMemory { project: Some("alpha"), ..nm("lesson", "shared token thing", None, 0.5) })
			.unwrap();
		b.remember(&NewMemory {
			project: Some("beta"),
			scope: Scope::Global,
			..nm("lesson", "shared token thing", None, 0.5)
		})
		.unwrap();

		assert_eq!(b.recall("shared token", 8, Some("alpha"), None).unwrap().len(), 1, "project pre-filter");
		assert_eq!(b.recall("shared token", 8, None, Some(Scope::Global)).unwrap().len(), 1, "scope pre-filter");
		assert_eq!(b.recall("shared token", 8, None, None).unwrap().len(), 2, "no filter sees both");
	}

	// recall_body bumps usage_count + last_used_ts on every call; an unknown id is
	// None (and bumps nothing).
	#[test]
	fn recall_body_bumps_usage_and_missing_is_none() {
		let b = mem();
		let id = b.remember(&nm("discovery", "bump me", Some("the body"), 0.5)).unwrap();
		assert_eq!(b.recall_body(&id).unwrap().as_deref(), Some("the body"));
		b.recall_body(&id).unwrap();

		let (uc, lu): (i64, Option<String>) = b
			.conn()
			.query_row("SELECT usage_count, last_used_ts FROM memory WHERE id = ?1", params![id], |r| {
				Ok((r.get(0)?, r.get(1)?))
			})
			.unwrap();
		assert_eq!(uc, 2, "each recall_body bumps usage_count");
		assert!(lu.is_some(), "last_used_ts stamped");
		assert!(b.recall_body("nope").unwrap().is_none(), "unknown id → None");
	}

	// the chokepoint rejects an unknown type and an out-of-range salience.
	#[test]
	fn rejects_unknown_type_and_bad_salience() {
		let b = mem();
		assert!(b.remember(&nm("rumor", "x", None, 0.5)).is_err(), "unknown type rejected");
		assert!(b.remember(&nm("lesson", "x", None, 1.5)).is_err(), "salience > 1 rejected");
		assert!(b.remember(&nm("lesson", "x", None, -0.1)).is_err(), "salience < 0 rejected");
	}

	// the external-content FTS5 triggers keep the index in sync on UPDATE and
	// DELETE (the §10.2-mandated test): update title → old term gone, new found;
	// delete → gone entirely.
	#[test]
	fn fts_triggers_keep_external_content_in_sync() {
		let b = mem();
		let id = b.remember(&nm("discovery", "alpha keyword", None, 0.5)).unwrap();
		assert_eq!(b.recall("alpha", 8, None, None).unwrap().len(), 1);

		b.conn().execute("UPDATE memory SET title = 'omega keyword' WHERE id = ?1", params![id]).unwrap();
		assert!(b.recall("alpha", 8, None, None).unwrap().is_empty(), "old title de-indexed");
		assert_eq!(b.recall("omega", 8, None, None).unwrap().len(), 1, "new title indexed");

		b.conn().execute("DELETE FROM memory WHERE id = ?1", params![id]).unwrap();
		assert!(b.recall("omega", 8, None, None).unwrap().is_empty(), "delete removes from FTS");
	}

	// a soft-deleted (evicted) row is hidden from both recall and recall_body, but
	// the row itself survives (provenance preserved — §10.2).
	#[test]
	fn evicted_rows_are_hidden() {
		let b = mem();
		let id = b.remember(&nm("discovery", "soft delete target", Some("b"), 0.5)).unwrap();
		b.conn().execute("UPDATE memory SET evicted_at = CURRENT_TIMESTAMP WHERE id = ?1", params![id]).unwrap();
		assert!(b.recall("soft delete", 8, None, None).unwrap().is_empty(), "evicted hidden from recall");
		assert!(b.recall_body(&id).unwrap().is_none(), "evicted hidden from recall_body");
		let n: i64 = b.conn().query_row("SELECT COUNT(*) FROM memory WHERE id = ?1", params![id], |r| r.get(0)).unwrap();
		assert_eq!(n, 1, "the row survives the soft delete");
	}

	// punctuation-heavy queries are sanitised (never an FTS syntax error), and a
	// query with no usable tokens returns empty rather than erroring.
	#[test]
	fn recall_handles_punctuation_and_empty_query() {
		let b = mem();
		b.remember(&nm("bugfix", "path/to::thing failed", None, 0.5)).unwrap();
		assert_eq!(b.recall("path/to::thing", 8, None, None).unwrap().len(), 1, "punctuation tokenised, not an error");
		assert!(b.recall("   \"\"  *  ", 8, None, None).unwrap().is_empty(), "no usable tokens → empty");
	}

	// recall_primed carries the body and, crucially, does NOT bump usage_count or
	// last_used_ts — ambient injection is not deliberate retrieval use (the signal
	// the future decay slice will read). The row must be byte-identical after.
	#[test]
	fn recall_primed_returns_body_and_never_bumps_usage() {
		let b = mem();
		let id = b
			.remember(&nm("lesson", "rustls provider handshake", Some("use the native roots store"), 0.5))
			.unwrap();
		let before: (i64, Option<String>) = b
			.conn()
			.query_row("SELECT usage_count, last_used_ts FROM memory WHERE id = ?1", params![id], |r| {
				Ok((r.get(0)?, r.get(1)?))
			})
			.unwrap();

		let hits = b.recall_primed("rustls provider handshake", 5, None).unwrap();
		assert_eq!(hits.len(), 1);
		assert_eq!(hits[0].title, "rustls provider handshake");
		assert_eq!(hits[0].r#type, "lesson");
		assert_eq!(hits[0].body, "use the native roots store", "body carried inline");

		let after: (i64, Option<String>) = b
			.conn()
			.query_row("SELECT usage_count, last_used_ts FROM memory WHERE id = ?1", params![id], |r| {
				Ok((r.get(0)?, r.get(1)?))
			})
			.unwrap();
		assert_eq!(before, after, "priming is ambient — usage_count/last_used_ts unchanged");
	}

	// The CONTENT-token floor (t5 fix, eval v2 arm a*1): keep a hit iff it shares
	// ≥1 stopword-stripped token with the query. This pins the fix between the two
	// failure modes — the old raw `need≥2` floor (drops the single-content-token
	// paraphrase, KEEP row below) and naive relaxation to raw `need≥1` (leaks a row
	// that FTS-matched on a stopword, DROP row below).
	#[test]
	fn recall_primed_floor_keeps_content_drops_stopword_only() {
		let b = mem();
		// KEEP: shares exactly ONE content token ("rustls") — the old need≥2 raw floor
		// dropped this (the t5 regime); a*1 recovers it.
		b.remember(&nm("lesson", "explore t1: tls setup", Some("the rustls roots, then run again"), 0.4)).unwrap();
		// DROP: FTS-matches only via the stopword "add"; zero content overlap. Raw
		// need≥1 (arm b) would leak this; the content floor rejects it.
		b.remember(&nm("lesson", "explore t2: csv parser", Some("add commas, split fields"), 0.4)).unwrap();

		let hits = b.recall_primed("add rustls handshake support", 5, None).unwrap();
		assert_eq!(hits.len(), 1, "only the content-overlap row survives; the stopword-only match is dropped");
		assert!(hits[0].body.contains("rustls roots"), "kept the single-content-token paraphrase");
	}

	// An empty store (and a no-usable-token query) yields no primed hits — the loop
	// then omits the section entirely (empty beats noise).
	#[test]
	fn recall_primed_empty_when_nothing_relevant() {
		let b = mem();
		assert!(b.recall_primed("anything at all", 5, None).unwrap().is_empty(), "empty store → no hits");
		b.remember(&nm("lesson", "totally unrelated csv parser", Some("split on commas"), 0.5)).unwrap();
		assert!(
			b.recall_primed("rustls tls handshake", 5, None).unwrap().is_empty(),
			"no shared tokens → floor yields empty, not noise"
		);
	}

	// ───────────────────────────────────────────────────────────────────────────
	// t5 PRECISION COUNTER-PROBE (research/28 §12 follow-up) — throwaway measurement,
	// not a CI gate (`#[ignore]`; run with `cargo test -p board -- --ignored --nocapture
	// t5_floor`). Answers the question that blocked t5: relaxing the `need≥2` floor
	// recovers the commit-discipline paraphrase (12/14→13/14) — but at what NOISE cost?
	// Measures BOTH sides over the REAL FTS5/BM25 engine, simulating each floor policy
	// in the probe WITHOUT changing `recall_primed`'s shipped logic. Deterministic.
	#[test]
	#[ignore = "throwaway precision probe; run explicitly with --ignored --nocapture"]
	fn t5_floor_precision_counter_probe() {
		// The research/28 14-row corpus (artifact-distilled) — the warm relevant store.
		let corpus: &[(&str, &str, &str)] = &[
			("lesson", "tokio::time::sleep panics in the synchronous agent loop",
			 "The harness agent loop is single-threaded with no tokio runtime; calling tokio::time::sleep panicked at runtime. Use std::thread::sleep for the inter-attempt backoff delay instead."),
			("bugfix", "worktree confinement for file tools (sandbox escape fix)",
			 "File tools could reach outside the worktree via absolute paths and parent traversal. Canonicalize and assert the path stays under the worktree root before any read or write."),
			("decision", "Strategy A — Rust-native, local-first, hardware-specific harness",
			 "Build the harness core in Rust and own it; mine external references for designs, not runtime dependencies; target this machine's hardware; keep it forever-personal."),
			("constraint", "oMLX outputs are exercise-only and never landed",
			 "Generation and embedding results from the local server are throwaway: write under /tmp, ignored tests are acceptable, never commit anything derived from them."),
			("decision", "FTS5 external-content sidecar for the memory plane",
			 "The episodic store uses SQLite full-text search BM25 over an external-content table, with triggers keeping the index synchronized on insert, update, and delete."),
			("lesson", "salience is a pure tiebreak, not a weighted blend",
			 "recall orders by BM25 ascending then salience descending; the salience score never blends into the relevance ranking in the lexical slice."),
			("bugfix", "same-millisecond identifier collision in mint_id",
			 "Two memories minted within one millisecond produced colliding ids. Added a per-process atomic counter suffix so identifiers stay distinct and lexically sortable."),
			("decision", "reciprocal rank fusion for combining ranked signal lists",
			 "All-signals fusion with constant sixty: the combined score sums one over sixty-plus-rank across each input ranking. No hand-tuned weight blend."),
			("lesson", "WAL journal mode plus a busy timeout avoids dropped writes",
			 "Intermittent lost rows under two racing writers stopped after setting the write-ahead-log journal mode and a busy timeout on the connection."),
			("discovery", "the Align gate must be cleared by a human, not a machine",
			 "report_gate rejects a machine source for the criteria-confirmed gate; only a person can authorize the move out of the alignment step."),
			("constraint", "commit discipline — stage named paths, never add everything",
			 "Only commit when asked; stage explicit file paths; never use add-all; never stage editor cruft or the local agent settings directory."),
			("lesson", "clamp injected memory bodies at both head and tail",
			 "A head-only truncation dropped the actionable closing half of an explore post-mortem. The clamp now keeps both ends around an elision marker."),
			("feature", "auto-context priming injects recalled lessons into worker prompts",
			 "recall_primed pulls project-scoped post-mortems and injects them under a recalled-experience heading in the worker's system prompt before it starts."),
			("refactor", "single shared recall query so the two ops never drift",
			 "Extracted one source-of-truth SQL for the pre-filter and BM25 ordering; the index op and the body op differ only in projection, never in the query."),
		];
		// RECOVERY set: the 14 paraphrase queries + the title-substring of the true target.
		let recovery: &[(&str, &str)] = &[
			("make the retry wrapper wait between attempts without crashing the runtime", "tokio::time::sleep"),
			("stop the agent from escaping its isolated work area into the wider repository", "worktree confinement"),
			("why did we choose to own the core in rust instead of vendoring dependencies", "Strategy A"),
			("can I keep the embeddings the local server produced", "oMLX outputs are exercise-only"),
			("how does the recall store do keyword matching under the hood", "FTS5 external-content"),
			("does the importance weight get added into the ranking math", "salience is a pure tiebreak"),
			("two records written in the same instant overwrote each other", "same-millisecond identifier"),
			("how do we merge several ranked result lists into one ordering", "reciprocal rank fusion"),
			("rows vanished when two things wrote to the database at once", "WAL journal mode"),
			("can the model approve its own alignment step", "Align gate must be cleared by a human"),
			("rules for what to put in a git commit", "commit discipline"),
			("the recalled hint lost its useful second half when shortened", "clamp injected memory bodies"),
			("worktree confinement for file tools", "worktree confinement"),
			("reciprocal rank fusion", "reciprocal rank fusion"),
		];
		// NOISE set: realistic ticket-shaped titles on topics with NO memory in the store.
		// Each may share incidental token(s) with a row; the correct result is EMPTY.
		let noise: &[&str] = &[
			"add a dark mode toggle to the settings page",
			"fix the flaky timeout in the integration test suite",
			"upgrade the project to the latest serde version",
			"write end-user documentation for the public api endpoints",
			"investigate high memory usage in the image resizer",
			"add pagination to the search results list",
			"rename the user profile fields in the database",
			"cache the rendered markdown for faster page loads",
			"support webp images in the upload handler",
			"add a healthcheck endpoint for the load balancer",
			"throttle outbound webhook delivery under backpressure",
			"migrate the build from make to a justfile",
		];

		let b = mem();
		for (ty, title, body) in corpus {
			b.remember(&nm(ty, title, Some(body), 0.5)).unwrap();
		}
		const K: usize = 5;

		// Floorless top-K with raw BM25 score, mirroring recall_rows' ordering exactly.
		let scored = |query: &str| -> Vec<(String, String, f64)> {
			let Some(match_expr) = fts_query(query) else { return Vec::new() };
			let conn = b.conn();
			let mut stmt = conn
				.prepare(
					"SELECT m.title, m.body, bm25(memory_fts) AS score
					   FROM memory_fts JOIN memory m ON m.rowid = memory_fts.rowid
					  WHERE memory_fts MATCH ?1 AND m.evicted_at IS NULL
					  ORDER BY bm25(memory_fts) ASC, m.salience DESC LIMIT ?2",
				)
				.unwrap();
			stmt.query_map(params![match_expr, K as i64], |r| {
				Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default(), r.get::<_, f64>(2)?))
			})
			.unwrap()
			.collect::<rusqlite::Result<Vec<_>>>()
			.unwrap()
		};
		// overlap count for a candidate, exactly as recall_primed computes it.
		let overlap = |query: &str, title: &str, body: &str| -> usize {
			let q: Vec<String> = overlap_tokens(query);
			let hay: std::collections::HashSet<String> =
				overlap_tokens(&format!("{title} {body}")).into_iter().collect();
			q.iter().filter(|t| hay.contains(*t)).count()
		};
		let need_a = |query: &str| -> usize { overlap_tokens(query).len().clamp(1, 2) };

		// Sweep BM25 thresholds for policy C (keep if overlap≥1 AND score ≤ thresh).
		// BM25 here is negative; MORE negative = stronger match, so a smaller (more
		// negative) threshold is stricter. Picked to bracket the observed distribution.
		let thresholds = [-8.0_f64, -6.0, -5.0, -4.0, -3.0, -2.0];

		println!("\n==================== t5 FLOOR PRECISION COUNTER-PROBE ====================");
		println!("corpus={} rows, K={K}, recovery queries={}, noise queries={}",
			corpus.len(), recovery.len(), noise.len());

		// ---- RECOVERY: did the true target survive the policy filter within top-K? ----
		let mut rec_a = 0usize;
		let mut rec_b = 0usize;
		let mut rec_c = [0usize; 6];
		println!("\n--- single-token-overlap candidates in the RECOVERY set (the decisive regime) ---");
		for (q, target) in recovery {
			let cands = scored(q);
			let need = need_a(q);
			let mut keep_a = false;
			let mut keep_b = false;
			let mut keep_c = [false; 6];
			for (title, body, score) in &cands {
				if !title.to_lowercase().contains(&target.to_lowercase()) {
					continue; // only the true target counts for recovery
				}
				let ov = overlap(q, title, body);
				if ov >= need { keep_a = true; }
				if ov >= 1 { keep_b = true; }
				for (i, th) in thresholds.iter().enumerate() {
					if ov >= 1 && *score <= *th { keep_c[i] = true; }
				}
				if ov == 1 {
					println!("  RECOVER q=\"{:.38}\"  target overlap={ov} need={need} bm25={:.2}", q, score);
				}
			}
			if keep_a { rec_a += 1; }
			if keep_b { rec_b += 1; }
			for i in 0..thresholds.len() { if keep_c[i] { rec_c[i] += 1; } }
		}

		// ---- NOISE: how many noise queries leak ≥1 hit under each policy? ----
		let mut noise_a = 0usize;
		let mut noise_b = 0usize;
		let mut noise_c = [0usize; 6];
		println!("\n--- single-token-overlap candidates in the NOISE set (false positives the floor should kill) ---");
		for q in noise {
			let cands = scored(q);
			let need = need_a(q);
			let mut hit_a = false;
			let mut hit_b = false;
			let mut hit_c = [false; 6];
			for (title, body, score) in &cands {
				let ov = overlap(q, title, body);
				if ov >= need { hit_a = true; }
				if ov >= 1 { hit_b = true; }
				for (i, th) in thresholds.iter().enumerate() {
					if ov >= 1 && *score <= *th { hit_c[i] = true; }
				}
				if ov == 1 {
					println!("  NOISE   q=\"{:.38}\"  -> \"{:.30}\" overlap={ov} need={need} bm25={:.2}", q, title, score);
				}
			}
			if hit_a { noise_a += 1; }
			if hit_b { noise_b += 1; }
			for i in 0..thresholds.len() { if hit_c[i] { noise_c[i] += 1; } }
		}

		let n_rec = recovery.len();
		let n_noise = noise.len();
		println!("\n==================== VERDICT ====================");
		println!("policy                         recovery        noise(false-pos queries)");
		println!("A: need≥2 (SHIPPED)            {rec_a}/{n_rec}           {noise_a}/{n_noise}");
		println!("B: need≥1 (blind relax)        {rec_b}/{n_rec}           {noise_b}/{n_noise}");
		for (i, th) in thresholds.iter().enumerate() {
			println!("C: need≥1 & bm25≤{:>5.1}        {}/{n_rec}           {}/{n_noise}", th, rec_c[i], noise_c[i]);
		}
		println!("=================================================\n");
	}

	// ───────────────────────────────────────────────────────────────────────────
	// t5 FLOOR EVAL v2 (S1, research/31 §4) — re-calibration of the BM25-score floor
	// on an EXPANDED golden set (36 corpus rows, 3 provenance classes), with a
	// deterministic calibration/holdout split so the chosen θ is NOT overfit, and an
	// exact two-sided binomial McNemar between the shipped floor (a) and the candidate
	// floor (b′). The v1 probe above measured θ on 14 rows and was honestly flagged as
	// overfit; this is the gate that decides whether t5 ships. Throwaway measurement
	// (`#[ignore]`); run with `cargo test -p board -- --ignored --nocapture t5_floor_eval_v2`.
	//
	// Arms:  a  = need ≥ min(2, n_query_tokens) distinct ≥3-char token overlap (SHIPPED)
	//        b  = need ≥ 1 overlap (blind relax — the floor we will NOT ship)
	//        b′ = need ≥ 1 overlap AND bm25 ≤ θ  (the candidate t5 fix)
	// Ship rule (§4): b′ ships iff, on the HELD-OUT split, it beats (a) on recovery
	// AND its noise-leak is not worse than (a), with McNemar p<0.05 on discordant pairs.
	// An honest null (directional but underpowered) is an acceptable, reportable outcome.
	#[test]
	#[ignore = "throwaway re-calibration eval; run with --ignored --nocapture"]
	fn t5_floor_eval_v2() {
		// (key, type, title, body) — 14 v1 rows + 22 new atomic lessons (S1.1 fan-out).
		let corpus: &[(&str, &str, &str, &str)] = &[
			("tokio_sleep", "lesson", "tokio::time::sleep panics in the synchronous agent loop",
			 "The harness agent loop is single-threaded with no tokio runtime; calling tokio::time::sleep panicked at runtime. Use std::thread::sleep for the inter-attempt backoff delay instead."),
			("worktree_confine", "bugfix", "worktree confinement for file tools (sandbox escape fix)",
			 "File tools could reach outside the worktree via absolute paths and parent traversal. Canonicalize and assert the path stays under the worktree root before any read or write."),
			("strategy_a", "decision", "Strategy A — Rust-native, local-first, hardware-specific harness",
			 "Build the harness core in Rust and own it; mine external references for designs, not runtime dependencies; target this machine's hardware; keep it forever-personal."),
			("omlx_exercise", "constraint", "oMLX outputs are exercise-only and never landed",
			 "Generation and embedding results from the local server are throwaway: write under /tmp, ignored tests are acceptable, never commit anything derived from them."),
			("fts5_sidecar", "decision", "FTS5 external-content sidecar for the memory plane",
			 "The episodic store uses SQLite full-text search BM25 over an external-content table, with triggers keeping the index synchronized on insert, update, and delete."),
			("salience_tiebreak", "lesson", "salience is a pure tiebreak, not a weighted blend",
			 "recall orders by BM25 ascending then salience descending; the salience score never blends into the relevance ranking in the lexical slice."),
			("mint_id_collision", "bugfix", "same-millisecond identifier collision in mint_id",
			 "Two memories minted within one millisecond produced colliding ids. Added a per-process atomic counter suffix so identifiers stay distinct and lexically sortable."),
			("rrf_fusion", "decision", "reciprocal rank fusion for combining ranked signal lists",
			 "All-signals fusion with constant sixty: the combined score sums one over sixty-plus-rank across each input ranking. No hand-tuned weight blend."),
			("wal_busy", "lesson", "WAL journal mode plus a busy timeout avoids dropped writes",
			 "Intermittent lost rows under two racing writers stopped after setting the write-ahead-log journal mode and a busy timeout on the connection."),
			("align_human", "discovery", "the Align gate must be cleared by a human, not a machine",
			 "report_gate rejects a machine source for the criteria-confirmed gate; only a person can authorize the move out of the alignment step."),
			("commit_discipline", "constraint", "commit discipline — stage named paths, never add everything",
			 "Only commit when asked; stage explicit file paths; never use add-all; never stage editor cruft or the local agent settings directory."),
			("clamp_body", "lesson", "clamp injected memory bodies at both head and tail",
			 "A head-only truncation dropped the actionable closing half of an explore post-mortem. The clamp now keeps both ends around an elision marker."),
			("priming", "feature", "auto-context priming injects recalled lessons into worker prompts",
			 "recall_primed pulls project-scoped post-mortems and injects them under a recalled-experience heading in the worker's system prompt before it starts."),
			("shared_recall", "refactor", "single shared recall query so the two ops never drift",
			 "Extracted one source-of-truth SQL for the pre-filter and BM25 ordering; the index op and the body op differ only in projection, never in the query."),
			// ---- S1.1 new lessons ----
			("kind_profiles", "feature", "ticket kind selects a per-kind gate profile",
			 "kind is first-class; code kinds like build, bugfix and refactor require the code gate set via a kind_is_code matcher while non-code kinds skip them, so a novel kind never silently acquires un-runnable gates."),
			("land_worktree", "decision", "land uses plain git worktree, not pi-iso",
			 "Landing needs branch and base control plus a squash merge; pi-iso is built for run then extract-diff then discard, so it stays a workspace crate and was dropped from the agent dependencies."),
			("confusion_bounce", "feature", "a logged confusion bounces the ticket back to Align",
			 "Recording a confusion on an in-progress ticket sends it back to the alignment state and re-locks the tools until the operator re-aligns."),
			("ready_no_cte", "lesson", "the ready predicate needs no recursive CTE",
			 "Direct blockers are terminal so the readiness check is cycle-safe by construction; it shipped as a plain NOT EXISTS instead of the originally planned recursive common table expression."),
			("harden_dispatch", "decision", "harden dispatch keys on diff content, not repo layout",
			 "Mutation testing is dispatched by which file extensions changed — Rust goes to cargo-mutants, Python to cosmic-ray, neither yields a vacuous pass — because diff content is unambiguous where repo layout is not."),
			("cosmic_ray", "decision", "python harden runs cosmic-ray as an operator tool",
			 "cosmic-ray is installed like git and cargo-mutants, not linked as a crate dependency; the pipeline is config then baseline then init then filter-git then exec then dump, judged by parsing the dump output."),
			("mutation_score", "constraint", "mutation score excludes unviable and counts timeouts against",
			 "The score is caught over caught plus missed plus timeout; un-compilable unviable mutants are excluded as roughly equivalent, and timeouts count against as un-killed mutants."),
			("oracle_intact", "feature", "oracle integrity gate forbids editing committed tests",
			 "For build and bugfix tickets a convention-matched oracle file that changed versus base fails the gate and bails; the agent may add new tests but may not edit a committed oracle."),
			("loop_gate_sig", "feature", "the loop gate detects identical consecutive tool calls",
			 "A signature over each turn's ordered tool-call names and arguments fingerprints repetition; run-level strikes nudge on the first and stop on the second, robust to two-cycle switching."),
			("loop_gate_agnostic", "decision", "the loop gate keys on provider-agnostic response fields",
			 "Detection reads only the tool calls and the stop reason from the response, so the identical mechanism works on the anthropic backend and the local server alike."),
			("intra_turn", "bugfix", "intra-turn over-deliberation escapes the loop gate",
			 "The loop gate only catches repetition across turns; a model re-analyzing one anchor within a single content field instead hits the length cutoff and is labeled a truncated stop."),
			("think_ladder", "decision", "bounded thinking is a fixed preset ladder",
			 "The thinking budget uses fixed presets — off, then low, medium and high at 1024, 2048 and 4096 — defaulting to 2048 and env-overridable, with no per-turn self-triage which is unreliable and adds a call."),
			("no_refeed", "constraint", "bounded-thinking reasoning is never re-fed to context",
			 "Thinking-on helps only the turn that produces the output; re-feeding the reasoning burns the window for no gain because the local model has no signed thinking chain to preserve."),
			("context_reasoning_bound", "lesson", "the context window is reasoning-bound, not retrieval-bound",
			 "The advertised 256K native window is a retrieval claim; effective two-hop reasoning degrades by roughly 16 to 32K, so compaction triggers at 32K with a 20K keep-recent to match the reasoning budget."),
			("qwen_untagged", "discovery", "the local Qwen emits untagged reasoning, no think tags",
			 "The served Qwen model never emits think tags; its reasoning lands untagged in the prose content field, so the think-strip design anticipated a signal that does not exist."),
			("adv_review", "decision", "adversarial review is mandatory for sticky or silent designs",
			 "A design-stage adversarial review is required, not honor-system, when a mistake is hard to reverse or silent — schema, state-machine primitives, races, retrieval quality — applied before the first build slice."),
			("user_version", "lesson", "a user_version migrator precedes any schema change",
			 "A base schema using only create-table-if-not-exists silently no-ops on a column add, so a user_version migrator is the prerequisite for safe schema evolution and deferral."),
			("trajectory_derived", "decision", "a run's trajectory path is derived, never stored",
			 "Trajectory paths are a pure function of ticket, attempt and run id and are derived at read time, since storing them risks a dangling path after the worktree is removed."),
			("raw_signals", "constraint", "outcome signals are recorded raw, never pre-scalarized",
			 "Telemetry records raw outcome signals and defers the reward function over those signals to the training pipeline, rather than baking a reward shape into capture."),
			("mem_provenance", "discovery", "memory ops own provenance and skip the event table",
			 "Memory mutations do not write the ticket-scoped event audit table because memories can be project or global scoped; they instead carry their own created, usage and evicted timestamps."),
			("type_rust_scope_sql", "decision", "memory type validates in Rust, scope in SQL",
			 "scope uses an immutable SQL check constraint while type validates against a Rust const set with no check, because the case-law type vocabulary grows and a check would force a migration each time."),
			("soft_delete", "decision", "evicted memory soft-deletes, never hard-deletes",
			 "Eviction stamps an evicted timestamp and every read filters it out, preserving provenance so a later model-swap guard can still reason over the historical rows."),
		];

		// (query, Option<target_key>, provenance). None target ⇒ noise (correct = empty).
		// provenance ∈ {"clean","para","noise"}. clean = near-verbatim title (regression
		// guard, both arms should recover); para = symptom-framed, the single-token regime
		// where (a) is expected to fail and (b′) to recover; noise = topic absent from store.
		let queries: &[(&str, Option<&str>, &str)] = &[
			// ---- clean (near-verbatim) ----
			("tokio sleep panics in the agent loop", Some("tokio_sleep"), "clean"),
			("worktree confinement for file tools", Some("worktree_confine"), "clean"),
			("FTS5 external-content sidecar memory plane", Some("fts5_sidecar"), "clean"),
			("reciprocal rank fusion ranked signal lists", Some("rrf_fusion"), "clean"),
			("WAL journal mode busy timeout dropped writes", Some("wal_busy"), "clean"),
			("commit discipline stage named paths", Some("commit_discipline"), "clean"),
			("loop gate identical consecutive tool calls", Some("loop_gate_sig"), "clean"),
			("bounded thinking preset ladder budget", Some("think_ladder"), "clean"),
			("mutation score unviable timeouts", Some("mutation_score"), "clean"),
			("oracle integrity committed tests", Some("oracle_intact"), "clean"),
			("user_version migrator schema change", Some("user_version"), "clean"),
			("memory type validates rust scope sql", Some("type_rust_scope_sql"), "clean"),
			// ---- para (symptom-framed, single-token-overlap regime) ----
			("make the backoff wait between retries without crashing", Some("tokio_sleep"), "para"),
			("keep the model inside its sandbox and out of the main checkout", Some("worktree_confine"), "para"),
			("why own the core instead of pulling in libraries", Some("strategy_a"), "para"),
			("are the vectors from the local server safe to keep", Some("omlx_exercise"), "para"),
			("how does keyword search work in the recall store", Some("fts5_sidecar"), "para"),
			("does the importance weight bleed into the ranking", Some("salience_tiebreak"), "para"),
			("two records written the same instant clobbered each other", Some("mint_id_collision"), "para"),
			("blend several ranked lists into one ordering", Some("rrf_fusion"), "para"),
			("rows disappeared under two concurrent writers", Some("wal_busy"), "para"),
			("can the agent sign off its own alignment step", Some("align_human"), "para"),
			("the injected hint lost its useful ending when trimmed", Some("clamp_body"), "para"),
			("pick the mutation tool from which files changed", Some("harden_dispatch"), "para"),
			("stop the model rewriting the committed test files", Some("oracle_intact"), "para"),
			("catch the agent repeating the same action turn after turn", Some("loop_gate_sig"), "para"),
			("the model keeps re-reading one spot and runs out of tokens", Some("intra_turn"), "para"),
			("how large is the effective reasoning window really", Some("context_reasoning_bound"), "para"),
			("does the local model wrap its thinking in tags", Some("qwen_untagged"), "para"),
			("when is a design review required before building", Some("adv_review"), "para"),
			("how do we add a column without breaking old databases", Some("user_version"), "para"),
			("where does a run's trajectory file live", Some("trajectory_derived"), "para"),
			("evicted records should still leave a trace behind", Some("soft_delete"), "para"),
			// ---- noise (no memory; correct answer is EMPTY) ----
			("add a dark mode toggle to the settings page", None, "noise"),
			("fix the flaky timeout in the integration test suite", None, "noise"),
			("upgrade the project to the latest serde version", None, "noise"),
			("write end-user documentation for the public api endpoints", None, "noise"),
			("investigate high memory usage in the image resizer", None, "noise"),
			("add pagination to the search results list", None, "noise"),
			("rename the user profile fields in the database", None, "noise"),
			("cache the rendered markdown for faster page loads", None, "noise"),
			("support webp images in the upload handler", None, "noise"),
			("add a healthcheck endpoint for the load balancer", None, "noise"),
			("throttle outbound webhook delivery under backpressure", None, "noise"),
			("migrate the build from make to a justfile", None, "noise"),
			("add oauth login with google as a provider", None, "noise"),
			("reduce docker image size for the web frontend", None, "noise"),
			("add a csv export button to the dashboard", None, "noise"),
			("fix the off-by-one in the date picker widget", None, "noise"),
			("localize the interface strings into spanish", None, "noise"),
			("add rate limiting to the public rest endpoints", None, "noise"),
			("replace the spinner with a skeleton loader", None, "noise"),
			("compress old log files with zstd every night", None, "noise"),
			("add keyboard shortcuts to the editor pane", None, "noise"),
			("paginate the audit viewer for large histories", None, "noise"),
			("add a profile avatar cropper to onboarding", None, "noise"),
			("send a weekly digest email to inactive users", None, "noise"),
		];

		let b = mem();
		let title_to_key: std::collections::HashMap<String, &str> =
			corpus.iter().map(|(k, _, t, _)| (t.to_string(), *k)).collect();
		for (_k, ty, title, body) in corpus {
			b.remember(&nm(ty, title, Some(body), 0.5)).unwrap();
		}
		const K: usize = 5;

		// Floorless top-K with raw BM25 score (mirrors recall_rows' ordering exactly),
		// returning (key, score) so recovery can match by stable key not title-substring.
		let scored = |query: &str| -> Vec<(&str, f64)> {
			let Some(match_expr) = fts_query(query) else { return Vec::new() };
			let conn = b.conn();
			let mut stmt = conn
				.prepare(
					"SELECT m.title, bm25(memory_fts) AS score
					   FROM memory_fts JOIN memory m ON m.rowid = memory_fts.rowid
					  WHERE memory_fts MATCH ?1 AND m.evicted_at IS NULL
					  ORDER BY bm25(memory_fts) ASC, m.salience DESC LIMIT ?2",
				)
				.unwrap();
			stmt.query_map(params![match_expr, K as i64], |r| {
				Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
			})
			.unwrap()
			.collect::<rusqlite::Result<Vec<_>>>()
			.unwrap()
			.into_iter()
			.map(|(t, s)| (title_to_key[&t], s))
			.collect()
		};
		// overlap count for a candidate vs query, exactly as recall_primed computes it.
		let overlap = |query: &str, key: &str| -> usize {
			let (_k, _ty, title, body) = corpus.iter().find(|(k, ..)| k == &key).unwrap();
			let q: Vec<String> = overlap_tokens(query);
			let hay: std::collections::HashSet<String> =
				overlap_tokens(&format!("{title} {body}")).into_iter().collect();
			q.iter().filter(|t| hay.contains(*t)).count()
		};
		let need_a = |query: &str| -> usize { overlap_tokens(query).len().clamp(1, 2) };

		// Content-token overlap (stopword-stripped) — the candidate t5 fix a*.
		let content_overlap = |query: &str, key: &str| -> usize {
			let (_k, _ty, title, body) = corpus.iter().find(|(k, ..)| k == &key).unwrap();
			let q: Vec<String> = content_tokens(query);
			let hay: std::collections::HashSet<String> =
				content_tokens(&format!("{title} {body}")).into_iter().collect();
			q.iter().filter(|t| hay.contains(*t)).count()
		};
		let need_astar = |query: &str| -> usize { content_tokens(query).len().clamp(1, 2) };

		let thresholds = [-10.0_f64, -8.0, -7.0, -6.0, -5.0, -4.5, -4.0, -3.5, -3.0, -2.5, -2.0, -1.5, -1.0];

		// Per-query correctness under each arm. For a target query: correct ⇔ the target
		// key survives the arm's floor within top-K. For noise: correct ⇔ arm keeps zero.
		let correct_a = |q: &str, target: Option<&str>| -> bool {
			let cands = scored(q);
			let need = need_a(q);
			let kept: Vec<&str> = cands.iter().filter(|(k, _)| overlap(q, k) >= need).map(|(k, _)| *k).collect();
			match target { Some(t) => kept.contains(&t), None => kept.is_empty() }
		};
		let correct_b = |q: &str, target: Option<&str>| -> bool {
			let cands = scored(q);
			let kept: Vec<&str> = cands.iter().filter(|(k, _)| overlap(q, k) >= 1).map(|(k, _)| *k).collect();
			match target { Some(t) => kept.contains(&t), None => kept.is_empty() }
		};
		let correct_bp = |q: &str, target: Option<&str>, th: f64| -> bool {
			let cands = scored(q);
			let kept: Vec<&str> =
				cands.iter().filter(|(k, s)| overlap(q, k) >= 1 && *s <= th).map(|(k, _)| *k).collect();
			match target { Some(t) => kept.contains(&t), None => kept.is_empty() }
		};
		// a* — the candidate t5 fix: SAME need≥2 floor but on STOPWORD-STRIPPED tokens.
		let correct_astar = |q: &str, target: Option<&str>| -> bool {
			let cands = scored(q);
			let need = need_astar(q);
			let kept: Vec<&str> =
				cands.iter().filter(|(k, _)| content_overlap(q, k) >= need).map(|(k, _)| *k).collect();
			match target { Some(t) => kept.contains(&t), None => kept.is_empty() }
		};
		// a*1 — stopword-stripped but need≥1 (isolates the strip from the count).
		let correct_astar1 = |q: &str, target: Option<&str>| -> bool {
			let cands = scored(q);
			let kept: Vec<&str> =
				cands.iter().filter(|(k, _)| content_overlap(q, k) >= 1).map(|(k, _)| *k).collect();
			match target { Some(t) => kept.contains(&t), None => kept.is_empty() }
		};
		// c — the combined candidate: content need≥1 AND bm25≤θ (both levers together).
		let correct_c = |q: &str, target: Option<&str>, th: f64| -> bool {
			let cands = scored(q);
			let kept: Vec<&str> = cands
				.iter()
				.filter(|(k, s)| content_overlap(q, k) >= 1 && *s <= th)
				.map(|(k, _)| *k)
				.collect();
			match target { Some(t) => kept.contains(&t), None => kept.is_empty() }
		};

		// Deterministic calibration/holdout split: every other query within each
		// provenance class (even local index → calibration, odd → holdout). Keeps the
		// class balance even on both sides, and θ is picked ONLY on calibration.
		let mut calib: Vec<usize> = Vec::new();
		let mut holdout: Vec<usize> = Vec::new();
		{
			let mut per_class: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
			for (i, (_q, _t, prov)) in queries.iter().enumerate() {
				let c = per_class.entry(prov).or_insert(0);
				if (*c).is_multiple_of(2) { calib.push(i) } else { holdout.push(i) }
				*c += 1;
			}
		}

		// Counts over a slice of query indices, for one arm given as a closure.
		let tally = |idxs: &[usize], f: &dyn Fn(&str, Option<&str>) -> bool| -> (usize, usize, usize, usize) {
			// returns (recovery_hits, recovery_total, noise_leaks, noise_total)
			let (mut rh, mut rt, mut nl, mut nt) = (0, 0, 0, 0);
			for &i in idxs {
				let (q, target, _prov) = queries[i];
				if target.is_some() {
					rt += 1;
					if f(q, target) { rh += 1 }
				} else {
					nt += 1;
					if !f(q, None) { nl += 1 } // leak = NOT correct on a noise query
				}
			}
			(rh, rt, nl, nt)
		};

		// ---- θ selection on CALIBRATION for arm c (content need≥1 & bm25≤θ): max
		// recovery s.t. noise-leak ≤ the STRICT content arm a* (need≥2 content). The
		// budget is the principled precision benchmark ("empty beats noise", Slice A),
		// fixed before reading the result — c must be at least as precise as a* while
		// recovering more. ----
		let (_, _, astar_noise_calib, _) = tally(&calib, &correct_astar);
		let mut best_th = thresholds[0];
		let mut best_rec = -1i64;
		let mut best_leak = usize::MAX;
		for &th in &thresholds {
			let (rh, _, nl, _) = tally(&calib, &move |q, t| correct_c(q, t, th));
			if nl <= astar_noise_calib && (rh as i64 > best_rec || (rh as i64 == best_rec && nl < best_leak)) {
				best_rec = rh as i64;
				best_leak = nl;
				best_th = th;
			}
		}
		let viable = best_rec >= 0;

		// ---- McNemar (exact two-sided binomial) between any two arms on a slice ----
		let mcnemar_pair = |idxs: &[usize],
		                    f: &dyn Fn(&str, Option<&str>) -> bool,
		                    g: &dyn Fn(&str, Option<&str>) -> bool|
		 -> (usize, usize, f64) {
			let (mut bo, mut co) = (0usize, 0usize); // bo=f-correct&g-wrong, co=f-wrong&g-correct
			for &i in idxs {
				let (q, target, _p) = queries[i];
				let x = f(q, target);
				let y = g(q, target);
				if x && !y { bo += 1 }
				if !x && y { co += 1 }
			}
			(bo, co, binom_two_sided(bo, co))
		};

		let report = |label: &str, idxs: &[usize]| {
			let (arh, art, anl, ant) = tally(idxs, &correct_a);
			let (brh, _, bnl, _) = tally(idxs, &correct_b);
			let (srh, _, snl, _) = tally(idxs, &correct_astar);
			let (s1rh, _, s1nl, _) = tally(idxs, &correct_astar1);
			let (prh, _, pnl, _) = tally(idxs, &move |q, t| correct_bp(q, t, best_th));
			println!("\n--- {label} (n={}) ---", idxs.len());
			println!("arm                                   recovery       noise-leak");
			println!("a:  need≥2 raw          (SHIPPED)     {arh}/{art}          {anl}/{ant}");
			println!("b:  need≥1 raw          (blind relax) {brh}/{art}          {bnl}/{ant}");
			println!("b′: need≥1 raw & bm25≤{best_th:<5}          {prh}/{art}          {pnl}/{ant}");
			println!("a*: need≥2 stopword-stripped         {srh}/{art}          {snl}/{ant}");
			println!("a*1:need≥1 stopword-stripped         {s1rh}/{art}          {s1nl}/{ant}");
			let (crh, _, cnl, _) = tally(idxs, &move |q, t| correct_c(q, t, best_th));
			println!("c:  content need≥1 & bm25≤{best_th:<5}      {crh}/{art}          {cnl}/{ant}");
			// Per-axis McNemar a vs a* (the FIX). Mixed-set McNemar cancels a precision
			// gain against a recall cost ~1:1 in COUNT and reads ~p=1.0 — wrong frame.
			// Decompose: recovery-only (does a* cost recall?) and noise-only (does a*
			// fix precision?), each its own discordant test.
			let rec_idx: Vec<usize> = idxs.iter().copied().filter(|&i| queries[i].1.is_some()).collect();
			let noi_idx: Vec<usize> = idxs.iter().copied().filter(|&i| queries[i].1.is_none()).collect();
			let (rb, rc, rp) = mcnemar_pair(&rec_idx, &correct_a, &correct_astar);
			let (nb, nc, np) = mcnemar_pair(&noi_idx, &correct_a, &correct_astar);
			println!("McNemar a vs a*  | RECALL : discordant a✓a*✗={rb} a✗a*✓={rc}  p={rp:.4} (recall cost — a* kills the t5 regime)");
			println!("McNemar a vs a*  | NOISE  : discordant a✓a*✗={nb} a✗a*✓={nc}  p={np:.5} (precision gain)");
			// a vs a*1 — the ACTUAL ship: strip stopwords, keep need≥1. Recall held, noise cut.
			let (r1b, r1c, r1p) = mcnemar_pair(&rec_idx, &correct_a, &correct_astar1);
			let (n1b, n1c, n1p) = mcnemar_pair(&noi_idx, &correct_a, &correct_astar1);
			println!("McNemar a vs a*1 | RECALL : discordant a✓a*1✗={r1b} a✗a*1✓={r1c}  p={r1p:.4} (recall held — t5 regime intact)");
			println!("McNemar a vs a*1 | NOISE  : discordant a✓a*1✗={n1b} a✗a*1✓={n1c}  p={n1p:.5} (precision gain, no constant)");
		};

		println!("\n==================== t5 FLOOR EVAL v2 (S1) ====================");
		println!("corpus={} rows  queries={} (clean+para+noise)  K={K}", corpus.len(), queries.len());
		println!("calibration n={}  holdout n={}", calib.len(), holdout.len());
		if viable {
			println!("θ* selected on CALIBRATION = {best_th} (recovery {best_rec}, noise-leak {best_leak} ≤ a*'s {astar_noise_calib})");
		} else {
			println!("NO viable θ on calibration (no θ keeps noise-leak ≤ arm a*). Honest null — do NOT ship.");
		}

		// θ sweep on the FULL set for BOTH bm25-floor arms (diagnostic — the frontier).
		let all: Vec<usize> = (0..queries.len()).collect();
		println!("\n--- θ sweep on FULL set: b′=raw+bm25  vs  c=content+bm25 (recovery / noise-leak) ---");
		let (afr, aft, afn, afnt) = tally(&all, &correct_a);
		println!("a:  need≥2 raw (SHIPPED)              {afr}/{aft}          {afn}/{afnt}");
		for &th in &thresholds {
			let (br, _, bn, _) = tally(&all, &move |q, t| correct_bp(q, t, th));
			let (cr, ct, cn, cnt) = tally(&all, &move |q, t| correct_c(q, t, th));
			println!("bm25≤{th:<6}   b′ {br}/{aft} {bn}/{afnt}     c {cr}/{ct} {cn}/{cnt}");
		}

		report("CALIBRATION", &calib);
		report("HELD-OUT", &holdout);
		report("FULL", &(0..queries.len()).collect::<Vec<_>>());

		// Decisive-regime cut: para queries whose target shares exactly 1 CONTENT token
		// (stopword-stripped) with the query — the true single-token regime (the raw cut
		// was empty because stopwords inflated every overlap). These are the cases a*
		// (need≥2 content) drops, and the honest cost of the precision win.
		let cut: Vec<usize> = queries
			.iter()
			.enumerate()
			.filter(|(_, (q, t, prov))| *prov == "para" && t.map(|k| content_overlap(q, k) == 1).unwrap_or(false))
			.map(|(i, _)| i)
			.collect();
		report("CONTENT-OVERLAP==1 PARA CUT (the true t5 regime)", &cut);

		println!("\n--- paraphrases a* (need≥2 content) DROPS (the recall cost, itemized) ---");
		for &(q, target, prov) in queries {
			if prov == "para"
				&& let Some(t) = target
				&& correct_a(q, target)
				&& !correct_astar(q, target)
			{
				println!("  DROP co={} q=\"{:.46}\" → {t}", content_overlap(q, t), q);
			}
		}

		println!("\n==================== END t5 FLOOR EVAL v2 ====================\n");
	}

	/// Exact two-sided McNemar p-value via the binomial sign test on discordant pairs:
	/// under H0 each discordant pair is a fair coin, so p = 2·P(X ≥ max(b,c)) with
	/// X ~ Binom(b+c, 0.5), clamped to 1.0. Zero discordant pairs → p = 1.0 (no signal).
	/// Standalone (no statistics dep) — n is tiny so the f64 sum is exact enough.
	fn binom_two_sided(b: usize, c: usize) -> f64 {
		let n = b + c;
		if n == 0 {
			return 1.0;
		}
		let k = b.max(c);
		let mut tail = 0.0f64;
		for i in k..=n {
			tail += binom_coeff(n, i);
		}
		(2.0 * tail / 2f64.powi(n as i32)).min(1.0)
	}

	/// C(n, k) as f64 via the multiplicative formula (n small here).
	fn binom_coeff(n: usize, k: usize) -> f64 {
		let k = k.min(n - k);
		let mut acc = 1.0f64;
		for i in 0..k {
			acc = acc * (n - i) as f64 / (i + 1) as f64;
		}
		acc
	}

	// overlap_tokens drops <3-char boilerplate and de-dupes; clamp_body keeps BOTH
	// ends so an explore post-mortem's Strategy head and validation tail survive.
	#[test]
	fn overlap_tokens_and_clamp_body_are_disciplined() {
		let toks = overlap_tokens("Use the rustls TLS handshake to to fix it");
		assert!(toks.contains(&"rustls".to_string()) && toks.contains(&"handshake".to_string()));
		assert!(!toks.iter().any(|t| t.len() < 3), "short boilerplate dropped");
		assert_eq!(toks.iter().filter(|t| *t == "to").count(), 0, "≥3-char only");

		let short = "short body";
		assert_eq!(clamp_body(short, PRIMED_BODY_CAP), short, "under cap → unchanged");
		let long = format!("Strategy: {}{}", "x".repeat(500), " did not satisfy `cargo test`");
		let c = clamp_body(&long, PRIMED_BODY_CAP);
		assert!(c.chars().count() <= PRIMED_BODY_CAP, "clamped to cap");
		assert!(c.starts_with("Strategy:"), "head survives");
		assert!(c.ends_with("`cargo test`"), "tail survives a head+tail clamp");
		assert!(c.contains('…'), "elision marker present");
	}
}
