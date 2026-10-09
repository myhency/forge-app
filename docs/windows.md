# Windows

Forge is written to run on Windows 11 as well as macOS. Nothing here has been run on a real Windows PC yet:
the Windows code is checked by compiling for `x86_64-pc-windows-msvc` from a Mac
(`cargo check --workspace --all-targets --target x86_64-pc-windows-msvc`), and the path logic is tested with
both `/` and `\` paths on any platform.

## What the PC needs

- Windows 11 (it has WebView2, which the window runs on).
- **Git for Windows.** Claude Code runs its Bash tool through Git Bash. If it is not on the usual path, set
  `CLAUDE_CODE_GIT_BASH_PATH`.
- Nothing else: Node 22 comes in the package (`runtime/node.exe`).

## Build a package

On the platform the package is for (the program is built with that platform's toolchain):

```
node scripts/package/build.mjs
```

- macOS gives `dist/Forge.app` and a zip. The build is signed ad hoc, so a colleague who downloads it opens it
  once with right-click > Open (or `xattr -dr com.apple.quarantine Forge.app`).
- Windows gives `dist/Forge-win-x64/` (`Forge.exe` and `runtime/`) and a zip. It is a folder to unzip, not an
  installer.

`scripts/package/stage-runtime.mjs` gathers `runtime/` (Node, the built sidecar, its production dependencies with
the Claude binary of the target platform). `--mirror <url>` downloads Node from an internal mirror instead of
nodejs.org. Forge finds `runtime/` beside its program (Windows), in `Contents/Resources` (macOS app), and falls
back to the `node` on the PATH when running from the source tree. `FORGE_NODE` and `FORGE_SIDECAR_SCRIPT` override.

## What differs on Windows

| Area | Windows |
| --- | --- |
| Project folder names under `~/.claude/projects` | the path is made without the `\\?\` prefix (`dunce`) so it matches Claude Code's |
| Paths in the chat, the file links, background task output | `C:\x\y`, `C:/x/y` and `/x/y` are all read as absolute |
| Command key | Ctrl (Meta is the Windows key); labels read `Ctrl+K` |
| Terminal | PowerShell (`powershell.exe`) |
| Open a link, show a file, open with the default app | `rundll32 url.dll,FileProtocolHandler`, `explorer /select,` |
| Open in VS Code | `code.cmd` (VS Code's installer puts it on the PATH) |
| Skill folders for a session | junctions instead of symlinks (no administrator rights needed) |

## Not checked yet (do these on a Windows PC)

- The window opens and the sidecar starts from `runtime/node.exe`; a session answers.
- Opening a folder, the session list of a project that already has sessions, the Files panel, Changes (git), Search.
- Terminal panel (ConPTY), including `claude` run inside it.
- Show in Explorer, Open in VS Code, delete a session to the Recycle Bin.
- Rust unit tests on Windows (`cargo test --workspace`): some tests still write Unix paths such as `/work/a`
  and may fail there without the app being wrong. The Unix-only ones are already skipped.
