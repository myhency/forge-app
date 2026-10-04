use dioxus::prelude::*;
use forge_domain::EditorPrefs;
use forge_features::use_workspace;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    Tree,
    PreviewTabs,
    AutoSave,
    TabIndent,
    Wrap,
    Guides,
    Whitespace,
    ScrollPastEnd,
}

impl Item {
    fn label(self) -> &'static str {
        match self {
            Item::Tree => "Show file tree",
            Item::PreviewTabs => "Preview tabs",
            Item::AutoSave => "Auto save",
            Item::TabIndent => "Tab key indents",
            Item::Wrap => "Word wrap",
            Item::Guides => "Indent guides",
            Item::Whitespace => "Show whitespace",
            Item::ScrollPastEnd => "Scroll past end",
        }
    }

    fn is_on(self, prefs: &EditorPrefs) -> bool {
        match self {
            Item::Tree => prefs.tree,
            Item::PreviewTabs => prefs.preview_tabs,
            Item::AutoSave => prefs.auto_save,
            Item::TabIndent => prefs.tab_indent,
            Item::Wrap => prefs.wrap,
            Item::Guides => prefs.indent_guides,
            Item::Whitespace => prefs.whitespace,
            Item::ScrollPastEnd => prefs.scroll_past_end,
        }
    }

    fn toggle(self, prefs: &mut EditorPrefs) {
        let field = match self {
            Item::Tree => &mut prefs.tree,
            Item::PreviewTabs => &mut prefs.preview_tabs,
            Item::AutoSave => &mut prefs.auto_save,
            Item::TabIndent => &mut prefs.tab_indent,
            Item::Wrap => &mut prefs.wrap,
            Item::Guides => &mut prefs.indent_guides,
            Item::Whitespace => &mut prefs.whitespace,
            Item::ScrollPastEnd => &mut prefs.scroll_past_end,
        };
        *field = !*field;
    }
}

// The menu's items in groups; a divider sits between groups.
const GROUPS: [&[Item]; 3] = [
    &[Item::Tree],
    &[Item::PreviewTabs, Item::AutoSave, Item::TabIndent],
    &[Item::Wrap, Item::Guides, Item::Whitespace, Item::ScrollPastEnd],
];

/// The `⋮` menu in the Files panel header: the editor settings. A tick click keeps the menu open
/// and takes effect at once. `Tab size` opens a small menu to the left.
#[component]
pub fn FilesMenu() -> Element {
    let workspace = use_workspace();
    let mut open = use_signal(|| false);
    let mut sizes = use_signal(|| false);
    let prefs = workspace.editor_prefs();
    let tab_size = prefs.clamped_tab_size();
    rsx! {
        div { class: "picker",
            button {
                class: if open() { "picker-button open" } else { "picker-button" },
                r#type: "button",
                title: "Editor settings",
                "aria-label": "Editor settings",
                "aria-haspopup": "menu",
                "aria-expanded": "{open()}",
                onclick: move |_| {
                    sizes.set(false);
                    open.set(!open());
                },
                "⋮"
            }
            if open() {
                div { class: "picker-backdrop", onclick: move |_| open.set(false) }
                div {
                    class: "picker-menu down",
                    role: "menu",
                    tabindex: "0",
                    onmounted: move |mounted| async move {
                        let _ = mounted.set_focus(true).await;
                    },
                    onkeydown: move |event: KeyboardEvent| {
                        if event.key() == Key::Escape {
                            event.stop_propagation();
                            open.set(false);
                        }
                    },
                    for (index, group) in GROUPS.iter().enumerate() {
                        if index > 0 {
                            div { class: "picker-divider" }
                        }
                        for item in group.iter().copied() {
                            button {
                                class: "picker-item",
                                r#type: "button",
                                role: "menuitemcheckbox",
                                "aria-checked": "{item.is_on(&prefs)}",
                                onclick: move |_| workspace.update_editor_prefs(|prefs| item.toggle(prefs)),
                                span { class: "picker-check", if item.is_on(&prefs) { "✓" } }
                                span { class: "picker-text", "{item.label()}" }
                            }
                        }
                    }
                    div { class: "picker-divider" }
                    div { class: "files-menu-row",
                        button {
                            class: "picker-item",
                            r#type: "button",
                            "aria-haspopup": "menu",
                            "aria-expanded": "{sizes()}",
                            onclick: move |_| sizes.set(!sizes()),
                            span { class: "picker-check" }
                            span { class: "picker-text", "Tab size" }
                            span { class: "menu-key", "{tab_size} ›" }
                        }
                        if sizes() {
                            div { class: "picker-menu files-menu-sub", role: "menu",
                                for size in [2u8, 4, 8] {
                                    button {
                                        class: "picker-item",
                                        r#type: "button",
                                        role: "menuitemradio",
                                        "aria-checked": "{size == tab_size}",
                                        onclick: move |_| {
                                            workspace.update_editor_prefs(|prefs| prefs.tab_size = size);
                                            sizes.set(false);
                                        },
                                        span { class: "picker-check", if size == tab_size { "✓" } }
                                        span { class: "picker-text", "{size}" }
                                    }
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
    use super::*;

    const ALL: [Item; 8] = [
        Item::Tree,
        Item::PreviewTabs,
        Item::AutoSave,
        Item::TabIndent,
        Item::Wrap,
        Item::Guides,
        Item::Whitespace,
        Item::ScrollPastEnd,
    ];

    #[test]
    fn each_menu_item_toggles_its_own_setting_and_nothing_else() {
        for item in ALL {
            let before = EditorPrefs::default();
            let mut after = before.clone();
            let was = item.is_on(&before);
            item.toggle(&mut after);
            assert_eq!(item.is_on(&after), !was, "{}", item.label());
            for other in ALL.into_iter().filter(|other| *other != item) {
                assert_eq!(other.is_on(&after), other.is_on(&before), "{} changed {}", item.label(), other.label());
            }
            assert_eq!(after.tab_size, before.tab_size);
        }
    }

    #[test]
    fn the_defaults_show_the_ticks_the_real_app_shows() {
        let prefs = EditorPrefs::default();
        let on: Vec<&str> = ALL.into_iter().filter(|item| item.is_on(&prefs)).map(Item::label).collect();
        assert_eq!(on, vec!["Show file tree", "Preview tabs", "Auto save", "Tab key indents", "Word wrap", "Indent guides"]);
    }
}
