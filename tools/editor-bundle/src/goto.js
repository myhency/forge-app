// Go to line: a one field panel above the editor (like the find bar). Enter jumps (a number past the
// end goes to the last line), Escape closes; 0, a negative number and text are ignored.
import { StateEffect, StateField } from "@codemirror/state";
import { EditorView, keymap, showPanel } from "@codemirror/view";

const setGoto = StateEffect.define();

const gotoField = StateField.define({
  create: () => false,
  update(open, tr) {
    for (const e of tr.effects) if (e.is(setGoto)) return e.value;
    return open;
  },
});

export function gotoTools() {
  function open(view) {
    view.dispatch({ effects: setGoto.of(true) });
    view.dom.querySelector(".cm-goto-bar input")?.focus();
  }

  function close(view) {
    view.dispatch({ effects: setGoto.of(false) });
    view.focus();
  }

  function makePanel(view) {
    const dom = document.createElement("div");
    dom.className = "cm-goto-bar";
    const input = document.createElement("input");
    input.type = "text";
    input.inputMode = "numeric";
    input.placeholder = `Go to line (1-${view.state.doc.lines})`;
    input.addEventListener("keydown", (event) => {
      if (event.key === "Enter" && !event.isComposing && event.keyCode !== 229) {
        event.preventDefault();
        // Only a whole number counts: "3abc" and "1e9" are not line numbers.
        if (!/^\s*\d+\s*$/.test(input.value)) return;
        const n = Number.parseInt(input.value, 10);
        if (n < 1) return;
        const line = view.state.doc.line(Math.min(n, view.state.doc.lines));
        view.dispatch({ selection: { anchor: line.from }, effects: [setGoto.of(false), EditorView.scrollIntoView(line.from, { y: "center" })] });
        view.focus();
      } else if (event.key === "Escape") {
        event.preventDefault();
        close(view);
      }
    });
    dom.appendChild(input);
    return { dom, top: true, mount: () => input.focus() };
  }

  const panel = showPanel.compute([gotoField], (state) => (state.field(gotoField) ? makePanel : null));
  const keys = keymap.of([{ key: "Ctrl-g", run: (view) => { open(view); return true; } }]);
  return { extensions: [gotoField, panel, keys], open };
}
