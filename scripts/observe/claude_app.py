#!/usr/bin/env python3
"""Capture and click the installed Claude desktop app, to look at it before copying a feature.

  claude_app.py shot OUT.png
  claude_app.py click X Y [--shown-scale 1.2]
  claude_app.py scroll X Y LINES   wheel at the point; positive LINES scroll up (older content)
  claude_app.py type TEXT        paste TEXT at the cursor (through the clipboard, so IME does not matter)
  claude_app.py key NAME [mod..] press return, escape, tab, up, down, left, right, or a letter, with cmd/shift/option/ctrl

X and Y are pixels of the image as the viewer showed it; pass the factor the viewer
printed (for example 1.20) as --shown-scale. Needs the Quartz module and Screen Recording
and Accessibility permission for the terminal. This drives the user's real app and account:
open menus and pages. Never choose a destructive item. Send a message only when the user has
allowed it, and then only in a throwaway folder (round 7 used /private/tmp/forge-observe).
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
    move = Quartz.CGEventCreateMouseEvent(None, Quartz.kCGEventMouseMoved, point, Quartz.kCGMouseButtonLeft)
    Quartz.CGEventPost(Quartz.kCGHIDEventTap, move)
    time.sleep(0.3)
    for kind in (Quartz.kCGEventLeftMouseDown, Quartz.kCGEventLeftMouseUp):
        event = Quartz.CGEventCreateMouseEvent(None, kind, point, Quartz.kCGMouseButtonLeft)
        Quartz.CGEventPost(Quartz.kCGHIDEventTap, event)
        time.sleep(0.05)


def scroll(x, y, lines, shown_scale):
    bring_to_front()
    b = find_window()["kCGWindowBounds"]
    point = (b["X"] + x * shown_scale / PIXELS_PER_POINT, b["Y"] + y * shown_scale / PIXELS_PER_POINT)
    move = Quartz.CGEventCreateMouseEvent(None, Quartz.kCGEventMouseMoved, point, Quartz.kCGMouseButtonLeft)
    Quartz.CGEventPost(Quartz.kCGHIDEventTap, move)
    time.sleep(0.3)
    event = Quartz.CGEventCreateScrollWheelEvent(None, Quartz.kCGScrollEventUnitPixel, 1, int(lines))
    Quartz.CGEventPost(Quartz.kCGHIDEventTap, event)


MODIFIERS = {"cmd": "command", "shift": "shift", "option": "option", "ctrl": "control"}
# Letters go by physical key, so a Korean input method does not turn them into jamo.
LETTER_CODES = {"a": 0, "s": 1, "d": 2, "f": 3, "h": 4, "g": 5, "z": 6, "x": 7, "c": 8, "v": 9, "b": 11,
                "q": 12, "w": 13, "e": 14, "r": 15, "y": 16, "t": 17, "o": 31, "u": 32, "i": 34, "p": 35,
                "l": 37, "j": 38, "k": 40, "n": 45, "m": 46}
KEY_CODES = {"return": 36, "tab": 48, "escape": 53, "left": 123, "right": 124, "down": 125, "up": 126}


def type_text(text):
    bring_to_front()
    subprocess.run(["pbcopy"], input=text.encode(), check=True)
    press("v", ["cmd"])


def press(name, mods):
    bring_to_front()
    using = ", ".join(f"{MODIFIERS[m]} down" for m in mods)
    clause = f" using {{{using}}}" if mods else ""
    if name in KEY_CODES or name in LETTER_CODES:
        action = f"key code {KEY_CODES.get(name, LETTER_CODES.get(name))}"
    else:
        action = f'keystroke "{name}"'
    subprocess.run(["osascript", "-e", f'tell application "System Events" to {action}{clause}'], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    shot_parser = sub.add_parser("shot")
    shot_parser.add_argument("out")
    click_parser = sub.add_parser("click")
    click_parser.add_argument("x", type=float)
    click_parser.add_argument("y", type=float)
    click_parser.add_argument("--shown-scale", type=float, default=1.2)
    scroll_parser = sub.add_parser("scroll")
    scroll_parser.add_argument("x", type=float)
    scroll_parser.add_argument("y", type=float)
    scroll_parser.add_argument("lines", type=float)
    scroll_parser.add_argument("--shown-scale", type=float, default=1.2)
    type_parser = sub.add_parser("type")
    type_parser.add_argument("text")
    key_parser = sub.add_parser("key")
    key_parser.add_argument("name")
    key_parser.add_argument("mods", nargs="*")
    args = parser.parse_args()
    if args.command == "scroll":
        scroll(args.x, args.y, args.lines, args.shown_scale)
    elif args.command == "type":
        type_text(args.text)
    elif args.command == "key":
        press(args.name, args.mods)
    elif args.command == "shot":
        shot(args.out)
    else:
        click(args.x, args.y, args.shown_scale)


if __name__ == "__main__":
    sys.exit(main())
