"""Preflight in the same unprivileged process environment that execs GIL."""
import os
from pathlib import Path
import socket

if os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY") or os.isatty(0):
    raise SystemExit("Harbor worker requires a display-free, noninteractive session")
# Harbor enforces no-network with a deny-all sidecar, so an interface may still exist.
try:
    connection = socket.create_connection(("huggingface.co", 443), timeout=4)
except OSError:
    pass
else:
    connection.close()
    raise SystemExit("offline worker could reach the model download host")

Path("/work/project").mkdir()
with open("/work/result.json", "w") as result:
    os.dup2(result.fileno(), 1)
os.execv("/usr/local/bin/gil", ["gil", "benchmark", "--in-container", "/case/task.txt",
                                "/case/adapter.py", "/work/project", "/work/record"])
