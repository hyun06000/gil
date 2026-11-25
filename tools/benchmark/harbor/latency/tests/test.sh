#!/bin/sh
set -eu
mkdir -p /logs/verifier
echo 0 > /logs/verifier/reward.txt
python - <<'PY'
import csv
import json
from pathlib import Path
from statistics import median

summary = json.loads(Path("/work/result.json").read_text())
assert summary["authority"] == "benchmark_adapter" and summary["verdict"] == "success"
interview, experiment = summary["history"]
assert [interview["kind"], experiment["kind"]] == ["interview", "experiment"]
def report(cycle, kind):
    return [s["report"] for s in cycle["steps"] if s["kind"] == kind][-1]
assert report(interview, "question")["response"].strip()
spec = report(interview, "synthesis")
assert spec["approved"] == "benchmark_accept" and "median" in spec["statement"].lower()
assert report(experiment, "verify")["evidence_files"] == "observation.json"
obs = json.loads(Path("/work/project/observation.json").read_text())
with open("/case/latencies.csv") as f:
    expected = median(int(row["latency_ms"]) for row in csv.DictReader(f))
assert obs == {"operation":"median", "value":expected}, obs
assert float(report(experiment, "verify")["result"]) == expected
assert experiment["report"]["verdict"] == "success"
PY
echo 1 > /logs/verifier/reward.txt
