# Editor bundle

CodeMirror 6 for the Files panel, bundled into `crates/forge-shared/assets/editor.js`.
Forge puts that file inline in the window head (`forge_shared::editor_head`), like xterm.js, so
building Forge needs no Node. Only rebuild when `src/main.js` or a dependency changes:

    cd tools/editor-bundle
    npm install
    npm run build

`npm test` runs the built bundle in jsdom and checks the messages it sends to Rust (dirty, the
text two seconds later, flush, missing, drop). Commit `package-lock.json` and the rebuilt `editor.js`. The licences of CodeMirror and Lezer
(MIT) are in `crates/forge-shared/assets/EDITOR-LICENSE`.

The page API (`window.ForgeEditor`) is described at the top of `src/main.js`; the Rust side that
drives it is `crates/forge-widgets/src/files_editor.rs`.

The bars and menus the editor draws itself (find bar, go to line, line comments, right click menu) take their text from `src/i18n.js`: the English sentence is the key, and Rust sends the person's language with `strings({...})` (`strings_for` in `files_editor.rs`). A new sentence in the JS needs a `t("...")` there, a `msg!("...")` in `strings_for` and an entry in `crates/forge-shared/src/ko.rs`.
