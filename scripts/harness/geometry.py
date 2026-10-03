"""Pure helpers for turning pixels of a window capture into screen points."""
import struct

PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def png_size(path):
    with open(path, "rb") as file:
        header = file.read(24)
    if len(header) < 24 or not header.startswith(PNG_SIGNATURE) or header[12:16] != b"IHDR":
        raise ValueError(f"{path} is not a PNG")
    return struct.unpack(">II", header[16:24])


def to_screen_point(px, py, image_size, bounds):
    """Map a pixel of the capture to a screen point.

    The capture is taken without the window shadow (`screencapture -o`), so the image covers exactly
    `bounds`; the scale is the ratio of image size to window size, whatever the display's density.
    """
    image_width, image_height = image_size
    x, y, width, height = bounds
    if not (0 <= px < image_width and 0 <= py < image_height):
        raise ValueError(f"pixel ({px}, {py}) is outside the {image_width}x{image_height} image")
    return x + px * width / image_width, y + py * height / image_height


def select_window(listed_windows, pid, owner="forge-app", title="Forge"):
    """The Forge window of process `pid` as {'id', 'bounds': (x, y, w, h)}, or None.

    Matching the pid keeps the scripts from capturing or clicking another Forge the user has open
    (a dev build reading the real ~/.claude has the same owner and title). No pid, no window.
    """
    if pid is None:
        return None
    for window in listed_windows or []:
        if (
            window.get("kCGWindowOwnerPID") == pid
            and window.get("kCGWindowOwnerName") == owner
            and window.get("kCGWindowName") == title
        ):
            b = window["kCGWindowBounds"]
            return {"id": window["kCGWindowNumber"], "bounds": (b["X"], b["Y"], b["Width"], b["Height"])}
    return None
