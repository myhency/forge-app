use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::ChangesViewPrefs;
use forge_entities::{cap_lines, gap_after, gap_lines, icon_of, paired_spans, parse_patch, side_by_side, split_by_spans, DiffLineKind, PatchHunk, PatchLine, Span};
use forge_features::{is_folded, run_blocking, split_folder, toggle_fold, use_workspace, ChangesSession};
use forge_ports::{ChangeSource, ChangeStatus, DiffOptions, FileChange, PatchState};
use forge_shared::tr;

use crate::files_icons::FileTypeIcon;

// How many lines of one file are drawn at first and added by each `Show N more lines`, and the
// most that are ever drawn: the web view stops laying out a list near 65,536 children.
const LINE_STEP: usize = 2_000;
const LINE_MAX: usize = 20_000;

/// Scrolls the stack to a file's section.
pub(crate) fn scroll_to_diff(path: &str) {
    let literal = serde_json::to_string(path).unwrap_or_default();
    document::eval(&format!(
        "(function(p){{const el=[...document.querySelectorAll('[data-diff-path]')].find(e=>e.dataset.diffPath===p);if(el){{el.scrollIntoView({{block:'start'}});}}}})({literal});"
    ));
}

// A section's key: its path and a print of what it shows, so a reload that changes the diff starts
// the section afresh (the unchanged lines it read and a large diff it loaded belong to the old one).
fn section_key(source: &ChangeSource, change: &FileChange) -> String {
    let mut hasher = DefaultHasher::new();
    format!("{source:?}").hash(&mut hasher);
    match &change.patch {
        PatchState::Text(text) => text.hash(&mut hasher),
        other => format!("{other:?}").hash(&mut hasher),
    }
    (change.file.added, change.file.removed).hash(&mut hasher);
    format!("{}#{:x}", change.file.path, hasher.finish())
}

/// Every changed file one under the other, each foldable. `order` is the order of `files` to draw.
#[component]
pub fn DiffStack(
    id: LiveSessionId,
    cwd: PathBuf,
    source: ChangeSource,
    files: Vec<FileChange>,
    order: Vec<usize>,
    prefs: ChangesViewPrefs,
    session: ChangesSession,
    on_menu: EventHandler<(String, bool, f64, f64)>,
) -> Element {
    rsx! {
        div { class: "cd-stack",
            for (position, index) in order.iter().copied().enumerate() {
                FileSection {
                    key: "{section_key(&source, &files[index])}",
                    id,
                    cwd: cwd.clone(),
                    source: source.clone(),
                    change: files[index].clone(),
                    prefs: prefs.clone(),
                    folded: is_folded(&session, &files[index].file.path, position),
                    on_menu,
                }
            }
        }
    }
}

fn line_class(kind: DiffLineKind) -> &'static str {
    match kind {
        DiffLineKind::Context => "context",
        DiffLineKind::Added => "added",
        DiffLineKind::Removed => "removed",
    }
}

fn sign(kind: DiffLineKind) -> &'static str {
    match kind {
        DiffLineKind::Context => " ",
        DiffLineKind::Added => "+",
        DiffLineKind::Removed => "-",
    }
}

fn number(value: Option<u32>) -> String {
    value.map(|n| n.to_string()).unwrap_or_default()
}

fn line_row(line: &PatchLine, spans: &[Span], highlight: bool) -> Element {
    let pieces = if highlight && !spans.is_empty() { split_by_spans(&line.text, spans) } else { vec![(line.text.clone(), false)] };
    rsx! {
        div { class: "diff-line {line_class(line.kind)}",
            span { class: "cd-no", "{number(line.old_no)}" }
            span { class: "cd-no", "{number(line.new_no)}" }
            span { class: "diff-sign", "{sign(line.kind)}" }
            span { class: "diff-text",
                for (piece , hot) in pieces {
                    span { class: if hot { "cd-word" } else { "" }, "{piece}" }
                }
            }
        }
    }
}

fn cell(line: Option<&PatchLine>, spans: &[Span], highlight: bool) -> Element {
    match line {
        Some(line) => {
            let pieces = if highlight && !spans.is_empty() { split_by_spans(&line.text, spans) } else { vec![(line.text.clone(), false)] };
            let number = if line.kind == DiffLineKind::Added { line.new_no } else { line.old_no.or(line.new_no) };
            rsx! {
                div { class: "diff-line {line_class(line.kind)}",
                    span { class: "cd-no", "{self::number(number)}" }
                    span { class: "diff-sign", "{sign(line.kind)}" }
                    span { class: "diff-text",
                        for (piece , hot) in pieces {
                            span { class: if hot { "cd-word" } else { "" }, "{piece}" }
                        }
                    }
                }
            }
        }
        None => rsx! { div { class: "diff-line empty" } },
    }
}

fn hunk_view(hunk: &PatchHunk, prefs: &ChangesViewPrefs) -> Element {
    let spans = if prefs.highlight_words { paired_spans(&hunk.lines) } else { Vec::new() };
    let none: Vec<Span> = Vec::new();
    let spans_of = |index: usize| spans.get(index).unwrap_or(&none);
    if prefs.side_by_side {
        let rows = side_by_side(&hunk.lines);
        rsx! {
            div { class: "cd-hunk cd-sbs",
                for (left , right) in rows {
                    {cell(left.map(|i| &hunk.lines[i]), left.map(spans_of).unwrap_or(&none), prefs.highlight_words)}
                    {cell(right.map(|i| &hunk.lines[i]), right.map(spans_of).unwrap_or(&none), prefs.highlight_words)}
                }
            }
        }
    } else {
        rsx! {
            div { class: "cd-hunk",
                for (index , line) in hunk.lines.iter().enumerate() {
                    {line_row(line, spans_of(index), prefs.highlight_words)}
                }
            }
        }
    }
}

#[component]
fn FileSection(
    id: LiveSessionId,
    cwd: PathBuf,
    source: ChangeSource,
    change: FileChange,
    prefs: ChangesViewPrefs,
    folded: bool,
    on_menu: EventHandler<(String, bool, f64, f64)>,
) -> Element {
    let workspace = use_workspace();
    let path = change.file.path.clone();
    let deleted = change.file.status == ChangeStatus::Deleted;
    let (folder, name) = split_folder(&path);
    // The text of a file that was too large, once asked for.
    let mut loaded = use_signal(|| None::<Result<String, String>>);
    // The unchanged lines a click on a gap row read, by the hunk before the gap.
    let mut gaps = use_signal(HashMap::<usize, Vec<PatchLine>>::new);
    let mut limit = use_signal(|| LINE_STEP);
    let text: Option<String> = match (&change.patch, loaded()) {
        (PatchState::Text(text), _) => Some(text.clone()),
        (_, Some(Ok(text))) => Some(text),
        _ => None,
    };
    let hunks = text.as_deref().map(parse_patch).unwrap_or_default();
    let (shown, hidden) = cap_lines(&hunks, limit());
    let (added, removed) = (change.file.added.unwrap_or(0), change.file.removed.unwrap_or(0));
    // A file that did not exist before is all in its one hunk: nothing follows it.
    let created = hunks.first().is_some_and(|hunk| hunk.old_start == 0 && hunk.lines.iter().all(|line| line.kind == DiffLineKind::Added));
    let wrap_class = if prefs.wrap { "cd-lines" } else { "cd-lines nowrap" };

    let body = if folded {
        rsx! {}
    } else {
        match (&change.patch, loaded()) {
            (PatchState::Binary, _) => rsx! { div { class: "cd-note", {tr!("No textual changes to show")} } },
            (PatchState::TooLarge, None) => {
                let (cwd, source, path) = (cwd.clone(), source.clone(), path.clone());
                rsx! {
                    div { class: "cd-note",
                        button {
                            class: "cd-gap",
                            r#type: "button",
                            onclick: move |_| {
                                let (git, cwd, source, path) = (workspace.git(), cwd.clone(), source.clone(), path.clone());
                                let options = DiffOptions { ignore_whitespace: workspace.changes_view_prefs().hide_whitespace };
                                spawn(async move {
                                    if let Some(result) = run_blocking(move || git.file_patch(&cwd, &source, &options, &path)).await {
                                        loaded.set(Some(result.map_err(|error| error.to_string())));
                                    }
                                });
                            },
                            {tr!("Load diff")}
                        }
                    }
                }
            }
            (PatchState::TooLarge, Some(Err(error))) => rsx! { div { class: "cd-note error", "{error}" } },
            _ if hunks.is_empty() => rsx! { div { class: "cd-note", {tr!("No textual changes to show")} } },
            _ => rsx! {
                div { class: "{wrap_class}",
                    for (index , hunk) in shown.iter().enumerate() {
                        {hunk_view(hunk, &prefs)}
                        if hidden > 0 && index + 1 == shown.len() {
                            // The lines after the cut are not drawn, so there is no gap to open.
                        } else if let Some(lines) = gaps.read().get(&index) {
                            div { class: "cd-hunk",
                                for line in lines.iter() {
                                    {line_row(line, &[], false)}
                                }
                            }
                        } else if gap_after(&hunks, index).is_some() && !(created && index + 1 == hunks.len()) {
                            {
                                let (cwd, source, path, hunks) = (cwd.clone(), source.clone(), path.clone(), hunks.clone());
                                rsx! {
                                    button {
                                        class: "cd-gap",
                                        r#type: "button",
                                        onclick: move |_| {
                                            let (git, cwd, source, path, hunks) = (workspace.git(), cwd.clone(), source.clone(), path.clone(), hunks.clone());
                                            spawn(async move {
                                                let Some(read) = run_blocking(move || git.file_lines(&cwd, &source, &path)).await else { return };
                                                // A file that cannot be read has no more lines to show.
                                                let lines = gap_lines(&hunks, index, &read.unwrap_or_default());
                                                gaps.write().insert(index, lines);
                                            });
                                        },
                                        span { class: "cd-chevron", "⌄" }
                                        {tr!("Show more lines")}
                                    }
                                }
                            }
                        }
                    }
                    if hidden > 0 && limit() < LINE_MAX {
                        button {
                            class: "cd-gap",
                            r#type: "button",
                            onclick: move |_| limit += LINE_STEP,
                            span { class: "cd-chevron", "⌄" }
                            {tr!("Show {count} more lines", count = hidden.min(LINE_STEP))}
                        }
                    } else if hidden > 0 {
                        div { class: "cd-note", {tr!("{count} more lines are not shown", count = hidden)} }
                    }
                }
            },
        }
    };

    rsx! {
        section { class: "cd-file", "data-diff-path": "{path}",
            div {
                class: "cd-file-head",
                role: "button",
                tabindex: "0",
                onclick: {
                    let path = path.clone();
                    move |_| workspace.update_changes_session(id, |session| toggle_fold(session, &path, folded))
                },
                oncontextmenu: {
                    let path = path.clone();
                    move |event: MouseEvent| {
                        event.prevent_default();
                        let point = event.client_coordinates();
                        on_menu.call((path.clone(), deleted, point.x, point.y));
                    }
                },
                span { class: "cd-chevron", if folded { "▸" } else { "▾" } }
                FileTypeIcon { icon: icon_of(&path) }
                span { class: "cd-name", "{name}" }
                if !folder.is_empty() {
                    span { class: "cd-folder", "{folder}" }
                }
                span { class: "cd-counts",
                    span { class: "cd-add", "+{added}" }
                    " "
                    span { class: "cd-del", "−{removed}" }
                }
            }
            {body}
        }
    }
}
