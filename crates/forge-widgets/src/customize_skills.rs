use dioxus::prelude::*;
use forge_entities::render_markdown;
use forge_features::{
    body_after_front_matter, explore_skills, highlighted_code, file_depth, file_leaf, find_skill, is_markdown_file, origin_label, skill_groups,
    use_customize, use_workspace, Busy, CustomizeMode, Dialog, Job, Loaded, SkillPane, SkillRow, SkillText,
};
use forge_shared::{tr, Button, ButtonKind, LoadingRows, Icon, IconKind, IconSize};

#[component]
pub fn SkillsTab() -> Element {
    let customize = use_customize();
    let Some(view) = customize.skills() else {
        return match customize.view() {
            Loaded::Failed(message) => rsx! {
                div { class: "status-line error", "{message}" }
                Button { label: tr!("Try again"), onclick: move |_| customize.reload() }
            },
            // The plugins were read and the skills were not: the reason is in the bar above.
            Loaded::Ready(_) => rsx! {
                div { class: "status-line error", {tr!("The skills could not be read.")} }
                Button { label: tr!("Try again"), onclick: move |_| customize.reload() }
            },
            Loaded::Loading => rsx! { LoadingRows {} },
        };
    };
    if let Some(id) = customize.detail() {
        return match find_skill(&view, &id) {
            Some(row) => rsx! { SkillDetail { row } },
            None => rsx! {
                div { class: "hint",
                    {tr!("That skill is no longer there. ")}
                    Button { label: tr!("Back"), onclick: move |_| customize.close_detail() }
                }
            },
        };
    }
    let query = customize.query();
    match customize.mode() {
        CustomizeMode::Mine => {
            let groups = skill_groups(&view, &query);
            if groups.mine.is_empty() && groups.from_plugins.is_empty() {
                let text = if query.trim().is_empty() { tr!("No skills yet. Use + Add to upload or create one.") } else { tr!("Nothing matches.") };
                return rsx! { div { class: "hint", "{text}" } };
            }
            rsx! {
                if !groups.mine.is_empty() { SkillGroup { title: tr!("Made by me"), rows: groups.mine } }
                if !groups.from_plugins.is_empty() { SkillGroup { title: tr!("From plugins"), rows: groups.from_plugins } }
            }
        }
        CustomizeMode::Explore => {
            if !view.has_marketplaces {
                return rsx! {
                    div { class: "customize-note",
                        p { {tr!("Add a marketplace in the Plugins tab to find skills.")} }
                        Button { label: tr!("Add marketplace"), kind: ButtonKind::Primary, onclick: move |_| customize.open_dialog(Dialog::AddMarketplace) }
                    }
                };
            }
            let cards = explore_skills(&view, &query);
            if cards.is_empty() {
                return rsx! {
                    div { class: "hint",
                        {tr!("No skills to find in the marketplaces you added. Skills of a plugin that lives in another Git repository show after the plugin is installed.")}
                    }
                };
            }
            rsx! {
                div { class: "customize-grid",
                    for row in cards {
                        SkillCard { key: "{row.entry.id}", row }
                    }
                }
            }
        }
    }
}

#[component]
fn SkillGroup(title: String, rows: Vec<SkillRow>) -> Element {
    let customize = use_customize();
    rsx! {
        div { class: "customize-group",
            h2 { "{title} " span { class: "count", "{rows.len()}" } }
            for row in rows {
                {
                    let (id, path) = (row.entry.id.clone(), row.entry.path.clone());
                    let (name, on) = (row.entry.name.clone(), row.enabled);
                    rsx! {
                        div {
                            key: "{id}",
                            class: "customize-row",
                            onclick: move |_| customize.open_skill(id.clone(), path.clone()),
                            div { class: "customize-icon", Icon { kind: IconKind::Sparkle } }
                            div { class: "customize-main",
                                div { class: "customize-name", "{row.entry.name}" span { class: "tag", "{origin_label(&row.entry.origin)}" } }
                                div { class: "customize-desc", "{row.entry.description}" }
                            }
                            button {
                                class: if on { "switch on" } else { "switch" },
                                disabled: !row.switchable(),
                                title: if row.switchable() { if on { tr!("Turn off") } else { tr!("Turn on") } } else { tr!("Turned on and off with its plugin") },
                                onclick: move |event| {
                                    event.stop_propagation();
                                    customize.set_skill_enabled(name.clone(), !on);
                                },
                                span { class: "knob" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn SkillCard(row: SkillRow) -> Element {
    let customize = use_customize();
    let plugin_id = row.entry.origin.plugin_id().unwrap_or_default().to_string();
    let busy = customize.busy(&plugin_id);
    let (id, path) = (row.entry.id.clone(), row.entry.path.clone());
    let install = plugin_id.clone();
    rsx! {
        div { class: "customize-card", onclick: move |_| customize.open_skill(id.clone(), path.clone()),
            div { class: "customize-icon", Icon { kind: IconKind::Sparkle } }
            div { class: "customize-main",
                div { class: "customize-name", "{row.entry.name}" }
                div { class: "customize-card-desc", "{row.entry.description}" }
                div { class: "customize-source", "{plugin_id}" }
            }
            if busy != Busy::No {
                span { class: "spinner", title: if busy == Busy::Waiting { tr!("Waiting…") } else { tr!("Working…") } }
            } else {
                button {
                    class: "customize-plus",
                    title: tr!("Install the plugin {plugin}", plugin = plugin_id),
                    onclick: move |event| {
                        event.stop_propagation();
                        customize.run(Job::Install(install.clone()));
                    },
                    "+"
                }
            }
        }
    }
}

#[component]
fn SkillDetail(row: SkillRow) -> Element {
    let customize = use_customize();
    let pane = customize.skill_pane();
    let plugin_id = row.entry.origin.plugin_id().map(str::to_string);
    let busy = plugin_id.as_deref().map_or(Busy::No, |id| customize.busy(id));
    rsx! {
        div { class: "customize-detail",
            button { class: "customize-back", onclick: move |_| customize.close_detail(), {tr!("← Skills")} }
            div { class: "customize-detail-head",
                div { class: "customize-icon big", Icon { kind: IconKind::Sparkle } }
                div {
                    h2 { "{row.entry.name}" }
                    div { class: "hint", "{origin_label(&row.entry.origin)}" }
                }
                div { class: "customize-detail-action",
                    if busy != Busy::No {
                        span { class: "spinner", title: tr!("Working…") }
                    } else if !row.entry.installed {
                        if let Some(id) = plugin_id.clone() {
                            Button { label: tr!("Install plugin"), kind: ButtonKind::Primary, onclick: move |_| customize.run(Job::Install(id.clone())) }
                        }
                    }
                }
            }
            div { class: "customize-tabs",
                button {
                    class: if pane == SkillPane::Overview { "customize-tab active" } else { "customize-tab" },
                    onclick: move |_| customize.set_skill_pane(SkillPane::Overview),
                    {tr!("Overview")}
                }
                button {
                    class: if pane == SkillPane::Files { "customize-tab active" } else { "customize-tab" },
                    onclick: move |_| customize.set_skill_pane(SkillPane::Files),
                    {tr!("Files")}
                }
            }
            match pane {
                SkillPane::Overview => rsx! {
                    div { class: "customize-section", div { class: "label", {tr!("Description")} } p { "{row.entry.description}" } }
                    div { class: "customize-section", div { class: "label", {tr!("Made by")} } p { "{origin_label(&row.entry.origin)}" } }
                    if let Some(id) = plugin_id.clone() {
                        div { class: "customize-section", div { class: "label", {tr!("Plugin")} } p { "{id}" } }
                    }
                    div { class: "customize-section", div { class: "label", {tr!("Folder")} } p { class: "mono", "{row.entry.path.display()}" } }
                },
                SkillPane::Files => rsx! { SkillFiles { folder: row.entry.path.clone() } },
            }
        }
    }
}

#[component]
fn SkillFiles(folder: std::path::PathBuf) -> Element {
    let customize = use_customize();
    let workspace = use_workspace();
    let files = customize.skill_files();
    let selected = customize.skill_file();
    let source = customize.skill_source();
    rsx! {
        div { class: "skill-files-bar",
            Button { label: tr!("Show in folder"), onclick: move |_| workspace.reveal(&folder) }
        }
        div { class: "skill-files",
            div { class: "skill-tree-column",
            div { class: "skill-tree",
                for file in files {
                    {
                        let relative = file.relative.clone();
                        let depth = file_depth(&relative);
                        let active = selected.as_deref() == Some(relative.as_str());
                        rsx! {
                            button {
                                key: "{relative}",
                                class: if active { "skill-tree-row active" } else { "skill-tree-row" },
                                style: "padding-left: {8 + depth * 14}px",
                                disabled: file.is_dir,
                                onclick: move |_| customize.select_skill_file(relative.clone()),
                                if file.is_dir { Icon { kind: IconKind::Chevron, size: IconSize::Small } }
                                "{file_leaf(&file.relative)}"
                            }
                        }
                    }
                }
            }
            if customize.skill_files_cut() {
                div { class: "hint skill-tree-note", {tr!("Only the first files are listed. Use Show in folder to see all of them.")} }
            }
            }
            div { class: "skill-view",
                if let Some(path) = selected.clone() {
                    div { class: "skill-view-head",
                        span { class: "mono", "{path}" }
                        if is_markdown_file(&path) {
                            div { class: "customize-switch",
                                button { class: if !source { "active" } else { "" }, onclick: move |_| customize.set_skill_source(false), {tr!("Rendered")} }
                                button { class: if source { "active" } else { "" }, onclick: move |_| customize.set_skill_source(true), {tr!("Source")} }
                            }
                        }
                    }
                }
                match customize.skill_text() {
                    SkillText::Empty => rsx! { div { class: "hint", {tr!("Choose a file.")} } },
                    SkillText::Loading => rsx! { div { class: "hint", {tr!("Loading…")} } },
                    SkillText::Failed(message) => rsx! { div { class: "status-line error", "{message}" } },
                    SkillText::Ready(text) => {
                        let rendered = !source && selected.as_deref().is_some_and(is_markdown_file);
                        if rendered {
                            let html = render_markdown(body_after_front_matter(&text));
                            rsx! { div { class: "skill-rendered markdown", dangerous_inner_html: "{html}" } }
                        } else if text.is_empty() {
                            rsx! { div { class: "hint", {tr!("This file is empty.")} } }
                        } else if let Some(html) = selected.as_deref().and_then(|path| highlighted_code(path, &text)) {
                            rsx! { pre { class: "skill-code", dangerous_inner_html: "{html}" } }
                        } else {
                            rsx! { pre { class: "skill-code", "{text}" } }
                        }
                    }
                }
            }
        }
    }
}
