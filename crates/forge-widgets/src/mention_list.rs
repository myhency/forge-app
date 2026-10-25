use dioxus::prelude::*;
use forge_entities::icon_of;
use forge_features::{use_composer_state, MentionStatus};
use forge_shared::tr;

use crate::files_icons::FileTypeIcon;

// Keeps the highlighted row in sight after the arrows moved it past the visible eight.
const SHOW_HIGHLIGHT: &str =
    "requestAnimationFrame(() => document.querySelector('.pane.focused .mention-item.selected')?.scrollIntoView({ block: 'nearest' }));";

/// The `@` mention list above the composer: the session folder's files matching what follows the
/// `@`, each with its type icon, name and folder; a tick on a file already attached. The keys go
/// through the text area (`CommandTextArea`); a click picks.
#[component]
pub fn MentionList() -> Element {
    let composer = use_composer_state();
    let view = composer.mention();
    let highlight = view.as_ref().map(|view| view.highlight);
    use_effect(use_reactive!(|highlight| {
        if highlight.is_some() {
            document::eval(SHOW_HIGHLIGHT);
        }
    }));
    let Some(view) = view else {
        return rsx! {};
    };
    let note = match view.status {
        MentionStatus::Loading => Some(tr!("Loading…")),
        MentionStatus::Empty => Some(tr!("No files in this folder")),
        MentionStatus::NoMatch => Some(tr!("No files match")),
        MentionStatus::Listed { .. } => None,
    };
    rsx! {
        div { class: "mention-menu", role: "listbox",
            for (index, row) in view.rows.iter().enumerate() {
                div {
                    key: "{row.path}",
                    role: "option",
                    "aria-selected": "{index == view.highlight}",
                    class: match (index == view.highlight, row.attached) {
                        (true, true) => "mention-item selected attached",
                        (true, false) => "mention-item selected",
                        (false, true) => "mention-item attached",
                        (false, false) => "mention-item",
                    },
                    title: "{row.path}",
                    // mousedown, not click: a click would first take the focus from the text box.
                    onmousedown: move |event: MouseEvent| {
                        event.prevent_default();
                        composer.mention_pick(index);
                    },
                    FileTypeIcon { icon: icon_of(&row.path) }
                    span { class: "mention-name", "{row.name}" }
                    span { class: "mention-folder", "{row.folder}" }
                    if row.attached {
                        span { class: "mention-tick", "aria-label": "Attached", "✓" }
                    }
                }
            }
            if let Some(note) = note {
                div { class: "mention-note", "{note}" }
            }
            if view.status == (MentionStatus::Listed { cut: true }) {
                div { class: "mention-note", {tr!("Only some files are listed.")} }
            }
        }
    }
}
