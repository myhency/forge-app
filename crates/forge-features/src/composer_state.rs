use std::path::{Path, PathBuf};

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::{with_cards, Attachment, Card};
use forge_entities::{mention_query, MenuAction, MenuKey, MentionRange, MentionRow};

use crate::attachments::{add_attachments, check_attachments, folder_label, with_folder_hints};
use crate::blocking::run_blocking;
use crate::mention_popup::{absolute_in, attached_in, byte_to_utf16, key_decision, mention_rows, provisional_cursor, utf16_to_byte, without_range, KeyDecision, MentionOutcome, MentionPopup, MentionStatus};
use crate::thumbnail::ComposerAttachment;
use crate::trust::{answer_card, apply_pick, classify_added_folder, resolved, FolderAdd, TrustChoice};
use crate::workspace::{use_workspace, Workspace, WorkspaceServices};
use forge_shared::{tr, Button, ButtonKind, Icon, IconKind, IconSize};

/// What a send takes out of the composer, decided without any UI.
#[derive(Debug, Clone, PartialEq)]
pub struct Outgoing {
    pub text: String,
    pub attachments: Vec<Attachment>,
}

/// `None` when there is nothing to send (no text, no chips and no cards). Folder hints go after the text and
/// only for folders not yet announced.
pub fn plan_send(text: &str, attachments: &[Attachment], unannounced: &[PathBuf], cards: &[Card]) -> Option<Outgoing> {
    let text = text.trim();
    if text.is_empty() && attachments.is_empty() && cards.is_empty() {
        return None;
    }
    Some(Outgoing { text: with_cards(cards, &with_folder_hints(text, unannounced)), attachments: attachments.to_vec() })
}

/// The `@` mention list as the composer shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct MentionView {
    pub range: MentionRange,
    pub rows: Vec<MentionRow>,
    pub highlight: usize,
    pub status: MentionStatus,
}

// Puts the text area's cursor at `utf16` once the text area holds the text of `length` units (the
// new text reaches the page after this call).
fn place_cursor(length: usize, utf16: usize) {
    document::eval(&format!(
        "let tries = 0; const place = () => {{ const a = document.activeElement; \
         if (a && a.tagName === 'TEXTAREA' && a.value.length === {length}) {{ a.setSelectionRange({utf16}, {utf16}); }} \
         else if (tries++ < 10) {{ requestAnimationFrame(place); }} }}; requestAnimationFrame(place);"
    ));
}

/// What is in the composer besides what the text area owns: chips for attached files and added
/// folders, and the small signals the `+` menu uses. Provided by the message box and read by the
/// menu, which lives in another crate.
#[derive(Clone, Copy)]
pub struct ComposerState {
    pub draft: Signal<String>,
    attachments: Signal<Vec<Attachment>>,
    /// Folders added in an open session, with whether the model was told about them yet.
    folders: Signal<Vec<(PathBuf, bool)>>,
    error: Signal<Option<String>>,
    popup_token: Signal<u32>,
    picking: Signal<bool>,
    /// A folder the person picked that is not trusted yet; the card under the chips asks about it.
    pending_trust: Signal<Option<PathBuf>>,
    /// The open session whose folder the `@` mention lists; `None` on the new-session screen.
    mention_session: Signal<Option<LiveSessionId>>,
    /// The text area's text and cursor (a byte offset) as last read; a cursor read for other text
    /// than the draft's (a pick, a send, a failed send put back) opens nothing.
    cursor: Signal<Option<(String, usize)>>,
    mention_popup: Signal<MentionPopup>,
    /// The number of the latest cursor read sent to the page.
    cursor_reads: Signal<u64>,
    services: Signal<WorkspaceServices>,
    workspace: Workspace,
}

pub fn use_composer_state_provider(draft: Signal<String>) -> ComposerState {
    // Read here, in the component: an event handler cannot call `use_context`.
    let services = use_context::<WorkspaceServices>();
    let workspace = use_workspace();
    let state = use_hook(move || ComposerState {
        draft,
        attachments: Signal::new(Vec::new()),
        folders: Signal::new(Vec::new()),
        error: Signal::new(None),
        popup_token: Signal::new(0),
        picking: Signal::new(false),
        pending_trust: Signal::new(None),
        mention_session: Signal::new(None),
        cursor: Signal::new(None),
        mention_popup: Signal::new(MentionPopup::default()),
        cursor_reads: Signal::new(0),
        services: Signal::new(services),
        workspace,
    });
    use_context_provider(move || state);
    state
}

pub fn use_composer_state() -> ComposerState {
    use_context::<ComposerState>()
}

impl ComposerState {
    pub fn attachments(&self) -> Vec<Attachment> {
        self.attachments.read().clone()
    }

    pub fn folders(&self) -> Vec<PathBuf> {
        self.folders.read().iter().map(|(path, _)| path.clone()).collect()
    }

    pub fn error(&self) -> Option<String> {
        self.error.read().clone()
    }

    pub fn popup_token(&self) -> u32 {
        *self.popup_token.read()
    }

    pub fn remove_attachment(&self, index: usize) {
        let mut list = self.attachments;
        if index < list.peek().len() {
            list.write().remove(index);
        }
    }

    pub fn remove_folder(&self, index: usize) {
        let mut list = self.folders;
        if index < list.peek().len() {
            list.write().remove(index);
        }
    }

    /// Puts the files of a message that could not be sent back as chips (none twice).
    pub fn restore_attachments(&self, attachments: Vec<Attachment>) {
        let mut list = self.attachments;
        add_attachments(&mut list.write(), attachments.into_iter().map(|a| a.path).collect());
    }

    /// Adds files as chips (none twice).
    pub fn add_paths(&self, paths: Vec<PathBuf>) {
        let mut list = self.attachments;
        add_attachments(&mut list.write(), paths);
    }

    pub fn dismiss_error(&self) {
        let mut error = self.error;
        error.set(None);
    }

    /// Opens the file dialog and adds what was chosen as chips.
    pub fn add_files(&self) {
        if *self.picking.peek() {
            return;
        }
        let services = (*self.services.peek()).clone();
        let mut picking = self.picking;
        let mut attachments = self.attachments;
        picking.set(true);
        // Not `spawn_forever`: the signals written below belong to this composer, so the task must end
        // with it (switching session while the dialog is open drops them; writing to a dropped signal
        // panics). The chips are discarded with the composer anyway.
        spawn(async move {
            let picked = services.file_picker.pick_files().await;
            picking.set(false);
            add_attachments(&mut attachments.write(), picked);
        });
    }

    /// Opens the folder dialog. A trusted folder (or one below it) becomes a chip at once; any other
    /// is held in `pending_trust` until the person answers the card.
    pub fn add_folder(&self) {
        if *self.picking.peek() {
            return;
        }
        let services = (*self.services.peek()).clone();
        let mut picking = self.picking;
        let mut folders = self.folders;
        let mut pending = self.pending_trust;
        picking.set(true);
        spawn(async move {
            let picked = services.folder_picker.pick_folder().await;
            picking.set(false);
            let Some(path) = picked else { return };
            // A list that cannot be read counts as empty: the person is asked, never waved through.
            let trusted = services.settings.load().await.map(|config| config.trusted_folders).unwrap_or_default();
            let added: Vec<PathBuf> = folders.peek().iter().map(|(existing, _)| existing.clone()).collect();
            // `canonicalize` can stall on a slow share: off the UI thread.
            let picked = path.clone();
            let verdict = run_blocking(move || classify_added_folder(&trusted, &added, &picked, &resolved(&picked))).await.unwrap_or(FolderAdd::NeedsAsking);
            apply_pick(&mut folders.write(), &mut pending.write(), verdict, path);
        });
    }

    pub fn pending_trust(&self) -> Option<PathBuf> {
        self.pending_trust.read().clone()
    }

    /// `Trust and add`: the chip now, and the folder on the trusted list. When the list cannot be
    /// saved the chip stays and a notice says so (the folder is then as for `Add this time`).
    pub fn trust_and_add(&self) {
        let Some(path) = self.answer(TrustChoice::TrustAndAdd) else { return };
        let services = (*self.services.peek()).clone();
        let workspace = self.workspace;
        // Root scope: the save must finish even if the composer goes away meanwhile.
        dioxus::core::spawn_forever(async move {
            let folder = run_blocking(move || resolved(&path)).await;
            let Some(folder) = folder else { return };
            let saved = services.settings.save_code_settings(|config| {
                forge_domain::add_trusted(&mut config.trusted_folders, folder);
            });
            if let Err(error) = saved.await {
                workspace.report(tr!("Could not save the trusted folder: {error}", error = error));
            }
        });
    }

    /// `Add this time`: the chip only; nothing is saved.
    pub fn add_once(&self) {
        self.answer(TrustChoice::AddOnce);
    }

    /// `Cancel` and Esc: the card goes, no chip.
    pub fn cancel_trust(&self) {
        self.answer(TrustChoice::Cancel);
    }

    fn answer(&self, choice: TrustChoice) -> Option<PathBuf> {
        let mut folders = self.folders;
        let mut pending = self.pending_trust;
        let to_save = answer_card(&mut folders.write(), &mut pending.write(), choice);
        crate::refocus::refocus_composer();
        to_save
    }

    // A message goes without the folder the card is still asking about, and the card goes with it.
    fn drop_pending_trust(&self) {
        let mut pending = self.pending_trust;
        if pending.peek().is_some() {
            pending.set(None);
        }
    }

    /// Lets the `@` mention list the files of this session's folder.
    pub fn set_mention_session(&self, id: LiveSessionId) {
        let mut session = self.mention_session;
        if *session.peek() != Some(id) {
            session.set(Some(id));
        }
    }

    pub fn mentions_on(&self) -> bool {
        self.mention_session.peek().is_some()
    }

    /// A keystroke changed the text to `text` (called before the draft takes it). The page's read of
    /// the cursor takes a round trip, so until it is back the cursor is worked out from the edit:
    /// the list stays open while typing and Enter keeps picking.
    pub fn note_typed(&self, text: &str) {
        let Some(id) = *self.mention_session.peek() else { return };
        let old = self.draft.peek().clone();
        let old_cursor = match &*self.cursor.peek() {
            Some((seen, cursor)) if *seen == old => *cursor,
            _ => old.len(),
        };
        let cursor = provisional_cursor(&old, old_cursor, text);
        self.keep_cursor(id, text.to_string(), Some(cursor));
    }

    /// Starts a read of the cursor and gives its number; `None` when none is needed (no mentions
    /// here, or no `@` in the text: a long pasted draft is not sent to the page for nothing).
    pub fn begin_cursor_read(&self) -> Option<u64> {
        if !self.mentions_on() || !self.draft.peek().contains('@') {
            return None;
        }
        let mut reads = self.cursor_reads;
        let next = reads.peek().wrapping_add(1);
        reads.set(next);
        Some(next)
    }

    /// The text area's text and cursor as read from the page (`utf16` is `selectionStart`, `None`
    /// while text is selected: no cursor, so no list). Only the latest read counts, and only while
    /// its text is still the draft's: an older read coming back late is ignored.
    pub fn note_cursor(&self, read: u64, text: String, utf16: Option<usize>) {
        let Some(id) = *self.mention_session.peek() else { return };
        if read != *self.cursor_reads.peek() || text != *self.draft.peek() {
            return;
        }
        let byte = utf16.and_then(|utf16| utf16_to_byte(&text, utf16));
        self.keep_cursor(id, text, byte);
    }

    // With an `@` at the cursor the session folder is walked, when that is due.
    fn keep_cursor(&self, id: LiveSessionId, text: String, byte: Option<usize>) {
        let opens = byte.and_then(|byte| mention_query(&text, byte)).is_some();
        let next = byte.map(|byte| (text, byte));
        let mut cursor = self.cursor;
        if *cursor.peek() != next {
            cursor.set(next);
        }
        if opens {
            self.workspace.refresh_session_files(id);
        }
    }

    // The `@…` span at the cursor, read for the draft as it is now.
    fn mention_range(&self) -> Option<MentionRange> {
        let draft = self.draft.read();
        let cursor = self.cursor.read();
        let (text, byte) = cursor.as_ref().filter(|(text, _)| *text == *draft)?;
        mention_query(text, *byte)
    }

    fn mention_folder(&self) -> Option<PathBuf> {
        let id = (*self.mention_session.read())?;
        let view = self.workspace.live_view(id)?;
        let folder = view.read().project_path.clone();
        Some(folder)
    }

    /// The open mention list, or `None` when it is closed.
    pub fn mention(&self) -> Option<MentionView> {
        let id = (*self.mention_session.read())?;
        let range = self.mention_range();
        let popup = self.mention_popup.read();
        if !popup.is_open(range.as_ref()) {
            return None;
        }
        let range = range?;
        let folder = self.mention_folder()?;
        let attachments = self.attachments.read();
        let (rows, status) = mention_rows(&self.workspace.session_files(id), &range.query, &|relative| attached_in(&folder, relative, &attachments));
        let highlight = popup.highlight(&range, rows.len());
        Some(MentionView { range, rows, highlight, status })
    }

    /// What a key in the text area does: the open list's, a send, or the text area's own.
    pub fn key_decision(&self, key: MenuKey, composing: bool, shift: bool) -> KeyDecision {
        key_decision(self.mention().is_some(), key, composing, shift)
    }

    /// A key the open list took.
    pub fn mention_act(&self, action: MenuAction) {
        let Some(view) = self.mention() else { return };
        let mut popup = self.mention_popup;
        let outcome = popup.write().act(&view.range, &view.rows, action);
        self.apply_mention(outcome);
    }

    /// A click on a row of the open list.
    pub fn mention_pick(&self, index: usize) {
        let Some(view) = self.mention() else { return };
        let mut popup = self.mention_popup;
        let outcome = popup.write().pick(&view.range, &view.rows, index);
        self.apply_mention(outcome);
    }

    // A pick takes the `@…` text out and attaches the file, the way the `+` menu does.
    fn apply_mention(&self, outcome: MentionOutcome) {
        let MentionOutcome::Picked(pick) = outcome else { return };
        let Some(folder) = self.mention_folder() else { return };
        let mut draft = self.draft;
        let text = without_range(&draft.peek(), &pick.range);
        let (length, at) = (byte_to_utf16(&text, text.len()), byte_to_utf16(&text, pick.range.start));
        let Some(path) = absolute_in(&folder, &pick.path) else { return };
        let mut cursor = self.cursor;
        cursor.set(Some((text.clone(), pick.range.start)));
        draft.set(text);
        let mut popup = self.mention_popup;
        popup.set(MentionPopup::default());
        self.add_paths(vec![path]);
        place_cursor(length, at);
    }

    /// Puts `/` in the text area (after a space when there is text) and asks it to open its
    /// command list.
    pub fn insert_slash(&self) {
        let mut draft = self.draft;
        let mut token = self.popup_token;
        let current = draft.peek().clone();
        if current.is_empty() {
            draft.set("/".to_string());
        } else if current.ends_with(char::is_whitespace) {
            draft.set(format!("{current}/"));
        } else {
            draft.set(format!("{current} /"));
        }
        let next = token.peek().wrapping_add(1);
        token.set(next);
    }

    /// Looks at the chips before a send. A refusal sets the error and leaves everything where it
    /// is; a pass clears the error and the chips.
    fn check_and_clear(&self, attachments: &[Attachment]) -> bool {
        let size_of = |path: &Path| std::fs::metadata(path).ok().filter(|m| m.is_file()).map(|m| m.len());
        let mut error = self.error;
        if let Err(message) = check_attachments(attachments, size_of) {
            error.set(Some(message));
            return false;
        }
        error.set(None);
        let mut list = self.attachments;
        list.write().clear();
        true
    }

    /// What a new session's first message takes: like `take_for_send`, but the folders do not become
    /// `@` lines: they go to the SDK as start options (`take_folders`).
    pub fn take_for_new_session(&self, text: &str) -> Option<Outgoing> {
        let attachments = self.attachments();
        let outgoing = plan_send(text, &attachments, &[], &[])?;
        if !self.check_and_clear(&attachments) {
            return None;
        }
        self.drop_pending_trust();
        Some(outgoing)
    }

    /// The folders for a new session: all of them, since no model has been told about them yet.
    pub fn take_folders(&self) -> Vec<PathBuf> {
        let mut list = self.folders;
        let folders = list.peek().iter().map(|(path, _)| path.clone()).collect();
        list.write().clear();
        folders
    }

    /// Takes what a send needs. Checks the chips first: a refusal leaves everything where it is
    /// and sets the error. On success the chips are cleared and the folders marked as told.
    /// Call it only at the moment of sending, so a failed send has lost nothing.
    pub fn take_for_send(&self, text: &str, cards: &[Card]) -> Option<Outgoing> {
        let attachments = self.attachments();
        let unannounced: Vec<PathBuf> =
            self.folders.peek().iter().filter(|(_, told)| !told).map(|(path, _)| path.clone()).collect();
        let outgoing = plan_send(text, &attachments, &unannounced, cards)?;
        if !self.check_and_clear(&attachments) {
            return None;
        }
        let mut folders = self.folders;
        for (_, told) in folders.write().iter_mut() {
            *told = true;
        }
        self.drop_pending_trust();
        Some(outgoing)
    }
}

/// The attached files and added folders as removable chips above the text.
#[component]
pub fn ComposerChips() -> Element {
    let composer = use_composer_state();
    let attachments = composer.attachments();
    let folders = composer.folders();
    if attachments.is_empty() && folders.is_empty() {
        return rsx! {};
    }
    rsx! {
        div { class: "chips",
            for (index, attachment) in attachments.iter().enumerate() {
                ComposerAttachment {
                    key: "a{attachment.path.display()}",
                    attachment: attachment.clone(),
                    on_remove: EventHandler::new(move |_| composer.remove_attachment(index)),
                }
            }
            for (index, folder) in folders.iter().enumerate() {
                span { key: "f{index}", class: "chip", title: "{folder.display()}",
                    span { class: "chip-kind", Icon { kind: IconKind::Folder, size: IconSize::Small } }
                    span { class: "chip-name", "{folder_label(folder)}" }
                    button {
                        class: "chip-remove",
                        r#type: "button",
                        "aria-label": "Remove",
                        onclick: move |_| composer.remove_folder(index),
                        "×"
                    }
                }
            }
        }
    }
}

/// The question for a folder that is not trusted yet, under the chips of the message box. Takes the
/// focus when it opens so Esc cancels.
#[component]
pub fn TrustCard() -> Element {
    let composer = use_composer_state();
    let Some(path) = composer.pending_trust() else {
        return rsx! {};
    };
    rsx! {
        // A one-item keyed list: a second pick changes the key, so the card is built again and takes
        // the focus again (a lone element with a new key would be reused as it is).
        for folder in [path] {
            div {
                key: "{folder.display()}",
                class: "trust-card",
                tabindex: "0",
                onmounted: move |mounted| async move {
                    let _ = mounted.set_focus(true).await;
                },
                onkeydown: move |event: KeyboardEvent| {
                    if event.key() == Key::Escape {
                        event.stop_propagation();
                        composer.cancel_trust();
                    }
                },
                div { class: "trust-question", {tr!("Allow Claude to use this folder and its subfolders?")} }
                div { class: "trust-path", title: "{folder.display()}", "{folder.display()}" }
                div { class: "trust-buttons",
                    Button { label: tr!("Cancel"), onclick: move |_| composer.cancel_trust() }
                    Button { label: tr!("Add this time"), onclick: move |_| composer.add_once() }
                    Button { label: tr!("Trust and add"), kind: ButtonKind::Primary, onclick: move |_| composer.trust_and_add() }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::AttachmentKind;

    fn image() -> Attachment {
        Attachment { path: "/x/a.png".into(), kind: AttachmentKind::Image }
    }

    #[test]
    fn nothing_typed_and_nothing_attached_sends_nothing() {
        assert_eq!(plan_send("   ", &[], &[], &[]), None);
    }

    #[test]
    fn chips_alone_can_be_sent() {
        let out = plan_send("", &[image()], &[], &[]).unwrap();
        assert_eq!(out.text, "");
        assert_eq!(out.attachments, vec![image()]);
    }

    #[test]
    fn text_is_trimmed_and_unannounced_folders_follow_it() {
        let out = plan_send("  fix  ", &[], &[PathBuf::from("/extra dir")], &[]).unwrap();
        assert_eq!(out.text, "fix\n@/extra dir");
    }

    #[test]
    fn a_folder_alone_does_not_make_a_message() {
        assert_eq!(plan_send("", &[], &[PathBuf::from("/extra")], &[]), None);
    }

    #[test]
    fn cards_alone_can_be_sent_and_come_before_the_text() {
        let card = forge_domain::Card {
            id: 1,
            kind: forge_domain::CardKind::Comment,
            path: "/p/a.py".into(),
            range: forge_domain::LineRange::new(3, 3),
            text: "why".into(),
        };
        let out = plan_send("", &[], &[], &[card.clone()]).unwrap();
        assert_eq!(forge_domain::split_cards(&out.text), (vec![forge_domain::Card { id: 0, ..card.clone() }], String::new()));
        let out = plan_send("  hi ", &[], &[], &[card]).unwrap();
        assert!(out.text.ends_with("\n\nhi"));
    }
}
