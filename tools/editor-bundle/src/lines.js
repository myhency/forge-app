// Line tools for the editor: pick lines (gutter click, drag, shift click), the bar over them,
// comment boxes between lines, and the right click menu.
//
// Selection and comments are plain CodeMirror state, so they follow the tab's EditorState. A pick
// is { from, to } in 1 based line numbers.
import { StateEffect, StateField } from "@codemirror/state";
import { Decoration, EditorView, GutterMarker, WidgetType, gutter, lineNumbers, showTooltip } from "@codemirror/view";
import { redo, selectAll, undo } from "@codemirror/commands";

const setPick = StateEffect.define();
const setComments = StateEffect.define();
const setEditing = StateEffect.define(); // { id } of a saved comment shown as editing, or null
const openDraft = StateEffect.define(); // { from, to } of a new comment box, or null

const pickField = StateField.define({
  create: () => null,
  update(value, tr) {
    for (const e of tr.effects) if (e.is(setPick)) return e.value;
    return value;
  },
  provide: (f) => [
    EditorView.decorations.compute([f], (state) => pickDecorations(state, state.field(f))),
    showTooltip.compute([f], (state) => {
      const pick = state.field(f);
      if (!pick) return null;
      const pos = state.doc.line(Math.min(pick.from, state.doc.lines)).from;
      return { pos, above: true, strictSide: true, arrow: false, create: () => ({ dom: barDom(state) }) };
    }),
  ],
});

function pickDecorations(state, pick) {
  if (!pick) return Decoration.none;
  const marks = [];
  const last = Math.min(pick.to, state.doc.lines);
  for (let n = pick.from; n <= last; n++) marks.push(Decoration.line({ class: "cm-picked-line" }).range(state.doc.line(n).from));
  return Decoration.set(marks);
}

let currentView = null; // set by `lineTools`, so the bar (built from state) can reach the view
function barDom() {
  const dom = document.createElement("div");
  dom.className = "cm-pick-bar";
  for (const [act, label] of [["attach", "Attach as context"], ["comment", "Add comment"]]) {
    const button = document.createElement("button");
    button.type = "button";
    button.dataset.act = act;
    button.textContent = label;
    // mousedown, not click: the editor must not lose the pick to a blur first.
    button.addEventListener("mousedown", (event) => {
      event.preventDefault();
      if (act === "attach") currentView.__lines.attach();
      else currentView.__lines.startComment();
    });
    dom.appendChild(button);
  }
  return dom;
}

class PlusMarker extends GutterMarker {
  toDOM() {
    const span = document.createElement("span");
    span.className = "cm-pick-plus";
    span.textContent = "+";
    return span;
  }
}
const plus = new PlusMarker();

// Saved comments and the draft box. Block widgets must come from a state field.
const commentsField = StateField.define({
  create: () => ({ list: [], editing: null, draft: null }),
  update(value, tr) {
    let next = value;
    for (const e of tr.effects) {
      if (e.is(setComments)) next = { ...next, list: e.value, editing: next.editing && e.value.some((c) => c.id === next.editing) ? next.editing : null };
      if (e.is(setEditing)) next = { ...next, editing: e.value, draft: null };
      if (e.is(openDraft)) next = { ...next, draft: e.value, editing: null };
    }
    return next;
  },
  provide: (f) => EditorView.decorations.compute([f], (state) => commentDecorations(state, state.field(f))),
});

function commentDecorations(state, { list, editing, draft }) {
  const widgets = [];
  const at = (to) => state.doc.line(Math.min(Math.max(to, 1), state.doc.lines)).to;
  for (const c of list) widgets.push(Decoration.widget({ widget: new CommentBox(c, c.id === editing), block: true, side: 1 }).range(at(c.to)));
  if (draft) widgets.push(Decoration.widget({ widget: new CommentBox({ id: null, from: draft.from, to: draft.to, text: "" }, true), block: true, side: 1 }).range(at(draft.to)));
  return Decoration.set(widgets, true);
}

function title(c) {
  return c.from === c.to ? `Line ${c.from}` : `Lines ${c.from}-${c.to}`;
}

class CommentBox extends WidgetType {
  constructor(comment, editing) {
    super();
    this.comment = comment;
    this.editing = editing;
  }
  eq(other) {
    const a = this.comment, b = other.comment;
    return this.editing === other.editing && a.id === b.id && a.from === b.from && a.to === b.to && a.text === b.text;
  }
  ignoreEvent() {
    return true;
  }
  toDOM(view) {
    const c = this.comment;
    const box = document.createElement("div");
    box.className = "cm-comment-box " + (this.editing ? "editing" : "saved");
    const head = document.createElement("div");
    head.className = "cm-comment-title";
    head.textContent = title(c);
    box.appendChild(head);
    const send = view.__lines.send;
    if (this.editing) {
      const input = document.createElement("input");
      input.type = "text";
      input.placeholder = "Add comment";
      input.value = c.text;
      const ok = document.createElement("button");
      ok.type = "button";
      ok.className = "cm-comment-ok";
      ok.textContent = "⊕";
      const submit = () => {
        const text = input.value.trim();
        if (text === "") return;
        send({ t: "comment", key: view.__lines.key(), from: c.from, to: c.to, text, id: c.id });
        view.dispatch({ effects: [setEditing.of(null), openDraft.of(null)] });
      };
      ok.addEventListener("mousedown", (event) => { event.preventDefault(); submit(); });
      input.addEventListener("keydown", (event) => {
        if (event.key === "Enter" && !event.isComposing) { event.preventDefault(); submit(); }
        if (event.key === "Escape") view.dispatch({ effects: [setEditing.of(null), openDraft.of(null)] });
      });
      box.append(input, ok);
    } else {
      const text = document.createElement("div");
      text.className = "cm-comment-text";
      text.textContent = c.text;
      const edit = document.createElement("button");
      edit.type = "button";
      edit.className = "cm-comment-edit";
      edit.textContent = "✎";
      const del = document.createElement("button");
      del.type = "button";
      del.className = "cm-comment-delete";
      del.textContent = "🗑";
      del.addEventListener("mousedown", (event) => { event.preventDefault(); event.stopPropagation(); send({ t: "comment_delete", key: view.__lines.key(), id: c.id }); });
      edit.addEventListener("mousedown", (event) => { event.preventDefault(); event.stopPropagation(); view.dispatch({ effects: setEditing.of(c.id) }); });
      box.addEventListener("mousedown", () => view.dispatch({ effects: setEditing.of(c.id) }));
      box.append(text, edit, del);
    }
    return box;
  }
}

export function lineTools({ send, getKey }) {
  let anchor = null;

  function lineOf(view, pos) {
    return view.state.doc.lineAt(pos).number;
  }
  function pick(view, a, b) {
    view.dispatch({ effects: setPick.of({ from: Math.min(a, b), to: Math.max(a, b) }) });
  }
  function startPick(view, line, shift) {
    const current = view.state.field(pickField);
    anchor = shift && current ? (line < current.from ? current.to : current.from) : line;
    pick(view, anchor, line);
    const move = (event) => {
      const pos = view.posAtCoords({ x: event.clientX, y: event.clientY });
      if (pos != null) pick(view, anchor, lineOf(view, pos));
    };
    const up = () => {
      document.removeEventListener("mousemove", move);
      document.removeEventListener("mouseup", up);
    };
    document.addEventListener("mousemove", move);
    document.addEventListener("mouseup", up);
  }

  function attach(view) {
    const p = view.state.field(pickField);
    if (!p) return;
    const lines = [];
    for (let n = p.from; n <= Math.min(p.to, view.state.doc.lines); n++) lines.push(view.state.doc.line(n).text);
    send({ t: "attach", key: getKey(), from: p.from, to: p.to, text: lines.join("\n") });
  }
  function startComment(view, from, to) {
    const p = view.state.field(pickField);
    const range = from != null ? { from, to } : p;
    if (!range) return;
    pick(view, range.from, range.to);
    view.dispatch({ effects: openDraft.of({ from: range.from, to: range.to }) });
    setTimeout(() => view.dom.querySelector(".cm-comment-box.editing input")?.focus(), 0);
  }

  const gutterMarkers = gutter({
    class: "cm-plus-gutter",
    lineMarker(view, line) {
      const p = view.state.field(pickField);
      return p && view.state.doc.lineAt(line.from).number === p.from ? plus : null;
    },
    lineMarkerChange: (update) => update.transactions.some((tr) => tr.effects.some((e) => e.is(setPick))),
  });

  const extensions = [
    pickField,
    commentsField,
    gutterMarkers,
    lineNumbers({
      domEventHandlers: {
        mousedown(view, line, event) {
          if (event.button !== 0) return false;
          startPick(view, lineOf(view, line.from), event.shiftKey);
          return true;
        },
      },
    }),
    EditorView.domEventHandlers({
      contextmenu(event, view) {
        event.preventDefault();
        const pos = view.posAtCoords({ x: event.clientX, y: event.clientY });
        if (pos != null) {
          const line = lineOf(view, pos);
          const p = view.state.field(pickField);
          if (!p || line < p.from || line > p.to) pick(view, line, line);
        }
        openMenu(view, event.clientX, event.clientY);
        return true;
      },
    }),
  ];

  function openMenu(view, x, y) {
    document.querySelector(".cm-ctx-menu")?.remove();
    const menu = document.createElement("div");
    menu.className = "cm-ctx-menu";
    const items = [
      ["Attach as context", () => attach(view)],
      ["Add comment", () => startComment(view)],
      null,
      ["Undo", () => undo(view)],
      ["Redo", () => redo(view)],
      null,
      ["Cut", () => document.execCommand("cut")],
      ["Copy", () => document.execCommand("copy")],
      ["Paste", async () => { try { view.dispatch(view.state.replaceSelection(await navigator.clipboard.readText())); } catch { /* no clipboard access */ } }],
      null,
      ["Select all", () => selectAll(view)],
    ];
    for (const item of items) {
      if (item === null) {
        const rule = document.createElement("div");
        rule.className = "cm-ctx-rule";
        menu.appendChild(rule);
        continue;
      }
      const button = document.createElement("button");
      button.type = "button";
      button.textContent = item[0];
      button.addEventListener("mousedown", (event) => { event.preventDefault(); menu.remove(); item[1](); view.focus(); });
      menu.appendChild(button);
    }
    menu.style.left = `${Math.min(x, window.innerWidth - 190)}px`;
    menu.style.top = `${Math.min(y, window.innerHeight - 260)}px`;
    document.body.appendChild(menu);
    const close = () => { menu.remove(); document.removeEventListener("mousedown", close, true); };
    setTimeout(() => document.addEventListener("mousedown", close, true), 0);
  }

  return {
    extensions,
    bind(view) {
      currentView = view;
      view.__lines = { send, key: getKey, attach: () => attach(view), startComment: () => startComment(view) };
    },
    setComments(view, list) {
      view.dispatch({ effects: setComments.of(list) });
    },
    focusComment(view, id) {
      const c = view.state.field(commentsField).list.find((x) => x.id === id);
      if (!c) return;
      view.dispatch({ effects: [setEditing.of(id), EditorView.scrollIntoView(view.state.doc.line(Math.min(c.to, view.state.doc.lines)).from, { y: "center" })] });
      setTimeout(() => view.dom.querySelector(".cm-comment-box.editing input")?.focus(), 0);
    },
    startComment: (view, from, to) => startComment(view, from, to),
  };
}
