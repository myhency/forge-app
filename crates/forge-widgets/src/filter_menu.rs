use dioxus::prelude::*;
use forge_features::{
    current_index, filter_key, group_label, refocus_workspace, sort_label, use_workspace, FilterChoice, FilterMenu,
    FilterOutcome, FilterRow, FILTER_ROWS, GROUP_ITEMS, SORT_ITEMS,
};

/// The icon beside "+ New session" and the menu it opens: sort by, group by, show empty groups.
/// The menu takes focus when it opens; arrows move, Right opens a submenu, Left or Escape closes
/// it, Enter chooses (the keys are the physical ones, so a Korean input method does not matter).
#[component]
pub fn SidebarFilterMenu() -> Element {
    let workspace = use_workspace();
    let mut open = use_signal(|| false);
    let mut menu = use_signal(FilterMenu::default);
    let prefs = workspace.prefs();
    let state = menu();

    let sub_items: Vec<(String, bool, FilterChoice)> = match FILTER_ROWS[state.top] {
        FilterRow::Sort => SORT_ITEMS
            .iter()
            .map(|item| (sort_label(*item).to_string(), prefs.sort_by == *item, FilterChoice::Sort(*item)))
            .collect(),
        FilterRow::Group => GROUP_ITEMS
            .iter()
            .map(|item| (group_label(*item).to_string(), prefs.group_by == *item, FilterChoice::Group(*item)))
            .collect(),
        FilterRow::ShowEmpty => Vec::new(),
    };

    rsx! {
        div { class: "filter-wrap",
            button {
                class: if open() { "icon-button open" } else { "icon-button" },
                title: "Sort, group and filter sessions",
                onclick: move |_| {
                    menu.set(FilterMenu::default());
                    open.set(!open());
                },
                "⇅"
            }
            if open() {
                div {
                    class: "menu-backdrop",
                    onclick: move |_| {
                        open.set(false);
                        refocus_workspace();
                    },
                }
                div {
                    class: "row-menu filter-menu",
                    tabindex: "0",
                    onclick: move |event| event.stop_propagation(),
                    onmounted: move |mounted| async move {
                        let _ = mounted.set_focus(true).await;
                    },
                    onkeydown: move |event: KeyboardEvent| {
                        let modified = event
                            .modifiers()
                            .intersects(Modifiers::CONTROL | Modifiers::META | Modifiers::ALT);
                        let Some(key) = filter_key(&event.code().to_string()).filter(|_| !modified) else {
                            return;
                        };
                        event.stop_propagation();
                        event.prevent_default();
                        let mut next = menu();
                        let outcome = next.key(key, &workspace.prefs());
                        menu.set(next);
                        match outcome {
                            FilterOutcome::Stay => {}
                            FilterOutcome::Choose(choice) => {
                                workspace.update_prefs(|prefs| choice.apply(prefs));
                                open.set(false);
                                refocus_workspace();
                            }
                            FilterOutcome::Close => {
                                open.set(false);
                                refocus_workspace();
                            }
                        }
                    },
                    for (index, row) in FILTER_ROWS.iter().enumerate() {
                        button {
                            key: "{index}",
                            class: if state.top == index { "row-menu-item highlight" } else { "row-menu-item" },
                            onclick: {
                                let row = *row;
                                move |_| {
                                    if row == FilterRow::ShowEmpty {
                                        workspace.update_prefs(|prefs| FilterChoice::ToggleShowEmpty.apply(prefs));
                                        open.set(false);
                                        refocus_workspace();
                                    } else {
                                        menu.set(FilterMenu { top: index, sub: Some(current_index(row, &workspace.prefs())) });
                                    }
                                }
                            },
                            match row {
                                FilterRow::Sort => rsx! {
                                    span { "Sort by" }
                                    span { class: "value", "{sort_label(prefs.sort_by)} ›" }
                                },
                                FilterRow::Group => rsx! {
                                    span { "Group by" }
                                    span { class: "value", "{group_label(prefs.group_by)} ›" }
                                },
                                FilterRow::ShowEmpty => rsx! {
                                    span { "Show empty groups" }
                                    span { class: "value", if prefs.show_empty { "✓" } }
                                },
                            }
                        }
                    }
                    if let Some(highlighted) = state.sub {
                        div { class: "filter-submenu",
                            for (index, (label, current, choice)) in sub_items.into_iter().enumerate() {
                                button {
                                    key: "{index}",
                                    class: if highlighted == index { "row-menu-item highlight" } else { "row-menu-item" },
                                    onclick: move |_| {
                                        workspace.update_prefs(|prefs| choice.apply(prefs));
                                        open.set(false);
                                        refocus_workspace();
                                    },
                                    span { "{label}" }
                                    span { class: "value", if current { "✓" } }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
