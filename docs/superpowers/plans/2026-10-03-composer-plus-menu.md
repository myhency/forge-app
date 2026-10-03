# Composer `+` Menu and Environment Chips Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `+` button in the composer that opens a menu (attach file or photo, add folder, slash commands, connectors, plugins), attachment and folder chips, and environment chips on the new-session screen.

**Architecture:** Attachments travel as a list of `{ kind, path }` on `send_message`; the sidecar turns images into base64 `image` blocks and other files into `@path` lines. The UI checks attachments before sending, so a bad file never leaves the box. Folders go to the SDK as `additionalDirectories` at session start; in an open session the SDK cannot add one, so the next message carries an `@folder` line. The composer's input state moves into a small context (`ComposerState`) so the `+` menu, a widget, can use it.

**Tech Stack:** Rust (Dioxus desktop, tokio, serde, rfd), TypeScript sidecar (vitest), the Claude Agent SDK 0.3.283.

**Spec:** `docs/superpowers/specs/2026-10-03-composer-plus-menu-design.md`

## Global Constraints

- Menu rows, in this order and no others: Add file or photo (⌘U), Add folder, Slash commands, Connectors ▸, Plugins ▸. No GitHub row.
- Image types: png, jpeg (`jpg`, `jpeg`), gif, webp. Image size limit: 5 MB (5 * 1024 * 1024 bytes). Everything else is a plain file.
- Menu keys use the physical key (`event.code()`), never the character (Korean input method, Plan 15).
- `attachments` and `additional_directories` are optional on the wire: a command without them stays valid.
- The "로컬" chip is not a button (no other environment exists).
- Connector and plugin toggles apply to the next new session only; the submenu footer says so.
- Commit messages end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Run from the worktree root `/Users/james/orca/workspaces/forge/plan-18`. `sidecar/node_modules` does not exist here: run `npm ci` in `sidecar/` once (Task 1).

## Deviations from the spec

Two, both made while reading the code; the user approved the spec, so say so in the final report.

1. **No `attachment_failed` event.** The UI checks each file (exists, image under 5 MB) before it sends, and keeps text and chips in the box on a failure. That needs no new event and no "unsent attachments" state. If a file still cannot be read inside the sidecar (deleted in between), the sidecar sends the message with a line `[could not read <path>]` in place of the file.
2. **`FilePickerPort` is its own trait**, implemented by the same `RfdFolderPicker` struct as `FolderPickerPort`, and `WorkspaceServices` gets a `file_picker` field.

## Review Focus

- A message with chips and no text can be sent; an empty message with no chips cannot (Task 8 test).
- The same file added twice shows one chip (Task 5 test).
- A path with spaces or non-ASCII characters (`내 문서/사진 1.png`) survives the wire and becomes `@/path with spaces` text (Task 3, 4 and 7 tests).
- A file over 5 MB (image) is refused with the file name in the message; a non-image file of any size is not refused (Task 5 test).
- A refused or failed send keeps the text and the chips, because `take_for_send` clears them only after the check passes and only at the moment of sending (the refusal is the Task 7 `check_attachments` tests; the order is checked by hand in Task 13 step 4).

## File Structure

| File | Responsibility |
| --- | --- |
| `crates/forge-domain/src/attachment.rs` (new) | `Attachment`, `AttachmentKind` |
| `crates/forge-domain/src/session.rs` | `Message.attachments` |
| `sidecar/src/protocol.ts`, `sidecar/src/content.ts` (new), `sidecar/src/session.ts` | wire fields, building the SDK message content, `additionalDirectories` |
| `crates/forge-ports/src/sidecar_port.rs` | `send_message(text, attachments)`, `StartSessionParams.additional_directories` |
| `crates/forge-ports/src/file_picker_port.rs` (new) | `FilePickerPort` |
| `crates/forge-adapter-sidecar/src/{protocol,process}.rs` | wire form |
| `crates/forge-adapter-dialog/src/lib.rs` | `pick_files` through rfd |
| `crates/forge-application/src/{live_session,live_session_actor,event,test_support}.rs`, `use_cases/sidecar_commands.rs` | `LiveCommand::Send { text, attachments }`, `AddDirectories` |
| `crates/forge-features/src/attachments.rs` (new) | pure logic: kind by extension, add without duplicates, check, text with `@` lines |
| `crates/forge-features/src/composer_state.rs` (new) | `ComposerState` context: draft, attachments, folders, open-popup token, error |
| `crates/forge-features/src/send_message.rs`, `command_box.rs`, `new_session.rs`, `workspace.rs`, `settings.rs` | wiring |
| `crates/forge-widgets/src/composer_controls.rs`, `plus_menu.rs` (new), `chat_window.rs` | the menu and chips |
| `crates/forge-entities/src/message.rs` | attachment chips on a user message |
| `crates/forge-shared/src/theme.css` | styles |

---

### Task 1: Check what the real SDK does (throwaway, nothing committed)

The spec's folder behaviour and the image path were read from type files only. Look at them with the real SDK before building on them (as Plan 16 did).

**Files:**
- Create (not committed): `sidecar/probe-attach.mjs`

- [ ] **Step 1: Install and build**

Run: `cd sidecar && npm ci && npm run build`
Expected: `dist/index.js` exists, no errors.

- [ ] **Step 2: Write the probe**

```js
// sidecar/probe-attach.mjs: throwaway. Usage: node probe-attach.mjs <image.png> <outside-folder>
import { query } from "@anthropic-ai/claude-agent-sdk";
import { readFileSync, writeFileSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const [image, outside] = process.argv.slice(2);
const cwd = mkdtempSync(join(tmpdir(), "forge-probe-"));
writeFileSync(join(outside, "secret.txt"), "the word is PINEAPPLE");
writeFileSync(join(cwd, "note.txt"), "this file says KIWI");

const messages = [
  {
    type: "user",
    parent_tool_use_id: null,
    message: {
      role: "user",
      content: [
        { type: "image", source: { type: "base64", media_type: "image/png", data: readFileSync(image).toString("base64") } },
        { type: "text", text: `Describe the image in one sentence. Then read @${join(cwd, "note.txt")}. Then read ${join(outside, "secret.txt")} and tell me its word. If a tool is refused, say so.` },
      ],
    },
  },
];
async function* prompt() { for (const m of messages) yield m; }

const asked = [];
const q = query({
  prompt: prompt(),
  options: {
    cwd,
    permissionMode: "default",
    canUseTool: async (tool, input) => { asked.push(`${tool} ${JSON.stringify(input)}`); return { behavior: "allow", updatedInput: input }; },
  },
});
for await (const m of q) {
  if (m.type === "assistant") for (const b of m.message.content) if (b.type === "text") console.log("CLAUDE:", b.text);
  if (m.type === "result") break;
}
console.log("PERMISSION PROMPTS:", asked);
```

- [ ] **Step 3: Run it**

Run: `node probe-attach.mjs <any small png> /tmp/forge-outside` (create `/tmp/forge-outside` first; the scratchpad directory is fine too).
Expected, in this order of importance:
  1. The model describes the image (the base64 block works).
  2. It reads `note.txt` and says KIWI (the `@path` text works).
  3. Reading the outside file produces a permission prompt for a path outside cwd that Forge's existing prompt can answer, then PINEAPPLE.

- [ ] **Step 4: Decide and record**

If 1 and 2 hold, go on. If 3 holds, an open session keeps the `@folder` hint. If the outside file is refused without a prompt, change the "Add folder" row in Task 9 to be disabled in an open session with the tooltip "Choose folders before the first message", and delete the `@folder` hint code in Tasks 5 and 8. Write the outcome in one paragraph at the top of the "As built" section (Task 13). If 1 fails, stop and report: the image path needs another design.

- [ ] **Step 5: Remove the probe**

Run: `rm sidecar/probe-attach.mjs` (nothing to commit).

---

### Task 2: Domain types and the message's attachments

**Files:**
- Create: `crates/forge-domain/src/attachment.rs`
- Modify: `crates/forge-domain/src/lib.rs`, `crates/forge-domain/src/session.rs:36-52`
- Modify (add `attachments: Vec::new(),` to each `Message {` literal): `crates/forge-features/src/sidebar.rs:585`, `crates/forge-adapter-claude-fs/src/sessions.rs:278`, `crates/forge-application/src/live_session.rs:227`, `crates/forge-application/src/registry.rs:264-265`, `crates/forge-application/src/event.rs:26`

**Interfaces:**
- Produces: `forge_domain::Attachment { path: PathBuf, kind: AttachmentKind }`, `forge_domain::AttachmentKind::{Image, File}`, `Attachment::file_name(&self) -> String`, `Message.attachments: Vec<Attachment>`.

- [ ] **Step 1: Write the failing test** (in the new file, bottom)

```rust
use std::path::PathBuf;

/// What kind of thing a chip stands for. Decided by the extension, not by reading the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentKind {
    Image,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    pub path: PathBuf,
    pub kind: AttachmentKind,
}

impl Attachment {
    /// The last component of the path, for a chip. A path with none shows in full.
    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .map_or_else(|| self.path.to_string_lossy().to_string(), |name| name.to_string_lossy().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_name_is_the_last_component_even_with_spaces_and_hangul() {
        let attachment = Attachment { path: PathBuf::from("/내 문서/사진 1.png"), kind: AttachmentKind::Image };
        assert_eq!(attachment.file_name(), "사진 1.png");
    }

    #[test]
    fn a_path_with_no_file_name_shows_in_full() {
        let attachment = Attachment { path: PathBuf::from("/"), kind: AttachmentKind::File };
        assert_eq!(attachment.file_name(), "/");
    }
}
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p forge-domain attachment`
Expected: the module is not declared (compile error) until Step 3.

- [ ] **Step 3: Wire it in**

In `lib.rs` add `mod attachment;` and `pub use attachment::{Attachment, AttachmentKind};`. In `session.rs` add to `Message` the field and to `Message::new` its initializer:

```rust
    /// Files and images the person attached to this message (user messages only).
    pub attachments: Vec<crate::Attachment>,
```

```rust
            attachments: Vec::new(),
```

Then add `attachments: Vec::new(),` to every `Message {` literal listed above.

- [ ] **Step 4: Run all tests**

Run: `cargo test --workspace`
Expected: PASS (the new tests and every old one).

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: Attachment and Message.attachments

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Sidecar builds the message content and passes folders

**Files:**
- Create: `sidecar/src/content.ts`, `sidecar/test/content.test.ts`
- Modify: `sidecar/src/protocol.ts`, `sidecar/src/session.ts` (`send_message` case, `pushUserMessage`, `start` options), `sidecar/test/session.test.ts` (one test)

**Interfaces:**
- Produces: `WireAttachment = { kind: "image" | "file"; path: string }`; `SidecarCommand` `send_message` gains `attachments?: WireAttachment[]`; `start_session` gains `additional_directories?: string[]`; `buildContent(text, attachments, read?)` returning `string | ContentBlock[]`.

- [ ] **Step 1: Write the failing tests** (`sidecar/test/content.test.ts`)

```ts
import { describe, expect, it } from "vitest";
import { buildContent } from "../src/content.js";

const png = Buffer.from("PNGDATA");
const read = (path: string): Buffer => {
  if (path.includes("missing")) throw new Error("ENOENT");
  return png;
};

describe("buildContent", () => {
  it("is the plain text when there are no attachments", () => {
    expect(buildContent("hello", [], read)).toBe("hello");
  });

  it("adds an @path line for a file and keeps the content a string", () => {
    expect(buildContent("look", [{ kind: "file", path: "/work/a b/notes 1.txt" }], read)).toBe(
      "look\n@/work/a b/notes 1.txt",
    );
  });

  it("makes an image a base64 image block before the text", () => {
    expect(buildContent("what is this", [{ kind: "image", path: "/work/사진 1.PNG" }], read)).toEqual([
      { type: "image", source: { type: "base64", media_type: "image/png", data: png.toString("base64") } },
      { type: "text", text: "what is this" },
    ]);
  });

  it("maps jpg, jpeg, gif and webp to their media types", () => {
    const types = ["a.jpg", "a.jpeg", "a.gif", "a.webp"].map((name) => {
      const blocks = buildContent("", [{ kind: "image", path: `/x/${name}` }], read) as any[];
      return blocks[0].source.media_type;
    });
    expect(types).toEqual(["image/jpeg", "image/jpeg", "image/gif", "image/webp"]);
  });

  it("mixes an image and a file: the file line goes in the text block", () => {
    const content = buildContent(
      "both",
      [
        { kind: "image", path: "/x/a.png" },
        { kind: "file", path: "/x/b.txt" },
      ],
      read,
    ) as any[];
    expect(content[1]).toEqual({ type: "text", text: "both\n@/x/b.txt" });
  });

  it("says so in the text when a file cannot be read, and still sends the rest", () => {
    expect(buildContent("hi", [{ kind: "image", path: "/x/missing.png" }], read)).toBe(
      "hi\n[could not read /x/missing.png]",
    );
  });

  it("sends attachments alone when the text is empty", () => {
    expect(buildContent("", [{ kind: "file", path: "/x/b.txt" }], read)).toBe("@/x/b.txt");
  });
});
```

Add to `session.test.ts`, after the "delivers queued send_message" test:

```ts
  it("builds image content for a send_message with attachments and passes additional directories", async () => {
    let captured: any;
    const received: unknown[] = [];
    const queryFn: QueryFn = (args) => {
      captured = args.options;
      return toQueryHandle((async function* () {
        for await (const msg of args.prompt as AsyncIterable<{ message: { content: unknown } }>) {
          received.push(msg.message.content);
          return;
        }
      })());
    };

    const session = new SidecarSession(queryFn, () => {});
    const startPromise = session.handle({ ...startCommand, additional_directories: ["/extra"] });
    await session.handle({ type: "send_message", id: "2", text: "hi", attachments: [{ kind: "file", path: "/x/b.txt" }] });
    await startPromise;

    expect(captured.additionalDirectories).toEqual(["/extra"]);
    expect(received).toEqual(["hi\n@/x/b.txt"]);
  });
```

- [ ] **Step 2: Run to see them fail**

Run: `cd sidecar && npx vitest run test/content.test.ts test/session.test.ts`
Expected: FAIL (`../src/content.js` missing; type errors for the new fields).

- [ ] **Step 3: Implement**

`sidecar/src/content.ts`:

```ts
import { readFileSync } from "node:fs";
import { extname } from "node:path";
import type { WireAttachment } from "./protocol.js";

export type ContentBlock =
  | { type: "text"; text: string }
  | { type: "image"; source: { type: "base64"; media_type: string; data: string } };

const MEDIA_TYPES: Record<string, string> = {
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".gif": "image/gif",
  ".webp": "image/webp",
};

// The content of a user message: a plain string unless an image is attached. A file is an `@path`
// line for the model's Read tool. A file that cannot be read is named in the text instead of
// failing the message: the rest still goes.
export function buildContent(
  text: string,
  attachments: WireAttachment[],
  read: (path: string) => Buffer = readFileSync,
): string | ContentBlock[] {
  const lines = text === "" ? [] : [text];
  const images: ContentBlock[] = [];
  for (const attachment of attachments) {
    if (attachment.kind === "file") {
      lines.push(`@${attachment.path}`);
      continue;
    }
    try {
      const mediaType = MEDIA_TYPES[extname(attachment.path).toLowerCase()] ?? "image/png";
      images.push({
        type: "image",
        source: { type: "base64", media_type: mediaType, data: read(attachment.path).toString("base64") },
      });
    } catch {
      lines.push(`[could not read ${attachment.path}]`);
    }
  }
  const joined = lines.join("\n");
  if (images.length === 0) return joined;
  return [...images, { type: "text", text: joined }];
}
```

`protocol.ts`: add `export interface WireAttachment { kind: "image" | "file"; path: string; }`, change the command variants to `| { type: "send_message"; id: string; text: string; attachments?: WireAttachment[] }` and add `additional_directories?: string[];` to `start_session`.

`session.ts`: `case "send_message": this.pushUserMessage(buildContent(command.text, command.attachments ?? [])); return;`; change `pushUserMessage(text: string)` to `pushUserMessage(content: string | ContentBlock[])` and use `content` in the message; in the `options` object of `start` add `additionalDirectories: command.additional_directories ?? [],`; import `buildContent` and the `ContentBlock` type from `./content.js`.

- [ ] **Step 4: Run to see them pass**

Run: `cd sidecar && npx vitest run && npm run typecheck`
Expected: PASS, no type errors.

- [ ] **Step 5: Commit**

```bash
git add sidecar && git commit -m "feat: the sidecar builds image and file content and passes extra folders

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Port and wire form on the Rust side

**Files:**
- Modify: `crates/forge-ports/src/sidecar_port.rs`, `crates/forge-adapter-sidecar/src/protocol.rs`, `crates/forge-adapter-sidecar/src/process.rs`, `crates/forge-adapter-sidecar/tests/fixtures/fake_sidecar.js`, `crates/forge-adapter-sidecar/tests/contract_test.rs`
- Modify (compile fixes): every `start_session(StartSessionParams {` literal and every `send_message(` call: `forge-ports/src/sidecar_port.rs:95`, `forge-adapter-sidecar/tests/factory_test.rs:24,41`, `forge-adapter-sidecar/examples/spike_drive_sidecar.rs:18,29`, `forge-adapter-sidecar/src/protocol.rs:104,136`, `forge-application/src/command_probe.rs:45,140`, `forge-application/src/live_session.rs:240`, `forge-adapter-sidecar/tests/contract_test.rs` (many), `forge-application/src/live_session_actor.rs:184`, `forge-application/src/use_cases/session_lifecycle.rs:26,64`, `forge-application/src/use_cases/sidecar_commands.rs`, `forge-application/src/test_support.rs:115`

**Interfaces:**
- Consumes: `forge_domain::{Attachment, AttachmentKind}` (Task 2).
- Produces: `SidecarPort::send_message(&self, text: String, attachments: Vec<Attachment>)`; `StartSessionParams.additional_directories: Vec<String>`.

- [ ] **Step 1: Write the failing tests**

In `forge-adapter-sidecar/src/protocol.rs` tests (next to the `start_session` conversion tests), add:

```rust
    #[test]
    fn send_message_with_attachments_serialises_kind_and_path() {
        use forge_domain::{Attachment, AttachmentKind};
        let command = SidecarCommand::send_message(
            "7".to_string(),
            "look".to_string(),
            vec![
                Attachment { path: "/내 문서/사진 1.png".into(), kind: AttachmentKind::Image },
                Attachment { path: "/x/b.txt".into(), kind: AttachmentKind::File },
            ],
        );
        let json: serde_json::Value = serde_json::to_value(&command).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "type": "send_message", "id": "7", "text": "look",
                "attachments": [
                    { "kind": "image", "path": "/내 문서/사진 1.png" },
                    { "kind": "file", "path": "/x/b.txt" }
                ]
            })
        );
    }

    #[test]
    fn send_message_without_attachments_leaves_the_field_out() {
        let command = SidecarCommand::send_message("7".to_string(), "hi".to_string(), Vec::new());
        let json: serde_json::Value = serde_json::to_value(&command).unwrap();
        assert_eq!(json, serde_json::json!({ "type": "send_message", "id": "7", "text": "hi" }));
    }

    fn params_with(additional_directories: Vec<String>) -> StartSessionParams {
        StartSessionParams {
            cwd: "/tmp/project".to_string(),
            resume_session_id: None,
            permission_mode: PermissionMode::Default,
            model: None,
            extensions: ActiveExtensions::default(),
            additional_directories,
        }
    }

    #[test]
    fn start_session_leaves_additional_directories_out_when_there_are_none() {
        let json = serde_json::to_value(SidecarCommand::start_session("1".to_string(), params_with(Vec::new()))).unwrap();
        assert!(json.get("additional_directories").is_none());
    }

    #[test]
    fn start_session_carries_additional_directories_when_there_are_some() {
        let json = serde_json::to_value(SidecarCommand::start_session(
            "1".to_string(),
            params_with(vec!["/extra dir".to_string()]),
        ))
        .unwrap();
        assert_eq!(json["additional_directories"], serde_json::json!(["/extra dir"]));
    }
```

In `contract_test.rs` add, after the round-trip test:

```rust
#[tokio::test]
async fn attachments_and_extra_folders_reach_the_sidecar() {
    let sidecar = spawn_fake();
    let mut params = default_params();
    params.additional_directories = vec!["/extra".to_string()];
    sidecar.start_session(params).await.unwrap();
    sidecar
        .send_message(
            "TRIGGER_ATTACHED".to_string(),
            vec![forge_domain::Attachment { path: "/x/a b.png".into(), kind: forge_domain::AttachmentKind::Image }],
        )
        .await
        .unwrap();
    let events = collect_events(&sidecar, 2).await;
    assert!(events.iter().any(|event| matches!(
        event, SidecarEvent::AssistantDelta { text } if text == r#"attached:[{"kind":"image","path":"/x/a b.png"}] dirs:["/extra"]"#
    )), "{events:?}");
}
```

Use the file's existing helpers for spawning and reading events (look at `start_session_and_send_message_round_trip` at `contract_test.rs:26` and copy its first lines and its way of reading two events; the names `spawn_fake` and `collect_events` above stand for whatever those are).

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p forge-adapter-sidecar`
Expected: compile errors (no `send_message` constructor, no field).

- [ ] **Step 3: Implement**

`sidecar_port.rs`: add `use forge_domain::Attachment;` to the import, the field

```rust
    /// Folders the session may use besides its own (the SDK's `additionalDirectories`).
    pub additional_directories: Vec<String>,
```

and change the trait method to `async fn send_message(&self, text: String, attachments: Vec<Attachment>) -> Result<(), SidecarError>;`.

`adapter-sidecar/src/protocol.rs`: change the variant and `StartSession`:

```rust
    SendMessage {
        id: String,
        text: String,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        attachments: Vec<WireAttachment>,
    },
```

and in `StartSession` add `#[serde(skip_serializing_if = "Vec::is_empty")] additional_directories: Vec<String>,`. Add:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct WireAttachment {
    kind: WireAttachmentKind,
    path: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum WireAttachmentKind {
    Image,
    File,
}
```

and in `impl SidecarCommand`:

```rust
    pub fn send_message(id: String, text: String, attachments: Vec<forge_domain::Attachment>) -> Self {
        SidecarCommand::SendMessage {
            id,
            text,
            attachments: attachments
                .into_iter()
                .map(|attachment| WireAttachment {
                    kind: match attachment.kind {
                        forge_domain::AttachmentKind::Image => WireAttachmentKind::Image,
                        forge_domain::AttachmentKind::File => WireAttachmentKind::File,
                    },
                    path: attachment.path.to_string_lossy().to_string(),
                })
                .collect(),
        }
    }
```

and `additional_directories: params.additional_directories,` in `start_session`. `process.rs`: `async fn send_message(&self, text: String, attachments: Vec<forge_domain::Attachment>)` calls `self.send_command(SidecarCommand::send_message(id, text, attachments))`. Add `forge-domain` to `forge-adapter-sidecar/Cargo.toml` if it is not already a dependency.

`fake_sidecar.js`: in `send_message`, before the final echo:

```js
      if (command.text === "TRIGGER_ATTACHED") {
        emit({
          type: "assistant_delta",
          text: `attached:${JSON.stringify(command.attachments ?? [])} dirs:${JSON.stringify(started.additional_directories ?? [])}`,
        });
        return;
      }
```

Fix every compile error the signature change causes: add `additional_directories: Vec::new(),` to each `StartSessionParams` literal, `, Vec::new()` to each `send_message(` call, and in the mock expectations change `.withf(|text: &String| …)` to `.withf(|text: &String, _| …)` and `.returning(|_| …)` to `.returning(|_, _| …)`. In `test_support.rs` add a `Call::SendAttached(String, Vec<Attachment>)` variant; the fake's `send_message(text, attachments)` records `Call::Send(text)` when `attachments` is empty and `Call::SendAttached(text, attachments)` otherwise, so the existing assertions keep passing.

- [ ] **Step 4: Run the whole workspace**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: attachments and extra folders on the sidecar port and wire

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Live session carries attachments and folders

**Files:**
- Modify: `crates/forge-application/src/live_session.rs:56-90` (command, handle), `crates/forge-application/src/live_session_actor.rs`, `crates/forge-application/src/event.rs:25`, `crates/forge-application/src/use_cases/sidecar_commands.rs`, tests in `live_session.rs`

**Interfaces:**
- Consumes: Task 4's port.
- Produces: `LiveSessionHandle::send_with(&self, text: String, attachments: Vec<Attachment>)` (`send(text)` stays and calls it with none); `LiveSessionHandle::add_directories(&self, dirs: Vec<PathBuf>)`; `push_user_message(session, text, attachments)`.

- [ ] **Step 1: Write the failing tests** (in `live_session.rs` tests; copy the setup of `a_spawn_failure_keeps_the_unsent_text…` at line 663 for how a session and a fake control are made, and name them as that test does)

```rust
    #[tokio::test]
    async fn a_message_with_attachments_reaches_the_sidecar_and_the_history() {
        // setup exactly like the test at "sending a message starts the sidecar then sends" (line ~285)
        let attachment = Attachment { path: "/x/a b.png".into(), kind: AttachmentKind::Image };
        handle.send_with("look".to_string(), vec![attachment.clone()]);
        // wait for the view the way that test does
        assert_eq!(
            control.calls(),
            vec![Call::Start(start_params(None)), Call::SendAttached("look".to_string(), vec![attachment.clone()])]
        );
        assert_eq!(seen.session.messages[0].attachments, vec![attachment]);
    }

    #[tokio::test]
    async fn added_directories_go_to_the_first_start() {
        handle.add_directories(vec!["/extra".into()]);
        handle.send("hi".to_string());
        let mut expected = start_params(None);
        expected.additional_directories = vec!["/extra".to_string()];
        assert_eq!(control.calls(), vec![Call::Start(expected), Call::Send("hi".to_string())]);
    }
```

In `event.rs` tests add:

```rust
    #[test]
    fn push_user_message_keeps_the_attachments() {
        let mut session = empty_session();
        let attachment = forge_domain::Attachment { path: "/x/b.txt".into(), kind: forge_domain::AttachmentKind::File };
        push_user_message(&mut session, "hi".to_string(), vec![attachment.clone()]);
        assert_eq!(session.messages[0].attachments, vec![attachment]);
    }
```

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p forge-application`
Expected: compile errors.

- [ ] **Step 3: Implement**

`event.rs`: `pub fn push_user_message(session: &mut Session, text: String, attachments: Vec<Attachment>)` stores them in the new `Message`. Update its existing call sites and tests (`push_user_message(&mut session, "hello".to_string(), Vec::new())`).

`SendMessageUseCase::execute(&self, session, text, attachments: Vec<Attachment>)`: `push_user_message(session, text.clone(), attachments.clone()); self.sidecar.send_message(text, attachments).await`. Update its two tests (pass `Vec::new()`).

`live_session.rs`: `LiveCommand::Send { text: String, attachments: Vec<Attachment> }`, `LiveCommand::AddDirectories(Vec<PathBuf>)`:

```rust
    pub fn send(&self, text: String) {
        self.send_with(text, Vec::new());
    }

    /// Sends a message with files and images attached.
    pub fn send_with(&self, text: String, attachments: Vec<Attachment>) {
        let _ = self.commands.send(LiveCommand::Send { text, attachments });
    }

    /// Folders the session may use besides its own. Only a session whose sidecar has not started
    /// takes them (the SDK reads them at start); later calls are ignored.
    pub fn add_directories(&self, directories: Vec<PathBuf>) {
        let _ = self.commands.send(LiveCommand::AddDirectories(directories));
    }
```

`live_session_actor.rs`: add the field `additional_directories: Vec<String>` (initialised empty in `new`); in `handle_command`: `LiveCommand::Send { text, attachments } => self.send(text, attachments).await,` and

```rust
            LiveCommand::AddDirectories(directories) => {
                if self.sidecar.is_none() {
                    for directory in directories {
                        let directory = directory.to_string_lossy().to_string();
                        if !self.additional_directories.contains(&directory) {
                            self.additional_directories.push(directory);
                        }
                    }
                }
            }
```

`send(&mut self, text: String, attachments: Vec<Attachment>)` passes `attachments.clone()` into `execute` and keeps using `text` for `keep_unsent`. `start_sidecar` fills `additional_directories: self.additional_directories.clone(),`.

- [ ] **Step 4: Run**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: a live session sends attachments and starts with extra folders

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: File dialog port and adapter

**Files:**
- Create: `crates/forge-ports/src/file_picker_port.rs`
- Modify: `crates/forge-ports/src/lib.rs`, `crates/forge-adapter-dialog/src/lib.rs`, `crates/forge-features/src/workspace.rs:25-32` (`WorkspaceServices`), `crates/forge-app/src/main.rs:117-118`, plus every place a `WorkspaceServices { … }` literal is built in tests (grep `WorkspaceServices {`)

**Interfaces:**
- Produces: `FilePickerPort::pick_files(&self) -> Vec<PathBuf>` (empty means cancelled), `WorkspaceServices.file_picker: Arc<dyn FilePickerPort>`, and `Workspace::pick_files(&self) -> Vec<PathBuf>` is NOT added here (Task 8 calls the service from `ComposerState`).

- [ ] **Step 1: Write the failing test** (in the new port file)

```rust
use std::path::PathBuf;

use async_trait::async_trait;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

/// Asks the person to choose files. An empty list means they cancelled.
#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait FilePickerPort: Send + Sync {
    async fn pick_files(&self) -> Vec<PathBuf>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_file_picker_returns_the_chosen_files_or_nothing() {
        let mut mock = MockFilePickerPort::new();
        let mut answers = vec![Vec::new(), vec![PathBuf::from("/a.png"), PathBuf::from("/b.txt")]];
        mock.expect_pick_files().times(2).returning(move || answers.pop().unwrap());

        assert_eq!(mock.pick_files().await.len(), 2);
        assert!(mock.pick_files().await.is_empty());
    }
}
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p forge-ports file_picker`
Expected: not declared.

- [ ] **Step 3: Implement**

`lib.rs`: `mod file_picker_port;`, `pub use file_picker_port::FilePickerPort;` and under the mocks cfg `pub use file_picker_port::MockFilePickerPort;`. Adapter:

```rust
#[async_trait]
impl FilePickerPort for RfdFolderPicker {
    async fn pick_files(&self) -> Vec<PathBuf> {
        rfd::AsyncFileDialog::new()
            .set_title("Add file or photo")
            .pick_files()
            .await
            .map(|handles| handles.into_iter().map(|handle| handle.path().to_path_buf()).collect())
            .unwrap_or_default()
    }
}
```

(import `FilePickerPort`; add one test that `Arc<dyn FilePickerPort>` can be made from `RfdFolderPicker`, like the existing one.) `WorkspaceServices`: `pub file_picker: Arc<dyn FilePickerPort>,`. `main.rs`: `let picker = Arc::new(RfdFolderPicker::new()); WorkspaceServices { …, folder_picker: picker.clone(), file_picker: picker }`. Fix test literals of `WorkspaceServices` (use `Arc::new(MockFilePickerPort::new())`).

- [ ] **Step 4: Run**

Run: `cargo test --workspace && cargo build -p forge-app`
Expected: PASS and the app builds.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: a file dialog port beside the folder dialog

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Attachment logic (pure, in `forge-features`)

**Files:**
- Create: `crates/forge-features/src/attachments.rs`
- Modify: `crates/forge-features/src/lib.rs`

**Interfaces:**
- Produces:
  - `kind_for_path(path: &Path) -> AttachmentKind`
  - `add_attachments(list: &mut Vec<Attachment>, picked: Vec<PathBuf>)` (no duplicates, keeps order)
  - `check_attachments(list: &[Attachment], size_of: impl Fn(&Path) -> Option<u64>) -> Result<(), String>`
  - `MAX_IMAGE_BYTES: u64 = 5 * 1024 * 1024`
  - `with_folder_hints(text: &str, folders: &[PathBuf]) -> String`

- [ ] **Step 1: Write the failing tests** (bottom of the new file)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn att(path: &str) -> Attachment {
        Attachment { path: path.into(), kind: kind_for_path(Path::new(path)) }
    }

    #[test]
    fn the_extension_decides_the_kind_whatever_its_case() {
        assert_eq!(kind_for_path(Path::new("/a/b.PNG")), AttachmentKind::Image);
        assert_eq!(kind_for_path(Path::new("/a/b.jpeg")), AttachmentKind::Image);
        assert_eq!(kind_for_path(Path::new("/a/b.webp")), AttachmentKind::Image);
        assert_eq!(kind_for_path(Path::new("/a/b.svg")), AttachmentKind::File);
        assert_eq!(kind_for_path(Path::new("/a/README")), AttachmentKind::File);
    }

    #[test]
    fn the_same_file_twice_is_one_chip_and_order_is_kept() {
        let mut list = Vec::new();
        add_attachments(&mut list, vec!["/a/1.png".into(), "/a/2.txt".into()]);
        add_attachments(&mut list, vec!["/a/1.png".into(), "/a/3.gif".into()]);
        let names: Vec<String> = list.iter().map(Attachment::file_name).collect();
        assert_eq!(names, ["1.png", "2.txt", "3.gif"]);
    }

    #[test]
    fn an_image_over_the_limit_is_refused_by_name() {
        let list = vec![att("/a/big photo.png")];
        let error = check_attachments(&list, |_| Some(MAX_IMAGE_BYTES + 1)).unwrap_err();
        assert!(error.contains("big photo.png"), "{error}");
        assert!(error.contains("5 MB"), "{error}");
    }

    #[test]
    fn an_image_at_the_limit_and_a_huge_text_file_are_fine() {
        let list = vec![att("/a/ok.png"), att("/a/huge.log")];
        assert!(check_attachments(&list, |path| {
            Some(if path.ends_with("ok.png") { MAX_IMAGE_BYTES } else { u64::MAX })
        })
        .is_ok());
    }

    #[test]
    fn a_file_that_is_gone_is_refused_by_name() {
        let list = vec![att("/a/gone.txt")];
        let error = check_attachments(&list, |_| None).unwrap_err();
        assert!(error.contains("gone.txt"), "{error}");
    }

    #[test]
    fn folders_become_at_lines_after_the_text_and_a_path_with_spaces_is_kept() {
        let text = with_folder_hints("fix it", &[PathBuf::from("/내 문서/proj a")]);
        assert_eq!(text, "fix it\n@/내 문서/proj a");
        assert_eq!(with_folder_hints("", &[PathBuf::from("/x")]), "@/x");
        assert_eq!(with_folder_hints("same", &[]), "same");
    }
}
```

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p forge-features attachments`
Expected: FAIL (module missing).

- [ ] **Step 3: Implement**

```rust
use std::path::{Path, PathBuf};

use forge_domain::{Attachment, AttachmentKind};

/// The largest image Forge sends. The API refuses much larger ones, and the whole file is read.
pub const MAX_IMAGE_BYTES: u64 = 5 * 1024 * 1024;

const IMAGE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "gif", "webp"];

pub fn kind_for_path(path: &Path) -> AttachmentKind {
    let image = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| IMAGE_EXTENSIONS.contains(&extension.to_lowercase().as_str()));
    if image {
        AttachmentKind::Image
    } else {
        AttachmentKind::File
    }
}

/// Adds the picked files as chips, skipping one that is already there.
pub fn add_attachments(list: &mut Vec<Attachment>, picked: Vec<PathBuf>) {
    for path in picked {
        if list.iter().any(|existing| existing.path == path) {
            continue;
        }
        let kind = kind_for_path(&path);
        list.push(Attachment { path, kind });
    }
}

/// Looks at every chip before a send. `size_of` is the file's size, `None` when it cannot be
/// read. Only images have a size limit: a text file is read by the model's own tool.
pub fn check_attachments(list: &[Attachment], size_of: impl Fn(&Path) -> Option<u64>) -> Result<(), String> {
    for attachment in list {
        let Some(size) = size_of(&attachment.path) else {
            return Err(format!("{} cannot be read.", attachment.file_name()));
        };
        if attachment.kind == AttachmentKind::Image && size > MAX_IMAGE_BYTES {
            return Err(format!("{} is larger than 5 MB.", attachment.file_name()));
        }
    }
    Ok(())
}

/// The text with a line `@folder` for each folder, so the model knows it may use them.
pub fn with_folder_hints(text: &str, folders: &[PathBuf]) -> String {
    let mut lines: Vec<String> = Vec::new();
    if !text.is_empty() {
        lines.push(text.to_string());
    }
    lines.extend(folders.iter().map(|folder| format!("@{}", folder.display())));
    lines.join("\n")
}
```

`lib.rs`: `mod attachments;` and `pub use attachments::{add_attachments, check_attachments, kind_for_path, with_folder_hints, MAX_IMAGE_BYTES};`.

- [ ] **Step 4: Run**

Run: `cargo test -p forge-features attachments`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: attachment logic: kind, dedupe, check, folder hints

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Composer state and sending with chips

**Files:**
- Create: `crates/forge-features/src/composer_state.rs`
- Modify: `crates/forge-features/src/send_message.rs`, `crates/forge-features/src/command_box.rs`, `crates/forge-features/src/lib.rs`

**Interfaces:**
- Consumes: Task 7's functions, `WorkspaceServices.file_picker`/`folder_picker`.
- Produces: `ComposerState` (`Copy`): `draft: Signal<String>`, and methods `attachments() -> Vec<Attachment>`, `folders() -> Vec<PathBuf>`, `error() -> Option<String>`, `remove_attachment(index)`, `remove_folder(index)`, `add_files()`, `add_folder()`, `insert_slash()`, `popup_token() -> u32`, `can_send(&self) -> bool`, `take_for_send(&self, text: &str) -> Option<Outgoing>`; `use_composer_state() -> ComposerState`; `Outgoing { text: String, attachments: Vec<Attachment> }`. `CommandTextArea` gets the prop `#[props(default)] open_token: u32`.

- [ ] **Step 1: Write the failing tests** (pure part first: `take_for_send` logic lives in a free function so it can be tested without Dioxus)

In `composer_state.rs`:

```rust
/// What a send takes out of the composer, decided without any UI.
#[derive(Debug, Clone, PartialEq)]
pub struct Outgoing {
    pub text: String,
    pub attachments: Vec<Attachment>,
}

/// `None` when there is nothing to send (no text and no chips). Folder hints go after the text and
/// only for folders not yet announced.
pub fn plan_send(text: &str, attachments: &[Attachment], unannounced: &[PathBuf]) -> Option<Outgoing> {
    let text = text.trim();
    if text.is_empty() && attachments.is_empty() {
        return None;
    }
    Some(Outgoing { text: with_folder_hints(text, unannounced), attachments: attachments.to_vec() })
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
        assert_eq!(plan_send("   ", &[], &[]), None);
    }

    #[test]
    fn chips_alone_can_be_sent() {
        let out = plan_send("", &[image()], &[]).unwrap();
        assert_eq!(out.text, "");
        assert_eq!(out.attachments, vec![image()]);
    }

    #[test]
    fn text_is_trimmed_and_unannounced_folders_follow_it() {
        let out = plan_send("  fix  ", &[], &[PathBuf::from("/extra dir")]).unwrap();
        assert_eq!(out.text, "fix\n@/extra dir");
    }

    #[test]
    fn a_folder_alone_does_not_make_a_message() {
        assert_eq!(plan_send("", &[], &[PathBuf::from("/extra")]), None);
    }
}
```

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p forge-features composer_state`
Expected: FAIL (module missing).

- [ ] **Step 3: Implement**

`composer_state.rs` (above the tests):

```rust
use std::path::PathBuf;

use dioxus::prelude::*;
use forge_domain::Attachment;

use crate::attachments::{add_attachments, check_attachments, with_folder_hints};
use crate::workspace::WorkspaceServices;

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
    services: Signal<WorkspaceServices>,
}

pub fn use_composer_state_provider(draft: Signal<String>) -> ComposerState {
    // Read here, in the component: an event handler cannot call `use_context`.
    let services = use_context::<WorkspaceServices>();
    let state = use_hook(move || ComposerState {
        draft,
        attachments: Signal::new(Vec::new()),
        folders: Signal::new(Vec::new()),
        error: Signal::new(None),
        popup_token: Signal::new(0),
        picking: Signal::new(false),
        services: Signal::new(services),
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
        dioxus::core::spawn_forever(async move {
            let picked = services.file_picker.pick_files().await;
            picking.set(false);
            add_attachments(&mut attachments.write(), picked);
        });
    }

    /// Opens the folder dialog and adds the chosen folder as a chip (once).
    pub fn add_folder(&self) {
        if *self.picking.peek() {
            return;
        }
        let services = (*self.services.peek()).clone();
        let mut picking = self.picking;
        let mut folders = self.folders;
        picking.set(true);
        dioxus::core::spawn_forever(async move {
            let picked = services.folder_picker.pick_folder().await;
            picking.set(false);
            let Some(path) = picked else { return };
            if !folders.peek().iter().any(|(existing, _)| *existing == path) {
                folders.write().push((path, false));
            }
        });
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
        token.set(token.peek().wrapping_add(1));
    }

    /// Takes what a send needs. Checks the chips first: a refusal leaves everything where it is
    /// and sets the error. On success the chips are cleared and the folders marked as told.
    /// Call it only at the moment of sending, so a failed send has lost nothing.
    pub fn take_for_send(&self, text: &str) -> Option<Outgoing> {
        let attachments = self.attachments();
        let unannounced: Vec<PathBuf> =
            self.folders.peek().iter().filter(|(_, told)| !told).map(|(path, _)| path.clone()).collect();
        let outgoing = plan_send(text, &attachments, &unannounced)?;
        let size_of = |path: &std::path::Path| std::fs::metadata(path).ok().filter(|m| m.is_file()).map(|m| m.len());
        if let Err(message) = check_attachments(&attachments, size_of) {
            let mut error = self.error;
            error.set(Some(message));
            return None;
        }
        let mut error = self.error;
        error.set(None);
        let mut list = self.attachments;
        list.write().clear();
        for (_, told) in self.folders.write().iter_mut() {
            *told = true;
        }
        Some(outgoing)
    }
}
```

`lib.rs`: `mod composer_state;` and `pub use composer_state::{use_composer_state, use_composer_state_provider, ComposerState, Outgoing};` and `pub use attachments::…` from Task 7.

Careful with `take_for_send`: `plan_send` can return `None` for "nothing to send" before the check, which is right; a refusal returns `None` too but has set the error.

`command_box.rs`: add the prop `#[props(default)] open_token: u32,` and, after the existing `use_effect`:

```rust
    // The `+` menu's "Slash commands" row put `/` in the text and asks for the list: treat the text
    // as typed, once per token. The first token is the starting value and opens nothing.
    let mut seen_token = use_signal(|| open_token);
    use_effect(use_reactive!(|open_token, value| {
        if open_token != *seen_token.peek() {
            seen_token.set(open_token);
            typed.set(Some(value.clone()));
            dismissed.set(false);
        }
    }));
```

`send_message.rs`: replace the local draft and `submit`:

```rust
    let draft = use_signal(String::new);
    let composer = use_composer_state_provider(draft);
    let mut draft = draft;
```

(keep the restore effect as it is, it uses `draft`), and

```rust
    let mut submit = move || {
        let Some(outgoing) = composer.take_for_send(&draft.read()) else {
            return;
        };
        if let Some(handle) = workspace.handle(id) {
            handle.send_with(outgoing.text, outgoing.attachments);
            draft.set(String::new());
        }
    };
```

Render, above `CommandTextArea`, the chips and the error:

```rust
            if let Some(message) = composer.error() {
                div { class: "banner error", onclick: move |_| composer.dismiss_error(), "{message}" }
            }
            ComposerChips {}
```

`ComposerChips` is a small component in the same file:

```rust
/// The attached files and added folders as removable chips above the text.
#[component]
fn ComposerChips() -> Element {
    let composer = use_composer_state();
    let attachments = composer.attachments();
    let folders = composer.folders();
    if attachments.is_empty() && folders.is_empty() {
        return rsx! {};
    }
    rsx! {
        div { class: "chips",
            for (index, attachment) in attachments.iter().enumerate() {
                span { key: "a{index}", class: "chip", title: "{attachment.path.display()}",
                    span { class: "chip-kind", if attachment.kind == AttachmentKind::Image { "🖼" } else { "📄" } }
                    span { class: "chip-name", "{attachment.file_name()}" }
                    button { class: "chip-remove", r#type: "button", "aria-label": "Remove", onclick: move |_| composer.remove_attachment(index), "×" }
                }
            }
            for (index, folder) in folders.iter().enumerate() {
                span { key: "f{index}", class: "chip", title: "{folder.display()}",
                    span { class: "chip-kind", "📁" }
                    span { class: "chip-name", "{folder.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()}" }
                    button { class: "chip-remove", r#type: "button", "aria-label": "Remove", onclick: move |_| composer.remove_folder(index), "×" }
                }
            }
        }
    }
}
```

Pass `open_token: composer.popup_token()` to `CommandTextArea`. Add ⌘U: wrap the `div { class: "composer", … }` with `onkeydown` that calls `composer.add_files()` when `event.code() == Code::KeyU && event.modifiers().meta()` (physical key) and `event.prevent_default()`. Also enable Send when chips exist: `can_send` stays as is; the Send button's `disabled` uses `!can_send` only, since `take_for_send` returns `None` on empty.

- [ ] **Step 4: Run**

Run: `cargo test -p forge-features && cargo build -p forge-app`
Expected: tests PASS, app builds.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: composer state, chips, and sending with attachments

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 9: The `+` menu

**Files:**
- Create: `crates/forge-widgets/src/plus_menu.rs`
- Modify: `crates/forge-widgets/src/composer_controls.rs` (reuse its menu look, see Step 3), `crates/forge-widgets/src/lib.rs`, `crates/forge-widgets/src/chat_window.rs:56-60`, `crates/forge-features/src/settings.rs`, `crates/forge-shared/src/theme.css`

**Interfaces:**
- Consumes: `use_composer_state()` (Task 8), `use_settings()`.
- Produces: `forge_widgets::PlusMenu { project: PathBuf, add_folder_enabled: bool }`; `Settings::connector_rows(&self) -> Result<Vec<ExtensionRow>, String>` (alias of `mcp_tab`, named for what the menu calls them) and `Settings::prepare_menu(&self, project: PathBuf)` (sets the project and reloads the catalog once).

- [ ] **Step 1: Write the failing test** (pure row table in `plus_menu.rs`, tested like `mode_rows`)

```rust
/// What a row of the `+` menu does when picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlusAction {
    AddFiles,
    AddFolder,
    SlashCommands,
    Connectors,
    Plugins,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlusRow {
    pub label: &'static str,
    pub action: PlusAction,
    pub shortcut: Option<&'static str>,
    pub submenu: bool,
}

pub fn plus_rows() -> [PlusRow; 5] {
    [
        PlusRow { label: "Add file or photo", action: PlusAction::AddFiles, shortcut: Some("⌘U"), submenu: false },
        PlusRow { label: "Add folder", action: PlusAction::AddFolder, shortcut: None, submenu: false },
        PlusRow { label: "Slash commands", action: PlusAction::SlashCommands, shortcut: None, submenu: false },
        PlusRow { label: "Connectors", action: PlusAction::Connectors, shortcut: None, submenu: true },
        PlusRow { label: "Plugins", action: PlusAction::Plugins, shortcut: None, submenu: true },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_are_the_five_the_desktop_app_shows_in_its_order() {
        let labels: Vec<&str> = plus_rows().iter().map(|row| row.label).collect();
        assert_eq!(labels, ["Add file or photo", "Add folder", "Slash commands", "Connectors", "Plugins"]);
    }

    #[test]
    fn only_connectors_and_plugins_open_a_submenu() {
        let with_submenu: Vec<PlusAction> = plus_rows().iter().filter(|r| r.submenu).map(|r| r.action).collect();
        assert_eq!(with_submenu, [PlusAction::Connectors, PlusAction::Plugins]);
    }
}
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p forge-widgets plus_menu`
Expected: FAIL (module missing).

- [ ] **Step 3: Implement the component**

`PlusMenu` follows `Picker`'s structure (read `composer_controls.rs:23-110` first and reuse its classes `picker-backdrop`, `picker-menu`, `picker-item`, `menu-key`, and its Escape and focus-on-mount handling): a `+` button (`picker-button`, aria-label "Add"), an `open` signal, a `submenu: Signal<Option<PlusAction>>`. Rows come from `plus_rows()`; a row click does:

```rust
match row.action {
    PlusAction::AddFiles => { open.set(false); composer.add_files(); }
    PlusAction::AddFolder => { open.set(false); composer.add_folder(); }
    PlusAction::SlashCommands => { open.set(false); composer.insert_slash(); refocus_composer(); }
    PlusAction::Connectors | PlusAction::Plugins => submenu.set(Some(row.action)),
}
```

Number keys 1 to 5 pick rows through `menu_digit_row(&event.code().to_string())` (physical key), same guard against modifiers as `Picker`. Escape: if a submenu is open close it and `stop_propagation`, else close the menu and `refocus_composer()`. The "Add folder" row is disabled when `add_folder_enabled` is false (class `picker-item disabled`, `disabled: true`), with `title` "Choose folders before the first message" (used only if Task 1 step 4 said so; otherwise the caller always passes `true`).

The submenu (a `div { class: "picker-menu picker-submenu" }` positioned to the right of the menu) lists the rows from `settings.connector_rows()` (Connectors) or `settings.plugin_tab()` (Plugins) as `picker-item` with a check on `enabled`; a click calls `settings.toggle(ExtensionKind::McpServer | ExtensionKind::Plugin, row.key.clone())` and leaves the menu open so the check updates. An `Err(message)` shows the message as one disabled line; an empty list shows "None installed". The last line of the submenu is the footer `div { class: "picker-title", "Applies to the next new session." }`.

When the menu opens call `settings.prepare_menu(project.clone())`. In `settings.rs` add:

```rust
    /// The MCP servers, as the `+` menu lists them: the same rows as the Settings tab.
    pub fn connector_rows(&self) -> Result<Vec<ExtensionRow>, String> {
        self.mcp_tab()
    }

    /// Points the lists at `project` and reads them again, so the `+` menu shows what Settings would.
    pub fn prepare_menu(&self, project: PathBuf) {
        self.set_project(Some(project));
        self.reload_catalog();
    }
```

Mount it in `chat_window.rs` as the first child of `SendMessageBox`: `PlusMenu { project: view.project_path.clone(), add_folder_enabled: true }`. Export from `lib.rs`: `pub use plus_menu::PlusMenu;`.

CSS to append to `theme.css`:

```css
/* Composer + menu, chips */
.picker-submenu { left: calc(100% + 4px); bottom: 0; min-width: 220px; }
.picker-item.disabled { opacity: 0.5; cursor: default; }
.picker-item .menu-key { margin-left: auto; color: var(--fg-muted); font-size: 11px; }
.chips { display: flex; flex-wrap: wrap; gap: 6px; }
.chip { display: inline-flex; align-items: center; gap: 4px; max-width: 220px; padding: 2px 4px 2px 8px; border: 1px solid var(--border); border-radius: 999px; background: var(--bg-hover); font-size: 12px; }
.chip-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.chip-remove { border: none; background: transparent; color: var(--fg-muted); cursor: pointer; font: inherit; line-height: 1; }
.chip-remove:hover { color: var(--fg); }
.env-chips { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
```

- [ ] **Step 4: Run, then look at it**

Run: `cargo test --workspace && cargo build -p forge-app`
Expected: PASS. Do not claim the menu works yet; Task 13 drives the real app.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: the composer + menu with connector and plugin submenus

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 10: Attachments on the sent message

**Files:**
- Modify: `crates/forge-entities/src/message.rs`

**Interfaces:**
- Consumes: `Message.attachments` (Task 2).

- [ ] **Step 1: Write the failing test** (there is a `#[cfg(test)]` module in `forge-entities`; if `message.rs` has no render test, test the pure label function below)

```rust
/// The text of a chip on a sent message: an icon and the file name.
pub fn attachment_label(attachment: &forge_domain::Attachment) -> String {
    let icon = match attachment.kind {
        forge_domain::AttachmentKind::Image => "🖼",
        forge_domain::AttachmentKind::File => "📄",
    };
    format!("{icon} {}", attachment.file_name())
}

#[cfg(test)]
mod attachment_tests {
    use super::*;
    use forge_domain::{Attachment, AttachmentKind};

    #[test]
    fn a_chip_shows_an_icon_and_the_file_name_only() {
        let image = Attachment { path: "/내 문서/사진 1.png".into(), kind: AttachmentKind::Image };
        let file = Attachment { path: "/x/notes.txt".into(), kind: AttachmentKind::File };
        assert_eq!(attachment_label(&image), "🖼 사진 1.png");
        assert_eq!(attachment_label(&file), "📄 notes.txt");
    }
}
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p forge-entities attachment`
Expected: FAIL until Step 3 is in.

- [ ] **Step 3: Implement**

In `MessageBubble`, after the `content` block add:

```rust
            if !message.attachments.is_empty() {
                div { class: "chips",
                    for attachment in message.attachments.iter() {
                        span { class: "chip", title: "{attachment.path.display()}", "{attachment_label(attachment)}" }
                    }
                }
            }
```

- [ ] **Step 4: Run**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: show attachments on a sent message

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 11: New-session screen: environment chips, attachments and folders

**Files:**
- Modify: `crates/forge-features/src/new_session.rs`, `crates/forge-features/src/workspace.rs` (`start_new`, ~line 503)

**Interfaces:**
- Consumes: `ComposerState` (Task 8), `LiveSessionHandle::add_directories` and `send_with` (Task 5).
- Produces: `Workspace::start_new(&self, project, mode, text, attachments: Vec<Attachment>, folders: Vec<PathBuf>)`.

- [ ] **Step 1: Write the failing test**

`start_new` needs the UI runtime; test the part that can be tested without it: that the registry's handle receives the folders before the message. Add to `live_session.rs` tests (it already tests `add_directories` then `send` in Task 5), so the ordering is pinned there. For this task add a pure test for the chip label of the folder in `forge-features/src/attachments.rs`:

```rust
/// The name a folder chip shows: the last component, or the whole path for `/`.
pub fn folder_label(folder: &Path) -> String {
    folder.file_name().map_or_else(|| folder.display().to_string(), |name| name.to_string_lossy().to_string())
}
```

```rust
    #[test]
    fn a_folder_chip_shows_the_last_component_and_root_shows_in_full() {
        assert_eq!(folder_label(Path::new("/내 문서/proj a")), "proj a");
        assert_eq!(folder_label(Path::new("/")), "/");
    }
```

Export it from `lib.rs` and use it in `ComposerChips` (Task 8) in place of the inline `file_name()` expression.

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p forge-features folder_label`
Expected: FAIL until the function exists.

- [ ] **Step 3: Implement**

`start_new`: after `this.attach(handle.clone())` and before sending:

```rust
                    if !folders.is_empty() {
                        handle.add_directories(folders);
                    }
                    handle.send_with(text, attachments);
```

`NewSessionComposer`: replace the Folder/Model/Mode fields' layout above the composer with an environment row, keeping Model and Mode as they are:

```rust
            div { class: "env-chips",
                span { class: "chip", "Local" }
                span { class: "chip",
                    span { class: "chip-name", "{folder_name}" }
                }
                button {
                    class: "picker-button",
                    r#type: "button",
                    disabled: workspace.picking_folder(),
                    onclick: move |_| workspace.open_folder(),
                    "Change folder…"
                }
            }
```

where `folder_name` is `folder_label` of the chosen folder (empty string when none). Keep the existing `Select` of known folders as it is (it is how a known project is picked); the chips show the choice and the new button opens the dialog. This is the smallest change that matches the desktop screen without removing a working control. The composer area gets the state and the `+` menu:

```rust
    let draft = use_signal(String::new);   // replaces `text`
    let composer = use_composer_state_provider(draft);
```

In `submit`: `let Some(outgoing) = composer.take_for_send(&draft.read()) else { return };` then `workspace.start_new(project, parse_mode(&mode.read()), outgoing.text, outgoing.attachments, composer.take_folders());` — add to `ComposerState`:

```rust
    /// The folders for a new session: all of them, since none has been announced to a model yet.
    pub fn take_folders(&self) -> Vec<PathBuf> {
        let mut list = self.folders;
        let folders = list.peek().iter().map(|(path, _)| path.clone()).collect();
        list.write().clear();
        folders
    }
```

and clear the draft. Because `take_for_send` adds `@folder` hints for unannounced folders, the new session must NOT use that hint (it passes folders through `add_directories`): in `new_session.rs` call a variant `composer.take_for_new_session(&draft.read())` that does the same but uses an empty `unannounced` list:

```rust
    pub fn take_for_new_session(&self, text: &str) -> Option<Outgoing> {
        let attachments = self.attachments();
        let outgoing = plan_send(text, &attachments, &[])?;
        // same check and clearing as `take_for_send`, factored into `check_and_clear`
        self.check_and_clear(&attachments)?;
        Some(outgoing)
    }
```

Move the shared "check, set error, clear chips" body of `take_for_send` into `fn check_and_clear(&self, attachments: &[Attachment]) -> Option<()>` and have both call it (and `take_for_send` additionally marks folders as told). Render `ComposerChips` (make it `pub` and export it) and the error banner above the `CommandTextArea`, pass `open_token`, and put `forge_widgets`' `PlusMenu` in the composer row: `NewSessionComposer` is in `forge-features` and cannot name a widget, so move the composer row of the new-session screen into `NewSessionPanel` in `forge-widgets/src/new_session_panel.rs` by passing the menu as `children` of `NewSessionComposer` (`NewSessionComposer { PlusMenu { project: …, add_folder_enabled: true } }`), rendered first in its `composer-row`. `PlusMenu` needs a project for `prepare_menu`; on this screen use the chosen folder (`workspace.chosen_new_folder()`), and make the `project` prop `Option<PathBuf>` (with `None`, skip `prepare_menu`).

- [ ] **Step 4: Run**

Run: `cargo test --workspace && cargo build -p forge-app`
Expected: PASS and builds.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: environment chips, attachments and folders on the new-session screen

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 12: Whole-workspace check

**Files:** none new.

- [ ] **Step 1: Everything**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && (cd sidecar && npm run typecheck && npx vitest run && npm run build)`
Expected: all PASS, no warnings. Fix what appears (do not silence a lint without a reason).

- [ ] **Step 2: Commit any fixes**

```bash
git add -A && git commit -m "fix: lint and type findings from the whole-workspace check

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

(Skip the commit if nothing changed.)

---

### Task 13: Check it in the real SDK and the real app, then record it

Tests prove the pieces; this proves the feature. Use the harness (`scripts/harness/README.md`, memory "Driving the Forge app visually": screencapture, Quartz clicks, `FORGE_CLAUDE_HOME` fixture). Use a throwaway folder; a message sent here uses the real account, so send only the two short messages below.

**Files:**
- Modify: `docs/superpowers/specs/2026-10-03-composer-plus-menu-design.md` (add "As built"), `docs/superpowers/roadmap/backlog.md` (CMP-01, CMP-06 rows), `docs/superpowers/roadmap/README.md` (Plan 18 row)

- [ ] **Step 1: Launch with the fixture** (`scripts/harness/launch.sh`), open a session in the throwaway folder.

- [ ] **Step 2: Menu.** Click `+`. Expected: five rows in the order of the spec, ⌘U on the first, no GitHub row. Press `5` then Escape (Connectors submenu opens, Escape closes it, a second Escape closes the menu). Press `3` ("Slash commands"): `/` appears and the command list opens. Capture a screenshot after each click (layout moves).

- [ ] **Step 3: Attach.** Press ⌘U (or the row), choose one small png and one txt. Expected: two chips; the same file chosen again adds none; × removes a chip. Send with the text "Describe the image and tell me the first word of the text file." Expected: chips clear; the sent message shows two chips; Claude describes the image and quotes the file.

- [ ] **Step 4: Too big.** Attach a png over 5 MB. Expected: a banner naming the file, text and chips stay.

- [ ] **Step 5: Folder.** Add a folder outside the project, then send "List the files in the folder I added." Expected: an `@folder` line is sent once; a permission card appears and the answer lists the files. A second message does not repeat the line.

- [ ] **Step 6: New-session screen.** Open it. Expected: "Local", the folder name and "Change folder…" above the composer; `+` in the composer row; a folder added here goes to the first start (the sidebar session works in both folders).

- [ ] **Step 7: Write down what was and was not seen**

Add an "As built" section to the spec: what was built, the two deviations (top of this plan), the Task 1 outcome, and a list of what Step 2 to 6 did not check (for example: ⌘U with the Korean input method, the Connectors toggle's effect on the next new session). In `backlog.md` set CMP-01 to `Plan 18 (built: file or photo, folder, slash, connectors, plugins; not built: GitHub issue)` and CMP-06 to `Plan 18 (built: chips; the "로컬" menu was not seen)`, and in `README.md` mark the Plan 18 row `(built)`.

- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "docs: record Plan 18 as built, and what was not seen in the real app

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```
