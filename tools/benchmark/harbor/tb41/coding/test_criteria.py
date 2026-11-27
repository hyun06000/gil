"""The criteria ledger and the part each step of an Experiment has in it."""
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
REFUSES = PY + "\"import sys\nfrom fast_networkx import order\ntry:\n    order('ab')\nexcept TypeError:\n    sys.exit(0)\nprint('a string was accepted')\nsys.exit(1)\""
KEEPS = PY + "\"from fast_networkx import order\nl=['b','A']\norder(l)\nassert l == ['b','A']\""
RAW = [{"requirement": "duplicates are kept", "kind": "behaviour", "source": "task"},
       {"requirement": "the input is not modified", "kind": "invariant", "source": "task: input"},
       {"requirement": "a non-list is refused", "kind": "failure", "source": "task"},
       {"requirement": "it is fast on the hidden cases", "kind": "behaviour", "source": ""}]


def ledger_with(**probes):
    ledger = Ledger(parse(json.dumps(RAW)))
    for key, script in probes.items():
        ledger.define(key, "the state for " + key, script)
    return ledger


class LedgerTests(unittest.TestCase):
    def test_items_come_from_the_criteria_answer_whole_and_a_bad_answer_is_refused(self):
        items = parse("```json\n" + json.dumps({"criteria": RAW + [{"requirement": "x", "kind": "odd"}]}) + "\n```")
        self.assertEqual([(i["id"], i["kind"], i["status"], i["probe"]) for i in items],
                         [("K1", "behaviour", "unrun", None), ("K2", "invariant", "unrun", None), ("K3", "failure", "unrun", None),
                          ("K4", "behaviour", "unrun", None), ("K5", "behaviour", "unrun", None)])
        self.assertEqual(items[1]["source"], "task: input")
        # Nothing caps or shortens what the model listed.
        long = [{"requirement": "r" * 900 + str(i), "source": "s" * 700} for i in range(40)]
        self.assertEqual([(len(i["requirement"]), len(i["source"])) for i in parse(json.dumps(long))][::39], [(901, 700), (902, 700)])
        for bad, why in (("I think", "not JSON"), ("[]", "non-empty"), ('{"a":1}', "non-empty"),
                         ('["x"]', "object with a requirement"), ('[{"kind":"failure"}]', "object with a requirement")):
            with self.subTest(bad=bad), self.assertRaisesRegex(ValueError, why):
                parse(bad)

    def test_a_criterion_has_no_result_until_a_define_says_what_solved_means_and_a_result_belongs_to_its_submission(self):
        ledger = ledger_with()
        a, b = digest({NAME: "1"}), digest({NAME: "2"})
        self.assertEqual((ledger.counts(a), ledger.open(a)), ({"undefined": 4}, ["K1", "K2", "K3", "K4"]))
        ledger.define("K3", "a string is refused with TypeError", REFUSES)
        ledger.define("K2", "the list is byte-identical afterwards", KEEPS)
        self.assertEqual(ledger.counts(a), {"undefined": 2, "unrun": 2})
        self.assertEqual(ledger.record("K3", 0, "ok", a), "holds")
        self.assertEqual((ledger.status("K3", a), ledger.status("K3", b)), ("holds", "stale"))
        self.assertEqual(ledger.record("K2", "none, stopped after 100 s", "", a), "fails")
        self.assertEqual((ledger.open(a), ledger.having(a, "holds")), (["K1", "K2", "K4"], ["K3"]))
        rows = {r["id"]: r for r in ledger.shown(a, ["K2"])}
        # The targets show the state and the probe that were fixed; the others only where they stand.
        self.assertEqual((rows["K2"]["status"], rows["K2"]["target"], rows["K2"]["probe"], rows["K2"]["state"]),
                         ("fails", True, KEEPS, "the list is byte-identical afterwards"))
        self.assertNotIn("probe", rows["K3"])
        # A new definition of what solved means voids what the old probe showed.
        ledger.define("K3", "a string and a tuple are refused", REFUSES + " # and more")
        self.assertEqual((ledger.status("K3", a), ledger.items["K3"]["failed_on"]), ("unrun", []))

    def test_no_count_decides_anything_and_setting_aside_is_a_judgement(self):
        ledger = ledger_with(K1=KEEPS)
        versions = [digest({NAME: str(i)}) for i in range(8)]
        for version in versions:
            ledger.record("K1", 1, "no", version)
        ledger.record("K1", 1, "no", versions[-1])     # the same submission again is one failure
        row = ledger.shown(versions[-1])[0]
        self.assertEqual((ledger.status("K1", versions[-1]), row["failed_on_submissions"], "K1" in ledger.open(versions[-1])),
                         ("fails", 8, True))
        ledger.set_aside("K1", "the source does not require this order")
        self.assertEqual((ledger.status("K1", versions[-1]), "K1" in ledger.open(versions[-1])), ("set_aside", False))
        self.assertIn("does not require", ledger.shown(versions[-1])[0]["set_aside"])

    def test_a_reflection_can_add_requirements_the_ledger_lacked(self):
        ledger = ledger_with()
        added = ledger.add([{"requirement": "an empty list gives an empty list", "kind": "behaviour", "source": "task"},
                            {"kind": "failure"}, "not an object"])
        self.assertEqual((added, ledger.items["K5"]["requirement"], len(ledger.items)), (["K5"], "an empty list gives an empty list", 5))

    def test_every_schema_is_strict(self):
        def strict(schema):
            if schema.get("type") == "object":
                self.assertEqual((schema["required"], schema["additionalProperties"]), (list(schema["properties"]), False))
                for child in schema["properties"].values():
                    strict(child)
            elif schema.get("type") == "array":
                strict(schema["items"])
        for schema in (criteria.CRITERIA, criteria.TARGETS, criteria.PROBES, criteria.HYPOTHESIS, criteria.CANDIDATES, criteria.SET_ASIDE):
            strict(schema)
        self.assertEqual(list(criteria.PROBES["properties"]["probes"]["items"]["properties"]), ["id", "state", "probe"])
        self.assertEqual(list(criteria.HYPOTHESIS["properties"]), ["method", "files"])


class VerifyLedgerTests(unittest.TestCase):
    def drive(self, actions, *, criteria=None, targets=(), scope=None, source=BASE, cycle="cycle:C3"):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        root = Path(temp.name).resolve()
        self.app, self.work = root / "app", root / "project"
        self.app.mkdir(); self.work.mkdir()
        (root / "deadline").write_text(str(time.time() + 400))
        (self.app / NAME).write_text(source)
        (self.app / "fast_networkx").mkdir()
        self.events, self.prompts, self.flags = [], [], []
        script = iter(actions)

        def model(prompt, **options):
            model.call_id = str(len(self.prompts))
            self.prompts.append(prompt)
            self.flags.append(options)
            return json.dumps(next(script))

        check = [sys.executable, "-B", "-c", "from fast_networkx import order; assert order(['a','B']) == ['a','B']"]
        options = dict(cycle=cycle, baseline={"files": {NAME: hashlib.sha256(source.encode()).hexdigest()}},
                       verify_ref="step:C3/S3", criteria=criteria, targets=targets, scope=scope) if cycle else {}
        with patch.multiple(solver, APP=self.app, WORK=self.work, TASK="vf2-speedup-networkx", ask=model), \
                patch.dict(solver.CHECKS, {"vf2-speedup-networkx": check}), \
                patch.object(solver, "record", side_effect=lambda **e: self.events.append(e)):
            return solver.solve("Order labels", "Hypothesis to carry out: refuse a non-list", **options)

    def tools(self):
        return [e for e in self.events if e.get("event") == "tool"]

    def test_verify_runs_the_probes_define_fixed_and_can_write_none(self):
        ledger = ledger_with(K3=REFUSES, K2=KEEPS)
        obs = self.drive([
            {"action": "probe", "criterion": "K3"},
            {"action": "run", "command": "true", "criterion": "K1"},      # a command of its own proves no criterion
            {"action": "probe", "criterion": "K1"},                       # nothing was defined for it
            {"action": "probe", "criterion": "K9"},
            {"action": "write", "path": NAME, "content": GUARDED},
            {"action": "probe", "criterion": "K3"},
            {"action": "check"}, {"action": "finish"}], criteria=ledger.json(), targets=["K3"], scope={NAME})
        tools = self.tools()
        self.assertEqual(self.flags[0], {"notes": True, "ledger": True})
        self.assertIn("carries out the hypothesis above and nothing more", self.prompts[0])
        self.assertIn("You write no probe and change none", self.prompts[0])
        # Before anything was done the target's probe was run on the entry files.
        self.assertEqual((obs["entry"], [e["event"] for e in self.events[:2]]), ({"K3": "fails"}, ["verify_plan", "probe_entry"]))
        self.assertTrue(tools[0]["result"].startswith("criterion K3 fails on the current files. exit 1: a string was accepted"))
        self.assertNotIn("criterion", tools[1])
        self.assertIn("K1 has no probe; the defined ones are ['K2', 'K3']", tools[2]["error"])
        self.assertIn("K9 has no probe", tools[3]["error"])
        self.assertTrue(tools[5]["result"].startswith("criterion K3 holds on the current files."))
        by = {i["id"]: i for i in obs["criteria"]}
        # The probe text is what Define wrote, unchanged; K1 has no result from the labelled command.
        self.assertEqual((by["K3"]["probe"], by["K3"]["status"], by["K1"]["status"], by["K1"]["probe"]), (REFUSES, "holds", "unrun", None))
        self.assertEqual(obs["ledger"], {"undefined": 2, "holds": 2})
        # The other defined probe was run on the final files when the step ended, without the model.
        self.assertEqual(obs["swept"], [{"criterion": "K2", "before": "unrun", "after": "holds"}])
        self.assertEqual((obs["stop"], obs["outside_hypothesis"]), ("finished", []))

    def test_verify_changes_only_the_files_the_hypothesis_names(self):
        ledger = ledger_with(K3=REFUSES)
        obs = self.drive([
            {"action": "write", "path": "fast_networkx/_guard.py", "content": "x = 1\n"},
            {"action": "edit", "path": "fast_networkx/_guard.py", "old": "x", "new": "y"},
            {"action": "run", "command": "printf 'y = 2\\n' > fast_networkx/_other.py"},
            {"action": "write", "path": NAME, "content": GUARDED},
            {"action": "check"}, {"action": "finish"}], criteria=ledger.json(), targets=["K3"], scope={NAME})
        errors = [e.get("error", "") for e in self.tools()]
        self.assertIn("fast_networkx/_guard.py is not a file the hypothesis names (['fast_networkx.py'])", errors[0])
        self.assertIn("A method that needs it is a different hypothesis: finish and say so", errors[0])
        self.assertIn("is not a file the hypothesis names", errors[1])
        self.assertFalse((self.app / "fast_networkx/_guard.py").exists())
        # A command changed a submission file the hypothesis did not name: it is recorded for the analysis.
        self.assertEqual(obs["outside_hypothesis"], ["fast_networkx/_other.py"])
        self.assertIn("Files the hypothesis names: ['fast_networkx.py']", self.prompts[0])
        self.assertEqual(obs["ledger"]["holds"], 1)

    def test_the_closing_run_finds_a_criterion_a_later_change_broke_and_settles_the_local_check(self):
        ledger = ledger_with(K3=REFUSES, K2=KEEPS)
        obs = self.drive([
            {"action": "probe", "criterion": "K3"},                 # holds on the guarded entry files
            {"action": "write", "path": NAME, "content": BASE},     # removes the refusal
            {"action": "probe", "criterion": "K2"},
        ] + [{"action": "probe", "criterion": "K2"}] * 20, criteria=ledger.json(), targets=["K2"], scope={NAME}, source=GUARDED)
        self.assertEqual(obs["stop"], "stalled")
        self.assertEqual(obs["swept"], [{"criterion": "K3", "before": "stale", "after": "fails"}])
        self.assertEqual(obs["check_exit"], 0)                      # never asked for; run when the step ended
        by = {i["id"]: i for i in obs["criteria"]}
        self.assertEqual((by["K3"]["status"], by["K2"]["status"], by["K3"]["digest"]), ("fails", "holds", obs["submission_digest"]))
        self.assertEqual(self.events[-1]["event"], "probe_sweep")

    def test_without_a_ledger_nothing_of_it_reaches_the_request(self):
        for cycle in (None, "cycle:C3"):
            with self.subTest(cycle=cycle):
                obs = self.drive([{"action": "run", "command": "echo hi"}, {"action": "probe", "criterion": "K1"},
                                  {"action": "write", "path": NAME, "content": GUARDED},
                                  {"action": "check"}, {"action": "finish"}], cycle=cycle)
                self.assertEqual(self.flags, [{"notes": True}] * 5)
                self.assertIn("No criteria are defined for this session", self.tools()[1]["error"])
                self.assertFalse(any("carries out the hypothesis above" in p or "Files the hypothesis names" in p for p in self.prompts))
                self.assertFalse({"criteria", "targets", "swept", "ledger", "entry", "outside_hypothesis"} & set(obs))
                self.assertFalse(any(e.get("event") in ("probe", "probe_sweep", "probe_entry") for e in self.events))


class ParticipantLedgerTests(unittest.TestCase):
    def setUp(self):
        clock = patch.object(participant, "minutes_left", return_value=41.6)
        clock.start()
        self.addCleanup(clock.stop)
        self.asked = []

    def model(self, answer):
        def ask(prompt, choices=(), schema=None):
            self.asked.append({"prompt": prompt, "schema": schema})
            return answer
        return ask

    def test_a_ledger_is_asked_for_once_under_a_schema_and_an_unusable_answer_means_no_ledger(self):
        items = participant.derive(self.model(json.dumps({"criteria": RAW})), "task", "spec")
        self.assertEqual([i["id"] for i in items], ["K1", "K2", "K3", "K4"])
        self.assertEqual(self.asked[0]["schema"], criteria.CRITERIA)
        self.assertIn("List first every failure case", self.asked[0]["prompt"])
        self.assertIsNone(participant.derive(self.model("still not json"), "task", "spec"))
        self.assertIsNone(participant.derive(self.model(json.dumps({"criteria": []})), "task", "spec"))
        self.assertEqual(len(self.asked), 3)

    def test_define_chooses_its_problem_from_the_candidates_and_fixes_state_and_probe_before_any_method(self):
        ledger, entry = ledger_with(), digest({NAME: "1"})
        picked = participant.choose(self.model(json.dumps({"targets": ["K3", "K9", "K3", "K1"]})), ledger, entry, "Earlier: K3 failed", ["K2", "K3"])
        # Only candidates can be chosen; what is not named stays open.
        self.assertEqual((picked, ledger.open(entry)), (["K3"], ["K1", "K2", "K3", "K4"]))
        self.assertIn("Candidates: ['K2', 'K3']", self.asked[0]["prompt"])
        self.assertIn("About 42 minutes of the trial are left.", self.asked[0]["prompt"])
        for unusable in ("no idea", json.dumps({"targets": []}), json.dumps({"targets": ["K1"]})):
            self.assertEqual(participant.choose(self.model(unusable), ledger, entry, "", ["K2", "K3"]), ["K2", "K3"])
        self.asked.clear()
        defined = participant.define_states(self.model(json.dumps({"probes": [
            {"id": "K3", "state": "a string is refused with TypeError", "probe": REFUSES},
            {"id": "K2", "state": "", "probe": KEEPS},                 # no state: not a definition
            {"id": "K1", "state": "kept", "probe": "exit 0"},          # not a target of this experiment
            {"id": "K3", "state": "again", "probe": "exit 0"}]})), ledger, ["K3", "K2"], "")
        self.assertEqual((defined, ledger.items["K3"]["probe"], ledger.items["K3"]["state"]), (["K3"], REFUSES, "a string is refused with TypeError"))
        self.assertEqual((ledger.items["K2"]["probe"], ledger.items["K1"]["probe"]), (None, None))
        asked = self.asked[0]
        self.assertEqual(asked["schema"], criteria.PROBES)
        for phrase in ("fixed here, before any method is chosen", "the way an outside party would",
                       "does not call the solution to produce them", "the starting state contains that something"):
            self.assertIn(phrase, asked["prompt"])

    def test_the_hypothesis_names_the_files_and_what_reflections_wrote_is_read_back(self):
        with tempfile.TemporaryDirectory() as temp, patch.multiple(solver, APP=Path(temp).resolve(), TASK="vf2-speedup-networkx"), \
                patch.object(participant, "APP", Path(temp).resolve()):
            self.assertEqual(participant.scope_of('Refuse a non-list.\nFiles: ["fast_networkx.py", "/etc/passwd", "README.md"]'),
                             {"fast_networkx.py"})
            self.assertEqual(participant.scope_of("Nothing to change.\nFiles: []"), set())
            self.assertIsNone(participant.scope_of("An older hypothesis with no file list"))
        closed = lambda lesson: {"steps": [{"kind": "outcome", "report": {"verdict": "x", "lesson": lesson}}]}
        self.assertEqual(participant.noted(closed('Solved.\nCandidates: {"candidates": ["K2"], "new": []}\nmore'), "Candidates"),
                         {"candidates": ["K2"], "new": []})
        self.assertEqual(participant.noted(closed('The probe was wrong.\nSet aside: {"K2": "the source allows a copy"}'), "Set aside"),
                         {"K2": "the source allows a copy"})
        for none in (closed("No label here"), closed("Candidates: not json"), {"steps": []}, None):
            self.assertEqual(participant.noted(none, "Candidates"), {})

    def test_an_experiment_starts_from_its_ancestor_s_ledger_with_what_the_abandoned_one_left(self):
        a, b = digest({NAME: "1"}), digest({NAME: "2"})
        held, lost = ledger_with(K3=REFUSES), ledger_with(K3=REFUSES, K2=KEEPS)
        held.record("K3", 0, "", a)
        lost.record("K3", 1, "", b)
        lost.record("K2", 0, "", b)
        result = lambda ledger: {"steps": [{"kind": "verify", "report": {"result": json.dumps({"criteria": ledger.json()})}}]}
        cycles = {"cycle:C3": {**result(held), "parent": "cycle:C2"}, "cycle:C2": {"steps": [], "parent": None}}
        items = {i["id"]: i for i in participant.entry_ledger(cycles, cycles["cycle:C3"], result(lost))}
        # What held in the world the run went back to still holds there; the lost world's failure is remembered.
        self.assertEqual((items["K3"]["status"], items["K3"]["digest"], items["K3"]["failed_on"]), ("holds", a, [b]))
        # A definition only the abandoned Experiment made is kept; its result is stale in the restored world.
        self.assertEqual((items["K2"]["probe"], Ledger(list(items.values())).status("K2", a)), (KEEPS, "stale"))
        self.assertEqual(participant.entry_ledger(cycles, cycles["cycle:C2"], result(lost))[2]["failed_on"], [b])
        self.assertIsNone(participant.entry_ledger(cycles, cycles["cycle:C2"], None))


if __name__ == "__main__":
    unittest.main()
