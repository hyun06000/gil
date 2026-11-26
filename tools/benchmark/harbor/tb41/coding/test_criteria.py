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
        self.assertEqual(len(parse(json.dumps([{"requirement": f"r{i}"} for i in range(40)]))), criteria.LIMIT)
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
        # Failure cases are taken first, then what must not change, then behaviour; skipped ones wait.
        self.assertEqual(ledger.targets(b), ["K3", "K2", "K1"])
        self.assertEqual(ledger.targets(b, {"K3"}), ["K2", "K1", "K3"])
        rows = {r["id"]: r for r in ledger.shown(a, ["K2"])}
        self.assertEqual((rows["K2"]["status"], rows["K2"]["target"], rows["K2"]["probe"]), ("fails", True, "cmd2"))
        self.assertEqual((rows["K1"]["probe_plan"], rows["K4"]["status"]), ("call it", "unverifiable"))

    def test_a_probe_that_fails_on_every_submission_is_questioned_and_then_set_aside(self):
        ledger = Ledger(parse(json.dumps(RAW)))
        versions = [digest({NAME: str(i)}) for i in range(5)]
        for version in versions[:2]:
            ledger.record("K1", "cmd", 1, "no", version)
        ledger.record("K1", "cmd", 1, "no", versions[1])     # the same submission again counts once
        self.assertFalse(ledger.questioned("K1"))
        ledger.record("K1", "cmd", 1, "no", versions[2])
        self.assertTrue(ledger.questioned("K1"))
        self.assertIn("Compare what it asserts", ledger.shown(versions[2])[0]["notice"])
        ledger.record("K1", "cmd", 1, "no", versions[3])
        self.assertEqual((ledger.status("K1", versions[3]), "K1" in ledger.open(versions[3])), ("disputed", False))
        # A corrected probe that passes settles it.
        self.assertEqual(ledger.record("K1", "better", 0, "", versions[3]), "holds")
        self.assertEqual(ledger.status("K1", versions[3]), "holds")


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
            {"action": "check"}, {"action": "finish"}, {"action": "finish"}], criteria=items, targets=["K3", "K2"])
        tools = self.tools()
        self.assertTrue(tools[0]["result"].startswith("criterion K3 holds on the current files. exit 0"))
        self.assertEqual(tools[0]["criterion"], "K3")
        self.assertNotIn("criterion", tools[1])
        self.assertIn("K9 is not a criterion of the ledger", tools[2]["error"])
        shown = lambda prompt: {r["id"]: r for r in json.loads(prompt.splitlines()[-3])}
        self.assertEqual(self.flags[0], {"notes": True, "ledger": True})
        self.assertIn("Targets of this experiment: ['K3', 'K2']", self.prompts[0])
        self.assertEqual((shown(self.prompts[1])["K3"]["status"], shown(self.prompts[4])["K3"]["status"]), ("holds", "stale"))
        # K3 was seen on the earlier files only, so the first finish says so; the second closes the step.
        self.assertIn("Targets not holding on the current files: ['K3']", tools[6]["error"])
        self.assertEqual(obs["stop"], "finished")
        self.assertEqual(obs["swept"], [{"criterion": "K3", "before": "stale", "after": "holds"}])

    def test_finish_is_refused_once_while_a_target_does_not_hold_and_the_closing_sweep_settles_what_went_stale(self):
        items = parse(json.dumps(RAW))
        obs = self.drive([
            {"action": "run", "command": REFUSES, "criterion": "K3"},
            {"action": "check"},
            {"action": "finish"},                                   # K2 has no result yet: refused
            {"action": "run", "command": KEEPS, "criterion": "K2"},
            {"action": "edit", "path": NAME, "old": "'list'", "new": "'a list is required'"},   # both go stale
            {"action": "finish"},                                   # needs a check first
            {"action": "check"},
            {"action": "finish"},                                   # the stale targets: refused once more? no: once per Verify
        ], criteria=items, targets=["K3", "K2"])
        errors = [e.get("error", "") for e in self.tools()]
        self.assertIn("Targets not holding on the current files: ['K2']", errors[2])
        self.assertIn("Calling finish again closes this experiment with them open", errors[2])
        self.assertIn("check before finish", errors[5])
        self.assertEqual(obs["stop"], "finished")
        # Both stored probes were run again on the final files without a model call.
        self.assertEqual(obs["swept"], [{"criterion": "K2", "before": "stale", "after": "holds"},
                                        {"criterion": "K3", "before": "stale", "after": "holds"}])
        self.assertEqual((obs["ledger"], obs["targets"]), ({"unrun": 1, "holds": 2, "unverifiable": 1}, ["K3", "K2"]))
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
