import struct
import tempfile
import unittest
from pathlib import Path

from geometry import png_size, to_screen_point


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


if __name__ == "__main__":
    unittest.main()
