//! Searching past assistant conversations, and deleting run history. Deleting history removes
//! the run's trace and its link to messages, but never deletes proposals or pages. Those are
//! workspace content and stay until the user removes them.

use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::State;

use crate::commands::{with_active, AppState};
use crate::error::AppResult;
use crate::util;

const MAX_RESULTS: i64 = 50;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConversationHit {
    pub conversation_id: String,
    pub title: String,
    pub snippet: String,
    pub updated_at: String,
}

fn like_pattern(input: &str) -> String {
    let escaped = input.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    format!("%{escaped}%")
}

/// Finds conversations whose title or messages contain the text. Matching is a plain substring
/// search, so it works the same for every language and for punctuation.
pub fn search_conversations(conn: &Connection, ws: &str, query: &str) -> AppResult<Vec<ConversationHit>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let pattern = like_pattern(query);
    let mut stmt = conn.prepare(
        "SELECT c.id, c.title, c.updated_at,
                (SELECT m.content FROM ai_messages m
                  WHERE m.conversation_id = c.id AND m.content LIKE ?2 ESCAPE '\\'
                  ORDER BY m.created_at LIMIT 1)
         FROM ai_conversations c
         WHERE c.workspace_id = ?1
           AND (c.title LIKE ?2 ESCAPE '\\'
                OR EXISTS (SELECT 1 FROM ai_messages m WHERE m.conversation_id = c.id AND m.content LIKE ?2 ESCAPE '\\'))
         ORDER BY c.updated_at DESC
         LIMIT ?3",
    )?;
    let rows = stmt.query_map(params![ws, pattern, MAX_RESULTS], |row| {
        let content: Option<String> = row.get(3)?;
        Ok(ConversationHit {
            conversation_id: row.get(0)?,
            title: row.get(1)?,
            snippet: snippet_around(&content.unwrap_or_default(), query),
            updated_at: row.get(2)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// About 120 characters around the first match, so the result list shows why it matched.
fn snippet_around(text: &str, query: &str) -> String {
    let lower = text.to_lowercase();
    let needle = query.to_lowercase();
    let Some(byte_index) = lower.find(&needle) else {
        return text.chars().take(120).collect();
    };
    let chars: Vec<char> = text.chars().collect();
    let char_index = text[..byte_index.min(text.len())].chars().count();
    let start = char_index.saturating_sub(50);
    let end = (char_index + query.chars().count() + 70).min(chars.len());
    let mut out: String = chars[start..end].iter().collect();
    if start > 0 {
        out.insert(0, '…');
    }
    if end < chars.len() {
        out.push('…');
    }
    out
}

/// Removes a run's trace and detaches it from its messages and proposals. Proposals and pages
/// are kept, because they are content the user may still want.
pub fn delete_run(conn: &Connection, ws: &str, run_id: &str) -> AppResult<()> {
    util::validate_id(run_id)?;
    let owned: i64 = conn.query_row(
        "SELECT COUNT(*) FROM ai_runs WHERE id = ?1 AND workspace_id = ?2",
        params![run_id, ws],
        |r| r.get(0),
    )?;
    if owned == 0 {
        return Err(crate::error::AppError::NotFound("Run".into()));
    }
    let tx = crate::db::Tx::begin(conn)?;
    tx.execute("UPDATE ai_messages SET run_id = NULL WHERE run_id = ?1", params![run_id])?;
    tx.execute("UPDATE change_proposals SET run_id = NULL WHERE run_id = ?1", params![run_id])?;
    tx.execute("DELETE FROM ai_tool_events WHERE run_id = ?1", params![run_id])?;
    tx.execute("DELETE FROM ai_runs WHERE id = ?1", params![run_id])?;
    tx.commit()?;
    Ok(())
}

#[tauri::command]
pub async fn ai_search_conversations(state: State<'_, AppState>, query: String) -> AppResult<Vec<ConversationHit>> {
    with_active(&state.active, |a| search_conversations(&a.conn, &a.info.id, &query))
}

#[tauri::command]
pub async fn ai_delete_run(state: State<'_, AppState>, run_id: String) -> AppResult<()> {
    with_active(&state.active, |a| delete_run(&a.conn, &a.info.id, &run_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, pages};

    fn setup() -> (tempfile::TempDir, Connection, String) {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = db::open(&dir.path().join("h.db")).unwrap();
        db::migrate(&mut conn).unwrap();
        let ws = util::new_id();
        conn.execute("INSERT INTO workspace_meta (id, name, created_at) VALUES (?1, 'H', ?2)", params![ws, util::now()]).unwrap();
        (dir, conn, ws)
    }

    fn conversation(conn: &Connection, ws: &str, title: &str, message: &str) -> String {
        let id = util::new_id();
        conn.execute(
            "INSERT INTO ai_conversations (id, workspace_id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
            params![id, ws, title, util::now()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ai_messages (id, workspace_id, conversation_id, role, content, created_at) VALUES (?1, ?2, ?3, 'user', ?4, ?5)",
            params![util::new_id(), ws, id, message, util::now()],
        )
        .unwrap();
        id
    }

    #[test]
    fn finds_conversations_by_title_or_message_text() {
        let (_d, conn, ws) = setup();
        let a = conversation(&conn, &ws, "Pricing", "What is the croissant price?");
        conversation(&conn, &ws, "Staffing", "Who covers Saturday?");
        let hits = search_conversations(&conn, &ws, "croissant").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].conversation_id, a);
        assert!(hits[0].snippet.contains("croissant"));
        assert_eq!(search_conversations(&conn, &ws, "Staffing").unwrap().len(), 1);
    }

    #[test]
    fn wildcards_in_the_query_are_literal() {
        let (_d, conn, ws) = setup();
        conversation(&conn, &ws, "Discounts", "We offer 50% off");
        conversation(&conn, &ws, "Other", "nothing here");
        assert_eq!(search_conversations(&conn, &ws, "50%").unwrap().len(), 1);
        assert!(search_conversations(&conn, &ws, "%").unwrap().len() == 1, "a lone % matches only the literal percent sign");
        assert!(search_conversations(&conn, &ws, "_").unwrap().is_empty());
    }

    #[test]
    fn conversations_from_another_workspace_are_not_returned() {
        let (_d, conn, ws) = setup();
        let other = util::new_id();
        conn.execute("INSERT INTO workspace_meta (id, name, created_at) VALUES (?1, 'O', ?2)", params![other, util::now()]).unwrap();
        conversation(&conn, &other, "Secret", "croissant secret");
        assert!(search_conversations(&conn, &ws, "croissant").unwrap().is_empty());
    }

    #[test]
    fn deleting_a_run_keeps_its_proposals_and_pages() {
        let (_d, conn, ws) = setup();
        let page = pages::create(&conn, &ws, "Keep me", None).unwrap();
        let run = util::new_id();
        conn.execute(
            "INSERT INTO ai_runs (id, workspace_id, kind, status, provider, model, started_at) VALUES (?1, ?2, 'chat', 'completed', 'ollama', 'm', ?3)",
            params![run, ws, util::now()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ai_tool_events (id, run_id, step, tool, args_json, ok, summary, created_at) VALUES (?1, ?2, 1, 'list_tasks', '{}', 1, 'ok', ?3)",
            params![util::new_id(), run, util::now()],
        )
        .unwrap();
        let proposal = crate::ai::proposals::create(&conn, &ws, Some(&run), "create_page", None, None, &serde_json::json!({"title": "x", "markdown": "y"}), "s", "+ y").unwrap();

        delete_run(&conn, &ws, &run).unwrap();

        let events: i64 = conn.query_row("SELECT COUNT(*) FROM ai_tool_events", [], |r| r.get(0)).unwrap();
        let runs: i64 = conn.query_row("SELECT COUNT(*) FROM ai_runs", [], |r| r.get(0)).unwrap();
        assert_eq!((events, runs), (0, 0));
        assert_eq!(crate::ai::proposals::get(&conn, &ws, &proposal.id).unwrap().status, "pending");
        assert!(pages::get(&conn, &ws, &page.id).is_ok());
    }
}
