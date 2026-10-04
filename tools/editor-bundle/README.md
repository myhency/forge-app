# Editor bundle

CodeMirror 6 for the Files panel, bundled into `crates/forge-shared/assets/editor.js`.
Forge puts that file inline in the window head (`forge_shared::editor_head`), like xterm.js, so
building Forge needs no Node. Only rebuild when `src/main.js` or a dependency changes:

    cd tools/editor-bundle
    npm install
    npm run build

Commit `package-lock.json` and the rebuilt `editor.js`. The licences of CodeMirror and Lezer
(MIT) are in `crates/forge-shared/assets/EDITOR-LICENSE`.

The page API (`window.ForgeEditor`) is described at the top of `src/main.js`; the Rust side that
drives it is `crates/forge-widgets/src/files_editor.rs`.
