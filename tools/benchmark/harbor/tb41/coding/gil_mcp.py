"""The GIL MCP server of a trial, as the stand-in agent and the stand-in user reach it.

The server is the product's own (`gil mcp --serve`) in its unattended mode: no Monitor, and whoever
runs the session answers for the user. Three parties share this one connection:

- the model gets the server's instructions and every tool the server lists for a model;
- the runner (this module) keeps the two runner tools to itself;
- the stand-in user is a fixed policy: it approves what GIL puts before the user, and to anything
  the agent asks it answers that the task statement is all there is and the agent's own
  recommendation stands.
"""
import json
import os
import shutil
import subprocess
from pathlib import Path

RUNNER = ["runner"]
# What GIL adds to the project folder. None of it belongs in a graded submission.
ADDED = (".gil", ".gilignore", "AGENTS.md", "CLAUDE.md")
REPLY = ("I have nothing to add to the task statement: it is everything I want. Where you recommend "
         "something, go with your recommendation. Do not wait for more from me.")
OPENING = ("The user says: Let's do this as a GIL project. The project folder is {root} (use it as "
           "project_root). Start it with start_kind tracked_scope, and work on the task through GIL: the "
           "interview, then experiments. I will approve what GIL asks me to approve.")
# How the unattended server begins the paragraph that says a decision of the user is awaited.
AWAITING = "Awaiting the user's decision"
SAY = {"type": "function", "name": "say", "strict": True,
       "description": "Say something to the user or ask the user a question, and get the reply.",
       "parameters": {"type": "object", "properties": {"message": {"type": "string"}},
                      "required": ["message"], "additionalProperties": False}}
RESULT_LIMIT = 24000


def shortened(text):
    """A long GIL answer keeps both ends. A receipt opens with its guidance, but gil_context ends with what
    matters most: the Cycles left behind by going back with their lessons, the current Cycle, and where the
    agent stands. Cutting the end off hid every earlier failure from the agent (found 2026-11-27)."""
    if len(text) <= RESULT_LIMIT:
        return text
    head = RESULT_LIMIT // 4
    tail = RESULT_LIMIT - head
    return f"{text[:head]}\n[… {len(text) - RESULT_LIMIT} characters left out here …]\n{text[-tail:]}"


class Server:
    """A minimal MCP client over the server's stdio."""

    def __init__(self, command, env):
        self.process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                        stderr=subprocess.DEVNULL, text=True, env=env)
        self.sent = 0
        hello = self.request("initialize", {"protocolVersion": "2025-06-18", "capabilities": {},
                                            "clientInfo": {"name": "gil-benchmark-runner", "version": "1"}})
        self.instructions = hello["result"].get("instructions") or ""
        self.post("notifications/initialized")
        self.tools = self.request("tools/list")["result"]["tools"]

    def post(self, method, params=None):
        self.process.stdin.write(json.dumps({"jsonrpc": "2.0", "method": method, "params": params or {}}) + "\n")
        self.process.stdin.flush()

    def request(self, method, params=None):
        self.sent += 1
        self.process.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.sent, "method": method,
                                             "params": params or {}}) + "\n")
        self.process.stdin.flush()
        while True:
            line = self.process.stdout.readline()
            if not line:
                raise RuntimeError("The GIL MCP server closed its output")
            frame = json.loads(line)
            if frame.get("id") == self.sent:
                return frame

    def call(self, name, arguments):
        frame = self.request("tools/call", {"name": name, "arguments": arguments})
        if "error" in frame:
            # A wire refusal: the arguments did not fit the tool. Nothing ran.
            return True, str(frame["error"].get("message")), {}
        result = frame["result"]
        text = "\n".join(block.get("text", "") for block in result.get("content", []) if block.get("type") == "text")
        return bool(result.get("isError")), text, result.get("structuredContent") or {}

    def close(self):
        self.process.stdin.close()
        self.process.wait(timeout=20)


class Gil:
    def __init__(self, binary, root, record=None):
        self.root = str(root)
        self.record = record
        # What the folder already held under these names is the task's own, and stays.
        self.own = {name for name in ADDED if (Path(root) / name).exists()}
        env = {**os.environ, "GIL_MCP_DISPLAY": "headless", "GIL_MCP_APPROVAL": "unattended"}
        self.server = Server([str(binary), "mcp", "--serve"], env)
        self.names = set()
        self.tools = [SAY]
        for tool in self.server.tools:
            if (tool.get("_meta") or {}).get("ui", {}).get("visibility") == RUNNER:
                continue
            self.names.add(tool["name"])
            self.tools.append({"type": "function", "name": tool["name"], "strict": False,
                               "description": tool.get("description") or "", "parameters": tool["inputSchema"]})
        self.system = "You have the GIL MCP server connected. Its instructions:\n\n" + self.server.instructions
        self.opening = OPENING.format(root=self.root)
        self.log = []

    def call(self, name, arguments):
        """One tool call of the model, then the stand-in user's answer to whatever now waits for the user."""
        if name not in self.names:
            raise ValueError(f"{name} is not a tool of this session")
        if not isinstance(arguments, dict):
            raise ValueError("Tool arguments must be an object")
        refused, text, structured = self.server.call(name, arguments)
        said = ("REFUSED: " if refused else "") + shortened(text)
        answered = self.answer(structured.get("awaiting_user") or [])
        self.log.append({"tool": name, "refused": refused, "answered": answered})
        if answered:
            # The receipt was written while the decision was still awaited and tells the agent to end
            # its turn and wait. The decision has been given in the meantime, so that paragraph is
            # taken out: an agent that obeyed it ended the whole session after two calls (2026-11-28).
            said = "\n".join(line for line in said.split("\n") if not line.startswith(AWAITING))
            said = ("The user has now decided: " + "; ".join(answered) + ". Nothing is awaited any more: do not end "
                    "your turn, go on with the next step now. The receipt below was written before that decision.\n" + said)
        return said

    def answer(self, subjects):
        """The stand-in user approves exactly what GIL shows as waiting, for the digest read."""
        if not subjects:
            return []
        _, _, review = self.server.call("gil_unattended_review", {"project_root": self.root})
        answered = []
        for wait in review.get("pending") or []:
            refused, text, _ = self.server.call("gil_unattended_decide", {
                "project_root": self.root, "subject": wait["subject"], "digest": wait["digest"], "decision": "approve"})
            answered.append(f"{wait['subject']} {'could not be approved: ' + text[:300] if refused else 'approved'}")
        return answered

    def open_cycle(self):
        """The reference of the Cycle GIL still has open, or None at a closed boundary or before a project exists."""
        _, _, read = self.server.call("gil_plan_read", {"project_root": self.root})
        cycle = read.get("current_cycle") or {}
        return cycle.get("ref") if cycle.get("state") == "open" else None

    def say(self, message):
        self.log.append({"say": str(message)[:2000]})
        return "The user replies: " + REPLY

    def close(self):
        """End the server and take what GIL added out of the submission folder, keeping it beside the trial's logs."""
        # Whether the record was left in the middle of a Cycle is a fact of the run, kept with its result.
        try:
            left_open = self.open_cycle()
        except Exception:
            left_open = "unknown"
        try:
            self.server.close()
        except Exception:
            self.server.process.kill()
        moved = []
        for name in ADDED:
            at = Path(self.root) / name
            if name in self.own or not (at.exists() or at.is_symlink()):
                continue
            if self.record:
                Path(self.record).mkdir(parents=True, exist_ok=True)
                shutil.move(str(at), str(Path(self.record) / name))
            elif at.is_dir():
                shutil.rmtree(at)
            else:
                at.unlink()
            moved.append(name)
        return {"removed_from_submission": moved, "open_at_end": left_open, "calls": self.log}
