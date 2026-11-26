"""Transcript-shaped Verify exercises with a scripted editor and real subprocess checks."""
import hashlib
import json
import os
from pathlib import Path
import shlex
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

with patch.dict(os.environ, TB_TASK="react-lead-form"):
    import solver


class VerifyTests(unittest.TestCase):
    def exercise(self, task, name, original, broken, repaired, command, diagnostic, setup):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            app, work = root / "app", root / "project"
            app.mkdir()
            work.mkdir()
            (root / "deadline").write_text(str(time.time() + 300))
            path = app / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(original)
            setup(app)
            baseline = {"files": {name: hashlib.sha256(path.read_bytes()).hexdigest()}}
            events = []
            with patch.multiple(solver, APP=app, WORK=work, TASK=task), patch.dict(solver.CHECKS, {task: command}), patch.object(solver, "record", side_effect=lambda **e: events.append(e)):
                actions = iter([{"action": "write", "path": name, "content": broken}, {"action": "check"}])
                def fail(_prompt, notes=False):
                    return json.dumps(next(actions))
                fail.call_id = "failed"
                with patch.object(solver, "ask", fail):
                    obs = solver.solve("Use the public fixture", cycle="cycle:C3", baseline=baseline,
                                       verify_ref="step:C3/S3")
                self.assertTrue(obs["regression"])
                self.assertNotEqual(obs["check_exit"], 0)
                self.assertIn(diagnostic, obs["check_output"])
                failed_check = {"verify": "step:C3/S3", "exit": obs["check_exit"], "output": obs["check_output"]}
                # The separate native Core integration test exercises the real revisit/projection.
                path.write_text(original)
                steps = []
                def branch(prompt, notes=False):
                    state = json.loads(prompt.splitlines()[-1])
                    check = state["local_check"]
                    self.assertEqual(check["command"], command)
                    current = state["files"][name]
                    self.assertEqual(current["sha256"], hashlib.sha256(path.read_bytes()).hexdigest())
                    branch.call_id = str(len(steps))
                    self.assertLess(len(steps), 8, "fixture did not make progress")
                    if check["current_exit"] == 0:
                        self.assertFalse(check["pending"])
                        action = ({"action": "finish"} if current.get("unchanged_writes")
                                  else {"action": "write", "path": name, "content": repaired})
                    elif current["written_here"]:
                        self.assertEqual(current["pages"], {})
                        self.assertEqual(check["last_failure"], failed_check)
                        self.assertTrue(check["pending"])
                        action = {"action": "check"}
                    elif state["repeat_reads"]:
                        self.assertEqual(current["pages"]["0"]["reads"], 2)
                        self.assertTrue(current["pages"]["0"]["eof"])
                        action = {"action": "write", "path": name, "content": repaired}
                    else:
                        # Relative and absolute spellings refer to the same version and page.
                        action = {"action": "read", "path": str(path) if current["pages"] else name}
                    steps.append(action["action"])
                    return json.dumps(action)
                with patch.object(solver, "ask", branch):
                    fixed = solver.solve("Address the failed public check", cycle="cycle:C4", baseline=baseline,
                                         verify_ref="step:C4/S3", failed_check=failed_check)
            self.assertEqual(steps, ["read", "read", "write", "check", "write", "finish"])
            self.assertEqual(fixed["check_exit"], 0)
            self.assertEqual(obs["check_command"], fixed["check_command"])
            self.assertEqual(path.read_text(), repaired)
            self.assertEqual([e["read_count"] for e in events if "read_count" in e], [1, 2])
            self.assertEqual(fixed["repeat_reads"], 1)
            self.assertEqual(json.loads((work/"verify-activity.json").read_text())["verify"], "step:C4/S3")

    def test_react_quoted_file_key_reruns_build(self):
        def setup(app):
            compiler = Path(__file__).resolve().parents[5] / "mcp-app/node_modules/.bin/esbuild"
            (app / "package.json").write_text(json.dumps({"scripts": {"build":
                shlex.quote(str(compiler)) + " src/lib/submitLead.ts --outfile=out.js"}}))
        self.exercise("react-lead-form", "src/lib/submitLead.ts", "export const files = {};",
                      "export const files = {crm_leads.json: []};",
                      "export const files = {'crm_leads.json': []};", ["npm", "run", "build"], 'Expected "}"', setup)

    def test_vf2_repeat_read_then_fix_directed_edge(self):
        self.exercise("vf2-speedup-networkx", "fast_networkx/__init__.py",
                      "def has_edge(edges, a, b): return any(e == (a,b) for e in edges)",
                      "def has_edge(edges, a, b): return (a,b) in edges or (b,a) in edges",
                      "def has_edge(edges, a, b): return (a,b) in edges",
                      [sys.executable, "-B", "-c", "from fast_networkx import has_edge; "
                       "assert has_edge([(1,2)],1,2); assert not has_edge([(1,2)],2,1), 'reversed edge'"],
                      "reversed edge", lambda app: None)
