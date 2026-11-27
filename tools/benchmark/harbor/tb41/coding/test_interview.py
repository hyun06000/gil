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
        (self.root / "large.txt").write_text("b" * (solver.PAGE + 5))
        (self.root / "outside").symlink_to(self.root.parent)
        refs = participant.cited_sources("Use `contract.txt`, `large.txt`, `outside`, `/etc/passwd` and `../missing`.")
        self.assertEqual(list(refs), ["contract.txt", "large.txt"])
        # A named file is carried whole; only what one request cannot carry is marked partial.
        self.assertEqual((refs["contract.txt"]["truncated"], len(refs["contract.txt"]["text"])), (False, 9500))
        self.assertEqual((refs["large.txt"]["truncated"], len(refs["large.txt"]["text"])), (True, solver.PAGE))

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
        listed = json.dumps({"criteria": [{"requirement": "bytes round trip", "kind": "behaviour", "source": "contract.txt", "same_as": ""}]})
        with patch.object(participant, "ask", side_effect=["continue interview", "Draft roadmap", "Missing required codec mode.", "clarify", listed]) as model:
            report = participant.reply(request("synthesis", [q, i]))["report"]
            self.assertIn("benchmark_revise", report)
            self.assertTrue(all("Round trip untested" in c.args[0] for c in model.call_args_list[1:4]))
            # The interview's last act is to number the problems: the list is part of its Synthesis.
            self.assertIn("Write the numbered list of problems", model.call_args_list[4].args[0])
            self.assertIn("This is the first list", model.call_args_list[4].args[0])
            self.assertIn('Criteria: [{"id": "K1", "kind": "behaviour", "requirement": "bytes round trip"', report)
        with patch.object(participant, "ask", side_effect=["read contract.txt", "Clarify gap", "open", "Pending"]) as model:
            model.call_id = "fixture"
            with patch.object(participant, "record"):
                participant.reply(request("question", [q, i, step("synthesis", statement="Draft roadmap", approved="benchmark_revise")]))
            self.assertIn("Round trip untested", model.call_args_list[0].args[0])
            self.assertIn("Draft roadmap", model.call_args_list[0].args[0])

    def test_reads_unmentioned_file_and_reuses_observation_in_roadmap(self):
        import mailbox_client
        (self.root / "codec.cpp").write_text("a" * solver.PAGE + "END_POLICY")
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory)
            with patch.object(mailbox_client, "MAIL", log), patch.object(participant, "MAIL", log):
                with patch.object(participant, "ask", side_effect=["read codec.cpp", "read codec.cpp", f"codec.cpp (byte {solver.PAGE})", "Question", "open", "Answer"]) as model:
                    model.call_id = "fixture"
                    report = participant.reply(request("question", []))["report"]
                    self.assertNotIn("continue interview", model.call_args_list[0].args[1])
                    self.assertIn(f"Observed file: codec.cpp (byte {solver.PAGE})", report)
                    self.assertIn("END_POLICY", model.call_args_list[2].args[0])
                reads = participant.interview_reads()
                self.assertEqual([r["offset"] for r in reads], [0, solver.PAGE])
                with patch.object(participant, "ask", side_effect=["Roadmap", "First action available", "start", "no list"]) as model:
                    participant.reply(request("synthesis", [step("question", question="Q"), step("interpretation", unresolved="Gap")]))
                    self.assertIn("END_POLICY", model.call_args_list[0].args[0])
        self.assertEqual((self.root / "codec.cpp").read_text(), "a" * solver.PAGE + "END_POLICY")

    def test_rewrite_stays_in_synthesis_and_provisional_trial_starts(self):
        replies = iter(["Cannot implement now", "No plan", "rewrite",
                        "Try a reversible mesh size and compare refinement", "Can start", "start"])
        statement, decision = participant.experiment_handoff(lambda *args: next(replies), "Unknown mesh", "")
        self.assertEqual(decision, "accept")
        self.assertIn("reversible mesh", statement)
        self.assertIn("Handoff review (start)", statement)

    def test_unrepaired_draft_or_blocking_conflict_is_not_approved(self):
        # A draft is rewritten for as long as its review asks; here the step's time ends it after four.
        endless = ["Bad", "Missing action", "rewrite"] + ["Bad again", "Still missing action", "rewrite"] * 4
        for replies, time_for in ((["Draft", "Which conflicting output type is required?", "clarify"], []),
                                  (endless, [True, True, True, True, False])):
            answers = iter(replies)
            with patch.object(participant, "step_time_allows", side_effect=time_for):
                statement, decision = participant.experiment_handoff(lambda *args: next(answers), "Gap", "")
            self.assertEqual(decision, "revise")
            self.assertIn(replies[-2], statement)
            self.assertEqual(next(answers, None), None, "every scripted reply was asked for")

    def test_empty_project_question_cites_brief(self):
        with patch.object(participant, "ask", side_effect=["What output?", "open", "Unspecified"]) as model:
            report = participant.reply(request("question", []))["report"]
            self.assertIn("Task brief (no readable source observed)", report)
            self.assertEqual(model.call_count, 3)

    def test_answered_candidate_becomes_finding(self):
        with patch.object(participant, "ask", side_effect=[
                "Require a round trip?", "answered", "Contract section 1 requires it.", "another question",
                "Which representation?", "open", "Measure the two representations."]) as model:
            report = participant.reply(request("question", []))["report"]
            # Whether to ask again was the model's choice, made from what was screened.
            self.assertEqual(model.call_args_list[3].args[1], ["another question", "no open question remains"])
            self.assertIn("answered: Contract section 1", model.call_args_list[3].args[0])
            self.assertIn("Contract section 1", model.call_args_list[4].args[0])
            self.assertIn("answered: Contract section 1", report)
            self.assertIn("Which representation?", report)
            self.assertNotIn("Require a round trip?", report)

    def test_no_open_candidate_preserves_review(self):
        with patch.object(participant, "ask", side_effect=[
                "Candidate A", "answered", "Specified in brief.", "another question",
                "Candidate B", "immaterial", "Equivalent actions.", "another question",
                "Candidate C", "answered", "Also in the brief.", "no open question remains"]) as model:
            report = participant.reply(request("question", []))["report"]
            self.assertIn("No decision-changing open question", report)
            self.assertIn("immaterial: Equivalent actions.", report)
            # Three candidates were screened: the review ends when the model judges none remains.
            self.assertIn("answered: Also in the brief.", report)
            self.assertEqual(model.call_count, 12)

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
