use std::path::PathBuf;

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_features::{fallback_source, run_blocking, source_for, stack_order, use_workspace, vanished_commit, ChangeView, FileAction, MenuTarget, Panel};
use forge_ports::{BranchInfo, ChangeSource, CommitInfo, DiffOptions, FileChange, GitError};

use crate::changes_header::{position_class, BranchButton, FileSearch, ViewMenu};
use crate::changes_stack::{scroll_to_diff, DiffStack};
use crate::changes_tree::{ChangesTree, CommitsSection};
use crate::context_menu::ContextMenu;
use crate::files_state::{absolute, copy_text_for, menu_root};
use crate::panel_frame::PanelFrame;
use forge_shared::tr;

/// The Changes panel: every changed file as a stacked diff and a folder tree of the files at the
/// right. It is a viewer; it reloads on Refresh, when a tool call of the session finishes, and when
/// what is compared changes. See the spec `2026-10-17-changes-panel-design.md`.
#[component]
pub fn ChangesPanel(id: LiveSessionId, cwd: PathBuf) -> Element {
    let workspace = use_workspace();
    // Back to what this session compared last time; before the loaders' effects run.
    use_hook(move || workspace.restore_change_mode(id));
    let mode_memo = use_memo(move || {
        let state = workspace.changes_session(id);
        (state.view, state.base)
    });
    // The Claude session id appears with the first message; a mode chosen before then is saved then.
    let key_memo = use_memo(move || workspace.claude_session_key(id));
    use_effect(move || {
        let _ = mode_memo();
        let _ = key_memo();
        workspace.remember_change_mode(id);
    });
    let mut refresh = use_signal(|| 0u32);
    let mut list_open = use_signal(|| true);
    // ⇧⌘Y shows or hides the list.
    use_effect(move || {
        if workspace.changes_list_request() {
            workspace.clear_changes_list_request();
            let shown = *list_open.peek();
            list_open.set(!shown);
        }
    });
    let mut info = use_signal(|| None::<Result<BranchInfo, GitError>>);
    let mut commits = use_signal(|| None::<Vec<CommitInfo>>);
    // The files with the source they were really loaded with (the uncommitted changes after a
    // missing base): the sections read more lines and large diffs against that source.
    let mut result = use_signal(|| None::<(ChangeSource, Result<Vec<FileChange>, GitError>)>);
    // The base that was asked for and was not there; the list then shows the uncommitted changes.
    let mut missing_base = use_signal(|| None::<String>);
    // The count changes only when a tool call finishes; reading the session's view directly in an
    // effect would rerun it on every streamed chunk of a reply.
    let finished = use_memo(move || workspace.finished_tool_calls(id));
    // Each run takes a ticket; a result is kept only if no later run started meanwhile.
    let mut info_ticket = use_signal(|| 0u64);
    let mut changes_ticket = use_signal(|| 0u64);
    let mut commits_ticket = use_signal(|| 0u64);
    let mut dirty = use_signal(|| true);
    let mut dirty_ticket = use_signal(|| 0u64);

    let session = workspace.changes_session(id);
    let prefs = workspace.changes_view_prefs();
    // What the loaders compare with, as memos: an effect reads them so it reruns when they change
    // (the branch list arriving with the default base, a picked base or view, the whitespace
    // option) and not when anything else in the session's state changes (a folded file).
    let base_memo = {
        let cwd = cwd.clone();
        use_memo(move || {
            workspace
                .changes_session(id)
                .base
                .or_else(|| workspace.worktree_base(&cwd))
                .or_else(|| info().and_then(|read| read.ok()).and_then(|branches| branches.default_base))
        })
    };
    let source_memo = use_memo(move || source_for(&workspace.changes_session(id).view, base_memo().as_deref()));
    let hide_memo = use_memo(move || workspace.changes_view_prefs().hide_whitespace);

    {
        let cwd = cwd.clone();
        use_effect(move || {
            let _ = refresh();
            let _ = finished();
            let ticket = info_ticket.peek().wrapping_add(1);
            info_ticket.set(ticket);
            let (git, cwd) = (workspace.git(), cwd.clone());
            spawn(async move {
                let Some(read) = run_blocking(move || git.branches(&cwd)).await else { return };
                if *info_ticket.peek() == ticket {
                    info.set(Some(read));
                }
            });
        });
    }
    {
        let cwd = cwd.clone();
        use_effect(move || {
            let _ = refresh();
            let _ = finished();
            let ticket = dirty_ticket.peek().wrapping_add(1);
            dirty_ticket.set(ticket);
            let (git, cwd) = (workspace.git(), cwd.clone());
            spawn(async move {
                let Some(read) = run_blocking(move || git.is_dirty(&cwd)).await else { return };
                if *dirty_ticket.peek() == ticket {
                    // An error (not a repository, git missing) keeps the row: showing more is safer.
                    dirty.set(read.unwrap_or(true));
                }
            });
        });
    }
    {
        let cwd = cwd.clone();
        use_effect(move || {
            let _ = refresh();
            let _ = finished();
            let (source, hide) = (source_memo(), hide_memo());
            let ticket = changes_ticket.peek().wrapping_add(1);
            changes_ticket.set(ticket);
            let (git, cwd) = (workspace.git(), cwd.clone());
            let options = DiffOptions { ignore_whitespace: hide };
            spawn(async move {
                let Some((used, read, missing)) = run_blocking(move || {
                    let read = git.changes(&cwd, &source, &options);
                    match read.as_ref().err().and_then(|error| fallback_source(&source, error)) {
                        Some((instead, base)) => {
                            let read = git.changes(&cwd, &instead, &options);
                            (instead, read, Some(base))
                        }
                        None => (source, read, None),
                    }
                })
                .await
                else {
                    return;
                };
                if *changes_ticket.peek() == ticket {
                    missing_base.set(missing);
                    result.set(Some((used, read)));
                }
            });
        });
    }
    {
        let cwd = cwd.clone();
        use_effect(move || {
            let _ = refresh();
            let _ = finished();
            let ticket = commits_ticket.peek().wrapping_add(1);
            commits_ticket.set(ticket);
            let Some(base) = base_memo() else {
                commits.set(None);
                return;
            };
            let (git, cwd) = (workspace.git(), cwd.clone());
            spawn(async move {
                let Some(read) = run_blocking(move || git.commits(&cwd, &base)).await else { return };
                if *commits_ticket.peek() == ticket {
                    let read = read.ok();
                    if let Some(list) = &read {
                        if vanished_commit(&workspace.changes_session(id).view, list) {
                            workspace.update_changes_session(id, |session| session.view = ChangeView::All);
                        }
                    }
                    commits.set(read);
                }
            });
        });
    }

    let mut menu = use_signal(|| None::<(String, bool, f64, f64)>);
    let mut searching = use_signal(|| false);

    let commits_section = match commits() {
        Some(list) => rsx! { CommitsSection { id, commits: list, view: session.view.clone(), dirty: dirty() } },
        None => rsx! {},
    };

    let tools = rsx! {
        BranchButton {
            id,
            info: info().and_then(|read| read.ok()),
            base: base_memo(),
            commits: commits().unwrap_or_default(),
            view: session.view.clone(),
            dirty: dirty(),
        }
        button {
            class: "icon-button",
            r#type: "button",
            title: tr!("Search changed files"),
            "aria-label": "Search changed files",
            onclick: move |_| searching.set(true),
            "⌕"
        }
        ViewMenu {
            id,
            prefs: prefs.clone(),
            paths: match result() {
                Some((_, Ok(files))) => files.iter().map(|change| change.file.path.clone()).collect(),
                _ => Vec::new(),
            },
            list_open: list_open(),
            on_toggle_list: move |_| list_open.set(!list_open()),
            on_refresh: move |_| refresh.set(refresh() + 1),
        }
    };

    let menu_view = match menu() {
        Some((path, deleted, x, y)) => {
            // Git paths are relative to the repository root, which may be above the session folder.
            let root = menu_root(info().and_then(|read| read.ok()).map(|branches| branches.root).as_deref(), &cwd);
            let (cwd, run_path) = (root, path.clone());
            rsx! {
                ContextMenu {
                    entries: forge_features::file_menu(MenuTarget::Change { deleted }),
                    x,
                    y,
                    on_close: move |_| menu.set(None),
                    on_select: move |action: FileAction| {
                        menu.set(None);
                        if let Some(text) = copy_text_for(action, &cwd, &run_path) {
                            forge_entities::copy_text(&text);
                            return;
                        }
                        let full = absolute(&cwd, &run_path);
                        match action {
                            FileAction::Attach => workspace.queue_file(id, full),
                            FileAction::OpenFile => workspace.open_file(id, full.to_string_lossy().to_string()),
                            FileAction::OpenVsCode => workspace.open_with_app(&full, Some("Visual Studio Code")),
                            FileAction::OpenDefault => workspace.open_with_app(&full, None),
                            FileAction::Reveal => workspace.reveal(&full),
                            _ => {}
                        }
                    },
                }
            }
        }
        None => rsx! {},
    };

    // With no files to list (an error, a commit with an empty diff, a clean tree) the right column
    // still offers the commits, so the person can pick another row.
    let with_commits = |note: Element| -> Element {
        if list_open() && commits().is_some() {
            rsx! {
                div { class: "changes-wrap",
                    div { class: "changes-body {position_class(prefs.file_list_position)}",
                        div { class: "cd-main", {note} }
                        ChangesTree {
                            id,
                            files: Vec::new(),
                            prefs: prefs.clone(),
                            closed: session.closed_folders.clone(),
                            open_sections: session.open_sections.clone(),
                            on_pick: move |_: String| {},
                            on_menu: move |target| menu.set(Some(target)),
                            commits: commits_section.clone(),
                        }
                    }
                }
            }
        } else {
            note
        }
    };

    let body = match result() {
        None => rsx! { div { class: "dock-note", {tr!("Loading…")} } },
        Some((_, Err(GitError::NotARepository))) => rsx! { div { class: "dock-note", {tr!("No changes to show")} } },
        Some((_, Err(error))) => with_commits(rsx! { div { class: "dock-note error", "{error}" } }),
        Some((_, Ok(files))) if files.is_empty() => with_commits(rsx! { div { class: "dock-note", {tr!("No changes to show")} } }),
        Some((used, Ok(files))) => {
            let listed: Vec<_> = files.iter().map(|change| change.file.clone()).collect();
            let order = stack_order(&listed, prefs.group_by_folder, prefs.separate_special);
            let notice = missing_base().map(|name| tr!("Base branch {name} was not found; showing uncommitted changes", name = name));
            rsx! {
                div { class: "changes-wrap",
                    div { class: "changes-body {position_class(prefs.file_list_position)}",
                        div { class: "cd-main",
                            if let Some(text) = notice {
                                div { class: "cd-note", "{text}" }
                            }
                            DiffStack {
                                id,
                                cwd: cwd.clone(),
                                source: used,
                                files: files.clone(),
                                order,
                                prefs: prefs.clone(),
                                session: session.clone(),
                                on_menu: move |target| menu.set(Some(target)),
                            }
                        }
                        if list_open() {
                            ChangesTree {
                                id,
                                files: listed,
                                prefs: prefs.clone(),
                                closed: session.closed_folders.clone(),
                                open_sections: session.open_sections.clone(),
                                on_pick: move |path: String| scroll_to_diff(&path),
                                on_menu: move |target| menu.set(Some(target)),
                                commits: commits_section,
                            }
                        }
                    }
                }
            }
        }
    };

    rsx! {
        PanelFrame { id, panel: Panel::Changes, tools,
            {menu_view}
            if searching() {
                if let Some((_, Ok(files))) = result() {
                    FileSearch {
                        files: files.iter().map(|change| change.file.clone()).collect::<Vec<_>>(),
                        on_pick: move |path: String| scroll_to_diff(&path),
                        on_close: move |_| searching.set(false),
                    }
                }
            }
            {body}
        }
    }
}
