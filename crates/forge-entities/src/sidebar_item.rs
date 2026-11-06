use dioxus::prelude::*;
use forge_shared::{tr, Icon, IconKind, IconSize};

/// What the marker beside a sidebar entry says. Defined here (not in `forge-application`) so this
/// crate depends only on the domain and the UI kit; a widget maps a session's phase onto it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionMarker {
    Idle,
    Running,
    NeedsAttention,
}

#[component]
pub fn SidebarItem(
    title: String,
    /// The folder under the title (empty: none). Cut short before the branch is.
    subtitle: String,
    /// The branch of a worktree session, drawn before the folder with a branch mark.
    #[props(default)] branch: Option<String>,
    marker: SessionMarker,
    active: bool,
    /// Shown in the other pane of a split screen: a softer highlight than `active`.
    #[props(default)] shown: bool,
    /// An archived session wears a small archive mark before its title.
    #[props(default)] archived: bool,
    /// Claude has not named the session yet: a shimmering bar stands in for the title.
    #[props(default)] naming: bool,
    /// A pinned row (the `고정됨` group) wears a code mark before its title, as in the desktop app.
    #[props(default)] pinned: bool,
    /// A reply the person has not looked at: a blue dot at the row's right end (the desktop app's).
    #[props(default)] unread: bool,
    onclick: EventHandler<MouseEvent>,
    #[props(default)] children: Element,
) -> Element {
    let class = match (active, shown) {
        (true, _) => "sidebar-item active",
        (false, true) => "sidebar-item shown",
        (false, false) => "sidebar-item",
    };
    rsx! {
        div { class, onclick: move |evt| onclick.call(evt),
            if pinned {
                span { class: "pinned-mark", Icon { kind: IconKind::Code, size: IconSize::Small } }
            }
            if archived {
                span { class: "archived-mark", title: tr!("Archived"), Icon { kind: IconKind::Archive, size: IconSize::Small } }
            }
            div { class: "item-text",
                if naming {
                    span { class: "title-skeleton", title: "{title}", "aria-label": "{title}" }
                } else {
                    span { class: "title", "{title}" }
                }
                if branch.is_some() || !subtitle.is_empty() {
                    div { class: "item-meta",
                        if let Some(branch) = branch {
                            span { class: "item-branch", title: "{branch}",
                                Icon { kind: IconKind::Fork, size: IconSize::Small }
                                span { class: "branch-name", "{branch}" }
                            }
                        }
                        if !subtitle.is_empty() {
                            span { class: "subtitle", "{subtitle}" }
                        }
                    }
                }
            }
            // The desktop app's rows have no mark at the left (round 37): an orange dot at the right
            // while the session waits for the person, a grey one while a new session is being named.
            if marker == SessionMarker::NeedsAttention {
                span { class: "attention-dot", title: tr!("Needs input") }
            } else if naming {
                span { class: "attention-dot naming" }
            } else if unread {
                span { class: "attention-dot unread", title: tr!("Unread") }
            }
            {children}
        }
    }
}
