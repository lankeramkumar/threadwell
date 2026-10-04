//! Pages form a tree per workspace. Each page stores a Tiptap document as JSON.
//!
//! Content writes are optimistic: the caller sends the revision it last saw, and a
//! mismatch is rejected as a conflict instead of overwriting newer content. Trashing
//! is a soft delete. A trashed page and its descendants are hidden from the tree
//! and from search until restored.

use std::collections::HashSet;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::Value;

use crate::error::{validation, AppError, AppResult};
use crate::markdown;
use crate::search;
use crate::util;

const MAX_TITLE_CHARS: usize = 200;
const MAX_BODY_BYTES: usize = 2_000_000;
const MAX_DEPTH: usize = 64;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PageSummary {
    pub id: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub revision: i64,
    pub is_favorite: bool,
    pub deleted_at: Option<String>,
    pub updated_at: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub id: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub body: Value,
    pub revision: i64,
    pub is_favorite: bool,
    pub created_at: String,
    pub updated_at: String,
}

pub fn validate_body(body: &Value) -> AppResult<String> {
    if body.get("type").and_then(Value::as_str) != Some("doc") {
        return validation("Page content must be a document");
    }
    let json = serde_json::to_string(body)?;
    if json.len() > MAX_BODY_BYTES {
        return validation("This page is too large to save. Split it into smaller pages.");
    }
    Ok(json)
}

pub fn list(conn: &Connection, ws: &str) -> AppResult<Vec<PageSummary>> {
    let mut stmt = conn.prepare(
        "SELECT id, parent_id, title, revision, is_favorite, deleted_at, updated_at
         FROM pages WHERE workspace_id = ?1 AND deleted_at IS NULL
         ORDER BY position, title COLLATE NOCASE",
    )?;
    let rows = stmt.query_map(params![ws], summary_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn list_trash(conn: &Connection, ws: &str) -> AppResult<Vec<PageSummary>> {
    let mut stmt = conn.prepare(
        "SELECT id, parent_id, title, revision, is_favorite, deleted_at, updated_at
         FROM pages WHERE workspace_id = ?1 AND deleted_at IS NOT NULL
         ORDER BY deleted_at DESC",
    )?;
    let rows = stmt.query_map(params![ws], summary_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn summary_row(row: &rusqlite::Row) -> rusqlite::Result<PageSummary> {
    Ok(PageSummary {
        id: row.get(0)?,
        parent_id: row.get(1)?,
        title: row.get(2)?,
        revision: row.get(3)?,
        is_favorite: row.get::<_, i64>(4)? == 1,
        deleted_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

pub fn get(conn: &Connection, ws: &str, id: &str) -> AppResult<Page> {
    util::validate_id(id)?;
    let row = conn
        .query_row(
            "SELECT id, parent_id, title, body_json, revision, is_favorite, created_at, updated_at
             FROM pages WHERE id = ?1 AND workspace_id = ?2 AND deleted_at IS NULL",
            params![id, ws],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            },
        )
        .optional()?;
    let Some((id, parent_id, title, body_json, revision, fav, created_at, updated_at)) = row else {
        return Err(AppError::NotFound("Page".into()));
    };
    Ok(Page {
        id,
        parent_id,
        title,
        body: serde_json::from_str(&body_json)?,
        revision,
        is_favorite: fav == 1,
        created_at,
        updated_at,
    })
}

fn ensure_live(conn: &Connection, ws: &str, id: &str) -> AppResult<()> {
    util::validate_id(id)?;
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pages WHERE id = ?1 AND workspace_id = ?2 AND deleted_at IS NULL",
        params![id, ws],
        |row| row.get(0),
    )?;
    if exists == 0 {
        return Err(AppError::NotFound("Page".into()));
    }
    Ok(())
}

fn next_position(conn: &Connection, ws: &str, parent: Option<&str>) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT COALESCE(MAX(position), 0) + 1 FROM pages
         WHERE workspace_id = ?1 AND parent_id IS ?2 AND deleted_at IS NULL",
        params![ws, parent],
        |row| row.get(0),
    )?)
}

pub fn create(conn: &Connection, ws: &str, title: &str, parent: Option<&str>) -> AppResult<Page> {
    let title = util::validate_line(title, "Title", MAX_TITLE_CHARS)?;
    if let Some(parent) = parent {
        ensure_live(conn, ws, parent)?;
    }
    let id = util::new_id();
    let now = util::now();
    let body = markdown::empty_doc();
    let body_json = validate_body(&body)?;
    let position = next_position(conn, ws, parent)?;
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO pages (id, workspace_id, parent_id, title, body_json, revision, position, is_favorite, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, 0, ?7, ?7)",
        params![id, ws, parent, title, body_json, position, now],
    )?;
    search::index_page(&tx, &id, &title, &body)?;
    tx.commit()?;
    get(conn, ws, &id)
}

/// Saves title and body if `expected_revision` still matches the stored revision.
pub fn update(
    conn: &Connection,
    ws: &str,
    id: &str,
    title: &str,
    body: &Value,
    expected_revision: i64,
) -> AppResult<Page> {
    let title = util::validate_line(title, "Title", MAX_TITLE_CHARS)?;
    let body_json = validate_body(body)?;
    ensure_live(conn, ws, id)?;
    let tx = conn.unchecked_transaction()?;
    let current: i64 = tx.query_row("SELECT revision FROM pages WHERE id = ?1", params![id], |row| row.get(0))?;
    if current != expected_revision {
        return Err(AppError::Conflict(
            "This page changed in another window. Reload it before saving again.".into(),
        ));
    }
    tx.execute(
        "UPDATE pages SET title = ?1, body_json = ?2, revision = revision + 1, updated_at = ?3 WHERE id = ?4",
        params![title, body_json, util::now(), id],
    )?;
    refresh_links(&tx, ws, id, body)?;
    search::index_page(&tx, id, &title, body)?;
    tx.commit()?;
    get(conn, ws, id)
}

/// Replaces outgoing links for a page with the live internal targets in its body.
fn refresh_links(tx: &Connection, ws: &str, id: &str, body: &Value) -> AppResult<()> {
    tx.execute("DELETE FROM page_links WHERE from_page_id = ?1", params![id])?;
    for target in markdown::link_targets(body) {
        if target == id || util::validate_id(&target).is_err() {
            continue;
        }
        let live: i64 = tx.query_row(
            "SELECT COUNT(*) FROM pages WHERE id = ?1 AND workspace_id = ?2 AND deleted_at IS NULL",
            params![target, ws],
            |row| row.get(0),
        )?;
        if live == 1 {
            tx.execute(
                "INSERT OR IGNORE INTO page_links (from_page_id, to_page_id) VALUES (?1, ?2)",
                params![id, target],
            )?;
        }
    }
    Ok(())
}

/// Lists ids of pages whose outgoing link points at `id`. Used by the editor to show backlinks.
pub fn backlinks(conn: &Connection, ws: &str, id: &str) -> AppResult<Vec<PageSummary>> {
    ensure_live(conn, ws, id)?;
    let mut stmt = conn.prepare(
        "SELECT p.id, p.parent_id, p.title, p.revision, p.is_favorite, p.deleted_at, p.updated_at
         FROM page_links l JOIN pages p ON p.id = l.from_page_id
         WHERE l.to_page_id = ?1 AND p.workspace_id = ?2 AND p.deleted_at IS NULL
         ORDER BY p.title COLLATE NOCASE",
    )?;
    let rows = stmt.query_map(params![id, ws], summary_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn move_to(conn: &Connection, ws: &str, id: &str, parent: Option<&str>) -> AppResult<()> {
    ensure_live(conn, ws, id)?;
    if let Some(parent) = parent {
        ensure_live(conn, ws, parent)?;
        check_no_cycle(conn, id, parent)?;
    }
    let position = next_position(conn, ws, parent)?;
    conn.execute(
        "UPDATE pages SET parent_id = ?1, position = ?2, updated_at = ?3 WHERE id = ?4",
        params![parent, position, util::now(), id],
    )?;
    Ok(())
}

fn check_no_cycle(conn: &Connection, id: &str, new_parent: &str) -> AppResult<()> {
    let mut current = Some(new_parent.to_string());
    for _ in 0..MAX_DEPTH {
        let Some(node) = current else { return Ok(()) };
        if node == id {
            return validation("A page cannot be moved under itself or one of its children");
        }
        current = conn
            .query_row("SELECT parent_id FROM pages WHERE id = ?1", params![node], |row| row.get(0))
            .optional()?
            .flatten();
    }
    validation("The page hierarchy is too deep to move this page")
}

pub fn set_favorite(conn: &Connection, ws: &str, id: &str, favorite: bool) -> AppResult<()> {
    ensure_live(conn, ws, id)?;
    conn.execute(
        "UPDATE pages SET is_favorite = ?1, updated_at = ?2 WHERE id = ?3",
        params![i64::from(favorite), util::now(), id],
    )?;
    Ok(())
}

/// Soft-deletes a page and its live descendants, removing them from the search index.
pub fn trash(conn: &Connection, ws: &str, id: &str) -> AppResult<usize> {
    ensure_live(conn, ws, id)?;
    let tx = conn.unchecked_transaction()?;
    let ids = descendant_ids(&tx, id)?;
    let now = util::now();
    for page_id in &ids {
        tx.execute(
            "UPDATE pages SET deleted_at = ?1 WHERE id = ?2",
            params![now, page_id],
        )?;
        search::remove_page(&tx, page_id)?;
    }
    tx.commit()?;
    Ok(ids.len())
}

fn descendant_ids(conn: &Connection, root: &str) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "WITH RECURSIVE tree(id) AS (
             SELECT id FROM pages WHERE id = ?1 AND deleted_at IS NULL
             UNION ALL
             SELECT p.id FROM pages p JOIN tree t ON p.parent_id = t.id WHERE p.deleted_at IS NULL
         )
         SELECT id FROM tree",
    )?;
    let ids: HashSet<String> = stmt
        .query_map(params![root], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(ids.into_iter().collect())
}

/// Restores a trashed page. If its parent is still trashed, it becomes a top-level page.
pub fn restore(conn: &Connection, ws: &str, id: &str) -> AppResult<()> {
    util::validate_id(id)?;
    let tx = conn.unchecked_transaction()?;
    let row: Option<(Option<String>, String, String)> = tx
        .query_row(
            "SELECT parent_id, title, body_json FROM pages
             WHERE id = ?1 AND workspace_id = ?2 AND deleted_at IS NOT NULL",
            params![id, ws],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let Some((parent, title, body_json)) = row else {
        return Err(AppError::NotFound("Trashed page".into()));
    };
    let parent_live = match &parent {
        Some(p) => tx.query_row(
            "SELECT COUNT(*) FROM pages WHERE id = ?1 AND deleted_at IS NULL",
            params![p],
            |row| row.get::<_, i64>(0),
        )? == 1,
        None => true,
    };
    let new_parent = if parent_live { parent } else { None };
    tx.execute(
        "UPDATE pages SET deleted_at = NULL, parent_id = ?1, updated_at = ?2 WHERE id = ?3",
        params![new_parent, util::now(), id],
    )?;
    let body: Value = serde_json::from_str(&body_json)?;
    search::index_page(&tx, id, &title, &body)?;
    tx.commit()?;
    Ok(())
}

pub fn count_live(conn: &Connection, ws: &str) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM pages WHERE workspace_id = ?1 AND deleted_at IS NULL",
        params![ws],
        |row| row.get(0),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn setup() -> (tempfile::TempDir, Connection, String) {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = db::open(&dir.path().join("t.db")).unwrap();
        db::migrate(&mut conn).unwrap();
        let ws = util::new_id();
        conn.execute(
            "INSERT INTO workspace_meta (id, name, created_at) VALUES (?1, 'W', ?2)",
            params![ws, util::now()],
        )
        .unwrap();
        (dir, conn, ws)
    }

    fn doc_with_text(text: &str) -> Value {
        markdown::from_markdown(text)
    }

    #[test]
    fn create_update_and_reload_round_trip() {
        let (_d, conn, ws) = setup();
        let page = create(&conn, &ws, "Plan", None).unwrap();
        assert_eq!(page.revision, 1);
        let saved = update(&conn, &ws, &page.id, "Plan v2", &doc_with_text("Hello world"), 1).unwrap();
        assert_eq!(saved.revision, 2);
        let reloaded = get(&conn, &ws, &page.id).unwrap();
        assert_eq!(reloaded.title, "Plan v2");
        assert_eq!(markdown::plain_text(&reloaded.body), "Hello world");
    }

    #[test]
    fn stale_revision_is_rejected_without_overwriting() {
        let (_d, conn, ws) = setup();
        let page = create(&conn, &ws, "Doc", None).unwrap();
        update(&conn, &ws, &page.id, "Doc", &doc_with_text("first"), 1).unwrap();
        let stale = update(&conn, &ws, &page.id, "Doc", &doc_with_text("stale"), 1);
        assert!(matches!(stale, Err(AppError::Conflict(_))));
        assert_eq!(markdown::plain_text(&get(&conn, &ws, &page.id).unwrap().body), "first");
    }

    #[test]
    fn rejects_invalid_titles_and_documents() {
        let (_d, conn, ws) = setup();
        assert!(create(&conn, &ws, "   ", None).is_err());
        assert!(create(&conn, &ws, &"x".repeat(201), None).is_err());
        let page = create(&conn, &ws, "ok", None).unwrap();
        assert!(update(&conn, &ws, &page.id, "ok", &serde_json::json!({"type": "script"}), 1).is_err());
    }

    #[test]
    fn cannot_move_page_under_its_descendant() {
        let (_d, conn, ws) = setup();
        let a = create(&conn, &ws, "A", None).unwrap();
        let b = create(&conn, &ws, "B", Some(&a.id)).unwrap();
        let c = create(&conn, &ws, "C", Some(&b.id)).unwrap();
        assert!(move_to(&conn, &ws, &a.id, Some(&c.id)).is_err());
        assert!(move_to(&conn, &ws, &a.id, Some(&a.id)).is_err());
        move_to(&conn, &ws, &c.id, None).unwrap();
    }

    #[test]
    fn trash_hides_subtree_from_tree_and_search_until_restored() {
        let (_d, conn, ws) = setup();
        let parent = create(&conn, &ws, "Parent", None).unwrap();
        let child = create(&conn, &ws, "Child", Some(&parent.id)).unwrap();
        update(&conn, &ws, &child.id, "Child", &doc_with_text("unique zebra"), 1).unwrap();
        assert_eq!(search::search(&conn, &ws, "zebra").unwrap().len(), 1);

        assert_eq!(trash(&conn, &ws, &parent.id).unwrap(), 2);
        assert!(list(&conn, &ws).unwrap().is_empty());
        assert!(search::search(&conn, &ws, "zebra").unwrap().is_empty());
        assert_eq!(list_trash(&conn, &ws).unwrap().len(), 2);

        restore(&conn, &ws, &parent.id).unwrap();
        assert_eq!(list(&conn, &ws).unwrap().len(), 1);
    }

    #[test]
    fn restoring_under_trashed_parent_promotes_to_top_level() {
        let (_d, conn, ws) = setup();
        let parent = create(&conn, &ws, "P", None).unwrap();
        let child = create(&conn, &ws, "K", Some(&parent.id)).unwrap();
        trash(&conn, &ws, &parent.id).unwrap();
        restore(&conn, &ws, &child.id).unwrap();
        assert_eq!(get(&conn, &ws, &child.id).unwrap().parent_id, None);
    }

    #[test]
    fn internal_links_create_backlinks_and_ignore_missing_targets() {
        let (_d, conn, ws) = setup();
        let target = create(&conn, &ws, "Target", None).unwrap();
        let source = create(&conn, &ws, "Source", None).unwrap();
        let body = serde_json::json!({
            "type": "doc",
            "content": [{ "type": "paragraph", "content": [
                { "type": "text", "text": "see", "marks": [{ "type": "link", "attrs": { "href": format!("threadwell://page/{}", target.id) } }] },
                { "type": "text", "text": "ghost", "marks": [{ "type": "link", "attrs": { "href": "threadwell://page/00000000-0000-4000-8000-000000000000" } }] }
            ]}]
        });
        update(&conn, &ws, &source.id, "Source", &body, 1).unwrap();
        let back = backlinks(&conn, &ws, &target.id).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].id, source.id);
        let links: i64 = conn.query_row("SELECT COUNT(*) FROM page_links", [], |r| r.get(0)).unwrap();
        assert_eq!(links, 1);
    }

    #[test]
    fn pages_are_scoped_to_their_workspace() {
        let (_d, conn, ws) = setup();
        let other = util::new_id();
        conn.execute(
            "INSERT INTO workspace_meta (id, name, created_at) VALUES (?1, 'Other', ?2)",
            params![other, util::now()],
        )
        .unwrap();
        let foreign = create(&conn, &other, "Secret", None).unwrap();
        assert!(matches!(get(&conn, &ws, &foreign.id), Err(AppError::NotFound(_))));
        assert!(list(&conn, &ws).unwrap().is_empty());
        assert!(matches!(trash(&conn, &ws, &foreign.id), Err(AppError::NotFound(_))));
    }

    #[test]
    fn favorites_persist() {
        let (_d, conn, ws) = setup();
        let page = create(&conn, &ws, "Fav", None).unwrap();
        set_favorite(&conn, &ws, &page.id, true).unwrap();
        assert!(get(&conn, &ws, &page.id).unwrap().is_favorite);
    }
}
