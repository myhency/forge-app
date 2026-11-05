use dioxus::prelude::*;
use forge_shared::{tr, Icon, IconKind, IconSize, StatusDot, Tone};

/// What the marker beside a sidebar entry says. Defined here (not in `forge-application`) so this
/// crate depends only on the domain and the UI kit; a widget maps a session's phase onto it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionMarker {
    Idle,
    Running,
    NeedsAttention,
}

impl SessionMarker {
    fn tone(self) -> Tone {
        match self {
            SessionMarker::Idle => Tone::Muted,
            SessionMarker::Running => Tone::Active,
            // Still a running session; the orange dot at the row's right end says it waits (the
            // desktop app's row, round 37).
            SessionMarker::NeedsAttention => Tone::Active,
        }
    }
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
            StatusDot { tone: marker.tone() }
            if archived {
                span { class: "archived-mark", title: tr!("Archived"), Icon { kind: IconKind::Archive, size: IconSize::Small } }
            }
            div { class: "item-text",
                span { class: "title", "{title}" }
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
            if marker == SessionMarker::NeedsAttention {
                span { class: "attention-dot", title: tr!("Needs input") }
            }
            {children}
        }
    }
}
