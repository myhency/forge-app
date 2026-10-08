#!/usr/bin/env python3
"""Writes a local plugin marketplace (two plugins) to /private/tmp/forge-harness-market, so the Customize page can
be tried in the harness: in the app, Add marketplace with that folder, then install `hello`.

Run it through launch.sh:  FORGE_HARNESS_EXTRA=scripts/harness/add_market_fixture.py scripts/harness/launch.sh
"""
import json
from pathlib import Path

DEFAULT_DIR = Path("/private/tmp/forge-harness-market")

PLUGINS = [
    ("hello", "1.0.0", "Says hello. The first plugin of the harness marketplace."),
    ("second", "0.2.0", "A second plugin, still not installed."),
]


def add(directory: Path = DEFAULT_DIR) -> Path:
    (directory / ".claude-plugin").mkdir(parents=True, exist_ok=True)
    (directory / ".claude-plugin" / "marketplace.json").write_text(
        json.dumps(
            {
                "name": "harness-market",
                "owner": {"name": "harness"},
                "plugins": [
                    {"name": name, "source": f"./plugins/{name}", "description": description, "version": version}
                    for name, version, description in PLUGINS
                ],
            },
            indent=2,
        )
    )
    for name, version, description in PLUGINS:
        plugin = directory / "plugins" / name
        (plugin / ".claude-plugin").mkdir(parents=True, exist_ok=True)
        (plugin / ".claude-plugin" / "plugin.json").write_text(
            json.dumps({"name": name, "version": version, "description": description}, indent=2)
        )
        skill = plugin / "skills" / "greet"
        skill.mkdir(parents=True, exist_ok=True)
        (skill / "SKILL.md").write_text(f"---\nname: greet\ndescription: Greets from {name}\n---\nSay hello from {name}.\n")
    return directory


if __name__ == "__main__":
    print(f"marketplace written to {add()}")
