use dioxus::prelude::*;
use forge_shared::{tr, Icon, IconKind, IconSize};
use forge_features::{
    current_index, direction_label, display_label, display_on, filter_key, group_label, refocus_workspace, sort_label, use_workspace,
    FilterChoice, FilterMenu, FilterOutcome, FilterRow, DISPLAY_ITEMS, FILTER_ROWS, GROUP_DIVIDER_BEFORE, GROUP_ITEMS, SORT_ITEMS,
};

/// The filter icon in a group's heading and the menu it opens (under the icon, over the sidebar's edge as the desktop app's
/// does: the menu is placed by the icon's position in the window, so the sidebar does not cut the submenu off), in the desktop app's order: group by, sort by (the key and the
/// direction), what to show (metadata, empty groups), a reset to the defaults, and `View all` (the Sessions page). A choice leaves the
/// menu open, as the desktop app's does; only the reset closes it.
/// The menu takes focus when it opens; arrows move, Right opens a submenu, Left or Escape closes
/// it, Enter chooses (the keys are the physical ones, so a Korean input method does not matter).
#[component]
pub fn SidebarFilterMenu() -> Element {
    let workspace = use_workspace();
    let ui = use_context::<FilterUi>();
    let mut open = ui.open;
    let mut menu = ui.menu;
    let anchor = ui.anchor;
    let prefs = workspace.prefs();
    let state = menu();

    // The sort submenu draws a divider between the keys and the direction rows, the group one before `None`, the
    // display one after the metadata.
    let divider_before = match FILTER_ROWS[state.top] {
        FilterRow::Sort => SORT_ITEMS.len(),
        FilterRow::Group => GROUP_DIVIDER_BEFORE,
        FilterRow::Display => 1,
        _ => usize::MAX,
    };
    let sub_items: Vec<(String, bool, FilterChoice)> = match FILTER_ROWS[state.top] {
        FilterRow::Sort => SORT_ITEMS
            .iter()
            .map(|item| (sort_label(*item).to_string(), prefs.sort_by == *item, FilterChoice::Sort(*item)))
            .chain([false, true].map(|reversed| (direction_label(reversed).to_string(), prefs.reversed == reversed, FilterChoice::Reverse(reversed))))
            .collect(),
        FilterRow::Group => GROUP_ITEMS
            .iter()
            .map(|item| (group_label(*item).to_string(), prefs.group_by == *item, FilterChoice::Group(*item)))
            .collect(),
        FilterRow::Display => DISPLAY_ITEMS
            .iter()
            .map(|item| {
                let choice = match item {
                    forge_features::DisplayItem::Metadata => FilterChoice::ToggleMetadata,
                    forge_features::DisplayItem::EmptyGroups => FilterChoice::ToggleShowEmpty,
                };
                (display_label(*item).to_string(), display_on(*item, &prefs), choice)
            })
            .collect(),
        FilterRow::Reset | FilterRow::ViewAll => Vec::new(),
    };
    let at_defaults = prefs.is_default_view();

    rsx! {
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
                    style: match anchor() {
                        Some((right, bottom)) => format!("position: fixed; left: auto; top: {}px; right: calc(100vw - {}px);", bottom + 4.0, right),
                        None => String::new(),
                    },
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
                                if choice == FilterChoice::ViewAll {
                                    workspace.show_sessions();
                                }
                                if choice.closes_menu() {
                                    open.set(false);
                                    if choice != FilterChoice::ViewAll {
                                        refocus_workspace();
                                    }
                                }
                            }
                            FilterOutcome::Close => {
                                open.set(false);
                                refocus_workspace();
                            }
                        }
                    },
                    for (index, row) in FILTER_ROWS.iter().enumerate() {
                        // The reset is a button, not a setting: a divider sets it apart. The group takes the key, so
                        // the divider and the button are one keyed item.
                        Fragment { key: "{index}",
                        if *row == FilterRow::Reset {
                            div { class: "row-menu-divider", role: "separator" }
                        }
                        button {
                            class: if state.top == index { "row-menu-item highlight" } else { "row-menu-item" },
                            disabled: *row == FilterRow::Reset && at_defaults,
                            onclick: {
                                let row = *row;
                                move |_| {
                                    if row == FilterRow::Reset {
                                        workspace.update_prefs(|prefs| FilterChoice::Reset.apply(prefs));
                                        open.set(false);
                                        refocus_workspace();
                                    } else if row == FilterRow::ViewAll {
                                        workspace.show_sessions();
                                        open.set(false);
                                    } else {
                                        menu.set(FilterMenu { top: index, sub: Some(current_index(row, &workspace.prefs())) });
                                    }
                                }
                            },
                            match row {
                                FilterRow::Sort => rsx! {
                                    span { {tr!("Sort by")} }
                                    span { class: "value", "{sort_label(prefs.sort_by)} " Icon { kind: IconKind::Chevron, size: IconSize::Small } }
                                },
                                FilterRow::Group => rsx! {
                                    span { {tr!("Group by")} }
                                    span { class: "value", "{group_label(prefs.group_by)} " Icon { kind: IconKind::Chevron, size: IconSize::Small } }
                                },
                                FilterRow::Display => rsx! {
                                    span { {tr!("Display")} }
                                    span { class: "value", Icon { kind: IconKind::Chevron, size: IconSize::Small } }
                                },
                                FilterRow::Reset => rsx! { span { {tr!("Reset to defaults")} } },
                                FilterRow::ViewAll => rsx! { span { {tr!("View all")} } },
                            }
                        }
                        }
                    }
                    if let Some(highlighted) = state.sub {
                        div { class: "filter-submenu",
                            for (index, (label, current, choice)) in sub_items.into_iter().enumerate() {
                                Fragment { key: "{index}",
                                if index == divider_before {
                                    div { class: "row-menu-divider", role: "separator" }
                                }
                                button {
                                    class: if highlighted == index { "row-menu-item highlight" } else { "row-menu-item" },
                                    onclick: move |_| {
                                        workspace.update_prefs(|prefs| choice.apply(prefs));
                                    },
                                    span { "{label}" }
                                    span { class: "value", if current { Icon { kind: IconKind::Check, size: IconSize::Small } } }
                                }
                                }
                            }
                        }
                    }
                }
            }
    }
}

/// What the menu and the icons share: the sidebar has one menu, and each group heading has an icon that opens it under
/// itself. The menu does not belong to a heading, so it stays open when a choice changes the groups and the heading
/// that carried the icon goes away (as in the desktop app).
#[derive(Clone, Copy)]
pub struct FilterUi {
    open: Signal<bool>,
    menu: Signal<FilterMenu>,
    /// Where the icon that opened the menu is in the window (its right and bottom edge).
    anchor: Signal<Option<(f64, f64)>>,
    /// Which icon opened it, so only that one is drawn as pressed.
    owner: Signal<usize>,
}

pub fn use_filter_ui_provider() -> FilterUi {
    use_context_provider(|| FilterUi {
        open: Signal::new(false),
        menu: Signal::new(FilterMenu::default()),
        anchor: Signal::new(None),
        owner: Signal::new(0),
    })
}

/// The icon in a group's heading (it shows while the pointer is on the heading).
#[component]
pub fn FilterButton() -> Element {
    let ui = use_context::<FilterUi>();
    let mut open = ui.open;
    let mut menu = ui.menu;
    let mut anchor = ui.anchor;
    let mut owner = ui.owner;
    let mut wrap = use_signal(|| None::<std::rc::Rc<MountedData>>);
    let id = use_hook(|| {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    });
    let mine = open() && owner() == id;
    rsx! {
        div {
            class: if mine { "filter-wrap open" } else { "filter-wrap" },
            onmounted: move |mounted| wrap.set(Some(mounted.data())),
            button {
                class: if mine { "icon-button open" } else { "icon-button" },
                title: tr!("Filter, sort and group sessions"),
                onclick: move |_| {
                    menu.set(FilterMenu::default());
                    if mine {
                        open.set(false);
                        return;
                    }
                    spawn(async move {
                        let mounted = wrap.peek().clone();
                        if let Some(mounted) = mounted {
                            if let Ok(rect) = mounted.get_client_rect().await {
                                anchor.set(Some((rect.origin.x + rect.size.width, rect.origin.y + rect.size.height)));
                            }
                        }
                        owner.set(id);
                        open.set(true);
                    });
                },
                Icon { kind: IconKind::Filter }
            }
        }
    }
}
