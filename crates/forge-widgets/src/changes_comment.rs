//! The box under a diff line of the Changes panel: a comment being typed, or a saved one. A comment
//! is the card the Files panel makes too (`Workspace::add_comment`), so it rides with the next message
//! and shows in both panels. See the spec `2026-10-17-changes-panel-2-design.md`, stage 50B.

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::{Card, LineRange};
use forge_features::{comment_key, comment_text, input_confirms, recently_composed, use_workspace, CommentKey, Workspace};
use forge_shared::tr;
use std::time::Instant;

/// What saving a comment asks of the card list. A value, so the decision can be tested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Store {
    Add { path: String, line: u32, text: String },
    Update { card_id: u64, text: String },
}

/// Decides what saving `raw` does: nothing for a blank text, an update for a saved card, an add
/// for a new one. `run` performs it. True when something was saved.
pub(crate) fn apply(path: &str, line: u32, card: &Option<Card>, raw: &str, mut run: impl FnMut(Store)) -> bool {
    let Some(text) = comment_text(raw) else { return false };
    match card {
        Some(card) => run(Store::Update { card_id: card.id, text }),
        None => run(Store::Add { path: path.to_string(), line, text }),
    }
    true
}

/// Saves `raw` through the workspace's cards.
pub(crate) fn store(workspace: Workspace, id: LiveSessionId, path: &str, line: u32, card: &Option<Card>, raw: &str) -> bool {
    apply(path, line, card, raw, |action| match action {
        Store::Add { path, line, text } => {
            workspace.add_comment(id, path, LineRange::new(line, line), text);
        }
        Store::Update { card_id, text } => workspace.update_comment(id, card_id, text),
    })
}

/// `card: None` is a new comment being typed: `on_done` runs when it is saved or cancelled, so the
/// slot that holds it can close (a saved comment then comes back from the cards as a `Some` box).
/// `card: Some(..)` is a saved comment with a pencil and a trash.
#[component]
pub(crate) fn DiffCommentBox(id: LiveSessionId, path: String, line: u32, card: Option<Card>, on_done: EventHandler<()>) -> Element {
    let workspace = use_workspace();
    let mut text = use_signal(|| card.as_ref().map(|card| card.text.clone()).unwrap_or_default());
    // When an input method last confirmed text without changing it: the webview reports no composition
    // events here, and the keydown of the Enter that confirms a syllable arrives just after that input.
    let mut composed_at = use_signal(|| None::<Instant>);
    let mut editing = use_signal(|| card.is_none());
    let title = tr!("Line {n}", n = line);
    let saved_text = card.as_ref().map(|card| card.text.clone()).unwrap_or_default();

    // The one way out of the field: save (Enter or the round icon), cancel (Escape).
    let finish = {
        let (path, card) = (path.clone(), card.clone());
        move |save: bool| {
            if save {
                if store(workspace, id, &path, line, &card, &text()) {
                    editing.set(false);
                    if card.is_none() {
                        on_done.call(());
                    }
                }
            } else if let Some(card) = &card {
                text.set(card.text.clone());
                editing.set(false);
            } else {
                on_done.call(());
            }
        }
    };
    let (mut finish_key, mut finish_icon) = (finish.clone(), finish);

    rsx! {
        div { class: if editing() { "cd-comment editing" } else { "cd-comment saved" },
            div { class: "cd-comment-title", "{title}" }
            if editing() {
                textarea {
                    class: "cd-comment-input",
                    rows: "2",
                    placeholder: tr!("Tell Claude what to change"),
                    value: "{text}",
                    oninput: move |event| {
                        let value = event.value();
                        if input_confirms(&text(), &value) {
                            composed_at.set(Some(Instant::now()));
                        }
                        text.set(value);
                    },
                    onmounted: move |mounted| async move {
                        let _ = mounted.set_focus(true).await;
                    },
                    onkeydown: move |event: KeyboardEvent| {
                        let key = event.key();
                        match comment_key(key == Key::Enter, key == Key::Escape, event.modifiers().shift(), event.is_composing(), recently_composed(composed_at(), Instant::now())) {
                            CommentKey::Save => {
                                event.prevent_default();
                                finish_key(true);
                            }
                            CommentKey::Cancel => {
                                // The panel's own Escape handling (closing a menu) must not also run.
                                event.stop_propagation();
                                finish_key(false);
                            }
                            CommentKey::Ignore => {}
                        }
                    },
                }
                button {
                    class: "cd-comment-ok",
                    r#type: "button",
                    title: tr!("Add comment"),
                    "aria-label": tr!("Add comment"),
                    onclick: move |_| finish_icon(true),
                    "↑"
                }
            } else {
                div { class: "cd-comment-text", onclick: move |_| editing.set(true), "{saved_text}" }
                if let Some(card) = card.clone() {
                    button {
                        class: "cd-comment-edit",
                        r#type: "button",
                        title: tr!("Edit comment"),
                        "aria-label": tr!("Edit comment"),
                        onclick: move |_| editing.set(true),
                        svg { view_box: "0 0 16 16", fill: "none", stroke: "currentColor", stroke_width: "1.5", stroke_linecap: "round", stroke_linejoin: "round", path { d: "M2 14l1-4 8-8 3 3-8 8z" } }
                    }
                    button {
                        class: "cd-comment-delete",
                        r#type: "button",
                        title: tr!("Delete comment"),
                        "aria-label": tr!("Delete comment"),
                        onclick: move |_| workspace.remove_card(id, card.id),
                        svg { view_box: "0 0 16 16", fill: "none", stroke: "currentColor", stroke_width: "1.5", stroke_linecap: "round", stroke_linejoin: "round", path { d: "M2.5 4h11M6 4V2.5h4V4M4 4l.7 9.5h6.6L12 4" } }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::{CardKind, LineRange};

    fn saved() -> Card {
        Card { id: 9, kind: CardKind::Comment, path: "/r/a.rs".to_string(), range: LineRange::new(3, 3), text: "old".to_string() }
    }

    #[test]
    fn a_blank_text_stores_nothing() {
        let mut calls = Vec::new();
        let kept = apply("/r/a.rs", 3, &None, "  \n", |c| calls.push(c));
        assert!(!kept);
        assert!(calls.is_empty());
        let kept = apply("/r/a.rs", 3, &Some(saved()), "", |c| calls.push(c));
        assert!(!kept);
        assert!(calls.is_empty());
    }

    #[test]
    fn a_new_text_is_added_on_its_line_and_path() {
        let mut calls = Vec::new();
        assert!(apply("/r/a.rs", 3, &None, " fix it ", |c| calls.push(c)));
        assert_eq!(calls, vec![Store::Add { path: "/r/a.rs".to_string(), line: 3, text: "fix it".to_string() }]);
    }

    #[test]
    fn a_text_for_a_saved_card_updates_that_card() {
        let mut calls = Vec::new();
        assert!(apply("/r/a.rs", 3, &Some(saved()), "new", |c| calls.push(c)));
        assert_eq!(calls, vec![Store::Update { card_id: 9, text: "new".to_string() }]);
    }
}
