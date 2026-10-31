use dioxus::prelude::*;
use forge_features::{environment_card_rows, refocus_composer, use_new_session_draft, use_workspace, PickerKind};
use forge_shared::{tr, Icon, IconKind, IconSize};

use crate::environment_chip::apply_environment;

/// The card above the new-session box: a few saved environments to start from with one press, and "Choose an
/// environment" below them for the full menu of the chip under the box. It is a shortcut into that list, so a
/// row need not be the environment the box is on. It is not shown while nothing is saved.
#[component]
pub fn EnvironmentCard() -> Element {
    let workspace = use_workspace();
    let draft = use_new_session_draft();
    let environments = workspace.environments();
    let rows = environment_card_rows(&environments);
    if rows.is_empty() {
        return rsx! {};
    }
    let picked = draft.environment.read().clone();
    rsx! {
        div { class: "env-shortcut", role: "menu", "aria-label": tr!("Environment"),
            for environment in rows {
                button {
                    class: "picker-item",
                    r#type: "button",
                    role: "menuitemradio",
                    "aria-checked": "{picked.as_deref() == Some(environment.id.as_str())}",
                    title: environment.folders.first().map(|folder| folder.display().to_string()).unwrap_or_default(),
                    onclick: {
                        let id = environment.id.clone();
                        move |_| {
                            apply_environment(workspace, draft, Some(id.clone()));
                            refocus_composer();
                        }
                    },
                    Icon { kind: IconKind::Laptop }
                    span { class: "picker-text", "{environment.name}" }
                    if picked.as_deref() == Some(environment.id.as_str()) {
                        span { class: "picker-check", Icon { kind: IconKind::Check } }
                    }
                }
            }
            div { class: "picker-divider" }
            button {
                class: "picker-item more",
                r#type: "button",
                role: "menuitem",
                onclick: move |_| workspace.request_picker(PickerKind::Environment),
                span { class: "picker-text", {tr!("Choose an environment")} }
                Icon { kind: IconKind::Chevron, size: IconSize::Small, open: true }
            }
        }
    }
}
