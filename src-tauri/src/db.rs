use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;

use crate::error::{validation, AppResult};

pub const DB_FILE: &str = "threadwell.db";
pub const ATTACHMENTS_DIR: &str = "attachments";

/// Ordered, append-only migrations. Index + 1 is the schema version.
const MIGRATIONS: &[&str] = &[include_str!("../migrations/0001_init.sql")];

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
