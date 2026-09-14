"""Drive `physis-system-tui` through a pty and print what it rendered.

A terminal UI that is never started in CI rots silently, and a screenshot in a
document is not evidence. This opens a real pty, sets a window size (a pty
without one renders nothing, which is how this script first "passed"), sends
keys, and prints the raw frames for inspection.

    cargo build --offline -p physis-core --no-default-features --features tui \
        --bin physis-system-tui
    python3 scripts/system-tui-smoke.py ~/dev/randomesh p 'rag bridge' ENTER

Keys are passed as arguments: literal text, or TAB / ENTER / UP / DOWN / ESC.
Mouse handling is exercised by sending SGR sequences (\x1b[<0;col;rowM).
"""
import fcntl, os, pty, select, struct, sys, termios, time

BIN = "/home/gio/dev/physis-pro/target/debug/physis-system-tui"
ROOT = sys.argv[1] if len(sys.argv) > 1 else "/home/gio/dev/randomesh"
keys = sys.argv[2:] or []

pid, fd = pty.fork()
if pid == 0:
    os.environ["TERM"] = "xterm-256color"
    os.execv(BIN, [BIN, "--root", ROOT])
fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 140, 0, 0))
os.set_blocking(fd, False)
buf = b""

def pump(seconds=1.0):
    global buf
    end = time.time() + seconds
    while time.time() < end:
        r, _, _ = select.select([fd], [], [], 0.1)
        if r:
            try:
                buf += os.read(fd, 65536)
            except OSError:
                break

pump(1.5)
for k in keys:
    seq = {"TAB": "\t", "ENTER": "\r", "DOWN": "\x1b[B", "UP": "\x1b[A", "ESC": "\x1b"}.get(k, k)
    os.write(fd, seq.encode())
    pump(0.8)
os.write(fd, b"q")
pump(0.5)
text = buf.decode("utf-8", "replace")
print(text[-6000:])
