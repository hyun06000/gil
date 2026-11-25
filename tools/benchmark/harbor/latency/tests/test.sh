#!/bin/sh
set -eu
mkdir -p /logs/verifier
echo 0 > /logs/verifier/reward.txt
python - <<'PY'
import csv
import json
import re
from pathlib import Path
from statistics import median

summary = json.loads(Path("/work/result.json").read_text())
assert summary["authority"] == "benchmark_adapter" and summary["verdict"] == "success"
interview, experiment = summary["history"]
assert [interview["kind"], experiment["kind"]] == ["interview", "experiment"]
def report(cycle, kind):
    return [s["report"] for s in cycle["steps"] if s["kind"] == kind][-1]
question = report(interview, "question")
assert question["question"].rstrip().endswith("?"), question
assert "median" in question["response"].lower(), question
# This task provides no sample values in the brief. Numerical claims before Verify lack evidence.
before_execution = [r for r in interview["steps"] + experiment["steps"]
                    if r["kind"] in {"question","interpretation","synthesis","define","hypothesis"}]
for s in before_execution:
    for key, text in s["report"].items():
        if key not in {"basis_refs","benchmark_source"}:
            assert not re.search(r"\d|\b(?:hundred|thousand|thirteen)\b", text, re.I), (s["kind"], key, text)
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
