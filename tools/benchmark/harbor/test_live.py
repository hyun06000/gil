"""What a person can follow while a coding trial runs: one line per model reply, and a host-side copy."""
import asyncio
import contextlib
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import types
import unittest
from unittest.mock import patch


class LiveTests(unittest.TestCase):
    def test_every_reply_is_printed_with_the_previous_tool_result_and_kept_on_the_host(self):
        base = types.ModuleType("harbor.agents.base")
        base.BaseAgent = type("BaseAgent", (), {})
        fake = {"harbor": types.ModuleType("harbor"), "harbor.agents": types.ModuleType("harbor.agents"), "harbor.agents.base": base}
        with patch.dict(sys.modules, fake), tempfile.TemporaryDirectory() as temp, \
                patch.dict(os.environ, {"GIL_HARBOR_API_KEY": "test", "TB_ARM": "off", "TB_TASK": "react-lead-form"}):
            sys.path.insert(0, str(Path(__file__).parent / "tb41"))
            try:
                from coding_agent import CodeAgent
            finally:
                sys.path.pop(0)
            agent = CodeAgent()
            agent.logs_dir = Path(temp)
            client = types.SimpleNamespace(calls=7, usage=lambda: {"estimated_usd": 0.0123456})
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                agent.live({"stage": "solver", "last": {"action": "read", "path": "a.ts", "result": "line one\nline two"}},
                           json.dumps({"action": "write", "path": "src/a.ts", "content": "x" * 5000, "note": "add the key"}), client)
                agent.live({"stage": "solver", "last": {"action": "run", "error": "empty command"}},
                           json.dumps({"action": "run", "command": "npm run submit"}), client)
                agent.live({"stage": "hypothesis", "last": None}, "The build will pass.", client)
            lines = out.getvalue().splitlines()
            self.assertEqual(lines[0], '[off] 7 solver | before: line one line two | now: {"action": "write", "path": "src/a.ts", "note": "add the key"}')
            self.assertEqual(lines[1], '[off] 7 solver | before: ERROR empty command | now: {"action": "run", "command": "npm run submit"}')
            self.assertEqual(lines[2], "[off] 7 hypothesis | before:  | now: The build will pass.")
            kept = [json.loads(l) for l in (Path(temp) / "live.jsonl").read_text().splitlines()]
            self.assertEqual(len(kept), 3)
            # The host copy keeps the whole reply, including the file content the job log leaves out.
            self.assertEqual(len(json.loads(kept[0]["answer"])["content"]), 5000)
            self.assertEqual((kept[0]["call"], kept[0]["usd"], kept[2]["stage"]), (7, 0.01235, "hypothesis"))

    def test_the_request_carries_the_last_tool_event(self):
        sys.path.insert(0, str(Path(__file__).parent / "tb41" / "coding"))
        try:
            import mailbox_client
        finally:
            sys.path.pop(0)
        with tempfile.TemporaryDirectory() as temp:
            mail = Path(temp)
            (mail / "reply.json").write_text(json.dumps({"answer": "ok"}))
            with patch.object(mailbox_client, "MAIL", mail):
                mailbox_client.record(event="tool", id="1", action="check", result="exit 1: " + "e" * 900, chars=908)
                self.assertEqual(mailbox_client.ask("next?", stage="solver"), "ok")
                request = json.loads((mail / "request.json").read_text())
        self.assertEqual((request["stage"], request["last"]["action"], len(request["last"]["result"])), ("solver", "check", 300))
        self.assertNotIn("chars", request["last"])


if __name__ == "__main__":
    unittest.main()
