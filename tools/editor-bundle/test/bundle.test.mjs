// Runs the built bundle (../../crates/forge-shared/assets/editor.js) in jsdom and checks the messages
// it sends to Rust. Build first: `npm run build`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { JSDOM } from "jsdom";

const code = readFileSync(new URL("../../../crates/forge-shared/assets/editor.js", import.meta.url), "utf8");
// Objects made inside jsdom have another Object prototype, so compare plain copies.
const plain = (value) => JSON.parse(JSON.stringify(value));
const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

function boot() {
  const dom = new JSDOM(`<!doctype html><body><div id="host"></div></body>`, { runScripts: "outside-only", pretendToBeVisual: true });
  const w = dom.window;
  // CodeMirror measures layout; jsdom has none, so give it the few calls it makes.
  w.Range.prototype.getClientRects = () => ({ length: 0, item: () => null, [Symbol.iterator]: function* () {} });
  w.Range.prototype.getBoundingClientRect = () => ({ top: 0, left: 0, right: 0, bottom: 0, width: 0, height: 0 });
  w.document.elementFromPoint = () => null;
  w.eval(code);
  const sent = [];
  const editor = w.ForgeEditor.create(w.document.getElementById("host"), (message) => sent.push(message));
  return { w, editor, sent };
}

test("flush answers with the text of the current tab, of another tab, and null for an unknown one", () => {
  const { editor, sent } = boot();
  editor.load("a.py", "x = 1\n", "python", false);
  editor.load("b.py", "y = 2\n", "python", true);
  editor.show("a.py");
  editor.flush("a.py");
  editor.flush("b.py");
  editor.flush("nope.py");
  assert.deepEqual(plain(sent), [
    { t: "flush", key: "a.py", s: "x = 1\n" },
    { t: "flush", key: "b.py", s: "y = 2\n" },
    { t: "flush", key: "nope.py", s: null },
  ]);
  editor.destroy();
});

test("showing a tab the page never loaded asks Rust for it, and a dropped tab is forgotten", () => {
  const { editor, sent } = boot();
  editor.show("nope.py");
  editor.load("b.py", "y = 2\n", "python", false);
  editor.drop("b.py");
  editor.flush("b.py");
  assert.deepEqual(plain(sent), [
    { t: "missing", key: "nope.py" },
    { t: "flush", key: "b.py", s: null },
  ]);
  editor.destroy();
});

test("the first edit says dirty at once and the text follows about two seconds later, once", async () => {
  const { w, editor, sent } = boot();
  editor.load("a.py", "x = 1\n", "python", false);
  const line = w.document.querySelector(".cm-line");
  line.textContent = "x = 12";
  w.document.querySelector(".cm-content").dispatchEvent(new w.Event("input", { bubbles: true }));
  await wait(300);
  assert.deepEqual(sent.map((message) => message.t), ["dirty"]);
  await wait(2300);
  assert.deepEqual(sent.map((message) => message.t), ["dirty", "change"]);
  assert.equal(sent[1].key, "a.py");
  assert.match(sent[1].s, /^x = 12/);
  editor.destroy();
});

// Types into the current tab the way a person does.
function type(w, text) {
  w.document.querySelector(".cm-line").textContent = text;
  w.document.querySelector(".cm-content").dispatchEvent(new w.Event("input", { bubbles: true }));
}

test("loading the file again while an edit waits drops that edit instead of sending it (Cancel on the conflict banner)", async () => {
  const { w, editor, sent } = boot();
  editor.load("a.py", "x = 1\n", "python", false);
  type(w, "x = 99");
  await wait(100);
  editor.load("a.py", "disk text\n", "python", false);
  await wait(2400);
  assert.deepEqual(plain(sent).map((message) => message.t), ["dirty"], "no change may be sent for the discarded edit");
  editor.flush("a.py");
  assert.equal(plain(sent).at(-1).s, "disk text\n");
  editor.destroy();
});

test("a load that must only happen while the tab is clean is skipped when an edit waits, and says so", async () => {
  const { w, editor, sent } = boot();
  editor.load("a.py", "x = 1\n", "python", false);
  type(w, "x = 99");
  await wait(100);
  editor.load("a.py", "disk text\n", "python", false, true);
  assert.deepEqual(plain(sent).map((message) => message.t), ["dirty", "skipped"]);
  assert.equal(plain(sent)[1].key, "a.py");
  editor.flush("a.py");
  assert.match(plain(sent).at(-1).s, /^x = 99/, "the person's text is still in the editor");
  editor.destroy();
});

test("a load that must only happen while the tab is clean goes ahead when nothing waits", () => {
  const { editor, sent } = boot();
  editor.load("a.py", "x = 1\n", "python", false);
  editor.load("a.py", "disk text\n", "python", false, true);
  editor.flush("a.py");
  assert.deepEqual(plain(sent), [{ t: "flush", key: "a.py", s: "disk text\n" }]);
  editor.destroy();
});

test("asking for the text does not cancel the save that is waiting for its timer", async () => {
  const { w, editor, sent } = boot();
  editor.load("a.py", "x = 1\n", "python", false);
  type(w, "x = 5");
  await wait(100);
  editor.flush("a.py");
  await wait(2300);
  assert.deepEqual(plain(sent).map((message) => message.t), ["dirty", "flush", "change"]);
  editor.destroy();
});

test("leaving the editor sends a waiting edit at once, so a click on a panel button never leaves it behind", async () => {
  const { w, editor, sent } = boot();
  editor.load("a.py", "x = 1\n", "python", false);
  type(w, "x = 7");
  await wait(100);
  w.document.querySelector(".cm-content").dispatchEvent(new w.FocusEvent("blur"));
  assert.deepEqual(plain(sent).map((message) => message.t), ["dirty", "change"]);
  assert.match(plain(sent)[1].s, /^x = 7/);
  editor.destroy();
});

const mouse = (w, type) => new w.MouseEvent(type, { bubbles: true, button: 0 });

// jsdom has no layout, so every gutter element maps to the first line; which line a number maps to is
// checked in the real window (plan 25B task 8).
test("clicking a gutter number picks a line and the bar attaches it", () => {
  const { w, editor, sent } = boot();
  editor.load("a.py", "one\ntwo\nthree\n", "python", false);
  const numbers = w.document.querySelectorAll(".cm-lineNumbers .cm-gutterElement");
  // The first gutter element is a hidden spacer: the lines start at index 1.
  numbers[1].dispatchEvent(mouse(w, "mousedown"));
  w.document.dispatchEvent(mouse(w, "mouseup"));
  assert.ok(w.document.querySelector(".cm-picked-line"), "the picked line has a band");
  const attach = w.document.querySelector(".cm-pick-bar [data-act=attach]");
  assert.ok(attach, "the bar shows over a picked line");
  attach.dispatchEvent(mouse(w, "mousedown"));
  assert.deepEqual(plain(sent.filter((m) => m.t === "attach")), [{ t: "attach", key: "a.py", from: 1, to: 1, text: "one" }]);
  editor.destroy();
});

test("a comment is sent with its lines and shows as a saved box once Rust sends it back", () => {
  const { w, editor, sent } = boot();
  editor.load("a.py", "one\ntwo\nthree\n", "python", false);
  editor.startComment(1, 2);
  const input = w.document.querySelector(".cm-comment-box.editing input");
  assert.ok(input, "an editing box opens under the last picked line");
  input.value = "why?";
  w.document.querySelector(".cm-comment-box.editing .cm-comment-ok").dispatchEvent(mouse(w, "mousedown"));
  assert.deepEqual(plain(sent.filter((m) => m.t === "comment")), [{ t: "comment", key: "a.py", from: 1, to: 2, text: "why?", id: null }]);
  editor.comments("a.py", [{ id: 7, from: 1, to: 2, text: "why?" }]);
  const saved = w.document.querySelector(".cm-comment-box.saved");
  assert.ok(saved);
  assert.equal(saved.querySelector(".cm-comment-text").textContent, "why?");
  saved.querySelector(".cm-comment-delete").dispatchEvent(mouse(w, "mousedown"));
  assert.deepEqual(plain(sent.filter((m) => m.t === "comment_delete")), [{ t: "comment_delete", key: "a.py", id: 7 }]);
  editor.destroy();
});

test("comments of another tab do not show, and come back with their tab", () => {
  const { w, editor } = boot();
  editor.load("a.py", "one\n", "python", false);
  editor.load("b.py", "two\n", "python", false);
  editor.comments("a.py", [{ id: 1, from: 1, to: 1, text: "mine" }]);
  assert.equal(w.document.querySelectorAll(".cm-comment-box").length, 0);
  editor.show("a.py");
  assert.equal(w.document.querySelectorAll(".cm-comment-box.saved").length, 1);
  editor.destroy();
});

test("focusing a comment turns its box into the editing one", () => {
  const { w, editor } = boot();
  editor.load("a.py", "one\ntwo\n", "python", false);
  editor.comments("a.py", [{ id: 3, from: 2, to: 2, text: "hello" }]);
  editor.focusComment("a.py", 3);
  assert.ok(w.document.querySelector(".cm-comment-box.editing input"));
  editor.destroy();
});

// The editor's view, found the way a probe can: CodeMirror keeps it on the content element.
function viewOf(w) {
  const content = w.document.querySelector(".cm-content");
  const holder = content[Object.keys(content).find((k) => k.startsWith("cm"))];
  return holder.view || holder.root?.view || holder.owner?.view;
}

test("a comment box and a picked line stay on their lines when lines are inserted above", () => {
  const { w, editor } = boot();
  editor.load("a.py", "l1\nl2\nl3\nl4\nl5", "python", false);
  editor.comments("a.py", [{ id: 1, from: 3, to: 3, text: "c" }]);
  editor.startComment(3, 3);
  const view = viewOf(w);
  view.dispatch({ changes: { from: 0, insert: "new1\nnew2\n" } });
  const content = w.document.querySelector(".cm-content");
  const rows = [...content.children].map((e) => (e.classList.contains("cm-comment-box") ? "[BOX]" : e.textContent));
  // The saved box follows its line number (3): it sits under the third line of the new text.
  assert.deepEqual(rows.filter((r) => r !== "[BOX]").slice(0, 3), ["new1", "new2", "l1"]);
  assert.ok(rows.indexOf("[BOX]") >= 0 && rows.indexOf("[BOX]") === rows.indexOf("l1") + 1, rows.join("|"));
  assert.ok(w.document.querySelector(".cm-picked-line"), "the pick band is still drawn");
  editor.destroy();
});

test("a focus request for a comment of a tab that is not shown yet is applied when it shows", () => {
  const { w, editor } = boot();
  editor.load("a.py", "one\ntwo\n", "python", false);
  editor.load("b.py", "x\n", "python", false);
  editor.comments("a.py", [{ id: 3, from: 2, to: 2, text: "hello" }]);
  editor.focusComment("a.py", 3);
  editor.show("a.py");
  assert.ok(w.document.querySelector(".cm-comment-box.editing input"));
  editor.destroy();
});

test("attaching a pick that lies past the end of the file sends nothing", () => {
  const { w, editor, sent } = boot();
  editor.load("a.py", "one\ntwo\nthree\nfour\n", "python", false);
  editor.startComment(4, 4);
  const view = viewOf(w);
  view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: "x" } });
  w.document.querySelector(".cm-pick-bar [data-act=attach]").dispatchEvent(mouse(w, "mousedown"));
  assert.deepEqual(plain(sent.filter((m) => m.t === "attach")), []);
  editor.destroy();
});
