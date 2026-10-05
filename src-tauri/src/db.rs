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
];

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
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.execute_batch(&format!("PRAGMA user_version = {version}"))?;
        tx.commit()?;
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
