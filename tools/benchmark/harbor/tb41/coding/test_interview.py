"""Exercise the coding participant's report flow without a model or a paid trial."""
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

with patch.dict(os.environ, TB_TASK="ks-solver-cpp"):
    import participant
    import solver


def step(kind, **report):
    return {"kind": kind, "ref": "step:" + kind, "report": report}


def request(kind, interview, experiment=None):
    history = [{"steps": interview}]
    if experiment is not None:
        history.append({"steps": experiment})
    return {"task": "Use `contract.txt`", "history": history,
            "next": {"kind": kind, "cycle": "interview" if experiment is None else "experiment", "action": "close"}}


class InterviewTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name).resolve()
        for target in (solver, participant):
            ctx = patch.object(target, "APP", self.root)
            ctx.start()
            self.addCleanup(ctx.stop)

    def test_cites_public_files_only_and_marks_partial_text(self):
        (self.root / "contract.txt").write_text("a" * 9500)
        (self.root / "outside").symlink_to(self.root.parent)
        refs = participant.cited_sources("Use `contract.txt`, `outside`, `/etc/passwd` and `../missing`.")
        self.assertEqual(list(refs), ["contract.txt"])
        self.assertTrue(refs["contract.txt"]["truncated"])
        self.assertEqual(len(refs["contract.txt"]["text"]), 9000)

    def test_interview_and_revision_keep_evidence_and_gaps(self):
        (self.root / "contract.txt").write_text("Output must round trip.")
        q = step("question", question="How is round trip checked?", response="Only the compiler is available.")
        with patch.object(participant, "ask", side_effect=["Round trip required", "Round trip untested"]) as model:
            report = participant.reply(request("interpretation", [q]))["report"]
            self.assertIn("Round trip untested", report)
            prompt = model.call_args.args[0]
            self.assertIn("Output must round trip.", prompt)
            self.assertIn("g++", prompt)
        i = step("interpretation", interpretation="Round trip required", unresolved="Round trip untested")
        with patch.object(participant, "ask", side_effect=["Draft roadmap", "revise"]) as model:
            report = participant.reply(request("synthesis", [q, i]))["report"]
            self.assertIn("benchmark_revise", report)
            self.assertTrue(all("Round trip untested" in c.args[0] for c in model.call_args_list))
        with patch.object(participant, "ask", side_effect=["Clarify gap", "Pending"]) as model:
            participant.reply(request("question", [q, i, step("synthesis", statement="Draft roadmap", approved="benchmark_revise")]))
            self.assertIn("Round trip untested", model.call_args_list[0].args[0])
            self.assertIn("Draft roadmap", model.call_args_list[0].args[0])

    def test_success_criteria_reach_verify_and_analysis(self):
        interview = [step("synthesis", statement="Codec roadmap")]
        experiment = [step("define", success_condition="Bytes round trip"), step("hypothesis", hypothesis="Encode then decode")]
        with patch.object(participant, "solve", return_value={}) as solve:
            participant.reply(request("verify", interview, experiment))
            self.assertIn("Bytes round trip", solve.call_args.args[1])
        (self.root / "observation.json").write_text(json.dumps({"files": {"codec.cpp": "digest"}, "check_exit": 0}))
        with patch.object(participant, "WORK", self.root), patch.object(participant, "ask", side_effect=["Only compile observed", "incomplete"]) as model:
            report = participant.reply(request("analysis", interview, experiment))["report"]
            self.assertIn("success_condition_met: |\n  no", report)
            self.assertIn("Bytes round trip", model.call_args.args[0])
