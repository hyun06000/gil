import json,sys,hashlib,shutil
from datetime import datetime
from pathlib import Path
root,arm,task=sys.argv[1:]; root=Path(root)
trial,=root.glob("pair/*/result.json")
d=json.loads(trial.read_text()); folder=trial.parent
transcript = folder / "agent/transcript.jsonl"
if transcript.is_file():
    from transcript_view import render
    shutil.copyfile(transcript, root / "transcript.jsonl")
interruption = folder / "agent/gil-interrupted.tgz"
if interruption.is_file():
    shutil.copyfile(interruption, root / interruption.name)
# The mcp arm's project record, taken out of the task folder before grading.
kept = folder / "agent/gil-record.tgz"
if kept.is_file():
    shutil.copyfile(kept, root / kept.name)

def load(path):
    f=folder/path
    return json.loads(f.read_text()) if f.exists() else {}
u=load("agent/luna-usage.json"); obs=load("agent/observation.json")
checks=load("verifier/ctrf.json").get("results",{}).get("tests",[])
timing=d.get("agent_execution") or {}
t=timing.get("finished_at") and (datetime.fromisoformat(timing["finished_at"])-datetime.fromisoformat(timing["started_at"])).total_seconds()
files=obs.get("files",{}); receipts={}
for name,digest in files.items():
    f=folder/"artifacts/app"/name
    receipts[name]=f.is_file() and hashlib.sha256(f.read_bytes()).hexdigest()==digest
error=(d.get("exception_info") or {}).get("exception_type")
r={"task":task,"arm":arm,"usage":u,"observation":obs,"receipts":receipts,"checks":{c["name"]:c["status"] for c in checks},"seconds":t,"rewards":(d.get("verifier_result") or {}).get("rewards"),"error":error}
# These runs are internal measurements, not Terminal-Bench scores: say how they differ from the published setup.
import os
r["conditions"] = {
    "internal_measurement": True,
    "task_commit": os.environ.get("TB_UPSTREAM"),
    "agent_timeout_multiplier": 0.125,
    "task_toml_edited": 'network_mode = "no-network" added under [environment]',
    "dockerfile_edited": "python3 installed in the task image" if task == "react-lead-form" else None,
    "trials_per_condition_in_this_job": 1,
    "cost_limit_usd": os.environ.get("TB_BUDGET_USD") or "5",
    "official_metric": "rewards.reward (binary); the count of passed checks is our own supplementary reading",
}
if arm == "on":
    r["gil"] = load("agent/gil-result.json")
if error:
    message = str(d["exception_info"].get("exception_message", ""))
    if message.startswith("Luna API HTTP "):
        r["model_error"] = message[:500]
(root/(arm+".json")).write_text(json.dumps(r,indent=2))
print(json.dumps(r),flush=True)
if transcript.is_file():
    render(root)
if error:
    print(str(d["exception_info"].get("exception_message",""))[-2000:],flush=True)
if error or not r["rewards"] or not receipts or not all(receipts.values()): sys.exit(1)
