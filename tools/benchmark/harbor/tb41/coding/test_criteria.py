"""The criteria ledger: what an item's status means, how a Verify fills it, and what the participant makes of it."""
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
    import participant
import criteria
from criteria import Ledger, digest, parse

NAME = "fast_networkx.py"
BASE = "def order(labels):\n    return sorted(labels, key=str.casefold)\n"
GUARDED = "def order(labels):\n    if not isinstance(labels, list):\n        raise TypeError('list')\n    return sorted(labels, key=str.casefold)\n"
PY = sys.executable + " -B -c "
REFUSES = PY + "\"import sys\nfrom fast_networkx import order\ntry:\n    order('ab')\nexcept TypeError:\n    sys.exit(0)\nsys.exit(1)\""
KEEPS = PY + "\"from fast_networkx import order\nl=['b','A']\norder(l)\nassert l == ['b','A']\""
RAW = [{"requirement": "duplicates are kept", "kind": "behaviour", "probe": "call it"},
       {"requirement": "the input is not modified", "kind": "invariant", "source": "task: input", "probe": "compare"},
       {"requirement": "a non-list is refused", "kind": "failure", "probe": "pass a string"},
       {"requirement": "it is fast on the hidden cases", "kind": "behaviour", "probe": None}]


class LedgerTests(unittest.TestCase):
    def test_items_come_from_a_json_array_and_a_bad_answer_says_what_to_repair(self):
        items = parse("```json\n" + json.dumps(RAW + [{"requirement": "x", "kind": "odd"}]) + "\n```")
        self.assertEqual([(i["id"], i["kind"], i["status"]) for i in items],
                         [("K1", "behaviour", "unrun"), ("K2", "invariant", "unrun"), ("K3", "failure", "unrun"),
                          ("K4", "behaviour", "unrun"), ("K5", "behaviour", "unrun")])
        self.assertEqual((items[1]["source"], items[3]["plan"]), ("task: input", None))
        # Nothing caps or shortens what the model listed.
        long = [{"requirement": "r" * 900 + str(i), "source": "s" * 700, "probe": "p" * 900} for i in range(40)]
        self.assertEqual([(len(i["requirement"]), len(i["source"]), len(i["plan"])) for i in parse(json.dumps(long))][::39], [(901, 700, 900), (902, 700, 900)])
        for bad, why in (("I think", "not a JSON array"), ("[]", "non-empty"), ('{"a":1}', "non-empty"),
                         ('["x"]', "is an object"), ('[{"kind":"failure"}]', "needs a requirement")):
            with self.subTest(bad=bad), self.assertRaisesRegex(ValueError, why):
                parse(bad)

    def test_a_result_belongs_to_the_submission_it_was_seen_on(self):
        ledger = Ledger(parse(json.dumps(RAW)))
        a, b = digest({NAME: "1"}), digest({NAME: "2"})
        self.assertEqual(ledger.counts(a), {"unrun": 3, "unverifiable": 1})
        self.assertEqual(ledger.record("K3", "cmd", 0, "ok", a), "holds")
        self.assertEqual((ledger.status("K3", a), ledger.status("K3", b)), ("holds", "stale"))
        self.assertEqual(ledger.record("K2", "cmd2", "none, stopped after 60 s", "", a), "fails")
        self.assertEqual(ledger.open(a), ["K1", "K2"])
        self.assertEqual(ledger.open(b), ["K1", "K2", "K3"])
        self.assertEqual(ledger.having(a, "holds"), ["K3"])
        rows = {r["id"]: r for r in ledger.shown(a, ["K2"])}
        self.assertEqual((rows["K2"]["status"], rows["K2"]["target"], rows["K2"]["probe"]), ("fails", True, "cmd2"))
        self.assertEqual((rows["K1"]["probe_plan"], rows["K4"]["status"]), ("call it", "unverifiable"))

    def test_no_count_decides_anything_and_setting_a_criterion_aside_is_a_judgement_that_a_passing_probe_undoes(self):
        ledger = Ledger(parse(json.dumps(RAW)))
        versions = [digest({NAME: str(i)}) for i in range(8)]
        for version in versions:
            ledger.record("K1", "cmd", 1, "no", version)
        ledger.record("K1", "cmd", 1, "no", versions[-1])     # the same submission again is one failure
        # Eight failures later the criterion is simply still failing; the history is shown, not acted on.
        row = ledger.shown(versions[-1])[0]
        self.assertEqual((ledger.status("K1", versions[-1]), row["failed_on_submissions"], "K1" in ledger.open(versions[-1])),
                         ("fails", 8, True))
        self.assertNotIn("notice", row)
        ledger.set_aside("K1", "the probe asserts an order the source does not require")
        self.assertEqual((ledger.status("K1", versions[-1]), "K1" in ledger.open(versions[-1])), ("set_aside", False))
        self.assertIn("does not require", ledger.shown(versions[-1])[0]["set_aside"])
        self.assertEqual(ledger.record("K1", "better", 0, "", versions[-1]), "holds")
        self.assertEqual(ledger.status("K1", versions[-1]), "holds")


class VerifyLedgerTests(unittest.TestCase):
    def drive(self, actions, *, criteria=None, targets=(), source=GUARDED, cycle="cycle:C3"):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        root = Path(temp.name).resolve()
        self.app, self.work = root / "app", root / "project"
        self.app.mkdir(); self.work.mkdir()
        (root / "deadline").write_text(str(time.time() + 400))
        (self.app / NAME).write_text(source)
        self.events, self.prompts, self.flags = [], [], []
        script = iter(actions)

        def model(prompt, **options):
            model.call_id = str(len(self.prompts))
            self.prompts.append(prompt)
            self.flags.append(options)
            return json.dumps(next(script))

        check = [sys.executable, "-B", "-c", "from fast_networkx import order; assert order(['a','B']) == ['a','B']"]
        options = dict(cycle=cycle, baseline={"files": {NAME: hashlib.sha256(source.encode()).hexdigest()}},
                       verify_ref="step:C3/S3", criteria=criteria, targets=targets) if cycle else {}
        with patch.multiple(solver, APP=self.app, WORK=self.work, TASK="vf2-speedup-networkx", ask=model), \
                patch.dict(solver.CHECKS, {"vf2-speedup-networkx": check}), \
                patch.object(solver, "record", side_effect=lambda **e: self.events.append(e)):
            return solver.solve("Order labels", **options)

    def tools(self):
        return [e for e in self.events if e.get("event") == "tool"]

    def test_a_probe_is_recorded_against_its_criterion_and_goes_stale_when_the_files_change(self):
        items = parse(json.dumps(RAW))
        obs = self.drive([
            {"action": "run", "command": REFUSES, "criterion": "K3"},
            {"action": "run", "command": "echo no criterion", "criterion": ""},
            {"action": "run", "command": "true", "criterion": "K9"},
            {"action": "edit", "path": NAME, "old": "'list'", "new": "'a list is required'"},
            {"action": "run", "command": KEEPS, "criterion": "K2"},
            {"action": "check"}, {"action": "finish"}], criteria=items, targets=["K3", "K2"])
        tools = self.tools()
        self.assertTrue(tools[0]["result"].startswith("criterion K3 holds on the current files. exit 0"))
        self.assertEqual(tools[0]["criterion"], "K3")
        self.assertNotIn("criterion", tools[1])
        self.assertIn("K9 is not a criterion of the ledger", tools[2]["error"])
        shown = lambda prompt: {r["id"]: r for r in json.loads(prompt.splitlines()[-3])}
        self.assertEqual(self.flags[0], {"notes": True, "ledger": True})
        self.assertIn("Targets of this experiment: ['K3', 'K2']", self.prompts[0])
        self.assertEqual((shown(self.prompts[1])["K3"]["status"], shown(self.prompts[4])["K3"]["status"]), ("holds", "stale"))
        # Finishing is the model's judgement: nothing holds it back, and the ledger says what it left.
        self.assertEqual((tools[6]["result"], obs["stop"]), ("finished", "finished"))
        self.assertEqual(obs["swept"], [{"criterion": "K3", "before": "stale", "after": "holds"}])

    def test_the_closing_sweep_settles_what_went_stale_and_leaves_what_was_never_probed(self):
        items = parse(json.dumps(RAW))
        obs = self.drive([
            {"action": "run", "command": REFUSES, "criterion": "K3"},
            {"action": "run", "command": KEEPS, "criterion": "K2"},
            {"action": "edit", "path": NAME, "old": "'list'", "new": "'a list is required'"},   # both go stale
            {"action": "finish"},                                   # the harness asks for a check after a change
            {"action": "check"},
            {"action": "finish"},
        ], criteria=items, targets=["K3", "K2", "K1"])
        self.assertIn("check before finish", self.tools()[3]["error"])
        self.assertEqual(obs["stop"], "finished")
        # Both stored probes were run again on the final files without a model call; K1 had none.
        self.assertEqual(obs["swept"], [{"criterion": "K2", "before": "stale", "after": "holds"},
                                        {"criterion": "K3", "before": "stale", "after": "holds"}])
        self.assertEqual((obs["ledger"], obs["targets"]), ({"unrun": 1, "holds": 2, "unverifiable": 1}, ["K3", "K2", "K1"]))
        self.assertEqual(obs["submission_digest"], digest(obs["files"]))
        self.assertEqual(self.events[-1]["event"], "probe_sweep")

    def test_the_sweep_finds_a_criterion_that_a_later_change_broke_and_runs_the_local_check_left_pending(self):
        items = parse(json.dumps(RAW))
        obs = self.drive([
            {"action": "run", "command": REFUSES, "criterion": "K3"},
            {"action": "write", "path": NAME, "content": BASE},     # removes the refusal
            {"action": "run", "command": KEEPS, "criterion": "K2"},
        ] + [{"action": "run", "command": "true", "criterion": ""}] * 20, criteria=items, targets=["K2"])
        self.assertEqual(obs["stop"], "stalled")
        self.assertEqual(obs["swept"], [{"criterion": "K3", "before": "stale", "after": "fails"}])
        self.assertEqual(obs["check_exit"], 0)                      # never asked for; run by the sweep
        by = {i["id"]: i for i in obs["criteria"]}
        self.assertEqual((by["K3"]["status"], by["K2"]["status"], by["K3"]["digest"]), ("fails", "holds", obs["submission_digest"]))

    def test_without_a_ledger_nothing_of_it_reaches_the_request(self):
        for cycle in (None, "cycle:C3"):
            with self.subTest(cycle=cycle):
                obs = self.drive([{"action": "run", "command": "echo hi"}, {"action": "write", "path": NAME, "content": BASE},
                                  {"action": "check"}, {"action": "finish"}], cycle=cycle)
                self.assertEqual(self.flags, [{"notes": True}] * 4)
                self.assertFalse(any("Criteria ledger" in p or "criterion" in p for p in self.prompts))
                self.assertFalse({"criteria", "targets", "swept", "ledger"} & set(obs))
                self.assertFalse(any(e.get("event") in ("probe", "probe_sweep") for e in self.events))


class ParticipantLedgerTests(unittest.TestCase):
    def test_a_ledger_is_derived_with_one_repair_and_given_up_after_two_bad_answers(self):
        answers = iter(["not json", json.dumps(RAW)])
        asked = []
        with patch.object(participant, "cited_sources", return_value={}):
            items = participant.derive(lambda prompt: asked.append(prompt) or next(answers), "task", "spec")
            self.assertEqual([i["id"] for i in items], ["K1", "K2", "K3", "K4"])
            self.assertIn("could not be used (not a JSON array", asked[1])
            self.assertIn("List first every failure case", asked[0])
            self.assertIsNone(participant.derive(lambda prompt: "still not json", "task", "spec"))

    def test_the_model_chooses_the_targets_and_only_a_failed_experiment_s_reflection_sets_a_criterion_aside(self):
        entry = digest({NAME: "1"})
        asked = []
        def model(answer):
            return lambda prompt: asked.append(prompt) or answer
        ledger = Ledger(parse(json.dumps(RAW)))
        clock = patch.object(participant, "minutes_left", return_value=41.6)
        clock.start()
        self.addCleanup(clock.stop)
        picked = participant.choose(model(json.dumps({"targets": ["K3", "K9", "K3", "K4"], "set_aside": {"K1": "later"}})),
                                    ledger, entry, "Earlier: K3 failed twice")
        # Unknown ids, repeats and a criterion nothing local can show are not targets. What the
        # choice does not name stays open: choosing cannot set anything aside.
        self.assertEqual(picked, ["K3"])
        self.assertEqual(ledger.open(entry), ["K1", "K2", "K3"])
        self.assertIn("Earlier: K3 failed twice", asked[0])
        self.assertIn("Every criterion you do not name stays open", asked[0])
        # The model is told the time left and judges with it; no threshold decides for it.
        self.assertIn("About 42 minutes of the trial are left.", asked[0])
        for unusable in ("no idea", "[]", json.dumps({"targets": []}), json.dumps({"targets": ["K9"]})):
            with self.subTest(answer=unusable):
                self.assertEqual(participant.choose(model(unusable), ledger, entry, ""), ["K1", "K2", "K3"])
        # The reflection that closed a failed Experiment may name a criterion as wrong; its record carries it.
        closed = lambda lesson: {"steps": [{"kind": "outcome", "report": {"verdict": "failure", "lesson": lesson}}]}
        self.assertEqual(participant.set_aside_by(closed('The probe was wrong.\nSet aside: {"K2": "the source allows a copy"}')),
                         {"K2": "the source allows a copy"})
        for none in (closed("No criterion was wrong.\nSet aside: {}"), closed("An older record"), {"steps": []}, None):
            self.assertEqual(participant.set_aside_by(none), {})

    def test_an_experiment_starts_from_its_ancestor_s_ledger_with_the_abandoned_one_s_probes(self):
        held, lost = parse(json.dumps(RAW)), parse(json.dumps(RAW))
        a, b = digest({NAME: "1"}), digest({NAME: "2"})
        Ledger(held).record("K3", REFUSES, 0, "", a)
        abandoned = Ledger(lost)
        abandoned.record("K3", REFUSES, 1, "", b)
        abandoned.record("K2", KEEPS, 0, "", b)
        result = lambda items: {"steps": [{"kind": "verify", "report": {"result": json.dumps({"criteria": items})}}]}
        cycles = {"cycle:C3": {**result(held), "parent": "cycle:C2"}, "cycle:C2": {"steps": [], "parent": None}}
        items = {i["id"]: i for i in participant.entry_ledger(cycles, cycles["cycle:C3"], result(lost))}
        # What held in the world the run went back to still holds there; the lost world's failure is remembered.
        self.assertEqual((items["K3"]["status"], items["K3"]["digest"], items["K3"]["failed_on"]), ("holds", a, [b]))
        # A probe only the abandoned Experiment wrote is kept; its result is stale in the restored world.
        self.assertEqual((items["K2"]["command"], Ledger(list(items.values())).status("K2", a)), (KEEPS, "stale"))
        # With no ledger above, the abandoned Experiment's own is the starting point.
        self.assertEqual(participant.entry_ledger(cycles, cycles["cycle:C2"], result(lost))[2]["failed_on"], [b])
        self.assertIsNone(participant.entry_ledger(cycles, cycles["cycle:C2"], None))


if __name__ == "__main__":
    unittest.main()
