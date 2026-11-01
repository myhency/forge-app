#!/usr/bin/env python3
"""Sets the harness config's theme to dark (or light, or system: `FORGE_HARNESS_THEME=light`), so a screen can be looked at in dark mode.

Run it through launch.sh:  FORGE_HARNESS_EXTRA=scripts/harness/add_dark_theme.py scripts/harness/launch.sh
Several samples at once: the extra script of launch.sh is one file; run this one first by hand (the fixture is built before),
or call it from the sample script of your choice.
"""
import json
import os
from pathlib import Path

CONFIG = Path(os.environ.get("FORGE_CONFIG_PATH", "/private/tmp/forge-harness-home/forge-config/config.json"))


def set_theme(theme="dark", config=CONFIG):
    config.parent.mkdir(parents=True, exist_ok=True)
    data = json.loads(config.read_text()) if config.exists() else {"base_url": None, "api_key": None}
    data.setdefault("appearance", {})["theme"] = theme
    config.write_text(json.dumps(data, indent=2))


if __name__ == "__main__":
    set_theme(os.environ.get("FORGE_HARNESS_THEME", "dark"))
