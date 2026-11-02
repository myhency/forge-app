use dioxus::prelude::*;
use forge_features::{kind_label, updated_day, use_settings, use_workspace};
use forge_shared::{tr, Button, ButtonKind, SettingRow, Switch};

/// The note being read: which folder, which file, and the text as typed so far.
#[derive(Clone, PartialEq)]
struct Open {
    project: String,
    file: String,
    name: String,
    saved: String,
    text: String,
    confirm_delete: bool,
}

/// The Memory page: the switch for Claude's auto memory and the notes it has kept, by folder, each one readable,
/// editable and removable.
#[component]
pub fn MemorySection() -> Element {
    let settings = use_settings();
    let workspace = use_workspace();
    let mut version = use_signal(|| 0u32);
    let mut open = use_signal(|| None::<Open>);
    let mut message = use_signal(|| None::<String>);
    let notes = use_resource(move || async move {
        version();
        workspace.memory_list().await
    });
    rsx! {
        h2 { {tr!("Memory")} }
        SettingRow {
            title: tr!("Auto memory"),
            description: tr!("Claude keeps notes about you and your projects between sessions and reads them when a session starts. Applies to new sessions."),
            Switch { on: settings.auto_memory(), onchange: move |on| settings.set_auto_memory(on) }
        }
        h3 { class: "settings-group", {tr!("Notes")} }
        div { class: "hint", {tr!("Claude writes these as you work. Open one to read or change it.")} }
        if let Some(text) = message() {
            div { class: "status-line error", "{text}" }
        }
        match &*notes.read() {
            None => rsx! { div { class: "hint", {tr!("Loading…")} } },
            Some(Err(text)) => rsx! { div { class: "status-line error", "{text}" } },
            Some(Ok(projects)) if projects.is_empty() => rsx! { div { class: "hint", {tr!("No notes yet.")} } },
            Some(Ok(projects)) => rsx! {
                for project in projects.clone() {
                    div { key: "{project.key}", class: "memory-project",
                        div { class: "memory-project-title", "{project.label}" }
                        div { class: "memory-project-path mono customize-wrap", "{project.path}" }
                        for entry in project.entries.clone() {
                            {
                                let chosen = open().is_some_and(|o| o.project == project.key && o.file == entry.file);
                                let (key, file) = (project.key.clone(), entry.file.clone());
                                let name = entry.name.clone();
                                rsx! {
                                    button {
                                        key: "{entry.file}",
                                        class: if chosen { "memory-row active" } else { "memory-row" },
                                        onclick: move |_| {
                                            message.set(None);
                                            match workspace.memory_read(&key, &file) {
                                                Ok(text) => open.set(Some(Open { project: key.clone(), file: file.clone(), name: name.clone(), saved: text.clone(), text, confirm_delete: false })),
                                                Err(text) => message.set(Some(text)),
                                            }
                                        },
                                        span { class: "memory-row-name", "{entry.name}" }
                                        if !entry.kind.is_empty() {
                                            span { class: "memory-row-kind", {kind_label(&entry.kind)} }
                                        }
                                        span { class: "memory-row-date", {tr!("Updated {date}", date = updated_day(entry.updated))} }
                                        if !entry.description.is_empty() {
                                            span { class: "memory-row-desc", "{entry.description}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
        }
        if let Some(note) = open() {
            div { class: "memory-editor",
                h3 { class: "customize-wrap", "{note.name}" }
                textarea {
                    class: "memory-text mono",
                    value: "{note.text}",
                    oninput: move |event| {
                        if let Some(current) = open.write().as_mut() {
                            current.text = event.value();
                        }
                    },
                }
                div { class: "memory-buttons",
                    Button {
                        label: tr!("Save"),
                        kind: ButtonKind::Primary,
                        disabled: note.text == note.saved,
                        onclick: {
                            let note = note.clone();
                            move |_| match workspace.memory_write(&note.project, &note.file, &note.text) {
                                Ok(()) => {
                                    message.set(None);
                                    if let Some(current) = open.write().as_mut() {
                                        current.saved = current.text.clone();
                                    }
                                    version += 1;
                                }
                                Err(text) => message.set(Some(text)),
                            }
                        },
                    }
                    Button { label: tr!("Close"), onclick: move |_| open.set(None) }
                    Button {
                        label: if note.confirm_delete { tr!("Move to the Trash?") } else { tr!("Delete") },
                        kind: ButtonKind::Danger,
                        onclick: {
                            let note = note.clone();
                            move |_| {
                                if !note.confirm_delete {
                                    if let Some(current) = open.write().as_mut() {
                                        current.confirm_delete = true;
                                    }
                                    return;
                                }
                                match workspace.memory_delete(&note.project, &note.file) {
                                    Ok(()) => {
                                        message.set(None);
                                        open.set(None);
                                        version += 1;
                                    }
                                    Err(text) => message.set(Some(text)),
                                }
                            }
                        },
                    }
                }
            }
        }
    }
}
