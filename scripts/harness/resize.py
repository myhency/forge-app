#!/usr/bin/env python3
"""resize.py <width> <height>: set the size of the Forge window in points, then print its new bounds."""
import subprocess
import sys
import time

from geometry import resize_script
from window import find_window, harness_pid


def main():
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    pid = harness_pid()
    if pid is None:
        raise SystemExit("the harness app is not running (launch.sh)")
    subprocess.run(["osascript", "-e", resize_script(pid, sys.argv[1], sys.argv[2])], check=True)
    time.sleep(1.0)
    window = find_window()
    print(f"window bounds in points (x, y, w, h): {window['bounds']}" if window else "no Forge window")


if __name__ == "__main__":
    main()
