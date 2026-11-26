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
        mail = patch.object(participant, "MAIL", self.root.parent / (self.root.name + "-absent"))
        mail.start()
        self.addCleanup(mail.stop)
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
        with patch.object(participant, "ask", side_effect=["continue interview", "Draft roadmap", "revise"]) as model:
            report = participant.reply(request("synthesis", [q, i]))["report"]
            self.assertIn("benchmark_revise", report)
            self.assertTrue(all("Round trip untested" in c.args[0] for c in model.call_args_list[1:]))
        with patch.object(participant, "ask", side_effect=["read contract.txt", "Clarify gap", "open", "Pending"]) as model:
            model.call_id = "fixture"
            with patch.object(participant, "record"):
                participant.reply(request("question", [q, i, step("synthesis", statement="Draft roadmap", approved="benchmark_revise")]))
            self.assertIn("Round trip untested", model.call_args_list[0].args[0])
            self.assertIn("Draft roadmap", model.call_args_list[0].args[0])

    def test_reads_unmentioned_file_and_reuses_observation_in_roadmap(self):
        import mailbox_client
        (self.root / "codec.cpp").write_text("a" * 6000 + "END_POLICY")
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory)
            with patch.object(mailbox_client, "MAIL", log), patch.object(participant, "MAIL", log):
                with patch.object(participant, "ask", side_effect=["read codec.cpp", "read codec.cpp", "codec.cpp (byte 6000)", "Question", "open", "Answer"]) as model:
                    model.call_id = "fixture"
                    report = participant.reply(request("question", []))["report"]
                    self.assertNotIn("continue interview", model.call_args_list[0].args[1])
                    self.assertIn("Observed file: codec.cpp (byte 6000)", report)
                    self.assertIn("END_POLICY", model.call_args_list[2].args[0])
                reads = participant.interview_reads()
                self.assertEqual([r["offset"] for r in reads], [0, 6000])
                with patch.object(participant, "ask", side_effect=["Roadmap", "accept"]) as model:
                    participant.reply(request("synthesis", [step("question", question="Q"), step("interpretation", unresolved="Gap")]))
                    self.assertIn("END_POLICY", model.call_args_list[0].args[0])
        self.assertEqual((self.root / "codec.cpp").read_text(), "a" * 6000 + "END_POLICY")

    def test_empty_project_question_cites_brief(self):
        with patch.object(participant, "ask", side_effect=["What output?", "open", "Unspecified"]) as model:
            report = participant.reply(request("question", []))["report"]
            self.assertIn("Task brief (no readable source observed)", report)
            self.assertEqual(model.call_count, 3)

    def test_answered_candidate_becomes_finding(self):
        with patch.object(participant, "ask", side_effect=[
                "Require a round trip?", "answered", "Contract section 1 requires it.",
                "Which representation?", "open", "Measure the two representations."]) as model:
            report = participant.reply(request("question", []))["report"]
            self.assertIn("Contract section 1", model.call_args_list[3].args[0])
            self.assertIn("answered: Contract section 1", report)
            self.assertIn("Which representation?", report)
            self.assertNotIn("Require a round trip?", report)

    def test_no_open_candidate_preserves_review(self):
        with patch.object(participant, "ask", side_effect=[
                "Candidate A", "answered", "Specified in brief.",
                "Candidate B", "immaterial", "Equivalent actions."]):
            report = participant.reply(request("question", []))["report"]
            self.assertIn("No decision-changing open question", report)
            self.assertIn("immaterial: Equivalent actions.", report)

    def test_calendar_hours_do_not_establish_rounding_direction(self):
        page = {"path": "calendar", "offset": 0, "result": "openTimeUtc=09:00"}
        candidate = {"claim": "Weekend rolls forward", "path": "calendar", "quote": "openTimeUtc=09:00"}
        with patch.object(participant, "ask", side_effect=[json.dumps([candidate]), "insufficient"]) as model:
            self.assertEqual(participant.reviewed_promotions(model, "Direction unknown", [page], []), [])
        for old, quote in [([], "next opening"), ([page["result"]], page["result"])]:
            with patch.object(participant, "ask", return_value=json.dumps([dict(candidate, quote=quote)])) as model:
                self.assertEqual(participant.reviewed_promotions(model, "Direction unknown", [page], old), [])
                self.assertEqual(model.call_count, 1)
        page["result"] = "Weekend rolls forward to next opening"
        candidate["quote"] = page["result"]
        with patch.object(participant, "ask", side_effect=[json.dumps([candidate]), "supported"]) as model:
            self.assertEqual(participant.reviewed_promotions(model, "Direction unknown", [page], []),
                             [dict(candidate, offset=0)])

    def test_analysis_rechecks_changed_evidence_without_destroying_progress(self):
        (self.root / "lib.cpp").write_text("int solve(){return 3;}")
        progress = [{"criterion": c, "status": "demonstrated", "sources": {"lib.cpp": "old"}}
                    for c in ("Signature exists", "Accuracy threshold")]
        observed = {"files": {"lib.cpp": "new"}, "verify_progress": progress}
        result = participant.analysis_checklist(observed, lambda prompt: json.dumps([
            {"index": 0, "path": "lib.cpp", "quote": "int solve()", "reason": "Signature exists"},
            {"index": 1, "path": "lib.cpp", "quote": "nonexistent test result", "reason": "Passed"}]))
        self.assertEqual([item["status"] for item in result], ["demonstrated", "recheck"])
        self.assertEqual(progress[1]["status"], "demonstrated")

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
