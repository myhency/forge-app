# Keeping the Files panel's tabs and unsaved edits (Plan 25D) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Closing the Files panel, switching session or expanding another panel no longer drops the open tabs, and text typed with auto save off comes back.

**Architecture:** `Workspace` keeps a `FilesStore` per live session (tabs, markdown source set, unsaved text as drafts). The panel starts from the store and writes tab changes back; every text the editor page reports is noted as a draft when it differs from disk; when a tab's file is read again the draft is loaded into the editor (as an edited tab) instead of the disk text, with the conflict banner when the file changed meanwhile.

**Tech Stack:** Rust (Dioxus 0.7.10), CodeMirror 6 bundle (esbuild, jsdom tests).

**Spec:** `docs/superpowers/specs/2026-10-05-files-state-design.md`

## Global Constraints

- Branch `plan-25D` in `/Users/james/orca/workspaces/forge/plan-25B`; base `master`. Commit trailer: `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Nothing typed is ever dropped: a draft is cleared only when the text equals the disk text, was written, or the tab was closed/discarded on purpose (`Don't save`, the banner's Cancel, closing a tab).
- A read only tab (lossy, "show as text anyway") never gets a draft.
- After a bundle change: `cd tools/editor-bundle && npm run build && npm test` (the built `crates/forge-shared/assets/editor.js` is committed).
- Out of scope: undo history, scroll position, tabs across an app restart.

## Review Focus

- A draft for a file that changed on disk meanwhile must come back with the conflict banner and the typed text intact (Task 1 test of `restore`, Task 4 real window).
- A draft for a file that was deleted meanwhile must not be lost: the tab shows the read error and the draft stays in the store (Task 4).
- An empty text that differs from a non-empty disk text is a draft (Task 1 test).
- After a successful save (auto save on) no stale draft may come back and overwrite newer disk text (Task 1 test of note/clear order, Task 4).
- Closing a tab (`×`, close others, close all, `Don't save`) drops its draft; closing the panel does not (Task 4).
- A CRLF file: the draft is the normalised text and is written back with CRLF (existing `to_disk`); restoring must not flip it (Task 4 note).
- A session that is closed drops its store (Task 2).

---

### Task 1: The store and its rules (forge-features)

**Files:**
- Create: `crates/forge-features/src/files_store.rs`
- Modify: `crates/forge-features/src/lib.rs`

**Interfaces:**
- Consumes: `EditorTabs`, `text_hash` (`editor_doc.rs`), `forge_ports::FileVersion`.
- Produces: `Draft { text: String, base: FileVersion }`; `Restore { FromDisk, Draft(String), DraftWithConflict(String) }`; `FilesStore` (`Default`, `Clone`, `PartialEq`) with `pub tabs: EditorTabs`, `pub source: HashSet<String>`, `note_text(&mut self, path: &str, text: &str, disk_hash: u64, base: FileVersion)`, `clear_draft(&mut self, path: &str)`, `has_draft(&self, path: &str) -> bool`, `restore(&self, path: &str, fresh: FileVersion) -> Restore`.

- [ ] **Step 1: Failing tests** — create `files_store.rs` with only the tests module below, plus `mod files_store;` and `pub use files_store::{Draft, FilesStore, Restore};` in `lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor_doc::text_hash;

    fn v(n: u128) -> FileVersion {
        FileVersion { modified_ns: n, len: n as u64 }
    }

    #[test]
    fn a_text_that_differs_from_disk_becomes_a_draft_and_one_that_matches_clears_it() {
        let mut store = FilesStore::default();
        store.note_text("a.py", "typed", text_hash("disk"), v(1));
        assert!(store.has_draft("a.py"));
        store.note_text("a.py", "disk", text_hash("disk"), v(1));
        assert!(!store.has_draft("a.py"));
    }

    #[test]
    fn an_empty_text_that_differs_from_the_disk_is_a_draft() {
        let mut store = FilesStore::default();
        store.note_text("a.py", "", text_hash("disk"), v(1));
        assert_eq!(store.restore("a.py", v(1)), Restore::Draft(String::new()));
    }

    #[test]
    fn no_draft_means_the_disk_text() {
        assert_eq!(FilesStore::default().restore("a.py", v(1)), Restore::FromDisk);
    }

    #[test]
    fn a_draft_comes_back_as_it_was_when_the_disk_is_unchanged() {
        let mut store = FilesStore::default();
        store.note_text("a.py", "typed", text_hash("disk"), v(1));
        assert_eq!(store.restore("a.py", v(1)), Restore::Draft("typed".to_string()));
    }

    #[test]
    fn a_draft_comes_back_with_a_conflict_when_the_disk_changed_meanwhile() {
        let mut store = FilesStore::default();
        store.note_text("a.py", "typed", text_hash("disk"), v(1));
        assert_eq!(store.restore("a.py", v(2)), Restore::DraftWithConflict("typed".to_string()));
    }

    #[test]
    fn a_save_then_a_stale_note_order_never_resurrects_old_text() {
        let mut store = FilesStore::default();
        store.note_text("a.py", "typed", text_hash("disk"), v(1));
        // The write finished: the disk now holds "typed", and the next report equals it.
        store.clear_draft("a.py");
        store.note_text("a.py", "typed", text_hash("typed"), v(2));
        assert_eq!(store.restore("a.py", v(2)), Restore::FromDisk);
    }

    #[test]
    fn drafts_are_per_file() {
        let mut store = FilesStore::default();
        store.note_text("a.py", "x", text_hash("a"), v(1));
        assert!(!store.has_draft("b.py"));
        store.clear_draft("b.py");
        assert!(store.has_draft("a.py"));
    }
}
```

- [ ] **Step 2: Run, expect FAIL** — `cargo test -p forge-features files_store 2>&1 | grep -E "^error" | head -3`.

- [ ] **Step 3: Implement** (above the tests):

```rust
use std::collections::{HashMap, HashSet};

use forge_ports::FileVersion;

use crate::editor_doc::text_hash;
use crate::editor_tabs::EditorTabs;

/// Text typed in a tab that is not on disk, and the version of the file it was typed against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    pub text: String,
    pub base: FileVersion,
}

/// What a tab shows when its file is read again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Restore {
    /// No draft: the text on disk.
    FromDisk,
    /// The draft, and the file is as it was when the text was typed.
    Draft(String),
    /// The draft, but the file changed on disk since: the conflict banner goes with it.
    DraftWithConflict(String),
}

/// What the Files panel of one session keeps while the panel is not on screen: its tabs, the
/// markdown files shown as source, and the unsaved text of tabs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FilesStore {
    pub tabs: EditorTabs,
    pub source: HashSet<String>,
    drafts: HashMap<String, Draft>,
}

impl FilesStore {
    /// The editor reported `text` for `path`. `disk_hash` is the hash of the text on disk (line ends
    /// as `\n`), `base` the version it was read at.
    pub fn note_text(&mut self, path: &str, text: &str, disk_hash: u64, base: FileVersion) {
        if text_hash(text) == disk_hash {
            self.drafts.remove(path);
        } else {
            self.drafts.insert(path.to_string(), Draft { text: text.to_string(), base });
        }
    }

    pub fn clear_draft(&mut self, path: &str) {
        self.drafts.remove(path);
    }

    pub fn has_draft(&self, path: &str) -> bool {
        self.drafts.contains_key(path)
    }

    /// What to show for `path` now that its file was read at version `fresh`.
    pub fn restore(&self, path: &str, fresh: FileVersion) -> Restore {
        match self.drafts.get(path) {
            None => Restore::FromDisk,
            Some(draft) if draft.base == fresh => Restore::Draft(draft.text.clone()),
            Some(draft) => Restore::DraftWithConflict(draft.text.clone()),
        }
    }
}
```

(`editor_tabs` and `editor_doc` are private modules of the same crate: `use crate::...` works; `text_hash` may need `pub(crate)`/`pub` — it is `pub` already.)

- [ ] **Step 4: Run** — `cargo test -p forge-features files_store` → 7 pass.
- [ ] **Step 5: Commit** — `feat: the Files panel's store of tabs and unsaved text (Plan 25D)`.

---

### Task 2: The store in `Workspace`

**Files:**
- Modify: `crates/forge-features/src/workspace.rs` (field next to `cards`, wrappers, `close`)

**Interfaces:**
- Consumes: Task 1.
- Produces on `Workspace`: `files_view(id) -> (EditorTabs, HashSet<String>)`, `save_files_view(id, tabs: EditorTabs, source: HashSet<String>)`, `note_draft(id, path: &str, text: &str, disk_hash: u64, base: FileVersion)`, `clear_draft(id, path: &str)`, `restore_draft(id, path: &str, fresh: FileVersion) -> Restore`; `close(id)` also drops the session's store (and its unsent cards).

- [ ] **Step 1: Implement.** There is no pure logic here beyond Task 1, so the test is a compile plus the existing workspace tests: add the field `files_stores: Signal<HashMap<LiveSessionId, FilesStore>>` (init `Signal::new(HashMap::new())` in `use_workspace_provider`), imports (`FilesStore, Restore` from `crate::files_store`, `EditorTabs` from `crate::editor_tabs`, `FileVersion` from `forge_ports`), and next to the cards wrappers:

```rust
/// The Files panel's tabs and markdown source set of this session, as kept when the panel left.
pub fn files_view(&self, id: LiveSessionId) -> (EditorTabs, HashSet<String>) {
    let stores = self.files_stores.peek();
    stores.get(&id).map(|store| (store.tabs.clone(), store.source.clone())).unwrap_or_default()
}

pub fn save_files_view(&self, id: LiveSessionId, tabs: EditorTabs, source: HashSet<String>) {
    let mut stores = self.files_stores;
    let mut write = stores.write();
    let store = write.entry(id).or_default();
    store.tabs = tabs;
    store.source = source;
}

/// The editor reported `text` for a tab: kept as a draft when it is not what is on disk.
pub fn note_draft(&self, id: LiveSessionId, path: &str, text: &str, disk_hash: u64, base: FileVersion) {
    let mut stores = self.files_stores;
    stores.write().entry(id).or_default().note_text(path, text, disk_hash, base);
}

pub fn clear_draft(&self, id: LiveSessionId, path: &str) {
    let mut stores = self.files_stores;
    if let Some(store) = stores.write().get_mut(&id) {
        store.clear_draft(path);
    }
}

pub fn restore_draft(&self, id: LiveSessionId, path: &str, fresh: FileVersion) -> Restore {
    self.files_stores.peek().get(&id).map_or(Restore::FromDisk, |store| store.restore(path, fresh))
}
```

In `close(id)` (~line 852) add, before or after the `live` retain: `let mut stores = self.files_stores; stores.write().remove(&id); let mut cards = self.cards; cards.write().take(id);` (`Cards::take` exists; the returned cards are dropped).

- [ ] **Step 2: Run** — `cargo test -p forge-features` → pass; `cargo build` clean.
- [ ] **Step 3: Commit** — `feat: the Files store lives in Workspace per session (Plan 25D)`.

---

### Task 3: Loading a tab as edited (page and bridge)

**Files:**
- Modify: `crates/forge-widgets/src/files_editor.rs`, `tools/editor-bundle/src/main.js`, `tools/editor-bundle/test/bundle.test.mjs`, rebuilt `crates/forge-shared/assets/editor.js`

**Interfaces:**
- Produces: page `load(key, doc, language, readonly, ifClean, dirty)` (when `dirty` and not read only: sends `{ t: "dirty", key }` once and arms the normal 2 s `change` timer); Rust `load_command(key, doc, language, readonly, if_clean, dirty)` and `EditorHandle::load(key, doc, language, readonly, if_clean, dirty)`.

- [ ] **Step 1: Failing tests.** In `files_editor.rs` change the two `load_command` assertions to the six argument form and add:

```rust
#[test]
fn a_load_can_say_the_text_is_not_what_is_on_disk() {
    assert_eq!(load_command("a.py", "x", "python", false, false, true)["dirty"], serde_json::json!(true));
    assert_eq!(load_command("a.py", "x", "python", false, false, false)["dirty"], serde_json::json!(false));
}
```

(the existing expected JSON gets `"dirty": false` and the calls get a trailing `false`). In `bundle.test.mjs` append:

```js
test("a tab loaded as edited says dirty once, keeps its text and its change follows", async () => {
  const { w, editor, sent } = boot();
  editor.load("a.py", "typed text\n", "python", false, false, true);
  assert.deepEqual(plain(sent.filter((m) => m.t === "dirty")), [{ t: "dirty", key: "a.py" }]);
  assert.match(w.document.querySelector(".cm-content").textContent, /typed text/);
  await wait(2300);
  const change = plain(sent).find((m) => m.t === "change");
  assert.ok(change, "the 2 s change follows so auto save can write it");
  assert.equal(change.s, "typed text\n");
  editor.destroy();
});

test("a read only tab loaded as edited does not say dirty", () => {
  const { editor, sent } = boot();
  editor.load("a.py", "x\n", "python", true, false, true);
  assert.deepEqual(plain(sent).filter((m) => m.t === "dirty"), []);
  editor.destroy();
});
```

Run `cargo test -p forge-widgets files_editor 2>&1 | grep -E "^error" | head -2` and `cd tools/editor-bundle && npm run build && npm test 2>&1 | grep -E "^# (pass|fail)|^not ok"` → FAIL.

- [ ] **Step 2: Implement.** `load_command` gets `dirty: bool` and the field `"dirty": dirty`; `EditorHandle::load` takes and passes it; the bridge: `editor.load(m.key, m.doc, m.language, m.readonly, m.if_clean, m.dirty);`. Fix every caller (the compiler lists them: `take_text` in `files_state.rs` passes the new argument; Task 4 supplies the real value, for now pass `false`).

In `main.js`: `load(newKey, doc, language, readonly, ifClean, dirty)`; after the tab is set up at the end of the `whenIdle` body (after `applyPendingFocus`/`afterSwitch`), add:

```js
if (dirty && !readonly) {
  // The text is not what is on disk (a draft that came back): the first edit logic runs now.
  dirtySent = true;
  send({ t: "dirty", key: newKey });
  clearTimeout(timer);
  timer = setTimeout(fire, SAVE_DELAY_MS);
}
```

Update the header comment (`load(key, doc, language, readonly, ifClean, dirty)`).

- [ ] **Step 3: Run** — both suites pass; `cargo build` ok.
- [ ] **Step 4: Commit** (with the rebuilt `editor.js`) — `feat: a tab can be loaded as edited (Plan 25D)`.

---

### Task 4: Wire the panel to the store

**Files:**
- Modify: `crates/forge-widgets/src/files_state.rs`, `crates/forge-widgets/src/files_panel.rs`

**Interfaces:**
- Consumes: Tasks 1 to 3.

- [ ] **Step 1: Start from the store, write it back.** In `files_panel.rs`:

```rust
let (stored_tabs, stored_source) = use_hook(|| workspace.files_view(id));
let ctx = FilesCtx {
    ...
    tabs: use_signal(|| stored_tabs.clone()),
    ...
    source: use_signal(|| stored_source.clone()),
    ...
};
```

and after `ctx` exists:

```rust
// The tabs and the markdown source set go to the store whenever they change, so they outlive the panel.
{
    let (tabs, source) = (ctx.tabs, ctx.source);
    use_effect(move || {
        workspace.save_files_view(id, tabs.read().clone(), source.read().clone());
    });
}
// A panel that comes back shows the selected tab again (the others are read when shown).
{
    let ctx = ctx.clone();
    use_future(move || {
        let ctx = ctx.clone();
        async move { ctx.show_selected() }
    });
}
```

- [ ] **Step 2: Note drafts.** In `files_state.rs`:

```rust
// What the editor holds for a tab is kept as a draft when it differs from the disk text, so it
// survives the panel (and the page that holds it) going away.
fn note_draft(&self, key: &str, text: &str) {
    if self.kinds.peek().get(key).is_some_and(TabKind::read_only) {
        return;
    }
    let Some((hash, base)) = self.docs.peek().get(key).map(|doc| (doc.clean, doc.loaded)) else { return };
    self.workspace.note_draft(self.id, key, text, hash, base);
}
```

In `on_event`: `FromPage::Change { key, s }` and `FromPage::Save { key, s }` call `self.note_draft(&key, &s);` before `self.change(...)`; `FromPage::Flush { key, s }`: `if let Some(text) = &s { self.note_draft(&key, text); }` before `self.flushed(key, s)`.

- [ ] **Step 3: Clear drafts on purpose.** `forget(path)` (called on every close and on a preview tab being replaced) adds `self.workspace.clear_draft(self.id, path);`; `reload(key)` (the banner's Cancel) adds `self.workspace.clear_draft(self.id, &key);` as its first line; in `write(...)`, right after `doc.written(version, &text)` succeeds, call `ctx.workspace.clear_draft(ctx.id, &key);` (inside the `Ok(version)` arm, outside the `docs.write()` borrow: bind `let wrote = matches!(result, Ok(_));` before and clear after the block).

- [ ] **Step 4: Restore on read.** In `apply`, the text arm becomes:

```rust
Ok(FileContent::Text { text, version, lossy }) => {
    let kind = TabKind::Text { lossy, anyway: false };
    let read_only = kind.read_only();
    // A draft kept while the panel was away comes back (never into a read only tab).
    let restore = if read_only { Restore::FromDisk } else { self.workspace.restore_draft(self.id, &path, version) };
    docs.write().insert(path.clone(), TabDoc::new(version, &text));
    kinds.write().insert(path.clone(), kind);
    let (shown, dirty) = match restore {
        Restore::FromDisk => (normalize_newlines(&text), false),
        Restore::Draft(draft) => (draft, true),
        Restore::DraftWithConflict(draft) => {
            self.with_doc(&path, |doc| doc.write_failed(&WriteError::Changed));
            (draft, true)
        }
    };
    self.take_text(&path, &shown, read_only, quiet, dirty);
}
```

`take_text` gains `dirty: bool` and passes it to `self.handle.load(path, text, editor_language(path), read_only, quiet, dirty)`; the `show_anyway` caller passes `false`; the `Err` arm (a file that cannot be read) keeps `TabKind::Failed` and does **not** touch the draft (it stays in the store). Add `use forge_features::Restore;`.

- [ ] **Step 5: Run** — `cargo test --workspace 2>&1 | grep "test result" | awk '{s+=$4; f+=$6} END {print s" passed, "f" failed"}'` → 0 failed; `cargo build` clean.
- [ ] **Step 6: Commit** — `feat: the Files panel keeps its tabs and unsaved text while it is away (Plan 25D)`.

---

### Task 5: Check in the real window, fix, write down what was found

**Files:**
- Modify: `docs/superpowers/specs/2026-10-05-files-state-design.md` (As built + results), `docs/superpowers/roadmap/*.md` if it tracks it

- [ ] **Step 1: Whole run** — `cargo test --workspace` and `cd tools/editor-bundle && npm test` green; record the counts.
- [ ] **Step 2: Drive the app** with the harness (memory `driving-the-forge-app-visually`; screen unlocked; `scripts/harness/launch.sh`; Files from the session `⋮` menu, which toggles; create files in `/private/tmp/forge-harness/my-app`; `ctl.py` for right click and drag; `click.py` takes pixels of the 2400 wide capture, viewer coordinates times 1.2; Korean input: use key codes, a jamo typed is enough to make an edit; auto save is turned off from the Files `⋮` menu, `Auto save` item). Check, one line each:
  1. Open two files (tabs), select the second; close the Files panel (`×`) and open it again: both tabs are back, in order, the same one selected.
  2. Turn auto save off, type in a tab, close and reopen the panel: the typed text is there (the tab shows it as edited: it saves with `⌘S`).
  3. The same after switching to another session and back.
  4. The same after expanding another panel (Terminal) and returning.
  5. With a draft kept, change the file from a shell, close and reopen the panel: the text is kept and the conflict banner shows; `Overwrite` writes it.
  6. With a draft kept, delete the file from a shell, reopen the panel: the tab shows the read error; recreate the file and reload the tab: the draft is not lost silently (report what is seen).
  7. Closing a tab with `×` and `Don't save` drops the draft: reopening the panel shows no such tab; a markdown file shown as source stays source.
  8. With auto save on, type, wait 3 s, close and reopen the panel: the saved text shows and no stale draft appears.
  Each defect found: failing test if one can exist, fix, commit `fix: …`. Clean up the files and the harness afterwards.
- [ ] **Step 3: Docs** — "As built" and "Checked in the real window" in the spec; update the memory note of 25A's known limitation as solved.
- [ ] **Step 4: Commit** — `docs: Plan 25D checked in the real window`.
- [ ] **Step 5: Finish** — invoke `superpowers:finishing-a-development-branch`; base `master`. Do not merge without the user's word.
