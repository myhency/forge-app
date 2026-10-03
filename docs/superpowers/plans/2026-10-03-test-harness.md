# Plan 13: UI Test Harness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Claude can launch the Forge desktop app against isolated, known data, capture the window and click through it.

**Architecture:** One small Rust change (`FORGE_CONFIG_PATH`, beside the existing `FORGE_CLAUDE_HOME`) isolates Forge's own config. Everything else is scripts under `scripts/harness/`: a Python fixture builder, shell launch/stop scripts, and Python capture/click scripts that use the macOS Quartz module. Pure logic (fixture content, coordinate math) is unit-tested; the screen-driving parts are verified by one end-to-end run.

**Tech Stack:** Rust (`crates/forge-app`), Python 3.11 standard library + `Quartz` (pyobjc, already installed for the system `python3`), bash, `screencapture`, `osascript`.

**Spec:** `docs/superpowers/specs/2026-10-03-test-harness-design.md`

## Global Constraints

- macOS only; the terminal needs Screen Recording and Accessibility permission (already granted).
- Python scripts use the standard library plus `Quartz` only; no `pip install`.
- Fixture transcript `cwd` values are the resolved `/private/tmp/...` form (the project list drops a directory whose escaped name does not round-trip from the cwd).
- The harness never touches the real `~/.claude` or the real Forge config; the app runs with `FORGE_CLAUDE_HOME` and `FORGE_CONFIG_PATH` both set.
- System Events `click` fails with -25208; clicks are `Quartz.CGEventPost` mouse events.
- Out of scope: scenario runner (B), SSR render tests (C), sending a real message, IME, dark mode.

## Review Focus

- `build_fixture.py` pointed at a directory that is not a harness directory (for example `~`) must refuse and delete nothing. Pinned in Task 2.
- `launch.sh` run twice must refuse the second launch instead of starting a second app or overwriting the pid file. Checked in Task 3.
- `stop.sh` with a stale pid file, a recycled pid, or no app running must not error and must not kill an unrelated process. Checked in Task 3.
- `click.py` given a point outside the captured image must fail without clicking. Pinned in Task 4.
- A window on a display with a negative Y origin must map to a negative screen Y. Pinned in Task 4.
- `capture.py` with no Forge window must exit non-zero with a clear message after the wait, not hang. Checked in Task 4.

---

### Task 1: `FORGE_CONFIG_PATH` in the app

**Files:**
- Modify: `crates/forge-app/src/main.rs` (add `config_path` next to `claude_paths`, ~line 91; use it in `build_services`, ~line 114; tests in the existing `mod tests`)

**Interfaces:**
- Produces: env var `FORGE_CONFIG_PATH` (path to the Forge `config.json`); Task 3 sets it.

- [ ] **Step 1: Write the failing tests**

In `mod tests` of `crates/forge-app/src/main.rs`, after `no_default_and_no_override_is_none`, add:

```rust
    #[test]
    fn with_no_config_override_the_default_config_path_is_used() {
        assert_eq!(
            config_path(None, Some("/home/me/.config/Forge/config.json".into())),
            Some("/home/me/.config/Forge/config.json".into())
        );
    }

    #[test]
    fn an_empty_config_override_counts_as_none() {
        assert_eq!(
            config_path(Some("".into()), Some("/home/me/.config/Forge/config.json".into())),
            Some("/home/me/.config/Forge/config.json".into())
        );
    }

    #[test]
    fn a_config_override_replaces_the_default() {
        assert_eq!(
            config_path(Some("/tmp/fixture/forge-config/config.json".into()), None),
            Some("/tmp/fixture/forge-config/config.json".into())
        );
    }

    #[test]
    fn no_default_and_no_config_override_is_none() {
        assert_eq!(config_path(None, None), None);
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-app config_`
Expected: FAIL to compile with "cannot find function `config_path`".

- [ ] **Step 3: Implement**

In `crates/forge-app/src/main.rs`, directly after the closing brace of `claude_paths` add:

```rust
// Where Forge keeps its own settings (base URL, API key, model, enabled extensions).
// `FORGE_CONFIG_PATH` points it at another file so a test run never reads or overwrites the real one.
fn config_path(
    override_path: Option<std::ffi::OsString>,
    default_path: Option<std::path::PathBuf>,
) -> Option<std::path::PathBuf> {
    match override_path.filter(|path| !path.is_empty()) {
        Some(path) => Some(std::path::PathBuf::from(path)),
        None => default_path,
    }
}
```

In `build_services` replace
`let config_path = default_config_path().expect("could not resolve the config directory");`
with
```rust
    let config_path = config_path(std::env::var_os("FORGE_CONFIG_PATH"), default_config_path())
        .expect("could not resolve the config directory");
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p forge-app`
Expected: PASS (all tests, including the four new ones and the four `claude_paths` ones).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-app/src/main.rs
git commit -m "feat: FORGE_CONFIG_PATH runs the app against another Forge config

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Fixture builder

**Files:**
- Create: `scripts/harness/build_fixture.py`
- Test: `scripts/harness/test_build_fixture.py`

**Interfaces:**
- Produces: `build(root, projects_root=DEFAULT_PROJECTS_ROOT) -> dict` with keys `claude_home`, `config_path`, `projects_root` (all `Path`); CLI `build_fixture.py <dir>`. Layout under `<dir>`: `.claude/`, `.claude.json`, `forge-config/config.json`. Task 3 relies on exactly those three names.

- [ ] **Step 1: Write the failing tests**

Create `scripts/harness/test_build_fixture.py`:

```python
import json
import os
import tempfile
import unittest
from pathlib import Path

import build_fixture as bf


def read_transcripts(claude_home):
    """{claude project dir name: [(cwd, first-line dict, mtime, other lines)]}"""
    found = {}
    for project_dir in sorted((claude_home / "projects").iterdir()):
        entries = []
        for transcript in sorted(project_dir.glob("*.jsonl")):
            lines = [json.loads(line) for line in transcript.read_text().splitlines()]
            entries.append((lines[0]["cwd"], lines, transcript.stat().st_mtime))
        found[project_dir.name] = entries
    return found


class BuildFixtureTest(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(os.path.realpath(tempfile.mkdtemp()))
        self.addCleanup(lambda: __import__("shutil").rmtree(self.tmp, ignore_errors=True))
        self.root = self.tmp / "home"
        self.projects = self.tmp / "projects"
        self.result = bf.build(self.root, self.projects)
        self.claude_home = self.result["claude_home"]

    def test_layout(self):
        self.assertEqual(self.claude_home, self.root / ".claude")
        self.assertEqual(self.result["config_path"], self.root / "forge-config" / "config.json")
        self.assertTrue((self.root / ".claude.json").is_file())

    def test_config_has_no_credentials(self):
        config = json.loads(self.result["config_path"].read_text())
        self.assertIsNone(config["base_url"])
        self.assertIsNone(config["api_key"])

    def test_my_app_and_my_underscore_app_share_one_claude_directory(self):
        transcripts = read_transcripts(self.claude_home)
        shared = bf.escape_project_path(self.projects / "my-app")
        self.assertEqual(shared, bf.escape_project_path(self.projects / "my_app"))
        cwds = {cwd for cwd, _, _ in transcripts[shared]}
        self.assertEqual(cwds, {str(self.projects / "my-app"), str(self.projects / "my_app")})
        self.assertTrue((self.projects / "my-app").is_dir())
        self.assertFalse((self.projects / "my_app").exists())

    def test_two_folders_named_app_have_their_own_directories(self):
        transcripts = read_transcripts(self.claude_home)
        a = bf.escape_project_path(self.projects / "a" / "app")
        b = bf.escape_project_path(self.projects / "b" / "app")
        self.assertNotEqual(a, b)
        self.assertEqual([cwd for cwd, _, _ in transcripts[a]], [str(self.projects / "a" / "app")])
        self.assertEqual([cwd for cwd, _, _ in transcripts[b]], [str(self.projects / "b" / "app")])
        self.assertTrue((self.projects / "a" / "app").is_dir())
        self.assertTrue((self.projects / "b" / "app").is_dir())

    def test_existing_cwds_are_already_resolved(self):
        for entries in read_transcripts(self.claude_home).values():
            for cwd, _, _ in entries:
                if Path(cwd).exists():
                    self.assertEqual(cwd, os.path.realpath(cwd))

    def test_every_session_has_a_title_and_a_fixed_mtime(self):
        for entries in read_transcripts(self.claude_home).values():
            for _, lines, mtime in entries:
                kinds = {line["type"] for line in lines}
                self.assertTrue(kinds & {"custom-title", "ai-title"}, lines)
                self.assertEqual(mtime, int(mtime))
                self.assertLess(mtime, 1_800_000_000)
                self.assertGreater(mtime, 1_700_000_000)

    def test_sessions_have_different_mtimes_so_the_order_is_known(self):
        mtimes = [m for entries in read_transcripts(self.claude_home).values() for _, _, m in entries]
        self.assertEqual(len(mtimes), len(set(mtimes)))

    def test_building_again_replaces_the_previous_fixture(self):
        (self.claude_home / "stray.txt").write_text("x")
        bf.build(self.root, self.projects)
        self.assertFalse((self.claude_home / "stray.txt").exists())
        self.assertEqual(len(read_transcripts(self.claude_home)), 3)

    def test_refuses_a_directory_that_is_not_a_harness_directory(self):
        precious = self.tmp / "precious"
        precious.mkdir()
        (precious / "keep.txt").write_text("x")
        with self.assertRaises(SystemExit):
            bf.build(precious, self.projects)
        self.assertTrue((precious / "keep.txt").exists())

    def test_refuses_a_projects_root_that_is_not_a_harness_directory(self):
        precious = self.tmp / "precious"
        precious.mkdir()
        (precious / "keep.txt").write_text("x")
        with self.assertRaises(SystemExit):
            bf.build(self.tmp / "home2", precious)
        self.assertTrue((precious / "keep.txt").exists())


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd scripts/harness && python3 -m unittest test_build_fixture -v`
Expected: FAIL/ERROR with `ModuleNotFoundError: No module named 'build_fixture'`.

- [ ] **Step 3: Implement**

Create `scripts/harness/build_fixture.py`:

```python
#!/usr/bin/env python3
"""Builds the fake Claude home and Forge config that the harness launches Forge against.

    build_fixture.py <dir>

Writes <dir>/.claude, <dir>/.claude.json and <dir>/forge-config/config.json, plus real project
directories under /private/tmp/forge-harness. The content is the same every run. Both directories
are wiped first, but only if they are empty or carry the .forge-harness marker this script writes.
"""
import json
import os
import re
import shutil
import sys
from datetime import datetime, timezone
from pathlib import Path

MARKER = ".forge-harness"
DEFAULT_PROJECTS_ROOT = Path("/private/tmp/forge-harness")


def escape_project_path(path):
    """Claude's project directory name: every non-ASCII-alphanumeric character becomes '-'."""
    return re.sub(r"[^A-Za-z0-9]", "-", str(path))


def _epoch(day, hour):
    return int(datetime(2026, 9, day, hour, tzinfo=timezone.utc).timestamp())


def _session(number, title_type, title, prompt, day, hour):
    return {
        "id": f"00000000-0000-4000-8000-{number:012d}",
        "title_type": title_type,
        "title": title,
        "prompt": prompt,
        "mtime": _epoch(day, hour),
    }


# (path relative to the projects root, exists on disk, sessions). `my-app` and `my_app` escape to
# the same Claude directory, and `my_app` is gone, which is the collision Plan 12 handled. The two
# `app` folders share a name and differ only by parent.
SCENARIO = [
    ("my-app", True, [
        _session(1, "custom-title", "Fix the login bug", "Why does login fail?", 30, 12),
        _session(2, "ai-title", "Add a dark theme", "Can you add a dark theme?", 28, 9),
    ]),
    ("my_app", False, [
        _session(3, "ai-title", "Old my_app session", "Set up the project", 20, 9),
    ]),
    ("a/app", True, [
        _session(4, "custom-title", "Session in a/app", "Hello from a", 29, 15),
    ]),
    ("b/app", True, [
        _session(5, "ai-title", "Session in b/app", "Hello from b", 27, 11),
    ]),
]


def _fresh_directory(path):
    if path.exists() and any(path.iterdir()) and not (path / MARKER).exists():
        raise SystemExit(f"refusing to wipe {path}: it is not a harness directory (no {MARKER} file)")
    if path.exists():
        shutil.rmtree(path)
    path.mkdir(parents=True)
    (path / MARKER).write_text("")


def _write_session(project_dir, cwd, session):
    transcript = project_dir / f"{session['id']}.jsonl"
    lines = [
        {"type": "user", "cwd": str(cwd), "sessionId": session["id"],
         "message": {"role": "user", "content": session["prompt"]}},
        {"type": session["title_type"],
         ("customTitle" if session["title_type"] == "custom-title" else "aiTitle"): session["title"],
         "sessionId": session["id"]},
    ]
    transcript.write_text("".join(json.dumps(line) + "\n" for line in lines))
    os.utime(transcript, (session["mtime"], session["mtime"]))


def build(root, projects_root=DEFAULT_PROJECTS_ROOT):
    root = Path(root)
    projects_root = Path(os.path.realpath(projects_root))
    _fresh_directory(root)
    _fresh_directory(projects_root)

    claude_home = root / ".claude"
    (claude_home / "projects").mkdir(parents=True)
    (root / ".claude.json").write_text("{}")
    config_path = root / "forge-config" / "config.json"
    config_path.parent.mkdir(parents=True)
    config_path.write_text(json.dumps({"base_url": None, "api_key": None}))

    for relative, exists, sessions in SCENARIO:
        cwd = projects_root / relative
        if exists:
            cwd.mkdir(parents=True)
        project_dir = claude_home / "projects" / escape_project_path(cwd)
        project_dir.mkdir(exist_ok=True)
        for session in sessions:
            _write_session(project_dir, cwd, session)

    return {"claude_home": claude_home, "config_path": config_path, "projects_root": projects_root}


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: build_fixture.py <dir>")
    result = build(sys.argv[1])
    print(f"FORGE_CLAUDE_HOME={result['claude_home']}")
    print(f"FORGE_CONFIG_PATH={result['config_path']}")
```

- [ ] **Step 4: Run to verify they pass**

Run: `cd scripts/harness && python3 -m unittest test_build_fixture -v`
Expected: PASS, 10 tests.

- [ ] **Step 5: Commit**

```bash
git add scripts/harness/build_fixture.py scripts/harness/test_build_fixture.py
git commit -m "feat: harness fixture builder writes a fake Claude home and Forge config

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Launch and stop scripts

**Files:**
- Create: `scripts/harness/launch.sh`, `scripts/harness/stop.sh` (both executable)

**Interfaces:**
- Consumes: `build_fixture.py <dir>` and its layout from Task 2.
- Produces: files `/private/tmp/forge-harness.pid` (the app's pid) and `/private/tmp/forge-harness.log` (its output); default run directory `/private/tmp/forge-harness-home` (override with `FORGE_HARNESS_DIR`). Task 4 expects a window owned by `forge-app` named `Forge`.

- [ ] **Step 1: Write `launch.sh`**

```bash
#!/usr/bin/env bash
# Builds Forge, builds a fresh fixture and starts the app against it in the background.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
run_dir="${FORGE_HARNESS_DIR:-/private/tmp/forge-harness-home}"
pid_file=/private/tmp/forge-harness.pid
log_file=/private/tmp/forge-harness.log

if [[ -f "$pid_file" ]] && kill -0 "$(cat "$pid_file")" 2>/dev/null; then
  echo "the harness app is already running (pid $(cat "$pid_file")); run stop.sh first" >&2
  exit 1
fi

# A fresh worktree has no sidecar build.
if [[ ! -f "$root/sidecar/dist/index.js" ]]; then
  (cd "$root/sidecar" && npm ci && npm run build)
fi
(cd "$root" && cargo build -p forge-app)

python3 "$here/build_fixture.py" "$run_dir" >/dev/null

binary="${CARGO_TARGET_DIR:-$root/target}/debug/forge-app"
FORGE_CLAUDE_HOME="$run_dir/.claude" \
FORGE_CONFIG_PATH="$run_dir/forge-config/config.json" \
  nohup "$binary" >"$log_file" 2>&1 &
echo $! >"$pid_file"
echo "forge-app started (pid $(cat "$pid_file")), log: $log_file; next: capture.py"
```

- [ ] **Step 2: Write `stop.sh`**

```bash
#!/usr/bin/env bash
# Quits the harness app and removes its fixture. Safe to run when nothing is running.
set -uo pipefail

run_dir="${FORGE_HARNESS_DIR:-/private/tmp/forge-harness-home}"
pid_file=/private/tmp/forge-harness.pid

if [[ -f "$pid_file" ]]; then
  pid="$(cat "$pid_file")"
  # Only signal a live process that really is forge-app (the pid may have been recycled).
  if kill -0 "$pid" 2>/dev/null && [[ "$(ps -p "$pid" -o comm= 2>/dev/null)" == *forge-app ]]; then
    kill "$pid"
  fi
  rm -f "$pid_file"
fi

# Only directories this harness created carry the marker.
for dir in "$run_dir" /private/tmp/forge-harness; do
  if [[ -f "$dir/.forge-harness" ]]; then
    rm -rf "$dir"
  fi
done
echo "stopped"
```

- [ ] **Step 3: Make them executable and run the checks**

Run:
```bash
chmod +x scripts/harness/launch.sh scripts/harness/stop.sh
scripts/harness/stop.sh                      # nothing running: prints "stopped", exit 0
echo "99999999" > /private/tmp/forge-harness.pid; scripts/harness/stop.sh; echo "exit $?"   # stale pid: exit 0, pid file gone
scripts/harness/launch.sh                    # builds, starts
scripts/harness/launch.sh; echo "exit $?"    # second launch must refuse, exit 1
ps -p "$(cat /private/tmp/forge-harness.pid)" -o comm=   # ends with forge-app
scripts/harness/stop.sh
ls /private/tmp/forge-harness-home /private/tmp/forge-harness 2>&1 | head   # both gone
```
Expected: matches the comments above. If the first `cargo build` or `npm ci` is slow that is normal.

- [ ] **Step 4: Commit**

```bash
git add scripts/harness/launch.sh scripts/harness/stop.sh
git commit -m "feat: harness launch and stop scripts

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Capture and click scripts

**Files:**
- Create: `scripts/harness/geometry.py`, `scripts/harness/window.py`, `scripts/harness/capture.py`, `scripts/harness/click.py`
- Test: `scripts/harness/test_geometry.py`

**Interfaces:**
- Produces (`geometry.py`, no Quartz import so it is testable anywhere): `png_size(path) -> (width, height)` and `to_screen_point(px, py, image_size, bounds) -> (x, y)` where `bounds = (x, y, width, height)` in screen points; raises `ValueError` if the pixel is outside the image.
- Produces (`window.py`): `SHOT_PATH = "/private/tmp/forge-harness-shot.png"`, `find_window() -> dict | None` with keys `id`, `bounds`, and `bring_to_front()`.
- `capture.py [out.png]` prints the output path, the window bounds and the image size; `click.py <x> <y> [--image PATH]` takes pixels of that image.

- [ ] **Step 1: Write the failing tests**

Create `scripts/harness/test_geometry.py`:

```python
import struct
import tempfile
import unittest
from pathlib import Path

from geometry import png_size, to_screen_point


def write_png_header(path, width, height):
    # 8-byte signature, then the IHDR chunk: length, type, width, height.
    Path(path).write_bytes(
        b"\x89PNG\r\n\x1a\n" + struct.pack(">I", 13) + b"IHDR" + struct.pack(">II", width, height) + b"\x00" * 5
    )


class GeometryTest(unittest.TestCase):
    def test_png_size_reads_the_header(self):
        with tempfile.TemporaryDirectory() as tmp:
            png = Path(tmp) / "a.png"
            write_png_header(png, 2400, 1600)
            self.assertEqual(png_size(png), (2400, 1600))

    def test_png_size_rejects_a_file_that_is_not_a_png(self):
        with tempfile.TemporaryDirectory() as tmp:
            bad = Path(tmp) / "a.png"
            bad.write_bytes(b"not a png at all, not a png at all")
            with self.assertRaises(ValueError):
                png_size(bad)

    def test_a_retina_pixel_maps_to_window_points(self):
        # A 1200x800 point window captured at 2x is 2400x1600 pixels.
        self.assertEqual(to_screen_point(240, 160, (2400, 1600), (100, 50, 1200, 800)), (220, 130))

    def test_a_window_on_a_display_above_the_main_one_keeps_its_negative_y(self):
        self.assertEqual(to_screen_point(0, 0, (2400, 1600), (0, -900, 1200, 800)), (0, -900))
        x, y = to_screen_point(2399, 1599, (2400, 1600), (0, -900, 1200, 800))
        self.assertAlmostEqual(x, 1199.5)
        self.assertAlmostEqual(y, -900 + 1599 * 800 / 1600)
        self.assertLess(y, 0)

    def test_a_pixel_outside_the_image_is_an_error(self):
        for point in [(-1, 0), (0, -1), (2400, 0), (0, 1600)]:
            with self.assertRaises(ValueError):
                to_screen_point(*point, (2400, 1600), (0, 0, 1200, 800))


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd scripts/harness && python3 -m unittest test_geometry -v`
Expected: ERROR `ModuleNotFoundError: No module named 'geometry'`.

- [ ] **Step 3: Implement `geometry.py`**

```python
"""Pure helpers for turning pixels of a window capture into screen points."""
import struct

PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def png_size(path):
    with open(path, "rb") as file:
        header = file.read(24)
    if len(header) < 24 or not header.startswith(PNG_SIGNATURE) or header[12:16] != b"IHDR":
        raise ValueError(f"{path} is not a PNG")
    return struct.unpack(">II", header[16:24])


def to_screen_point(px, py, image_size, bounds):
    """Map a pixel of the capture to a screen point.

    The capture is taken without the window shadow (`screencapture -o`), so the image covers exactly
    `bounds`; the scale is the ratio of image size to window size, whatever the display's density.
    """
    image_width, image_height = image_size
    x, y, width, height = bounds
    if not (0 <= px < image_width and 0 <= py < image_height):
        raise ValueError(f"pixel ({px}, {py}) is outside the {image_width}x{image_height} image")
    return x + px * width / image_width, y + py * height / image_height
```

- [ ] **Step 4: Run to verify they pass**

Run: `cd scripts/harness && python3 -m unittest test_geometry -v`
Expected: PASS, 5 tests.

- [ ] **Step 5: Write `window.py`**

```python
"""Finding and activating the Forge window (macOS, needs the Quartz module)."""
import subprocess

import Quartz

SHOT_PATH = "/private/tmp/forge-harness-shot.png"
OWNER = "forge-app"
TITLE = "Forge"


def find_window():
    """The on-screen Forge window as {'id', 'bounds': (x, y, w, h)}, or None."""
    windows = Quartz.CGWindowListCopyWindowInfo(
        Quartz.kCGWindowListOptionOnScreenOnly | Quartz.kCGWindowListExcludeDesktopElements,
        Quartz.kCGNullWindowID,
    )
    for window in windows or []:
        if window.get("kCGWindowOwnerName") == OWNER and window.get("kCGWindowName") == TITLE:
            b = window["kCGWindowBounds"]
            return {"id": window["kCGWindowNumber"], "bounds": (b["X"], b["Y"], b["Width"], b["Height"])}
    return None


def bring_to_front():
    subprocess.run(
        ["osascript", "-e", f'tell application "System Events" to set frontmost of process "{OWNER}" to true'],
        check=False,
        capture_output=True,
    )
```

- [ ] **Step 6: Write `capture.py`**

```python
#!/usr/bin/env python3
"""capture.py [out.png]: screenshot the Forge window (no shadow) and print where and how big it is."""
import subprocess
import sys
import time
from pathlib import Path

from geometry import png_size
from window import SHOT_PATH, bring_to_front, find_window

WAIT_SECONDS = 30
# The first frame after launch can be blank, which compresses to almost nothing.
MIN_PNG_BYTES = 20_000


def wait_for_window():
    deadline = time.monotonic() + WAIT_SECONDS
    while time.monotonic() < deadline:
        window = find_window()
        if window:
            return window
        time.sleep(0.5)
    raise SystemExit(f"no Forge window after {WAIT_SECONDS}s; is the app running (launch.sh)?")


def shoot(window_id, out):
    subprocess.run(["screencapture", "-o", "-x", f"-l{window_id}", out], check=True)


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else SHOT_PATH
    window = wait_for_window()
    bring_to_front()
    time.sleep(0.5)
    shoot(window["id"], out)
    if Path(out).stat().st_size < MIN_PNG_BYTES:
        time.sleep(1.0)
        window = wait_for_window()
        shoot(window["id"], out)
    print(f"image: {out}")
    print(f"window bounds (x, y, w, h): {window['bounds']}")
    print(f"image size (w, h): {png_size(out)}")


if __name__ == "__main__":
    main()
```

- [ ] **Step 7: Write `click.py`**

```python
#!/usr/bin/env python3
"""click.py <x> <y> [--image PATH]: click the pixel (x, y) of the last capture of the Forge window."""
import argparse
import time

import Quartz

from geometry import png_size, to_screen_point
from window import SHOT_PATH, bring_to_front, find_window


def click_at(x, y):
    for event_type in (Quartz.kCGEventMouseMoved, Quartz.kCGEventLeftMouseDown, Quartz.kCGEventLeftMouseUp):
        event = Quartz.CGEventCreateMouseEvent(None, event_type, (x, y), Quartz.kCGMouseButtonLeft)
        Quartz.CGEventPost(Quartz.kCGHIDEventTap, event)
        time.sleep(0.05)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("x", type=float)
    parser.add_argument("y", type=float)
    parser.add_argument("--image", default=SHOT_PATH)
    args = parser.parse_args()

    window = find_window()
    if not window:
        raise SystemExit("no Forge window; is the app running (launch.sh)?")
    try:
        point = to_screen_point(args.x, args.y, png_size(args.image), window["bounds"])
    except ValueError as error:
        raise SystemExit(f"not clicking: {error}")
    bring_to_front()
    time.sleep(0.3)
    click_at(*point)
    print(f"clicked screen point ({point[0]:.1f}, {point[1]:.1f})")


if __name__ == "__main__":
    main()
```

- [ ] **Step 8: Check the error paths with no app running**

Run:
```bash
cd scripts/harness && chmod +x capture.py click.py
time python3 capture.py; echo "exit $?"
python3 click.py 5000 5000; echo "exit $?"
```
Expected: `capture.py` waits about 30 s then exits non-zero with "no Forge window after 30s"; `click.py` exits non-zero with "no Forge window". (The out-of-image refusal is covered by the unit test; the live check is in Task 5.)

- [ ] **Step 9: Commit**

```bash
git add scripts/harness
git commit -m "feat: harness capture and click scripts

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: README and end-to-end acceptance

**Files:**
- Create: `scripts/harness/README.md`

- [ ] **Step 1: Write the README**

```markdown
# UI test harness

Runs the real Forge window against a fake Claude home so its UI can be checked without hand testing.

    scripts/harness/launch.sh            # build, fixture, start the app in the background
    python3 scripts/harness/capture.py   # screenshot -> /private/tmp/forge-harness-shot.png (open it to look)
    python3 scripts/harness/click.py X Y # click pixel X,Y of that screenshot, then capture again
    scripts/harness/stop.sh              # quit the app, remove the fixture
    python3 -m unittest discover -s scripts/harness   # tests of the fixture builder and the click math

The app runs with `FORGE_CLAUDE_HOME` and `FORGE_CONFIG_PATH` pointing into `/private/tmp/forge-harness-home`,
so the real `~/.claude` and the real Forge config are never read or written. The fixture holds
`my-app` and the missing `my_app` (one Claude directory), and `a/app` and `b/app`.

Needs the terminal to have Screen Recording and Accessibility permission. A session started from the
app still runs on the real Claude, and a fake home has no login, so sending a message, IME, dark mode
and other perception checks are not covered. A blank first capture is retried once.
```

- [ ] **Step 2: Run the whole test suite**

Run: `cargo test -p forge-app && python3 -m unittest discover -s scripts/harness`
Expected: PASS.

- [ ] **Step 3: End-to-end acceptance**

Run `scripts/harness/launch.sh`, then `python3 scripts/harness/capture.py`, then Read the PNG. Expected: the sidebar shows `my-app` (with the sessions "Fix the login bug" and "Add a dark theme"), `my_app`'s session folded into the same folder per Plan 12 behavior, and two `app` headings told apart by parent (`a`, `b`). Click the session title "Session in a/app" with `click.py`, capture again and confirm that session opens. Click a point outside the image (`click.py 99999 5`) and confirm it refuses. Then `scripts/harness/stop.sh`.
If the screenshot is blank, offset by shadow, or the click lands off target, fix the script (do not hard-code a correction) and re-run.

- [ ] **Step 4: Update the memory notes**

Update `plan-13-test-harness.md` (what is done now) and `driving-the-forge-app-visually.md` (use `scripts/harness/` instead of the manual recipe).

- [ ] **Step 5: Commit**

```bash
git add scripts/harness/README.md
git commit -m "docs: harness README; Plan 13 built

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```
