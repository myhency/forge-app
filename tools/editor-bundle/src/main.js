// window.ForgeEditor.create(host, send) builds one CodeMirror editor in `host`.
//
// One editor serves every tab: each tab's EditorState (text, undo history) and scroll position are
// kept per key, so switching tabs keeps what the person did.
//
// Rust to page (methods): load(key, doc, language, readonly, ifClean), show(key), drop(key),
//   settings({ wrap, indent_guides, whitespace, scroll_past_end, tab_indent, tab_size }),
//   flush(key), comments(key, [{ id, from, to, text }]), focusComment(key, id),
//   find(key, { query, match_case, whole_word, regex, line }), focus(), destroy().
// Page to Rust (send): { t: "dirty", key }, { t: "change", key, s }, { t: "save", key, s },
//   { t: "flush", key, s }, { t: "missing", key }, { t: "skipped", key },
//   { t: "attach", key, from, to, text }, { t: "comment", key, from, to, text, id }, { t: "comment_delete", key, id }.
//
// `change` is sent 2 s after the last edit, once per pause, never during an IME composition, and at
// once when the editor loses focus. `load(..., ifClean)` does nothing and says `skipped` when an
// edit of the same tab is waiting (a quiet refresh must never replace what the person typed).
import { Compartment, EditorState } from "@codemirror/state";
import {
  Decoration,
  EditorView,
  ViewPlugin,
  drawSelection,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightWhitespace,
  keymap,
  scrollPastEnd,
} from "@codemirror/view";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import {
  HighlightStyle,
  StreamLanguage,
  bracketMatching,
  foldGutter,
  foldKeymap,
  indentOnInput,
  indentUnit,
  syntaxHighlighting,
} from "@codemirror/language";
import { tags as t } from "@lezer/highlight";
import { css } from "@codemirror/lang-css";
import { html } from "@codemirror/lang-html";
import { javascript } from "@codemirror/lang-javascript";
import { json } from "@codemirror/lang-json";
import { markdown } from "@codemirror/lang-markdown";
import { python } from "@codemirror/lang-python";
import { rust } from "@codemirror/lang-rust";
import { yaml } from "@codemirror/lang-yaml";
import { shell } from "@codemirror/legacy-modes/mode/shell";
import { findTools } from "./find.js";
import { lineTools } from "./lines.js";

const SAVE_DELAY_MS = 2000;
const COMPOSING_RETRY_MS = 300;

const LANGUAGES = {
  javascript: () => javascript(),
  jsx: () => javascript({ jsx: true }),
  typescript: () => javascript({ typescript: true }),
  tsx: () => javascript({ jsx: true, typescript: true }),
  python: () => python(),
  rust: () => rust(),
  json: () => json(),
  markdown: () => markdown(),
  html: () => html(),
  css: () => css(),
  yaml: () => yaml(),
  shell: () => StreamLanguage.define(shell),
};

// Colours come from Forge's CSS variables, so the editor follows the light and dark theme.
const highlight = HighlightStyle.define([
  { tag: [t.comment, t.lineComment, t.blockComment], color: "var(--hl-comment)", fontStyle: "italic" },
  { tag: [t.keyword, t.controlKeyword, t.definitionKeyword, t.moduleKeyword, t.operatorKeyword], color: "var(--hl-keyword)" },
  { tag: [t.string, t.special(t.string), t.regexp], color: "var(--hl-string)" },
  { tag: [t.number, t.bool, t.null, t.atom], color: "var(--hl-number)" },
  { tag: [t.function(t.variableName), t.function(t.propertyName), t.definition(t.function(t.variableName))], color: "var(--hl-function)" },
  { tag: [t.typeName, t.className, t.namespace], color: "var(--hl-type)" },
  { tag: t.heading, color: "var(--hl-function)", fontWeight: "bold" },
  { tag: t.link, color: "var(--hl-function)", textDecoration: "underline" },
  { tag: t.emphasis, fontStyle: "italic" },
  { tag: t.strong, fontWeight: "bold" },
]);

const theme = EditorView.theme({
  "&": { height: "100%", fontSize: "12px", backgroundColor: "var(--bg)", color: "var(--fg)" },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": { fontFamily: "var(--mono)", lineHeight: "1.5", overflow: "auto" },
  ".cm-content": { caretColor: "var(--fg)" },
  ".cm-cursor": { borderLeftColor: "var(--fg)" },
  ".cm-gutters": { backgroundColor: "var(--bg)", color: "var(--fg-muted)", border: "none" },
  ".cm-activeLine": { backgroundColor: "var(--bg-hover)" },
  ".cm-activeLineGutter": { backgroundColor: "transparent", color: "var(--fg)" },
  ".cm-selectionBackground, &.cm-focused .cm-selectionBackground": { backgroundColor: "var(--accent-soft)" },
  ".cm-highlightSpace": {
    backgroundImage: "radial-gradient(circle at 50% 55%, var(--fg-muted) 20%, transparent 5%)",
  },
  ".cm-indent-guides": {
    backgroundImage:
      "repeating-linear-gradient(to right, var(--border) 0, var(--border) 1px, transparent 1px, transparent calc(var(--forge-tab, 4) * 1ch))",
  },
});

// Marks the leading whitespace of every visible line; the theme draws a thin line every tab size.
const guideMark = Decoration.mark({ class: "cm-indent-guides" });
function guideDecorations(view) {
  const ranges = [];
  for (const { from, to } of view.visibleRanges) {
    for (let pos = from; pos <= to; ) {
      const line = view.state.doc.lineAt(pos);
      const lead = /^[ \t]+/.exec(line.text);
      if (lead) ranges.push(guideMark.range(line.from, line.from + lead[0].length));
      pos = line.to + 1;
    }
  }
  return Decoration.set(ranges, true);
}
const indentGuides = ViewPlugin.fromClass(
  class {
    constructor(view) {
      this.decorations = guideDecorations(view);
    }
    update(update) {
      if (update.docChanged || update.viewportChanged) this.decorations = guideDecorations(update.view);
    }
  },
  { decorations: (plugin) => plugin.decorations },
);

window.ForgeEditor = {
  create(host, send) {
    const config = new Compartment();
    let settings = { wrap: true, indent_guides: true, whitespace: false, scroll_past_end: false, tab_indent: true, tab_size: 4 };
    let key = null;
    let timer = null;
    let dirtySent = false;
    // key -> { state, scroll, readonly }
    const states = new Map();
    // key -> the comments Rust last sent for that file
    const commentsByKey = new Map();
    // key -> a comment to open for editing as soon as that tab shows
    const pendingFocus = new Map();
    const tools = lineTools({ send, getKey: () => key });
    // The find bar belongs to the editor, not to a tab: its state is kept here and put back on a switch.
    let findState = null;
    const find = findTools({ remember: (state) => { findState = state; } });
    // key -> a find to open as soon as that tab shows
    const pendingFind = new Map();

    // The extensions the settings control, kept in one compartment so a change reconfigures them all.
    function configExtensions(readonly) {
      return [
        settings.wrap ? EditorView.lineWrapping : [],
        settings.indent_guides ? indentGuides : [],
        settings.whitespace ? highlightWhitespace() : [],
        settings.scroll_past_end ? scrollPastEnd() : [],
        EditorState.tabSize.of(settings.tab_size),
        indentUnit.of(" ".repeat(settings.tab_size)),
        keymap.of([...(settings.tab_indent ? [indentWithTab] : [])]),
        EditorState.readOnly.of(readonly),
        EditorView.editable.of(!readonly),
      ];
    }

    function onUpdate(update) {
      if (!update.docChanged || key === null) return;
      if (!dirtySent) {
        dirtySent = true;
        send({ t: "dirty", key });
      }
      clearTimeout(timer);
      timer = setTimeout(fire, SAVE_DELAY_MS);
    }

    function fire() {
      if (view.composing) {
        timer = setTimeout(fire, COMPOSING_RETRY_MS);
        return;
      }
      timer = null;
      dirtySent = false;
      send({ t: "change", key, s: view.state.doc.toString() });
    }

    // Sends a change that is waiting for its timer, so a switch of tab never leaves one behind.
    function flushPending() {
      if (timer !== null && key !== null) {
        clearTimeout(timer);
        timer = null;
        dirtySent = false;
        send({ t: "change", key, s: view.state.doc.toString() });
      }
    }

    function makeState(doc, language, readonly) {
      const lang = LANGUAGES[language];
      return EditorState.create({
        doc,
        extensions: [
          ...tools.extensions,
          ...find.extensions,
          foldGutter(),
          history(),
          drawSelection(),
          highlightActiveLine(),
          highlightActiveLineGutter(),
          bracketMatching(),
          indentOnInput(),
          syntaxHighlighting(highlight),
          theme,
          lang ? lang() : [],
          keymap.of([
            { key: "Mod-s", run: () => { send({ t: "save", key, s: view.state.doc.toString() }); return true; } },
            ...defaultKeymap,
            ...historyKeymap,
            ...foldKeymap,
          ]),
          config.of(configExtensions(readonly)),
          EditorView.updateListener.of(onUpdate),
          // A click on a panel button blurs the editor first, so an edit is not left for the timer.
          EditorView.domEventHandlers({ blur: () => { flushPending(); } }),
        ],
      });
    }

    const view = new EditorView({ parent: host, state: EditorState.create({ doc: "", extensions: [theme] }) });
    view.dom.style.setProperty("--forge-tab", String(settings.tab_size));
    tools.bind(view);

    function saveCurrent() {
      const entry = key === null ? null : states.get(key);
      if (entry) {
        entry.state = view.state;
        entry.scroll = view.scrollDOM.scrollTop;
      }
    }

    // A tab switch or a load waits for an IME composition to end, so it never cuts one in half.
    function whenIdle(work) {
      if (view.composing) setTimeout(() => whenIdle(work), 100);
      else work();
    }

    function afterSwitch(forKey) {
      find.restore(view, findState);
      const pending = pendingFind.get(forKey);
      if (pending) {
        pendingFind.delete(forKey);
        find.open(view, pending);
      }
    }

    function applyPendingFocus(forKey) {
      const id = pendingFocus.get(forKey);
      if (id === undefined) return;
      pendingFocus.delete(forKey);
      tools.focusComment(view, id);
    }

    function reapply(readonly) {
      view.dispatch({ effects: config.reconfigure(configExtensions(readonly)) });
    }

    return {
      load(newKey, doc, language, readonly, ifClean) {
        whenIdle(() => {
          if (newKey === key) {
            if (ifClean && (timer !== null || dirtySent)) {
              send({ t: "skipped", key: newKey });
              return;
            }
            // The text is being replaced on purpose: an edit waiting for its timer is discarded, not saved.
            clearTimeout(timer);
            timer = null;
          } else {
            flushPending();
          }
          saveCurrent();
          key = newKey;
          dirtySent = false;
          const state = makeState(doc, language, readonly);
          states.set(newKey, { state, scroll: 0, readonly });
          view.setState(state);
          reapply(readonly);
          tools.bind(view);
          tools.setComments(view, commentsByKey.get(newKey) || []);
          applyPendingFocus(newKey);
          afterSwitch(newKey);
          view.scrollDOM.scrollTop = 0;
        });
      },
      show(newKey) {
        whenIdle(() => {
          const entry = states.get(newKey);
          if (!entry) {
            send({ t: "missing", key: newKey });
            return;
          }
          flushPending();
          saveCurrent();
          key = newKey;
          dirtySent = false;
          view.setState(entry.state);
          reapply(entry.readonly);
          tools.bind(view);
          tools.setComments(view, commentsByKey.get(newKey) || []);
          applyPendingFocus(newKey);
          afterSwitch(newKey);
          view.scrollDOM.scrollTop = entry.scroll;
        });
      },
      drop(oldKey) {
        states.delete(oldKey);
        commentsByKey.delete(oldKey);
        pendingFind.delete(oldKey);
        if (key === oldKey) {
          clearTimeout(timer);
          timer = null;
          key = null;
          view.setState(EditorState.create({ doc: "", extensions: [theme] }));
        }
      },
      settings(next) {
        settings = { ...settings, ...next };
        view.dom.style.setProperty("--forge-tab", String(settings.tab_size));
        const entry = key === null ? null : states.get(key);
        if (entry) reapply(entry.readonly);
      },
      flush(askedKey) {
        if (askedKey === key) {
          // Only reports the text; a save that waits for its timer still happens.
          send({ t: "flush", key: askedKey, s: view.state.doc.toString() });
        } else {
          const entry = states.get(askedKey);
          send({ t: "flush", key: askedKey, s: entry ? entry.state.doc.toString() : null });
        }
      },
      comments(forKey, list) {
        commentsByKey.set(forKey, list);
        if (forKey === key) {
          tools.setComments(view, list);
          applyPendingFocus(forKey);
        }
      },
      focusComment(forKey, id) {
        // A tab that is not shown (or not loaded) yet takes the request when it shows.
        if (forKey === key) tools.focusComment(view, id);
        else pendingFocus.set(forKey, id);
      },
      find(forKey, opts) {
        // A tab that is not shown (or not loaded) yet takes the request when it shows.
        if (forKey === key) find.open(view, opts);
        else pendingFind.set(forKey, opts);
      },
      startComment(from, to) {
        tools.startComment(view, from, to);
      },
      focus() {
        view.focus();
      },
      destroy() {
        clearTimeout(timer);
        view.destroy();
      },
    };
  },
};
