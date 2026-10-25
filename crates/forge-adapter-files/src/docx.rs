//! Reads a docx file's body into blocks of text for the Files panel's document view.

use std::collections::HashMap;

use forge_ports::{DocBlock, DocumentView, FileContent, FilesError, TextRun};
use quick_xml::events::{BytesStart, Event};

use crate::office_zip;

const MAX_BLOCKS: usize = 2_000;
const MAX_ROWS: usize = 100;
const MAX_COLUMNS: usize = 20;
// One paragraph, heading, list item or table cell keeps at most this many characters.
const MAX_TEXT_CHARS: usize = 10_000;
// A part stops being read after this many XML events, so a part of millions of empty tags cannot
// keep the read busy for long.
const EVENT_LIMIT: usize = 5_000_000;
// Word nests a list 9 deep (`w:ilvl` 0 to 8).
const MAX_LIST_LEVEL: u8 = 8;
// Elements whose text is not shown: tracked deletions and moves away, drawings (text boxes are not
// drawn), the fallback copy of alternate content, and the old properties a tracked change keeps.
const HIDDEN: [&str; 14] = [
    "del",
    "moveFrom",
    "drawing",
    "pict",
    "object",
    "Fallback",
    "pPrChange",
    "rPrChange",
    "sectPrChange",
    "tblPrChange",
    "trPrChange",
    "tcPrChange",
    "tblGridChange",
    "numberingChange",
];

fn failed(error: impl std::fmt::Display) -> FilesError {
    FilesError::Failed(error.to_string())
}

/// The body of a docx file (`word/document.xml`) as blocks. Every part is checked against the zip
/// guards before it is parsed; entities other than the five predefined ones and character
/// references are not resolved, and nothing outside the file's own zip is opened.
pub(crate) fn read_docx(bytes: Vec<u8>) -> Result<FileContent, FilesError> {
    let limit = office_zip::PART_BYTES;
    let mut archive = office_zip::open(&bytes, "This document is password-protected, so it cannot be shown.")?;
    let too_big = || failed("This document unpacks to too much data to show.");
    if office_zip::declares_too_much(&mut archive, limit) {
        return Err(too_big());
    }
    let document = office_zip::read_part(&mut archive, "word/document.xml", limit, too_big)?
        .ok_or_else(|| failed("This file has no word/document.xml, so it is not a Word document."))?;
    // A missing or damaged styles or numbering part only costs its lookups.
    let styles = office_zip::read_part(&mut archive, "word/styles.xml", limit, too_big)?.map(|xml| heading_styles(&xml)).unwrap_or_default();
    let numbering = office_zip::read_part(&mut archive, "word/numbering.xml", limit, too_big)?.map(|xml| Numbering::of(&xml)).unwrap_or_default();
    let mut body = Body::new(&styles, &numbering);
    if walk(&document, |event| body.event(event))? {
        body.truncated = true;
    }
    Ok(FileContent::Document(DocumentView { blocks: body.blocks, truncated: body.truncated }))
}

// Feeds the part's XML events to `on` until it returns false or the part ends. True when the
// event limit stopped it. XML whose tags do not match or are left open fails with text.
fn walk(xml: &[u8], mut on: impl FnMut(Event<'_>) -> bool) -> Result<bool, FilesError> {
    let mut reader = quick_xml::Reader::from_reader(xml);
    let mut open = 0usize;
    for _ in 0..EVENT_LIMIT {
        let event = reader.read_event().map_err(failed)?;
        match &event {
            Event::Eof if open > 0 => return Err(failed("The XML ends before all its tags are closed.")),
            Event::Eof => return Ok(false),
            Event::Start(_) => open += 1,
            Event::End(_) => open = open.saturating_sub(1),
            _ => {}
        }
        if !on(event) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn attribute(tag: &BytesStart<'_>, name: &str) -> Option<String> {
    tag.attributes().flatten().find(|attribute| attribute.key.local_name().as_ref() == name).map(|attribute| attribute.value.into_owned())
}

// A `w:b` or `w:i` is on unless its `w:val` turns it off.
fn is_on(tag: &BytesStart<'_>) -> bool {
    !attribute(tag, "val").is_some_and(|value| matches!(value.as_str(), "0" | "false" | "off"))
}

// `text` cut to `room` characters, and whether it was cut.
fn cut(text: &str, room: usize) -> (&str, bool) {
    match text.char_indices().nth(room) {
        Some((at, _)) => (&text[..at], true),
        None => (text, false),
    }
}

// The heading level of a style id or name: `Heading1`…`Heading9` or `heading 1`… (above 6 is 6),
// and `Title` is 1.
fn level_of_style(name: &str) -> Option<u8> {
    let name = name.to_ascii_lowercase().replace(' ', "");
    if name == "title" {
        return Some(1);
    }
    let level: u8 = name.strip_prefix("heading")?.parse().ok()?;
    (1..=9).contains(&level).then_some(level.min(6))
}

// The heading level of each paragraph style whose name (not id) is a heading's, since Word in
// some languages gives the built-in heading styles other ids (Korean Word uses `1`, `2` …).
fn heading_styles(xml: &[u8]) -> HashMap<String, u8> {
    let mut levels = HashMap::new();
    let mut style: Option<String> = None;
    let _ = walk(xml, |event| {
        match event {
            Event::Start(tag) if tag.local_name().as_ref() == "style" => style = attribute(&tag, "styleId"),
            Event::End(tag) if tag.local_name().as_ref() == "style" => style = None,
            Event::Empty(tag) | Event::Start(tag) if tag.local_name().as_ref() == "name" => {
                if let (Some(id), Some(level)) = (style.as_ref(), attribute(&tag, "val").as_deref().and_then(level_of_style)) {
                    levels.insert(id.clone(), level);
                }
            }
            _ => {}
        }
        true
    });
    levels
}

/// The number formats of `word/numbering.xml`, to tell a numbered list from a bulleted one.
#[derive(Default)]
struct Numbering {
    // (abstractNumId, level) → numFmt
    formats: HashMap<(String, u8), String>,
    // numId → abstractNumId
    lists: HashMap<String, String>,
    // (numId, level) → the numFmt of a level override
    overrides: HashMap<(String, u8), String>,
}

impl Numbering {
    fn of(xml: &[u8]) -> Self {
        let mut numbering = Self::default();
        let (mut abstract_id, mut num_id, mut level): (Option<String>, Option<String>, Option<u8>) = (None, None, None);
        let _ = walk(xml, |event| {
            match event {
                Event::Start(tag) => match tag.local_name().as_ref() {
                    "abstractNum" => abstract_id = attribute(&tag, "abstractNumId"),
                    "num" => num_id = attribute(&tag, "numId"),
                    "lvl" | "lvlOverride" => level = attribute(&tag, "ilvl").and_then(|value| value.parse().ok()),
                    _ => {}
                },
                Event::End(tag) => match tag.local_name().as_ref() {
                    "abstractNum" => abstract_id = None,
                    "num" => num_id = None,
                    "lvlOverride" => level = None,
                    _ => {}
                },
                Event::Empty(tag) => match tag.local_name().as_ref() {
                    // The first format wins: alternate content puts a fallback format after it.
                    "numFmt" => {
                        if let (Some(level), Some(format)) = (level, attribute(&tag, "val")) {
                            if let Some(id) = &abstract_id {
                                numbering.formats.entry((id.clone(), level)).or_insert(format);
                            } else if let Some(id) = &num_id {
                                numbering.overrides.entry((id.clone(), level)).or_insert(format);
                            }
                        }
                    }
                    "abstractNumId" => {
                        if let (Some(id), Some(target)) = (&num_id, attribute(&tag, "val")) {
                            numbering.lists.insert(id.clone(), target);
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
            true
        });
        numbering
    }

    // Numbered unless the level's format is `bullet`; a level that cannot be resolved is a bullet.
    fn numbered(&self, num_id: &str, level: u8) -> bool {
        let format = self.overrides.get(&(num_id.to_string(), level)).or_else(|| self.formats.get(&(self.lists.get(num_id)?.clone(), level)));
        format.is_some_and(|format| format != "bullet")
    }
}

#[derive(Default)]
struct Paragraph {
    style: Option<String>,
    listed: bool,
    num_id: Option<String>,
    level: u8,
    runs: Vec<TextRun>,
    chars: usize,
}

/// The state of a walk through `word/document.xml`. Only the outermost table has rows and cells;
/// the text of a nested table goes into the outer cell it sits in.
struct Body<'a> {
    styles: &'a HashMap<String, u8>,
    numbering: &'a Numbering,
    blocks: Vec<DocBlock>,
    truncated: bool,
    // How deep the walk is inside a hidden element.
    hidden: usize,
    paragraph: Option<Paragraph>,
    in_properties: bool,
    in_run: bool,
    in_run_properties: bool,
    in_text: bool,
    bold: bool,
    italic: bool,
    table_depth: usize,
    rows: Vec<Vec<String>>,
    in_row: bool,
    // The open cell of the outermost table and its character count.
    cell: Option<(String, usize)>,
}

impl<'a> Body<'a> {
    fn new(styles: &'a HashMap<String, u8>, numbering: &'a Numbering) -> Self {
        Self {
            styles,
            numbering,
            blocks: Vec::new(),
            truncated: false,
            hidden: 0,
            paragraph: None,
            in_properties: false,
            in_run: false,
            in_run_properties: false,
            in_text: false,
            bold: false,
            italic: false,
            table_depth: 0,
            rows: Vec::new(),
            in_row: false,
            cell: None,
        }
    }

    // False once the block cap is reached, which ends the walk.
    fn event(&mut self, event: Event<'_>) -> bool {
        if self.hidden > 0 {
            match event {
                Event::Start(_) => self.hidden += 1,
                Event::End(_) => self.hidden -= 1,
                _ => {}
            }
            return true;
        }
        match event {
            Event::Start(tag) => {
                if HIDDEN.contains(&tag.local_name().as_ref()) {
                    self.hidden = 1;
                    return true;
                }
                self.open(&tag, false);
            }
            Event::Empty(tag) => self.open(&tag, true),
            Event::End(tag) => return self.close(tag.local_name().as_ref()),
            Event::Text(text) if self.in_text => self.add_text(&text.xml10_content()),
            Event::CData(text) if self.in_text => self.add_text(&text.xml10_content()),
            Event::GeneralRef(reference) if self.in_text => {
                let resolved = match reference.resolve_char_ref() {
                    Ok(Some(character)) => Some(character.to_string()),
                    Ok(None) => quick_xml::escape::resolve_predefined_entity(&reference).map(str::to_string),
                    Err(_) => None,
                };
                if let Some(text) = resolved {
                    self.add_text(&text);
                }
            }
            _ => {}
        }
        true
    }

    fn open(&mut self, tag: &BytesStart<'_>, empty: bool) {
        let in_properties = self.in_properties;
        match tag.local_name().as_ref() {
            "p" if !empty && self.paragraph.is_none() => {
                self.paragraph = Some(Paragraph::default());
                self.in_properties = false;
            }
            "pPr" if !empty && self.paragraph.is_some() && !self.in_run => self.in_properties = true,
            "pStyle" if in_properties => self.with_paragraph(|paragraph| paragraph.style = attribute(tag, "val")),
            "numPr" if in_properties => self.with_paragraph(|paragraph| paragraph.listed = true),
            "ilvl" if in_properties => {
                let level = attribute(tag, "val").and_then(|value| value.parse::<u8>().ok()).unwrap_or(0).min(MAX_LIST_LEVEL);
                self.with_paragraph(|paragraph| paragraph.level = level);
            }
            "numId" if in_properties => self.with_paragraph(|paragraph| paragraph.num_id = attribute(tag, "val")),
            "r" if !empty => {
                self.in_run = true;
                (self.bold, self.italic) = (false, false);
            }
            "rPr" if !empty && self.in_run => self.in_run_properties = true,
            "b" if self.in_run_properties => self.bold = is_on(tag),
            "i" if self.in_run_properties => self.italic = is_on(tag),
            "t" if !empty && self.in_run => self.in_text = true,
            "tab" | "br" | "cr" if self.in_run && !self.in_run_properties => self.add_text(" "),
            "tbl" if !empty => {
                self.table_depth += 1;
                if self.table_depth == 1 {
                    self.rows.clear();
                    (self.in_row, self.cell) = (false, None);
                }
            }
            "tr" if !empty && self.table_depth == 1 => {
                self.in_row = self.rows.len() < MAX_ROWS;
                if self.in_row {
                    self.rows.push(Vec::new());
                } else {
                    self.truncated = true;
                }
            }
            "tc" if self.table_depth == 1 && self.in_row => {
                if self.rows.last().is_some_and(|row| row.len() >= MAX_COLUMNS) {
                    self.truncated = true;
                } else if empty {
                    if let Some(row) = self.rows.last_mut() {
                        row.push(String::new());
                    }
                } else {
                    self.cell = Some((String::new(), 0));
                }
            }
            _ => {}
        }
    }

    fn with_paragraph(&mut self, change: impl FnOnce(&mut Paragraph)) {
        if let Some(paragraph) = self.paragraph.as_mut() {
            change(paragraph);
        }
    }

    // False once the block cap is reached.
    fn close(&mut self, name: &str) -> bool {
        match name {
            "p" => return self.end_paragraph(),
            "pPr" => self.in_properties = false,
            "r" => (self.in_run, self.in_run_properties) = (false, false),
            "rPr" => self.in_run_properties = false,
            "t" => self.in_text = false,
            "tc" if self.table_depth == 1 => {
                if let (Some((text, _)), Some(row)) = (self.cell.take(), self.rows.last_mut()) {
                    row.push(text);
                }
            }
            "tr" if self.table_depth == 1 => self.in_row = false,
            "tbl" if self.table_depth > 0 => {
                self.table_depth -= 1;
                if self.table_depth == 0 {
                    self.cell = None;
                    let rows = std::mem::take(&mut self.rows);
                    if !rows.is_empty() {
                        return self.push(DocBlock::Table { rows });
                    }
                }
            }
            _ => {}
        }
        true
    }

    // Adds text to the open paragraph with the open run's bold and italic, up to the text cap.
    fn add_text(&mut self, text: &str) {
        let Some(paragraph) = self.paragraph.as_mut() else { return };
        let (kept, was_cut) = cut(text, MAX_TEXT_CHARS - paragraph.chars);
        self.truncated |= was_cut;
        if kept.is_empty() {
            return;
        }
        paragraph.chars += kept.chars().count();
        match paragraph.runs.last_mut() {
            Some(last) if (last.bold, last.italic) == (self.bold, self.italic) => last.text.push_str(kept),
            _ => paragraph.runs.push(TextRun { text: kept.to_string(), bold: self.bold, italic: self.italic }),
        }
    }

    // A paragraph with text becomes a block, or inside a table joins its cell's text with a space.
    fn end_paragraph(&mut self) -> bool {
        self.in_properties = false;
        let Some(paragraph) = self.paragraph.take() else { return true };
        let text: String = paragraph.runs.iter().map(|run| run.text.as_str()).collect();
        if text.trim().is_empty() {
            return true;
        }
        if self.table_depth > 0 {
            if let Some((cell, chars)) = self.cell.as_mut() {
                let joined = if cell.is_empty() { text } else { format!(" {text}") };
                let (kept, was_cut) = cut(&joined, MAX_TEXT_CHARS - *chars);
                self.truncated |= was_cut;
                *chars += kept.chars().count();
                cell.push_str(kept);
            }
            return true;
        }
        let heading = paragraph.style.as_deref().and_then(|style| level_of_style(style).or_else(|| self.styles.get(style).copied()));
        let block = if let Some(level) = heading {
            DocBlock::Heading { level, text }
        } else if paragraph.listed && paragraph.num_id.as_deref() != Some("0") {
            let numbered = paragraph.num_id.as_deref().is_some_and(|id| self.numbering.numbered(id, paragraph.level));
            DocBlock::ListItem { level: paragraph.level, numbered, runs: paragraph.runs }
        } else {
            DocBlock::Paragraph { runs: paragraph.runs }
        };
        self.push(block)
    }

    fn push(&mut self, block: DocBlock) -> bool {
        if self.blocks.len() == MAX_BLOCKS {
            self.truncated = true;
            return false;
        }
        self.blocks.push(block);
        true
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use forge_ports::{DocBlock, DocumentView, FilesPort, TextRun};

    use super::*;
    use crate::LocalFiles;

    const W: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#;

    // A hand-made docx of this body and these other parts.
    fn docx_with(body: &str, parts: &[(&str, String)]) -> Vec<u8> {
        let document = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document {W}><w:body>{body}</w:body></w:document>"#);
        let mut all = vec![
            (
                "[Content_Types].xml",
                r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#.to_string(),
            ),
            ("word/document.xml", document),
        ];
        all.extend(parts.iter().map(|(name, text)| (*name, text.clone())));
        zip_of(&all)
    }

    fn zip_of(parts: &[(&str, String)]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, text) in parts {
            zip.start_file(*name, options).unwrap();
            zip.write_all(text.as_bytes()).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    fn docx(body: &str) -> Vec<u8> {
        docx_with(body, &[])
    }

    fn view(bytes: Vec<u8>) -> DocumentView {
        match read_docx(bytes) {
            Ok(FileContent::Document(view)) => view,
            other => panic!("not a document: {other:?}"),
        }
    }

    fn failure(bytes: Vec<u8>) -> String {
        match read_docx(bytes) {
            Err(FilesError::Failed(message)) => message,
            other => panic!("did not fail: {other:?}"),
        }
    }

    fn p(text: &str) -> String {
        format!("<w:p><w:r><w:t>{text}</w:t></w:r></w:p>")
    }

    fn styled(style: &str, text: &str) -> String {
        format!(r#"<w:p><w:pPr><w:pStyle w:val="{style}"/></w:pPr><w:r><w:t>{text}</w:t></w:r></w:p>"#)
    }

    fn run(text: &str, bold: bool, italic: bool) -> TextRun {
        TextRun { text: text.to_string(), bold, italic }
    }

    fn plain(text: &str) -> DocBlock {
        DocBlock::Paragraph { runs: vec![run(text, false, false)] }
    }

    fn heading(level: u8, text: &str) -> DocBlock {
        DocBlock::Heading { level, text: text.to_string() }
    }

    fn table(body: &[&[&str]]) -> String {
        let rows: String = body.iter().map(|row| format!("<w:tr>{}</w:tr>", row.iter().map(|cell| format!("<w:tc>{}</w:tc>", p(cell))).collect::<String>())).collect();
        format!("<w:tbl>{rows}</w:tbl>")
    }

    #[test]
    fn a_docx_gives_headings_paragraphs_and_runs_with_bold_and_italic() {
        let body = [
            styled("Title", "The report"),
            styled("Heading1", "Intro"),
            r#"<w:p><w:r><w:t xml:space="preserve">plain </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>strong</w:t></w:r><w:r><w:rPr><w:i/><w:b w:val="0"/></w:rPr><w:t xml:space="preserve"> slanted</w:t></w:r><w:r><w:rPr><w:b w:val="false"/></w:rPr><w:tab/><w:t>a&amp;b</w:t><w:br/><w:t>&#x41;</w:t></w:r><w:hyperlink w:history="1"><w:r><w:rPr><w:b w:val="1"/></w:rPr><w:t>link</w:t></w:r></w:hyperlink></w:p>"#.to_string(),
            // Bold on the paragraph mark is not bold text.
            r#"<w:p><w:pPr><w:rPr><w:b/></w:rPr></w:pPr><w:r><w:t>after</w:t></w:r></w:p>"#.to_string(),
        ]
        .concat();
        let view = view(docx(&body));
        assert_eq!(
            view.blocks,
            vec![
                heading(1, "The report"),
                heading(1, "Intro"),
                DocBlock::Paragraph { runs: vec![run("plain ", false, false), run("strong", true, false), run(" slanted", false, true), run(" a&b A", false, false), run("link", true, false)] },
                plain("after"),
            ]
        );
        assert!(!view.truncated);
    }

    #[test]
    fn heading_levels_above_six_become_six() {
        let body = [styled("Heading2", "two"), styled("Heading6", "six"), styled("Heading7", "seven"), styled("Heading9", "nine")].concat();
        assert_eq!(view(docx(&body)).blocks, vec![heading(2, "two"), heading(6, "six"), heading(6, "seven"), heading(6, "nine")]);
    }

    #[test]
    fn a_heading_style_with_another_id_is_found_by_its_name() {
        // Word in Korean names the built-in heading styles `1`, `2` … with the name `heading 1`.
        let styles = format!(r#"<?xml version="1.0"?><w:styles {W}><w:style w:type="paragraph" w:styleId="1"><w:name w:val="heading 1"/></w:style><w:style w:type="paragraph" w:styleId="a"><w:name w:val="Title"/></w:style><w:style w:type="paragraph" w:styleId="b"><w:name w:val="Body"/></w:style></w:styles>"#);
        let body = [styled("1", "one"), styled("a", "title"), styled("b", "body")].concat();
        assert_eq!(view(docx_with(&body, &[("word/styles.xml", styles)])).blocks, vec![heading(1, "one"), heading(1, "title"), plain("body")]);
    }

    fn listed(num_id: &str, level: u8, text: &str) -> String {
        format!(r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="{level}"/><w:numId w:val="{num_id}"/></w:numPr></w:pPr><w:r><w:t>{text}</w:t></w:r></w:p>"#)
    }

    fn item(level: u8, numbered: bool, text: &str) -> DocBlock {
        DocBlock::ListItem { level, numbered, runs: vec![run(text, false, false)] }
    }

    #[test]
    fn a_list_paragraph_is_a_list_item_numbered_by_numbering_xml() {
        let numbering = format!(
            r#"<?xml version="1.0"?><w:numbering {W}><w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:numFmt w:val="bullet"/></w:lvl><w:lvl w:ilvl="1"><w:numFmt w:val="decimal"/></w:lvl></w:abstractNum><w:abstractNum w:abstractNumId="1"><w:lvl w:ilvl="0"><w:numFmt w:val="lowerLetter"/></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num><w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num><w:num w:numId="3"><w:abstractNumId w:val="0"/><w:lvlOverride w:ilvl="0"><w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl></w:lvlOverride></w:num></w:numbering>"#
        );
        let body = [listed("1", 0, "dot"), listed("1", 1, "one"), listed("2", 0, "a"), listed("3", 0, "over"), listed("9", 0, "unknown"), listed("0", 0, "none")].concat();
        assert_eq!(
            view(docx_with(&body, &[("word/numbering.xml", numbering)])).blocks,
            vec![item(0, false, "dot"), item(1, true, "one"), item(0, true, "a"), item(0, true, "over"), item(0, false, "unknown"), plain("none")]
        );
        // Without a numbering part every list item is a bullet.
        assert_eq!(view(docx(&listed("2", 0, "a"))).blocks, vec![item(0, false, "a")]);
    }

    fn rows(cells: &[&[&str]]) -> DocBlock {
        DocBlock::Table { rows: cells.iter().map(|row| row.iter().map(|cell| cell.to_string()).collect()).collect() }
    }

    #[test]
    fn a_table_becomes_rows_of_cell_text() {
        let two_paragraphs = format!("<w:tbl><w:tr><w:tc>{}{}</w:tc><w:tc/></w:tr></w:tbl>", p("x"), p("y"));
        let body = [table(&[&["a", "b"], &["c", "d"]]), two_paragraphs].concat();
        assert_eq!(view(docx(&body)).blocks, vec![rows(&[&["a", "b"], &["c", "d"]]), rows(&[&["x y", ""]])]);
    }

    #[test]
    fn a_nested_table_is_flattened_into_the_cell() {
        let inner = table(&[&["in1", "in2"], &["in3"]]);
        let body = format!("<w:tbl><w:tr><w:tc>{}{inner}</w:tc><w:tc>{}</w:tc></w:tr></w:tbl>{}", p("outer"), p("next"), p("after"));
        let view = view(docx(&body));
        assert_eq!(view.blocks, vec![rows(&[&["outer in1 in2 in3", "next"]]), plain("after")]);
        assert!(!view.truncated);
    }

    #[test]
    fn deleted_text_is_hidden_and_inserted_text_is_shown() {
        let body = r#"<w:p><w:r><w:t xml:space="preserve">keep </w:t></w:r><w:del w:id="1"><w:r><w:delText>gone</w:delText></w:r></w:del><w:del w:id="2"><w:r><w:t>gone too</w:t></w:r></w:del><w:ins w:id="3"><w:r><w:t>added</w:t></w:r></w:ins></w:p><w:p><w:pPr><w:pStyle w:val="Heading1"/><w:pPrChange w:id="4"><w:pPr><w:pStyle w:val="Normal"/></w:pPr></w:pPrChange></w:pPr><w:r><w:t>moved up</w:t></w:r></w:p>"#;
        assert_eq!(view(docx(body)).blocks, vec![plain("keep added"), heading(1, "moved up")]);
    }

    #[test]
    fn an_empty_body_gives_no_blocks() {
        for body in ["", "<w:p/>", r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr></w:p><w:sectPr/>"#] {
            let view = view(docx(body));
            assert_eq!(view.blocks, vec![], "{body}");
            assert!(!view.truncated);
        }
    }

    #[test]
    fn more_than_2000_blocks_are_cut_and_marked() {
        let view_of = |count: usize| view(docx(&(0..count).map(|index| p(&index.to_string())).collect::<String>()));
        let exact = view_of(2000);
        assert_eq!((exact.blocks.len(), exact.truncated), (2000, false));
        let over = view_of(2001);
        assert_eq!((over.blocks.len(), over.truncated), (2000, true));
        assert_eq!(over.blocks[1999], plain("1999"));
    }

    #[test]
    fn a_table_over_100_rows_or_20_columns_is_cut_and_marked() {
        let row = |columns: usize| (0..columns).map(|column| column.to_string()).collect::<Vec<_>>();
        let table_of = |rows: usize, columns: usize| {
            let cells: Vec<Vec<String>> = (0..rows).map(|_| row(columns)).collect();
            let cells: Vec<Vec<&str>> = cells.iter().map(|row| row.iter().map(String::as_str).collect()).collect();
            let cells: Vec<&[&str]> = cells.iter().map(Vec::as_slice).collect();
            view(docx(&table(&cells)))
        };
        let shape = |view: &DocumentView| match &view.blocks[..] {
            [DocBlock::Table { rows }] => (rows.len(), rows.iter().map(Vec::len).max().unwrap_or(0), view.truncated),
            other => panic!("{other:?}"),
        };
        assert_eq!(shape(&table_of(100, 20)), (100, 20, false));
        assert_eq!(shape(&table_of(101, 1)), (100, 1, true));
        assert_eq!(shape(&table_of(1, 21)), (1, 20, true));
    }

    #[test]
    fn a_paragraph_over_10000_characters_is_cut() {
        let exact = view(docx(&p(&"가".repeat(10_000))));
        assert_eq!((exact.blocks, exact.truncated), (vec![plain(&"가".repeat(10_000))], false));
        let split = format!("<w:p><w:r><w:t>{}</w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>{}</w:t></w:r></w:p>", "a".repeat(6000), "b".repeat(6000));
        let over = view(docx(&split));
        assert_eq!(over.blocks, vec![DocBlock::Paragraph { runs: vec![run(&"a".repeat(6000), false, false), run(&"b".repeat(4000), true, false)] }]);
        assert!(over.truncated);
        let cell = view(docx(&table(&[&[&"c".repeat(10_001)]])));
        assert_eq!(cell.blocks, vec![rows(&[&[&"c".repeat(10_000)]])]);
        assert!(cell.truncated);
    }

    // The zip with the unpacked size its central directory declares for `name` set to `size`.
    fn declaring(mut bytes: Vec<u8>, name: &str, size: u32) -> Vec<u8> {
        let at = (0..bytes.len() - 46)
            .find(|&at| {
                let name_len = usize::from(u16::from_le_bytes([bytes[at + 28], bytes[at + 29]]));
                bytes[at..].starts_with(b"PK\x01\x02") && bytes[at + 46..].starts_with(name.as_bytes()) && name_len == name.len()
            })
            .expect("no central directory entry");
        bytes[at + 24..at + 28].copy_from_slice(&size.to_le_bytes());
        bytes
    }

    #[test]
    fn a_part_declaring_over_100_mb_is_refused() {
        let bytes = docx(&p("x"));
        assert!(read_docx(declaring(bytes.clone(), "word/document.xml", 100 * 1024 * 1024)).is_ok());
        let message = failure(declaring(bytes, "word/document.xml", 100 * 1024 * 1024 + 1));
        assert!(message.contains("too much"), "{message}");
    }

    #[test]
    fn a_docx_without_document_xml_fails_with_text() {
        let message = failure(zip_of(&[("word/other.xml", "<x/>".to_string())]));
        assert!(message.contains("document.xml"), "{message}");
    }

    #[test]
    fn malformed_xml_fails_with_text_and_does_not_panic() {
        for body in ["<w:p><w:r><w:t>x</w:r></w:p>", "<w:p><w:r><w:t>x", "<w:p <<", "</w:tbl></w:tbl></w:tc><w:p>"] {
            assert!(!failure(docx(body)).is_empty(), "{body}");
        }
    }

    fn read_file(name: &str, bytes: &[u8]) -> Result<FileContent, FilesError> {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(name), bytes).unwrap();
        LocalFiles::new().read(dir.path(), name)
    }

    #[test]
    fn a_non_zip_file_named_docx_fails_with_text() {
        let Err(FilesError::Failed(message)) = read_file("bad.docx", b"just words") else { panic!("did not fail") };
        assert!(!message.is_empty());
    }

    #[test]
    fn a_compound_file_named_docx_fails_with_password_text() {
        // An encrypted document is a compound file that holds an `EncryptedPackage` stream.
        let mut compound = cfb::CompoundFile::create_with_version(cfb::Version::V3, Cursor::new(Vec::new())).unwrap();
        compound.create_stream("/EncryptedPackage").unwrap().write_all(&[0u8; 5000]).unwrap();
        compound.flush().unwrap();
        let bytes = compound.into_inner().into_inner();
        let Err(FilesError::Failed(message)) = read_file("locked.docx", &bytes) else { panic!("did not fail") };
        assert!(message.to_lowercase().contains("password"), "{message}");
    }

    #[test]
    fn an_uppercase_docx_extension_is_read() {
        let Ok(FileContent::Document(view)) = read_file("UP.DOCX", &docx(&p("x"))) else { panic!("not a document") };
        assert_eq!(view.blocks, vec![plain("x")]);
    }
}
