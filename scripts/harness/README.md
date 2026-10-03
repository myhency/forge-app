# UI test harness

Runs the real Forge window against a fake Claude home so its UI can be checked without hand testing.

    scripts/harness/launch.sh            # build, fixture, start the app in the background
    python3 scripts/harness/capture.py   # screenshot -> /private/tmp/forge-harness-shot.png (open it to look)
    python3 scripts/harness/click.py X Y # click pixel X,Y of that screenshot, then capture again
    scripts/harness/stop.sh              # quit the app, remove the fixture
    python3 -m unittest discover -s scripts/harness   # tests of the fixture builder and the click math

The app runs with `FORGE_CLAUDE_HOME` and `FORGE_CONFIG_PATH` pointing into `/private/tmp/forge-harness-home`,
so the real `~/.claude` and the real Forge config are never read or written. The fixture holds
`my-app` and the missing `my_app` (one Claude directory, two headings), and `a/app` and `b/app`
(headings `app (a)` and `app (b)`).

`click.py` takes pixels of the PNG file. An image viewer may show it scaled down (the viewer says by how
much); multiply the coordinates you read off by that factor.

Needs the terminal to have Screen Recording and Accessibility permission. A session started from the
app still runs on the real Claude, and a fake home has no login, so sending a message, IME, dark mode
and other perception checks are not covered. A blank first capture is retried once.
