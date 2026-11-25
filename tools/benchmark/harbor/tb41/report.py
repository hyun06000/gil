"""Collect only comparison measures. A missing verifier result is distinguishable from zero."""

from datetime import datetime
import json
import os
from pathlib import Path
import sys

from receipt import receipt

root = Path(sys.argv[1])
arm = sys.argv[2]


def read(path):
    return json.loads(path.read_text()) if path.is_file() else {}


trials = list(root.glob("pair/*/result.json"))
if len(trials) > 1:
    raise ValueError("Expected one trial")
folder = trials[0].parent if trials else root / "missing"
trial = read(folder / "result.json")
usage = read(folder / "agent/luna-usage.json")
obs = read(folder / "agent/observation.json")
gil = read(folder / "agent/gil-result.json")
timing = trial.get("agent_execution") or {}
duration = None
if timing.get("started_at") and timing.get("finished_at"):
    duration = (
        datetime.fromisoformat(timing["finished_at"])
        - datetime.fromisoformat(timing["started_at"])
    ).total_seconds()
error = (trial.get("exception_info") or {}).get("exception_type")
rewards = (trial.get("verifier_result") or {}).get("rewards") or {}
result = {
    "arm": arm,
    "task": "sound-change-cascade",
    "version": "4.1.0",
    "model": "gpt-5.6-luna",
    "rewards": rewards,
    "submission": receipt(folder),
    "error": error or (None if trials else "MissingTrial"),
    "elapsed_seconds": duration,
    "usage": usage,
    "training": obs,
    "gil_verdict": gil.get("verdict"),
    "steps": [s["kind"] for c in gil.get("history", []) for s in c["steps"]],
    "run_id": os.getenv("GITHUB_RUN_ID"),
    "commit": os.getenv("GITHUB_SHA"),
}
(root / (arm + ".json")).write_text(json.dumps(result, indent=2))
print(json.dumps(result), flush=True)
submission = result["submission"]
if arm in ("reference", "empty"):
    expected = int(arm == "reference")
    if (
        result["error"]
        or rewards.get("reward") != expected
        or not submission.get("checks")
        or "ordering_count" not in submission
        or bool(submission["ordering_count"]) != bool(expected)
        or not submission["ordering_matches"]
    ):
        raise SystemExit("Official verifier or submission transport failed the control")
    raise SystemExit(0)
if (
    not submission.get("ordering_matches")
    or not submission.get("rule_count")
    or submission["rule_count"] != obs.get("rules")
):
    raise SystemExit("Missing or inconsistent collected submission")
if result["error"] or not rewards or not usage or not obs or (arm == "on" and not gil):
    if trial.get("exception_info"):
        print(
            trial["exception_info"].get("exception_message", "")[-1200:],
            file=sys.stderr,
        )
    raise SystemExit("Incomplete comparison trial. See Harbor verifier and agent logs.")
