use dioxus::prelude::*;
use forge_domain::PluginEntry;
use forge_features::{
    explore_groups, group_title, mine_groups, use_customize, Busy, CustomizeMode, Dialog, Job, Loaded, PluginGroup,
};
use forge_shared::{Button, ButtonKind};

#[component]
pub fn PluginsTab() -> Element {
    let customize = use_customize();
    let view = match customize.view() {
        Loaded::Loading => return rsx! { div { class: "hint", "Loading…" } },
        Loaded::Failed(message) => {
            return rsx! {
                div { class: "status-line error", "{message}" }
                Button { label: "Try again", onclick: move |_| customize.reload() }
            }
        }
        Loaded::Ready(view) => view,
    };
    if let Some(id) = customize.detail() {
        return match view.snapshot.plugins.iter().find(|plugin| plugin.id == id) {
            Some(plugin) => rsx! { PluginDetail { plugin: plugin.clone() } },
            // The plugin is gone (uninstalled elsewhere): back to the list.
            None => rsx! { div { class: "hint", "That plugin is no longer there." Button { label: "Back", onclick: move |_| customize.close_detail() } } },
        };
    }
    let query = customize.query();
    match customize.mode() {
        CustomizeMode::Mine => {
            let groups = mine_groups(&view.snapshot.plugins, &view.enabled, &query);
            if groups.is_empty() {
                return rsx! { div { class: "hint", "No plugins installed. Use Explore to find some." } };
            }
            rsx! { MineList { groups } }
        }
        CustomizeMode::Explore => {
            let groups = explore_groups(&view.snapshot.plugins, &view.enabled, &query);
            if view.snapshot.marketplaces.is_empty() {
                return rsx! {
                    div { class: "customize-note",
                        p { "Add a marketplace to find plugins." }
                        Button { label: "Add marketplace", kind: ButtonKind::Primary, onclick: move |_| customize.open_dialog(Dialog::AddMarketplace) }
                    }
                };
            }
            if groups.is_empty() {
                return rsx! { div { class: "hint", "Nothing matches." } };
            }
            rsx! { ExploreGrid { groups } }
        }
    }
}

#[component]
fn MineList(groups: Vec<PluginGroup>) -> Element {
    let customize = use_customize();
    let mut menu: Signal<Option<String>> = use_signal(|| None);
    rsx! {
        for group in groups {
            div { key: "{group.marketplace}", class: "customize-group",
                h2 { "{group_title(&group.marketplace)} " span { class: "count", "{group.rows.len()}" } }
                for row in group.rows {
                    {
                        let id = row.entry.id.clone();
                        let busy = customize.busy(&id);
                        rsx! {
                            div {
                                key: "{id}",
                                class: "customize-row",
                                onclick: {
                                    let id = id.clone();
                                    move |_| customize.open_detail(id.clone())
                                },
                                div { class: "customize-icon", "⌬" }
                                div { class: "customize-main",
                                    div { class: "customize-name",
                                        "{row.entry.name}"
                                        span { class: "tag", "{row.entry.marketplace}" }
                                        if !row.enabled { span { class: "tag off", "Off" } }
                                    }
                                    div { class: "customize-desc", "{row.entry.description}" }
                                }
                                if let Some(version) = row.entry.version.clone() { div { class: "customize-meta", "{forge_features::version_label(&version)}" } }
                                if busy != Busy::No { span { class: "spinner", title: if busy == Busy::Waiting { "Waiting…" } else { "Working…" } } }
                                button {
                                    class: "icon-button",
                                    title: "More",
                                    // Its label and action came from the list as it was; wait for the list to be read again.
                                    disabled: busy != Busy::No,
                                    onclick: {
                                        let id = id.clone();
                                        move |event| {
                                            event.stop_propagation();
                                            menu.set(if menu() == Some(id.clone()) { None } else { Some(id.clone()) });
                                        }
                                    },
                                    "⋮"
                                }
                                if menu() == Some(id.clone()) {
                                    div { class: "menu-backdrop", onclick: move |event| { event.stop_propagation(); menu.set(None) } }
                                    div { class: "customize-menu row-menu",
                                        button {
                                            onclick: {
                                                let id = id.clone();
                                                let on = !row.enabled;
                                                move |event| {
                                                    event.stop_propagation();
                                                    menu.set(None);
                                                    customize.run(Job::SetEnabled(id.clone(), on));
                                                }
                                            },
                                            if row.enabled { "Turn off" } else { "Turn on" }
                                        }
                                        if row.entry.can_uninstall() {
                                        button {
                                            class: "danger",
                                            onclick: {
                                                let id = id.clone();
                                                move |event| {
                                                    event.stop_propagation();
                                                    menu.set(None);
                                                    customize.open_dialog(Dialog::ConfirmUninstall(id.clone()));
                                                }
                                            },
                                            "Uninstall"
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
    }
}

#[component]
fn ExploreGrid(groups: Vec<PluginGroup>) -> Element {
    let customize = use_customize();
    rsx! {
        for group in groups {
            div { key: "{group.marketplace}", class: "customize-group",
                h2 { "{group_title(&group.marketplace)} " span { class: "count", "{group.rows.len()}" } }
                div { class: "customize-grid",
                    for row in group.rows {
                        {
                            let id = row.entry.id.clone();
                            let busy = customize.busy(&id);
                            let installed = row.entry.installed;
                            rsx! {
                                div {
                                    key: "{id}",
                                    class: "customize-card",
                                    onclick: {
                                        let id = id.clone();
                                        move |_| customize.open_detail(id.clone())
                                    },
                                    div { class: "customize-icon", "⌬" }
                                    div { class: "customize-main",
                                        div { class: "customize-name", "{row.entry.name}" }
                                        div { class: "customize-card-desc", "{row.entry.description}" }
                                        div { class: "customize-source", "{group_title(&row.entry.marketplace)}" }
                                    }
                                    if busy != Busy::No {
                                        span { class: "spinner", title: if busy == Busy::Waiting { "Waiting…" } else { "Working…" } }
                                    } else if installed {
                                        span { class: "customize-check", title: "Installed", "✓" }
                                    } else {
                                        button {
                                            class: "customize-plus",
                                            title: "Install",
                                            onclick: {
                                                let id = id.clone();
                                                move |event| {
                                                    event.stop_propagation();
                                                    customize.run(Job::Install(id.clone()));
                                                }
                                            },
                                            "+"
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
}

#[component]
fn PluginDetail(plugin: PluginEntry) -> Element {
    let customize = use_customize();
    let busy = customize.busy(&plugin.id);
    let id = plugin.id.clone();
    rsx! {
        div { class: "customize-detail",
            button { class: "customize-back", onclick: move |_| customize.close_detail(), "← Plugins" }
            div { class: "customize-detail-head",
                div { class: "customize-icon big", "⌬" }
                div {
                    h2 { "{plugin.name}" }
                    div { class: "hint",
                        "{group_title(&plugin.marketplace)}"
                        if let Some(version) = plugin.version.clone() { " · {forge_features::version_label(&version)}" }
                    }
                }
                div { class: "customize-detail-action",
                    if busy != Busy::No {
                        span { class: "spinner", title: "Working…" }
                    } else if plugin.installed && !plugin.can_uninstall() {
                        span { class: "hint", "Installed in the {plugin.scope.clone().unwrap_or_default()} scope. Remove it there." }
                    } else if plugin.installed {
                        Button {
                            label: "Uninstall",
                            kind: ButtonKind::Danger,
                            onclick: {
                                let id = id.clone();
                                move |_| customize.open_dialog(Dialog::ConfirmUninstall(id.clone()))
                            },
                        }
                    } else {
                        Button {
                            label: "Install",
                            kind: ButtonKind::Primary,
                            onclick: {
                                let id = id.clone();
                                move |_| customize.run(Job::Install(id.clone()))
                            },
                        }
                    }
                }
            }
            if !plugin.description.is_empty() {
                div { class: "customize-section", div { class: "label", "Description" } p { "{plugin.description}" } }
            }
            if let Some(path) = plugin.install_path.clone() {
                div { class: "customize-section", div { class: "label", "Installed at" } p { class: "mono", "{path.display()}" } }
            }
        }
    }
}
