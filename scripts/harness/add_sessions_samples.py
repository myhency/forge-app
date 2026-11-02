#!/usr/bin/env python3
"""Adds to the harness fixture what the Sessions page needs: the worktree session of add_worktree_samples.py and one
archived session ("Add a dark theme", session 2 of `my-app`).

Run it through launch.sh:  FORGE_HARNESS_EXTRA=scripts/harness/add_sessions_samples.py scripts/harness/launch.sh
"""
import json
import os
from pathlib import Path

import add_worktree_samples as worktree
import build_fixture as bf

CONFIG = Path("/private/tmp/forge-harness-home/forge-config/config.json")
ARCHIVED_ID = "00000000-0000-4000-8000-000000000002"


def add(projects_root=bf.DEFAULT_PROJECTS_ROOT, config=CONFIG):
    worktree.add(projects_root=projects_root, config=config)
    repo = Path(os.path.realpath(projects_root)) / "my-app"
    data = json.loads(config.read_text())
    data["archived"] = [{"project": str(repo), "session_id": ARCHIVED_ID}]
    config.write_text(json.dumps(data, indent=2))


if __name__ == "__main__":
    add()
