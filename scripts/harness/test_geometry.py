import struct
import tempfile
import unittest
from pathlib import Path

from geometry import png_size, resize_script, select_window, to_screen_point


def write_png_header(path, width, height):
    # 8-byte signature, then the IHDR chunk: length, type, width, height.
    Path(path).write_bytes(
        b"\x89PNG\r\n\x1a\n" + struct.pack(">I", 13) + b"IHDR" + struct.pack(">II", width, height) + b"\x00" * 5
    )


class GeometryTest(unittest.TestCase):
    def test_png_size_reads_the_header(self):
        with tempfile.TemporaryDirectory() as tmp:
            png = Path(tmp) / "a.png"
            write_png_header(png, 2400, 1600)
            self.assertEqual(png_size(png), (2400, 1600))

    def test_png_size_rejects_a_file_that_is_not_a_png(self):
        with tempfile.TemporaryDirectory() as tmp:
            bad = Path(tmp) / "a.png"
            bad.write_bytes(b"not a png at all, not a png at all")
            with self.assertRaises(ValueError):
                png_size(bad)

    def test_a_retina_pixel_maps_to_window_points(self):
        # A 1200x800 point window captured at 2x is 2400x1600 pixels.
        self.assertEqual(to_screen_point(240, 160, (2400, 1600), (100, 50, 1200, 800)), (220, 130))

    def test_a_window_on_a_display_above_the_main_one_keeps_its_negative_y(self):
        self.assertEqual(to_screen_point(0, 0, (2400, 1600), (0, -900, 1200, 800)), (0, -900))
        x, y = to_screen_point(2399, 1599, (2400, 1600), (0, -900, 1200, 800))
        self.assertAlmostEqual(x, 1199.5)
        self.assertAlmostEqual(y, -900 + 1599 * 800 / 1600)
        self.assertLess(y, 0)

    def test_a_pixel_outside_the_image_is_an_error(self):
        for point in [(-1, 0), (0, -1), (2400, 0), (0, 1600)]:
            with self.assertRaises(ValueError):
                to_screen_point(*point, (2400, 1600), (0, 0, 1200, 800))


def listed(pid, owner="forge-app", name="Forge", number=1):
    return {
        "kCGWindowOwnerPID": pid,
        "kCGWindowOwnerName": owner,
        "kCGWindowName": name,
        "kCGWindowNumber": number,
        "kCGWindowBounds": {"X": 10.0, "Y": -20.0, "Width": 1200.0, "Height": 800.0},
    }


class SelectWindowTest(unittest.TestCase):
    def test_picks_the_window_of_the_harness_process(self):
        windows = [listed(111, number=1), listed(222, number=2)]
        self.assertEqual(select_window(windows, 222), {"id": 2, "bounds": (10.0, -20.0, 1200.0, 800.0)})

    def test_ignores_another_forge_window_that_is_not_the_harness_process(self):
        # A dev build the user has open has the same owner and title, but another pid.
        self.assertIsNone(select_window([listed(111)], 222))

    def test_without_a_harness_pid_there_is_no_window(self):
        self.assertIsNone(select_window([listed(111)], None))

    def test_ignores_windows_of_other_owners_or_titles(self):
        windows = [listed(222, owner="Safari"), listed(222, name="Settings")]
        self.assertIsNone(select_window(windows, 222))


class ResizeScriptTest(unittest.TestCase):
    def test_it_sets_the_size_of_the_window_of_that_process_only(self):
        script = resize_script(4242, 900, 800)
        self.assertIn("unix id is 4242", script)
        self.assertIn("set size of window 1", script)
        self.assertIn("{900, 800}", script)


if __name__ == "__main__":
    unittest.main()
