import json,sys,hashlib
from datetime import datetime
from pathlib import Path
root,arm,task=sys.argv[1:]; root=Path(root)
trial,=root.glob("pair/*/result.json")
d=json.loads(trial.read_text()); folder=trial.parent
transcript = folder / "agent/transcript.jsonl"
if transcript.is_file():
    import shutil
    from transcript_view import render
    shutil.copyfile(transcript, root / "transcript.jsonl")
    render(root)
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
(root/(arm+".json")).write_text(json.dumps(r,indent=2))
print(json.dumps(r),flush=True)
if error:
    print(str(d["exception_info"].get("exception_message",""))[-2000:],flush=True)
if error or not r["rewards"] or not receipts or not all(receipts.values()): sys.exit(1)
