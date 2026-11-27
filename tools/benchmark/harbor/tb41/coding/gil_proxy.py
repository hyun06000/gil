"""The GIL MCP server as an agent with its own harness (Codex) reaches it in a trial.

Such an agent connects to MCP servers itself, so the three parties of `gil_mcp.py` have to meet
inside one server process instead of inside our own stand-in agent. This module is that process:
an MCP server over stdio that stands in front of the product's server (`gil mcp --serve`, unattended).

- The model gets the product server's instructions and every tool it lists for a model, unchanged.
- The two runner tools never reach the model: this process keeps them and answers what GIL puts
  before the user, exactly as `gil_mcp.Gil` does for our own agent.
- `say` is the stand-in user: the same fixed reply.

When the agent's harness closes the connection (or ends this process), what GIL added to the project
folder is moved out of it and kept beside the trial's logs, so the graded folder holds the task's
files only.

Run as: python3 gil_proxy.py <gil binary> <project root> <logs dir>
"""
import json
import signal
import sys
import tarfile
import time
from pathlib import Path

from gil_mcp import Gil, SAY

LOG_RESULT = 6000  # characters of an answer kept in the call log


class Proxy:
    def __init__(self, binary, root, logs):
        self.logs = Path(logs)
        self.logs.mkdir(parents=True, exist_ok=True)
        self.record = self.logs / "gil-record"
        self.gil = Gil(binary, root, record=self.record)
        self.calls = (self.logs / "gil-calls.jsonl").open("a")
        self.closed = False
        # The product server's own tool definitions, minus the runner's, plus the stand-in user.
        listed = [tool for tool in self.gil.server.tools if tool["name"] in self.gil.names]
        self.tools = [{"name": t["name"], "description": t.get("description") or "", "inputSchema": t["inputSchema"]}
                      for t in listed]
        self.tools.append({"name": SAY["name"], "description": SAY["description"], "inputSchema": SAY["parameters"]})

    def note(self, **entry):
        print(json.dumps({"time": time.time(), **entry}, ensure_ascii=False), file=self.calls, flush=True)

    def call(self, name, arguments):
        """One tool call of the model. Returns (text, refused)."""
        try:
            if name == SAY["name"]:
                text = self.gil.say((arguments or {}).get("message", ""))
            else:
                text = self.gil.call(name, arguments)
        except (ValueError, KeyError, TypeError) as error:
            text = f"REFUSED: {error}"
        refused = text.startswith("REFUSED: ")
        self.note(tool=name, arguments=arguments, refused=refused, chars=len(text), result=text[:LOG_RESULT])
        return text, refused

    def answer(self, frame):
        """The reply to one JSON-RPC request, or None for a notification."""
        method, params = frame.get("method"), frame.get("params") or {}
        if "id" not in frame:
            return None
        reply = {"jsonrpc": "2.0", "id": frame["id"]}
        if method == "initialize":
            reply["result"] = {"protocolVersion": params.get("protocolVersion") or "2025-06-18",
                               "capabilities": {"tools": {}},
                               "serverInfo": {"name": "gil", "version": "benchmark-proxy"},
                               "instructions": self.gil.server.instructions}
        elif method == "tools/list":
            reply["result"] = {"tools": self.tools}
        elif method == "tools/call":
            text, refused = self.call(params.get("name"), params.get("arguments") or {})
            reply["result"] = {"content": [{"type": "text", "text": text}], "isError": refused}
        elif method == "ping":
            reply["result"] = {}
        else:
            reply["error"] = {"code": -32601, "message": f"Method not found: {method}"}
        return reply

    def close(self):
        """Take what GIL added out of the project folder and keep it with the trial's logs."""
        if self.closed:
            return
        self.closed = True
        try:
            state = self.gil.close()
            self.note(closed={k: v for k, v in state.items() if k != "calls"})
            if self.record.exists():
                with tarfile.open(self.logs / "gil-record.tgz", "w:gz") as archive:
                    archive.add(self.record, arcname="gil-record")
        except Exception as error:  # the trial's files must not depend on this bookkeeping
            self.note(close_failed=str(error))
        self.calls.close()


def main():
    proxy = Proxy(*sys.argv[1:4])

    def ended(*_):
        proxy.close()
        sys.exit(0)

    signal.signal(signal.SIGTERM, ended)
    signal.signal(signal.SIGHUP, ended)
    try:
        for line in sys.stdin:
            line = line.strip()
            if not line:
                continue
            reply = proxy.answer(json.loads(line))
            if reply is not None:
                sys.stdout.write(json.dumps(reply, ensure_ascii=False) + "\n")
                sys.stdout.flush()
    finally:
        proxy.close()


if __name__ == "__main__":
    main()
