use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use dioxus::prelude::*;
use forge_entities::{code_html, is_markdown, language_of, render_markdown};
use forge_features::{run_blocking, EditorTabs, TabDoc, Workspace};
use forge_ports::{FileContent, FilesError, FilesPort};

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
    /// A markdown tab goes back to its rendered view.
    Render,
    /// A tool call finished: re-read the file when the tab is clean and the disk changed.
    Refresh,
}

/// Everything the panel does, in one place. It is `Clone` (signals are `Copy`), so event handlers
/// take a clone. The text of an open file lives in the editor in the page; Rust keeps a `TabDoc`
/// (the version and a hash of what is on disk) per tab.
#[derive(Clone)]
pub struct FilesCtx {
    pub workspace: Workspace,
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
            self.read_and_apply(path);
        }
    }

    // Reads the file and applies the result when the tab is still open.
    fn read_and_apply(&self, path: String) {
        let ctx = self.clone();
        spawn(async move {
            let (files, root, relative) = (ctx.files(), ctx.cwd.clone(), path.clone());
            let Some(result) = run_blocking(move || files.read(&root, &relative)).await else { return };
            if ctx.tabs.peek().contains(&path) {
                ctx.apply(path, result);
            }
        });
    }

    fn apply(&self, path: String, result: Result<FileContent, FilesError>) {
        let (mut kinds, mut docs) = (self.kinds, self.docs);
        match result {
            Ok(FileContent::Text { text, version, lossy }) => {
                let kind = TabKind::Text { lossy, anyway: false };
                let read_only = kind.read_only();
                docs.write().insert(path.clone(), TabDoc::new(version, &text));
                kinds.write().insert(path.clone(), kind);
                self.take_text(&path, &text, read_only);
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

    // The text of a file that was just read: markdown is rendered, and the editor takes it when it is
    // the selected tab's view.
    fn take_text(&self, path: &str, text: &str, read_only: bool) {
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
            self.handle.load(path, text, editor_language(path), read_only);
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
                    ctx.take_text(&path, &text, read_only);
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
                    self.read_and_apply(path);
                }
            }
            FromPage::Dirty { key } => self.dirty(key),
            FromPage::Change { key, s } => self.change(key, s, false),
            FromPage::Save { key, s } => self.change(key, s, true),
            FromPage::Flush { key, s } => self.flushed(key, s),
            // The page never loaded this tab (it was not the visible one when its file was read).
            FromPage::Missing { key } => self.read_and_apply(key),
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

    // Writes `text`; with `close` the tab closes when the write worked (and the queue stops when it did not).
    fn write(&self, key: String, text: String, overwrite: bool, close: bool) {
        let expected = if overwrite { None } else { self.docs.peek().get(&key).map(|doc| doc.loaded) };
        let ctx = self.clone();
        spawn(async move {
            let (files, root, relative, body) = (ctx.files(), ctx.cwd.clone(), key.clone(), text.clone());
            let Some(result) = run_blocking(move || files.write(&root, &relative, &body, expected)).await else { return };
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
            Pending::Render => {
                if let Some(text) = text {
                    let mut markdown = self.markdown;
                    markdown.write().insert(key, render_markdown(&text));
                }
            }
            Pending::Refresh => self.refresh(key, text),
        }
    }

    fn ask(&self, key: &str, what: Pending) {
        let mut pending = self.pending;
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
        self.read_and_apply(key);
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
        if let Some((key, text)) = answered {
            self.write(key, text, false, true);
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
            ctx.read_and_apply(key);
        });
    }
}
