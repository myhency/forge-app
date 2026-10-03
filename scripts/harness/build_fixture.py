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
    title_key = "customTitle" if session["title_type"] == "custom-title" else "aiTitle"
    lines = [
        {"type": "user", "cwd": str(cwd), "sessionId": session["id"],
         "message": {"role": "user", "content": session["prompt"]}},
        {"type": session["title_type"], title_key: session["title"], "sessionId": session["id"]},
    ]
    transcript.write_text("".join(json.dumps(line) + "\n" for line in lines))
    os.utime(transcript, (session["mtime"], session["mtime"]))


def build(root, projects_root=DEFAULT_PROJECTS_ROOT):
    root = Path(root)
    projects_root = Path(os.path.realpath(projects_root))
    # Check both before wiping either, so a refusal leaves everything as it was.
    for path in (root, projects_root):
        if path.exists() and any(path.iterdir()) and not (path / MARKER).exists():
            raise SystemExit(f"refusing to wipe {path}: it is not a harness directory (no {MARKER} file)")
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
