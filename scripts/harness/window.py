"""Finding and activating the Forge window (macOS, needs the Quartz module)."""
import subprocess
from pathlib import Path

import Quartz

from geometry import select_window

SHOT_PATH = "/private/tmp/forge-harness-shot.png"
PID_FILE = "/private/tmp/forge-harness.pid"
OWNER = "forge-app"
TITLE = "Forge"


def harness_pid():
    """The pid launch.sh recorded, or None when the harness app was not launched."""
    try:
        return int(Path(PID_FILE).read_text().strip())
    except (OSError, ValueError):
        return None


def find_window():
    """The on-screen window of the harness app as {'id', 'bounds': (x, y, w, h)}, or None."""
    windows = Quartz.CGWindowListCopyWindowInfo(
        Quartz.kCGWindowListOptionOnScreenOnly | Quartz.kCGWindowListExcludeDesktopElements,
        Quartz.kCGNullWindowID,
    )
    return select_window(windows, harness_pid(), OWNER, TITLE)


def bring_to_front():
    pid = harness_pid()
    if pid is None:
        return
    script = f'tell application "System Events" to set frontmost of (first process whose unix id is {pid}) to true'
    subprocess.run(["osascript", "-e", script], check=False, capture_output=True)
