use std::ops::Deref;
use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;

use crate::error::{validation, AppResult};

pub const DB_FILE: &str = "threadwell.db";
pub const ATTACHMENTS_DIR: &str = "attachments";

/// Ordered, append-only migrations. Index + 1 is the schema version.
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_init.sql"),
    include_str!("../migrations/0002_ai.sql"),
    include_str!("../migrations/0003_knowledge_meetings_recipes.sql"),
    include_str!("../migrations/0004_local_imports.sql"),
];

/// Migrations that rebuild tables other tables reference. They run with foreign-key
/// enforcement off, and the result is checked before commit.
const FK_OFF_MIGRATIONS: &[usize] = &[2];

pub fn schema_version_latest() -> i64 {
    MIGRATIONS.len() as i64
}

pub fn open(path: &Path) -> AppResult<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.busy_timeout(Duration::from_secs(5))?;
    Ok(conn)
}

/// Applies pending migrations, each in its own transaction.
pub fn migrate(conn: &mut Connection) -> AppResult<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if current > schema_version_latest() {
        return validation("This workspace was created by a newer version of Threadwell");
    }
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let version = index as i64 + 1;
        let fk_off = FK_OFF_MIGRATIONS.contains(&index);
        if fk_off {
            conn.pragma_update(None, "foreign_keys", "OFF")?;
        }
        let result = (|| -> AppResult<()> {
            let tx = conn.transaction()?;
            tx.execute_batch(sql)?;
            if fk_off {
                let violations: i64 = tx.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0))?;
                if violations > 0 {
                    return validation("A migration left broken references and was not applied");
                }
            }
            tx.execute_batch(&format!("PRAGMA user_version = {version}"))?;
            tx.commit()?;
            Ok(())
        })();
        if fk_off {
            conn.pragma_update(None, "foreign_keys", "ON")?;
        }
        result?;
    }
    Ok(())
}

pub fn quick_check(conn: &Connection) -> AppResult<()> {
    let result: String = conn.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if result != "ok" {
        return validation("The workspace database failed an integrity check");
    }
    Ok(())
}

/// A transaction that nests safely. At top level it is `BEGIN IMMEDIATE`; inside another
/// `Tx` it becomes a savepoint. Dropping without `commit` rolls back.
pub struct Tx<'a> {
    conn: &'a Connection,
    savepoint: bool,
    finished: bool,
}

impl<'a> Tx<'a> {
    pub fn begin(conn: &'a Connection) -> rusqlite::Result<Self> {
        let savepoint = !conn.is_autocommit();
        conn.execute_batch(if savepoint { "SAVEPOINT threadwell_sp" } else { "BEGIN IMMEDIATE" })?;
        Ok(Self { conn, savepoint, finished: false })
    }

    pub fn commit(mut self) -> rusqlite::Result<()> {
        self.conn.execute_batch(if self.savepoint {
            "RELEASE threadwell_sp"
        } else {
            "COMMIT"
        })?;
        self.finished = true;
        Ok(())
    }
}

impl Drop for Tx<'_> {
    fn drop(&mut self) {
        if !self.finished {
            let sql = if self.savepoint {
                "ROLLBACK TO threadwell_sp; RELEASE threadwell_sp"
            } else {
                "ROLLBACK"
            };
            let _ = self.conn.execute_batch(sql);
        }
    }
}

impl Deref for Tx<'_> {
    type Target = Connection;
    fn deref(&self) -> &Connection {
        self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrating_from_v2_keeps_runs_and_their_children() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v2.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(MIGRATIONS[0]).unwrap();
            conn.execute_batch(MIGRATIONS[1]).unwrap();
            conn.execute_batch("PRAGMA user_version = 2").unwrap();
            conn.execute_batch(
                "INSERT INTO workspace_meta (id, name, created_at) VALUES ('w', 'W', 'now');
                 INSERT INTO ai_runs (id, workspace_id, kind, status, provider, model, started_at)
                   VALUES ('r', 'w', 'chat', 'completed', 'ollama', 'm', 'now');
                 INSERT INTO ai_tool_events (id, run_id, step, tool, args_json, ok, summary, created_at)
                   VALUES ('e', 'r', 1, 'search_workspace', '{}', 1, 'ok', 'now');",
            )
            .unwrap();
        }
        let mut conn = Connection::open(&path).unwrap();
        migrate(&mut conn).unwrap();
        let runs: i64 = conn.query_row("SELECT COUNT(*) FROM ai_runs", [], |r| r.get(0)).unwrap();
        let events: i64 = conn.query_row("SELECT COUNT(*) FROM ai_tool_events", [], |r| r.get(0)).unwrap();
        assert_eq!((runs, events), (1, 1));
        conn.execute("INSERT INTO ai_runs (id, workspace_id, kind, status, provider, model, started_at) VALUES ('m', 'w', 'meeting', 'running', 'ollama', 'm', 'now')", []).unwrap();
        let violations: i64 = conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0)).unwrap();
        assert_eq!(violations, 0);
    }

    #[test]
    fn nested_transactions_roll_back_only_the_inner_part() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = open(&dir.path().join("tx.db")).unwrap();
        migrate(&mut conn).unwrap();
        let outer = Tx::begin(&conn).unwrap();
        outer.execute("INSERT INTO settings (key, value) VALUES ('theme', 'dark')", []).unwrap();
        {
            let inner = Tx::begin(&outer).unwrap();
            inner.execute("INSERT INTO settings (key, value) VALUES ('ai.model', 'x')", []).unwrap();
            // inner dropped without commit: rolled back to its savepoint
        }
        outer.commit().unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM settings", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
    }
}
