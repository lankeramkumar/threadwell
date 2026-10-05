//! Knowledge index for the assistant: page chunks, embeddings, and hybrid retrieval.
//!
//! Chunks are rebuilt when a page is saved. A chunk whose text did not change keeps its row
//! and its embedding, so editing one paragraph re-embeds one chunk. Embeddings are keyed by
//! chunk and model, so changing the embedding model triggers re-embedding rather than
//! mixing vector spaces. Search is brute-force cosine over the workspace's chunks. That is
//! fine at the sizes this app targets, and the cost is measured in the README.
//!
//! Pages marked `ai_excluded` and trashed pages never appear in retrieval results.

use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::db::Tx;
use crate::error::AppResult;
use crate::markdown;
use crate::search;
use crate::util;

pub const CHUNK_MAX_CHARS: usize = 900;

#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub heading: String,
    pub text: String,
    pub hash: String,
}

pub fn content_hash(text: &str) -> String {
    hex::encode(Sha256::digest(text.as_bytes()))
}

/// Splits a page into chunks at headings, then at paragraph boundaries once a chunk reaches
/// `CHUNK_MAX_CHARS`. Blocks longer than the limit are split by characters.
pub fn chunk_body(body: &Value) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut heading = String::new();
    let mut buffer = String::new();
    let blocks = body.get("content").and_then(Value::as_array).cloned().unwrap_or_default();
    for block in &blocks {
        let text = markdown::plain_text(block);
        if text.is_empty() {
            continue;
        }
        if block.get("type").and_then(Value::as_str) == Some("heading") {
            flush(&mut chunks, &heading, &mut buffer);
            heading = text;
            continue;
        }
        for piece in split_long(&text) {
            if !buffer.is_empty() && buffer.chars().count() + piece.chars().count() + 1 > CHUNK_MAX_CHARS {
                flush(&mut chunks, &heading, &mut buffer);
            }
            if !buffer.is_empty() {
                buffer.push('\n');
            }
            buffer.push_str(&piece);
        }
    }
    flush(&mut chunks, &heading, &mut buffer);
    chunks
}

fn split_long(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= CHUNK_MAX_CHARS {
        return vec![text.to_string()];
    }
    chars.chunks(CHUNK_MAX_CHARS).map(|c| c.iter().collect()).collect()
}

fn flush(chunks: &mut Vec<Chunk>, heading: &str, buffer: &mut String) {
    let text = buffer.trim().to_string();
    buffer.clear();
    if !text.is_empty() {
        let hash = content_hash(&text);
        chunks.push(Chunk { heading: heading.to_string(), text, hash });
    }
}

/// The text that is embedded. The page title and heading give context that a short chunk
/// may lack.
pub fn embed_text(title: &str, heading: &str, text: &str) -> String {
    if heading.is_empty() {
        format!("{title}\n{text}")
    } else {
        format!("{title} — {heading}\n{text}")
    }
}

/// Brings a page's chunks in line with its body. Called from the page save transaction.
pub fn reindex_page(conn: &Connection, ws: &str, page_id: &str, body: &Value) -> AppResult<()> {
    let chunks = chunk_body(body);
    let tx = Tx::begin(conn)?;
    let existing: Vec<(String, String)> = {
        let mut stmt = tx.prepare("SELECT id, content_hash FROM page_chunks WHERE page_id = ?1")?;
        let rows = stmt.query_map(params![page_id], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<_, _>>()?
    };
    let mut reusable: HashMap<String, Vec<String>> = HashMap::new();
    for (id, hash) in &existing {
        reusable.entry(hash.clone()).or_default().push(id.clone());
    }
    let mut used: HashSet<String> = HashSet::new();
    for (ord, chunk) in chunks.iter().enumerate() {
        let reused = reusable.get_mut(&chunk.hash).and_then(|ids| ids.pop());
        match reused {
            Some(id) => {
                tx.execute(
                    "UPDATE page_chunks SET ord = ?1, heading = ?2 WHERE id = ?3",
                    params![ord as i64, chunk.heading, id],
                )?;
                used.insert(id);
            }
            None => {
                let id = util::new_id();
                tx.execute(
                    "INSERT INTO page_chunks (id, page_id, workspace_id, ord, heading, text, content_hash, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![id, page_id, ws, ord as i64, chunk.heading, chunk.text, chunk.hash, util::now()],
                )?;
                used.insert(id);
            }
        }
    }
    for (id, _) in &existing {
        if !used.contains(id) {
            tx.execute("DELETE FROM page_chunks WHERE id = ?1", params![id])?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// The section of a linked source file that best matches the question: the heading of the chunk
/// containing the most question terms. Returns None for notes, and when no section matches.
pub fn source_section(conn: &Connection, page_id: &str, question: &str) -> AppResult<Option<String>> {
    let is_source: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pages WHERE id = ?1 AND source_id IS NOT NULL",
        params![page_id],
        |row| row.get(0),
    )?;
    if is_source == 0 {
        return Ok(None);
    }
    let terms: Vec<String> = search::keyword_terms(question).into_iter().map(|t| t.to_lowercase()).collect();
    if terms.is_empty() {
        return Ok(None);
    }
    let mut stmt = conn.prepare("SELECT heading, text FROM page_chunks WHERE page_id = ?1 ORDER BY ord")?;
    let rows = stmt.query_map(params![page_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
    let mut best: Option<(usize, String)> = None;
    for row in rows {
        let (heading, text) = row?;
        let lowered = text.to_lowercase();
        let score: usize = terms.iter().map(|t| lowered.matches(t.as_str()).count()).sum();
        if score > 0 && !heading.trim().is_empty() && best.as_ref().map_or(true, |(s, _)| score > *s) {
            best = Some((score, heading));
        }
    }
    Ok(best.map(|(_, heading)| heading))
}

pub fn is_excluded(conn: &Connection, page_id: &str) -> AppResult<bool> {
    let excluded: i64 = conn.query_row(
        "SELECT ai_excluded FROM pages WHERE id = ?1",
        params![page_id],
        |row| row.get(0),
    )?;
    Ok(excluded == 1)
}

pub fn set_excluded(conn: &Connection, ws: &str, page_id: &str, excluded: bool) -> AppResult<()> {
    util::validate_id(page_id)?;
    let changed = conn.execute(
        "UPDATE pages SET ai_excluded = ?1, updated_at = ?2 WHERE id = ?3 AND workspace_id = ?4 AND deleted_at IS NULL",
        params![i64::from(excluded), util::now(), page_id, ws],
    )?;
    if changed == 0 {
        return Err(crate::error::AppError::NotFound("Page".into()));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Embeddings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PendingChunk {
    pub chunk_id: String,
    pub input: String,
    pub hash: String,
}

/// Chunks of eligible pages (live, not excluded) that have no embedding for `model`.
pub fn pending_chunks(conn: &Connection, ws: &str, model: &str, limit: usize) -> AppResult<Vec<PendingChunk>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, p.title, c.heading, c.text, c.content_hash
         FROM page_chunks c JOIN pages p ON p.id = c.page_id
         WHERE c.workspace_id = ?1 AND p.deleted_at IS NULL AND p.ai_excluded = 0
           AND NOT EXISTS (
               SELECT 1 FROM chunk_embeddings e
               WHERE e.chunk_id = c.id AND e.model = ?2 AND e.content_hash = c.content_hash)
         ORDER BY p.id, c.ord
         LIMIT ?3",
    )?;
    let rows = stmt.query_map(params![ws, model, limit as i64], |row| {
        let title: String = row.get(1)?;
        let heading: String = row.get(2)?;
        let text: String = row.get(3)?;
        Ok(PendingChunk {
            chunk_id: row.get(0)?,
            input: embed_text(&title, &heading, &text),
            hash: row.get(4)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn store_embedding(conn: &Connection, chunk_id: &str, model: &str, vector: &[f32], hash: &str) -> AppResult<()> {
    let bytes: Vec<u8> = vector.iter().flat_map(|f| f.to_le_bytes()).collect();
    conn.execute(
        "INSERT OR REPLACE INTO chunk_embeddings (chunk_id, model, dims, vector, content_hash)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![chunk_id, model, vector.len() as i64, bytes, hash],
    )?;
    Ok(())
}

/// Counts of eligible chunks and how many have an embedding for `model`.
pub fn index_counts(conn: &Connection, ws: &str, model: &str) -> AppResult<(i64, i64)> {
    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM page_chunks c JOIN pages p ON p.id = c.page_id
         WHERE c.workspace_id = ?1 AND p.deleted_at IS NULL AND p.ai_excluded = 0",
        params![ws],
        |row| row.get(0),
    )?;
    let embedded: i64 = conn.query_row(
        "SELECT COUNT(*) FROM page_chunks c JOIN pages p ON p.id = c.page_id
         JOIN chunk_embeddings e ON e.chunk_id = c.id AND e.model = ?2 AND e.content_hash = c.content_hash
         WHERE c.workspace_id = ?1 AND p.deleted_at IS NULL AND p.ai_excluded = 0",
        params![ws, model],
        |row| row.get(0),
    )?;
    Ok((embedded, total))
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

fn decode(bytes: &[u8]) -> Vec<f32> {
    bytes.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

struct VectorHit {
    page_id: String,
    snippet: String,
    score: f32,
}

fn vector_hits(conn: &Connection, ws: &str, model: &str, query: &[f32], limit: usize) -> AppResult<Vec<VectorHit>> {
    let mut stmt = conn.prepare(
        "SELECT e.vector, c.page_id, c.text
         FROM chunk_embeddings e
         JOIN page_chunks c ON c.id = e.chunk_id AND c.content_hash = e.content_hash
         JOIN pages p ON p.id = c.page_id
         WHERE e.model = ?1 AND c.workspace_id = ?2 AND e.dims = ?3
           AND p.deleted_at IS NULL AND p.ai_excluded = 0",
    )?;
    let rows = stmt.query_map(params![model, ws, query.len() as i64], |row| {
        Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
    })?;
    let mut hits: Vec<VectorHit> = Vec::new();
    for row in rows {
        let (bytes, page_id, text) = row?;
        let score = cosine(query, &decode(&bytes));
        hits.push(VectorHit { page_id, snippet: text.chars().take(160).collect(), score });
    }
    hits.sort_by(|a, b| b.score.total_cmp(&a.score));
    hits.truncate(limit);
    Ok(hits)
}

// ---------------------------------------------------------------------------
// Hybrid retrieval
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Lexical,
    Hybrid,
}

impl Mode {
    pub fn parse(value: &str) -> Option<Mode> {
        match value {
            "lexical" => Some(Mode::Lexical),
            "hybrid" => Some(Mode::Hybrid),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Weights {
    pub lexical: f32,
    pub vector: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Retrieved {
    pub page_id: String,
    pub title: String,
    pub snippet: String,
    pub score: f32,
    pub lexical: f32,
    pub vector: f32,
    /// For linked source files: the section (such as "Lines 41–80") that best matches the question.
    pub section: Option<String>,
}

/// Ranks pages for a question. Lexical scores come from FTS5 BM25 and vector scores from
/// cosine similarity, and each is normalized to 0–1 across the candidates before weighting.
/// In `Lexical` mode the vector stage is skipped entirely.
pub fn retrieve(
    conn: &Connection,
    ws: &str,
    question: &str,
    query_vector: Option<&[f32]>,
    model: &str,
    mode: Mode,
    weights: Weights,
    limit: usize,
) -> AppResult<Vec<Retrieved>> {
    let mut lexical: HashMap<String, (String, String, f32)> = HashMap::new();
    if let Some(fts) = search::fts_or_query(question) {
        let mut stmt = conn.prepare(
            "SELECT p.id, p.title, snippet(page_search, 2, '[', ']', '…', 12), -bm25(page_search)
             FROM page_search JOIN pages p ON p.id = page_search.page_id
             WHERE page_search MATCH ?1 AND p.workspace_id = ?2 AND p.deleted_at IS NULL AND p.ai_excluded = 0
             ORDER BY rank LIMIT 30",
        )?;
        let rows = stmt.query_map(params![fts, ws], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, f64>(3)?))
        })?;
        for row in rows {
            let (id, title, snippet, score) = row?;
            lexical.entry(id).or_insert((title, snippet, score as f32));
        }
    }
    let max_lex = lexical.values().map(|v| v.2).fold(0.0_f32, f32::max);

    let mut vectors: HashMap<String, (String, f32)> = HashMap::new();
    if let (Mode::Hybrid, Some(query)) = (mode, query_vector) {
        for hit in vector_hits(conn, ws, model, query, 200)? {
            let entry = vectors.entry(hit.page_id).or_insert((hit.snippet.clone(), 0.0));
            if hit.score > entry.1 {
                *entry = (hit.snippet, hit.score);
            }
        }
    }
    let max_vec = vectors.values().map(|v| v.1.max(0.0)).fold(0.0_f32, f32::max);

    let mut ids: HashSet<String> = lexical.keys().cloned().collect();
    ids.extend(vectors.keys().cloned());
    let mut results = Vec::new();
    for id in ids {
        let lex_norm = match lexical.get(&id) {
            Some(v) if max_lex > 0.0 => v.2 / max_lex,
            _ => 0.0,
        };
        let vec_norm = match vectors.get(&id) {
            Some(v) if max_vec > 0.0 => v.1.max(0.0) / max_vec,
            _ => 0.0,
        };
        let score = match mode {
            Mode::Lexical => lex_norm,
            Mode::Hybrid => weights.lexical * lex_norm + weights.vector * vec_norm,
        };
        if score <= 0.0 {
            continue;
        }
        let (title, snippet) = match lexical.get(&id) {
            Some(v) => (v.0.clone(), v.1.clone()),
            None => {
                let title: String = conn.query_row("SELECT title FROM pages WHERE id = ?1", params![id], |r| r.get(0))?;
                (title, vectors.get(&id).map(|v| v.0.clone()).unwrap_or_default())
            }
        };
        results.push(Retrieved { page_id: id, title, snippet, score, lexical: lex_norm, vector: vec_norm, section: None });
    }
    results.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.page_id.cmp(&b.page_id)));
    results.truncate(limit);
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn doc(blocks: Vec<Value>) -> Value {
        json!({ "type": "doc", "content": blocks })
    }

    fn para(text: &str) -> Value {
        json!({ "type": "paragraph", "content": [{ "type": "text", "text": text }] })
    }

    fn heading(text: &str) -> Value {
        json!({ "type": "heading", "attrs": { "level": 2 }, "content": [{ "type": "text", "text": text }] })
    }

    #[test]
    fn chunks_follow_headings() {
        let chunks = chunk_body(&doc(vec![heading("Auth"), para("passkeys"), heading("Billing"), para("monthly")]));
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].heading, "Auth");
        assert_eq!(chunks[1].text, "monthly");
    }

    #[test]
    fn long_sections_split_within_the_limit() {
        let long = "word ".repeat(600);
        let chunks = chunk_body(&doc(vec![para(&long), para(&long)]));
        assert!(chunks.len() >= 3);
        assert!(chunks.iter().all(|c| c.text.chars().count() <= CHUNK_MAX_CHARS));
    }

    #[test]
    fn identical_text_has_identical_hash_and_changes_hash_when_edited() {
        let a = chunk_body(&doc(vec![para("same")]));
        let b = chunk_body(&doc(vec![para("same")]));
        let c = chunk_body(&doc(vec![para("changed")]));
        assert_eq!(a[0].hash, b[0].hash);
        assert_ne!(a[0].hash, c[0].hash);
    }

    #[test]
    fn cosine_is_one_for_parallel_vectors_and_zero_for_empty() {
        assert!((cosine(&[1.0, 2.0], &[2.0, 4.0]) - 1.0).abs() < 1e-6);
        assert_eq!(cosine(&[0.0, 0.0], &[1.0, 1.0]), 0.0);
    }

    #[test]
    fn embed_text_carries_title_and_heading() {
        assert_eq!(embed_text("Plan", "Auth", "passkeys"), "Plan — Auth\npasskeys");
        assert_eq!(embed_text("Plan", "", "x"), "Plan\nx");
    }

    // ---- database-backed tests ----

    use crate::{db, pages, util};

    struct Db {
        _dir: tempfile::TempDir,
        conn: Connection,
        ws: String,
    }

    fn db_with_workspace() -> Db {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = db::open(&dir.path().join("k.db")).unwrap();
        db::migrate(&mut conn).unwrap();
        let ws = util::new_id();
        conn.execute(
            "INSERT INTO workspace_meta (id, name, created_at) VALUES (?1, 'K', ?2)",
            params![ws, util::now()],
        )
        .unwrap();
        Db { _dir: dir, conn, ws }
    }

    fn save(d: &Db, id: &str, title: &str, text: &str, revision: i64) -> i64 {
        let body = markdown::from_markdown(text);
        pages::update(&d.conn, &d.ws, id, title, &body, revision).unwrap().revision
    }

    fn embed_all(d: &Db, model: &str, vector_for: impl Fn(&str) -> Vec<f32>) {
        for pending in pending_chunks(&d.conn, &d.ws, model, 1000).unwrap() {
            store_embedding(&d.conn, &pending.chunk_id, model, &vector_for(&pending.input), &pending.hash).unwrap();
        }
    }

    #[test]
    fn editing_one_paragraph_reembeds_only_that_chunk() {
        let d = db_with_workspace();
        let page = pages::create(&d.conn, &d.ws, "Plan", None).unwrap();
        let rev = save(&d, &page.id, "Plan", "First paragraph about auth.\n\nSecond paragraph about billing.", page.revision);
        embed_all(&d, "m", |_| vec![1.0, 0.0]);
        assert!(pending_chunks(&d.conn, &d.ws, "m", 10).unwrap().is_empty());

        save(&d, &page.id, "Plan", "First paragraph about auth.\n\nSecond paragraph about invoices.", rev);
        let pending = pending_chunks(&d.conn, &d.ws, "m", 10).unwrap();
        assert_eq!(pending.len(), 1, "only the edited paragraph should need embedding");
        assert!(pending[0].input.contains("invoices"));
    }

    #[test]
    fn switching_embedding_model_requires_new_vectors() {
        let d = db_with_workspace();
        let page = pages::create(&d.conn, &d.ws, "Plan", None).unwrap();
        save(&d, &page.id, "Plan", "Some text.", page.revision);
        embed_all(&d, "old-model", |_| vec![1.0]);
        assert_eq!(pending_chunks(&d.conn, &d.ws, "new-model", 10).unwrap().len(), 1);
        assert_eq!(index_counts(&d.conn, &d.ws, "new-model").unwrap(), (0, 1));
    }

    #[test]
    fn excluded_and_trashed_pages_never_come_back_from_retrieval() {
        let d = db_with_workspace();
        let secret = pages::create(&d.conn, &d.ws, "Secret", None).unwrap();
        save(&d, &secret.id, "Secret", "passkeys decision details", secret.revision);
        let open = pages::create(&d.conn, &d.ws, "Open", None).unwrap();
        save(&d, &open.id, "Open", "passkeys are mentioned here too", open.revision);
        let weights = Weights { lexical: 1.0, vector: 0.0 };

        let before = retrieve(&d.conn, &d.ws, "passkeys", None, "m", Mode::Lexical, weights, 10).unwrap();
        assert_eq!(before.len(), 2);

        set_excluded(&d.conn, &d.ws, &secret.id, true).unwrap();
        let after = retrieve(&d.conn, &d.ws, "passkeys", None, "m", Mode::Lexical, weights, 10).unwrap();
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].page_id, open.id);

        pages::trash(&d.conn, &d.ws, &open.id).unwrap();
        assert!(retrieve(&d.conn, &d.ws, "passkeys", None, "m", Mode::Lexical, weights, 10).unwrap().is_empty());
    }

    #[test]
    fn hybrid_finds_a_paraphrase_that_keywords_miss() {
        let d = db_with_workspace();
        let fingerprint = pages::create(&d.conn, &d.ws, "Sign-in", None).unwrap();
        save(&d, &fingerprint.id, "Sign-in", "Users unlock the app with a fingerprint sensor.", fingerprint.revision);
        let keyword = pages::create(&d.conn, &d.ws, "Glossary", None).unwrap();
        save(&d, &keyword.id, "Glossary", "Biometric login is listed as a term here.", keyword.revision);

        // Synthetic vectors: the fingerprint page is semantically closest to the query.
        embed_all(&d, "m", |text| if text.contains("fingerprint") { vec![1.0, 0.1] } else { vec![0.0, 1.0] });
        let query = vec![1.0, 0.0];
        let weights = Weights { lexical: 0.4, vector: 0.6 };

        let lexical = retrieve(&d.conn, &d.ws, "biometric login", Some(query.as_slice()), "m", Mode::Lexical, weights, 5).unwrap();
        assert_eq!(lexical.len(), 1);
        assert_eq!(lexical[0].title, "Glossary");

        let hybrid = retrieve(&d.conn, &d.ws, "biometric login", Some(query.as_slice()), "m", Mode::Hybrid, weights, 5).unwrap();
        assert!(hybrid.iter().any(|h| h.title == "Sign-in"), "hybrid should surface the semantic match");
        assert_eq!(hybrid[0].title, "Sign-in");
    }

    #[test]
    fn embeddings_stay_within_the_workspace() {
        let d = db_with_workspace();
        let other = util::new_id();
        d.conn
            .execute(
                "INSERT INTO workspace_meta (id, name, created_at) VALUES (?1, 'Other', ?2)",
                params![other, util::now()],
            )
            .unwrap();
        let foreign = pages::create(&d.conn, &other, "Foreign", None).unwrap();
        save_in(&d.conn, &other, &foreign.id, "Foreign", "shared vocabulary here", foreign.revision);
        embed_all_in(&d.conn, &other, "m", vec![1.0, 0.0]);
        let hits = retrieve(&d.conn, &d.ws, "vocabulary", Some(&[1.0f32, 0.0][..]), "m", Mode::Hybrid, Weights { lexical: 0.5, vector: 0.5 }, 5).unwrap();
        assert!(hits.is_empty());
    }

    fn save_in(conn: &Connection, ws: &str, id: &str, title: &str, text: &str, revision: i64) {
        let body = markdown::from_markdown(text);
        pages::update(conn, ws, id, title, &body, revision).unwrap();
    }

    fn embed_all_in(conn: &Connection, ws: &str, model: &str, vector: Vec<f32>) {
        for pending in pending_chunks(conn, ws, model, 1000).unwrap() {
            store_embedding(conn, &pending.chunk_id, model, &vector, &pending.hash).unwrap();
        }
    }
}
