import json
import os
import tempfile
import unittest
from pathlib import Path

import add_market_fixture as market


class AddMarketFixtureTest(unittest.TestCase):
    def setUp(self):
        self.dir = Path(os.path.realpath(tempfile.mkdtemp())) / "market"
        market.add(self.dir)

    def test_the_marketplace_file_names_two_plugins(self):
        data = json.loads((self.dir / ".claude-plugin" / "marketplace.json").read_text())
        self.assertEqual(data["name"], "harness-market")
        self.assertEqual([p["name"] for p in data["plugins"]], ["hello", "second"])

    def test_each_plugin_has_a_manifest_and_a_skill(self):
        for name in ("hello", "second"):
            plugin = self.dir / "plugins" / name
            self.assertEqual(json.loads((plugin / ".claude-plugin" / "plugin.json").read_text())["name"], name)
            self.assertTrue((plugin / "skills" / "greet" / "SKILL.md").is_file())

    def test_the_upload_fixture_has_a_skill_folder_and_a_zip(self):
        import zipfile

        base = Path(os.path.realpath(tempfile.mkdtemp())) / "uploads"
        market.add_uploads(base)
        folder = base / "upload-me"
        self.assertIn("name: upload-me", (folder / "SKILL.md").read_text())
        self.assertTrue((folder / "notes" / "extra.md").is_file())
        with zipfile.ZipFile(base / "zipped.zip") as archive:
            self.assertIn("zipped/SKILL.md", archive.namelist())
            self.assertIn("name: zipped", archive.read("zipped/SKILL.md").decode())


if __name__ == "__main__":
    unittest.main()
