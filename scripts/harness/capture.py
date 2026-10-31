#!/usr/bin/env python3
"""capture.py [out.png]: screenshot the Forge window (no shadow) and print where and how big it is."""
import subprocess
import sys
import time
from pathlib import Path

from geometry import png_size
from window import SHOT_PATH, bring_to_front, find_window

WAIT_SECONDS = 30
# The first frame after launch can be blank, which compresses to almost nothing.
MIN_PNG_BYTES = 20_000


def wait_for_window():
    deadline = time.monotonic() + WAIT_SECONDS
    while time.monotonic() < deadline:
        window = find_window()
        if window:
            return window
        time.sleep(0.5)
    raise SystemExit(f"no Forge window after {WAIT_SECONDS}s; is the app running (launch.sh)?")


def shoot(window_id, out):
    subprocess.run(["screencapture", "-o", "-x", f"-l{window_id}", out], check=True)


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else SHOT_PATH
    window = wait_for_window()
    bring_to_front()
    time.sleep(0.5)
    shoot(window["id"], out)
    if Path(out).stat().st_size < MIN_PNG_BYTES:
        time.sleep(1.0)
        window = wait_for_window()
        shoot(window["id"], out)
    print(f"image: {out}")
    print(f"window bounds (x, y, w, h): {window['bounds']}")
    print(f"image size (w, h): {png_size(out)}")


if __name__ == "__main__":
    main()
