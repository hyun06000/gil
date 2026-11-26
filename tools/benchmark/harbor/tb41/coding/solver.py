"""Shared source-editing tools for the three pinned CPU tasks."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

from mailbox_client import ask, record

WORK = Path("/work/project")
APP = Path("/app")
TASK = os.environ["TB_TASK"]
CHECKS = {
    "ks-solver-cpp": ["g++", "-O3", "-std=c++17", "-DKS_SOLVER_LIBRARY", "-I/app", "-c", "solution.cpp", "-o", "/tmp/ks.o"],
    "react-lead-form": ["npm", "run", "build"],
    "vf2-speedup-networkx": ["python3", "-B", "-c",
                             "import runpy; runpy.run_path('/case/graph_smoke.py', run_name='__main__')"],
}


def source(name, write=False):
    path=(APP/name).resolve()
    rel=path.relative_to(APP).as_posix()
    if "node_modules" in path.parts or ".git" in path.parts or not path.is_relative_to(APP):
        raise ValueError("Use task source paths")
    if write:
        allowed = {
          "ks-solver-cpp": rel=="solution.cpp" or (rel.endswith(".hpp") and rel!="oracle.hpp"),
          "react-lead-form": rel.startswith("src/") and not rel.startswith(("src/spec/","src/data/")) and rel.endswith((".ts",".tsx",".css")),
          "vf2-speedup-networkx": rel=="fast_networkx.py" or rel.startswith("fast_networkx/"),
        }
        if not allowed[TASK]: raise ValueError("Choose a writable source path for this task")
    return path


def solve(task,plan="", *, cycle=None, baseline=None):
    public=[]
    for directory,children,files in os.walk(APP):
        children[:]=[v for v in children if v not in ("node_modules",".git","__pycache__","build","dist")]
        public.extend(str((Path(directory)/f).relative_to(APP)) for f in files)
        if len(public)>600: break
    history=[]; writes={}; checked=None; calls=0
    checkpoint = None
    if plan and cycle is None:
        from checkpoint import Checkpoint
        checkpoint = Checkpoint()
    if cycle:
        from checkpoint import capture_file
        record(event="verify_plan", cycle=cycle, plan=plan)
        if baseline:
            writes.update(baseline["files"])
    check_output = ""
    original_hashes = {}
    regression = False
    recoveries = 0
    last_failure = ""
    first_write = first_check = None
    deadline=float((WORK.parent / "deadline").read_text())
    while time.time()<deadline-100:
        prompt=f"""{task}
{plan}
Public source files: {sorted(public)[:120]}
Files you have edited: {list(writes)}
Recent tool results: {json.dumps(history[-12:],ensure_ascii=False)[-35000:]}
Work toward the full official task. Call exactly one of the supplied editor tools. Available actions:
{{"action":"read","path":"relative/file","offset":0}} (next 9000 characters)
{{"action":"write","path":"relative/file","content":"complete source file"}}
{{"action":"check"}} (run task compiler/build/import smoke)
{{"action":"finish"}} after editing the solution and checking it. An incomplete solution can be graded.
For vf2 use fast_networkx.py or a fast_networkx package. For ks use solution.cpp. For React work in src/.
If a local check fails, use its output to fix the source. No remote tools or private test data."""
        if checkpoint and last_failure:
            prompt += "\nLast failed experiment and recovery: " + last_failure
        answer=ask(prompt); calls+=1
        obj = {}
        try:
            obj=json.loads(answer.strip().removeprefix("```json").removeprefix("```").removesuffix("```"))
            action=obj["action"]
            if action=="read":
                path=source(obj["path"])
                start=max(0,int(obj.get("offset",0)))
                response=path.read_text()[start:start+9000]
            elif action=="write":
                path=source(obj["path"],True)
                content=obj["content"]
                if not content.strip(): raise ValueError("empty source file")
                if cycle and path not in original_hashes:
                    original_hashes[path] = hashlib.sha256(path.read_bytes()).hexdigest() if path.exists() else None
                if checkpoint:
                    checkpoint.before_write(path)
                path.parent.mkdir(parents=True,exist_ok=True); path.write_text(content)
                writes[str(path.relative_to(APP))]=hashlib.sha256(path.read_bytes()).hexdigest()
                if cycle:
                    capture_file(WORK, APP, path)
                if first_write is None:
                    first_write = calls
                checked=None; response="written"
            elif action=="check":
                if first_check is None:
                    first_check = calls
                proc=subprocess.run(CHECKS[TASK],cwd=APP,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=100)
                checked=proc.returncode
                response=f"exit {checked}: "+proc.stdout.decode(errors="replace")[-3500:]
                if cycle:
                    for name, digest in writes.items():
                        if hashlib.sha256(source(name, True).read_bytes()).hexdigest() != digest:
                            raise RuntimeError(f'Submission changed during local check: {name}')
                    check_output = response
                    regression = bool(checked and baseline and any(
                        writes.get(str(path.relative_to(APP))) != digest for path, digest in original_hashes.items()))
                    if (not baseline and checked == 0 and writes) or regression:
                        record(event="tool", id=ask.call_id, action="check", cycle=cycle, result=response)
                        break
                if checkpoint and checked == 0 and writes:
                    checkpoint.accept(writes)
                elif checkpoint and checked != 0:
                    def observe(event, changes):
                        record(event=event, id=ask.call_id, changes=changes, failure=response)
                    if checkpoint.restore(writes, APP, observe):
                        recoveries += 1
                        last_failure = response + "\nRestored the last locally passing submission. Try a different change addressing this failure."
                        checked = None
                        response = last_failure
            elif action=="finish" and writes and checked is not None:
                record(event="tool", id=ask.call_id, action=action, result="finished")
                break
            else: raise ValueError("Write a nonempty submission, then run check before finish")
            record(event="tool", id=ask.call_id, action=action, path=obj.get("path"), offset=obj.get("offset", 0), result=response[:3500], chars=len(response))
            history.append([action,obj.get("path"),response])
        except (ValueError,OSError,TypeError,KeyError,AttributeError,subprocess.TimeoutExpired) as error:
            record(event="tool", id=ask.call_id, action=obj.get("action") if isinstance(obj, dict) else None, error=str(error))
            history.append(["error",str(error)])
    obs={"files":writes,"check_exit":checked,"solver_calls":calls,
         "first_write_call":first_write,"first_check_call":first_check}
    if cycle:
        obs.update(cycle=cycle, check_command=CHECKS[TASK], check_output=check_output,
                   regression=regression, local_baseline=not baseline and checked == 0 and bool(writes))
    if checkpoint:
        obs["verified_checkpoint_restores"] = recoveries
    (WORK/"observation.json").write_text(json.dumps(obs))
    return obs
