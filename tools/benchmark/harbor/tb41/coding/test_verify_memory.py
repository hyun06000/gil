"""What one GIL Verify carries from call to call: notes, held pages, and an end to calls that observe nothing new."""
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
import verify_activity

NAME = "fast_networkx.py"
ORIGINAL = "def order(labels): return sorted(labels)\n"
TASK = "vf2-speedup-networkx"


class Script:
    """A scripted model: each step sees the prompt and returns one editor call."""

    def __init__(self, step):
        self.step, self.prompts, self.flags = step, [], []

    def __call__(self, prompt, notes=False):
        self.call_id = str(len(self.prompts))
        self.prompts.append(prompt)
        self.flags.append(notes)
        return json.dumps(self.step(len(self.prompts), prompt))


class MemoryTests(unittest.TestCase):
    def run_verify(self, step, *, cycle="cycle:C3", check=None, source=ORIGINAL):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        root = Path(temp.name).resolve()
        self.app, work = root / "app", root / "project"
        self.app.mkdir()
        work.mkdir()
        (root / "deadline").write_text(str(time.time() + 300))
        (self.app / NAME).write_text(source)
        self.runs = root / "runs"
        command = check or [sys.executable, "-B", "-c",
                            f"open({str(self.runs)!r}, 'a').write('x'); from fast_networkx import order; "
                            "assert order(['B','a']) == ['a','B'], 'case order'"]
        baseline = {"files": {NAME: hashlib.sha256(source.encode()).hexdigest()}}
        self.events, model = [], Script(step)
        with patch.multiple(solver, APP=self.app, WORK=work, TASK=TASK, ask=model), \
                patch.dict(solver.CHECKS, {TASK: command}), \
                patch.object(solver, "record", side_effect=lambda **e: self.events.append(e)):
            options = dict(cycle=cycle, baseline=baseline, verify_ref="step:C3/S3") if cycle else {}
            obs = solver.solve("Order labels ignoring case", **options)
        return obs, model

    def test_a_note_and_a_read_page_stay_in_later_requests_and_the_same_page_is_not_sent_twice(self):
        fixed = "def order(labels): return sorted(labels, key=str.casefold)\n"

        def step(n, prompt):
            return [{"action": "read", "path": NAME, "offset": 0, "note": "sorted() has no key; add casefold next"},
                    {"action": "read", "path": NAME, "offset": 0, "note": "second look"},
                    {"action": "write", "path": NAME, "content": fixed, "note": "wrote the casefold key; check it"},
                    {"action": "check", "note": "expect exit 0"},
                    {"action": "finish", "note": "done"}][n - 1]

        obs, model = self.run_verify(step)
        self.assertEqual(model.flags, [True] * 5)
        self.assertNotIn("sorted(labels)", model.prompts[0])
        # After one read the request carries the page and the note; the result list keeps only a receipt.
        self.assertIn("sorted() has no key; add casefold next", model.prompts[1])
        self.assertEqual(model.prompts[1].count(json.dumps(ORIGINAL)[1:-1]), 1)
        self.assertIn('["read", "fast_networkx.py", "page [0,41) received"]', model.prompts[1])
        # The second read of the same version and offset is answered without the text, which is still held.
        reads = [e for e in self.events if e.get("action") == "read"]
        self.assertEqual([e["held"] for e in reads], [False, True])
        self.assertTrue(reads[1]["result"].startswith("Not sent again: fast_networkx.py [0,41)"))
        self.assertEqual(model.prompts[2].count(json.dumps(ORIGINAL)[1:-1]), 1)
        # A write replaces the version: its old page is dropped, the notes stay.
        self.assertNotIn(json.dumps(ORIGINAL)[1:-1], model.prompts[3])
        self.assertIn("wrote the casefold key; check it", model.prompts[4])
        self.assertEqual((obs["stop"], obs["check_exit"], obs["refused_reads"], obs["repeat_reads"], obs["notes"]),
                         ("finished", 0, 1, 1, 5))
        self.assertEqual(self.events[-2]["note"], "expect exit 0")

    def test_calls_that_observe_nothing_new_are_warned_about_and_then_close_the_verify(self):
        obs, model = self.run_verify(lambda n, prompt: {"action": "read", "path": NAME, "offset": 0, "note": f"look {n}"})
        limit = verify_activity.STALL_LIMIT
        # One new page, then `limit` refused reads; without the limit this loop runs to the deadline.
        self.assertEqual(obs["solver_calls"], 1 + limit)
        self.assertEqual((obs["stop"], obs["refused_reads"], obs["check_exit"], obs["first_write_call"]),
                         ("stalled", limit, None, None))
        self.assertEqual(self.events[-1]["event"], "verify_stalled")
        warned = [i for i, p in enumerate(model.prompts) if "calls in a row brought no new page" in p]
        self.assertEqual(warned, list(range(verify_activity.STALL_WARNING + 1, limit + 1)))
        self.assertIn(f"After {limit - verify_activity.STALL_WARNING} more, this Verify closes", model.prompts[warned[0]])
        # Notes are bounded: the oldest fall out of the request.
        self.assertLessEqual(model.prompts[-1].count('"note": "look '), verify_activity.NOTES_KEPT)

    def test_a_check_without_a_change_is_not_run_again_and_a_return_to_earlier_content_is_not_progress(self):
        other = "def order(labels): return sorted(labels, key=str.lower)\n"

        def step(n, prompt):
            if n <= 3:
                return {"action": "check", "note": "is the baseline passing?"}
            # Then alternate between two contents the Verify has already had.
            return {"action": "write", "path": NAME, "content": other if n % 2 == 0 else ORIGINAL, "note": "try again"}

        obs, model = self.run_verify(step)
        # The baseline check ran once (it fails: 'B' sorts before 'a' without a key); two repeats were not run.
        self.assertEqual(self.runs.read_text(), "x")
        checks = [e for e in self.events if e.get("action") == "check"]
        self.assertTrue(checks[1]["result"].startswith(
            "Not run again: no file changed since this check. Change a file if a requirement is still open, "
            "otherwise finish. exit 1"))
        # Call 4 writes new content (progress); every later write returns to content already seen.
        writes = [e for e in self.events if e.get("action") == "write"]
        self.assertEqual(len(writes), 1 + verify_activity.STALL_LIMIT)
        self.assertEqual(obs["stop"], "stalled")

    def test_a_new_page_a_new_content_or_a_fresh_check_each_restart_the_count(self):
        long = "# pad\n" * 2000 + ORIGINAL  # two pages
        contents = iter(f"def order(labels): return sorted(labels, key=str.casefold)  # {i}\n" for i in range(99))

        def step(n, prompt):
            kind = n % 4
            if n > 40:
                return {"action": "finish", "note": "enough"}
            if kind == 1:
                return {"action": "read", "path": NAME, "offset": 0, "note": "page one"}
            if kind == 2:
                return {"action": "read", "path": NAME, "offset": 9000, "note": "page two"}
            if kind == 3:
                return {"action": "write", "path": NAME, "content": "# pad\n" * 2000 + next(contents), "note": "edit"}
            return {"action": "check", "note": "check the edit"}

        obs, model = self.run_verify(step, source=long)
        self.assertEqual((obs["stop"], obs["solver_calls"], obs["refused_reads"]), ("finished", 41, 0))
        self.assertFalse(any("calls in a row" in p for p in model.prompts))
        # Both pages of the current version are held together.
        self.assertIn('"offset": 0', model.prompts[2])
        self.assertIn('"offset": 9000', model.prompts[2])

    def test_held_pages_stay_within_the_budget_and_an_evicted_page_is_sent_again(self):
        long = "".join(f"line {i:06d}\n" for i in range(4500))  # 54,000 characters, six pages
        offsets = [0, 9000, 18000, 27000, 0]
        obs, model = self.run_verify(lambda n, p: {"action": "read", "path": NAME, "offset": offsets[n - 1], "note": "scan"}
                                     if n <= 5 else {"action": "finish", "note": "stop"}, source=long)
        reads = [e for e in self.events if e.get("action") == "read"]
        # The fourth page pushed the first out, so the fifth read returns its text again.
        self.assertEqual([e["held"] for e in reads], [False] * 5)
        self.assertEqual(reads[4]["chars"], 9000)
        held = json.loads(model.prompts[5].split("returns no text): ")[1].splitlines()[0])
        self.assertEqual([p["offset"] for p in held], [18000, 27000, 0])
        self.assertLessEqual(sum(len(p["text"]) for p in held), verify_activity.PAGE_BUDGET)

    def test_the_control_arm_is_unchanged(self):
        seen = []

        def plain(prompt):  # the control's model takes no note flag
            plain.call_id = str(len(seen))
            seen.append(prompt)
            return json.dumps({"action": "read", "path": NAME, "offset": 0} if len(seen) <= 3 else
                              {"action": "write", "path": NAME, "content": ORIGINAL} if len(seen) == 4 else
                              {"action": "check"} if len(seen) == 5 else {"action": "finish"})

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            (root / "app").mkdir(); (root / "project").mkdir()
            (root / "deadline").write_text(str(time.time() + 300))
            (root / "app" / NAME).write_text(ORIGINAL)
            events = []
            with patch.multiple(solver, APP=root / "app", WORK=root / "project", TASK=TASK, ask=plain), \
                    patch.dict(solver.CHECKS, {TASK: [sys.executable, "-c", "pass"]}), \
                    patch.object(solver, "record", side_effect=lambda **e: events.append(e)):
                obs = solver.solve("Order labels ignoring case")
        self.assertEqual(set(obs), {"files", "check_exit", "solver_calls", "first_write_call", "first_check_call"})
        self.assertFalse(any('"note"' in p or "Pages you hold" in p or "GIL Verify" in p for p in seen))
        # Every repeated read is answered with the page, as before.
        self.assertEqual(seen[3].count(json.dumps(ORIGINAL)[1:-1]), 3)
        self.assertFalse(any("note" in e or "held" in e for e in events))


if __name__ == "__main__":
    unittest.main()
