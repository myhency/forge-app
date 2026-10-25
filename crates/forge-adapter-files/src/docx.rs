//! Reads a docx file's body into blocks of text for the Files panel's document view.

use std::collections::HashMap;

use forge_ports::{DocBlock, DocumentView, FileContent, FilesError, Rgb, TableCell, TextRun};
use quick_xml::events::{BytesStart, Event};

use crate::office_zip::{self, attribute, cut, failed, TextBudget};

const MAX_BLOCKS: usize = 2_000;
const MAX_ROWS: usize = 100;
const MAX_COLUMNS: usize = 20;
// All the tables of a file together keep at most this many cells.
const MAX_CELLS: usize = 50_000;
// All the paragraphs, list items and table cells of a file together keep at most this many runs
// (after neighbouring runs with the same format are joined), so the view has a bounded number of
// spans to draw.
const MAX_RUNS: usize = 100_000;
// One paragraph, heading, list item or table cell keeps at most this many characters.
const MAX_TEXT_CHARS: usize = 10_000;
// The whole file (styles, numbering and body together) stops being read after this many XML
// events (see `office_zip::walk`).
const EVENT_LIMIT: usize = 5_000_000;
// Word nests a list 9 deep (`w:ilvl` 0 to 8).
const MAX_LIST_LEVEL: u8 = 8;
// A style's list or colour is looked for this many `w:basedOn` steps up, so a cycle of styles ends.
const MAX_BASED_ON_STEPS: usize = 10;
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

/// The body of a docx file (`word/document.xml`) as blocks. Every part is checked against the zip
/// guards before it is parsed; entities other than the five predefined ones and character
/// references are not resolved, and nothing outside the file's own zip is opened.
pub(crate) fn read_docx(bytes: Vec<u8>) -> Result<FileContent, FilesError> {
    read_docx_with(bytes, EVENT_LIMIT)
}

fn read_docx_with(bytes: Vec<u8>, mut budget: usize) -> Result<FileContent, FilesError> {
    let limit = office_zip::PART_BYTES;
    let mut archive = office_zip::open(&bytes, "This document is password-protected, so it cannot be shown.")?;
    let too_big = || failed("This document unpacks to too much data to show.");
    if office_zip::declares_too_much(&mut archive, limit) {
        return Err(too_big());
    }
    let document = office_zip::read_part(&mut archive, "word/document.xml", limit, too_big)?
        .ok_or_else(|| failed("This file has no word/document.xml, so it is not a Word document."))?;
    // A missing or damaged styles or numbering part only costs its lookups; one over the small
    // part limit fails the file, like the pptx's list of slides.
    let small = office_zip::SMALL_PART_BYTES;
    let styles = office_zip::read_part(&mut archive, "word/styles.xml", small, too_big)?.map(|xml| Styles::of(&xml, &mut budget)).unwrap_or_default();
    let numbering = office_zip::read_part(&mut archive, "word/numbering.xml", small, too_big)?.map(|xml| Numbering::of(&xml, &mut budget)).unwrap_or_default();
    let mut body = Body::new(&styles, &numbering);
    if office_zip::walk(&document, &mut budget, |event| body.event(event))? {
        body.truncated = true;
    }
    Ok(FileContent::Document(DocumentView { blocks: body.blocks, truncated: body.truncated }))
}

// A `w:b` or `w:i` is on unless its `w:val` turns it off.
fn is_on(tag: &BytesStart<'_>) -> bool {
    !attribute(tag, "val").is_some_and(|value| matches!(value.as_str(), "0" | "false" | "off"))
}

// A `w:u` is on unless its `w:val` is `none` or turns it off.
fn is_underlined(tag: &BytesStart<'_>) -> bool {
    is_on(tag) && attribute(tag, "val").as_deref() != Some("none")
}

// The `w:val` of a `w:color` (`auto` and anything but six hex digits is no colour; a `w:themeColor`
// beside it is not resolved).
fn color_of(tag: &BytesStart<'_>) -> Option<Rgb> {
    attribute(tag, "val").as_deref().and_then(Rgb::from_hex)
}

// The `w:fill` of a `w:shd`, the same way.
fn fill_of(tag: &BytesStart<'_>) -> Option<Rgb> {
    attribute(tag, "fill").as_deref().and_then(Rgb::from_hex)
}

// The colour of a `w:highlight` name; `none` and unknown names are no highlight.
fn highlight_of(tag: &BytesStart<'_>) -> Option<Rgb> {
    let rgb = match attribute(tag, "val")?.as_str() {
        "yellow" => Rgb(0xFF, 0xFF, 0x00),
        "green" => Rgb(0x00, 0xFF, 0x00),
        "cyan" => Rgb(0x00, 0xFF, 0xFF),
        "magenta" => Rgb(0xFF, 0x00, 0xFF),
        "blue" => Rgb(0x00, 0x00, 0xFF),
        "red" => Rgb(0xFF, 0x00, 0x00),
        "darkBlue" => Rgb(0x00, 0x00, 0x80),
        "darkCyan" => Rgb(0x00, 0x80, 0x80),
        "darkGreen" => Rgb(0x00, 0x80, 0x00),
        "darkMagenta" => Rgb(0x80, 0x00, 0x80),
        "darkRed" => Rgb(0x80, 0x00, 0x00),
        "darkYellow" => Rgb(0x80, 0x80, 0x00),
        "darkGray" => Rgb(0x80, 0x80, 0x80),
        "lightGray" => Rgb(0xC0, 0xC0, 0xC0),
        "black" => Rgb(0x00, 0x00, 0x00),
        "white" => Rgb(0xFF, 0xFF, 0xFF),
        _ => return None,
    };
    Some(rgb)
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

// A list setting (`w:numPr`) of a paragraph or a style; either part may be left out.
#[derive(Default, Clone)]
struct ListSetting {
    num_id: Option<String>,
    level: Option<u8>,
}

fn list_level(tag: &BytesStart<'_>) -> u8 {
    attribute(tag, "val").and_then(|value| value.parse::<u8>().ok()).unwrap_or(0).min(MAX_LIST_LEVEL)
}

/// What `word/styles.xml` says about paragraph styles: the heading level of each style whose name
/// (not id) is a heading's, since Word in some languages gives the built-in heading styles other
/// ids (Korean Word uses `1`, `2` …), and the list each style carries (python-docx's `List Bullet`
/// and `List Number` put the list in the style, not the paragraph), and the text colour of each
/// style that sets one in its own run properties (`None` for `auto`, which stops the lookup).
#[derive(Default)]
struct Styles {
    headings: HashMap<String, u8>,
    lists: HashMap<String, ListSetting>,
    colors: HashMap<String, Option<Rgb>>,
    based_on: HashMap<String, String>,
}

impl Styles {
    fn of(xml: &[u8], budget: &mut usize) -> Self {
        let mut styles = Self::default();
        let mut style: Option<String> = None;
        let (mut in_properties, mut hidden) = (false, 0usize);
        // In the style's own `w:rPr`, not the one of its paragraph properties or of a table style's
        // conditional part (`w:tblStylePr`).
        let (mut in_run_properties, mut in_conditional) = (false, false);
        let _ = office_zip::walk(xml, budget, |event| {
            if hidden > 0 {
                match event {
                    Event::Start(_) => hidden += 1,
                    Event::End(_) => hidden -= 1,
                    _ => {}
                }
                return true;
            }
            match event {
                Event::Start(tag) if HIDDEN.contains(&tag.local_name().as_ref()) => hidden = 1,
                Event::Start(tag) if tag.local_name().as_ref() == "style" => style = attribute(&tag, "styleId"),
                Event::End(tag) if tag.local_name().as_ref() == "style" => (style, in_properties, in_run_properties, in_conditional) = (None, false, false, false),
                Event::Start(tag) if tag.local_name().as_ref() == "pPr" => in_properties = style.is_some(),
                Event::End(tag) if tag.local_name().as_ref() == "pPr" => in_properties = false,
                Event::Start(tag) if tag.local_name().as_ref() == "rPr" => in_run_properties = style.is_some() && !in_properties && !in_conditional,
                Event::End(tag) if tag.local_name().as_ref() == "rPr" => in_run_properties = false,
                Event::Start(tag) if tag.local_name().as_ref() == "tblStylePr" => in_conditional = true,
                Event::End(tag) if tag.local_name().as_ref() == "tblStylePr" => in_conditional = false,
                Event::Empty(tag) | Event::Start(tag) => {
                    let Some(id) = style.as_ref() else { return true };
                    match tag.local_name().as_ref() {
                        "name" => {
                            if let Some(level) = attribute(&tag, "val").as_deref().and_then(level_of_style) {
                                styles.headings.insert(id.clone(), level);
                            }
                        }
                        "basedOn" => {
                            if let Some(parent) = attribute(&tag, "val") {
                                styles.based_on.insert(id.clone(), parent);
                            }
                        }
                        "numPr" if in_properties => {
                            styles.lists.entry(id.clone()).or_default();
                        }
                        "numId" if in_properties => styles.lists.entry(id.clone()).or_default().num_id = attribute(&tag, "val"),
                        "ilvl" if in_properties => styles.lists.entry(id.clone()).or_default().level = Some(list_level(&tag)),
                        "color" if in_run_properties => {
                            styles.colors.insert(id.clone(), color_of(&tag));
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
            true
        });
        styles
    }

    // The list a style carries, its own settings first and then those of the styles it is based
    // on, at most `MAX_BASED_ON_STEPS` up. `None` when no style in the chain has a list.
    fn list_of(&self, style: &str) -> Option<ListSetting> {
        let mut found: Option<ListSetting> = None;
        let mut current = style;
        for _ in 0..=MAX_BASED_ON_STEPS {
            if let Some(own) = self.lists.get(current) {
                let setting = found.get_or_insert_with(ListSetting::default);
                setting.num_id = setting.num_id.take().or_else(|| own.num_id.clone());
                setting.level = setting.level.or(own.level);
            }
            let Some(parent) = self.based_on.get(current) else { break };
            current = parent;
        }
        found
    }

    // The text colour a style gives, its own first and then that of the styles it is based on, at
    // most `MAX_BASED_ON_STEPS` up. A style's own `auto` ends the lookup with no colour.
    fn color_of(&self, style: &str) -> Option<Rgb> {
        let mut current = style;
        for _ in 0..=MAX_BASED_ON_STEPS {
            if let Some(own) = self.colors.get(current) {
                return *own;
            }
            current = self.based_on.get(current)?;
        }
        None
    }
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
    fn of(xml: &[u8], budget: &mut usize) -> Self {
        let mut numbering = Self::default();
        let (mut abstract_id, mut num_id, mut level): (Option<String>, Option<String>, Option<u8>) = (None, None, None);
        let _ = office_zip::walk(xml, budget, |event| {
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

// The properties of the open run, as its `w:rPr` sets them.
#[derive(Default, Clone, Copy)]
struct RunFormat {
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    double_strike: bool,
    color: Option<Rgb>,
    highlight: Option<Rgb>,
    shading: Option<Rgb>,
}

impl RunFormat {
    // An empty run of this format; the shading shows where there is no named highlight.
    fn run(self) -> TextRun {
        TextRun {
            text: String::new(),
            bold: self.bold,
            italic: self.italic,
            underline: self.underline,
            strike: self.strike || self.double_strike,
            color: self.color,
            highlight: self.highlight.or(self.shading),
        }
    }
}

// Whether two runs look the same, so their text can be one run.
fn same_format(a: &TextRun, b: &TextRun) -> bool {
    (a.bold, a.italic, a.underline, a.strike, a.color, a.highlight) == (b.bold, b.italic, b.underline, b.strike, b.color, b.highlight)
}

// Whether `run` would start a new run after `runs` instead of joining the last one.
fn starts_a_run(runs: &[TextRun], run: &TextRun) -> bool {
    !runs.last().is_some_and(|last| same_format(last, run))
}

// Adds `run` to `runs`, joined to the last run when they look the same. A new run takes one of
// `runs_left`; when none is left the run is dropped and the result is false.
fn push_run(runs: &mut Vec<TextRun>, run: TextRun, runs_left: &mut usize) -> bool {
    match runs.last_mut() {
        Some(last) if same_format(last, &run) => last.text.push_str(&run.text),
        _ if *runs_left == 0 => return false,
        _ => {
            *runs_left -= 1;
            runs.push(run);
        }
    }
    true
}

#[derive(Default)]
struct Paragraph {
    style: Option<String>,
    // The paragraph's own `w:numPr`, which wins over the one of its style.
    list: Option<ListSetting>,
    runs: Vec<TextRun>,
    chars: usize,
}

/// The state of a walk through `word/document.xml`. Only the outermost table has rows and cells;
/// the text of a nested table goes into the outer cell it sits in.
struct Body<'a> {
    styles: &'a Styles,
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
    format: RunFormat,
    table_depth: usize,
    rows: Vec<Vec<TableCell>>,
    in_row: bool,
    // The open cell of the outermost table and its character count.
    cell: Option<(TableCell, usize)>,
    in_cell_properties: bool,
    // The cells all tables may still keep.
    cells_left: usize,
    // The runs all blocks may still keep.
    runs_left: usize,
    text: TextBudget,
}

impl<'a> Body<'a> {
    fn new(styles: &'a Styles, numbering: &'a Numbering) -> Self {
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
            format: RunFormat::default(),
            table_depth: 0,
            rows: Vec::new(),
            in_row: false,
            cell: None,
            in_cell_properties: false,
            cells_left: MAX_CELLS,
            runs_left: MAX_RUNS,
            text: TextBudget::new(office_zip::TOTAL_TEXT_BYTES),
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
            // Text inside run properties is not text (`in_text` is never set there, and properties
            // opened inside a `w:t` stop its text).
            Event::Text(text) if self.in_text && !self.in_run_properties => self.add_text(&text.xml10_content()),
            Event::CData(text) if self.in_text && !self.in_run_properties => self.add_text(&text.xml10_content()),
            Event::GeneralRef(reference) if self.in_text && !self.in_run_properties => {
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
            "numPr" if in_properties => self.with_paragraph(|paragraph| {
                paragraph.list.get_or_insert_with(ListSetting::default);
            }),
            "ilvl" if in_properties => self.with_paragraph(|paragraph| paragraph.list.get_or_insert_with(ListSetting::default).level = Some(list_level(tag))),
            "numId" if in_properties => self.with_paragraph(|paragraph| paragraph.list.get_or_insert_with(ListSetting::default).num_id = attribute(tag, "val")),
            "r" if !empty => {
                self.in_run = true;
                self.format = RunFormat::default();
            }
            // Properties opened inside a `w:t` stop its text but are not the run's properties.
            "rPr" if !empty && self.in_run => self.in_run_properties = true,
            "b" if self.in_run_properties && !self.in_text => self.format.bold = is_on(tag),
            "i" if self.in_run_properties && !self.in_text => self.format.italic = is_on(tag),
            "u" if self.in_run_properties && !self.in_text => self.format.underline = is_underlined(tag),
            "strike" if self.in_run_properties && !self.in_text => self.format.strike = is_on(tag),
            "dstrike" if self.in_run_properties && !self.in_text => self.format.double_strike = is_on(tag),
            "color" if self.in_run_properties && !self.in_text => self.format.color = color_of(tag),
            "highlight" if self.in_run_properties && !self.in_text => self.format.highlight = highlight_of(tag),
            "shd" if self.in_run_properties && !self.in_text => self.format.shading = fill_of(tag),
            // The cell's own properties, not those of a nested table's cell.
            "tcPr" if !empty && self.table_depth == 1 && self.cell.is_some() => self.in_cell_properties = true,
            "shd" if self.in_cell_properties => {
                if let Some((cell, _)) = self.cell.as_mut() {
                    cell.fill = fill_of(tag);
                }
            }
            "t" if !empty && self.in_run && !self.in_run_properties => self.in_text = true,
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
                if self.rows.last().is_some_and(|row| row.len() >= MAX_COLUMNS) || self.cells_left == 0 {
                    self.truncated = true;
                    return;
                }
                self.cells_left -= 1;
                if empty {
                    if let Some(row) = self.rows.last_mut() {
                        row.push(TableCell::default());
                    }
                } else {
                    self.cell = Some((TableCell::default(), 0));
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
            "tcPr" => self.in_cell_properties = false,
            "tc" if self.table_depth == 1 => {
                self.in_cell_properties = false;
                if let (Some((cell, _)), Some(row)) = (self.cell.take(), self.rows.last_mut()) {
                    row.push(cell);
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

    // Adds text to the open paragraph with the open run's format, up to the paragraph's text cap,
    // the file's text budget and the file's run budget.
    fn add_text(&mut self, text: &str) {
        let Some(paragraph) = self.paragraph.as_mut() else { return };
        let run = self.format.run();
        if self.runs_left == 0 && starts_a_run(&paragraph.runs, &run) {
            self.truncated = true;
            return;
        }
        let (kept, was_cut) = cut(text, MAX_TEXT_CHARS - paragraph.chars);
        let (kept, over_budget) = self.text.take(kept);
        self.truncated |= was_cut | over_budget;
        if kept.is_empty() {
            return;
        }
        paragraph.chars += kept.chars().count();
        push_run(&mut paragraph.runs, TextRun { text: kept.to_string(), ..run }, &mut self.runs_left);
    }

    // A paragraph with text becomes a block, or inside a table joins its cell's runs after a plain
    // space.
    fn end_paragraph(&mut self) -> bool {
        self.in_properties = false;
        let Some(paragraph) = self.paragraph.take() else { return true };
        let text: String = paragraph.runs.iter().map(|run| run.text.as_str()).collect();
        let heading = paragraph.style.as_deref().and_then(|style| level_of_style(style).or_else(|| self.styles.headings.get(style).copied()));
        // The runs of a paragraph that is not kept as runs go back to the budget: a cell takes them
        // again as it joins them, a heading keeps only its text.
        let kept_as_runs = !text.trim().is_empty() && self.table_depth == 0 && heading.is_none();
        if !kept_as_runs {
            self.runs_left += paragraph.runs.len();
        }
        if text.trim().is_empty() {
            return true;
        }
        if self.table_depth > 0 {
            if let Some((cell, chars)) = self.cell.as_mut() {
                // The text is already counted in the budget; the joining space is not.
                if !cell.runs.is_empty() && self.text.take(" ").1 {
                    self.truncated = true;
                    return true;
                }
                let space = (!cell.runs.is_empty()).then(|| TextRun { text: " ".to_string(), ..TextRun::default() });
                for run in space.into_iter().chain(paragraph.runs) {
                    let (kept, was_cut) = cut(&run.text, MAX_TEXT_CHARS - *chars);
                    self.truncated |= was_cut;
                    if !kept.is_empty() {
                        if !push_run(&mut cell.runs, TextRun { text: kept.to_string(), ..run }, &mut self.runs_left) {
                            self.truncated = true;
                            break;
                        }
                        *chars += kept.chars().count();
                    }
                    if was_cut {
                        break;
                    }
                }
            }
            return true;
        }
        // The paragraph's own list settings win; what it leaves out comes from its style.
        let from_style = paragraph.style.as_deref().and_then(|style| self.styles.list_of(style));
        let list = match (paragraph.list, from_style) {
            (Some(own), Some(style)) => Some(ListSetting { num_id: own.num_id.or(style.num_id), level: own.level.or(style.level) }),
            (own, style) => own.or(style),
        };
        let block = if let Some(level) = heading {
            let color = paragraph.style.as_deref().and_then(|style| self.styles.color_of(style));
            DocBlock::Heading { level, text, color }
        } else if let Some(list) = list.filter(|list| list.num_id.as_deref() != Some("0")) {
            let level = list.level.unwrap_or(0);
            let numbered = list.num_id.as_deref().is_some_and(|id| self.numbering.numbered(id, level));
            DocBlock::ListItem { level, numbered, runs: paragraph.runs }
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

    use forge_ports::{DocBlock, DocumentView, FilesPort, Rgb, TableCell, TextRun};

    use super::*;
    use crate::office_zip::test_support::{declaring, zip_of};
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
        TextRun { text: text.to_string(), bold, italic, ..TextRun::default() }
    }

    fn plain(text: &str) -> DocBlock {
        DocBlock::Paragraph { runs: vec![run(text, false, false)] }
    }

    fn heading(level: u8, text: &str) -> DocBlock {
        DocBlock::Heading { level, text: text.to_string(), color: None }
    }

    fn colored_heading(level: u8, text: &str, color: Rgb) -> DocBlock {
        DocBlock::Heading { level, text: text.to_string(), color: Some(color) }
    }

    // A paragraph of one run with these run properties.
    fn formatted(properties: &str, text: &str) -> String {
        format!("<w:p><w:r><w:rPr>{properties}</w:rPr><w:t>{text}</w:t></w:r></w:p>")
    }

    // The runs of the only paragraph of a body.
    fn runs_of(body: &str) -> Vec<TextRun> {
        match &view(docx(body)).blocks[..] {
            [DocBlock::Paragraph { runs }] => runs.clone(),
            other => panic!("not one paragraph: {other:?}"),
        }
    }

    const RED: Rgb = Rgb(0xFF, 0, 0);

    #[test]
    fn a_run_colour_is_kept_and_auto_is_none() {
        let body = r#"<w:p><w:r><w:rPr><w:color w:val="FF0000"/></w:rPr><w:t>red</w:t></w:r><w:r><w:rPr><w:color w:val="auto"/></w:rPr><w:t xml:space="preserve"> auto </w:t></w:r><w:r><w:rPr><w:color w:val="00aa00" w:themeColor="accent6"/></w:rPr><w:t>green</w:t></w:r><w:r><w:rPr><w:color w:themeColor="accent1"/></w:rPr><w:t xml:space="preserve"> theme</w:t></w:r></w:p>"#;
        assert_eq!(
            runs_of(body),
            vec![
                TextRun { color: Some(RED), ..run("red", false, false) },
                run(" auto ", false, false),
                TextRun { color: Some(Rgb(0, 0xAA, 0)), ..run("green", false, false) },
                run(" theme", false, false),
            ]
        );
        // A colour on the paragraph mark is not the text's colour.
        assert_eq!(view(docx(r#"<w:p><w:pPr><w:rPr><w:color w:val="FF0000"/></w:rPr></w:pPr><w:r><w:t>x</w:t></w:r></w:p>"#)).blocks, vec![plain("x")]);
    }

    #[test]
    fn a_hostile_colour_value_is_none() {
        for value in ["FF0000;background:url(x)", "FF00000", "FF00", "", "GG0000", "#FF0000", " FF0000", "FF0000&quot;", "\u{ff26}\u{ff26}0000"] {
            for properties in [format!(r#"<w:color w:val="{value}"/>"#), format!(r#"<w:shd w:val="clear" w:fill="{value}"/>"#)] {
                assert_eq!(runs_of(&formatted(&properties, "x")), vec![run("x", false, false)], "{properties}");
            }
            let cell = format!(r#"<w:tbl><w:tr><w:tc><w:tcPr><w:shd w:fill="{value}"/></w:tcPr>{}</w:tc></w:tr></w:tbl>"#, p("x"));
            assert_eq!(view(docx(&cell)).blocks, vec![rows(&[&["x"]])], "{value}");
        }
    }

    #[test]
    fn highlight_names_map_to_colours_and_none_means_none() {
        let names = [
            ("yellow", Rgb(0xFF, 0xFF, 0)),
            ("green", Rgb(0, 0xFF, 0)),
            ("cyan", Rgb(0, 0xFF, 0xFF)),
            ("magenta", Rgb(0xFF, 0, 0xFF)),
            ("blue", Rgb(0, 0, 0xFF)),
            ("red", Rgb(0xFF, 0, 0)),
            ("darkBlue", Rgb(0, 0, 0x80)),
            ("darkCyan", Rgb(0, 0x80, 0x80)),
            ("darkGreen", Rgb(0, 0x80, 0)),
            ("darkMagenta", Rgb(0x80, 0, 0x80)),
            ("darkRed", Rgb(0x80, 0, 0)),
            ("darkYellow", Rgb(0x80, 0x80, 0)),
            ("darkGray", Rgb(0x80, 0x80, 0x80)),
            ("lightGray", Rgb(0xC0, 0xC0, 0xC0)),
            ("black", Rgb(0, 0, 0)),
            ("white", Rgb(0xFF, 0xFF, 0xFF)),
        ];
        for (name, color) in names {
            assert_eq!(runs_of(&formatted(&format!(r#"<w:highlight w:val="{name}"/>"#), "x")), vec![TextRun { highlight: Some(color), ..run("x", false, false) }], "{name}");
        }
        for value in ["none", "purple", "YELLOW", "FFFF00", ""] {
            assert_eq!(runs_of(&formatted(&format!(r#"<w:highlight w:val="{value}"/>"#), "x")), vec![run("x", false, false)], "{value}");
        }
    }

    #[test]
    fn a_run_shading_fill_counts_as_highlight_when_there_is_no_highlight() {
        let shaded = TextRun { highlight: Some(Rgb(0xD9, 0xD9, 0xD9)), ..run("x", false, false) };
        assert_eq!(runs_of(&formatted(r#"<w:shd w:val="clear" w:color="auto" w:fill="D9D9D9"/>"#, "x")), vec![shaded.clone()]);
        // A named highlight wins over the shading, in either order.
        let yellow = TextRun { highlight: Some(Rgb(0xFF, 0xFF, 0)), ..run("x", false, false) };
        assert_eq!(runs_of(&formatted(r#"<w:highlight w:val="yellow"/><w:shd w:fill="D9D9D9"/>"#, "x")), vec![yellow.clone()]);
        assert_eq!(runs_of(&formatted(r#"<w:shd w:fill="D9D9D9"/><w:highlight w:val="yellow"/>"#, "x")), vec![yellow]);
        // A highlight of `none` is no highlight, so the shading shows, as Word draws it.
        assert_eq!(runs_of(&formatted(r#"<w:highlight w:val="none"/><w:shd w:fill="D9D9D9"/>"#, "x")), vec![shaded]);
        assert_eq!(runs_of(&formatted(r#"<w:shd w:val="clear" w:fill="auto"/>"#, "x")), vec![run("x", false, false)]);
    }

    #[test]
    fn underline_and_strike_are_on_unless_val_is_none_zero_or_false() {
        let underlined = TextRun { underline: true, ..run("x", false, false) };
        let struck = TextRun { strike: true, ..run("x", false, false) };
        for properties in [r#"<w:u w:val="single"/>"#, r#"<w:u w:val="double"/>"#, "<w:u/>"] {
            assert_eq!(runs_of(&formatted(properties, "x")), vec![underlined.clone()], "{properties}");
        }
        for properties in ["<w:strike/>", r#"<w:strike w:val="1"/>"#, r#"<w:strike w:val="true"/>"#, "<w:dstrike/>", r#"<w:strike w:val="0"/><w:dstrike/>"#] {
            assert_eq!(runs_of(&formatted(properties, "x")), vec![struck.clone()], "{properties}");
        }
        for properties in [r#"<w:u w:val="none"/>"#, r#"<w:u w:val="0"/>"#, r#"<w:u w:val="false"/>"#, r#"<w:strike w:val="0"/>"#, r#"<w:strike w:val="false"/>"#, r#"<w:dstrike w:val="0"/>"#, r#"<w:dstrike w:val="false"/>"#] {
            assert_eq!(runs_of(&formatted(properties, "x")), vec![run("x", false, false)], "{properties}");
        }
        // Each run starts plain again.
        let body = r#"<w:p><w:r><w:rPr><w:u w:val="single"/><w:strike/><w:color w:val="FF0000"/><w:highlight w:val="yellow"/></w:rPr><w:t>all</w:t></w:r><w:r><w:t>none</w:t></w:r></w:p>"#;
        assert_eq!(
            runs_of(body),
            vec![TextRun { underline: true, strike: true, color: Some(RED), highlight: Some(Rgb(0xFF, 0xFF, 0)), ..run("all", false, false) }, run("none", false, false)]
        );
    }

    // A paragraph style with this id, name, `basedOn` and run properties.
    fn run_style(id: &str, name: &str, based_on: Option<&str>, run_properties: &str) -> String {
        let based_on = based_on.map(|parent| format!(r#"<w:basedOn w:val="{parent}"/>"#)).unwrap_or_default();
        format!(r#"<w:style w:type="paragraph" w:styleId="{id}"><w:name w:val="{name}"/>{based_on}<w:rPr>{run_properties}</w:rPr></w:style>"#)
    }

    fn color(hex: &str) -> String {
        format!(r#"<w:color w:val="{hex}"/>"#)
    }

    #[test]
    fn a_heading_style_colour_is_the_headings_colour_following_basedon() {
        let blue = Rgb(0x2F, 0x54, 0x96);
        let styles = [
            run_style("HeadingBase", "Heading Base", None, &color("2F5496")),
            run_style("Heading1", "heading 1", Some("HeadingBase"), "<w:b/>"),
            run_style("Heading2", "heading 2", Some("HeadingBase"), &color("1F3864")),
            run_style("Heading3", "heading 3", Some("HeadingBase"), &color("auto")),
            // Korean Word's id for `heading 1`.
            run_style("1", "heading 1", Some("HeadingBase"), ""),
            // A colour inside the style's paragraph properties or a table style's conditional part is not the style's.
            r#"<w:style w:type="paragraph" w:styleId="Heading4"><w:name w:val="heading 4"/><w:pPr><w:rPr><w:color w:val="FF0000"/></w:rPr></w:pPr></w:style>"#.to_string(),
            r#"<w:style w:type="paragraph" w:styleId="Heading5"><w:name w:val="heading 5"/><w:tblStylePr w:type="firstRow"><w:rPr><w:color w:val="FF0000"/></w:rPr></w:tblStylePr></w:style>"#.to_string(),
        ]
        .concat();
        // The heading's own direct colour stays on its runs, not on the heading.
        let direct = r#"<w:p><w:pPr><w:pStyle w:val="Heading6"/></w:pPr><w:r><w:rPr><w:color w:val="FF0000"/></w:rPr><w:t>six</w:t></w:r></w:p>"#;
        let body = [styled("Heading1", "one"), styled("Heading2", "two"), styled("Heading3", "three"), styled("1", "korean"), styled("Heading4", "four"), styled("Heading5", "five"), direct.to_string()].concat();
        assert_eq!(
            view(docx_with(&body, &[("word/styles.xml", styles_of(&styles))])).blocks,
            vec![
                colored_heading(1, "one", blue),
                colored_heading(2, "two", Rgb(0x1F, 0x38, 0x64)),
                // Its own `auto` stops the colour of the style it is based on.
                heading(3, "three"),
                colored_heading(1, "korean", blue),
                heading(4, "four"),
                heading(5, "five"),
                heading(6, "six"),
            ]
        );
    }

    #[test]
    fn a_heading_style_with_themecolor_and_a_hex_val_uses_the_hex() {
        let styles = [
            run_style("Heading1", "heading 1", None, r#"<w:color w:val="2F5496" w:themeColor="accent1" w:themeShade="BF"/>"#),
            run_style("Heading2", "heading 2", None, r#"<w:color w:themeColor="accent1"/>"#),
        ]
        .concat();
        let body = [styled("Heading1", "one"), styled("Heading2", "two")].concat();
        assert_eq!(view(docx_with(&body, &[("word/styles.xml", styles_of(&styles))])).blocks, vec![colored_heading(1, "one", Rgb(0x2F, 0x54, 0x96)), heading(2, "two")]);
    }

    #[test]
    fn a_basedon_cycle_in_heading_colour_terminates() {
        let styles = [
            run_style("Heading1", "heading 1", Some("Heading2"), ""),
            run_style("Heading2", "heading 2", Some("Heading1"), ""),
            run_style("Heading3", "heading 3", Some("Heading3"), ""),
        ]
        .concat();
        let long_chain: String = (0..30).map(|index| run_style(&format!("S{index}"), &format!("heading {}", if index == 0 { 1 } else { 9 }), Some(&format!("S{}", index + 1)), "")).collect();
        let colored_end = run_style("S30", "S30", None, &color("FF0000"));
        let near = run_style("Near", "heading 2", Some("S25"), "");
        let body = [styled("Heading1", "one"), styled("Heading3", "three"), styled("S0", "far"), styled("Near", "near")].concat();
        assert_eq!(
            view(docx_with(&body, &[("word/styles.xml", styles_of(&[styles, long_chain, colored_end, near].concat()))])).blocks,
            // A chain longer than the bound stops looking; one within it finds the colour.
            vec![heading(1, "one"), heading(3, "three"), heading(1, "far"), colored_heading(2, "near", RED)]
        );
    }

    #[test]
    fn a_table_cell_keeps_its_runs_with_their_formatting_and_its_fill() {
        let white_bold = r#"<w:p><w:r><w:rPr><w:b/><w:color w:val="FFFFFF"/></w:rPr><w:t>Head</w:t></w:r></w:p>"#;
        let red = r#"<w:p><w:pPr><w:shd w:fill="00FF00"/></w:pPr><w:r><w:rPr><w:color w:val="FF0000"/><w:u w:val="single"/></w:rPr><w:t>y</w:t></w:r></w:p>"#;
        let inner = format!(r#"<w:tbl><w:tr><w:tc><w:tcPr><w:shd w:fill="00FF00"/></w:tcPr><w:p><w:r><w:rPr><w:strike/></w:rPr><w:t>in</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#);
        let body = format!(r#"<w:tbl><w:tr><w:tc><w:tcPr><w:tcW w:w="100"/><w:shd w:val="clear" w:color="auto" w:fill="1F3864"/></w:tcPr>{white_bold}</w:tc><w:tc>{}{red}{inner}</w:tc></w:tr></w:tbl>"#, p("x"));
        let plain_run = |text: &str| run(text, false, false);
        assert_eq!(
            view(docx(&body)).blocks,
            vec![DocBlock::Table {
                rows: vec![vec![
                    TableCell { runs: vec![TextRun { color: Some(Rgb(0xFF, 0xFF, 0xFF)), ..run("Head", true, false) }], fill: Some(Rgb(0x1F, 0x38, 0x64)) },
                    // A paragraph's or a nested cell's shading is not the cell's fill.
                    TableCell {
                        runs: vec![plain_run("x "), TextRun { color: Some(RED), underline: true, ..plain_run("y") }, plain_run(" "), TextRun { strike: true, ..plain_run("in") }],
                        fill: None,
                    },
                ]],
            }]
        );
    }

    #[test]
    fn a_cell_fill_auto_is_none() {
        let body = format!(r#"<w:tbl><w:tr><w:tc><w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="auto"/></w:tcPr>{}</w:tc><w:tc><w:tcPr><w:shd w:fill="ffff99"/></w:tcPr>{}</w:tc></w:tr></w:tbl>"#, p("a"), p("b"));
        assert_eq!(
            view(docx(&body)).blocks,
            vec![DocBlock::Table { rows: vec![vec![TableCell { runs: vec![run("a", false, false)], fill: None }, TableCell { runs: vec![run("b", false, false)], fill: Some(Rgb(0xFF, 0xFF, 0x99)) }]] }]
        );
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

    // A numbering part where list 1 is bulleted and list 2 numbered, at every level used here.
    fn two_lists() -> String {
        format!(
            r#"<?xml version="1.0"?><w:numbering {W}><w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:numFmt w:val="bullet"/></w:lvl><w:lvl w:ilvl="1"><w:numFmt w:val="bullet"/></w:lvl></w:abstractNum><w:abstractNum w:abstractNumId="1"><w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl><w:lvl w:ilvl="1"><w:numFmt w:val="decimal"/></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num><w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num></w:numbering>"#
        )
    }

    fn styles_of(styles: &str) -> String {
        format!(r#"<?xml version="1.0"?><w:styles {W}>{styles}</w:styles>"#)
    }

    // A paragraph style with this id, `basedOn` and paragraph properties.
    fn style(id: &str, based_on: Option<&str>, properties: &str) -> String {
        let based_on = based_on.map(|parent| format!(r#"<w:basedOn w:val="{parent}"/>"#)).unwrap_or_default();
        format!(r#"<w:style w:type="paragraph" w:styleId="{id}"><w:name w:val="{id}"/>{based_on}<w:pPr>{properties}</w:pPr></w:style>"#)
    }

    fn with_styles(styles: &str, body: &str) -> Vec<DocBlock> {
        view(docx_with(body, &[("word/styles.xml", styles_of(styles)), ("word/numbering.xml", two_lists())])).blocks
    }

    #[test]
    fn a_paragraph_whose_style_has_numpr_is_a_list_item() {
        // python-docx's `List Bullet` and `List Number` styles carry the list; the paragraph does not.
        let styles = [style("ListBullet", None, r#"<w:numPr><w:numId w:val="1"/></w:numPr>"#), style("ListNumber", None, r#"<w:numPr><w:ilvl w:val="1"/><w:numId w:val="2"/></w:numPr>"#)].concat();
        let own_level = r#"<w:p><w:pPr><w:pStyle w:val="ListNumber"/><w:numPr><w:ilvl w:val="0"/></w:numPr></w:pPr><w:r><w:t>own level</w:t></w:r></w:p>"#;
        let own_list = r#"<w:p><w:pPr><w:pStyle w:val="ListNumber"/><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>own list</w:t></w:r></w:p>"#;
        let body = [styled("ListBullet", "dot"), styled("ListNumber", "one"), own_level.to_string(), own_list.to_string()].concat();
        assert_eq!(with_styles(&styles, &body), vec![item(0, false, "dot"), item(1, true, "one"), item(0, true, "own level"), item(0, false, "own list")]);
    }

    #[test]
    fn a_style_based_on_a_list_style_inherits_the_list() {
        let styles = [
            style("ListNumber", None, r#"<w:numPr><w:numId w:val="2"/></w:numPr>"#),
            style("Child", Some("ListNumber"), r#"<w:numPr><w:ilvl w:val="1"/></w:numPr>"#),
            style("Grandchild", Some("Child"), ""),
        ]
        .concat();
        let body = [styled("Child", "child"), styled("Grandchild", "grandchild")].concat();
        assert_eq!(with_styles(&styles, &body), vec![item(1, true, "child"), item(1, true, "grandchild")]);
    }

    #[test]
    fn numid_zero_turns_a_style_list_off() {
        let styles = [style("ListBullet", None, r#"<w:numPr><w:numId w:val="1"/></w:numPr>"#), style("NotListed", Some("ListBullet"), r#"<w:numPr><w:numId w:val="0"/></w:numPr>"#)].concat();
        let off_here = r#"<w:p><w:pPr><w:pStyle w:val="ListBullet"/><w:numPr><w:numId w:val="0"/></w:numPr></w:pPr><w:r><w:t>off here</w:t></w:r></w:p>"#;
        let body = [styled("NotListed", "off in style"), off_here.to_string()].concat();
        assert_eq!(with_styles(&styles, &body), vec![plain("off in style"), plain("off here")]);
    }

    #[test]
    fn a_basedon_cycle_terminates() {
        let styles = [style("A", Some("B"), ""), style("B", Some("A"), ""), style("Self", Some("Self"), "")].concat();
        let long_chain: String = (0..30).map(|index| style(&format!("S{index}"), Some(&format!("S{}", index + 1)), "")).collect();
        let listed_end = style("S30", None, r#"<w:numPr><w:numId w:val="1"/></w:numPr>"#);
        let body = [styled("A", "a"), styled("Self", "self"), styled("S0", "far")].concat();
        // A chain longer than the bound stops looking, so the far list is not found.
        assert_eq!(with_styles(&[styles, long_chain, listed_end].concat(), &body), vec![plain("a"), plain("self"), plain("far")]);
    }

    fn rows(cells: &[&[&str]]) -> DocBlock {
        let cell = |text: &str| TableCell { runs: if text.is_empty() { vec![] } else { vec![run(text, false, false)] }, fill: None };
        DocBlock::Table { rows: cells.iter().map(|row| row.iter().map(|text| cell(text)).collect()).collect() }
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

    #[test]
    fn a_part_declaring_over_100_mb_is_refused() {
        let bytes = docx(&p("x"));
        assert!(read_docx(declaring(bytes.clone(), "word/document.xml", 100 * 1024 * 1024)).is_ok());
        let message = failure(declaring(bytes, "word/document.xml", 100 * 1024 * 1024 + 1));
        assert!(message.contains("too much"), "{message}");
    }

    #[test]
    fn all_tables_together_keep_at_most_50000_cells() {
        // 26 full tables of 100 x 20 empty cells: no table is over its own cap.
        let full_table = format!("<w:tbl>{}</w:tbl>", format!("<w:tr>{}</w:tr>", "<w:tc/>".repeat(20)).repeat(100));
        let all = view(docx(&full_table.repeat(26)));
        let cells: usize = all.blocks.iter().map(|block| match block {
            DocBlock::Table { rows } => rows.iter().map(Vec::len).sum(),
            _ => 0,
        }).sum();
        assert_eq!(cells, 50_000);
        assert!(all.truncated);
        assert!(!view(docx(&full_table.repeat(25))).truncated);
    }

    // `count` one-character runs that alternate between red and plain, so no two neighbours join.
    fn alternating_runs(count: usize) -> String {
        (0..count).map(|index| if index % 2 == 0 { r#"<w:r><w:rPr><w:color w:val="FF0000"/></w:rPr><w:t>a</w:t></w:r>"# } else { "<w:r><w:t>b</w:t></w:r>" }).collect()
    }

    fn run_count(view: &DocumentView) -> usize {
        view.blocks
            .iter()
            .map(|block| match block {
                DocBlock::Heading { .. } => 0,
                DocBlock::Paragraph { runs } | DocBlock::ListItem { runs, .. } => runs.len(),
                DocBlock::Table { rows } => rows.iter().flatten().map(|cell| cell.runs.len()).sum(),
            })
            .sum()
    }

    #[test]
    fn runs_are_capped_at_100000_per_document_and_marked_truncated() {
        // 120 paragraphs of 1,000 runs: 120,000 runs, each paragraph under its own caps.
        let paragraph = format!("<w:p>{}</w:p>", alternating_runs(1000));
        let over = view(docx(&paragraph.repeat(120)));
        assert_eq!(run_count(&over), 100_000);
        assert!(over.truncated);
        // The text of the dropped runs is dropped too.
        let text: usize = over.blocks.iter().map(|block| match block {
            DocBlock::Paragraph { runs } => runs.iter().map(|run| run.text.len()).sum(),
            _ => 0,
        }).sum();
        assert_eq!(text, 100_000);
        let exact = view(docx(&paragraph.repeat(100)));
        assert_eq!((run_count(&exact), exact.truncated), (100_000, false));
    }

    #[test]
    fn the_run_cap_also_covers_table_cells() {
        // 6 rows of 20 cells of 1,000 runs: 120,000 runs in 120 cells.
        let cell = format!("<w:tc><w:p>{}</w:p></w:tc>", alternating_runs(1000));
        let body = format!("<w:tbl>{}</w:tbl>", format!("<w:tr>{}</w:tr>", cell.repeat(20)).repeat(6));
        let over = view(docx(&body));
        assert_eq!(run_count(&over), 100_000);
        assert!(over.truncated);
        // Runs a paragraph holds for a moment before it joins its cell are not counted twice.
        let fits = view(docx(&format!("<w:tbl>{}</w:tbl>", format!("<w:tr>{}</w:tr>", cell.repeat(20)).repeat(5))));
        assert_eq!((run_count(&fits), fits.truncated), (100_000, false));
    }

    #[test]
    fn text_inside_rpr_is_ignored() {
        // A `w:t` inside the properties is not text; the properties after it still count.
        let inside = r#"<w:p><w:r><w:rPr><w:t>hidden</w:t><w:color w:val="FF0000"/></w:rPr><w:t>shown</w:t></w:r></w:p>"#;
        assert_eq!(runs_of(inside), vec![TextRun { color: Some(RED), ..run("shown", false, false) }]);
        // Properties inside a `w:t` change nothing, and their own text is not text.
        let toggles = r#"<w:p><w:r><w:t>a<w:rPr><w:color w:val="FF0000"/><w:b/>x</w:rPr>b<w:color w:val="00FF00"/>c</w:t></w:r></w:p>"#;
        assert_eq!(runs_of(toggles), vec![run("abc", false, false)]);
        // The probe's shape: many toggles inside one `w:t` inside `w:rPr` give no runs at all.
        let probe = format!("<w:p><w:r><w:rPr><w:t>{}</w:t></w:rPr></w:r></w:p>{}", r#"<w:color w:val="FF0000"/>a<w:color w:val="00FF00"/>b"#.repeat(1000), p("end"));
        let view = view(docx(&probe));
        assert_eq!((view.blocks, view.truncated), (vec![plain("end")], false));
    }

    #[test]
    fn an_ordinary_document_is_not_truncated_by_the_run_cap() {
        let paragraph = format!("<w:p>{}</w:p>", alternating_runs(5));
        let view = view(docx(&paragraph.repeat(2000)));
        assert_eq!((view.blocks.len(), run_count(&view), view.truncated), (2000, 10_000, false));
    }

    fn text_bytes(view: &DocumentView) -> usize {
        view.blocks
            .iter()
            .map(|block| match block {
                DocBlock::Heading { text, .. } => text.len(),
                DocBlock::Paragraph { runs } | DocBlock::ListItem { runs, .. } => runs.iter().map(|run| run.text.len()).sum(),
                DocBlock::Table { rows } => rows.iter().flatten().flat_map(|cell| &cell.runs).map(|run| run.text.len()).sum(),
            })
            .sum()
    }

    #[test]
    fn all_text_together_keeps_at_most_8_mb() {
        let paragraph = p(&"a".repeat(10_000));
        let cell = table(&[&[&"b".repeat(10_000)]]);
        let over = view(docx(&[paragraph.repeat(800), cell.repeat(100)].concat()));
        assert!(text_bytes(&over) <= office_zip::TOTAL_TEXT_BYTES, "{}", text_bytes(&over));
        assert!(text_bytes(&over) > office_zip::TOTAL_TEXT_BYTES - 10_000);
        assert!(over.truncated);
        let fits = view(docx(&paragraph.repeat(800)));
        assert_eq!((text_bytes(&fits), fits.truncated), (8_000_000, false));
    }

    #[test]
    fn a_styles_or_numbering_part_over_5_mb_fails_with_text() {
        let filler = format!("<!--{}-->", "x".repeat(5 * 1024 * 1024));
        for name in ["word/styles.xml", "word/numbering.xml"] {
            let message = failure(docx_with(&p("x"), &[(name, format!("<w:styles>{filler}</w:styles>"))]));
            assert!(message.contains("too much"), "{name}: {message}");
        }
    }

    #[test]
    fn the_parts_share_one_event_budget_and_running_out_marks_the_document_cut() {
        let styles = format!("<w:styles {W}>{}</w:styles>", r#"<w:style w:styleId="s"/>"#.repeat(1000));
        let bytes = docx_with(&p("x"), &[("word/styles.xml", styles)]);
        let Ok(FileContent::Document(starved)) = read_docx_with(bytes.clone(), 500) else { panic!("not a document") };
        assert_eq!((starved.blocks, starved.truncated), (vec![], true));
        let Ok(FileContent::Document(fed)) = read_docx_with(bytes, 5000) else { panic!("not a document") };
        assert_eq!((fed.blocks, fed.truncated), (vec![plain("x")], false));
    }

    #[test]
    fn a_doctype_entity_is_not_expanded() {
        let document = format!(r#"<?xml version="1.0"?><!DOCTYPE w:document [<!ENTITY e "EXPANDED"><!ENTITY big "&e;&e;&e;&e;">]><w:document {W}><w:body>{}</w:body></w:document>"#, p("a&e;&big;b"));
        let view = view(zip_of(&[("word/document.xml", document)]));
        assert_eq!((view.blocks, view.truncated), (vec![plain("ab")], false));
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
