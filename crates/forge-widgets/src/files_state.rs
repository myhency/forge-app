use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::LineRange;
use forge_entities::{code_html, is_markdown, language_of, render_markdown};
use forge_features::{normalize_newlines, run_blocking, EditorTabs, FileAction, TabDoc, Workspace};
use forge_ports::{FileContent, FilesError, FilesPort, WriteError};

use crate::files_editor::{editor_language, EditorHandle, FromPage};
use crate::files_kinds::{kind_of, TabKind};

// Highlighting a file larger than this would keep the UI busy, so the fallback shows plain text.
const HIGHLIGHT_LIMIT: usize = 200_000;

/// What a `flush` answer from the page is for. One per tab at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pending {
    /// Write the text (`overwrite`: ignore the version check).
    Save { overwrite: bool },
    /// The tab is being closed: decide whether to ask first.
    Close,
    /// Write the text the page holds now and close the tab (Save on the close question).
    SaveAndClose,
    /// A markdown tab goes back to its rendered view.
    Render,
    /// A tool call finished: re-read the file when the tab is clean and the disk changed.
    Refresh,
    /// `Reload file` from a menu: show the disk text, or raise the conflict banner when the tab has edits.
    Reload,
}

impl Pending {
    fn rank(self) -> u8 {
        match self {
            Pending::Refresh => 1,
            Pending::Render | Pending::Reload => 2,
            Pending::Save { overwrite: false } => 3,
            Pending::Save { overwrite: true } => 4,
            Pending::Close => 5,
            Pending::SaveAndClose => 6,
        }
    }

    /// Whether this question may take the place of one that still waits for the page's answer. A
    /// quiet refresh never takes the place of a close or a save, and nothing replaces itself.
    pub fn replaces(self, waiting: Pending) -> bool {
        self.rank() > waiting.rank()
    }
}

/// The absolute path a tree or tab path stands for.
pub fn absolute(cwd: &std::path::Path, path: &str) -> PathBuf {
    cwd.join(path)
}

/// The text a copy action puts on the clipboard, `None` for any other action.
pub fn copy_text_for(action: FileAction, cwd: &std::path::Path, path: &str) -> Option<String> {
    match action {
        FileAction::CopyAbsolute => Some(absolute(cwd, path).to_string_lossy().to_string()),
        FileAction::CopyRelative => Some(path.to_string()),
        FileAction::CopyName => Some(path.rsplit('/').next().unwrap_or(path).to_string()),
        _ => None,
    }
}

/// The text of a file for `Copy file contents`, or a message for the person.
pub fn contents_to_copy(content: &FileContent) -> Result<String, String> {
    match content {
        FileContent::Text { text, .. } => Ok(text.clone()),
        FileContent::TooLarge => Err("The file is too large to copy".to_string()),
        FileContent::Binary | FileContent::Image { .. } => Err("Only a text file can be copied".to_string()),
    }
}

/// Everything the panel does, in one place. It is `Clone` (signals are `Copy`), so event handlers
/// take a clone. The text of an open file lives in the editor in the page; Rust keeps a `TabDoc`
/// (the version and a hash of what is on disk) per tab.
#[derive(Clone)]
pub struct FilesCtx {
    pub workspace: Workspace,
    /// The session this panel belongs to: its cards live in the workspace under this id.
    pub id: LiveSessionId,
    pub cwd: PathBuf,
    pub handle: EditorHandle,
    pub tabs: Signal<EditorTabs>,
    pub kinds: Signal<HashMap<String, TabKind>>,
    pub docs: Signal<HashMap<String, TabDoc>>,
    /// Rendered markdown of a markdown tab.
    pub markdown: Signal<HashMap<String, String>>,
    /// Markdown tabs shown as source (editable) instead of rendered.
    pub source: Signal<HashSet<String>>,
    pub pending: Signal<HashMap<String, Pending>>,
    /// A tab with edits that cannot be saved on its own, and the text it holds, waiting for an answer.
    pub confirm: Signal<Option<(String, String)>>,
    /// Tabs still to close one by one (close others, close all).
    pub queue: Signal<Vec<String>>,
    /// Tabs with a write in flight (one write per tab at a time).
    pub writing: Signal<HashSet<String>>,
    /// Highlighted html and line numbers, used only when the editor bundle did not load.
    pub fallback: Signal<HashMap<String, (String, String)>>,
    pub ready: Signal<bool>,
    pub unavailable: Signal<bool>,
}

impl FilesCtx {
    fn files(&self) -> Arc<dyn FilesPort> {
        self.workspace.files()
    }

    // Changes one tab's `TabDoc`, if it is still open.
    fn with_doc(&self, key: &str, change: impl FnOnce(&mut TabDoc)) {
        let mut docs = self.docs;
        let mut write = docs.write();
        if let Some(doc) = write.get_mut(key) {
            change(doc);
        }
    }

    /// A search hit was clicked: show the file, then open the find bar on the line. The command waits
    /// behind the load in the page's queue; a tab that is not loaded yet keeps it until it shows.
    pub fn find_in_file(&self, path: &str, query: &str, options: forge_ports::SearchOptions, line: usize) {
        // A markdown file opens rendered, which has no lines to find in: show its source.
        if is_markdown(path) {
            let mut source = self.source;
            source.write().insert(path.to_string());
        }
        self.open(path.to_string());
        self.handle.find(path, query, options, line);
    }

    /// A menu item of the tree row or tab menu, for `path` (relative to the session folder).
    /// `ShowInTree` is the panel's: it owns the tree.
    pub fn run_action(&self, action: FileAction, path: &str) {
        if let Some(text) = copy_text_for(action, &self.cwd, path) {
            forge_entities::copy_text(&text);
            return;
        }
        match action {
            FileAction::Attach => self.attach_file(path),
            FileAction::OpenVsCode => self.workspace.open_with_app(&absolute(&self.cwd, path), Some("Visual Studio Code")),
            FileAction::OpenDefault => self.workspace.open_with_app(&absolute(&self.cwd, path), None),
            FileAction::Reveal => self.reveal(path),
            FileAction::FindInFile => self.find_in_file(path, "", forge_ports::SearchOptions::default(), 1),
            FileAction::CopyContents => {
                let (ctx, root, relative) = (self.clone(), self.cwd.clone(), path.to_string());
                let files = self.files();
                spawn(async move {
                    match run_blocking(move || files.read(&root, &relative)).await {
                        Some(Ok(content)) => match contents_to_copy(&content) {
                            Ok(text) => forge_entities::copy_text(&text),
                            Err(message) => ctx.workspace.report(message),
                        },
                        Some(Err(error)) => ctx.workspace.report(error.to_string()),
                        None => {}
                    }
                });
            }
            // See `reload_from_menu`. A tab with no editor state (an image, a binary, a failed read, or
            // no editor bundle) has nothing typed to lose: it is read again at once.
            FileAction::Reload => {
                if self.docs.peek().contains_key(path) && !*self.unavailable.peek() {
                    self.ask(path, Pending::Reload);
                } else if self.kinds.peek().contains_key(path) {
                    self.read_and_apply(path.to_string(), false);
                } else {
                    self.open(path.to_string());
                }
            }
            FileAction::Close => self.close(path.to_string()),
            FileAction::KeepOpen => self.keep(path),
            FileAction::CloseOthers => self.close_others(path.to_string()),
            FileAction::CloseAll => self.close_all(),
            FileAction::CopyAbsolute | FileAction::CopyRelative | FileAction::CopyName | FileAction::ShowInTree => {}
        }
    }

    /// `Attach as context` on a tree row or a tab: the file becomes a chip in the message box.
    pub fn attach_file(&self, path: &str) {
        self.workspace.queue_file(self.id, absolute(&self.cwd, path));
    }

    // The absolute path of a tab, as a card holds it.
    fn absolute_string(&self, key: &str) -> String {
        self.cwd.join(key).to_string_lossy().to_string()
    }

    fn selected(&self) -> Option<String> {
        self.tabs.peek().selected().map(str::to_string)
    }

    // Whether the editor in the page is what the tab shows: a text tab, and for markdown only in source mode.
    fn editor_visible(&self, path: &str) -> bool {
        !*self.unavailable.peek()
            && self.kinds.peek().get(path).is_some_and(TabKind::shows_editor)
            && (!is_markdown(path) || self.source.peek().contains(path))
    }

    /// Opens a file from the tree, a search hit or a link in the chat.
    pub fn open(&self, path: String) {
        let preview_tabs = self.workspace.editor_prefs().preview_tabs;
        let mut tabs = self.tabs;
        let replaced = tabs.write().open(&path, preview_tabs);
        if let Some(old) = replaced {
            self.forget(&old);
        }
        self.show_selected();
    }

    pub fn select(&self, path: &str) {
        let mut tabs = self.tabs;
        tabs.write().select(path);
        self.show_selected();
    }

    /// Makes the selected tab show: reads it the first time, else puts its editor state back.
    pub fn show_selected(&self) {
        let Some(path) = self.selected() else { return };
        if self.kinds.peek().contains_key(&path) {
            if self.editor_visible(&path) {
                self.handle.show(&path);
            }
        } else {
            let mut kinds = self.kinds;
            kinds.write().insert(path.clone(), TabKind::Loading);
            self.read_and_apply(path, false);
        }
    }

    // Reads the file and applies the result when the tab is still open. `quiet`: the editor only takes
    // the text when the person has no edit waiting (a refresh must never replace what was typed).
    fn read_and_apply(&self, path: String, quiet: bool) {
        let ctx = self.clone();
        spawn(async move {
            let (files, root, relative) = (ctx.files(), ctx.cwd.clone(), path.clone());
            let Some(result) = run_blocking(move || files.read(&root, &relative)).await else { return };
            if ctx.tabs.peek().contains(&path) {
                ctx.apply(path, result, quiet);
            }
        });
    }

    fn apply(&self, path: String, result: Result<FileContent, FilesError>, quiet: bool) {
        let (mut kinds, mut docs) = (self.kinds, self.docs);
        match result {
            Ok(FileContent::Text { text, version, lossy }) => {
                let kind = TabKind::Text { lossy, anyway: false };
                let read_only = kind.read_only();
                docs.write().insert(path.clone(), TabDoc::new(version, &text));
                kinds.write().insert(path.clone(), kind);
                self.take_text(&path, &normalize_newlines(&text), read_only, quiet);
            }
            Ok(other) => {
                docs.write().remove(&path);
                kinds.write().insert(path, kind_of(other));
            }
            Err(error) => {
                kinds.write().insert(path, TabKind::Failed(error.to_string()));
            }
        }
    }

    // The text of a file that was just read, line ends as `\n`: markdown is rendered, and the editor
    // takes it when it is the selected tab's view.
    fn take_text(&self, path: &str, text: &str, read_only: bool, quiet: bool) {
        if is_markdown(path) {
            let mut markdown = self.markdown;
            markdown.write().insert(path.to_string(), render_markdown(text));
        }
        if *self.unavailable.peek() {
            let language = if text.len() > HIGHLIGHT_LIMIT { "" } else { language_of(path) };
            let numbers = (1..=text.lines().count().max(1)).map(|n| n.to_string()).collect::<Vec<_>>().join("\n");
            let mut fallback = self.fallback;
            fallback.write().insert(path.to_string(), (code_html(language, text), numbers));
            return;
        }
        if self.selected().as_deref() == Some(path) && self.editor_visible(path) {
            self.handle.load(path, text, editor_language(path), read_only, quiet, false);
        }
    }

    /// "Show as text anyway" on a binary file: read only, never written back.
    pub fn show_anyway(&self, path: String) {
        let ctx = self.clone();
        spawn(async move {
            let (files, root, relative) = (ctx.files(), ctx.cwd.clone(), path.clone());
            let Some(result) = run_blocking(move || files.read_text_anyway(&root, &relative)).await else { return };
            let (mut kinds, mut docs) = (ctx.kinds, ctx.docs);
            match result {
                Ok(FileContent::Text { text, version, .. }) => {
                    let kind = TabKind::Text { lossy: false, anyway: true };
                    let read_only = kind.read_only();
                    docs.write().insert(path.clone(), TabDoc::new(version, &text));
                    kinds.write().insert(path.clone(), kind);
                    ctx.take_text(&path, &normalize_newlines(&text), read_only, false);
                }
                Ok(other) => {
                    kinds.write().insert(path, kind_of(other));
                }
                Err(error) => {
                    kinds.write().insert(path, TabKind::Failed(error.to_string()));
                }
            }
        });
    }

    pub fn reveal(&self, path: &str) {
        self.workspace.reveal(&self.cwd.join(path));
    }

    /// A message from the page.
    pub fn on_event(&self, event: FromPage) {
        match event {
            FromPage::Ready => {
                let mut ready = self.ready;
                ready.set(true);
            }
            FromPage::Unavailable => {
                let mut unavailable = self.unavailable;
                unavailable.set(true);
                // Tabs read before this was known get their plain viewer text now.
                for path in self.tabs.peek().paths() {
                    self.read_and_apply(path, false);
                }
            }
            FromPage::Dirty { key } => self.dirty(key),
            FromPage::Change { key, s } => self.change(key, s, false),
            FromPage::Save { key, s } => self.change(key, s, true),
            FromPage::Flush { key, s } => self.flushed(key, s),
            // The page never loaded this tab (it was not the visible one when its file was read).
            FromPage::Missing { key } => self.read_and_apply(key, false),
            // A quiet refresh found an edit waiting: the disk changed and so did the editor.
            FromPage::Skipped { key } => self.with_doc(&key, |doc| doc.write_failed(&WriteError::Changed)),
            FromPage::Attach { key, from, to, text } => {
                self.workspace.add_excerpt(self.id, self.absolute_string(&key), LineRange::new(from, to), text);
            }
            FromPage::Comment { key, from, to, text, id } => match id {
                Some(card_id) => self.workspace.update_comment(self.id, card_id, text),
                None => {
                    self.workspace.add_comment(self.id, self.absolute_string(&key), LineRange::new(from, to), text);
                }
            },
            FromPage::CommentDelete { id, .. } => self.workspace.remove_card(self.id, id),
        }
    }

    // The first edit of a tab: it stops being a preview tab, and the disk is checked for a conflict.
    fn dirty(&self, key: String) {
        let mut tabs = self.tabs;
        tabs.write().keep(&key);
        let ctx = self.clone();
        spawn(async move {
            let (files, root, relative) = (ctx.files(), ctx.cwd.clone(), key.clone());
            let Some(result) = run_blocking(move || files.stat(&root, &relative)).await else { return };
            ctx.with_doc(&key, |doc| doc.on_dirty(result.map_err(|error| error.to_string())));
        });
    }

    // The timer's text (or `⌘S` with `manual`): written unless auto save is off or a banner waits.
    fn change(&self, key: String, text: String, manual: bool) {
        let auto_save = self.workspace.editor_prefs().auto_save || manual;
        let allowed = self.docs.peek().get(&key).is_some_and(|doc| doc.may_write(auto_save) && doc.differs(&text));
        if allowed {
            self.write(key, text, false, false);
        }
    }

    // Writes `text` (the editor's text, line ends as `\n`); with `close` the tab closes when the write worked
    // (and the queue stops when it did not). A tab that must not be written (read only) is never written.
    // One write per tab at a time: a second waits for the first and then uses the version it left.
    fn write(&self, key: String, text: String, overwrite: bool, close: bool) {
        if self.kinds.peek().get(&key).is_some_and(TabKind::read_only) {
            return;
        }
        let ctx = self.clone();
        spawn(async move {
            while ctx.writing.peek().contains(&key) {
                tokio::time::sleep(Duration::from_millis(30)).await;
            }
            let mut writing = ctx.writing;
            writing.write().insert(key.clone());
            let prepared = {
                let docs = ctx.docs.peek();
                docs.get(&key).map(|doc| (if overwrite { None } else { Some(doc.loaded) }, doc.to_disk(&text)))
            };
            let Some((expected, body)) = prepared else {
                writing.write().remove(&key);
                return;
            };
            let (files, root, relative) = (ctx.files(), ctx.cwd.clone(), key.clone());
            let result = run_blocking(move || files.write(&root, &relative, &body, expected)).await;
            writing.write().remove(&key);
            let Some(result) = result else { return };
            let saved = result.is_ok();
            let mut docs = ctx.docs;
            if let Some(doc) = docs.write().get_mut(&key) {
                match result {
                    Ok(version) => doc.written(version, &text),
                    Err(error) => doc.write_failed(&error),
                }
            }
            if !close {
                return;
            }
            if saved {
                ctx.close_now(&key);
                ctx.pump();
            } else {
                // The tab stays, with its banner on screen.
                let mut queue = ctx.queue;
                queue.write().clear();
                ctx.select(&key);
            }
        });
    }

    fn flushed(&self, key: String, text: Option<String>) {
        let mut pending = self.pending;
        let Some(what) = pending.write().remove(&key) else { return };
        match what {
            Pending::Save { overwrite } => {
                if let Some(text) = text {
                    self.write(key, text, overwrite, false);
                }
            }
            Pending::Close => self.decide_close(key, text),
            Pending::SaveAndClose => {
                if let Some(text) = text {
                    self.write(key, text, false, true);
                }
            }
            Pending::Render => {
                if let Some(text) = text {
                    let mut markdown = self.markdown;
                    markdown.write().insert(key, render_markdown(&text));
                }
            }
            Pending::Refresh => self.refresh(key, text),
            Pending::Reload => self.reload_from_menu(key, text),
        }
    }

    fn ask(&self, key: &str, what: Pending) {
        let mut pending = self.pending;
        let replaces = pending.peek().get(key).is_none_or(|waiting| what.replaces(*waiting));
        if !replaces {
            return;
        }
        pending.write().insert(key.to_string(), what);
        self.handle.flush(key);
    }

    /// The banner's Overwrite, and Retry after a failed save: write what the editor holds now.
    pub fn save_from_editor(&self, key: &str, overwrite: bool) {
        if !overwrite {
            self.with_doc(key, TabDoc::clear_banner);
        }
        self.ask(key, Pending::Save { overwrite });
    }

    /// The banner's Cancel: drop the edits and show the file on disk.
    pub fn reload(&self, key: String) {
        self.read_and_apply(key, false);
    }

    pub fn dismiss_banner(&self, key: &str) {
        self.with_doc(key, TabDoc::clear_banner);
    }

    /// The tab's `×`, the tab menu, and the queue of a close others or close all.
    pub fn close(&self, path: String) {
        let has_editor_state = self.docs.peek().contains_key(&path) && !*self.unavailable.peek();
        if has_editor_state {
            self.ask(&path, Pending::Close);
        } else {
            self.close_now(&path);
            self.pump();
        }
    }

    // The page's text decides: a tab whose text is what is on disk closes at once; otherwise it is
    // saved (auto save on, no banner) or the person is asked.
    fn decide_close(&self, key: String, text: Option<String>) {
        let differs = match (&text, self.docs.peek().get(&key)) {
            (Some(text), Some(doc)) => doc.differs(text),
            _ => false,
        };
        if !differs {
            self.close_now(&key);
            self.pump();
            return;
        }
        let text = text.unwrap_or_default();
        let auto_save = self.workspace.editor_prefs().auto_save;
        let may_write = self.docs.peek().get(&key).is_some_and(|doc| doc.may_write(auto_save));
        if may_write {
            self.write(key, text, false, true);
        } else {
            self.select(&key);
            let mut confirm = self.confirm;
            confirm.set(Some((key, text)));
        }
    }

    /// Save on the close question.
    pub fn confirm_save(&self) {
        let mut confirm = self.confirm;
        let answered = confirm.write().take();
        // The question is not modal, so the person may have typed since it was asked: save what the
        // page holds now, not the text from then.
        if let Some((key, _)) = answered {
            self.ask(&key, Pending::SaveAndClose);
        }
    }

    /// Don't save on the close question.
    pub fn confirm_discard(&self) {
        let mut confirm = self.confirm;
        let answered = confirm.write().take();
        if let Some((key, _)) = answered {
            self.close_now(&key);
            self.pump();
        }
    }

    /// Cancel on the close question: the tab stays and the rest of the queue is dropped.
    pub fn confirm_cancel(&self) {
        let (mut confirm, mut queue) = (self.confirm, self.queue);
        confirm.set(None);
        queue.write().clear();
    }

    fn close_now(&self, path: &str) {
        let mut tabs = self.tabs;
        tabs.write().close(path);
        self.forget(path);
        self.show_selected();
    }

    // Everything remembered about a tab, here and in the page.
    fn forget(&self, path: &str) {
        self.handle.forget(path);
        let (mut kinds, mut docs, mut markdown, mut source, mut pending, mut fallback) =
            (self.kinds, self.docs, self.markdown, self.source, self.pending, self.fallback);
        kinds.write().remove(path);
        docs.write().remove(path);
        markdown.write().remove(path);
        source.write().remove(path);
        pending.write().remove(path);
        fallback.write().remove(path);
    }

    pub fn close_others(&self, keep: String) {
        let mut tabs = self.tabs;
        tabs.write().select(&keep);
        let others: Vec<String> = self.tabs.peek().paths().into_iter().filter(|path| *path != keep).collect();
        self.close_many(others);
    }

    pub fn close_all(&self) {
        let all = self.tabs.peek().paths();
        self.close_many(all);
    }

    fn close_many(&self, paths: Vec<String>) {
        let mut queue = self.queue;
        queue.set(paths);
        self.pump();
    }

    // Closes the next tab of the queue; each close continues the queue when it finishes.
    fn pump(&self) {
        let mut queue = self.queue;
        let next = {
            let mut waiting = queue.write();
            if waiting.is_empty() {
                None
            } else {
                Some(waiting.remove(0))
            }
        };
        if let Some(path) = next {
            self.close(path);
        }
    }

    pub fn keep(&self, path: &str) {
        let mut tabs = self.tabs;
        tabs.write().keep(path);
    }

    pub fn reorder(&self, from: &str, to: &str) {
        let mut tabs = self.tabs;
        tabs.write().reorder(from, to);
    }

    /// The markdown `</>` button: source (editable) or rendered.
    pub fn toggle_source(&self, path: &str) {
        let mut source = self.source;
        let is_source = source.peek().contains(path);
        if is_source {
            source.write().remove(path);
            self.ask(path, Pending::Render);
        } else {
            source.write().insert(path.to_string());
            self.show_selected();
        }
    }

    /// A tool call of the session finished: tabs with no edits are read again when the disk changed.
    pub fn refresh_all(&self) {
        for path in self.tabs.peek().paths() {
            if self.docs.peek().contains_key(&path) {
                if *self.unavailable.peek() {
                    self.refresh(path, None);
                } else {
                    self.ask(&path, Pending::Refresh);
                }
            }
        }
    }

    // `Reload file`: a tab with no edits shows the disk text; one with edits gets the conflict banner
    // (Cancel shows the disk text, Overwrite saves what is typed), so nothing typed is lost.
    fn reload_from_menu(&self, key: String, text: Option<String>) {
        let dirty = match (&text, self.docs.peek().get(&key)) {
            (Some(text), Some(doc)) => doc.differs(text),
            _ => false,
        };
        if dirty {
            self.with_doc(&key, |doc| doc.write_failed(&WriteError::Changed));
        } else {
            self.read_and_apply(key, false);
        }
    }

    fn refresh(&self, key: String, text: Option<String>) {
        let clean = match (&text, self.docs.peek().get(&key)) {
            (Some(text), Some(doc)) => doc.banner.is_none() && !doc.differs(text),
            (None, Some(doc)) => doc.banner.is_none(),
            _ => false,
        };
        if !clean {
            return;
        }
        let ctx = self.clone();
        spawn(async move {
            let (files, root, relative) = (ctx.files(), ctx.cwd.clone(), key.clone());
            let Some(Ok(version)) = run_blocking(move || files.stat(&root, &relative)).await else { return };
            let changed = ctx.docs.peek().get(&key).is_some_and(|doc| doc.loaded != version);
            if !changed {
                return;
            }
            if ctx.selected().as_deref() != Some(key.as_str()) {
                // The page still holds the old text of a tab that is not shown: let it go.
                ctx.handle.forget(&key);
            }
            ctx.read_and_apply(key, true);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Pending; 6] = [
        Pending::Refresh,
        Pending::Reload,
        Pending::Render,
        Pending::Save { overwrite: false },
        Pending::Close,
        Pending::SaveAndClose,
    ];

    #[test]
    fn a_refresh_never_replaces_a_close_or_a_save_that_waits_for_the_page() {
        for waiting in [Pending::Close, Pending::SaveAndClose, Pending::Save { overwrite: true }, Pending::Render] {
            assert!(!Pending::Refresh.replaces(waiting), "{waiting:?}");
        }
    }

    #[test]
    fn a_close_replaces_a_refresh_or_a_render_and_a_save_replaces_a_refresh() {
        assert!(Pending::Close.replaces(Pending::Refresh));
        assert!(Pending::Close.replaces(Pending::Render));
        assert!(Pending::Save { overwrite: false }.replaces(Pending::Refresh));
        assert!(Pending::SaveAndClose.replaces(Pending::Close));
    }

    #[test]
    fn a_reload_asked_from_the_menu_replaces_a_quiet_refresh_but_not_a_close_or_a_save() {
        assert!(Pending::Reload.replaces(Pending::Refresh));
        assert!(!Pending::Reload.replaces(Pending::Close));
        assert!(!Pending::Reload.replaces(Pending::Save { overwrite: false }));
        assert!(Pending::Close.replaces(Pending::Reload));
    }

    #[test]
    fn the_same_kind_of_question_does_not_replace_itself() {
        for pending in ALL {
            assert!(!pending.replaces(pending), "{pending:?}");
        }
    }

    #[test]
    fn an_overwrite_is_not_swallowed_by_a_plain_save_asked_later() {
        assert!(!Pending::Save { overwrite: false }.replaces(Pending::Save { overwrite: true }));
    }

    #[test]
    fn a_tree_or_tab_path_is_taken_from_the_session_folder() {
        assert_eq!(absolute(std::path::Path::new("/p"), "src/a.py"), PathBuf::from("/p/src/a.py"));
    }

    #[test]
    fn the_names_the_copy_actions_put_on_the_clipboard() {
        let cwd = std::path::Path::new("/p");
        assert_eq!(copy_text_for(FileAction::CopyAbsolute, cwd, "src/a b.py"), Some("/p/src/a b.py".to_string()));
        assert_eq!(copy_text_for(FileAction::CopyRelative, cwd, "src/a b.py"), Some("src/a b.py".to_string()));
        assert_eq!(copy_text_for(FileAction::CopyName, cwd, "src/a b.py"), Some("a b.py".to_string()));
        assert_eq!(copy_text_for(FileAction::Close, cwd, "src/a b.py"), None);
    }

    #[test]
    fn only_a_text_file_can_be_copied_as_contents() {
        use forge_ports::{FileContent, FileVersion};
        let version = FileVersion { modified_ns: 1, len: 3 };
        assert_eq!(contents_to_copy(&FileContent::Text { text: "abc".into(), version, lossy: false }), Ok("abc".to_string()));
        assert!(contents_to_copy(&FileContent::Binary).is_err());
        assert!(contents_to_copy(&FileContent::TooLarge).is_err());
        assert!(contents_to_copy(&FileContent::Image { bytes: vec![1], mime: "image/png".into() }).is_err());
    }
}
