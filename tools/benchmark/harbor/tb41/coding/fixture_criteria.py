#!/usr/bin/env python3
"""Scripted model and public task for the native Core test of a criteria-ledger Chain.

C2 leaves a checked first submission. C3 derives three criteria and gains the failure case. C4
gains the invariant but breaks the failure case, which only the closing sweep sees, so it fails
and the Core restores C3's world. C5 branches from C3 and gains the invariant; C6 gains the rest.
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
criteria.TARGETS = 1   # one criterion per Experiment, so the Chain has to continue
(work.parent / "deadline").write_text(str(time.time() + 3600))
py = sys.executable + " -B -c "
solver.CHECKS[solver.TASK] = [sys.executable, "-B", "-c", "from fast_networkx import order; assert order(['a','B']) == ['a','B']"]
PROBES = {
    "K1": py + "\"import sys\nfrom fast_networkx import order\ntry:\n    order('ab')\nexcept TypeError:\n    sys.exit(0)\nsys.exit(1)\"",
    "K2": py + "\"from fast_networkx import order\nlabels = ['b', 'A']\norder(labels)\nassert labels == ['b', 'A']\"",
    "K3": py + "\"from fast_networkx import order\nassert order(['b', 'A', 'b']) == ['A', 'b', 'b']\"",
}
LEDGER = [
    {"requirement": "labels are ordered ignoring case and duplicates are kept", "kind": "behaviour",
     "source": "task: ordering", "probe": "call order on a list with a duplicate"},
    {"requirement": "the list passed in is not modified", "kind": "invariant", "source": "task: input", "probe": "compare the list"},
    {"requirement": "a value that is not a list is refused with TypeError", "kind": "failure", "source": "task: input",
     "probe": "pass a string"},
]
BASE = "def order(labels):\n    return sorted(labels, key=str.casefold)\n"
GUARDED = "def order(labels):\n    if not isinstance(labels, list):\n        raise TypeError('a list is required')\n    return sorted(labels, key=str.casefold)\n"
UNGUARDED = "def order(labels):\n    return sorted(list(labels), key=str.casefold)\n"
NAME = "fast_networkx.py"
cycle = request["history"][-1]
ref = cycle["ref"]
# Parsed ids follow the order of the derived array: K1 behaviour, K2 invariant, K3 failure.
ID = {"behaviour": "K1", "invariant": "K2", "failure": "K3"}
probe = lambda kind, key: {"action": "run", "command": PROBES[key], "criterion": ID[kind], "note": f"probe {ID[kind]}"}
SCRIPT = {
    "cycle:C2": [{"action": "write", "path": NAME, "content": BASE}, {"action": "check"}],
    "cycle:C3": [{"action": "write", "path": NAME, "content": GUARDED}, probe("failure", "K1"), {"action": "check"}, {"action": "finish"}],
    "cycle:C4": [{"action": "write", "path": NAME, "content": UNGUARDED}, probe("invariant", "K2"), {"action": "check"}, {"action": "finish"}],
    "cycle:C5": [probe("invariant", "K2"), {"action": "check"}, {"action": "finish"}],
    "cycle:C6": [probe("behaviour", "K3"), {"action": "check"}, {"action": "finish"}],
}
calls = 0


def model(prompt, choices=(), stage="solver", notes=False, ledger=False):
    global calls
    calls += 1
    model.call_id = ref + "-" + stage + "-" + str(calls)
    if stage == "solver":
        # A ledger Verify asks for the criterion with each command; the first milestone does not.
        assert ledger == (ref != "cycle:C2"), (ref, ledger)
        if ref == "cycle:C5":
            assert (app / NAME).read_text() == GUARDED, "the abandoned Experiment's file was not restored"
        answer = json.dumps(SCRIPT[ref][calls - 1])
    elif stage == "define":
        assert ref == "cycle:C3", "the ledger is derived once and inherited afterwards"
        answer = json.dumps(LEDGER)
    elif stage == "hypothesis" and ref != "cycle:C2":
        assert "This step is text only" in prompt
        if ref == "cycle:C5":
            # The branch is told what the abandoned Experiment found.
            assert "Closed failed experiment cycle:C4" in prompt and "held_at_entry_now_failing" in prompt
        answer = "The change makes the target hold; its probe exiting non-zero would show otherwise."
    elif choices:
        answer = next((v for v in ["open", "start", "demonstrated", "continue interview"] if v in choices), choices[0])
    else:
        answer = "Order labels ignoring case, keep duplicates, leave the input alone, refuse a non-list."
    mailbox_client.record(event="fixture_exchange", cycle=ref, stage=stage, answer=answer)
    return answer


participant.ask = solver.ask = model
print(json.dumps(participant.reply(request)))
