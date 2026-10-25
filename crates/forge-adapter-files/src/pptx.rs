//! Reads a pptx file's slides into a title and indented lines of text for the Files panel.

use std::collections::HashMap;

use forge_ports::{FileContent, FilesError, Slide, SlideDeck, SlideItem};
use quick_xml::events::{BytesStart, Event};

use crate::office_zip::{self, attribute, cut, failed, TextBudget};

const MAX_SLIDES: usize = 200;
// One slide keeps at most this many lines, and all slides together at most `MAX_TOTAL_ITEMS`.
const MAX_ITEMS: usize = 2_000;
const MAX_TOTAL_ITEMS: usize = 10_000;
// One title or line keeps at most this many characters.
const MAX_TEXT_CHARS: usize = 10_000;
// All the slides together may unpack to at most this, whatever each one declares.
const TOTAL_SLIDE_BYTES: u64 = 200 * 1024 * 1024;
// The whole file stops being read after this many XML events, so a deck of millions of empty tags
// cannot keep the read busy for long.
const EVENT_LIMIT: usize = 5_000_000;
// PowerPoint nests a bullet 9 deep (`lvl` 0 to 8).
const MAX_LEVEL: u8 = 8;
// The fallback copy of alternate content repeats the text before it.
const HIDDEN: [&str; 1] = ["Fallback"];
const SLIDE_RELATIONSHIP: &str = "/slide";

/// The slides of a pptx file (`ppt/presentation.xml` and the slide parts it lists) as a title and
/// lines of text. Every part is checked against the zip guards before it is parsed; entities other
/// than the five predefined ones and character references are not resolved, and nothing outside the
/// file's own zip is opened. Layouts, masters and speaker notes are never read.
pub(crate) fn read_pptx(bytes: Vec<u8>) -> Result<FileContent, FilesError> {
    read_pptx_with(bytes, EVENT_LIMIT)
}

fn read_pptx_with(bytes: Vec<u8>, mut budget: usize) -> Result<FileContent, FilesError> {
    let mut archive = office_zip::open(&bytes, "This presentation is password-protected, so it cannot be shown.")?;
    let too_big = || failed("This presentation unpacks to too much data to show.");
    if office_zip::declares_too_much(&mut archive, office_zip::PART_BYTES) {
        return Err(too_big());
    }
    let presentation = office_zip::read_part(&mut archive, "ppt/presentation.xml", office_zip::SMALL_PART_BYTES, too_big)?
        .ok_or_else(|| failed("This file has no ppt/presentation.xml, so it is not a PowerPoint presentation."))?;
    let (listed, mut truncated) = slide_ids(&presentation, &mut budget)?;
    let targets = match office_zip::read_part(&mut archive, "ppt/_rels/presentation.xml.rels", office_zip::SMALL_PART_BYTES, too_big)? {
        Some(rels) => {
            let (targets, ran_out) = slide_targets(&rels, &mut budget)?;
            truncated |= ran_out;
            targets
        }
        None => HashMap::new(),
    };
    let mut text = TextBudget::new(office_zip::TOTAL_TEXT_BYTES);
    let mut items_left = MAX_TOTAL_ITEMS;
    let mut slides = Vec::new();
    let mut unpacked = 0u64;
    for (index, id) in listed.iter().enumerate() {
        if budget == 0 {
            truncated = true;
            break;
        }
        let Some(name) = targets.get(id).and_then(|target| part_name(target)) else { continue };
        // A listed slide whose part is missing is skipped; the numbers stay by position.
        let Some(xml) = office_zip::read_part(&mut archive, &name, office_zip::PART_BYTES, too_big)? else { continue };
        unpacked += xml.len() as u64;
        if unpacked > TOTAL_SLIDE_BYTES {
            return Err(too_big());
        }
        // Once the totals are spent a slide still shows its title, and its first line marks the cut.
        let mut reader = SlideReader { text, items_left, ..SlideReader::default() };
        truncated |= office_zip::walk(&xml, &mut budget, |event| reader.event(event))? | reader.truncated;
        (text, items_left) = (reader.text, reader.items_left);
        slides.push(Slide { number: index + 1, title: reader.title, items: reader.items });
    }
    Ok(FileContent::Slides(SlideDeck { slides, truncated }))
}

// The relationship id of a `sldId`: the attribute `r:id` (its prefix can be spelled otherwise), not
// the plain `id`.
fn relationship_id(tag: &BytesStart<'_>) -> Option<String> {
    tag.attributes().flatten().find(|attribute| attribute.key.local_name().as_ref() == "id" && attribute.key.prefix().is_some()).map(|attribute| attribute.value.into_owned())
}

// The relationship ids of `p:sldIdLst` in slide order, cut to the slide cap (true when it cut or
// the event budget ran out).
fn slide_ids(xml: &[u8], budget: &mut usize) -> Result<(Vec<String>, bool), FilesError> {
    let mut ids = Vec::new();
    let mut cut_off = false;
    let ran_out = office_zip::walk(xml, budget, |event| {
        if let Event::Empty(tag) | Event::Start(tag) = event {
            if tag.local_name().as_ref() == "sldId" {
                if ids.len() == MAX_SLIDES {
                    cut_off = true;
                    return false;
                }
                ids.extend(relationship_id(&tag));
            }
        }
        true
    })?;
    Ok((ids, cut_off || ran_out))
}

// The target of each slide relationship, by relationship id, and whether the event budget ran out.
fn slide_targets(xml: &[u8], budget: &mut usize) -> Result<(HashMap<String, String>, bool), FilesError> {
    let mut targets = HashMap::new();
    let ran_out = office_zip::walk(xml, budget, |event| {
        if let Event::Empty(tag) | Event::Start(tag) = event {
            let is_slide = attribute(&tag, "Type").is_some_and(|kind| kind.ends_with(SLIDE_RELATIONSHIP));
            if let (true, Some(id), Some(target)) = (tag.local_name().as_ref() == "Relationship" && is_slide, attribute(&tag, "Id"), attribute(&tag, "Target")) {
                targets.entry(id).or_insert(target);
            }
        }
        true
    })?;
    Ok((targets, ran_out))
}

// The zip name of a relationship target, which is relative to `ppt/` unless it starts with a slash.
// A target that climbs out of the zip has no name.
fn part_name(target: &str) -> Option<String> {
    let (base, relative) = match target.strip_prefix('/') {
        Some(rooted) => (Vec::new(), rooted),
        None => (vec!["ppt"], target),
    };
    let mut parts = base;
    for part in relative.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

struct Paragraph {
    level: u8,
    text: String,
    chars: usize,
}

/// The state of a walk through one slide part. Shapes, group shapes and table cells are all just
/// paragraphs here, so nesting needs no stack.
#[derive(Default)]
struct SlideReader {
    title: Option<String>,
    // Set once a shape's title is complete; a second title shape's text becomes lines.
    title_taken: bool,
    items: Vec<SlideItem>,
    truncated: bool,
    // How deep the walk is inside a hidden element.
    hidden: usize,
    in_shape: bool,
    in_title_shape: bool,
    paragraph: Option<Paragraph>,
    in_text: bool,
    // What the whole deck may still keep, handed from slide to slide.
    text: TextBudget,
    items_left: usize,
}

impl SlideReader {
    // False once the line cap is reached, which ends the walk.
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
        match tag.local_name().as_ref() {
            "sp" if !empty => (self.in_shape, self.in_title_shape) = (true, false),
            "ph" if self.in_shape => {
                if attribute(tag, "type").is_some_and(|kind| kind == "title" || kind == "ctrTitle") {
                    self.in_title_shape = true;
                }
            }
            "p" if !empty && self.paragraph.is_none() => self.paragraph = Some(Paragraph { level: 0, text: String::new(), chars: 0 }),
            "pPr" => {
                let level = attribute(tag, "lvl").and_then(|value| value.parse::<u8>().ok()).unwrap_or(0).min(MAX_LEVEL);
                if let Some(paragraph) = self.paragraph.as_mut() {
                    paragraph.level = level;
                }
            }
            "t" if !empty && self.paragraph.is_some() => self.in_text = true,
            "br" if self.paragraph.is_some() => self.add_text(" "),
            _ => {}
        }
    }

    // False once the line cap is reached.
    fn close(&mut self, name: &str) -> bool {
        match name {
            "t" => self.in_text = false,
            "p" => return self.end_paragraph(),
            "sp" => {
                self.title_taken |= self.in_title_shape && self.title.is_some();
                (self.in_shape, self.in_title_shape) = (false, false);
            }
            _ => {}
        }
        true
    }

    // Adds text to the open paragraph, up to the text cap.
    fn add_text(&mut self, text: &str) {
        let Some(paragraph) = self.paragraph.as_mut() else { return };
        let (kept, was_cut) = cut(text, MAX_TEXT_CHARS - paragraph.chars);
        let (kept, over_budget) = self.text.take(kept);
        self.truncated |= was_cut | over_budget;
        paragraph.chars += kept.chars().count();
        paragraph.text.push_str(kept);
    }

    // A paragraph with text becomes a line, or the title when it sits in the first title shape.
    fn end_paragraph(&mut self) -> bool {
        self.in_text = false;
        let Some(paragraph) = self.paragraph.take() else { return true };
        let text = paragraph.text.trim();
        if text.is_empty() {
            return true;
        }
        if self.in_title_shape && !self.title_taken {
            // The text is already counted in the budget; the joining space is not.
            if self.title.as_ref().is_some_and(|title| !title.is_empty()) && self.text.take(" ").1 {
                self.truncated = true;
                return true;
            }
            let title = self.title.get_or_insert_with(String::new);
            let joined = if title.is_empty() { text.to_string() } else { format!(" {text}") };
            let room = MAX_TEXT_CHARS - title.chars().count();
            let (kept, was_cut) = cut(&joined, room);
            self.truncated |= was_cut;
            title.push_str(kept);
            return true;
        }
        if self.items.len() == MAX_ITEMS || self.items_left == 0 {
            self.truncated = true;
            return false;
        }
        self.items_left -= 1;
        self.items.push(SlideItem { level: paragraph.level, text: text.to_string() });
        true
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use forge_ports::{FilesPort, Slide, SlideDeck, SlideItem};

    use super::*;
    use crate::office_zip::test_support::{declaring, zip_of};
    use crate::LocalFiles;

    const NS: &str = r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#;
    const SLIDE_TYPE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";

    fn slide_xml(shapes: &str) -> String {
        format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sld {NS}><p:cSld><p:spTree>{shapes}</p:spTree></p:cSld></p:sld>"#)
    }

    // A shape with these paragraphs, as (level, text); `placeholder` is its `p:ph type`.
    fn shape(placeholder: Option<&str>, paragraphs: &[(u8, &str)]) -> String {
        let ph = placeholder.map(|kind| format!(r#"<p:ph type="{kind}"/>"#)).unwrap_or_default();
        let body: String = paragraphs.iter().map(|(level, text)| format!(r#"<a:p><a:pPr lvl="{level}"/><a:r><a:t>{text}</a:t></a:r></a:p>"#)).collect();
        format!(r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="s"/><p:cNvSpPr/><p:nvPr>{ph}</p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/>{body}</p:txBody></p:sp>"#)
    }

    // A pptx listing `listed` (relationship id, target) in this order, with these parts.
    fn pptx_of(listed: &[(&str, &str)], parts: &[(&str, String)]) -> Vec<u8> {
        let ids: String = listed.iter().enumerate().map(|(index, (id, _))| format!(r#"<p:sldId id="{}" r:id="{id}"/>"#, 256 + index)).collect();
        let rels: String = listed.iter().map(|(id, target)| format!(r#"<Relationship Id="{id}" Type="{SLIDE_TYPE}" Target="{target}"/>"#)).collect();
        let mut all = vec![
            ("[Content_Types].xml", r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>"#.to_string()),
            ("ppt/presentation.xml", format!(r#"<?xml version="1.0"?><p:presentation {NS}><p:sldMasterIdLst><p:sldMasterId id="1" r:id="rIdM"/></p:sldMasterIdLst><p:sldIdLst>{ids}</p:sldIdLst></p:presentation>"#)),
            ("ppt/_rels/presentation.xml.rels", format!(r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdM" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>{rels}</Relationships>"#)),
        ];
        all.extend(parts.iter().map(|(name, text)| (*name, text.clone())));
        zip_of(&all)
    }

    // A pptx whose slides are these shape lists, listed as slide1, slide2 …
    fn pptx(slides: &[String]) -> Vec<u8> {
        let listed: Vec<(String, String)> = (1..=slides.len()).map(|n| (format!("rId{n}"), format!("slides/slide{n}.xml"))).collect();
        let listed: Vec<(&str, &str)> = listed.iter().map(|(id, target)| (id.as_str(), target.as_str())).collect();
        let parts: Vec<(String, String)> = slides.iter().enumerate().map(|(index, shapes)| (format!("ppt/slides/slide{}.xml", index + 1), slide_xml(shapes))).collect();
        let parts: Vec<(&str, String)> = parts.iter().map(|(name, text)| (name.as_str(), text.clone())).collect();
        pptx_of(&listed, &parts)
    }

    fn deck(bytes: Vec<u8>) -> SlideDeck {
        match read_pptx(bytes) {
            Ok(FileContent::Slides(deck)) => deck,
            other => panic!("not slides: {other:?}"),
        }
    }

    fn failure(bytes: Vec<u8>) -> String {
        match read_pptx(bytes) {
            Err(FilesError::Failed(message)) => message,
            other => panic!("did not fail: {other:?}"),
        }
    }

    fn item(level: u8, text: &str) -> SlideItem {
        SlideItem { level, text: text.to_string() }
    }

    fn slide(number: usize, title: Option<&str>, items: Vec<SlideItem>) -> Slide {
        Slide { number, title: title.map(str::to_string), items }
    }

    #[test]
    fn slides_come_in_presentation_order_not_file_name_order() {
        let parts = [("ppt/slides/slide1.xml", slide_xml(&shape(None, &[(0, "one")]))), ("ppt/slides/slide2.xml", slide_xml(&shape(None, &[(0, "two")])))];
        let bytes = pptx_of(&[("rId9", "slides/slide2.xml"), ("rId3", "slides/slide1.xml")], &parts);
        let deck = deck(bytes);
        assert_eq!(deck.slides, vec![slide(1, None, vec![item(0, "two")]), slide(2, None, vec![item(0, "one")])]);
        assert!(!deck.truncated);
    }

    #[test]
    fn a_title_placeholder_becomes_the_title_and_other_text_becomes_items_with_levels() {
        let one = [shape(Some("ctrTitle"), &[(0, "Big")]), shape(Some("body"), &[(0, "a"), (1, "b &amp; c"), (2, "&#x41;"), (0, "  "), (0, "")])].concat();
        let two = [shape(Some("title"), &[(0, "Head")]), shape(Some("title"), &[(0, "second")]), shape(Some("subTitle"), &[(0, "sub")])].concat();
        let deck = deck(pptx(&[one, two]));
        assert_eq!(
            deck.slides,
            vec![slide(1, Some("Big"), vec![item(0, "a"), item(1, "b & c"), item(2, "A")]), slide(2, Some("Head"), vec![item(0, "second"), item(0, "sub")])]
        );
    }

    #[test]
    fn text_in_a_group_shape_and_a_table_cell_is_included() {
        let group = format!(r#"<p:grpSp><p:nvGrpSpPr/><p:grpSpPr/>{}<p:grpSp>{}</p:grpSp></p:grpSp>"#, shape(None, &[(0, "in group")]), shape(None, &[(1, "deeper")]));
        let cell = |text: &str| format!(r#"<a:tc><a:txBody><a:bodyPr/><a:p><a:r><a:t>{text}</a:t></a:r></a:p></a:txBody></a:tc>"#);
        let table = format!(r#"<p:graphicFrame><a:graphic><a:graphicData><a:tbl><a:tr>{}{}</a:tr><a:tr>{}<a:tc/></a:tr></a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#, cell("c1"), cell("c2"), cell("c3"));
        let deck = deck(pptx(&[[group, table, shape(None, &[(0, "after")])].concat()]));
        assert_eq!(deck.slides[0].items, vec![item(0, "in group"), item(1, "deeper"), item(0, "c1"), item(0, "c2"), item(0, "c3"), item(0, "after")]);
    }

    #[test]
    fn a_slide_with_only_a_picture_has_no_items() {
        let picture = r#"<p:pic><p:nvPicPr><p:cNvPr id="4" name="Picture"/></p:nvPicPr><p:blipFill/><p:spPr/></p:pic>"#;
        let deck = deck(pptx(&[picture.to_string()]));
        assert_eq!(deck.slides, vec![slide(1, None, vec![])]);
    }

    #[test]
    fn a_listed_slide_whose_part_is_missing_is_skipped() {
        let parts = [("ppt/slides/slide2.xml", slide_xml(&shape(None, &[(0, "two")])))];
        let bytes = pptx_of(&[("rId1", "slides/slide1.xml"), ("rId2", "slides/slide2.xml"), ("rId7", "/ppt/slides/slide2.xml")], &parts);
        let deck = deck(bytes);
        assert_eq!(deck.slides, vec![slide(2, None, vec![item(0, "two")]), slide(3, None, vec![item(0, "two")])]);
    }

    #[test]
    fn broken_relationships_fail_with_text_or_give_no_slides_without_panicking() {
        // An id with no relationship, a relationship that is not a slide, and a target that climbs out.
        let parts = [("ppt/slides/slide1.xml", slide_xml(&shape(None, &[(0, "x")])))];
        let bytes = pptx_of(&[("rIdNone", "")], &parts);
        let mut archive_less = deck(bytes);
        assert_eq!(archive_less.slides, vec![]);
        archive_less = deck(pptx_of(&[("rId1", "../../../slides/slide1.xml")], &parts));
        assert_eq!(archive_less.slides, vec![]);
        // No relationships part at all: no slides.
        let without_rels = zip_of(&[("ppt/presentation.xml", format!(r#"<p:presentation {NS}><p:sldIdLst><p:sldId id="256" r:id="rId1"/></p:sldIdLst></p:presentation>"#))]);
        assert_eq!(deck(without_rels).slides, vec![]);
        // Broken XML fails with text.
        let broken = zip_of(&[("ppt/presentation.xml", "<p:presentation><p:sldIdLst>".to_string())]);
        assert!(!failure(broken).is_empty());
        let broken_slide = pptx_of(&[("rId1", "slides/slide1.xml")], &[("ppt/slides/slide1.xml", "<p:sld><a:p>".to_string())]);
        assert!(!failure(broken_slide).is_empty());
        // No presentation.xml: not a presentation.
        assert!(failure(zip_of(&[("x.xml", "<x/>".to_string())])).contains("presentation.xml"));
    }

    #[test]
    fn more_than_200_slides_are_cut_and_marked() {
        let slides_of = |count: usize| deck(pptx(&(0..count).map(|index| shape(None, &[(0, &index.to_string())])).collect::<Vec<_>>()));
        let exact = slides_of(200);
        assert_eq!((exact.slides.len(), exact.truncated), (200, false));
        let over = slides_of(201);
        assert_eq!((over.slides.len(), over.truncated), (200, true));
        assert_eq!(over.slides[199], slide(200, None, vec![item(0, "199")]));
    }

    #[test]
    fn an_item_over_10000_characters_is_cut() {
        let exact = deck(pptx(&[shape(None, &[(0, &"가".repeat(10_000))])]));
        assert_eq!((exact.slides[0].items.clone(), exact.truncated), (vec![item(0, &"가".repeat(10_000))], false));
        let over = deck(pptx(&[[shape(Some("title"), &[(0, &"t".repeat(10_001))]), shape(None, &[(0, &"a".repeat(10_001))])].concat()]));
        assert_eq!(over.slides[0].items, vec![item(0, &"a".repeat(10_000))]);
        assert_eq!(over.slides[0].title, Some("t".repeat(10_000)));
        assert!(over.truncated);
    }

    #[test]
    fn speaker_notes_are_not_shown() {
        let notes = format!(r#"<p:notes {NS}><p:cSld><p:spTree>{}</p:spTree></p:cSld></p:notes>"#, shape(None, &[(0, "secret")]));
        let parts = [("ppt/slides/slide1.xml", slide_xml(&shape(None, &[(0, "shown")]))), ("ppt/notesSlides/notesSlide1.xml", notes)];
        let deck = deck(pptx_of(&[("rId1", "slides/slide1.xml")], &parts));
        assert_eq!(deck.slides, vec![slide(1, None, vec![item(0, "shown")])]);
    }

    #[test]
    fn a_part_declaring_over_100_mb_is_refused() {
        let bytes = pptx(&[shape(None, &[(0, "x")])]);
        assert!(read_pptx(declaring(bytes.clone(), "ppt/slides/slide1.xml", 100 * 1024 * 1024)).is_ok());
        let message = failure(declaring(bytes, "ppt/slides/slide1.xml", 100 * 1024 * 1024 + 1));
        assert!(message.contains("too much"), "{message}");
    }

    #[test]
    fn a_presentation_part_over_5_mb_is_refused() {
        let filler = format!("<!--{}-->", "x".repeat(5 * 1024 * 1024));
        let bytes = zip_of(&[("ppt/presentation.xml", format!("<p:presentation>{filler}</p:presentation>"))]);
        assert!(failure(bytes).contains("too much"));
    }

    #[test]
    fn all_slides_together_keep_at_most_10000_items() {
        // Six slides of 2,000 lines each: no slide is over its own cap.
        let lines: Vec<(u8, &str)> = vec![(0, "x"); 2_000];
        let full = deck(pptx(&vec![shape(None, &lines); 6]));
        assert_eq!(full.slides.iter().map(|slide| slide.items.len()).sum::<usize>(), 10_000);
        assert!(full.truncated);
        let fits = deck(pptx(&vec![shape(None, &lines); 5]));
        assert_eq!((fits.slides.iter().map(|slide| slide.items.len()).sum::<usize>(), fits.truncated), (10_000, false));
    }

    #[test]
    fn all_text_together_keeps_at_most_8_mb() {
        let long = "a".repeat(10_000);
        let lines: Vec<(u8, &str)> = vec![(0, long.as_str()); 100];
        let text_bytes = |deck: &SlideDeck| -> usize {
            deck.slides.iter().map(|slide| slide.title.as_ref().map_or(0, String::len) + slide.items.iter().map(|item| item.text.len()).sum::<usize>()).sum()
        };
        let over = deck(pptx(&vec![[shape(Some("title"), &[(0, long.as_str())]), shape(None, &lines)].concat(); 9]));
        assert!(text_bytes(&over) <= office_zip::TOTAL_TEXT_BYTES, "{}", text_bytes(&over));
        assert!(text_bytes(&over) > office_zip::TOTAL_TEXT_BYTES - 10_000);
        assert!(over.truncated);
        let fits = deck(pptx(&vec![shape(None, &lines); 8]));
        assert_eq!((text_bytes(&fits), fits.truncated), (8_000_000, false));
    }

    #[test]
    fn running_out_of_events_in_the_list_of_slides_marks_the_deck_cut() {
        let bytes = pptx(&[shape(None, &[(0, "x")])]);
        let Ok(FileContent::Slides(starved)) = read_pptx_with(bytes.clone(), 3) else { panic!("not slides") };
        assert!(starved.truncated);
        let Ok(FileContent::Slides(fed)) = read_pptx_with(bytes, 5000) else { panic!("not slides") };
        assert_eq!((fed.slides.len(), fed.truncated), (1, false));
    }

    #[test]
    fn a_doctype_entity_is_not_expanded() {
        let slide = format!(r#"<?xml version="1.0"?><!DOCTYPE p:sld [<!ENTITY e "EXPANDED">]><p:sld {NS}><p:cSld><p:spTree>{}</p:spTree></p:cSld></p:sld>"#, shape(None, &[(0, "a&e;b")]));
        let deck = deck(pptx_of(&[("rId1", "slides/slide1.xml")], &[("ppt/slides/slide1.xml", slide)]));
        assert_eq!((deck.slides, deck.truncated), (vec![self::slide(1, None, vec![item(0, "ab")])], false));
    }

    fn read_file(name: &str, bytes: &[u8]) -> Result<FileContent, FilesError> {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(name), bytes).unwrap();
        LocalFiles::new().read(dir.path(), name)
    }

    #[test]
    fn a_non_zip_pptx_fails_with_text() {
        let Err(FilesError::Failed(message)) = read_file("bad.pptx", b"just words") else { panic!("did not fail") };
        assert!(!message.is_empty());
    }

    #[test]
    fn a_compound_file_named_pptx_fails_with_password_text() {
        let mut compound = cfb::CompoundFile::create_with_version(cfb::Version::V3, Cursor::new(Vec::new())).unwrap();
        compound.create_stream("/EncryptedPackage").unwrap().write_all(&[0u8; 5000]).unwrap();
        compound.flush().unwrap();
        let bytes = compound.into_inner().into_inner();
        let Err(FilesError::Failed(message)) = read_file("locked.pptx", &bytes) else { panic!("did not fail") };
        assert!(message.to_lowercase().contains("password"), "{message}");
    }

    #[test]
    fn an_uppercase_pptx_extension_is_read() {
        let Ok(FileContent::Slides(deck)) = read_file("UP.PPTX", &pptx(&[shape(None, &[(0, "x")])])) else { panic!("not slides") };
        assert_eq!(deck.slides, vec![slide(1, None, vec![item(0, "x")])]);
    }
}
