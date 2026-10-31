use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::{Card, ChangesViewPrefs};
use forge_entities::{cap_lines, gap_after, gap_lines, icon_of, paired_spans, parse_patch, side_by_side, split_by_spans, DiffLineKind, PatchHunk, PatchLine, Span};
use forge_features::{absolute_path, comment_line, commentable, comments_of, comments_on, is_folded, run_blocking, split_folder, toggle_fold, use_workspace, ChangesSession};
use forge_ports::{ChangeSource, ChangeStatus, DiffOptions, FileChange, GitError, PatchState, TOO_LARGE_TO_SHOW};
use forge_shared::{tr, Icon, IconKind, IconSize};

use crate::changes_comment::DiffCommentBox;
use crate::files_icons::FileTypeIcon;

// How many lines of one file are drawn at first and added by each `Show N more lines`, and the
// most that are ever drawn: the web view stops laying out a list near 65,536 children.
const LINE_STEP: usize = 2_000;
const LINE_MAX: usize = 20_000;

/// Scrolls the stack to a file's section.
pub(crate) fn scroll_to_diff(path: &str) {
    let literal = serde_json::to_string(path).unwrap_or_default();
    document::eval(&format!(
        "(function(p){{const el=[...document.querySelectorAll('.pane.focused [data-diff-path]')].find(e=>e.dataset.diffPath===p);if(el){{el.scrollIntoView({{block:'start'}});}}}})({literal});"
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
    root: Option<PathBuf>,
    source: ChangeSource,
    files: Vec<FileChange>,
    order: Vec<usize>,
    prefs: ChangesViewPrefs,
    session: ChangesSession,
    on_menu: EventHandler<(String, bool, f64, f64)>,
) -> Element {
    // The cards are read here once, so a change to them reaches a file only when its own comments
    // changed. Until the repository root is known the paths cannot be keyed, so no comments.
    let workspace = use_workspace();
    let cards = workspace.cards(id);
    let can_comment = commentable(&source) && root.is_some();
    let root = root.unwrap_or_default();
    rsx! {
        div { class: "cd-stack",
            for (position, index) in order.iter().copied().enumerate() {
                FileSection {
                    key: "{section_key(&source, &files[index])}",
                    id,
                    cwd: cwd.clone(),
                    root: root.clone(),
                    commentable: can_comment,
                    comments: if can_comment { comments_of(&cards, &absolute_path(&root, &files[index].file.path)) } else { Vec::new() },
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

/// What the rows need to offer comments on the new side of the diff. It exists only when the new
/// side is the working tree (`commentable`).
#[derive(Clone)]
struct CommentEnv {
    id: LiveSessionId,
    /// The file's absolute path: the key of its comment cards.
    path: String,
    cards: Vec<Card>,
    /// The line whose empty box is open.
    open: Option<u32>,
    on_open: EventHandler<u32>,
    on_close: EventHandler<()>,
}

/// The line a `+` on this row would comment on: the new number of an added or a context line; in
/// side by side only the right half has one.
fn offers_comment(line: &PatchLine, right: bool, side_by_side: bool) -> Option<u32> {
    if side_by_side && !right {
        return None;
    }
    comment_line(line)
}

/// The round `+` in the row's left margin (shown while the pointer is over the row).
fn add_button(line: Option<u32>, env: Option<&CommentEnv>) -> Element {
    let (Some(line), Some(env)) = (line, env) else { return rsx! {} };
    let on_open = env.on_open;
    rsx! {
        button {
            class: "cd-add-comment",
            r#type: "button",
            title: tr!("Add comment"),
            "aria-label": tr!("Add comment"),
            onclick: move |event: MouseEvent| {
                event.stop_propagation();
                on_open.call(line);
            },
            "+"
        }
    }
}

/// The saved comments of a line and, when its `+` was clicked, an empty box, under the row.
fn comment_slot(line: Option<u32>, env: Option<&CommentEnv>) -> Element {
    let (Some(line), Some(env)) = (line, env) else { return rsx! {} };
    let saved = comments_on(&env.cards, &env.path, line);
    if saved.is_empty() && env.open != Some(line) {
        return rsx! {};
    }
    let (id, path, on_close) = (env.id, env.path.clone(), env.on_close);
    let open_here = env.open == Some(line);
    rsx! {
        div { class: "cd-comment-slot",
            for card in saved {
                DiffCommentBox { key: "c{card.id}", id, path: path.clone(), line, card: Some(card), on_done: move |_| {} }
            }
            if open_here {
                DiffCommentBox { key: "new{line}", id, path: path.clone(), line, card: None, on_done: on_close }
            }
        }
    }
}

fn line_row(line: &PatchLine, spans: &[Span], highlight: bool, env: Option<&CommentEnv>) -> Element {
    let pieces = if highlight && !spans.is_empty() { split_by_spans(&line.text, spans) } else { vec![(line.text.clone(), false)] };
    let target = env.and_then(|_| offers_comment(line, false, false));
    rsx! {
        div { class: "diff-line {line_class(line.kind)}",
            {add_button(target, env)}
            span { class: "cd-no", "{number(forge_features::line_number(line))}" }
            span { class: "diff-sign", "{sign(line.kind)}" }
            span { class: "diff-text",
                for (piece , hot) in pieces {
                    span { class: if hot { "cd-word" } else { "" }, "{piece}" }
                }
            }
        }
        {comment_slot(target, env)}
    }
}

// The number a side by side half shows: the left half the old line number, the right half the new
// one (a context row is in both halves and must not show the old number on the right).
fn cell_number(line: &PatchLine, right: bool) -> Option<u32> {
    if right {
        line.new_no
    } else {
        line.old_no
    }
}

fn cell(line: Option<&PatchLine>, right: bool, spans: &[Span], highlight: bool, env: Option<&CommentEnv>) -> Element {
    match line {
        Some(line) => {
            let pieces = if highlight && !spans.is_empty() { split_by_spans(&line.text, spans) } else { vec![(line.text.clone(), false)] };
            let number = cell_number(line, right);
            let target = env.and_then(|_| offers_comment(line, right, true));
            rsx! {
                div { class: "diff-line {line_class(line.kind)}",
                    {add_button(target, env)}
                    span { class: "cd-no", "{self::number(number)}" }
                    span { class: "diff-sign", "{sign(line.kind)}" }
                    span { class: "diff-text",
                        for (piece , hot) in pieces {
                            span { class: if hot { "cd-word" } else { "" }, "{piece}" }
                        }
                    }
                }
                {comment_slot(target, env)}
            }
        }
        None => rsx! { div { class: "diff-line empty" } },
    }
}

fn hunk_view(hunk: &PatchHunk, prefs: &ChangesViewPrefs, env: Option<&CommentEnv>) -> Element {
    let spans = if prefs.highlight_words { paired_spans(&hunk.lines) } else { Vec::new() };
    let none: Vec<Span> = Vec::new();
    let spans_of = |index: usize| spans.get(index).unwrap_or(&none);
    if prefs.side_by_side {
        let rows = side_by_side(&hunk.lines);
        rsx! {
            div { class: "cd-hunk cd-sbs",
                for (left , right) in rows {
                    {cell(left.map(|i| &hunk.lines[i]), false, left.map(spans_of).unwrap_or(&none), prefs.highlight_words, env)}
                    {cell(right.map(|i| &hunk.lines[i]), true, right.map(spans_of).unwrap_or(&none), prefs.highlight_words, env)}
                }
            }
        }
    } else {
        rsx! {
            div { class: "cd-hunk",
                for (index , line) in hunk.lines.iter().enumerate() {
                    {line_row(line, spans_of(index), prefs.highlight_words, env)}
                }
            }
        }
    }
}

#[component]
fn FileSection(
    id: LiveSessionId,
    cwd: PathBuf,
    root: PathBuf,
    commentable: bool,
    comments: Vec<Card>,
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
    let counts = forge_features::header_counts(&change.file);
    // A file that did not exist before is all in its one hunk: nothing follows it.
    let created = hunks.first().is_some_and(|hunk| hunk.old_start == 0 && hunk.lines.iter().all(|line| line.kind == DiffLineKind::Added));
    let wrap_class = if prefs.wrap { "cd-lines" } else { "cd-lines nowrap" };

    let mut open_comment = use_signal(|| None::<u32>);
    let env = commentable.then(|| CommentEnv {
        id,
        path: absolute_path(&root, &path),
        cards: comments.clone(),
        open: open_comment(),
        on_open: EventHandler::new(move |line| open_comment.set(Some(line))),
        on_close: EventHandler::new(move |_| open_comment.set(None)),
    });

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
                                        loaded.set(Some(result.map_err(|error| match error {
                                            GitError::Failed(text) if text == TOO_LARGE_TO_SHOW => tr!("This file is too large to show").to_string(),
                                            other => other.to_string(),
                                        })));
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
                        {hunk_view(hunk, &prefs, env.as_ref())}
                        if hidden > 0 && index + 1 == shown.len() {
                            // The lines after the cut are not drawn, so there is no gap to open.
                        } else if let Some(lines) = gaps.read().get(&index) {
                            div { class: "cd-hunk",
                                for line in lines.iter() {
                                    {line_row(line, &[], false, env.as_ref())}
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
                                        span { class: "cd-chevron open", Icon { kind: IconKind::Chevron, size: IconSize::Small } }
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
                            span { class: "cd-chevron open", Icon { kind: IconKind::Chevron, size: IconSize::Small } }
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
                onkeydown: {
                    let path = path.clone();
                    move |event: KeyboardEvent| {
                        if fold_key(&event.key()) {
                            event.prevent_default();
                            workspace.update_changes_session(id, |session| toggle_fold(session, &path, folded));
                        }
                    }
                },
                oncontextmenu: {
                    let path = path.clone();
                    move |event: MouseEvent| {
                        event.prevent_default();
                        let point = event.client_coordinates();
                        on_menu.call((path.clone(), deleted, point.x, point.y));
                    }
                },
                span { class: if folded { "cd-chevron" } else { "cd-chevron open" }, Icon { kind: IconKind::Chevron, size: IconSize::Small } }
                FileTypeIcon { icon: icon_of(&path) }
                span { class: "cd-name", "{name}" }
                if !folder.is_empty() {
                    span { class: "cd-folder", "{folder}" }
                }
                if let Some((added, removed)) = counts {
                    span { class: "cd-counts",
                        span { class: "cd-add", "+{added}" }
                        " "
                        span { class: "cd-del", "−{removed}" }
                    }
                }
            }
            {body}
        }
    }
}

/// Enter and Space fold a file header, as a click does.
fn fold_key(key: &Key) -> bool {
    matches!(key, Key::Enter) || matches!(key, Key::Character(text) if text == " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(kind: DiffLineKind, old_no: Option<u32>, new_no: Option<u32>) -> PatchLine {
        PatchLine { kind, text: String::new(), old_no, new_no }
    }

    #[test]
    fn enter_and_space_fold_a_file_other_keys_do_not() {
        assert!(fold_key(&Key::Enter));
        assert!(fold_key(&Key::Character(" ".to_string())));
        assert!(!fold_key(&Key::Character("a".to_string())));
        assert!(!fold_key(&Key::Tab));
    }

    #[test]
    fn each_half_shows_its_own_line_numbers() {
        // @@ -10,3 +10,5 @@: context, two added lines, context.
        let rows = [
            line(DiffLineKind::Context, Some(10), Some(10)),
            line(DiffLineKind::Added, None, Some(11)),
            line(DiffLineKind::Added, None, Some(12)),
            line(DiffLineKind::Context, Some(11), Some(13)),
        ];
        let right: Vec<_> = rows.iter().map(|row| cell_number(row, true)).collect();
        assert_eq!(right, vec![Some(10), Some(11), Some(12), Some(13)]);
        let left: Vec<_> = rows.iter().map(|row| cell_number(row, false)).collect();
        assert_eq!(left, vec![Some(10), None, None, Some(11)]);
        let removed = line(DiffLineKind::Removed, Some(5), None);
        assert_eq!(cell_number(&removed, false), Some(5));
        assert_eq!(cell_number(&removed, true), None);
    }

    #[test]
    fn a_plus_is_offered_on_added_and_context_lines_but_not_on_removed_ones() {
        let added = line(DiffLineKind::Added, None, Some(11));
        let context = line(DiffLineKind::Context, Some(10), Some(10));
        let removed = line(DiffLineKind::Removed, Some(5), None);
        assert_eq!(offers_comment(&added, false, false), Some(11));
        assert_eq!(offers_comment(&context, false, false), Some(10));
        assert_eq!(offers_comment(&removed, false, false), None);
    }

    #[test]
    fn a_revealed_gap_line_can_be_commented() {
        let hunks = parse_patch("@@ -2,1 +2,2 @@\n a\n+b\n@@ -20,1 +21,1 @@\n x\n");
        let file: Vec<String> = (1..=30).map(|n| format!("line {n}")).collect();
        let lines = gap_lines(&hunks, 0, &file);
        assert!(!lines.is_empty());
        assert_eq!(offers_comment(&lines[0], false, false), lines[0].new_no);
        assert!(offers_comment(&lines[0], false, false).is_some());
    }

    #[test]
    fn side_by_side_offers_it_in_the_right_half_only() {
        let context = line(DiffLineKind::Context, Some(10), Some(10));
        assert_eq!(offers_comment(&context, true, true), Some(10));
        assert_eq!(offers_comment(&context, false, true), None);
    }
}
