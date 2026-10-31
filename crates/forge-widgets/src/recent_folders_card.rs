use dioxus::prelude::*;
use forge_features::{recent_folder_card_rows, refocus_composer, use_new_session_draft, use_workspace};
use forge_shared::{tr, Icon, IconKind, IconSize};

/// The card above the new-session box: the recent folders to start in with one press, and "Choose a folder"
/// below them for the system dialog. It is not shown with an environment chosen or with no folder to list.
#[component]
pub fn RecentFoldersCard() -> Element {
    let workspace = use_workspace();
    let draft = use_new_session_draft();
    if draft.environment.read().is_some() {
        return rsx! {};
    }
    let rows = recent_folder_card_rows(&workspace.new_session_projects(), workspace.chosen_new_folder().as_deref());
    if rows.is_empty() {
        return rsx! {};
    }
    rsx! {
        div { class: "recent-folders", role: "menu", "aria-label": tr!("Recent"),
            for row in rows {
                button {
                    class: "picker-item",
                    r#type: "button",
                    role: "menuitemradio",
                    "aria-checked": "{row.checked}",
                    onclick: {
                        let path = row.path.clone();
                        move |_| {
                            workspace.set_new_folder(Some(path.clone()));
                            refocus_composer();
                        }
                    },
                    Icon { kind: IconKind::Folder }
                    span { class: "picker-text", "{row.label}" }
                    if row.checked {
                        span { class: "picker-check", Icon { kind: IconKind::Check } }
                    }
                }
            }
            div { class: "picker-divider" }
            button {
                class: "picker-item more",
                r#type: "button",
                role: "menuitem",
                disabled: workspace.picking_folder(),
                onclick: move |_| workspace.open_folder(),
                span { class: "picker-text", {tr!("Choose a folder")} }
                Icon { kind: IconKind::Chevron, size: IconSize::Small, open: true }
            }
        }
    }
}
