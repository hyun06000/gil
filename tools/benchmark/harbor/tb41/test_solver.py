import json
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import mock_open, patch

import solver


class SolverSubmissionTest(unittest.TestCase):
    def test_early_finish_and_empty_candidate_are_retried_before_submission(self):
        rule = {"name": "change", "src": "b", "tgt": "p", "left": "", "right": ""}
        replies = [
            {"action": "finish"},
            {"action": "evaluate", "rules": []},
            {"action": "evaluate", "rules": [rule]},
            {"action": "finish"},
        ]
        with tempfile.TemporaryDirectory() as temp:
            work = Path(temp)
            (work / "deadline").write_text(str(time.time() + 200))
            with (
                patch.object(solver, "WORK", work),
                patch.object(solver, "DEADLINE", work / "deadline"),
                patch("builtins.open", mock_open(read_data="ba\tpa\nbi\tpe\n")),
                patch.object(
                    solver, "ask", side_effect=[json.dumps(r) for r in replies]
                ),
                patch.object(
                    solver,
                    "evaluate",
                    side_effect=[
                        (0, [("ba", "ba", "pa")], solver.Counter()),
                        (1, [("bi", "pi", "pe")], solver.Counter()),
                    ],
                ),
                patch.object(solver, "submit") as submit,
            ):
                observation = solver.solve("Infer ordered rules")
            submit.assert_called_once_with([rule])
            self.assertEqual(observation["rejected_empty_finishes"], 1)
            self.assertEqual(observation["evaluated"], 1)
            self.assertEqual(observation["solver_calls"], 4)
