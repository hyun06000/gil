"""Preflight in the same unprivileged process environment that execs GIL."""
import os
from pathlib import Path
import socket
import ssl

if os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY") or os.isatty(0):
    raise SystemExit("Harbor worker requires a display-free, noninteractive session")
# Harbor redirects TCP to a transparent deny-all proxy. A local TCP handshake can succeed;
# only an authenticated TLS handshake demonstrates contact with the remote download host.
try:
    host = "huggingface.co"
    with socket.create_connection((host, 443), timeout=4) as connection:
        with ssl.create_default_context().wrap_socket(connection, server_hostname=host):
            raise SystemExit("offline worker could authenticate the remote model download host")
except OSError:
    pass

Path("/work/project").mkdir()
with open("/work/result.json", "w") as result:
    os.dup2(result.fileno(), 1)
os.execv("/usr/local/bin/gil", ["gil", "benchmark", "--in-container", "/case/task.txt",
                                "/case/adapter.py", "/work/project", "/work/record"])
