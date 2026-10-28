#!/usr/bin/env python3
"""Adds forks to the harness fixture, so the nested sidebar rows can be looked at: two forks of "Fix the login bug"
(session 1) and a fork of the first one, all in `my-app`.

Run it through launch.sh:  FORGE_HARNESS_EXTRA=scripts/harness/add_fork_samples.py scripts/harness/launch.sh
"""
import json
import os
from pathlib import Path

import build_fixture as bf

SOURCE = "00000000-0000-4000-8000-000000000001"
# (id, title, forked from, hour on day 30)
FORKS = [
    ("00000000-0000-4000-8000-000000000101", "Fix the login bug (fork)", SOURCE, 13),
    ("00000000-0000-4000-8000-000000000102", "Login bug, other idea (fork)", SOURCE, 14),
    ("00000000-0000-4000-8000-000000000103", "Fix the login bug (fork) (fork)", "00000000-0000-4000-8000-000000000101", 16),
]


def add(projects_root=bf.DEFAULT_PROJECTS_ROOT, claude_home=Path("/private/tmp/forge-harness-home/.claude")):
    cwd = Path(os.path.realpath(projects_root)) / "my-app"
    project_dir = Path(claude_home) / "projects" / bf.escape_project_path(cwd)
    for session_id, title, source, hour in FORKS:
        lines = [
            {"type": "user", "cwd": str(cwd), "sessionId": session_id, "forkedFrom": {"sessionId": source},
             "message": {"role": "user", "content": "Why does login fail?"}},
            {"type": "custom-title", "customTitle": title, "sessionId": session_id,
             "timestamp": f"2026-09-30T{hour:02d}:00:00.000Z", "forkedFrom": {"sessionId": source}},
        ]
        transcript = project_dir / f"{session_id}.jsonl"
        transcript.write_text("".join(json.dumps(line) + "\n" for line in lines))
        mtime = bf._epoch(30, hour)
        os.utime(transcript, (mtime, mtime))


def fold(config_path=Path("/private/tmp/forge-harness-home/forge-config/config.json")):
    """Folds the forks of the first fork and of the source (FORGE_HARNESS_FOLD=1), to look at the `▸` rows."""
    config = json.loads(config_path.read_text())
    config["sidebar"] = {**config.get("sidebar", {}), "collapsed": [f"fork:{FORKS[0][0]}"]}
    config_path.write_text(json.dumps(config))


if __name__ == "__main__":
    add()
    if os.environ.get("FORGE_HARNESS_FOLD"):
        fold()
