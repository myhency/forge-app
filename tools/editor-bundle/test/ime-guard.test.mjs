// Runs crates/forge-shared/assets/ime-guard.js in jsdom: the Enter that confirms an input-method
// composition must not reach any handler, a plain Enter must.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { JSDOM } from "jsdom";

const code = readFileSync(new URL("../../../crates/forge-shared/assets/ime-guard.js", import.meta.url), "utf8");

function boot() {
  const dom = new JSDOM(`<!doctype html><body><textarea id="t"></textarea></body>`, { runScripts: "outside-only" });
  const w = dom.window;
  w.eval(code);
  const area = w.document.getElementById("t");
  const seen = [];
  // The app's own handler sits below the document, like Dioxus' listener on the root.
  area.addEventListener("keydown", (event) => seen.push(event.key));
  area.focus();
  return { w, area, seen };
}

// Types text the way WebKit reports a Korean syllable: keydown 229, then an input.
function press(w, area, key, keyCode, value, inputType = "insertReplacementText") {
  area.dispatchEvent(new w.KeyboardEvent("keydown", { key, keyCode, bubbles: true, cancelable: true }));
  if (value !== undefined) {
    area.value = value;
    area.dispatchEvent(new w.InputEvent("input", { inputType, bubbles: true }));
  }
}

test("the Enter after an input that confirmed the text unchanged is swallowed, the next one is not", () => {
  const { w, area, seen } = boot();
  press(w, area, "ㅎ", 229, "ㅎ", "insertText");
  press(w, area, "ㅏ", 229, "하");
  press(w, area, "ㄴ", 229, "한");
  // WebKit re-sends the text on Enter, then the keydown arrives with isComposing false.
  area.dispatchEvent(new w.InputEvent("input", { inputType: "insertReplacementText", bubbles: true }));
  const enter = new w.KeyboardEvent("keydown", { key: "Enter", keyCode: 13, bubbles: true, cancelable: true });
  area.dispatchEvent(enter);
  assert.equal(enter.defaultPrevented, true, "no newline in a textarea");
  assert.deepEqual(seen, ["ㅎ", "ㅏ", "ㄴ"], "the confirming Enter did not reach the handler");
  press(w, area, "Enter", 13);
  assert.deepEqual(seen, ["ㅎ", "ㅏ", "ㄴ", "Enter"], "the second Enter did");
});

test("a plain Enter after ASCII typing is not swallowed", () => {
  const { w, area, seen } = boot();
  press(w, area, "a", 65, "a", "insertText");
  press(w, area, "Enter", 13);
  assert.deepEqual(seen, ["a", "Enter"]);
});

test("an Enter with isComposing or keyCode 229 is swallowed", () => {
  const { w, area, seen } = boot();
  area.dispatchEvent(new w.KeyboardEvent("keydown", { key: "Enter", keyCode: 229, bubbles: true, cancelable: true }));
  area.dispatchEvent(new w.KeyboardEvent("keydown", { key: "Enter", isComposing: true, bubbles: true, cancelable: true }));
  assert.deepEqual(seen, []);
});

test("typing the same character again after the field was cleared does not arm the guard", () => {
  const { w, area, seen } = boot();
  press(w, area, "a", 65, "a", "insertText");
  area.value = ""; // the app cleared the box after sending
  press(w, area, "a", 65, "a", "insertText");
  press(w, area, "Enter", 13);
  assert.deepEqual(seen, ["a", "a", "Enter"]);
});

// Builds a page with two textareas, a checkbox and a text input, with a handler under the document.
function boot2() {
  const dom = new JSDOM(
    `<!doctype html><body><textarea id="a"></textarea><textarea id="b"></textarea><input id="c" type="checkbox"><input id="r" type="radio"><input id="t" type="text"></body>`,
    { runScripts: "outside-only" },
  );
  const w = dom.window;
  w.eval(code);
  const seen = [];
  w.document.body.addEventListener("keydown", (event) => seen.push(event.target.id + ":" + event.key));
  return { w, seen, el: (id) => w.document.getElementById(id) };
}

function stamp(event, ms) {
  Object.defineProperty(event, "timeStamp", { value: ms, configurable: true });
  return event;
}
function key(w, target, k, ms, init = {}) {
  const event = stamp(new w.KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true, ...init }), ms);
  target.dispatchEvent(event);
  return event;
}
function input(w, target, ms, value) {
  if (value !== undefined) target.value = value;
  target.dispatchEvent(stamp(new w.InputEvent("input", { bubbles: true }), ms));
}

test("the swallow lasts 149 ms after the unchanged input and not 150 ms", () => {
  const { w, seen, el } = boot2();
  const a = el("a");
  key(w, a, "x", 1000); input(w, a, 1001, "x");
  key(w, a, "y", 1002); input(w, a, 1003, "xy");
  input(w, a, 2000); // unchanged
  assert.equal(key(w, a, "Enter", 2149, { keyCode: 13 }).defaultPrevented, true);
  input(w, a, 3000); // unchanged
  assert.equal(key(w, a, "Enter", 3150, { keyCode: 13 }).defaultPrevented, false);
  assert.deepEqual(seen.filter((s) => s.endsWith("Enter")), ["a:Enter"], "only the late one reached the handler");
});

test("a composing Enter keeps its default, the Enter after the unchanged input does not", () => {
  const { w, seen, el } = boot2();
  const a = el("a");
  const composing = key(w, a, "Enter", 10, { keyCode: 229 });
  assert.equal(composing.defaultPrevented, false, "the input method's own confirmation runs");
  const flagged = key(w, a, "Enter", 20, { isComposing: true });
  assert.equal(flagged.defaultPrevented, false);
  key(w, a, "k", 30); input(w, a, 31, "k");
  input(w, a, 40); // unchanged
  assert.equal(key(w, a, "Enter", 50, { keyCode: 13 }).defaultPrevented, true);
  assert.deepEqual(seen, ["a:k"]);
});

test("a key between the unchanged input and the Enter disarms the guard", () => {
  const { w, seen, el } = boot2();
  const a = el("a");
  key(w, a, "k", 10); input(w, a, 11, "k");
  input(w, a, 20); // unchanged
  key(w, a, "ArrowLeft", 30);
  assert.equal(key(w, a, "Enter", 40, { keyCode: 13 }).defaultPrevented, false);
  assert.deepEqual(seen, ["a:k", "a:ArrowLeft", "a:Enter"]);
});

test("a checkbox or radio never arms the guard", () => {
  const { w, seen, el } = boot2();
  for (const id of ["c", "r"]) {
    key(w, el(id), " ", 10);
    input(w, el(id), 20); // value stays "on"
    assert.equal(key(w, el(id), "Enter", 30, { keyCode: 13 }).defaultPrevented, false);
  }
  assert.deepEqual(seen, ["c: ", "c:Enter", "r: ", "r:Enter"]);
});

test("a text input still arms the guard", () => {
  const { w, seen, el } = boot2();
  const t = el("t");
  key(w, t, "k", 10); input(w, t, 11, "k");
  input(w, t, 20);
  assert.equal(key(w, t, "Enter", 30, { keyCode: 13 }).defaultPrevented, true);
  assert.deepEqual(seen, ["t:k"]);
});

test("an unchanged input in field A does not swallow Enter in field B", () => {
  const { w, seen, el } = boot2();
  const a = el("a"), b = el("b");
  key(w, a, "k", 10); input(w, a, 11, "k");
  input(w, a, 20); // unchanged, A armed
  assert.equal(key(w, b, "Enter", 30, { keyCode: 13 }).defaultPrevented, false);
  assert.deepEqual(seen, ["a:k", "b:Enter"]);
});

test("the xterm helper textarea is never guarded", () => {
  const { w, area, seen } = boot();
  area.className = "xterm-helper-textarea";
  press(w, area, "ㄴ", 229, "한");
  area.dispatchEvent(new w.InputEvent("input", { inputType: "insertReplacementText", bubbles: true }));
  const enter = new w.KeyboardEvent("keydown", { key: "Enter", keyCode: 13, bubbles: true, cancelable: true });
  area.dispatchEvent(enter);
  assert.equal(enter.defaultPrevented, false);
  assert.deepEqual(seen, ["ㄴ", "Enter"]);
});
