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
