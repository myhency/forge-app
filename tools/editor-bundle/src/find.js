// The in-file find bar: a panel at the top of the editor with the query, three toggles, a count
// and next / previous, plus a mark on every match (the current one outlined).
//
// The state is the editor's, not a tab's (`remember` is told every change, and the editor puts it
// back after a tab switch). Matching is a JS RegExp with the `u` flag.
import { StateEffect, StateField } from "@codemirror/state";
import { Decoration, EditorView, keymap, showPanel } from "@codemirror/view";

const MAX_MATCHES = 20000;
const WORD_BEFORE = "(?<![\\p{L}\\p{N}_])";
const WORD_AFTER = "(?![\\p{L}\\p{N}_])";

const setFind = StateEffect.define();

export function findMatches(text, { query, match_case, whole_word, regex }) {
  if (query === "") return { matches: [], invalid: false };
  let body = regex ? query : query.replace(/[.*+?^${}()|[\]\\\/-]/g, "\\$&");
  if (whole_word) body = `${WORD_BEFORE}(?:${body})${WORD_AFTER}`;
  let re;
  try {
    re = new RegExp(body, match_case ? "gu" : "giu");
  } catch {
    return { matches: [], invalid: true };
  }
  const matches = [];
  for (let m = re.exec(text); m !== null && matches.length < MAX_MATCHES; m = re.exec(text)) {
    if (m[0] === "") {
      re.lastIndex += 1;
      continue;
    }
    matches.push({ from: m.index, to: m.index + m[0].length });
  }
  return { matches, invalid: false };
}

const empty = { open: false, query: "", match_case: false, whole_word: false, regex: false, current: 0 };

const findField = StateField.define({
  create: () => empty,
  update(value, tr) {
    for (const e of tr.effects) if (e.is(setFind)) return { ...value, ...e.value };
    return value;
  },
});

function result(state) {
  const f = state.field(findField);
  return f.open ? findMatches(state.doc.toString(), f) : { matches: [], invalid: false };
}

const markAll = Decoration.mark({ class: "cm-find-match" });
const markCurrent = Decoration.mark({ class: "cm-find-match cm-find-current" });

const decorations = EditorView.decorations.compute(["doc", findField], (state) => {
  const f = state.field(findField);
  if (!f.open) return Decoration.none;
  const { matches } = result(state);
  return Decoration.set(matches.map((m, i) => (i === f.current ? markCurrent : markAll).range(m.from, m.to)));
});

export function findTools({ remember }) {
  function apply(view, patch, { scroll = false } = {}) {
    view.dispatch({ effects: setFind.of(patch) });
    remember(view.state.field(findField));
    if (scroll) goTo(view);
  }

  function goTo(view) {
    const f = view.state.field(findField);
    const { matches } = result(view.state);
    const m = matches[f.current];
    if (!m) return;
    view.dispatch({ selection: { anchor: m.from }, effects: EditorView.scrollIntoView(m.from, { y: "center" }) });
  }

  // The first match at or after `pos`, wrapping to the first one.
  function indexAt(matches, pos) {
    const i = matches.findIndex((m) => m.from >= pos);
    return i === -1 ? 0 : i;
  }

  function step(view, delta) {
    const { matches } = result(view.state);
    if (matches.length === 0) return;
    const f = view.state.field(findField);
    apply(view, { current: (f.current + delta + matches.length) % matches.length }, { scroll: true });
  }

  function makePanel(view) {
    const dom = document.createElement("div");
    dom.className = "cm-find-bar";
    const input = document.createElement("input");
    input.type = "text";
    input.placeholder = "Find";
    const toggles = [];
    for (const [opt, label, title] of [["match_case", "Aa", "Match case"], ["whole_word", "ab", "Whole word"], ["regex", ".*", "Regex"]]) {
      const button = document.createElement("button");
      button.type = "button";
      button.dataset.opt = opt;
      button.title = title;
      button.textContent = label;
      button.addEventListener("mousedown", (event) => {
        event.preventDefault();
        const f = view.state.field(findField);
        apply(view, { [opt]: !f[opt], current: 0 });
        jumpToCurrent(view);
      });
      toggles.push(button);
    }
    const countEl = document.createElement("span");
    countEl.className = "cm-find-count";
    const mk = (cls, text, delta) => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = cls;
      button.textContent = text;
      button.addEventListener("mousedown", (event) => { event.preventDefault(); step(view, delta); });
      return button;
    };
    const close = document.createElement("button");
    close.type = "button";
    close.className = "cm-find-close";
    close.textContent = "×";
    close.addEventListener("mousedown", (event) => { event.preventDefault(); closeBar(view); });

    input.addEventListener("input", () => {
      apply(view, { query: input.value, current: 0 });
      jumpToCurrent(view);
    });
    input.addEventListener("keydown", (event) => {
      if (event.key === "Enter" && !event.isComposing) {
        event.preventDefault();
        step(view, event.shiftKey ? -1 : 1);
      } else if (event.key === "Escape") {
        event.preventDefault();
        closeBar(view);
      }
    });

    dom.append(input, ...toggles, countEl, mk("cm-find-prev", "↑", -1), mk("cm-find-next", "↓", 1), close);

    function refresh(state) {
      const f = state.field(findField);
      const { matches, invalid } = result(state);
      if (document.activeElement !== input) input.value = f.query;
      input.classList.toggle("invalid", invalid);
      for (const button of toggles) {
        const on = f[button.dataset.opt];
        button.classList.toggle("on", on);
        button.setAttribute("aria-pressed", String(on));
      }
      countEl.textContent = f.query === "" ? "" : matches.length === 0 ? "No results" : `${Math.min(f.current, matches.length - 1) + 1} of ${matches.length}`;
    }
    refresh(view.state);
    return { dom, top: true, mount: () => input.focus(), update: (update) => refresh(update.state) };
  }

  // After the query or an option changed, the current match is the first one at or after the caret.
  function jumpToCurrent(view) {
    const { matches } = result(view.state);
    if (matches.length === 0) return;
    const index = indexAt(matches, view.state.selection.main.head);
    apply(view, { current: index }, { scroll: true });
  }

  function closeBar(view) {
    apply(view, { open: false });
    view.focus();
  }

  const panel = showPanel.compute([findField], (state) => (state.field(findField).open ? makePanel : null));

  const keys = keymap.of([
    {
      key: "Mod-f",
      run(view) {
        const sel = view.state.sliceDoc(view.state.selection.main.from, view.state.selection.main.to);
        const patch = { open: true };
        if (sel !== "" && !sel.includes("\n")) patch.query = sel;
        apply(view, patch);
        jumpToCurrent(view);
        view.dom.querySelector(".cm-find-bar input")?.select();
        return true;
      },
    },
  ]);

  return {
    extensions: [findField, decorations, panel, keys],
    // `line` is 1 based. The first match on that line, else the next one after it, is current.
    open(view, { line, ...options }) {
      apply(view, { open: true, ...options, current: 0 });
      const { matches } = result(view.state);
      if (matches.length === 0) return;
      const at = view.state.doc.line(Math.min(Math.max(line || 1, 1), view.state.doc.lines)).from;
      apply(view, { current: indexAt(matches, at) }, { scroll: true });
    },
    // Puts the editor's remembered state into a state that was just built (a tab switch).
    restore(view, state) {
      if (state && state.open) apply(view, state);
      else if (view.state.field(findField).open) apply(view, { open: false });
    },
  };
}
