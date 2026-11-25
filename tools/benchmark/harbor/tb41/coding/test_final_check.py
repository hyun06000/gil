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
                    return "Implement library\nConnect caller\nVerify external behavior"
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

    def test_finish_advances_items_and_reopens_changed_source(self):
        write = {"action":"write","path":"src/a.ts","content":"first"}
        finish = {"action":"finish"}
        check = {"action":"check"}
        actions = [write, check, finish, finish, dict(write, content="changed"),
                   {"action":"read","path":"src/a.ts"}, finish,
                   dict(write, path="src/caller.ts"), finish, finish, check, finish]
        reviews = ["Library present", "demonstrated", "Caller is missing", "pending",
                   "Library inspected again", "demonstrated", "Caller connected", "demonstrated",
                   "Only verifier exercises behavior", "deferred"]
        obs = self.run_case(actions, ["true"], "accepted plan", reviews)
        self.assertEqual([i["status"] for i in obs["checklist"]], ["demonstrated", "demonstrated", "deferred"])
        self.assertEqual(obs["checklist"][0]["assessment"], "Library inspected again")
        self.assertTrue(obs["check_matches_submission"])
        self.assertEqual(obs["solver_calls"], len(actions))

    def test_mutating_check_is_not_attributed_to_new_source(self):
        obs = self.run_case([{"action":"write","path":"src/a.ts","content":"a"}], ["sh","-c","echo b >> src/a.ts"])
        self.assertFalse(obs["check_matches_submission"])
        self.assertIsNone(obs["check_exit"])
