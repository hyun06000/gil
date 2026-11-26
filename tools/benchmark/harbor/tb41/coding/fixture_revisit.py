#!/usr/bin/env python3
"""Scripted model and public task for the native Core/participant integration test."""
import json
import os
from pathlib import Path
import sys
import time

os.environ.update(TB_TASK="vf2-speedup-networkx", GIL_TB41_CYCLE_RECOVERY="1")
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
(work.parent / "deadline").write_text(str(time.time() + 600))
check = "from fast_networkx import order; assert order(['a','B']) == ['a','B']; assert order(['a','a']) == ['a','a'], 'lost duplicate label'"
solver.CHECKS[solver.TASK] = [sys.executable, "-B", "-c", check]
cycle = request["history"][-1]
ref = cycle["ref"]
base = "def order(labels): return sorted(labels, key=str.lower)"
bad = "def order(labels): return sorted(set(labels), key=str.casefold)"
fixed = "def order(labels): return sorted(labels, key=str.casefold)"
source = app / "fast_networkx.py"
calls = 0


def model(prompt, choices=(), stage="solver"):
    global calls
    calls += 1
    model.call_id = ref + "-" + stage + "-" + str(calls)
    if stage == "solver":
        if calls == 1:
            if cycle["revisit_from"]:
                assert source.read_text() == base
                assert "lost duplicate label" in prompt and "sort the list directly" in prompt
                code = fixed
            else:
                code = base if ref == "cycle:C2" else bad
            answer = json.dumps(dict(action="write", path=source.name, content=code))
        else:
            answer = json.dumps(dict(action="check" if calls == 2 else "finish"))
    elif stage == "hypothesis" and cycle["revisit_from"]:
        assert "lost duplicate label" in prompt and "set before sorting" in prompt
        answer = "To retain duplicates, sort the list directly with a casefold key."
    elif stage == "hypothesis":
        answer = "Use a set before sorting" if ref == "cycle:C3" else "Check sorting on duplicate labels."
    elif choices:
        answer = next((v for v in ["open", "start", "demonstrated", "continue interview"] if v in choices), choices[0])
    else:
        answer = "Preserve duplicate labels and sort by casefold."
    mailbox_client.record(event="fixture_exchange", cycle=ref, stage=stage, prompt=prompt, answer=answer)
    return answer


participant.ask = solver.ask = model
print(json.dumps(participant.reply(request)))
