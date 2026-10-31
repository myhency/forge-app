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
            SessionMarker::NeedsAttention => Tone::Warning,
        }
    }
}

#[component]
pub fn SidebarItem(
    title: String,
    subtitle: String,
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
            span { class: "title", "{title}" }
            span { class: "subtitle", "{subtitle}" }
            {children}
        }
    }
}
