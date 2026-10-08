import json
import os
import tempfile
import unittest
from pathlib import Path

import add_preview_samples as samples
import build_fixture as bf


class AddPreviewSamplesTest(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(os.path.realpath(tempfile.mkdtemp()))
        self.run_dir = self.tmp / "run"
        self.projects = self.tmp / "projects"
        self.result = bf.build(self.run_dir, self.projects)
        samples.add(self.result["claude_home"], self.result["projects_root"])
        self.folder = self.result["projects_root"] / "my-app" / "samples"

    def test_every_sample_file_is_written(self):
        for name in ("data.csv", "big.csv", "data.json", "page.html", "pic.svg", "pic.png", "doc.pdf", "multi.pdf"):
            self.assertTrue((self.folder / name).is_file(), name)

    def test_the_big_csv_is_over_the_row_limit(self):
        self.assertGreater(len((self.folder / "big.csv").read_text().splitlines()), 600)

    def test_the_pdfs_look_like_pdfs_and_the_long_one_has_twelve_pages(self):
        self.assertTrue((self.folder / "doc.pdf").read_bytes().startswith(b"%PDF"))
        self.assertIn(b"/Count 12", (self.folder / "multi.pdf").read_bytes())

    def test_the_reply_names_each_sample_by_its_absolute_path(self):
        project_dir = self.result["claude_home"] / "projects" / bf.escape_project_path(self.result["projects_root"] / "my-app")
        transcript = project_dir / "00000000-0000-4000-8000-000000000006.jsonl"
        last = json.loads(transcript.read_text().splitlines()[-1])
        text = last["message"]["content"][0]["text"]
        for name in ("data.csv", "page.html", "doc.pdf", "pic.png", "pic.svg"):
            self.assertIn(str(self.folder / name), text)


if __name__ == "__main__":
    unittest.main()
