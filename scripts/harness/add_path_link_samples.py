#!/usr/bin/env python3
"""Adds a session "Path links" to the harness fixture whose reply names, by absolute path, a folder with an extension
(`Todo.app`, like a macOS bundle), a file that does not exist, an empty file, and a real text file, so each kind of link can be pressed.

Run it through launch.sh:  FORGE_HARNESS_EXTRA=scripts/harness/add_path_link_samples.py scripts/harness/launch.sh
"""
import json
import os
from pathlib import Path

import build_fixture as bf

SESSION_ID = "00000000-0000-4000-8000-000000000501"


def add(claude_home, projects_root):
    cwd = Path(projects_root) / "my-app"
    cwd.mkdir(parents=True, exist_ok=True)
    bundle = cwd / "dist" / "Todo.app"
    (bundle / "Contents").mkdir(parents=True, exist_ok=True)
    (bundle / "Contents" / "Info.plist").write_text("<plist/>\n")
    (cwd / "empty.txt").write_text("")
    (cwd / "notes.txt").write_text("a real file\n")
    paths = [bundle, cwd / "does-not-exist.txt", cwd / "empty.txt", cwd / "notes.txt"]
    text = "Made these:\n\n" + "\n\n".join(f"`{path}`" for path in paths)
    transcript = Path(claude_home) / "projects" / bf.escape_project_path(cwd) / f"{SESSION_ID}.jsonl"
    lines = [
        {"type": "user", "cwd": str(cwd), "sessionId": SESSION_ID, "message": {"role": "user", "content": "Path links: what did you make?"}},
        {"type": "assistant", "sessionId": SESSION_ID, "message": {"role": "assistant", "content": [{"type": "text", "text": text}]}},
    ]
    transcript.write_text("".join(json.dumps(line) + "\n" for line in lines))
    mtime = bf._epoch(30, 20)
    os.utime(transcript, (mtime, mtime))


if __name__ == "__main__":
    run_dir = Path(os.environ.get("FORGE_HARNESS_DIR", "/private/tmp/forge-harness-home"))
    add(run_dir / ".claude", Path(os.path.realpath(bf.DEFAULT_PROJECTS_ROOT)))
