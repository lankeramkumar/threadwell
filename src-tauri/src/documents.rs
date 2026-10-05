//! Text extraction for Word (`.docx`) and PDF files, used by folder import.
//!
//! `.docx` is read from its XML: headings, paragraphs, list items and tables become Markdown.
//! Formatting beyond that (fonts, colours, images, comments, tracked changes) is dropped.
//! `.pdf` yields plain text only. Layout is lost, and scanned PDFs have no text layer and need
//! OCR, which this build does not include.

use std::io::{Cursor, Read};

use quick_xml::events::Event;
use quick_xml::Reader;

/// Limit on decompressed XML, so a crafted archive cannot exhaust memory.
const MAX_XML_BYTES: u64 = 20 * 1024 * 1024;

pub fn docx_to_markdown(bytes: &[u8]) -> Result<String, String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| "This is not a valid Word document".to_string())?;
    let mut xml = String::new();
    {
        let entry = archive
            .by_name("word/document.xml")
            .map_err(|_| "The Word document has no main text part".to_string())?;
        entry
            .take(MAX_XML_BYTES)
            .read_to_string(&mut xml)
            .map_err(|_| "The Word document text could not be read".to_string())?;
    }
    let markdown = parse_document_xml(&xml)?;
    if markdown.trim().is_empty() {
        return Err("The Word document has no text".into());
    }
    Ok(markdown)
}

#[derive(Default)]
struct Para {
    heading: Option<u8>,
    list: bool,
    text: String,
}

/// Converts WordprocessingML to Markdown lines. Tables become rows with `|` separators.
pub fn parse_document_xml(xml: &str) -> Result<String, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut out: Vec<String> = Vec::new();
    let mut para: Option<Para> = None;
    let mut in_text = false;
    let mut table_rows: Option<Vec<String>> = None;
    let mut cell: Option<String> = None;
    let mut row: Option<Vec<String>> = None;

    loop {
        match reader.read_event() {
            Err(_) => return Err("The Word document XML is damaged".into()),
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match e.name().as_ref() {
                b"w:tbl" => table_rows = Some(Vec::new()),
                b"w:tr" => row = Some(Vec::new()),
                b"w:tc" => cell = Some(String::new()),
                b"w:p" => para = Some(Para::default()),
                b"w:pStyle" => {
                    if let Some(p) = para.as_mut() {
                        let style = attr(&e, b"w:val").unwrap_or_default();
                        p.heading = heading_level(&style);
                    }
                }
                b"w:numPr" => {
                    if let Some(p) = para.as_mut() {
                        p.list = true;
                    }
                }
                b"w:t" => in_text = true,
                b"w:tab" => push_text(&mut para, &mut cell, "\t"),
                b"w:br" | b"w:cr" => push_text(&mut para, &mut cell, " "),
                _ => {}
            },
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"w:t" => in_text = false,
                b"w:p" => {
                    if let Some(p) = para.take() {
                        let text = p.text.trim().to_string();
                        if let Some(c) = cell.as_mut() {
                            if !c.is_empty() && !text.is_empty() {
                                c.push(' ');
                            }
                            c.push_str(&text);
                        } else if !text.is_empty() {
                            let line = match (p.heading, p.list) {
                                (Some(level), _) => format!("{} {text}", "#".repeat(level as usize)),
                                (None, true) => format!("- {text}"),
                                _ => text,
                            };
                            out.push(line);
                        }
                    }
                }
                b"w:tc" => {
                    if let (Some(c), Some(r)) = (cell.take(), row.as_mut()) {
                        r.push(c.replace('|', "\\|"));
                    }
                }
                b"w:tr" => {
                    if let (Some(r), Some(t)) = (row.take(), table_rows.as_mut()) {
                        t.push(format!("| {} |", r.join(" | ")));
                    }
                }
                b"w:tbl" => {
                    if let Some(rows) = table_rows.take() {
                        if let Some(first) = rows.first() {
                            let cols = first.matches('|').count().saturating_sub(1).max(1);
                            out.push(String::new());
                            out.push(first.clone());
                            out.push(format!("|{}", " --- |".repeat(cols)));
                            out.extend(rows.iter().skip(1).cloned());
                            out.push(String::new());
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::Text(t)) => {
                if in_text {
                    let text = t.unescape().map_err(|_| "The Word document contains invalid text".to_string())?;
                    push_text(&mut para, &mut cell, &text);
                }
            }
            Ok(_) => {}
        }
    }
    Ok(out.join("\n\n"))
}

fn push_text(para: &mut Option<Para>, cell: &mut Option<String>, text: &str) {
    if let Some(p) = para.as_mut() {
        p.text.push_str(text);
    }
    let _ = cell;
}

fn attr(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<String> {
    e.attributes().flatten().find(|a| a.key.as_ref() == key).and_then(|a| String::from_utf8(a.value.into_owned()).ok())
}

fn heading_level(style: &str) -> Option<u8> {
    let digits: String = style.chars().filter(|c| c.is_ascii_digit()).collect();
    let level: u8 = digits.parse().ok()?;
    let is_heading = style.to_ascii_lowercase().starts_with("heading") || style.to_ascii_lowercase().starts_with("title");
    if is_heading && (1..=6).contains(&level) {
        Some(level)
    } else if style.eq_ignore_ascii_case("Title") {
        Some(1)
    } else {
        None
    }
}

/// Plain text from a PDF. Returns an error for files it cannot read, instead of panicking.
pub fn pdf_to_text(bytes: &[u8]) -> Result<String, String> {
    let owned = bytes.to_vec();
    let result = std::panic::catch_unwind(move || pdf_extract::extract_text_from_mem(&owned));
    let text = match result {
        Ok(Ok(text)) => text,
        Ok(Err(_)) => return Err("This PDF could not be read".into()),
        Err(_) => return Err("This PDF could not be read (it is damaged or uses unsupported features)".into()),
    };
    let cleaned = text.replace('\r', "");
    if cleaned.trim().is_empty() {
        return Err("This PDF has no text layer. Scanned PDFs need OCR, which Threadwell does not include".into());
    }
    Ok(cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn docx_bytes(document_xml: &str) -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buffer);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("word/document.xml", options).unwrap();
            zip.write_all(document_xml.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        buffer.into_inner()
    }

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Launch plan</w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">Ship by </w:t></w:r><w:r><w:t>15 November.</w:t></w:r></w:p>
<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>Copy sign-off</w:t></w:r></w:p>
<w:tbl><w:tr><w:tc><w:p><w:r><w:t>Area</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Owner</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:p><w:r><w:t>Copy</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Maria</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
</w:body></w:document>"#;

    #[test]
    fn word_headings_paragraphs_lists_and_tables_become_markdown() {
        let md = docx_to_markdown(&docx_bytes(SAMPLE)).unwrap();
        assert!(md.contains("# Launch plan"));
        assert!(md.contains("Ship by 15 November."), "runs within a paragraph are joined");
        assert!(md.contains("- Copy sign-off"));
        assert!(md.contains("| Area | Owner |"));
        assert!(md.contains("| Copy | Maria |"));
    }

    #[test]
    fn a_zip_without_document_xml_is_refused() {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buffer);
            zip.start_file("other.txt", zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(b"x").unwrap();
            zip.finish().unwrap();
        }
        assert!(docx_to_markdown(&buffer.into_inner()).is_err());
    }

    #[test]
    fn garbage_is_refused_without_panicking() {
        assert!(docx_to_markdown(b"not a zip at all").is_err());
        assert!(pdf_to_text(b"%PDF-1.4 truncated and damaged").is_err());
        assert!(pdf_to_text(&[0u8; 64]).is_err());
    }
}


/// Parses CSV (RFC 4180: quoted fields, doubled quotes, commas or newlines inside quotes) and
/// renders it as a Markdown table. The first row becomes the header. Ragged rows are padded.
pub fn csv_to_markdown(text: &str) -> Result<String, String> {
    const MAX_ROWS: usize = 2_000;
    let rows = parse_csv(text)?;
    let rows: Vec<Vec<String>> = rows.into_iter().filter(|r| r.iter().any(|c| !c.trim().is_empty())).collect();
    let Some(header) = rows.first() else {
        return Err("The CSV file has no rows".into());
    };
    let width = rows.iter().take(MAX_ROWS).map(Vec::len).max().unwrap_or(1).max(1);
    let cell = |value: &str| value.replace('\n', " ").replace('|', "\\|").trim().to_string();
    let line = |cells: &[String]| {
        let padded: Vec<String> = (0..width).map(|i| cells.get(i).map(|c| cell(c)).unwrap_or_default()).collect();
        format!("| {} |", padded.join(" | "))
    };
    let mut out = vec![line(header), format!("|{}", " --- |".repeat(width))];
    out.extend(rows.iter().skip(1).take(MAX_ROWS - 1).map(|r| line(r)));
    if rows.len() > MAX_ROWS {
        out.push(String::new());
        out.push(format!("_Only the first {MAX_ROWS} rows were imported._"));
    }
    Ok(out.join("\n"))
}

fn parse_csv(text: &str) -> Result<Vec<Vec<String>>, String> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(c);
            }
        } else {
            match c {
                '"' if field.is_empty() => in_quotes = true,
                ',' => row.push(std::mem::take(&mut field)),
                '\r' => {}
                '\n' => {
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                }
                _ => field.push(c),
            }
        }
    }
    if in_quotes {
        return Err("The CSV file has an unclosed quoted value".into());
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    Ok(rows)
}

#[cfg(test)]
mod csv_tests {
    use super::*;

    #[test]
    fn csv_becomes_a_markdown_table_with_quoted_fields() {
        let md = csv_to_markdown("Name,Note\nAna,\"likes, commas\"\nBen,\"says \"\"hi\"\"\"\n").unwrap();
        assert!(md.starts_with("| Name | Note |"));
        assert!(md.contains("| Ana | likes, commas |"));
        assert!(md.contains("| Ben | says \"hi\" |"));
    }

    #[test]
    fn ragged_rows_are_padded_and_pipes_escaped() {
        let md = csv_to_markdown("a,b,c\n1,2\nx|y,z,w\n").unwrap();
        assert!(md.contains("| 1 | 2 |  |"));
        assert!(md.contains("x\\|y"));
    }

    #[test]
    fn unclosed_quotes_and_empty_files_are_refused() {
        assert!(csv_to_markdown("a,\"open\n").is_err());
        assert!(csv_to_markdown("\n\n").is_err());
    }
}
