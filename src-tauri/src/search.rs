//! Lexical search over pages (FTS5) and tasks (substring match).
//!
//! `page_search` is derived data. Pages are authoritative, and `rebuild` regenerates
//! the index from them. Every query is scoped to one workspace id.

use rusqlite::{params, Connection, Transaction};
use serde::Serialize;
use serde_json::Value;

use crate::error::AppResult;
use crate::markdown;

pub const MAX_RESULTS: usize = 50;
const MAX_QUERY_CHARS: usize = 200;

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub kind: &'static str,
    pub id: String,
    pub title: String,
    pub snippet: String,
}

/// Replaces a page's index row. Callers pass the same title and body that were stored.
pub fn index_page(conn: &Transaction, page_id: &str, title: &str, body: &Value) -> AppResult<()> {
    conn.execute("DELETE FROM page_search WHERE page_id = ?1", params![page_id])?;
    conn.execute(
        "INSERT INTO page_search (page_id, title, body) VALUES (?1, ?2, ?3)",
        params![page_id, title, markdown::plain_text(body)],
    )?;
    Ok(())
}

pub fn remove_page(conn: &Connection, page_id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM page_search WHERE page_id = ?1", params![page_id])?;
    Ok(())
}

/// Drops and rebuilds the page index from live pages in one transaction.
pub fn rebuild(conn: &Connection, workspace_id: &str) -> AppResult<usize> {
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM page_search", [])?;
    let rows: Vec<(String, String, String)> = {
        let mut stmt = tx.prepare(
            "SELECT id, title, body_json FROM pages WHERE workspace_id = ?1 AND deleted_at IS NULL",
        )?;
        let mapped = stmt.query_map(params![workspace_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
        mapped.collect::<Result<_, _>>()?
    };
    for (id, title, body_json) in &rows {
        let body: Value = serde_json::from_str(body_json)?;
        index_page(&tx, id, title, &body)?;
    }
    tx.commit()?;
    Ok(rows.len())
}

/// Turns free text into a safe FTS5 query: each term is quoted, and the last term
/// matches as a prefix so results appear while typing. Terms are ANDed.
pub fn build_fts_query(input: &str) -> Option<String> {
    let terms: Vec<String> = input
        .chars()
        .take(MAX_QUERY_CHARS)
        .collect::<String>()
        .split_whitespace()
        .map(|t| t.replace('"', ""))
        .filter(|t| !t.is_empty())
        .collect();
    if terms.is_empty() {
        return None;
    }
    let last = terms.len() - 1;
    let quoted: Vec<String> = terms
        .iter()
        .enumerate()
        .map(|(i, t)| if i == last { format!("\"{t}\"*") } else { format!("\"{t}\"") })
        .collect();
    Some(quoted.join(" "))
}

fn like_pattern(input: &str) -> String {
    let escaped = input
        .chars()
        .take(MAX_QUERY_CHARS)
        .collect::<String>()
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

pub fn search(conn: &Connection, workspace_id: &str, input: &str) -> AppResult<Vec<SearchHit>> {
    let Some(fts) = build_fts_query(input) else {
        return Ok(Vec::new());
    };
    let mut hits = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT p.id, p.title, snippet(page_search, 2, '[', ']', '…', 12)
             FROM page_search
             JOIN pages p ON p.id = page_search.page_id
             WHERE page_search MATCH ?1 AND p.workspace_id = ?2 AND p.deleted_at IS NULL
             ORDER BY rank
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![fts, workspace_id, MAX_RESULTS as i64], |row| {
            Ok(SearchHit {
                kind: "page",
                id: row.get(0)?,
                title: row.get(1)?,
                snippet: row.get(2)?,
            })
        })?;
        hits.extend(rows.collect::<Result<Vec<_>, _>>()?);
    }
    let pattern = like_pattern(input);
    let mut stmt = conn.prepare(
        "SELECT id, title, description FROM tasks
         WHERE workspace_id = ?1 AND deleted_at IS NULL
           AND (title LIKE ?2 ESCAPE '\\' OR description LIKE ?2 ESCAPE '\\')
         ORDER BY updated_at DESC
         LIMIT ?3",
    )?;
    let rows = stmt.query_map(params![workspace_id, pattern, MAX_RESULTS as i64], |row| {
        let description: String = row.get(2)?;
        Ok(SearchHit {
            kind: "task",
            id: row.get(0)?,
            title: row.get(1)?,
            snippet: description.chars().take(120).collect(),
        })
    })?;
    hits.extend(rows.collect::<Result<Vec<_>, _>>()?);
    hits.truncate(MAX_RESULTS);
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fts_query_quotes_terms_and_prefixes_last() {
        assert_eq!(build_fts_query("auth \"magic\" link"), Some("\"auth\" \"magic\" \"link\"*".to_string()));
        assert_eq!(build_fts_query("   "), None);
    }

    #[test]
    fn fts_query_neutralizes_operators() {
        // Quotes are stripped so user input cannot break out of a term.
        assert_eq!(build_fts_query("a\" OR b"), Some("\"a\" \"OR\" \"b\"*".to_string()));
    }

    /// Manual measurement, not run by default. Reports p50/p95 for search over a generated
    /// 5,000-page workspace. Run with: cargo test --release -- --ignored --nocapture measures_search
    #[test]
    #[ignore = "manual measurement; see README Performance"]
    fn measures_search_latency_on_5000_pages() {
        use crate::{db, pages, util};
        use std::time::Instant;

        let dir = tempfile::tempdir().unwrap();
        let mut conn = db::open(&dir.path().join("bench.db")).unwrap();
        db::migrate(&mut conn).unwrap();
        let ws = util::new_id();
        conn.execute(
            "INSERT INTO workspace_meta (id, name, created_at) VALUES (?1, 'Bench', ?2)",
            params![ws, util::now()],
        )
        .unwrap();

        // Deterministic vocabulary so results are reproducible across runs.
        let mut seed: u64 = 42;
        let mut next = move || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) as usize
        };
        let vocab: Vec<String> = (0..3000).map(|i| format!("term{i}x{}", next() % 97)).collect();

        let build = Instant::now();
        for page_index in 0..5000 {
            let title = format!("Page {page_index} {}", vocab[next() % vocab.len()]);
            let page = pages::create(&conn, &ws, &title, None).unwrap();
            let words: Vec<&str> = (0..180).map(|_| vocab[next() % vocab.len()].as_str()).collect();
            let body = markdown::from_markdown(&format!("## Notes

{}", words.join(" ")));
            pages::update(&conn, &ws, &page.id, &title, &body, page.revision).unwrap();
        }
        let build_ms = build.elapsed().as_millis();

        let mut timings: Vec<u128> = Vec::new();
        for query_index in 0..200 {
            let query = format!("{} {}", vocab[query_index * 13 % vocab.len()], vocab[query_index * 7 % vocab.len()]);
            let start = Instant::now();
            let hits = search(&conn, &ws, &query).unwrap();
            timings.push(start.elapsed().as_micros());
            assert!(hits.len() <= MAX_RESULTS);
        }
        timings.sort_unstable();
        let p50 = timings[timings.len() / 2] as f64 / 1000.0;
        let p95 = timings[timings.len() * 95 / 100] as f64 / 1000.0;
        println!("bench: built 5000 pages in {build_ms} ms; search over 200 queries: p50 {p50:.2} ms, p95 {p95:.2} ms");
        assert!(p95 < 300.0, "search p95 {p95:.2} ms exceeds the 300 ms target");
    }

    #[test]
    fn like_pattern_escapes_wildcards() {
        assert_eq!(like_pattern("50%_off"), "%50\\%\\_off%");
    }
}
