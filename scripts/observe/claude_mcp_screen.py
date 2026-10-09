#!/usr/bin/env python3
"""Runs the bundled `claude` in a pty (the real config folder, so it is logged in) in a trusted folder, types
`/mcp`, walks Down to the first server that needs authentication, opens it, and saves the raw bytes seen at
each step. It never presses Authenticate. Usage: claude_mcp_screen.py <claude-binary> <out-dir> <trusted-cwd> <server-name>"""
import os, pty, re, select, sys, time

claude, out, cwd, target = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
os.makedirs(out, exist_ok=True)
env = dict(os.environ, TERM="xterm-256color")

pid, fd = pty.fork()
if pid == 0:
    os.chdir(cwd)
    os.execve(claude, [claude], env)

def read_for(seconds):
    data, end = b"", time.time() + seconds
    while time.time() < end:
        ready, _, _ = select.select([fd], [], [], 0.2)
        if ready:
            try:
                data += os.read(fd, 65536)
            except OSError:
                break
    return data

def plain(data):
    return re.sub(rb"\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b\][^\x07]*\x07", b" ", data).decode("utf8", "replace")

def save(name, data):
    with open(os.path.join(out, name), "wb") as f:
        f.write(data)
    print(name, len(data), "bytes")

save("01_input.bin", read_for(10))
os.write(fd, b"/mcp\r")
frame = read_for(4)
save("02_list.bin", frame)
stream = frame
for i in range(40):
    # The screen is drawn again as changed lines only, so the last line with the pointer is the selection.
    selected = [l for l in plain(stream).splitlines() if "\u276f" in l]
    print("selected:", selected[-1].strip() if selected else None)
    if selected and target in selected[-1]:
        break
    os.write(fd, b"\x1b[B")
    more = read_for(1.5)
    stream += more
    if i == 0:
        save("02b_down.bin", more)
save("02c_downs.bin", stream[len(frame):])
os.write(fd, b"\r")
save("03_item.bin", read_for(3))
os.write(fd, b"\x1b")
time.sleep(0.5)
os.write(fd, b"\x1b")
time.sleep(0.5)
os.write(fd, b"\x03\x03")
time.sleep(0.5)
os.kill(pid, 9)
