"""The run and edit tools, in the control arm and inside a GIL Verify."""
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

with patch.dict(os.environ, TB_TASK="vf2-speedup-networkx"):
    import solver

TASK = "vf2-speedup-networkx"
NAME = "fast_networkx/__init__.py"
ORIGINAL = "def order(labels):\n    return sorted(labels)\n"
CHECK = [sys.executable, "-B", "-c", "from fast_networkx import order; assert order(['B','a']) == ['a','B'], 'case order'"]


def sha(text):
    return hashlib.sha256(text.encode()).hexdigest()


class RunEditTests(unittest.TestCase):
    def drive(self, actions, *, cycle=None):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        root = Path(temp.name).resolve()
        self.app, self.work = root / "app", root / "project"
        (self.app / "fast_networkx").mkdir(parents=True)
        (self.app / "node_modules").mkdir()
        self.work.mkdir()
        (root / "deadline").write_text(str(time.time() + 400))
        (self.app / NAME).write_text(ORIGINAL)
        (self.app / "README.md").write_text("not a submission file\n")
        self.events, self.prompts = [], []
        script = iter(actions)

        def model(prompt, notes=False):
            model.call_id = str(len(self.prompts))
            self.prompts.append(prompt)
            return json.dumps(next(script))

        options = dict(cycle=cycle, baseline={"files": {NAME: sha(ORIGINAL)}}, verify_ref="step:C3/S3") if cycle else {}
        with patch.multiple(solver, APP=self.app, WORK=self.work, TASK=TASK, ask=model, RUN_SECONDS=2), \
                patch.dict(solver.CHECKS, {TASK: CHECK}), \
                patch.object(solver, "record", side_effect=lambda **e: self.events.append(e)):
            return solver.solve("Order labels ignoring case", **options)

    def tools(self):
        return [e for e in self.events if e.get("event") == "tool"]

    def test_run_returns_status_and_output_and_a_failing_behaviour_the_fixed_check_cannot_see(self):
        probe = f"{sys.executable} -B -c \"from fast_networkx import order; print(order(['b','b','A']))\""
        obs = self.drive([
            {"action": "run", "command": probe},
            {"action": "edit", "path": NAME, "old": "sorted(labels)", "new": "sorted(set(labels), key=str.casefold)"},
            {"action": "check"},                  # passes: the fixed check has no duplicate
            {"action": "run", "command": probe},  # the agent's own probe shows the lost duplicate
            {"action": "edit", "path": NAME, "old": "sorted(set(labels), key", "new": "sorted(labels, key"},
            {"action": "run", "command": probe},
            {"action": "check"}, {"action": "finish"}])
        runs = [e["result"] for e in self.tools() if e["action"] == "run"]
        self.assertEqual([r.strip() for r in runs], ["exit 0: ['A', 'b', 'b']", "exit 0: ['A', 'b']", "exit 0: ['A', 'b', 'b']"])
        self.assertEqual((self.app / NAME).read_text(), "def order(labels):\n    return sorted(labels, key=str.casefold)\n")
        self.assertEqual(obs["files"], {NAME: sha((self.app / NAME).read_text())})
        self.assertEqual((obs["check_exit"], obs["first_write_call"]), (0, 2))
        # The request names both tools.
        self.assertIn('{"action":"edit","path":"relative/file","old":', self.prompts[0])
        self.assertIn('{"action":"run","command":"shell command"} (sh in /app, no network, 2 s limit', self.prompts[0])
        # A file made for looking at things belongs in the project, where its tooling applies.
        self.assertIn("keep any file you create for this inside the task directory", self.prompts[0])

    def test_edit_needs_exactly_one_occurrence_and_a_writable_path(self):
        self.drive([
            {"action": "edit", "path": NAME, "old": "labels", "new": "names"},
            {"action": "edit", "path": NAME, "old": "absent", "new": "x"},
            {"action": "edit", "path": NAME, "old": "", "new": "x"},
            {"action": "edit", "path": "README.md", "old": "not", "new": "x"},
            {"action": "edit", "path": NAME, "old": "return sorted(labels)", "new": "return sorted(labels, key=str.casefold)"},
            {"action": "check"}, {"action": "finish"}])
        errors = [e.get("error") for e in self.tools()[:4]]
        self.assertIn("occurs 2 times", errors[0])
        self.assertIn("occurs 0 times", errors[1])
        self.assertIn("occurs 0 times", errors[2])
        self.assertIn("README.md is not part of the submission and cannot be changed by any tool. Only these are "
                      "graded and writable: fast_networkx.py and anything under fast_networkx/.", errors[3])
        self.assertEqual((self.app / "README.md").read_text(), "not a submission file\n")
        self.assertEqual(self.tools()[4]["result"], "written; the file is in view as it is now")

    def test_a_failed_edit_says_where_the_passage_went_wrong(self):
        self.drive([
            {"action": "edit", "path": NAME, "old": "def order(labels):   return sorted(labels)", "new": "x"},
            {"action": "edit", "path": NAME, "old": "def order(labels):\n    return list(labels)", "new": "x"},
            {"action": "edit", "path": NAME, "old": "    return sortd(labels)", "new": "x"},
            {"action": "edit", "path": NAME, "old": "class Graph:", "new": "x"},
            {"action": "edit", "path": NAME, "old": "labels", "new": "x"},
            {"action": "write", "path": NAME, "content": ORIGINAL.replace("sorted(labels)", "sorted(labels, key=str.casefold)")},
            {"action": "check"}, {"action": "finish"}])
        errors = [e.get("error", "") for e in self.tools()[:5]]
        self.assertIn("It matches except for spaces or line breaks", errors[0])
        self.assertIn("Its first line is at line 1 of the file, but what follows there differs", errors[1])
        self.assertIn("the closest line in the file is: return sorted(labels)", errors[2])
        self.assertIn("No line of the file resembles its first line.", errors[3])
        self.assertIn("occurs 2 times", errors[4])
        self.assertIn("Include a neighbouring line so the passage is unique.", errors[4])
        self.assertEqual((self.app / NAME).read_text(), ORIGINAL.replace("sorted(labels)", "sorted(labels, key=str.casefold)"))

    def test_a_refusal_repeated_says_so_and_the_request_states_the_time_left(self):
        self.drive([{"action": "edit", "path": "README.md", "old": "not", "new": "x"}] * 3 +
                   [{"action": "write", "path": NAME, "content": ORIGINAL.replace("sorted(labels)", "sorted(labels, key=str.casefold)")},
                    {"action": "edit", "path": "README.md", "old": "not", "new": "x"},
                    {"action": "check"}, {"action": "finish"}])
        errors = [e.get("error", "") for e in self.tools()]
        self.assertNotIn("in a row", errors[0])
        self.assertIn("(refused 2 times in a row with the same answer: repeating it will not change the result)", errors[1])
        self.assertIn("(refused 3 times in a row", errors[2])
        # A call that worked in between starts the count again.
        self.assertNotIn("in a row", errors[4])
        self.assertIn("refused 3 times in a row", self.prompts[3])
        self.assertRegex(self.prompts[0], r"\nTime left in this session: [45] minutes\.")

    def test_submission_files_a_command_creates_changes_or_removes_are_recorded_and_need_a_new_check(self):
        obs = self.drive([
            {"action": "edit", "path": NAME, "old": "sorted(labels)", "new": "sorted(labels, key=str.casefold)"},
            {"action": "check"},
            # A generated module inside the package, a rewrite of a tracked file, and files outside the submission.
            {"action": "run", "command": "printf 'FAST = 1\\n' > fast_networkx/_native.py; printf '# built\\n' >> fast_networkx/__init__.py; "
                                         "echo x > README.md; echo y > node_modules/z.py; echo z > /tmp/gil-run-scratch.txt"},
            {"action": "finish"},  # refused: the submission changed after the check
            {"action": "check"},
            {"action": "run", "command": "rm fast_networkx/_native.py"},
            {"action": "check"}, {"action": "finish"}])
        tools = self.tools()
        self.assertIn("Submission files changed by this command: ['fast_networkx/__init__.py', 'fast_networkx/_native.py']", tools[2]["result"])
        self.assertIn("check before finish", tools[3]["error"])
        self.assertIn("Submission files changed by this command: ['fast_networkx/_native.py']", tools[5]["result"])
        self.assertEqual(obs["files"], {NAME: sha((self.app / NAME).read_text())})
        self.assertTrue((self.app / NAME).read_text().endswith("# built\n"))

    def test_a_command_over_the_limit_is_stopped_with_everything_it_started(self):
        marker = Path(tempfile.gettempdir()) / f"gil-run-left-{os.getpid()}"
        marker.unlink(missing_ok=True)
        self.addCleanup(lambda: marker.unlink(missing_ok=True))
        started = time.time()
        self.drive([
            {"action": "run", "command": f"echo begun; (sleep 4; touch {marker}) & sleep 30"},
            {"action": "run", "command": f"(sleep 3; touch {marker}) > /dev/null 2>&1 & echo quick"},
            {"action": "write", "path": NAME, "content": ORIGINAL.replace("sorted(labels)", "sorted(labels, key=str.casefold)")},
            {"action": "check"}, {"action": "finish"}])
        tools = self.tools()
        self.assertTrue(tools[0]["result"].startswith("exit none, stopped after 2 s: begun"))
        self.assertEqual(tools[1]["result"].strip(), "exit 0: quick")
        self.assertLess(time.time() - started, 12)
        time.sleep(4.5)
        self.assertFalse(marker.exists(), "a background process outlived its command")

    def test_inside_a_verify_a_command_s_changes_reach_the_core_mirror_and_repeats_count_as_no_progress(self):
        same = {"action": "run", "command": "echo same", "note": "probe"}
        obs = self.drive([
            {"action": "run", "command": "printf 'def order(labels):\\n    return sorted(labels, key=str.casefold)\\n' > fast_networkx/__init__.py",
             "note": "rewrite with a key"},
            {"action": "check", "note": "check it"}] + [same] * 20, cycle="cycle:C3")
        mirror = self.work / "submission" / NAME
        self.assertEqual(mirror.read_text(), (self.app / NAME).read_text())
        self.assertEqual(obs["files"], {NAME: sha(mirror.read_text())})
        state = json.loads((self.work / "verify-activity.json").read_text())
        self.assertTrue(state["files"][NAME]["written_here"])
        self.assertEqual(state["files"][NAME]["previous_sha256"], sha(ORIGINAL))
        # The first `echo same` is new; the following identical runs on unchanged files are not.
        self.assertEqual((obs["stop"], obs["check_exit"]), ("stalled", 0))
        self.assertEqual(obs["solver_calls"], 3 + solver.STALL_LIMIT)
        self.assertEqual(self.tools()[2]["command"], "echo same")


if __name__ == "__main__":
    unittest.main()
