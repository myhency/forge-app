#!/usr/bin/env python3
"""Saves five environments into the harness config, so the card above the new-session box and the environment
menu can be looked at: three of them fill the card, the others wait under `더 보기`; `gone` points at a folder that
does not exist.

Run it through launch.sh:  FORGE_HARNESS_EXTRA=scripts/harness/add_environment_samples.py scripts/harness/launch.sh
"""
import json
import os
from pathlib import Path

import build_fixture as bf

CONFIG = Path("/private/tmp/forge-harness-home/forge-config/config.json")
# (id, name, folder, pinned, last used)
ENVIRONMENTS = [
    ("env-1", "my-app", "my-app", True, 400),
    ("env-2", "app (a)", "a/app", False, 300),
    ("env-3", "app (b)", "b/app", False, 200),
    ("env-4", "scratch", "my-app", False, 100),
    # Its folder does not exist: the new-session screen says so above the box.
    ("env-5", "gone", "gone-folder", False, 50),
]


def add(projects_root=bf.DEFAULT_PROJECTS_ROOT, config=CONFIG):
    root = Path(os.path.realpath(projects_root))
    config.parent.mkdir(parents=True, exist_ok=True)
    data = json.loads(config.read_text()) if config.exists() else {"base_url": None, "api_key": None}
    data["environments"] = [
        {"id": id, "name": name, "folders": [str(root / folder)], "pinned": pinned, "created_at": used, "last_used_at": used}
        for id, name, folder, pinned, used in ENVIRONMENTS
    ]
    config.write_text(json.dumps(data, indent=2))


if __name__ == "__main__":
    add()
