// The in-file find bar: a panel at the top of the editor with the query, three toggles, a count
// and next / previous, plus a mark on every match (the current one outlined).
//
// The state is the editor's, not a tab's (`remember` is told every change, and the editor puts it
// back after a tab switch). Matching is a JS RegExp with the `u` flag.
import { StateEffect, StateField } from "@codemirror/state";
import { Decoration, EditorView, keymap, showPanel } from "@codemirror/view";
import { t } from "./i18n.js";

const MAX_MATCHES = 20000;
const WORD_BEFORE = "(?<![\\p{L}\\p{N}_])";
const WORD_AFTER = "(?![\\p{L}\\p{N}_])";

const setFind = StateEffect.define();

export function findMatches(text, { query, match_case, whole_word, regex }) {
  if (query === "") return { matches: [], invalid: false };
  let body = regex ? query : query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  if (whole_word) body = `${WORD_BEFORE}(?:${body})${WORD_AFTER}`;
  let re;
  try {
    re = new RegExp(body, match_case ? "gmu" : "gimu");
  } catch {
    return { matches: [], invalid: true };
  }
  const matches = [];
  for (let m = re.exec(text); m !== null && matches.length < MAX_MATCHES; m = re.exec(text)) {
    if (m[0] === "") {
      // Step over one code point, never into the middle of a surrogate pair.
      re.lastIndex += text.codePointAt(re.lastIndex) > 0xffff ? 2 : 1;
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

const none = { matches: [], invalid: false };
let cached = null; // { doc, f, value }: the matches of the last (document, query, options)

// Every part of the bar (marks, count, next, the jump) asks for the matches: they are computed once
// per document and query, not once per ask.
function result(state) {
  const f = state.field(findField);
  if (!f.open) return none;
  const same = (a, b) => a.query === b.query && a.match_case === b.match_case && a.whole_word === b.whole_word && a.regex === b.regex;
  if (cached && cached.doc === state.doc && same(cached.f, f)) return cached.value;
  const value = findMatches(state.doc.toString(), f);
  cached = { doc: state.doc, f, value };
  return value;
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
    input.placeholder = t("Find");
    const toggles = [];
    for (const [opt, label, title] of [["match_case", "Aa", t("Match case")], ["whole_word", "ab", t("Whole word")], ["regex", ".*", t("Regex")]]) {
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
      if (event.key === "Enter" && !event.isComposing && event.keyCode !== 229) {
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
      countEl.textContent = f.query === "" ? "" : matches.length === 0 ? t("No results") : t("{n} of {m}", { n: Math.min(f.current, matches.length - 1) + 1, m: matches.length });
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
      const at = view.state.doc.line(Math.min(Math.max(line || 1, 1), view.state.doc.lines)).from;
      const { matches } = result(view.state);
      if (matches.length === 0) {
        // Nothing found, but the person still lands on the hit's line.
        view.dispatch({ selection: { anchor: at }, effects: EditorView.scrollIntoView(at, { y: "center" }) });
        return;
      }
      apply(view, { current: indexAt(matches, at) }, { scroll: true });
    },
    // Puts the editor's remembered state into a state that was just built (a tab switch).
    restore(view, state) {
      if (state && state.open) {
        apply(view, state);
        // `current` came from another tab: it is the first match at or after this tab's caret.
        const { matches } = result(view.state);
        if (matches.length > 0) apply(view, { current: indexAt(matches, view.state.selection.main.head) });
      } else if (view.state.field(findField).open) apply(view, { open: false });
    },
  };
}
