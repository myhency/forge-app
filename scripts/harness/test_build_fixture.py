import json
import os
import shutil
import tempfile
import unittest
from pathlib import Path

import build_fixture as bf


def read_transcripts(claude_home):
    """{claude project dir name: [(cwd, lines, mtime)]}"""
    found = {}
    for project_dir in sorted((claude_home / "projects").iterdir()):
        entries = []
        for transcript in sorted(project_dir.glob("*.jsonl")):
            lines = [json.loads(line) for line in transcript.read_text().splitlines()]
            entries.append((lines[0]["cwd"], lines, transcript.stat().st_mtime))
        found[project_dir.name] = entries
    return found


class BuildFixtureTest(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(os.path.realpath(tempfile.mkdtemp()))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)
        self.root = self.tmp / "home"
        self.projects = self.tmp / "projects"
        self.result = bf.build(self.root, self.projects)
        self.claude_home = self.result["claude_home"]

    def test_layout(self):
        self.assertEqual(self.claude_home, self.root / ".claude")
        self.assertEqual(self.result["config_path"], self.root / "forge-config" / "config.json")
        self.assertTrue((self.root / ".claude.json").is_file())

    def test_config_has_no_credentials(self):
        config = json.loads(self.result["config_path"].read_text())
        self.assertIsNone(config["base_url"])
        self.assertIsNone(config["api_key"])

    def test_my_app_and_my_underscore_app_share_one_claude_directory(self):
        transcripts = read_transcripts(self.claude_home)
        shared = bf.escape_project_path(self.projects / "my-app")
        self.assertEqual(shared, bf.escape_project_path(self.projects / "my_app"))
        cwds = {cwd for cwd, _, _ in transcripts[shared]}
        self.assertEqual(cwds, {str(self.projects / "my-app"), str(self.projects / "my_app")})
        self.assertTrue((self.projects / "my-app").is_dir())
        self.assertFalse((self.projects / "my_app").exists())

    def test_two_folders_named_app_have_their_own_directories(self):
        transcripts = read_transcripts(self.claude_home)
        a = bf.escape_project_path(self.projects / "a" / "app")
        b = bf.escape_project_path(self.projects / "b" / "app")
        self.assertNotEqual(a, b)
        self.assertEqual([cwd for cwd, _, _ in transcripts[a]], [str(self.projects / "a" / "app")])
        self.assertEqual([cwd for cwd, _, _ in transcripts[b]], [str(self.projects / "b" / "app")])
        self.assertTrue((self.projects / "a" / "app").is_dir())
        self.assertTrue((self.projects / "b" / "app").is_dir())

    def test_existing_cwds_are_already_resolved(self):
        for entries in read_transcripts(self.claude_home).values():
            for cwd, _, _ in entries:
                if Path(cwd).exists():
                    self.assertEqual(cwd, os.path.realpath(cwd))

    def test_every_session_has_a_title_and_a_fixed_mtime(self):
        for entries in read_transcripts(self.claude_home).values():
            for _, lines, mtime in entries:
                kinds = {line["type"] for line in lines}
                self.assertTrue(kinds & {"custom-title", "ai-title"}, lines)
                self.assertEqual(mtime, int(mtime))
                self.assertLess(mtime, 1_800_000_000)
                self.assertGreater(mtime, 1_700_000_000)

    def test_sessions_have_different_mtimes_so_the_order_is_known(self):
        mtimes = [m for entries in read_transcripts(self.claude_home).values() for _, _, m in entries]
        self.assertEqual(len(mtimes), len(set(mtimes)))

    def test_building_again_replaces_the_previous_fixture(self):
        (self.claude_home / "stray.txt").write_text("x")
        bf.build(self.root, self.projects)
        self.assertFalse((self.claude_home / "stray.txt").exists())
        self.assertEqual(len(read_transcripts(self.claude_home)), 3)

    def test_refuses_a_directory_that_is_not_a_harness_directory(self):
        precious = self.tmp / "precious"
        precious.mkdir()
        (precious / "keep.txt").write_text("x")
        with self.assertRaises(SystemExit):
            bf.build(precious, self.projects)
        self.assertTrue((precious / "keep.txt").exists())

    def test_refuses_a_projects_root_that_is_not_a_harness_directory(self):
        precious = self.tmp / "precious"
        precious.mkdir()
        (precious / "keep.txt").write_text("x")
        with self.assertRaises(SystemExit):
            bf.build(self.tmp / "home2", precious)
        self.assertTrue((precious / "keep.txt").exists())


if __name__ == "__main__":
    unittest.main()
