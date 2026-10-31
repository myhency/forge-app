#!/usr/bin/env python3
"""Adds the usage, path-link and environment samples together and sets the theme (`FORGE_HARNESS_THEME`, default `system`), so one
launch shows every screen of the 2026-10-31 round in either mode.

Run it through launch.sh:  FORGE_HARNESS_THEME=dark FORGE_HARNESS_EXTRA=scripts/harness/add_all_samples.py scripts/harness/launch.sh
"""
import os
from pathlib import Path

import add_dark_theme
import add_environment_samples
import add_path_link_samples
import add_usage_samples
import build_fixture as bf

if __name__ == "__main__":
    run_dir = Path(os.environ.get("FORGE_HARNESS_DIR", "/private/tmp/forge-harness-home"))
    root = Path(os.path.realpath(bf.DEFAULT_PROJECTS_ROOT))
    add_usage_samples.add(run_dir / ".claude", root)
    add_path_link_samples.add(run_dir / ".claude", root)
    add_environment_samples.add()
    add_dark_theme.set_theme(os.environ.get("FORGE_HARNESS_THEME", "system"))
