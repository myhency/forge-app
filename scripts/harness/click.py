#!/usr/bin/env python3
"""click.py <x> <y> [--image PATH] [--option]: click the pixel (x, y) of the last capture of the Forge window."""
import argparse
import time

import Quartz

from geometry import png_size, to_screen_point
from window import SHOT_PATH, bring_to_front, find_window


def click_at(x, y, option=False):
    for event_type in (Quartz.kCGEventMouseMoved, Quartz.kCGEventLeftMouseDown, Quartz.kCGEventLeftMouseUp):
        event = Quartz.CGEventCreateMouseEvent(None, event_type, (x, y), Quartz.kCGMouseButtonLeft)
        if option and event_type != Quartz.kCGEventMouseMoved:
            Quartz.CGEventSetFlags(event, Quartz.kCGEventFlagMaskAlternate)
        Quartz.CGEventPost(Quartz.kCGHIDEventTap, event)
        time.sleep(0.05)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("x", type=float)
    parser.add_argument("y", type=float)
    parser.add_argument("--image", default=SHOT_PATH)
    parser.add_argument("--option", action="store_true", help="hold Option during the click")
    args = parser.parse_args()

    window = find_window()
    if not window:
        raise SystemExit("no Forge window; is the app running (launch.sh)?")
    try:
        point = to_screen_point(args.x, args.y, png_size(args.image), window["bounds"])
    except ValueError as error:
        raise SystemExit(f"not clicking: {error}")
    bring_to_front()
    time.sleep(0.3)
    click_at(*point, option=args.option)
    print(f"clicked screen point ({point[0]:.1f}, {point[1]:.1f})")


if __name__ == "__main__":
    main()
