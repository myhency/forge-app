"""Finding and activating the Forge window (macOS, needs the Quartz module)."""
import subprocess

import Quartz

SHOT_PATH = "/private/tmp/forge-harness-shot.png"
OWNER = "forge-app"
TITLE = "Forge"


def find_window():
    """The on-screen Forge window as {'id', 'bounds': (x, y, w, h)}, or None."""
    windows = Quartz.CGWindowListCopyWindowInfo(
        Quartz.kCGWindowListOptionOnScreenOnly | Quartz.kCGWindowListExcludeDesktopElements,
        Quartz.kCGNullWindowID,
    )
    for window in windows or []:
        if window.get("kCGWindowOwnerName") == OWNER and window.get("kCGWindowName") == TITLE:
            b = window["kCGWindowBounds"]
            return {"id": window["kCGWindowNumber"], "bounds": (b["X"], b["Y"], b["Width"], b["Height"])}
    return None


def bring_to_front():
    subprocess.run(
        ["osascript", "-e", f'tell application "System Events" to set frontmost of process "{OWNER}" to true'],
        check=False,
        capture_output=True,
    )
