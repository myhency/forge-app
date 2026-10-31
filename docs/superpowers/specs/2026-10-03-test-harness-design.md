# Plan 13: UI test harness

## Goal

Claude can launch the Forge desktop app against known, isolated data, look at the window and click
through it, so UI changes can be checked without the user testing by hand. Plan 12's visual checks
(same-named folder headings, view-only state) were only confirmable this way.

Forge is a native Dioxus webview, so Playwright and the Chrome tools cannot drive it. The harness
drives the real window through macOS screen capture and synthetic mouse events.

## Scope

In scope (level A, scripts only): an app change to isolate Forge's own config, a fixture builder,
and launch, capture, click and stop scripts.

Out of scope, to revisit once the app is more mature:
- B: a scenario runner (fixture, launch, click, capture, assert expected state in one run), which needs
  image comparison or OCR.
- C: Dioxus SSR render tests of `Sidebar` and the workspace views as a `cargo test` fallback.
- Sending a real message (a fake home has no login), IME, dark mode and other perception checks stay
  with the user.

## Already done

`FORGE_CLAUDE_HOME` in `crates/forge-app/src/main.rs` (`claude_paths`, 4 tests) moves what Forge reads
from the Claude home; `.claude.json` is read from the sibling of that directory. Not redone here.

## Design

### 1. App change: `FORGE_CONFIG_PATH`

`FORGE_CLAUDE_HOME` does not move Forge's own config (`ProjectDirs("Forge")/config.json`, holding the
base URL, API key, model and the enabled skill, plugin and MCP lists), so a harness run would read and may
overwrite the user's real settings. Add `FORGE_CONFIG_PATH` to `build_services` in `crates/forge-app/src/main.rs`, as a pure
function shaped like `claude_paths`: an unset or empty value falls back to `default_config_path()`.
Built test-first, with the same cases as `claude_paths` (no override, empty override, override).

### 2. `scripts/harness/` (Python 3 standard library plus the installed Quartz module)

- `build_fixture.py <dir>` writes the same content every time into a fresh `<dir>`:
  - `<dir>/.claude/` and `<dir>/.claude.json`
  - `<dir>/forge-config/config.json` with no base URL and no API key
  - real project directories under `/private/tmp/forge-harness/`
  - scenarios: two folders colliding in one Claude directory (`my_app` gone, `my-app` present), two
    same-named folders (`a/app`, `b/app`), titles (`ai-title` and `custom-title` lines) and real
    mtimes set with `os.utime`.
  - transcript `cwd` values are the resolved `/private/tmp/...` form, because the project list drops a
    directory whose escaped name does not round-trip from the cwd.
- `launch.sh` builds `sidecar/dist` (`npm ci && npm run build`) if missing, builds the fixture, then
  runs `cargo run -p forge-app` in the background with `FORGE_CLAUDE_HOME` and `FORGE_CONFIG_PATH`
  set, and records the pid.
- `capture.py [out.png]` finds the window (`Quartz.CGWindowListCopyWindowInfo`, owner `forge-app`, name
  `Forge`), brings the app to the front, runs `screencapture -l<id> -x`, retries once on a blank first
  frame, and prints the window bounds and image size.
- `click.py <x> <y>` takes a pixel in the captured image, looks up the window bounds itself, converts
  by the ratio of window size to image size (no hard-coded scale or shadow constants) and posts
  `Quartz.CGEventPost` mouse events. System Events `click` fails with -25208 and is not used. The
  window may sit on a display with a negative Y.
- `stop.sh` quits the app and removes the fixture.
- `README.md`: one screen of usage, the Screen Recording and Accessibility permission requirement for
  the terminal, and the out-of-scope list.

### 3. Verification

- `build_fixture.py` is tested with `python3 -m unittest` (directory layout, resolved cwds, titles,
  mtimes).
- Acceptance is one end-to-end run: launch, capture, click, capture. The screenshots show `my-app`
  and the two `app` headings told apart by parent, and a click opens a session.
- The `FORGE_CONFIG_PATH` function has Rust unit tests; `cargo test` stays green.

## Risks

- Screen capture and Quartz clicks depend on permissions and on the window being visible; a blank
  first frame is normal and handled by one retry.
- Quartz (pyobjc) is available to the `python3` on this machine's PATH (anaconda 3.11.7) but not to
  `/usr/bin/python3`; the README states the dependency.
