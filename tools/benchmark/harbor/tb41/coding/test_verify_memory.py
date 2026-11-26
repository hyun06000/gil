"""What a request carries from call to call: the files in view (both arms), an end to calls that
observe nothing new (both arms), and the notes of a GIL Verify."""
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

NAME = "fast_networkx.py"
SPEC = "SPEC.md"
ORIGINAL = "def order(labels): return sorted(labels)\n"
FIXED = "def order(labels): return sorted(labels, key=str.casefold)\n"
TASK = "vf2-speedup-networkx"


def view_of(prompt):
    line = next(l for l in prompt.splitlines() if l.startswith("Files in view"))
    return json.loads(line.split("brings nothing new): ", 1)[1])


def results_of(prompt):
    return json.loads(next(l for l in prompt.splitlines() if l.startswith("Recent tool results: "))[len("Recent tool results: "):])


class Script:
    def __init__(self, step, noted):
        self.step, self.prompts, self.flags, self.noted = step, [], [], noted

    def __call__(self, prompt, **options):
        self.call_id = str(len(self.prompts))
        self.prompts.append(prompt)
        self.flags.append(options)
        action = dict(self.step(len(self.prompts), prompt))
        if not self.noted:
            action.pop("note", None)
        return json.dumps(action)


class MemoryTests(unittest.TestCase):
    def run_solver(self, step, *, cycle="cycle:C3", source=ORIGINAL, files=None):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        root = Path(temp.name).resolve()
        self.app, self.work = root / "app", root / "project"
        self.app.mkdir()
        self.work.mkdir()
        (root / "deadline").write_text(str(time.time() + 300))
        (self.app / NAME).write_text(source)
        for name, text in (files or {SPEC: "Sort ignoring case. Keep duplicates.\n"}).items():
            (self.app / name).write_text(text)
        self.runs = root / "runs"
        command = [sys.executable, "-B", "-c",
                   f"open({str(self.runs)!r}, 'a').write('x'); from fast_networkx import order; "
                   "assert order(['B','a']) == ['a','B'], 'case order'"]
        baseline = {"files": {NAME: hashlib.sha256(source.encode()).hexdigest()}}
        self.events, model = [], Script(step, noted=True)
        with patch.multiple(solver, APP=self.app, WORK=self.work, TASK=TASK, ask=model), \
                patch.dict(solver.CHECKS, {TASK: command}), \
                patch.object(solver, "record", side_effect=lambda **e: self.events.append(e)):
            options = dict(cycle=cycle, baseline=baseline, verify_ref="step:C3/S3") if cycle else {}
            obs = solver.solve("Order labels ignoring case", **options)
        return obs, model

    def tools(self, action=None):
        return [e for e in self.events if e.get("event") == "tool" and action in (None, e.get("action"))]

    def test_every_file_read_stays_in_view_whole_and_current_in_both_arms(self):
        for cycle in (None, "cycle:C3"):
            with self.subTest(arm="on" if cycle else "off"):
                obs, model = self.run_solver(lambda n, p: [
                    {"action": "read", "path": SPEC, "offset": 0, "note": "the spec asks for case-insensitive order"},
                    {"action": "read", "path": NAME, "offset": 0, "note": "sorted() has no key"},
                    {"action": "read", "path": SPEC, "offset": 0, "note": "second look"},
                    {"action": "edit", "path": NAME, "old": "sorted(labels)", "new": "sorted(labels, key=str.casefold)", "note": "add the key"},
                    {"action": "check", "note": "expect exit 0"},
                    {"action": "finish", "note": "done"}][n - 1], cycle=cycle)
                prompts = model.prompts
                self.assertEqual(view_of(prompts[0]), [])
                # The specification and the code are in one request, side by side.
                self.assertEqual([(p["path"], p["text"]) for p in view_of(prompts[2])],
                                 [(SPEC, "Sort ignoring case. Keep duplicates.\n"), (NAME, ORIGINAL)])
                # The results keep a receipt, not a second copy of the text.
                self.assertEqual(results_of(prompts[2])[-1], ["read", NAME, f"Now in view: {NAME} [0,41) of 41 characters"])
                self.assertEqual(prompts[2].count("def order(labels): return sorted(labels)"), 1)
                # Reading a file in view again brings a refusal, and the file stays where it was.
                reads = self.tools("read")
                self.assertEqual([e["shown"] for e in reads], [False, False, True])
                self.assertTrue(reads[2]["result"].startswith(f"Already in view and unchanged: {SPEC} [0,37)"))
                self.assertEqual(reads[1]["result"], ORIGINAL)
                # After the edit the view shows the new content without any read.
                self.assertEqual({p["path"]: p["text"] for p in view_of(prompts[4])}[NAME], FIXED)
                self.assertEqual(self.tools("edit")[0]["result"], "written; the file is in view as it is now")
                self.assertEqual((obs["stop"], obs["check_exit"]), ("finished", 0))

    def test_a_written_file_and_a_file_changed_by_a_command_are_in_view_without_a_read(self):
        obs, model = self.run_solver(lambda n, p: [
            {"action": "write", "path": NAME, "content": FIXED},
            {"action": "run", "command": "printf '# generated\\n' >> fast_networkx.py"},
            {"action": "read", "path": NAME, "offset": 0},   # nothing new: already shown as it is now
            {"action": "run", "command": "rm fast_networkx.py && printf 'def order(l): return sorted(l, key=str.casefold)\\n' > fast_networkx.py"},
            {"action": "check"}, {"action": "finish"}][n - 1], cycle=None)
        self.assertEqual(view_of(model.prompts[1])[0]["text"], FIXED)
        self.assertEqual(view_of(model.prompts[2])[0]["text"], FIXED + "# generated\n")
        self.assertTrue(self.tools("read")[0]["shown"])
        self.assertEqual(view_of(model.prompts[4])[0]["text"], "def order(l): return sorted(l, key=str.casefold)\n")
        self.assertEqual(obs["stop"], "finished")

    def test_the_view_holds_many_files_at_once_and_drops_the_least_recently_touched_past_its_budget(self):
        big = {f"part{i}.txt": f"{i}" * 50000 for i in range(6)}   # 300,000 characters, over the 240,000 budget
        order = [f"part{i}.txt" for i in range(6)] + ["part0.txt"]
        obs, model = self.run_solver(lambda n, p: {"action": "read", "path": order[n - 1], "offset": 0} if n <= 7 else
                                     {"action": "write", "path": NAME, "content": FIXED} if n == 8 else
                                     {"action": "check"} if n == 9 else {"action": "finish"}, cycle=None, files=big)
        # Four files (200,000 characters) fit; the fifth pushed the first out, so reading it again shows it.
        self.assertEqual([p["path"] for p in view_of(model.prompts[4])], ["part0.txt", "part1.txt", "part2.txt", "part3.txt"])
        self.assertEqual([p["path"] for p in view_of(model.prompts[5])], ["part1.txt", "part2.txt", "part3.txt", "part4.txt"])
        self.assertEqual([e["shown"] for e in self.tools("read")], [False] * 7)
        self.assertLessEqual(sum(len(p["text"]) for p in view_of(model.prompts[7])), solver.VIEW_BUDGET)
        # A file longer than one page is read by offset, and both pages are held.
        long = "".join(f"line {i:06d}\n" for i in range(6000))    # 72,000 characters
        obs, model = self.run_solver(lambda n, p: [{"action": "read", "path": NAME, "offset": 0},
                                                   {"action": "read", "path": NAME, "offset": 60000},
                                                   {"action": "check"}][min(n, 3) - 1], cycle=None, source=long)
        self.assertEqual([(p["offset"], p["end"], p["chars"]) for p in view_of(model.prompts[2])], [(0, 60000, 72000), (60000, 72000, 72000)])

    def test_results_are_kept_whole_or_not_at_all(self):
        history = [["run", f"cmd {i}", "x" * 9000] for i in range(40)]
        kept = solver.recent(history)
        self.assertEqual(kept, history[-len(kept):])
        self.assertLessEqual(len(json.dumps(kept)), solver.RESULTS_BUDGET)
        self.assertEqual(len(kept), 6)
        self.assertEqual(solver.recent([["run", "one", "y" * 200000]]), [["run", "one", "y" * 200000]])
        self.assertEqual(len(solver.recent([["read", "a", "ok"]] * 50)), solver.RESULTS_KEPT)

    def test_calls_that_observe_nothing_new_are_warned_about_and_then_end_the_editing_in_both_arms(self):
        for cycle in (None, "cycle:C3"):
            with self.subTest(arm="on" if cycle else "off"):
                obs, model = self.run_solver(lambda n, p: {"action": "read", "path": NAME, "offset": 0, "note": f"look {n}"}, cycle=cycle)
                limit = solver.STALL_LIMIT
                # One new file, then `limit` reads that bring nothing; without the limit this runs to the deadline.
                self.assertEqual((obs["solver_calls"], obs["stop"], obs["check_exit"], obs["first_write_call"]),
                                 (1 + limit, "stalled", None, None))
                self.assertEqual(self.events[-1]["event"], "verify_stalled" if cycle else "stalled")
                warned = [i for i, p in enumerate(model.prompts) if "calls in a row brought no new file text" in p]
                self.assertEqual(warned, list(range(solver.STALL_WARNING + 1, limit + 1)))
                self.assertIn(f"After {limit - solver.STALL_WARNING} more, editing ends", model.prompts[warned[0]])

    def test_a_check_without_a_change_is_not_run_again_and_a_return_to_earlier_content_is_not_progress(self):
        other = "def order(labels): return sorted(labels, key=str.lower)\n"

        def step(n, prompt):
            if n <= 3:
                return {"action": "check"}
            # Then alternate between two contents the session has already had.
            return {"action": "write", "path": NAME, "content": other if n % 2 == 0 else ORIGINAL}

        for cycle in (None, "cycle:C3"):
            with self.subTest(arm="on" if cycle else "off"):
                obs, model = self.run_solver(step, cycle=cycle)
                # The check ran once (it fails: 'B' sorts before 'a' without a key); two repeats were not run.
                self.assertEqual(self.runs.read_text(), "x")
                self.assertTrue(self.tools("check")[1]["result"].startswith(
                    "Not run again: no file changed since this check. Change a file if a requirement is still open, "
                    "otherwise finish. exit 1"))
                # Call 4 writes new content (progress); every later write returns to content already seen.
                self.assertEqual(len(self.tools("write")), 1 + solver.STALL_LIMIT)
                self.assertEqual(obs["stop"], "stalled")

    def test_a_new_file_a_new_content_or_a_new_command_result_each_restart_the_count(self):
        contents = iter(FIXED + f"# {i}\n" for i in range(99))
        names = iter(f"note{i}.txt" for i in range(99))

        def step(n, prompt):
            if n > 40:
                return {"action": "finish"}
            kind = n % 4
            if kind == 1:
                return {"action": "read", "path": next(names), "offset": 0}
            if kind == 2:
                return {"action": "run", "command": f"echo probe {n}"}
            if kind == 3:
                return {"action": "write", "path": NAME, "content": next(contents)}
            return {"action": "check"}

        obs, model = self.run_solver(step, cycle=None, files={f"note{i}.txt": f"note {i}\n" for i in range(12)})
        self.assertEqual((obs["stop"], obs["solver_calls"]), ("finished", 41))
        self.assertFalse(any("calls in a row" in p for p in model.prompts))

    def test_both_arms_carry_notes_and_only_a_verify_carries_its_receipts(self):
        step = lambda n, p: [{"action": "read", "path": NAME, "offset": 0, "note": "sorted() has no key; add casefold next"},
                             {"action": "write", "path": NAME, "content": FIXED, "note": "wrote the casefold key; check it"},
                             {"action": "check", "note": "expect exit 0"},
                             {"action": "finish", "note": "done"}][n - 1]
        obs, model = self.run_solver(step)
        self.assertEqual(model.flags, [{"notes": True}] * 4)
        self.assertIn("sorted() has no key; add casefold next", model.prompts[1])
        self.assertIn("wrote the casefold key; check it", model.prompts[3])
        self.assertEqual((obs["notes"], obs["refused_reads"], obs["repeat_reads"]), (4, 0, 0))
        self.assertEqual(self.tools("check")[0]["note"], "expect exit 0")
        self.assertIn("GIL Verify activity", model.prompts[0])
        # The control remembers its own notes in the same way; the Verify receipts are not its to carry.
        obs, model = self.run_solver(step, cycle=None)
        self.assertEqual(model.flags, [{"notes": True}] * 4)
        self.assertIn("sorted() has no key; add casefold next", model.prompts[1])
        self.assertIn("wrote the casefold key; check it", model.prompts[3])
        self.assertFalse(any("GIL Verify" in p for p in model.prompts))
        self.assertEqual(set(obs), {"files", "check_exit", "solver_calls", "first_write_call", "first_check_call", "stop", "notes"})
        self.assertEqual((obs["notes"], self.tools("check")[0]["note"]), (4, "expect exit 0"))
        # Notes are bounded: the oldest fall out of a request, and each is cut to its limit.
        obs, model = self.run_solver(lambda n, p: {"action": "run", "command": f"echo {n}", "note": f"step {n} " + "x" * 900} if n <= 20 else
                                     {"action": "write", "path": NAME, "content": FIXED} if n == 21 else
                                     {"action": "check"} if n == 22 else {"action": "finish"}, cycle=None)
        kept = json.loads(next(l for l in model.prompts[20].splitlines() if l.startswith("Your notes so far"))[len("Your notes so far, oldest first: "):])
        self.assertEqual((len(kept), kept[0]["call"], len(kept[0]["note"])), (solver.NOTES_KEPT, 5, solver.NOTE_LIMIT))


if __name__ == "__main__":
    unittest.main()
