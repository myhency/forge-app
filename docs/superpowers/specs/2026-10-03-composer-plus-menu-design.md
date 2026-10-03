# Plan 18: the composer `+` menu and environment chips (CMP-01, CMP-06)

Plan 14's roadmap put the `+` menu at Plan 18. The user chose on 2026-10-03: slash commands, attach file or photo, add folder, and the connectors and plugins submenus. The GitHub issue item and voice input (CMP-02) stay out. The environment chips (CMP-06) are in.

Evidence: `docs/superpowers/roadmap/observations.md`, round 1 ("`+` menu"): rows "파일 또는 사진 추가" (⌘U), "폴더 추가", "GitHub 이슈 가져오기" (greyed without a remote), "슬래시 명령어", "커넥터" (submenu), "플러그인" (submenu). The contents of the two submenus, and the "로컬" chip's menu, were never opened.

## What Forge has now

- `SendMessageBox` (`crates/forge-widgets/src/send_message.rs`): a text area with the slash-command popup, then a bottom row with the mode and model pickers, usage ring, Stop and Send.
- `SidecarPort::send_message(text: String)`: text only. The sidecar pushes `{ role: "user", content: text }` (`sidecar/src/session.ts`, `pushUserMessage`).
- `Picker` (`composer_controls.rs`): a button and a list above it, radio rows, number keys by physical key, Escape and outside click close it. It has no submenu and no rows without a check.
- A folder picker port (`FolderPickerPort`) for the new-session screen. No file picker.

## SDK facts (read from `sdk.d.ts` of the pinned SDK)

- `SDKUserMessage.message` is a Messages API `MessageParam`: `content` may be an array of blocks, including `image` blocks (base64 source).
- `additionalDirectories` is a start option only (as `--add-dir`; missing ones are skipped). `Query` has no method to add an arbitrary folder to a running session: `register_repo_root` accepts only a strict subdirectory of cwd or of a launch directory, and `addDirectories` exists only as a permission update returned from a permission prompt.
- Plain files need no SDK feature: the model reads a path with its Read tool.

## Design

**Menu.** A `+` button at the left of the composer's bottom row opens a menu above it. Rows, in the desktop order: Add file or photo (⌘U), Add folder, Slash commands, Connectors ▸, Plugins ▸. The GitHub row is not shown. `Picker` becomes a general menu component: rows may be radio rows (a check) or action rows (no check), and a row may open a second list beside the first. Keys stay by physical key; Escape closes the submenu first, then the menu.

**Attach file or photo.** The row (and ⌘U in the composer) opens a native file dialog through a new `FilePickerPort` (multiple files). Each pick becomes a chip above the text area, with a name and a × to remove it. Images (png, jpeg, gif, webp) are `Attachment::Image`, everything else `Attachment::File`. Send clears the chips. A draft with chips and no text can be sent.
- `send_message { id, text, attachments: [{ kind: "image"|"file", path }] }`. `attachments` is optional on the wire, so old callers stay valid.
- The sidecar builds the content: an image becomes a base64 `image` block (the sidecar reads the file; the media type comes from the extension); a file becomes a line `@<absolute path>` added to the text. With no images the content stays a plain string, as now.
- An image over 5 MB, or an unreadable file, is not sent: the sidecar answers with `setting_failed`-style non-fatal `attachment_failed { message }` and the text and the other attachments stay in the box (the unsent mechanism that Plan 12 added for failed sends).
- The chat view shows the sent attachments as chips on the user's message. They are written in the view's own message record; the transcript the SDK writes holds the image data and the `@path` text, and a reloaded session shows the text with the path as it is.

**Add folder.** The row opens the folder dialog.
- New-session screen: the folders are kept on the draft and sent as `additional_directories` in `start_session`, which the sidecar passes as `additionalDirectories`.
- Open session: the SDK cannot add one live, so Forge keeps the folder on the session view (shown as a chip, like an attachment, that is not cleared by Send) and adds a line `@<folder>` with the next message so the model knows. The model's tools reach it through the normal permission prompt. Whether this works with the real SDK is checked first in the plan (Task 1); if it does not, the row is greyed with "Choose the folder before the first message" in an open session.
- Folders chosen here are not stored: a resumed session starts without them.

**Slash commands.** The row puts `/` in the text area if it is empty (or after a space) and focuses it, which opens the existing popup. No new popup.

**Connectors and Plugins submenus.** Each lists what Settings knows (MCP servers; plugins) with a check on the enabled ones, read from the same state as Settings (spec §17). Choosing a row toggles it in Settings. It applies to the next new session, as in Settings; the submenu says so in a footer line. A running session is not changed. An empty list shows "None installed".

**Environment chips (new-session screen).** Above the composer: "Local" (a chip that is not a button, since no other environment exists), the folder name (a button that opens the folder dialog, as the select does today), and an add-folder button (the same row as above). The old select of folders is replaced by these.

**Left out.** GitHub issue; voice; attachments by paste or drag; the "로컬" menu; remembering folders; removing a folder from a running session.

## Errors

- File or folder dialog cancelled: nothing changes.
- Attachment unreadable or too big: the message is not sent, a banner says which file and why; text and chips stay.
- Sidecar write fails: as today (the session fails, the text comes back).

## Testing

- `forge-features`: attachment list logic (add, remove, kind by extension, duplicates ignored, clear on send), the `@path` text, the menu rows and their keys, the toggle rows from Settings state.
- `forge-ports`/adapters: the wire form of `send_message` with and without attachments; `start_session` with `additional_directories`.
- Sidecar (`node --test`): content building for no attachment, one image, one file, mixed; an oversize image and a missing file produce `attachment_failed` and send nothing.
- Real SDK, before calling it done (as in Plan 16): send one image and one text file in a throwaway folder and see the model describe them; add a folder in an open session and see the model reach it. Then drive the real app with the harness: open the menu, attach, remove, send. Anything not seen is written in the plan's "As built" section.

## As built

Built on branch `plan-18`: the `+` menu (five rows, ⌘U, physical-key digits 1 to 5, Connectors and Plugins submenus with a footer), attachment and folder chips, `send_message` with attachments, `start_session` with `additional_directories`, attachments shown on the sent message, and the environment chips on the new-session screen.

**Checked with the real SDK (0.3.283), before building (Plan Task 1).** A message with a base64 `image` block was described correctly; a file given as `@path` was read; a file outside the project folder raised one ordinary Read permission prompt and was then read. So an open session keeps the `@folder` hint and no row is greyed.

**Seen in the real app (harness, fixture home).** The `+` button in the new-session composer; the menu with the five rows in the spec's order, ⌘U on the first row and arrows on the last two; the Connectors submenu ("None installed" for the fixture, footer "Applies to the next new session."); "Slash commands" putting `/` in the text box and opening the command list; the "Local" and folder-name chips.

**Not seen in the real app.** Attaching a file through the native dialog and ⌘U (a ⌘U sent by script reached another application, so the dialog was never opened); removing a chip; the 5 MB refusal banner; adding a folder; sending a message with attachments from the app (the real SDK was checked by a script, not through Forge); the `@folder` line in an open session; a folder chosen on the new-session screen reaching `additionalDirectories` through the app; toggling a connector or plugin and seeing the next session start with it; ⌘U with the Korean input method; the menu in an open session (only the new-session screen was driven). These rest on unit tests: the wire form (Rust and sidecar), content building, attachment checks, `plan_send`, and the order "folders before the first send" in the live session.

**Deviations from the spec.**
1. No `attachment_failed` event: the UI checks each file (exists, image at most 5 MB) before sending and keeps text and chips on a refusal. If the sidecar still cannot read an image, it sends the message with `[could not read <path>]` in place of the image.
2. `FilePickerPort` is its own trait, implemented by `RfdFolderPicker`; `WorkspaceServices` has a `file_picker` field.
3. The new-session screen keeps its folder select and "Open folder…" button; the chips show "Local" and the folder name beside them. No second "change folder" button.
4. Chips and added folders live in the composer component (like the draft): switching to another session and back clears them.
