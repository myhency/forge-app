// The text of the editor's own bars and menus. The English sentence is the key; Rust sends the
// person's language with `strings({ "English": "text", ... })` when the editor starts and when the
// language changes. A sentence the table does not have stays English. `{name}` marks a value.
let table = {};

export function setStrings(map) {
  const next = {};
  if (map && typeof map === "object") {
    for (const [english, text] of Object.entries(map)) if (typeof text === "string") next[english] = text;
  }
  table = next;
}

export function t(english, values) {
  let text = Object.prototype.hasOwnProperty.call(table, english) ? table[english] : english;
  if (values) for (const [name, value] of Object.entries(values)) text = text.split(`{${name}}`).join(String(value));
  return text;
}
