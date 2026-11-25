"""Compare normal and explicitly headless MCP using the same Linux binary as the benchmark."""
import json
import os
from pathlib import Path
import subprocess
import tempfile

for mode in ("interactive", "headless"):
    base = Path(tempfile.mkdtemp())
    root = base / "project"
    root.mkdir()
    env = dict(os.environ, HOME=str(base), XDG_CONFIG_HOME=str(base / "config"))
    env.pop("GIL_MCP_DISPLAY", None)
    if mode == "headless":
        env["GIL_MCP_DISPLAY"] = mode
    with subprocess.Popen(["/usr/local/bin/gil", "mcp", "--serve"], stdin=subprocess.PIPE,
                          stdout=subprocess.PIPE, text=True, env=env) as proc:
        def send(method, params=None, request_id=None):
            msg = dict(jsonrpc="2.0", method=method)
            if params is not None:
                msg["params"] = params
            if request_id is not None:
                msg["id"] = request_id
            proc.stdin.write(json.dumps(msg) + "\n")
            proc.stdin.flush()
            if request_id is not None:
                response = json.loads(proc.stdout.readline())
                assert response["id"] == request_id and "result" in response, response
                return response["result"]
        send("initialize", {"protocolVersion":"2025-06-18","capabilities":{},
                            "clientInfo":{"name":"container-mode-test","version":"1"}}, 1)
        send("notifications/initialized")
        start = send("tools/call", {"name":"gil_start","arguments":{"project_root":str(root),"locale":"en"}}, 2)
        fields = start["structuredContent"]
        assert fields["ok"], start
        if mode == "interactive":
            assert fields["next_call"]["name"] == "show_gil_monitor", start
        else:
            assert "show_gil_monitor" not in json.dumps(start) and "next_action" in fields, start
        request = {"name":"gil_open","arguments":{"project_root":str(root),"kind":"question",
                   "contract":"objective: Learn the task\nnext_action: Ask the participant\ndone_when: Receive an answer"}}
        assert send("tools/call", request, 3)["structuredContent"]["ok"]
        context = send("tools/call", {"name":"gil_context","arguments":{"project_root":str(root)}}, 4)
        if mode == "headless":
            assert "wait" in context["structuredContent"]["next_action"], context
        assert "benchmark_accept" not in json.dumps(context), context
        proc.stdin.close()
        assert proc.wait(timeout=15) == 0
