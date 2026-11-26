import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

with patch.dict(os.environ, TB_TASK="ks-solver-cpp"):
    import solver


class FinalCheckTests(unittest.TestCase):
    def run_case(self, actions, command, plan="", reviews=()):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            responses = iter(actions)
            now = [0]
            review_answers = iter(reviews)
            def ask(prompt, choices=(), stage="solver"):
                if stage == "checklist":
                    return "edit | - | Implement library\nedit | 1 | Connect caller\nverify | 2 | Verify external behavior"
                if stage == "checklist_review":
                    return next(review_answers)
                action = next(responses)
                now[0] += 20
                return json.dumps(action)
            ask.call_id = "fixture"
            with patch.object(solver, "APP", root), patch.object(solver, "WORK", root), patch.object(solver, "TASK", "react-lead-form"), patch.object(solver, "record"), patch.object(solver, "ask", ask), patch.dict(solver.CHECKS, {"react-lead-form":command}), patch.object(solver.time, "time", side_effect=lambda: now[0]), patch.object(Path, "read_text", return_value=str((1000 if plan else 190) + 20 * (len(actions)-1) + 1)):
                return solver.solve("fixture", plan)

    def test_rechecks_after_last_edit_even_without_finish(self):
        write = {"action":"write","path":"src/a.ts","content":"first"}
        obs = self.run_case([write, {"action":"check"}, dict(write,content="last")], ["true"])
        self.assertEqual(obs["last_check"]["trigger"], "finalization")
        self.assertEqual(obs["last_check"]["files"], obs["files"])
        self.assertTrue(obs["check_matches_submission"])
        self.assertEqual(obs["check_exit"], 0)

    def test_does_not_repeat_current_failed_check(self):
        obs = self.run_case([{"action":"write","path":"src/a.ts","content":"a"}, {"action":"check"}, {"action":"finish"}], ["false"])
        self.assertEqual(obs["last_check"]["trigger"], "model")
        self.assertEqual(obs["check_exit"], 1)
        self.assertTrue(obs["check_matches_submission"])

    def test_independent_edit_can_precede_inspection_but_dependencies_hold(self):
        items = solver.parse_checklist("inspect | - | Audit outputs\nedit | - | Normalize\nedit | 1 | Replace ledger")
        self.assertEqual([i["id"] for i in solver.ready_items(items)], [1, 2])
        items[0]["status"] = "deferred"
        self.assertEqual([i["id"] for i in solver.ready_items(items)], [2])
        items[0]["status"] = "demonstrated"
        self.assertEqual([i["id"] for i in solver.ready_items(items)], [2, 3])
        with self.assertRaises(ValueError):
            solver.parse_checklist("edit | 1 | Self cycle")

    def test_verify_advances_without_erasing_earlier_review(self):
        write = {"action":"write","path":"src/a.ts","content":"first"}
        finish = {"action":"finish"}
        check = {"action":"check"}
        actions = [write, check, finish, finish, dict(write, content="changed"),
                   {"action":"read","path":"src/a.ts"},
                   dict(write, path="src/caller.ts"), finish, finish, check, finish]
        reviews = ["Library present", "demonstrated", "Caller is missing", "pending",
                   "Caller connected", "demonstrated",
                   "Only verifier exercises behavior", "deferred"]
        obs = self.run_case(actions, ["true"], "accepted plan", reviews)
        self.assertEqual([i["status"] for i in obs["verify_progress"]], ["demonstrated", "demonstrated", "deferred"])
        self.assertEqual(obs["verify_progress"][0]["assessment"], "Library present")
        self.assertTrue(obs["check_matches_submission"])
        self.assertEqual(obs["solver_calls"], len(actions))

    def test_read_identity_uses_full_file_version_and_character_offset(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp, "source")
            path.write_bytes(("한\r\n" + "x" * 9000 + "a").encode())
            seen = {}
            page, first = solver.read_page(path, 0, seen, 1)
            self.assertTrue(page.startswith("한\n"))
            self.assertIsNone(first["previous_read_call"])
            self.assertIsNone(solver.read_page(path, 1, seen, 2)[1]["previous_read_call"])
            self.assertEqual(solver.read_page(path, 0, seen, 3)[1]["previous_read_call"], 1)
            path.write_bytes(path.read_bytes()[:-1] + b"b")
            same_page, changed = solver.read_page(path, 0, seen, 4)
            self.assertEqual(same_page, page)
            self.assertIsNone(changed["previous_read_call"])

    def test_mutating_check_is_not_attributed_to_new_source(self):
        obs = self.run_case([{"action":"write","path":"src/a.ts","content":"a"}], ["sh","-c","echo b >> src/a.ts"])
        self.assertFalse(obs["check_matches_submission"])
        self.assertIsNone(obs["check_exit"])
