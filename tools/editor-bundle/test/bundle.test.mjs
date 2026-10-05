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

const ALL = { match_case: false, whole_word: false, regex: false };
const count = (w) => w.document.querySelector(".cm-find-count")?.textContent;

test("find opens the bar on the line, marks every match and counts them", () => {
  const { w, editor } = boot();
  editor.load("a.py", "fizz\nbuzz\nBuzz\n", "python", false);
  editor.find("a.py", { query: "buzz", ...ALL, line: 3 });
  assert.ok(w.document.querySelector(".cm-find-bar"));
  assert.equal(w.document.querySelectorAll(".cm-find-match").length, 2);
  assert.equal(count(w), "2 of 2");
  editor.destroy();
});

test("next and previous wrap around", () => {
  const { w, editor } = boot();
  editor.load("a.py", "a\na\na\n", "python", false);
  editor.find("a.py", { query: "a", ...ALL, line: 1 });
  assert.equal(count(w), "1 of 3");
  w.document.querySelector(".cm-find-prev").dispatchEvent(mouse(w, "mousedown"));
  assert.equal(count(w), "3 of 3");
  w.document.querySelector(".cm-find-next").dispatchEvent(mouse(w, "mousedown"));
  assert.equal(count(w), "1 of 3");
  editor.destroy();
});

test("typing in the field and the toggles change the matches", () => {
  const { w, editor } = boot();
  editor.load("a.py", "Buzz\nbuzz\nbuzzer\n", "python", false);
  editor.find("a.py", { query: "buzz", ...ALL, line: 1 });
  assert.equal(count(w), "1 of 3");
  w.document.querySelector("[data-opt=match_case]").dispatchEvent(mouse(w, "mousedown"));
  assert.equal(count(w), "1 of 2");
  w.document.querySelector("[data-opt=whole_word]").dispatchEvent(mouse(w, "mousedown"));
  assert.equal(count(w), "1 of 1");
  const input = w.document.querySelector(".cm-find-bar input");
  input.value = "zzz";
  input.dispatchEvent(new w.Event("input", { bubbles: true }));
  assert.equal(count(w), "No results");
  editor.destroy();
});

test("an invalid regex marks the field and finds nothing", () => {
  const { w, editor } = boot();
  editor.load("a.py", "a(b\n", "python", false);
  editor.find("a.py", { query: "(", ...ALL, regex: true, line: 1 });
  assert.ok(w.document.querySelector(".cm-find-bar input.invalid"));
  assert.equal(count(w), "No results");
  editor.destroy();
});

test("a pattern that can match nothing does not flood or hang", () => {
  const { w, editor } = boot();
  editor.load("a.py", "abc\n", "python", false);
  editor.find("a.py", { query: "x*", ...ALL, regex: true, line: 1 });
  assert.equal(count(w), "No results");
  editor.destroy();
});

test("whole word works for Hangul", () => {
  const { w, editor } = boot();
  editor.load("a.py", "가나 다\n가나다\n", "python", false);
  editor.find("a.py", { query: "가나", ...ALL, whole_word: true, line: 1 });
  assert.equal(count(w), "1 of 1");
  editor.destroy();
});

test("Escape closes the bar and Enter during an IME composition does not move", () => {
  const { w, editor } = boot();
  editor.load("a.py", "a\na\n", "python", false);
  editor.find("a.py", { query: "a", ...ALL, line: 1 });
  const input = w.document.querySelector(".cm-find-bar input");
  input.dispatchEvent(new w.KeyboardEvent("keydown", { key: "Enter", isComposing: true, bubbles: true }));
  assert.equal(count(w), "1 of 2");
  input.dispatchEvent(new w.KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
  assert.equal(w.document.querySelector(".cm-find-bar"), null);
  assert.equal(w.document.querySelectorAll(".cm-find-match").length, 0);
  editor.destroy();
});

test("a find for a tab that is not shown yet is applied when it shows, and the bar follows tab switches", () => {
  const { w, editor } = boot();
  editor.load("a.py", "one\n", "python", false);
  editor.load("b.py", "two\nneedle\n", "python", false);
  editor.show("a.py");
  editor.find("b.py", { query: "needle", ...ALL, line: 2 });
  assert.equal(w.document.querySelector(".cm-find-bar"), null);
  editor.show("b.py");
  assert.equal(count(w), "1 of 1");
  editor.show("a.py");
  // The bar is the editor's, not the tab's: it stays open with the same query.
  assert.equal(w.document.querySelector(".cm-find-bar input").value, "needle");
  editor.destroy();
});

test("a plain query with a dash is a literal, not an invalid pattern", () => {
  const { w, editor } = boot();
  editor.load("a.py", "foo-bar\nx --flag\n", "python", false);
  editor.find("a.py", { query: "foo-bar", ...ALL, line: 1 });
  assert.equal(count(w), "1 of 1");
  assert.equal(w.document.querySelector(".cm-find-bar input.invalid"), null);
  editor.find("a.py", { query: "--flag", ...ALL, line: 2 });
  assert.equal(count(w), "1 of 1");
  editor.destroy();
});

test("a hit on a line is reached even when the pattern finds nothing in the document", () => {
  const { w, editor } = boot();
  editor.load("a.py", "one\ntwo\nthree\n", "python", false);
  editor.find("a.py", { query: "zzz", ...ALL, line: 3 });
  assert.equal(count(w), "No results");
  assert.equal(viewOf(w).state.doc.lineAt(viewOf(w).state.selection.main.head).number, 3);
  editor.destroy();
});

test("^ and $ match at every line, like the panel's search", () => {
  const { w, editor } = boot();
  editor.load("a.py", "fn a;\nfn b;\n", "python", false);
  editor.find("a.py", { query: "^fn", ...ALL, regex: true, line: 1 });
  assert.equal(count(w), "1 of 2");
  editor.find("a.py", { query: ";$", ...ALL, regex: true, line: 1 });
  assert.equal(count(w), "1 of 2");
  editor.destroy();
});

test("Enter that commits an IME composition (keyCode 229) does not move to the next match", () => {
  const { w, editor } = boot();
  editor.load("a.py", "a\na\n", "python", false);
  editor.find("a.py", { query: "a", ...ALL, line: 1 });
  const input = w.document.querySelector(".cm-find-bar input");
  input.dispatchEvent(new w.KeyboardEvent("keydown", { key: "Enter", keyCode: 229, bubbles: true }));
  assert.equal(count(w), "1 of 2");
  editor.destroy();
});

test("after a tab switch the current match is one of the new tab's matches", () => {
  const { w, editor } = boot();
  editor.load("a.py", "a\na\na\na\n", "python", false);
  editor.load("b.py", "a\n", "python", false);
  editor.show("a.py");
  editor.find("a.py", { query: "a", ...ALL, line: 4 });
  assert.equal(count(w), "4 of 4");
  editor.show("b.py");
  assert.equal(count(w), "1 of 1");
  assert.equal(w.document.querySelectorAll(".cm-find-current").length, 1);
  editor.destroy();
});

test("an empty match in front of an astral character moves on instead of looping", () => {
  const { w, editor } = boot();
  editor.load("a.py", "😀b\n", "python", false);
  editor.find("a.py", { query: "b*", ...ALL, regex: true, line: 1 });
  assert.equal(count(w), "1 of 1");
  editor.destroy();
});

const gotoInput = (w) => w.document.querySelector(".cm-goto-bar input");
const enter = (w, input) => input.dispatchEvent(new w.KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
const head = (w) => viewOf(w).state.doc.lineAt(viewOf(w).state.selection.main.head).number;

test("Go to line opens a one field panel and Enter jumps to the line and closes it", () => {
  const { w, editor } = boot();
  editor.load("a.py", "a\nb\nc\nd\n", "python", false);
  editor.openGoto();
  const input = gotoInput(w);
  assert.ok(input);
  input.value = "3";
  enter(w, input);
  assert.equal(head(w), 3);
  assert.equal(gotoInput(w), null);
  editor.destroy();
});

test("a line past the end goes to the last line", () => {
  const { w, editor } = boot();
  editor.load("a.py", "a\nb\nc\n", "python", false);
  editor.openGoto();
  const input = gotoInput(w);
  input.value = "99";
  enter(w, input);
  assert.equal(head(w), viewOf(w).state.doc.lines);
  editor.destroy();
});

test("0, a negative and text are ignored and keep the panel open", () => {
  const { w, editor } = boot();
  editor.load("a.py", "a\nb\nc\n", "python", false);
  editor.openGoto();
  for (const bad of ["0", "-2", "abc", ""]) {
    const input = gotoInput(w);
    input.value = bad;
    enter(w, input);
    assert.ok(gotoInput(w), `panel stays for ${JSON.stringify(bad)}`);
    assert.equal(head(w), 1);
  }
  editor.destroy();
});

test("Escape closes the Go to line panel", () => {
  const { w, editor } = boot();
  editor.load("a.py", "a\n", "python", false);
  editor.openGoto();
  gotoInput(w).dispatchEvent(new w.KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
  assert.equal(gotoInput(w), null);
  editor.destroy();
});

test("the editor right click menu lists Go to line with its shortcut", () => {
  const { w, editor } = boot();
  editor.load("a.py", "a\nb\n", "python", false);
  const content = w.document.querySelector(".cm-content");
  content.dispatchEvent(new w.MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: 5, clientY: 5 }));
  const texts = [...w.document.querySelectorAll(".cm-ctx-menu button")].map((b) => b.textContent);
  assert.ok(texts.some((t) => t.startsWith("Go to line") && t.includes("⌃G")), texts.join("|"));
  editor.destroy();
});

test("Go to line ignores text that only starts with a number", () => {
  const { w, editor } = boot();
  editor.load("a.py", "a\nb\nc\n", "python", false);
  editor.openGoto();
  for (const bad of ["3abc", "1e9", "2 3"]) {
    const input = gotoInput(w);
    input.value = bad;
    enter(w, input);
    assert.ok(gotoInput(w), `panel stays for ${JSON.stringify(bad)}`);
    assert.equal(head(w), 1);
  }
  editor.destroy();
});

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
