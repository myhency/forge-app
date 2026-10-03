#!/usr/bin/env python3
"""Capture and click the installed Claude desktop app, to look at it before copying a feature.

  claude_app.py shot OUT.png
  claude_app.py click X Y [--shown-scale 1.2]

X and Y are pixels of the image as the viewer showed it; pass the factor the viewer
printed (for example 1.20) as --shown-scale. Needs the Quartz module and Screen Recording
and Accessibility permission for the terminal. This drives the user's real app and account:
open menus and pages, never send a message or choose a destructive item.
"""
import argparse
import subprocess
import sys
import time

import Quartz

OWNER = "Claude"
APP = "Claude"
MIN_WIDTH = 300
# A Retina capture has 2 pixels per point.
PIXELS_PER_POINT = 2.0


def find_window():
    windows = Quartz.CGWindowListCopyWindowInfo(
        Quartz.kCGWindowListOptionOnScreenOnly | Quartz.kCGWindowListExcludeDesktopElements,
        Quartz.kCGNullWindowID,
    )
    candidates = [
        w for w in windows
        if w.get("kCGWindowOwnerName") == OWNER
        and w.get("kCGWindowLayer") == 0
        and w["kCGWindowBounds"]["Width"] > MIN_WIDTH
    ]
    if not candidates:
        raise SystemExit(f"no {OWNER} window on screen; open the app first (open -a {APP})")
    return max(candidates, key=lambda w: w["kCGWindowBounds"]["Width"] * w["kCGWindowBounds"]["Height"])


def bring_to_front():
    subprocess.run(["osascript", "-e", f'tell application "{APP}" to activate'], check=False, capture_output=True)
    time.sleep(0.6)


def shot(out):
    bring_to_front()
    window = find_window()
    subprocess.run(["screencapture", "-o", "-x", f"-l{window['kCGWindowNumber']}", out], check=True)
    b = window["kCGWindowBounds"]
    print(f"image: {out}")
    print(f"window bounds in points (x, y, w, h): {b['X']}, {b['Y']}, {b['Width']}, {b['Height']}")


def click(x, y, shown_scale):
    bring_to_front()
    b = find_window()["kCGWindowBounds"]
    point = (b["X"] + x * shown_scale / PIXELS_PER_POINT, b["Y"] + y * shown_scale / PIXELS_PER_POINT)
    if not (b["X"] <= point[0] <= b["X"] + b["Width"] and b["Y"] <= point[1] <= b["Y"] + b["Height"]):
        raise SystemExit(f"({x}, {y}) is outside the {OWNER} window; nothing clicked")
    for kind in (Quartz.kCGEventLeftMouseDown, Quartz.kCGEventLeftMouseUp):
        event = Quartz.CGEventCreateMouseEvent(None, kind, point, Quartz.kCGMouseButtonLeft)
        Quartz.CGEventPost(Quartz.kCGHIDEventTap, event)
        time.sleep(0.05)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    shot_parser = sub.add_parser("shot")
    shot_parser.add_argument("out")
    click_parser = sub.add_parser("click")
    click_parser.add_argument("x", type=float)
    click_parser.add_argument("y", type=float)
    click_parser.add_argument("--shown-scale", type=float, default=1.2)
    args = parser.parse_args()
    if args.command == "shot":
        shot(args.out)
    else:
        click(args.x, args.y, args.shown_scale)


if __name__ == "__main__":
    sys.exit(main())
