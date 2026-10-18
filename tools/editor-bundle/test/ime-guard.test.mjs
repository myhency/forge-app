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
