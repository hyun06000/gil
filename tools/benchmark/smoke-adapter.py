#!/usr/bin/env python3
"""Deterministic participant for the container integration test."""
import json
import os
import sys
from pathlib import Path

req = json.load(sys.stdin)
next_step = req["next"]
kind = next_step.get("kind")
cycle = next_step["cycle"]
steps = req["history"][-1]["steps"]
task = json.loads(req["task"])
assert os.getuid() == 65534 and not os.environ.get("DISPLAY")
assert not Path("/var/run/docker.sock").exists()
assert set(Path("/sys/class/net").iterdir()) == {Path("/sys/class/net/lo")}

def last_ref(name):
    return next(s["ref"] for s in reversed(steps) if s["kind"] == name and s["report"])

def close(**fields):
    body = "".join(key + ": |\n" + "".join("  " + line + "\n" for line in value.splitlines())
                   for key, value in fields.items())
    return {"action": "close", "report": body}

if next_step["action"] == "open":
    name = next_step["recommended"]
    if task.get("early_write"):
        Path("too-early.txt").write_text("synthetic test")
    reply = {"action": "open", "kind": name, "contract":
             f"objective: {task['goal']}\nnext_action: Perform the {name} task\ndone_when: The {name} result is recorded"}
elif kind == "question":
    reply = close(question="What result does the task require?", choices="sum or product", response=task["goal"])
elif kind == "interpretation":
    reply = close(interpretation=task["goal"], unresolved="none")
elif kind == "synthesis":
    reply = close(statement=task["goal"], basis_refs=last_ref("question")+"\n"+last_ref("interpretation"),
                  approved=("benchmark_revise" if task.get("revise") and not any(s["kind"] == "synthesis" and s["report"] for s in steps) else task.get("decision", "benchmark_accept")))
elif kind == "define":
    reply = close(problem=task["goal"], success_condition="The observed sum equals the expected integer")
elif kind == "hypothesis":
    reply = close(hypothesis="Integer addition gives the expected result", rationale="Finite input",
                  guardrail="Use only the supplied integers")
elif kind == "verify":
    observed = sum(task["numbers"])
    if task.get("evidence", True):
        Path("observation.txt").write_text(str(observed)+"\n")
    reply = close(execution="Summed the supplied integers", result=str(observed), evidence_files="observation.txt")
elif kind == "analysis":
    observed = int(Path("observation.txt").read_text())
    matched = observed == task["expected"]
    reply = close(success_condition_met="yes" if matched else "no", guardrail_triggered="no",
                  interpretation=f"Observed {observed}, expected {task['expected']}")
elif kind == "outcome":
    verdict = "success" if cycle == "interview" or int(Path("observation.txt").read_text()) == task["expected"] else "failure"
    fields = {"verdict":verdict, "lesson":"Compared the observation with the benchmark specification",
              "next_direction.action":"close_cycle", "next_direction.reason":"Record the outcome"}
    if cycle == "interview":
        fields["synthesis_ref"] = last_ref("synthesis")
    reply = close(**fields)
elif kind == "cycle":
    verdict = steps[-1]["report"]["verdict"]
    direction = {"next_direction.action":"open_child" if verdict == "success" else "revisit",
                 "next_direction.reason":"Preserve the result for the next cycle"}
    if verdict == "failure":
        direction["next_direction.target_cycle_ref"] = "cycle:C1"
    reply = close(verdict=verdict, outcome_ref=last_ref("outcome"), handoff_summary="Synthetic test result", **direction)
else:
    raise ValueError(next_step)
print(json.dumps(reply))
