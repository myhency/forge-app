#!/usr/bin/env python3
"""Adds a session "Long message" to the harness fixture: a first message of twenty lines, so the folded user bubble
(its fade and 더 보기) can be checked.

Run it through launch.sh:  FORGE_HARNESS_EXTRA=scripts/harness/add_long_message_samples.py scripts/harness/launch.sh
"""
import json
import os
from pathlib import Path

import build_fixture as bf

SESSION_ID = "00000000-0000-4000-8000-000000000801"


def add(claude_home, projects_root):
    cwd = Path(projects_root) / "my-app"
    cwd.mkdir(parents=True, exist_ok=True)
    long = "Long message: please check each of these points.\n" + "\n".join(f"{n}. Point number {n} needs a look." for n in range(1, 21))
    transcript = Path(claude_home) / "projects" / bf.escape_project_path(cwd) / f"{SESSION_ID}.jsonl"
    lines = [
        {"type": "user", "cwd": str(cwd), "sessionId": SESSION_ID, "message": {"role": "user", "content": long}},
        {"type": "assistant", "sessionId": SESSION_ID, "message": {"role": "assistant", "content": [{"type": "text", "text": "All twenty points look fine."}]}},
    ]
    transcript.write_text("".join(json.dumps(line) + "\n" for line in lines))
    mtime = bf._epoch(30, 23)
    os.utime(transcript, (mtime, mtime))


if __name__ == "__main__":
    run_dir = Path(os.environ.get("FORGE_HARNESS_DIR", "/private/tmp/forge-harness-home"))
    add(run_dir / ".claude", Path(os.path.realpath(bf.DEFAULT_PROJECTS_ROOT)))
