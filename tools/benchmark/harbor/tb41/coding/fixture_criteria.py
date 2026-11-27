#!/usr/bin/env python3
"""Scripted model and public task for the native Core test of a criteria-ledger Chain.

C2 leaves a checked first submission. C3 derives three criteria, takes the failure case and fixes
its probe in Define. Its first hypothesis does not bring the state about; reflection doubts the
hypothesis, and the run goes back to C3's Define with the files as they were there. The second
hypothesis solves it, and reflection names the invariant as the next candidate. C4 solves the
invariant with a method that breaks the failure case, which the closing run of every probe sees;
reflection doubts the problem definition, C4 closes as failed and the Core restores C3's world.
C5 branches from C3 and solves the invariant; C6 solves the rest and names no candidate.
"""
import json
import os
from pathlib import Path
import sys
import time

os.environ.update(TB_TASK="vf2-speedup-networkx", GIL_TB41_CYCLE_RECOVERY="1")
import criteria
import mailbox_client
import participant
import solver

request = json.load(sys.stdin)
if request.get("previous_error"):
    raise ValueError(request["previous_error"])
work = Path.cwd()
app = work.parent / "app"
participant.WORK = solver.WORK = work
participant.MAIL = mailbox_client.MAIL = work.parent
participant.APP = solver.APP = app
(work.parent / "deadline").write_text(str(time.time() + 3600))
py = sys.executable + " -B -c "
solver.CHECKS[solver.TASK] = [sys.executable, "-B", "-c", "from fast_networkx import order; assert order(['a','B']) == ['a','B']"]
# Parsed ids follow the order of the derived array: K1 behaviour, K2 invariant, K3 failure.
LEDGER = [
    {"requirement": "labels are ordered ignoring case and duplicates are kept", "kind": "behaviour", "source": "task: ordering"},
    {"requirement": "the list passed in is not modified", "kind": "invariant", "source": "task: input"},
    {"requirement": "a value that is not a list is refused with TypeError", "kind": "failure", "source": "task: input"},
]
PROBES = {
    "K1": py + "\"from fast_networkx import order\nassert order(['b', 'A', 'b']) == ['A', 'b', 'b']\"",
    "K2": py + "\"from fast_networkx import order\nlabels = ['b', 'A']\norder(labels)\nassert labels == ['b', 'A']\"",
    "K3": py + "\"import sys\nfrom fast_networkx import order\ntry:\n    order('ab')\nexcept TypeError:\n    sys.exit(0)\nsys.exit(1)\"",
}
BASE = "def order(labels):\n    return sorted(labels, key=str.casefold)\n"
TRIED = "def order(labels):\n    labels = list(labels)\n    return sorted(labels, key=str.casefold)\n"
GUARDED = "def order(labels):\n    if not isinstance(labels, list):\n        raise TypeError('a list is required')\n    return sorted(labels, key=str.casefold)\n"
UNGUARDED = "def order(labels):\n    return sorted(list(labels), key=str.casefold)\n"
NAME = "fast_networkx.py"
cycle = request["history"][-1]
ref = cycle["ref"]
# Hypotheses already abandoned under this Define.
attempt = sum(1 for s in cycle["steps"] if s["kind"] == "outcome" and s["report"])
TARGET = {"cycle:C3": "K3", "cycle:C4": "K2", "cycle:C5": "K2", "cycle:C6": "K1"}
NEXT = {"cycle:C3": ["K2"], "cycle:C5": ["K1"], "cycle:C6": []}
probe = lambda key: {"action": "probe", "criterion": key, "note": f"run the probe of {key}"}
write = lambda content: {"action": "write", "path": NAME, "content": content}
SCRIPT = {
    ("cycle:C2", 0): [write(BASE), {"action": "check"}],
    ("cycle:C3", 0): [write(TRIED), probe("K3"), {"action": "check"}, {"action": "finish"}],
    ("cycle:C3", 1): [write(GUARDED), probe("K3"), {"action": "check"}, {"action": "finish"}],
    ("cycle:C4", 0): [write(UNGUARDED), probe("K2"), {"action": "check"}, {"action": "finish"}],
    ("cycle:C5", 0): [probe("K2"), {"action": "check"}, {"action": "finish"}],
    ("cycle:C6", 0): [probe("K1"), {"action": "check"}, {"action": "finish"}],
}
# What the files must be when an attempt starts: every going back restored them.
ENTRY = {("cycle:C3", 0): BASE, ("cycle:C3", 1): BASE, ("cycle:C4", 0): GUARDED, ("cycle:C5", 0): GUARDED, ("cycle:C6", 0): GUARDED}
calls = 0


def model(prompt, choices=(), stage="solver", notes=False, ledger=False, schema=None):
    global calls
    calls += 1
    model.call_id = ref + "-" + stage + "-" + str(calls)
    if stage == "solver":
        # A ledger Verify can run the probes its Define fixed; the first milestone has none.
        assert ledger == (ref != "cycle:C2"), (ref, ledger)
        if calls == 1 and (ref, attempt) in ENTRY:
            assert (app / NAME).read_text() == ENTRY[ref, attempt], f"{ref} attempt {attempt} did not start from the restored files"
        if ledger:
            assert "carries out the hypothesis above and nothing more" in prompt
        answer = json.dumps(SCRIPT[ref, attempt][calls - 1])
    elif schema == criteria.CRITERIA:
        assert (stage, ref) == ("define", "cycle:C3"), "the ledger is derived once and inherited afterwards"
        answer = json.dumps({"criteria": LEDGER})
    elif schema == criteria.TARGETS:
        assert stage == "define"
        if ref in ("cycle:C4", "cycle:C6"):
            # The parent's reflection named the candidates this Define chooses from.
            assert f"Candidates: {NEXT['cycle:C3' if ref == 'cycle:C4' else 'cycle:C5']}" in prompt, prompt[-300:]
        answer = json.dumps({"targets": [TARGET[ref]]})
    elif schema == criteria.PROBES:
        # Define fixes the state and its probe before any method exists.
        assert stage == "define"
        key = TARGET[ref]
        answer = json.dumps({"probes": [{"id": key, "state": "the requirement is observable from outside", "probe": PROBES[key]}]})
    elif schema == criteria.HYPOTHESIS:
        assert stage == "hypothesis" and "The Verify that follows carries out exactly this method" in prompt
        if (ref, attempt) == ("cycle:C3", 1):
            # The Define stands; the new hypothesis is told what the abandoned one found.
            assert "these hypotheses were tried and abandoned" in prompt and "targets_not_holding" in prompt
        if ref == "cycle:C5":
            assert "Closed failed experiment cycle:C4" in prompt and "held_at_entry_now_failing" in prompt
        changes = ref in ("cycle:C3", "cycle:C4")
        answer = json.dumps({"method": f"Attempt {attempt}: change order() so the defined state holds.", "files": [NAME] if changes else []})
    elif schema == criteria.CANDIDATES:
        assert stage == "outcome"
        answer = json.dumps({"candidates": NEXT[ref], "new": []})
    elif schema == criteria.SET_ASIDE:
        answer = json.dumps({"set_aside": []})
    elif "revise_hypothesis" in choices:
        # Reflection: in C3 the method was wrong; in C4 the experiment broke what the Chain had gained.
        answer = "revise_hypothesis" if ref == "cycle:C3" else "revise_problem"
    elif choices:
        answer = next((v for v in ["open", "start", "demonstrated", "continue interview"] if v in choices), choices[0])
    else:
        answer = "Order labels ignoring case, keep duplicates, leave the input alone, refuse a non-list."
    mailbox_client.record(event="fixture_exchange", cycle=ref, stage=stage, answer=answer)
    return answer


participant.ask = solver.ask = model
print(json.dumps(participant.reply(request)))
