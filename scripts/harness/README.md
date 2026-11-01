# UI test harness

Runs the real Forge window against a fake Claude home so its UI can be checked without hand testing.

    scripts/harness/launch.sh            # build, fixture, start the app in the background
    python3 scripts/harness/capture.py   # screenshot -> /private/tmp/forge-harness-shot.png (open it to look)
    python3 scripts/harness/click.py X Y [--option] # click pixel X,Y of that screenshot (--option holds ⌥), then capture again
    python3 scripts/harness/resize.py W H  # set the window size in points (needs Accessibility), then capture
    scripts/harness/stop.sh              # quit the app, remove the fixture
    python3 -m unittest discover -s scripts/harness   # tests of the fixture builder and the click math

`launch.sh` also builds the sidecar when `sidecar/dist` is missing or older than a file in `sidecar/src`; an old build lacks new events, so a card waits on a grey sentence forever.

The app runs with `FORGE_CLAUDE_HOME` and `FORGE_CONFIG_PATH` pointing into `/private/tmp/forge-harness-home`,
so the real `~/.claude` and the real Forge config are never read or written. The fixture holds
`my-app` and the missing `my_app` (one Claude directory, two headings), and `a/app` and `b/app`
(headings `app (a)` and `app (b)`).

`FORGE_HARNESS_EXTRA=scripts/harness/add_preview_samples.py scripts/harness/launch.sh` also writes preview samples (csv, html, svg, png, pdf) into `my-app/samples/` and a reply in the session "Read the config" that names five of them by absolute path. The html sample shows "script has not run" because scripts are off in the html frame.

`FORGE_HARNESS_EXTRA=scripts/harness/add_office_samples.py scripts/harness/launch.sh` writes office samples into `my-app/samples/`: an xlsx with two sheets and a formula never calculated, a 700-row sheet, a fake docx, and an xlsx outside the folder, and a reply that names them by absolute path. It also writes a real `report.docx` and `deck.pptx` (python-docx, python-pptx) into `my-app/office/` for the docx and pptx previews.

`click.py` takes pixels of the PNG file. An image viewer may show it scaled down (the viewer says by how
much); multiply the coordinates you read off by that factor.

Needs a `python3` that can `import Quartz` (pyobjc; the anaconda one here can, `/usr/bin/python3` cannot: otherwise
`pip install pyobjc-framework-Quartz`). The scripts only see the window of the app `launch.sh` started, never another
Forge you have open. The terminal needs Screen Recording and Accessibility permission. A session started from the
app runs on the real Claude with the real login, and the Claude it starts writes its transcript to the
real `~/.claude/projects/` (here `-private-tmp-forge-harness-my-app/`), not to the fake home: `FORGE_CLAUDE_HOME` only
changes what Forge reads. Sending a message therefore costs a real turn and leaves a real file to delete by hand.
Typing: `osascript keystroke` drops letters when a Korean input method is on, and ⌘A / ⌘V did not reach the page;
`key code N` works for single keys. IME and dark mode are not covered. A blank first capture is retried once.

## Reading and pressing through the accessibility tree (agent-desktop)

    npm install -g agent-desktop         # once; macOS asks for Accessibility (agent-desktop permissions --guide)
    scripts/harness/ad.sh snapshot       # JSON tree of buttons, fields, links with refs (@sXXXX:eN) and Korean labels
    scripts/harness/ad.sh click @sXXXX:eN  # headless press: no cursor, no focus change, works with the window off-screen
    scripts/harness/ad.sh type --headed @sXXXX:eN "한글 text"   # typing needs --headed (see below); other commands are passed through

`ad.sh` only addresses the window of the app `launch.sh` started. Refs belong to the snapshot that made them: snapshot again after every action
(`STALE_REF` means the page changed). Use it to find and press elements by name instead of by pixel; keep `capture.py` for
looking at layout, colour and dark mode, which the tree cannot show. Snapshots can hold private text, so do not paste them into public logs.

Typing: without `--headed`, `type` and `set-value` report `ACTION_FAILED` and the field stays empty (the webview ignores accessibility value writes).
`type --headed` sends real Unicode key events to the app and the Korean text arrived complete, with a Korean input method active in the terminal's session
(checked with "안녕하세요 한글 입력 테스트" in the composer; nothing is sent until Enter). It may move focus, so snapshot again afterwards.

Keys: `ad.sh press return` (also `escape`, `cmd+k`, ...) goes to the harness window by `--window-id` (checked: ⌘K search palette, typed "login", `press return` opened "Fix the login bug").
Do not press Enter in the composer unless you want a real turn: it starts the real Claude (see above).
