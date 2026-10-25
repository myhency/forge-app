use std::rc::Rc;

use dioxus::prelude::*;
use forge_entities::{preview_kind, PreviewKind};
use forge_features::offers_default_program;
use forge_ports::{Sheet, SheetBook};
use forge_shared::tr;

/// The spreadsheet letters of a zero-based column: 0 is `A`, 25 is `Z`, 26 is `AA`.
pub fn column_letters(index: usize) -> String {
    let mut letters = Vec::new();
    let mut rest = index + 1;
    while rest > 0 {
        rest -= 1;
        letters.push(b'A' + (rest % 26) as u8);
        rest /= 26;
    }
    letters.reverse();
    String::from_utf8(letters).unwrap_or_default()
}

/// The address of a zero-based cell, like `B3`.
pub fn cell_address(row: usize, column: usize) -> String {
    format!("{}{}", column_letters(column), row + 1)
}

/// Whether a cell's text reads as a number, so the grid aligns it to the right.
pub fn is_number(text: &str) -> bool {
    let text = text.trim().replace(',', "");
    !text.is_empty() && text.parse::<f64>().is_ok_and(f64::is_finite)
}

/// What the formula bar shows for the cell: its formula, or else its text.
pub fn formula_bar_text(sheet: &Sheet, row: usize, column: usize) -> String {
    match sheet.rows.get(row).and_then(|cells| cells.get(column)) {
        Some(cell) => cell.formula.clone().unwrap_or_else(|| cell.text.clone()),
        None => String::new(),
    }
}

/// How many columns the sheet's widest row has.
pub fn column_count(sheet: &Sheet) -> usize {
    sheet.rows.iter().map(Vec::len).max().unwrap_or(0)
}

/// Whether a binary file's notice offers the default program: any file that is not a program.
pub fn binary_offers_open(path: &str) -> bool {
    offers_default_program(path)
}

/// Whether a failed read offers the default program: only for a spreadsheet, whose reader may be what failed.
pub fn failed_offers_open(path: &str) -> bool {
    preview_kind(path) == PreviewKind::Sheet && offers_default_program(path)
}

/// The class of a grid cell; the selection is drawn by `selected_rule`, so a number keeps its alignment.
pub fn cell_class(number: bool) -> &'static str {
    if number { "sheet-cell number" } else { "sheet-cell" }
}

/// The style rule that outlines the selected cell, so a click changes this rule and not the grid.
pub fn selected_rule(row: usize, column: usize) -> String {
    format!("td.sheet-cell[data-r='{row}'][data-c='{column}'] {{ outline: 2px solid var(--accent); outline-offset: -2px; }}")
}

/// The body of an xlsx tab: a formula bar, the grid, and the sheet tabs.
#[component]
pub fn SheetView(book: SheetBook, path: String, on_open: EventHandler<()>) -> Element {
    let book = use_memo(use_reactive!(|book| Rc::new(book)));
    let mut active = use_signal(|| 0_usize);
    let mut selected = use_signal(|| (0_usize, 0_usize));
    let (row, column) = selected();
    let book = book();
    let index = if active() < book.sheets.len() { active() } else { 0 };
    let address = cell_address(row, column);
    let bar = book.sheets.get(index).map(|sheet| formula_bar_text(sheet, row, column)).unwrap_or_default();
    let rule = selected_rule(row, column);
    let truncated = book.sheets.get(index).is_some_and(|sheet| sheet.truncated);
    rsx! {
        div { class: "sheet-view", "aria-label": "{path}",
            style { "{rule}" }
            div { class: "sheet-toolbar",
                span { class: "sheet-address", "{address}" }
                span { class: "sheet-formula", "{bar}" }
                if binary_offers_open(&path) {
                    button { class: "files-kind-button", r#type: "button", onclick: move |_| on_open.call(()), {tr!("Open in the default program")} }
                }
            }
            if truncated {
                p { class: "files-csv-note", {tr!("Only the first 500 rows and 50 columns are shown.")} }
            }
            SheetGrid { key: "{index}", book: book.clone(), index, selected }
            div { class: "sheet-tabs",
                for (tab, sheet) in book.sheets.iter().enumerate() {
                    button {
                        key: "{tab}",
                        class: if tab == index { "sheet-tab active" } else { "sheet-tab" },
                        r#type: "button",
                        onclick: move |_| {
                            active.set(tab);
                            selected.set((0, 0));
                        },
                        "{sheet.name}"
                    }
                }
            }
        }
    }
}

/// The cells of one sheet. It only writes `selected`, never reads it, so a click does not rebuild the grid;
/// the selected cell is outlined by `selected_rule`.
#[component]
fn SheetGrid(book: Rc<SheetBook>, index: usize, mut selected: Signal<(usize, usize)>) -> Element {
    let Some(sheet) = book.sheets.get(index) else { return rsx! {} };
    let columns = column_count(sheet);
    rsx! {
        div { class: "sheet-grid",
            table {
                thead {
                    tr {
                        th { class: "sheet-corner" }
                        for index in 0..columns {
                            th { key: "{index}", class: "sheet-colhead", "{column_letters(index)}" }
                        }
                    }
                }
                tbody {
                    for (row_index, cells) in sheet.rows.iter().enumerate() {
                        tr { key: "{row_index}",
                            th { class: "sheet-rowhead", "{row_index + 1}" }
                            for (index, cell) in cells.iter().enumerate() {
                                td {
                                    key: "{index}",
                                    class: cell_class(is_number(&cell.text)),
                                    "data-r": "{row_index}",
                                    "data-c": "{index}",
                                    onclick: move |_| selected.set((row_index, index)),
                                    "{cell.text}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use forge_ports::SheetCell;

    use super::*;

    fn cell(text: &str, formula: Option<&str>) -> SheetCell {
        SheetCell { text: text.to_string(), formula: formula.map(str::to_string) }
    }

    #[test]
    fn columns_are_lettered_like_a_spreadsheet() {
        for (index, letters) in [(0, "A"), (25, "Z"), (26, "AA"), (51, "AZ"), (701, "ZZ"), (702, "AAA")] {
            assert_eq!(column_letters(index), letters, "{index}");
        }
    }

    #[test]
    fn an_address_is_the_column_letters_and_the_one_based_row() {
        assert_eq!(cell_address(2, 1), "B3");
        assert_eq!(cell_address(0, 0), "A1");
    }

    #[test]
    fn only_text_that_parses_as_a_number_is_a_number() {
        for yes in ["1", "-2.5", " 3 ", "1,234.5", "1e3"] {
            assert!(is_number(yes), "{yes}");
        }
        for no in ["", " ", "abc", "1a", "NaN", "inf"] {
            assert!(!is_number(no), "{no}");
        }
    }

    #[test]
    fn the_formula_bar_shows_the_formula_else_the_text_else_nothing() {
        let sheet = Sheet {
            name: "S".to_string(),
            rows: vec![vec![cell("3", Some("=1+2")), cell("x", None)]],
            truncated: false,
        };
        assert_eq!(formula_bar_text(&sheet, 0, 0), "=1+2");
        assert_eq!(formula_bar_text(&sheet, 0, 1), "x");
        assert_eq!(formula_bar_text(&sheet, 5, 5), "");
        assert_eq!(column_count(&sheet), 2);
    }

    #[test]
    fn a_number_keeps_its_alignment_class_and_the_selection_is_a_separate_rule() {
        assert_eq!(cell_class(false), "sheet-cell");
        assert_eq!(cell_class(true), "sheet-cell number");
        let rule = selected_rule(2, 1);
        assert!(rule.contains("[data-r='2'][data-c='1']") && rule.contains("outline"));
    }

    #[test]
    fn the_open_button_is_offered_for_files_that_are_not_programs() {
        assert!(binary_offers_open("a/report.docx"));
        assert!(!binary_offers_open("a/tool.exe"));
        assert!(failed_offers_open("a/book.xlsx"));
        assert!(!failed_offers_open("a/book.rs"));
    }
}
