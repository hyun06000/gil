"""The proxy an agent with its own harness reaches GIL through, driven over stdio as that harness does.

Needs a `gil` built with the product features; set GIL_TEST_BINARY to it. Skipped otherwise.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
BINARY = os.environ.get("GIL_TEST_BINARY")
CONTRACT = {"objective": "o", "next_action": "n", "done_when": "d"}


@unittest.skipUnless(BINARY and Path(BINARY).is_file(), "GIL_TEST_BINARY is not a product-feature gil")
class ProxyTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        base = Path(self.tmp.name).resolve()
        self.app, self.logs, home = base / "app", base / "logs", base / "home"
        for folder in (self.app, home):
            folder.mkdir()
        (self.app / "a.py").write_text("print('hi')\n")
        self.proxy = subprocess.Popen([sys.executable, str(HERE / "gil_proxy.py"), BINARY, str(self.app), str(self.logs)],
                                      stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, cwd=HERE,
                                      env={**os.environ, "HOME": str(home)})
        self.sent = 0
        self.addCleanup(self.tmp.cleanup)
        self.addCleanup(lambda: self.proxy.poll() is None and self.proxy.kill())

    def rpc(self, method, params=None):
        self.sent += 1
        self.proxy.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.sent, "method": method, "params": params or {}}) + "\n")
        self.proxy.stdin.flush()
        return json.loads(self.proxy.stdout.readline())

    def call(self, name, **arguments):
        result = self.rpc("tools/call", {"name": name, "arguments": arguments})["result"]
        return result["isError"], result["content"][0]["text"]

    def test_the_model_gets_gils_tools_without_the_runners_and_the_folder_ends_clean(self):
        hello = self.rpc("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}})
        self.assertIn("GIL", hello["result"]["instructions"])
        names = [tool["name"] for tool in self.rpc("tools/list")["result"]["tools"]]
        self.assertIn("gil_open", names)
        self.assertIn("say", names)
        self.assertFalse([name for name in names if "unattended" in name], "a runner tool reached the model")

        root = str(self.app)
        self.assertFalse(self.call("gil_start", project_root=root, start_kind="tracked_scope")[0])
        refused, said = self.call("gil_start_prepare", project_root=root, rules="node_modules/\n")
        # What GIL put before the user was answered by the stand-in, in the same call.
        self.assertFalse(refused, said)
        self.assertIn("start approved", said)
        self.assertIn("nothing to add", self.call("say", message="What do you want?")[1])
        self.assertFalse(self.call("gil_open", project_root=root, kind="question", contract=CONTRACT)[0])
        # The model cannot approve for the user.
        refused, said = self.call("gil_unattended_review", project_root=root)
        self.assertTrue(refused and "not a tool" in said, said)

        self.proxy.stdin.close()
        self.assertEqual(self.proxy.wait(timeout=30), 0)
        self.assertEqual(sorted(p.name for p in self.app.iterdir()), ["a.py"], "GIL's files stayed in the graded folder")
        self.assertTrue((self.logs / "gil-record.tgz").is_file())
        calls = [json.loads(line) for line in (self.logs / "gil-calls.jsonl").read_text().splitlines()]
        self.assertEqual([c["tool"] for c in calls if "tool" in c][:2], ["gil_start", "gil_start_prepare"])


if __name__ == "__main__":
    unittest.main()
