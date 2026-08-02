//! The rusqlite DDL — four tables, stolen lean from `br` (research/15) with the
//! slice-1 corrections baked into the keys:
//!  - `edge` PK carries `kind` → a pair can be both parent-child AND blocks (C1 #1).
//!  - `gate_results` PK carries `attempt` + a `source` col → rework invalidates
//!    prior passes, and human gates can't be machine-cleared (C1 #4, C3 #1/#2).
//!  - `event` is append-only → the JSONL export is a history, not a state dump (C1 #7).
//!
//! On top of the v0 baseline sits a `user_version` migrator (research/17 §8 M1):
//! `CREATE TABLE IF NOT EXISTS` silently no-ops on a column add, so additive
//! evolution needs a ratchet, not just idempotent DDL. `init` also sets WAL + a
//! busy_timeout (M3) so the coordinator's parallel writers don't drop rows.
//! Migration v1 adds the telemetry `run` table.

use anyhow::{Context, Result};
use rusqlite::Connection;

pub const SCHEMA: &str = "\
CREATE TABLE IF NOT EXISTS ticket (
    id                   TEXT PRIMARY KEY,
    kind                 TEXT NOT NULL,
    status               TEXT NOT NULL CHECK (status IN
                            ('todo','align','in_progress','verify','review','land','done','rework')),
    title                TEXT NOT NULL,
    plan                 TEXT,
    acceptance_criteria  TEXT,
    validation           TEXT,
    notes                TEXT,
    confusions           TEXT,
    priority             INTEGER NOT NULL DEFAULT 2,
    attempt              INTEGER NOT NULL DEFAULT 0,
    created_at           TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at           TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS edge (
    issue_id       TEXT NOT NULL,
    depends_on_id  TEXT NOT NULL,
    kind           TEXT NOT NULL DEFAULT 'blocks',
    created_at     TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (issue_id, depends_on_id, kind)
);

CREATE TABLE IF NOT EXISTS gate_results (
    issue_id  TEXT NOT NULL,
    gate      TEXT NOT NULL,
    provider  TEXT NOT NULL,
    source    TEXT NOT NULL CHECK (source IN ('human','machine')),
    attempt   INTEGER NOT NULL,
    passed    INTEGER NOT NULL CHECK (passed IN (0, 1)),
    note      TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (issue_id, gate, provider, attempt)
);

CREATE TABLE IF NOT EXISTS event (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    issue_id     TEXT NOT NULL,
    ts           TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    kind         TEXT NOT NULL,
    from_status  TEXT,
    to_status    TEXT,
    provider     TEXT,
    note         TEXT
);
";

/// Migrations applied on top of the v0 baseline (`SCHEMA`). Index `i` is schema
/// version `i+1`; a migration runs only when `user_version` is below its target,
/// then `user_version` is bumped. The codebase's first real migration mechanism
/// (research/17 §8 M1) — additive column/table evolution without losing banked data.
const MIGRATIONS: &[&str] = &[
	// v1 — the telemetry run index (research/17 §4 Unit A). One row per loop
	// execution: written at entry (ended_at NULL), closed at exit, so a crash is
	// the queryable `ended_at IS NULL` (M2). `run_id` is a sortable, caller-minted
	// PK; `attempt` joins gate_results at the run's epoch; model/provider/sampling
	// are the provenance the RL/audit views need (M-provenance, §6).
	"CREATE TABLE IF NOT EXISTS run (
	    run_id       TEXT PRIMARY KEY,
	    ticket_id    TEXT NOT NULL,
	    attempt      INTEGER NOT NULL,
	    model        TEXT NOT NULL,
	    provider     TEXT NOT NULL,
	    sampling     TEXT,
	    project      TEXT,
	    started_at   TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
	    ended_at     TEXT,
	    iters        INTEGER,
	    stop_reason  TEXT
	);",
	// v2 — the memory sidecar (research/18 §10.2). Episodic/semantic store distinct
	// from the wiki: rows decay, the wiki doesn't. The full durable schema lands now
	// (cheap) even though Slice A only exercises the lexical columns — the vector
	// (`embedding`/`embed_model`/`embed_dim`) and reflection (`proof_count`/
	// `promotion_state`/`reflected_at`) columns are present so deferred slices B/C
	// need no table rebuild. `type`/`scope` are validated in the board.rs chokepoint,
	// not a SQL CHECK on `type` (open vocabulary); `scope` keeps a CHECK (closed set).
	// `evicted_at` is a SOFT delete — filtered from recall, provenance preserved.
	// `memory_fts` is an external-content FTS5 index over (title,body,entities) kept
	// in sync by three explicit triggers using the 'delete' command form (the only
	// correct way to update an external-content index on row change/removal).
	"CREATE TABLE IF NOT EXISTS memory (
	    id              TEXT PRIMARY KEY,
	    ts              TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
	    type            TEXT NOT NULL,
	    title           TEXT NOT NULL,
	    body            TEXT,
	    salience        REAL NOT NULL DEFAULT 0.5 CHECK (salience >= 0 AND salience <= 1),
	    scope           TEXT NOT NULL CHECK (scope IN ('ticket','project','global')),
	    entities        TEXT,
	    files           TEXT,
	    project         TEXT,
	    ticket_id       TEXT,
	    embedding       BLOB,
	    embed_model     TEXT,
	    embed_dim       INTEGER,
	    usage_count     INTEGER NOT NULL DEFAULT 0,
	    last_used_ts    TEXT,
	    proof_count     INTEGER NOT NULL DEFAULT 0,
	    promotion_state TEXT NOT NULL DEFAULT 'none'
	                    CHECK (promotion_state IN ('none','candidate','promoted','rejected')),
	    reflected_at    TEXT,
	    evicted_at      TEXT,
	    created_at      TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
	);

	CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5 (
	    title, body, entities,
	    content='memory',
	    content_rowid='rowid'
	);

	CREATE TRIGGER IF NOT EXISTS memory_ai AFTER INSERT ON memory BEGIN
	    INSERT INTO memory_fts(rowid, title, body, entities)
	    VALUES (new.rowid, new.title, new.body, new.entities);
	END;

	CREATE TRIGGER IF NOT EXISTS memory_ad AFTER DELETE ON memory BEGIN
	    INSERT INTO memory_fts(memory_fts, rowid, title, body, entities)
	    VALUES ('delete', old.rowid, old.title, old.body, old.entities);
	END;

	CREATE TRIGGER IF NOT EXISTS memory_au AFTER UPDATE ON memory BEGIN
	    INSERT INTO memory_fts(memory_fts, rowid, title, body, entities)
	    VALUES ('delete', old.rowid, old.title, old.body, old.entities);
	    INSERT INTO memory_fts(rowid, title, body, entities)
	    VALUES (new.rowid, new.title, new.body, new.entities);
	END;",
];

/// Create the base schema, tune connection pragmas, and apply pending migrations.
/// Idempotent — safe on every open.
pub fn init(conn: &Connection) -> Result<()> {
	// WAL is persistent (a no-op once set) and lets parallel Recorders write
	// without dropping rows; busy_timeout is per-connection (research/17 §8 M3).
	// `:memory:` silently stays in "memory" journal mode — harmless.
	conn.execute_batch("PRAGMA journal_mode=WAL;\nPRAGMA busy_timeout=5000;")
		.context("setting connection pragmas")?;
	conn.execute_batch(SCHEMA).context("creating board schema")?;
	migrate(conn)
}

/// Apply migrations from the current `user_version` forward, one step at a time.
fn migrate(conn: &Connection) -> Result<()> {
	let mut version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
	while (version as usize) < MIGRATIONS.len() {
		conn.execute_batch(MIGRATIONS[version as usize])
			.with_context(|| format!("applying migration v{}", version + 1))?;
		version += 1;
		conn.pragma_update(None, "user_version", version).context("bumping user_version")?;
	}
	Ok(())
}
