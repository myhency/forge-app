use std::path::{Path, PathBuf};

use dioxus::prelude::*;
use forge_features::{refocus_workspace, use_workspace, variables_error_text, EnvironmentDraft, EnvironmentRequest};
use forge_shared::{tr, Button, ButtonKind};

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Form,
    Variables,
}

#[derive(Clone, Copy, PartialEq)]
enum Ask {
    None,
    /// Closing a changed dialog.
    Discard,
    Delete,
}

fn folder_name(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().to_string())
}

/// `새 환경` and `환경 편집`: one dialog for both. Renders nothing while no request is open.
#[component]
pub fn EnvironmentDialog() -> Element {
    let workspace = use_workspace();
    let Some(request) = workspace.environment_request() else {
        return rsx! {};
    };
    // A request is a different dialog: the key rebuilds the body with a fresh draft.
    let key = format!("{request:?}");
    rsx! { EnvironmentDialogBody { key: "{key}", request } }
}

#[component]
fn EnvironmentDialogBody(request: EnvironmentRequest) -> Element {
    let workspace = use_workspace();
    let stored = match &request {
        EnvironmentRequest::Edit(id) => workspace.environments().into_iter().find(|environment| environment.id == *id),
        EnvironmentRequest::New { .. } => None,
    };
    let initial = match (&request, &stored) {
        (EnvironmentRequest::Edit(_), Some(environment)) => EnvironmentDraft::of(environment),
        (EnvironmentRequest::New { folder }, _) => EnvironmentDraft::blank(folder.as_deref().map(folder_name).unwrap_or_default(), folder.clone()),
        // An environment deleted under an open edit dialog: nothing left to edit.
        (EnvironmentRequest::Edit(_), None) => {
            workspace.close_environment_dialog();
            return rsx! {};
        }
    };
    let editing = stored.is_some();
    let mut draft = use_signal({
        let initial = initial.clone();
        move || initial
    });
    let initial = use_signal(|| initial);
    let mut page = use_signal(|| Page::Form);
    let mut ask = use_signal(|| Ask::None);
    let mut adding = use_signal(|| false);
    let search = use_signal(String::new);
    let mut trust_question = use_signal(|| None::<PathBuf>);
    let mut variables_text = use_signal(String::new);

    // The branches of the first folder, when it is a git repository.
    let branches = use_resource(move || {
        let folder = draft.read().folders.first().cloned();
        async move {
            match folder {
                Some(folder) => workspace.folder_branches(folder).await,
                None => None,
            }
        }
    });

    let close = move || {
        workspace.close_environment_dialog();
        refocus_workspace();
    };
    let mut request_close = move || {
        if draft.read().changed_from(&initial.read()) {
            ask.set(Ask::Discard);
        } else {
            close();
        }
    };
    let save = {
        let stored = stored.clone();
        move || {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|elapsed| elapsed.as_millis() as i64).unwrap_or(0);
            let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|elapsed| elapsed.as_nanos() as u64).unwrap_or(0);
            let environment = draft.read().build(stored.as_ref(), seed, now);
            workspace.save_environment(environment);
            close();
        }
    };
    let can_save = draft.read().can_save();

    let add_folder = move |path: PathBuf| {
        spawn(async move {
            if draft.peek().folders.contains(&path) {
                adding.set(false);
                return;
            }
            if workspace.needs_trust(path.clone()).await {
                trust_question.set(Some(path));
            } else {
                draft.write().folders.push(path);
            }
            adding.set(false);
        });
    };

    rsx! {
        div {
            class: "settings-backdrop",
            onclick: move |_| request_close(),
            div {
                class: "env-dialog",
                role: "dialog",
                "aria-label": if editing { tr!("Edit environment") } else { tr!("New environment") },
                onclick: move |event| event.stop_propagation(),
                onkeydown: move |event: KeyboardEvent| {
                    if event.key() == Key::Escape {
                        event.stop_propagation();
                        match page() {
                            Page::Variables => page.set(Page::Form),
                            Page::Form => request_close(),
                        }
                    }
                },
                if page() == Page::Variables {
                    div { class: "env-head",
                        button { class: "env-back", r#type: "button", title: tr!("Back"), onclick: move |_| page.set(Page::Form), "‹" }
                        h2 { {tr!("Variables")} }
                        button { class: "settings-close", r#type: "button", title: tr!("Close (Esc)"), onclick: move |_| request_close(), "✕" }
                    }
                    p { class: "hint", {tr!("Set for sessions started from this environment.")} }
                    textarea {
                        class: "env-variables",
                        spellcheck: "false",
                        placeholder: "NODE_ENV=production\nGIT_AUTHOR_NAME=Your Name\n# Multiline values - wrap in quotes\nCONFIG=\"key1=val1\nkey2=val2\"",
                        value: "{variables_text}",
                        oninput: move |event| variables_text.set(event.value()),
                    }
                    {
                        let error = EnvironmentDraft { env_vars: variables_text(), ..draft.read().clone() }.variables_error();
                        rsx! {
                            if let Some(error) = error.as_ref() {
                                div { class: "banner error", {variables_error_text(error)} }
                            }
                            p { class: "hint", {tr!("Saved in Forge's settings file on this computer. Written in .env format and passed to Claude sessions started from this environment.")} }
                            div { class: "env-buttons",
                                span { class: "spacer" }
                                Button {
                                    label: tr!("Done"),
                                    kind: ButtonKind::Primary,
                                    disabled: error.is_some(),
                                    onclick: move |_| {
                                        draft.write().env_vars = variables_text();
                                        page.set(Page::Form);
                                    },
                                }
                            }
                        }
                    }
                } else {
                    div { class: "env-head",
                        h2 { if editing { {tr!("Edit environment")} } else { {tr!("New environment")} } }
                        button { class: "settings-close", r#type: "button", title: tr!("Close (Esc)"), onclick: move |_| request_close(), "✕" }
                    }
                    label { class: "env-label", {tr!("Name")} }
                    input {
                        class: "customize-field",
                        r#type: "text",
                        value: "{draft.read().name}",
                        onmounted: move |mounted| async move {
                            let _ = mounted.set_focus(true).await;
                        },
                        oninput: move |event| draft.write().name = event.value(),
                    }
                    div { class: "env-folders-head",
                        label { class: "env-label", {tr!("Folders")} span { class: "hint", " " {tr!("(optional)")} } }
                        button { class: "env-add", r#type: "button", onclick: move |_| adding.set(!adding()), {tr!("+ Add")} }
                        if adding() {
                            div { class: "picker-backdrop", onclick: move |_| adding.set(false) }
                            FolderPopover { search, onpick: add_folder, onclose: move |_| adding.set(false), taken: draft.read().folders.clone() }
                        }
                    }
                    if let Some(path) = trust_question() {
                        div { class: "trust-card", tabindex: "0",
                            div { class: "trust-question", {tr!("Allow Claude to use this folder and its subfolders?")} }
                            div { class: "trust-path", title: "{path.display()}", "{path.display()}" }
                            div { class: "trust-buttons",
                                Button { label: tr!("Cancel"), onclick: move |_| trust_question.set(None) }
                                Button {
                                    label: tr!("Trust and add"),
                                    kind: ButtonKind::Primary,
                                    onclick: {
                                        let path = path.clone();
                                        move |_| {
                                            workspace.trust_folder(path.clone());
                                            draft.write().folders.push(path.clone());
                                            trust_question.set(None);
                                        }
                                    },
                                }
                            }
                        }
                    }
                    if draft.read().folders.is_empty() {
                        div { class: "env-empty", {tr!("Choose a folder to start the session in.")} }
                    } else {
                        div { class: "env-folders",
                            for (index, folder) in draft.read().folders.clone().into_iter().enumerate() {
                                div { key: "{folder.display()}", class: "env-folder", title: "{folder.display()}",
                                    span { class: "env-folder-name", "{folder_name(&folder)}" }
                                    if index == 0 {
                                        match branches.read().clone().flatten() {
                                            Some(list) => rsx! {
                                                select {
                                                    class: "env-branch",
                                                    value: "{draft.read().branch.clone().or_else(|| list.current.clone()).unwrap_or_default()}",
                                                    onchange: move |event| draft.write().branch = Some(event.value()),
                                                    for name in list.local.clone() {
                                                        option { key: "{name}", value: "{name}", "{name}" }
                                                    }
                                                }
                                                label { class: "env-worktree",
                                                    input {
                                                        r#type: "checkbox",
                                                        checked: draft.read().worktree,
                                                        onchange: move |event| draft.write().worktree = event.checked(),
                                                    }
                                                    {tr!("Worktree")}
                                                }
                                            },
                                            None => rsx! { span { class: "env-note", {tr!("Not a Git repository")} } },
                                        }
                                    }
                                    button {
                                        class: "env-remove",
                                        r#type: "button",
                                        title: tr!("Remove"),
                                        onclick: move |_| {
                                            let mut folders = draft.write();
                                            if index < folders.folders.len() {
                                                folders.folders.remove(index);
                                            }
                                        },
                                        "×"
                                    }
                                }
                            }
                        }
                    }
                    button {
                        class: "env-variables-card",
                        r#type: "button",
                        onclick: move |_| {
                            variables_text.set(draft.read().env_vars.clone());
                            page.set(Page::Variables);
                        },
                        span { class: "env-label", {tr!("Variables")} }
                        span { class: "hint", { variables_summary(&draft.read().env_vars) } }
                        span { class: "env-chevron", "›" }
                    }
                    div { class: "env-buttons",
                        if editing {
                            button { class: "env-delete", r#type: "button", onclick: move |_| ask.set(Ask::Delete), {tr!("Delete environment")} }
                        }
                        span { class: "spacer" }
                        Button { label: tr!("Cancel"), onclick: move |_| request_close() }
                        Button {
                            label: if editing { tr!("Save") } else { tr!("Create environment") },
                            kind: ButtonKind::Primary,
                            disabled: !can_save,
                            onclick: {
                                let save = save.clone();
                                move |_| save()
                            },
                        }
                    }
                }
                if ask() != Ask::None {
                    div { class: "env-ask-backdrop",
                        div { class: "env-ask", role: "alertdialog",
                            match ask() {
                                Ask::Delete => rsx! {
                                    h3 { {tr!("Delete environment?")} }
                                    p { {tr!("“{name}” will be deleted for good. Sessions started from it are kept.", name = stored.as_ref().map(|environment| environment.name.clone()).unwrap_or_default())} }
                                    div { class: "env-buttons",
                                        span { class: "spacer" }
                                        Button { label: tr!("Cancel"), onclick: move |_| ask.set(Ask::None) }
                                        Button {
                                            label: tr!("Delete environment"),
                                            kind: ButtonKind::Danger,
                                            onclick: {
                                                let id = stored.as_ref().map(|environment| environment.id.clone());
                                                move |_| {
                                                    if let Some(id) = id.clone() {
                                                        workspace.delete_environment(id);
                                                    }
                                                    close();
                                                }
                                            },
                                        }
                                    }
                                },
                                _ => rsx! {
                                    h3 { if editing { {tr!("Save the changes?")} } else { {tr!("Create this environment?")} } }
                                    div { class: "env-buttons",
                                        Button { label: tr!("Discard"), kind: ButtonKind::Danger, onclick: move |_| close() }
                                        span { class: "spacer" }
                                        Button { label: tr!("Keep editing"), onclick: move |_| ask.set(Ask::None) }
                                        Button {
                                            label: if editing { tr!("Save") } else { tr!("Create environment") },
                                            kind: ButtonKind::Primary,
                                            disabled: !can_save,
                                            onclick: {
                                                let save = save.clone();
                                                move |_| save()
                                            },
                                        }
                                    }
                                },
                            }
                        }
                    }
                }
            }
        }
    }
}

fn variables_summary(text: &str) -> String {
    match forge_domain::parse_env(text) {
        Ok(list) if list.is_empty() => tr!("None in this environment").to_string(),
        Ok(list) => tr!("{count} variables", count = list.len()),
        Err(_) => tr!("Needs a fix").to_string(),
    }
}

#[component]
fn FolderPopover(search: Signal<String>, taken: Vec<PathBuf>, onpick: EventHandler<PathBuf>, onclose: EventHandler<()>) -> Element {
    let workspace = use_workspace();
    let query = search.read().to_lowercase();
    let known: Vec<PathBuf> = workspace
        .new_session_projects()
        .into_iter()
        .filter(|project| project.exists && !taken.contains(&project.path))
        .map(|project| project.path)
        .filter(|path| query.is_empty() || folder_name(path).to_lowercase().contains(&query))
        .collect();
    rsx! {
        div { class: "picker-menu env-folder-popover", role: "menu",
            input {
                class: "env-search",
                r#type: "text",
                value: "{search}",
                onmounted: move |mounted| async move {
                    let _ = mounted.set_focus(true).await;
                },
                oninput: move |event| search.set(event.value()),
            }
            for path in known {
                button {
                    key: "{path.display()}",
                    class: "picker-item",
                    r#type: "button",
                    title: "{path.display()}",
                    onclick: {
                        let path = path.clone();
                        move |_| onpick.call(path.clone())
                    },
                    span { class: "picker-check" }
                    span { class: "picker-text", span { class: "picker-label", "{folder_name(&path)}" } }
                }
            }
            div { class: "picker-divider" }
            button {
                class: "picker-item",
                r#type: "button",
                onclick: move |_| {
                    spawn(async move {
                        match workspace.pick_folder().await {
                            Some(path) => onpick.call(path),
                            None => onclose.call(()),
                        }
                    });
                },
                span { class: "picker-check", "📁" }
                span { class: "picker-text", span { class: "picker-label", {tr!("Choose…")} } }
            }
        }
    }
}
