"""Shared source-editing tools for the three pinned CPU tasks."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

from mailbox_client import ask

WORK = Path("/work/project")
APP = Path("/app")
TASK = os.environ["TB_TASK"]
CHECKS = {
    "ks-solver-cpp": ["g++", "-O3", "-std=c++17", "-DKS_SOLVER_LIBRARY", "-I/app", "-c", "solution.cpp", "-o", "/tmp/ks.o"],
    "react-lead-form": ["npm", "run", "build"],
    "vf2-speedup-networkx": ["python3", "-c", "import fast_networkx as f; g=f.Graph(); g.add_edge(1,2); assert f.vf2pp_is_isomorphic(g,g); print('import and graph smoke passed')"],
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


def solve(task,plan=""):
    public=[]
    for directory,children,files in os.walk(APP):
        children[:]=[v for v in children if v not in ("node_modules",".git","__pycache__","build","dist")]
        public.extend(str((Path(directory)/f).relative_to(APP)) for f in files)
        if len(public)>600: break
    history=[]; writes={}; checked=None; calls=0
    deadline=float(Path("/work/deadline").read_text())
    while time.time()<deadline-100:
        prompt=f"""{task}
{plan}
Public source files: {sorted(public)[:120]}
Files you have edited: {list(writes)}
Recent tool results: {json.dumps(history[-12:],ensure_ascii=False)[-35000:]}
Work toward the full official task. Respond with one JSON object. Available actions:
{{"action":"read","path":"relative/file","offset":0}} (next 9000 characters)
{{"action":"write","path":"relative/file","content":"complete source file"}}
{{"action":"check"}} (run task compiler/build/import smoke)
{{"action":"finish"}} after editing the solution and checking it. An incomplete solution can be graded.
For vf2 use fast_networkx.py or a fast_networkx package. For ks use solution.cpp. For React work in src/.
If a local check fails, use its output to fix the source. No remote tools or private test data."""
        answer=ask(prompt); calls+=1
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
                path.parent.mkdir(parents=True,exist_ok=True); path.write_text(content)
                writes[str(path.relative_to(APP))]=hashlib.sha256(path.read_bytes()).hexdigest()
                checked=None; response="written"
            elif action=="check":
                proc=subprocess.run(CHECKS[TASK],cwd=APP,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=100)
                checked=proc.returncode
                response=f"exit {checked}: "+proc.stdout.decode(errors="replace")[-3500:]
            elif action=="finish" and writes and checked is not None: break
            else: raise ValueError("Write a nonempty submission, then run check before finish")
            history.append([action,obj.get("path"),response])
        except (ValueError,OSError,TypeError,KeyError,AttributeError,subprocess.TimeoutExpired) as error:
            history.append(["error",str(error)])
    obs={"files":writes,"check_exit":checked,"solver_calls":calls}
    (WORK/"observation.json").write_text(json.dumps(obs))
    return obs
