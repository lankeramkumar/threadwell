//! Conversion between Tiptap-style document JSON and Markdown.
//!
//! Pages store a Tiptap JSON document. This module derives plain text for search,
//! extracts internal page links, and converts to and from a Markdown subset
//! (headings, paragraphs, lists, task lists, quotes, code, rules, tables,
//! bold/italic/code/links). Unsupported constructs degrade to plain paragraphs.

use serde_json::{json, Value};

pub const PAGE_LINK_PREFIX: &str = "threadwell://page/";
const MAX_NESTING: usize = 32;

pub fn empty_doc() -> Value {
    json!({ "type": "doc", "content": [{ "type": "paragraph" }] })
}

/// Concatenated text for search indexing, with block boundaries as newlines.
pub fn plain_text(node: &Value) -> String {
    let mut out = String::new();
    collect_text(node, &mut out);
    out.trim().to_string()
}

fn collect_text(node: &Value, out: &mut String) {
    if let Some(text) = node.get("text").and_then(Value::as_str) {
        out.push_str(text);
    }
    let is_block = node.get("type").and_then(Value::as_str).is_some_and(|t| t != "text");
    for child in children(node) {
        collect_text(child, out);
    }
    if is_block {
        out.push('\n');
    }
}

/// Page ids referenced through `threadwell://page/<id>` link marks, in order, unique.
pub fn link_targets(node: &Value) -> Vec<String> {
    let mut found = Vec::new();
    collect_links(node, &mut found);
    found
}

fn collect_links(node: &Value, found: &mut Vec<String>) {
    if let Some(marks) = node.get("marks").and_then(Value::as_array) {
        for mark in marks {
            let href = mark.pointer("/attrs/href").and_then(Value::as_str).unwrap_or("");
            if let Some(id) = href.strip_prefix(PAGE_LINK_PREFIX) {
                if !found.iter().any(|existing| existing == id) {
                    found.push(id.to_string());
                }
            }
        }
    }
    for child in children(node) {
        collect_links(child, found);
    }
}

// ---------------------------------------------------------------------------
// Document -> Markdown
// ---------------------------------------------------------------------------

pub fn to_markdown(doc: &Value) -> String {
    let blocks = children(doc)
        .iter()
        .map(render_block)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    let mut out = blocks.join("\n\n");
    out.push('\n');
    out
}

fn children(node: &Value) -> &[Value] {
    node.get("content").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

fn render_block(node: &Value) -> String {
    match node.get("type").and_then(Value::as_str).unwrap_or("") {
        "paragraph" => inline(children(node)),
        "heading" => {
            let level = node.pointer("/attrs/level").and_then(Value::as_u64).unwrap_or(1).clamp(1, 6) as usize;
            format!("{} {}", "#".repeat(level), inline(children(node)))
        }
        "bulletList" => render_list(node, |_| "- ".to_string()),
        "orderedList" => render_list(node, |i| format!("{}. ", i + 1)),
        "taskList" => render_list(node, |_| "- ".to_string()),
        "blockquote" => {
            let body = children(node).iter().map(render_block).collect::<Vec<_>>().join("\n\n");
            prefix_lines(&body, "> ", "> ")
        }
        "codeBlock" => {
            let language = node.pointer("/attrs/language").and_then(Value::as_str).unwrap_or("");
            let text: String = children(node).iter().filter_map(|c| c.get("text").and_then(Value::as_str)).collect();
            let fence = if text.contains("```") { "~~~~" } else { "```" };
            format!("{fence}{language}\n{text}\n{fence}")
        }
        "horizontalRule" => "---".to_string(),
        "table" => render_table(node),
        _ => children(node).iter().map(render_block).collect::<Vec<_>>().join("\n\n"),
    }
}

fn render_list(node: &Value, marker: impl Fn(usize) -> String) -> String {
    let is_task = node.get("type").and_then(Value::as_str) == Some("taskList");
    let mut lines = Vec::new();
    for (index, item) in children(node).iter().enumerate() {
        let prefix = if is_task {
            let checked = item.pointer("/attrs/checked").and_then(Value::as_bool).unwrap_or(false);
            if checked { "- [x] " } else { "- [ ] " }.to_string()
        } else {
            marker(index)
        };
        let body = children(item).iter().map(render_block).collect::<Vec<_>>().join("\n");
        let indent = " ".repeat(prefix.len());
        lines.push(prefix_lines(&body, &prefix, &indent));
    }
    lines.join("\n")
}

fn prefix_lines(body: &str, first: &str, rest: &str) -> String {
    body.lines()
        .enumerate()
        .map(|(i, line)| {
            let p = if i == 0 { first } else { rest };
            if line.is_empty() {
                p.trim_end().to_string()
            } else {
                format!("{p}{line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_table(node: &Value) -> String {
    let rows: Vec<Vec<String>> = children(node)
        .iter()
        .map(|row| {
            children(row)
                .iter()
                .map(|cell| inline(children(cell)).replace('|', "\\|").replace('\n', " "))
                .collect()
        })
        .collect();
    let Some(header) = rows.first() else { return String::new() };
    let width = header.len().max(1);
    let fmt = |cells: &[String]| {
        let padded: Vec<String> = (0..width).map(|i| cells.get(i).cloned().unwrap_or_default()).collect();
        format!("| {} |", padded.join(" | "))
    };
    let mut lines = vec![fmt(header), format!("| {} |", vec!["---"; width].join(" | "))];
    lines.extend(rows.iter().skip(1).map(|row| fmt(row)));
    lines.join("\n")
}

fn inline(nodes: &[Value]) -> String {
    let mut out = String::new();
    for node in nodes {
        match node.get("type").and_then(Value::as_str) {
            Some("hardBreak") => out.push_str("  \n"),
            Some("text") => {
                let text = node.get("text").and_then(Value::as_str).unwrap_or("");
                out.push_str(&apply_marks(text, node.get("marks")));
            }
            _ => out.push_str(&inline(children(node))),
        }
    }
    out
}

fn apply_marks(text: &str, marks: Option<&Value>) -> String {
    let names: Vec<(&str, Option<&Value>)> = marks
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .map(|m| (m.get("type").and_then(Value::as_str).unwrap_or(""), m.get("attrs")))
                .collect()
        })
        .unwrap_or_default();
    let has = |name: &str| names.iter().any(|(n, _)| *n == name);
    let mut out = if has("code") {
        // Code spans take their content verbatim: no escaping inside backticks.
        format!("`{text}`")
    } else {
        escape_markdown(text)
    };
    if has("bold") {
        out = format!("**{out}**");
    }
    if has("italic") {
        out = format!("*{out}*");
    }
    if let Some((_, attrs)) = names.iter().find(|(n, _)| *n == "link") {
        let href = attrs.and_then(|a| a.get("href")).and_then(Value::as_str).unwrap_or("");
        out = format!("[{out}]({href})");
    }
    out
}

fn escape_markdown(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '\\' | '*' | '_' | '[' | ']' | '`') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

// ---------------------------------------------------------------------------
// Markdown -> Document
// ---------------------------------------------------------------------------

pub fn from_markdown(source: &str) -> Value {
    let normalized = source.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.lines().collect();
    let blocks = parse_blocks(&lines, 0);
    if blocks.is_empty() {
        return empty_doc();
    }
    json!({ "type": "doc", "content": blocks })
}

/// Removes a leading level-1 heading and returns its text, used as the page title on import.
pub fn take_title(doc: &mut Value) -> Option<String> {
    let first = children(doc).first()?;
    if first.get("type").and_then(Value::as_str) != Some("heading")
        || first.pointer("/attrs/level").and_then(Value::as_u64) != Some(1)
    {
        return None;
    }
    let title = plain_text(first);
    let content = doc.get_mut("content")?.as_array_mut()?;
    content.remove(0);
    if content.is_empty() {
        content.push(json!({ "type": "paragraph" }));
    }
    Some(title)
}

fn parse_blocks(lines: &[&str], depth: usize) -> Vec<Value> {
    let mut blocks = Vec::new();
    if depth > MAX_NESTING {
        return blocks;
    }
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim_end();
        if trimmed.trim().is_empty() {
            i += 1;
            continue;
        }
        if let Some(fence) = fence_marker(trimmed) {
            let language = trimmed.trim_start().trim_start_matches(|c| c == '`' || c == '~').trim().to_string();
            let mut code = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with(fence) {
                code.push(lines[i]);
                i += 1;
            }
            i += 1; // closing fence, or end of input
            let mut block = json!({ "type": "codeBlock", "attrs": { "language": language } });
            if !code.is_empty() {
                block["content"] = json!([{ "type": "text", "text": code.join("\n") }]);
            }
            blocks.push(block);
            continue;
        }
        if let Some((level, text)) = heading(trimmed) {
            blocks.push(json!({ "type": "heading", "attrs": { "level": level }, "content": inline_nodes(text) }));
            i += 1;
            continue;
        }
        if is_rule(trimmed) {
            blocks.push(json!({ "type": "horizontalRule" }));
            i += 1;
            continue;
        }
        if trimmed.trim_start().starts_with('>') {
            let mut quoted = Vec::new();
            while i < lines.len() && lines[i].trim_start().starts_with('>') {
                let inner = lines[i].trim_start().trim_start_matches('>');
                quoted.push(inner.strip_prefix(' ').unwrap_or(inner));
                i += 1;
            }
            blocks.push(json!({ "type": "blockquote", "content": parse_blocks(&quoted, depth + 1) }));
            continue;
        }
        if is_table_start(lines, i) {
            let (table, next) = parse_table(lines, i);
            blocks.push(table);
            i = next;
            continue;
        }
        if list_marker(trimmed).is_some() {
            let (list, next) = parse_list(lines, i, depth);
            blocks.push(list);
            i = next;
            continue;
        }
        // Paragraph: consecutive lines until a blank line or another block starts.
        let mut para = Vec::new();
        while i < lines.len() {
            let current = lines[i].trim_end();
            if current.trim().is_empty() || (!para.is_empty() && starts_block(current)) {
                break;
            }
            para.push(current.trim());
            i += 1;
        }
        blocks.push(json!({ "type": "paragraph", "content": inline_nodes(&para.join(" ")) }));
    }
    blocks
}

fn starts_block(line: &str) -> bool {
    fence_marker(line).is_some()
        || heading(line).is_some()
        || is_rule(line)
        || line.trim_start().starts_with('>')
        || list_marker(line).is_some()
}

fn fence_marker(line: &str) -> Option<&'static str> {
    let t = line.trim_start();
    if t.starts_with("```") {
        Some("```")
    } else if t.starts_with("~~~") {
        Some("~~~")
    } else {
        None
    }
}

fn heading(line: &str) -> Option<(u64, &str)> {
    let t = line.trim_start();
    let hashes = t.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&hashes) && t[hashes..].starts_with(' ') {
        Some((hashes as u64, t[hashes..].trim()))
    } else {
        None
    }
}

fn is_rule(line: &str) -> bool {
    let t: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    t.len() >= 3 && (t.chars().all(|c| c == '-') || t.chars().all(|c| c == '*') || t.chars().all(|c| c == '_'))
}

#[derive(PartialEq, Clone, Copy)]
enum ListKind {
    Bullet,
    Ordered,
    Task,
}

/// Returns the list kind and the byte length of the marker (including its trailing space).
/// Only unindented markers start a list; nested lists are reached by recursion.
fn list_marker(line: &str) -> Option<(ListKind, usize)> {
    let bytes = line.as_bytes();
    if bytes.len() >= 2 && matches!(bytes[0], b'-' | b'*' | b'+') && bytes[1] == b' ' {
        let rest = &line[2..];
        if rest.starts_with("[ ] ") || rest.starts_with("[x] ") || rest.starts_with("[X] ") {
            return Some((ListKind::Task, 6));
        }
        return Some((ListKind::Bullet, 2));
    }
    let digits = line.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && line[digits..].starts_with(". ") {
        return Some((ListKind::Ordered, digits + 2));
    }
    None
}

fn parse_list(lines: &[&str], start: usize, depth: usize) -> (Value, usize) {
    let kind = list_marker(lines[start]).map_or(ListKind::Bullet, |(k, _)| k);
    let list_type = match kind {
        ListKind::Ordered => "orderedList",
        ListKind::Bullet => "bulletList",
        ListKind::Task => "taskList",
    };
    let mut items = Vec::new();
    let mut i = start;
    while i < lines.len() {
        let line = lines[i];
        let Some((item_kind, width)) = list_marker(line) else { break };
        if item_kind != kind {
            break;
        }
        // The task marker width already includes the "[ ] " checkbox.
        let first = &line[width..];
        let checked = kind == ListKind::Task && (line[2..].starts_with("[x] ") || line[2..].starts_with("[X] "));
        let mut body = vec![first.to_string()];
        i += 1;
        // Indented lines belong to the item. Blank lines count only when indented
        // content follows them.
        let mut pending_blank = 0;
        while i < lines.len() {
            let next = lines[i];
            if next.trim().is_empty() {
                pending_blank += 1;
                i += 1;
                continue;
            }
            let indent = next.len() - next.trim_start().len();
            if indent == 0 {
                break;
            }
            body.extend(std::iter::repeat_n(String::new(), pending_blank));
            pending_blank = 0;
            body.push(next.chars().skip(indent.min(width)).collect());
            i += 1;
        }
        i -= pending_blank;
        let refs: Vec<&str> = body.iter().map(String::as_str).collect();
        let mut children = parse_blocks(&refs, depth + 1);
        if children.is_empty() {
            children.push(json!({ "type": "paragraph" }));
        }
        items.push(if kind == ListKind::Task {
            json!({ "type": "taskItem", "attrs": { "checked": checked }, "content": children })
        } else {
            json!({ "type": "listItem", "content": children })
        });
    }
    let mut list = json!({ "type": list_type, "content": items });
    if kind == ListKind::Ordered {
        list["attrs"] = json!({ "start": 1 });
    }
    (list, i)
}

fn is_table_start(lines: &[&str], i: usize) -> bool {
    if i + 1 >= lines.len() {
        return false;
    }
    let header = lines[i].trim();
    let sep = lines[i + 1].trim();
    header.starts_with('|')
        && header.ends_with('|')
        && sep.starts_with('|')
        && sep.trim_matches('|').split('|').all(|c| {
            let c = c.trim();
            c.contains('-') && c.trim_matches(':').chars().all(|ch| ch == '-')
        })
}

fn split_row(line: &str) -> Vec<String> {
    let inner = line.trim().trim_start_matches('|');
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'|') {
            current.push('|');
            chars.next();
        } else if c == '|' {
            cells.push(current.trim().to_string());
            current.clear();
        } else {
            current.push(c);
        }
    }
    cells.push(current.trim().to_string());
    cells
}

fn parse_table(lines: &[&str], start: usize) -> (Value, usize) {
    let mut rows = Vec::new();
    let mut i = start;
    while i < lines.len() && lines[i].trim().starts_with('|') {
        if i != start + 1 {
            rows.push((i == start, split_row(lines[i])));
        }
        i += 1;
    }
    let row_nodes: Vec<Value> = rows
        .into_iter()
        .map(|(is_header, cells)| {
            let cell_type = if is_header { "tableHeader" } else { "tableCell" };
            let cells: Vec<Value> = cells
                .iter()
                .map(|text| json!({ "type": cell_type, "content": [{ "type": "paragraph", "content": inline_nodes(text) }] }))
                .collect();
            json!({ "type": "tableRow", "content": cells })
        })
        .collect();
    (json!({ "type": "table", "content": row_nodes }), i)
}

#[derive(Clone, Default)]
struct Marks {
    bold: bool,
    italic: bool,
    code: bool,
    link: Option<String>,
}

fn inline_nodes(text: &str) -> Vec<Value> {
    let mut nodes = Vec::new();
    parse_inline(text, &Marks::default(), &mut nodes);
    nodes
}

fn push_text(out: &mut Vec<Value>, text: &str, marks: &Marks) {
    if text.is_empty() {
        return;
    }
    let mut list = Vec::new();
    if marks.bold {
        list.push(json!({ "type": "bold" }));
    }
    if marks.italic {
        list.push(json!({ "type": "italic" }));
    }
    if marks.code {
        list.push(json!({ "type": "code" }));
    }
    if let Some(href) = &marks.link {
        list.push(json!({ "type": "link", "attrs": { "href": href } }));
    }
    let mut node = json!({ "type": "text", "text": text });
    if !list.is_empty() {
        node["marks"] = Value::Array(list);
    }
    out.push(node);
}

fn parse_inline(text: &str, marks: &Marks, out: &mut Vec<Value>) {
    let chars: Vec<char> = text.chars().collect();
    let mut buf = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && i + 1 < chars.len() && chars[i + 1].is_ascii_punctuation() {
            buf.push(chars[i + 1]);
            i += 2;
            continue;
        }
        if c == '`' {
            if let Some(end) = find_from(&chars, i + 1, &['`']) {
                push_text(out, &buf, marks);
                buf.clear();
                let code: String = chars[i + 1..end].iter().collect();
                push_text(out, &code, &Marks { code: true, ..marks.clone() });
                i = end + 1;
                continue;
            }
        }
        if c == '*' || c == '_' {
            let double = chars.get(i + 1) == Some(&c);
            let delim: &[char] = if double { &[c, c] } else { &[c] };
            if let Some(end) = find_from(&chars, i + delim.len(), delim) {
                if end > i + delim.len() {
                    push_text(out, &buf, marks);
                    buf.clear();
                    let inner: String = chars[i + delim.len()..end].iter().collect();
                    let next = if double {
                        Marks { bold: true, ..marks.clone() }
                    } else {
                        Marks { italic: true, ..marks.clone() }
                    };
                    parse_inline(&inner, &next, out);
                    i = end + delim.len();
                    continue;
                }
            }
        }
        if c == '[' {
            if let Some(close) = find_from(&chars, i + 1, &[']']) {
                if chars.get(close + 1) == Some(&'(') {
                    if let Some(paren) = find_from(&chars, close + 2, &[')']) {
                        let label: String = chars[i + 1..close].iter().collect();
                        let href: String = chars[close + 2..paren].iter().collect();
                        if !href.trim().is_empty() {
                            push_text(out, &buf, marks);
                            buf.clear();
                            let next = Marks { link: Some(href.trim().to_string()), ..marks.clone() };
                            parse_inline(&label, &next, out);
                            i = paren + 1;
                            continue;
                        }
                    }
                }
            }
        }
        buf.push(c);
        i += 1;
    }
    push_text(out, &buf, marks);
}

fn find_from(chars: &[char], start: usize, pattern: &[char]) -> Option<usize> {
    if chars.len() < pattern.len() {
        return None;
    }
    (start..=chars.len() - pattern.len()).find(|&j| chars[j..j + pattern.len()] == *pattern)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_joins_blocks_with_newlines() {
        let doc = from_markdown("# Title\n\nFirst para\n\n- item");
        let text = plain_text(&doc);
        assert!(text.contains("Title"));
        assert!(text.contains("First para"));
        assert!(text.contains("item"));
    }

    #[test]
    fn round_trips_common_blocks() {
        let source = "## Plan\n\nWe use **bold** and *italic* and `code`.\n\n- one\n- two\n\n1. first\n2. second\n\n- [ ] open task\n- [x] done task\n\n> quoted\n\n```rust\nlet x = 1;\n```\n\n---\n\n| Name | Owner |\n| --- | --- |\n| Auth | Ana |\n";
        let doc = from_markdown(source);
        assert_eq!(to_markdown(&doc), source);
    }

    #[test]
    fn imports_nested_lists() {
        let doc = from_markdown("- outer\n  - inner\n  - second inner\n- next");
        let list = &children(&doc)[0];
        let first_item = &children(list)[0];
        let nested = children(first_item)
            .iter()
            .find(|n| n["type"] == "bulletList")
            .expect("nested list");
        assert_eq!(children(nested).len(), 2);
    }

    #[test]
    fn escapes_literal_markers_on_export_and_import() {
        let doc = from_markdown("Price is 5\\* not *emphasis*");
        assert_eq!(plain_text(&doc), "Price is 5* not emphasis");
        assert!(to_markdown(&doc).contains("5\\*"));
    }

    #[test]
    fn extracts_internal_links_only() {
        let doc = json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [
                    { "type": "text", "text": "a", "marks": [{ "type": "link", "attrs": { "href": "threadwell://page/abc" } }] },
                    { "type": "text", "text": "b", "marks": [{ "type": "link", "attrs": { "href": "https://example.com" } }] }
                ]
            }]
        });
        assert_eq!(link_targets(&doc), vec!["abc".to_string()]);
    }

    #[test]
    fn take_title_removes_leading_h1() {
        let mut doc = from_markdown("# Meeting notes\n\nBody");
        assert_eq!(take_title(&mut doc).as_deref(), Some("Meeting notes"));
        assert_eq!(plain_text(&doc), "Body");
    }

    #[test]
    fn deeply_nested_input_is_bounded() {
        let source = "> ".repeat(200) + "deep";
        let _ = from_markdown(&source);
    }
}
