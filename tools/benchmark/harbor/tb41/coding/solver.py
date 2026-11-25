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


def snapshot(names):
    digests = {}
    for name in names:
        path = source(name)
        digests[name] = hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None
    return digests


def check_sources(names, trigger, timeout):
    before = snapshot(names)
    started = time.monotonic()
    try:
        proc = subprocess.run(CHECKS[TASK], cwd=APP, stdout=subprocess.PIPE,
                              stderr=subprocess.STDOUT, timeout=timeout)
        code = proc.returncode
        output = proc.stdout.decode(errors="replace")[-3500:]
    except subprocess.TimeoutExpired:
        code, output = None, "local check timed out"
    except OSError as error:
        code, output = None, str(error)
    return {"command": CHECKS[TASK], "files": before, "exit": code, "output": output,
            "seconds": time.monotonic() - started, "trigger": trigger}


def matches(receipt, files):
    return bool(receipt and files and all(files.values()) and receipt["files"] == files)


def make_checklist(task, plan):
    answer = ask(
        "Turn the accepted roadmap into ordered, independently reviewable deliverables. "
        "Use one concise criterion per line, implementation prerequisites first. Include every "
        "required caller and behavioral contract. Separate implementation from external-only "
        "verification. Return only the lines. Task: " + task + "\nRoadmap: " + plan,
        stage="checklist")
    criteria = dict.fromkeys(line.strip() for line in answer.splitlines() if line.strip())
    if not criteria:
        raise ValueError("Empty experiment checklist")
    return [{"criterion": line, "status": "pending"} for line in criteria]


def review_item(item, evidence, receipt):
    context = json.dumps({"criterion": item["criterion"], "editor_observations": evidence,
                          "last_check": receipt, "available_check": CHECKS[TASK]}, ensure_ascii=False)
    assessment = ask(
        "Assess this single checklist item against the observed work. Cite the source or check "
        "that demonstrates it, or name the next local action. A write hash or build alone cannot "
        "prove runtime behavior. Defer only a verification that the local tools cannot perform. "
        "Any remaining local implementation belongs in pending. " + context,
        stage="checklist_review")
    status = ask("Classify the assessment against these observations. Use demonstrated only for "
                 "a supported criterion, deferred only for unavailable verification, otherwise pending. "
                 + context + "\nAssessment: " + assessment,
                 ["pending", "demonstrated", "deferred"], stage="checklist_review")
    if status == "demonstrated" and not (evidence or receipt):
        status, assessment = "pending", "Inspect a source or run the available check to supply evidence."
    item.update(status=status, assessment=assessment)


def solve(task,plan=""):
    public=[]
    for directory,children,files in os.walk(APP):
        children[:]=[v for v in children if v not in ("node_modules",".git","__pycache__","build","dist")]
        public.extend(str((Path(directory)/f).relative_to(APP)) for f in files)
        if len(public)>600: break
    history=[]; writes={}; receipt=None; calls=0
    check_reserve = 5.0
    checklist = make_checklist(task, plan) if plan else []
    evidence = []
    touched = set()
    first_write = first_check = None
    deadline=float(Path("/work/deadline").read_text())
    # Preserve the existing post-solver window and allow for a pending model reply.
    edit_deadline = deadline - 100
    previous_item = None
    while time.time() < edit_deadline - 85 - check_reserve:
        active = next((item for item in checklist if item["status"] == "pending"), None)
        if active is not previous_item:
            evidence.clear()
            touched.clear()
        previous_item = active
        focus = ""
        if checklist:
            focus = (f"Checklist: {json.dumps(checklist, ensure_ascii=False)}\n"
                     f"Current criterion: {active['criterion'] if active else 'final source check'}. "
                     "Work on the current criterion. Call finish to request its evidence review. "
                     "After all items have been reviewed, check the final files and finish.")
        prompt=f"""{task}
{plan}
{focus}
Public source files: {sorted(public)[:120]}
Files you have edited: {list(writes)}
Recent tool results: {json.dumps(history[-12:],ensure_ascii=False)[-35000:]}
Work toward the full official task. Call exactly one of the supplied editor tools. Available actions:
{{"action":"read","path":"relative/file","offset":0}} (next 9000 characters)
{{"action":"write","path":"relative/file","content":"complete source file"}}
{{"action":"check"}} (run task compiler/build/import smoke)
{{"action":"finish"}} to review the current checklist item, or submit checked files when no item is pending.
For vf2 use fast_networkx.py or a fast_networkx package. For ks use solution.cpp. For React work in src/.
If a local check fails, use its output to fix the source. No remote tools or private test data."""
        answer=ask(prompt); calls+=1
        editor_id = ask.call_id
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
                path.parent.mkdir(parents=True,exist_ok=True); path.write_text(content)
                writes[str(path.relative_to(APP))]=hashlib.sha256(path.read_bytes()).hexdigest()
                if first_write is None:
                    first_write = calls
                name = str(path.relative_to(APP))
                for item in checklist:
                    if name in item.get("sources", {}) and item["sources"][name] != writes[name]:
                        item.update(status="pending", assessment="Source changed since review")
                response="written"
            elif action=="check":
                if first_check is None:
                    first_check = calls
                receipt = check_sources(writes, "model", min(100, max(1, edit_deadline - time.time())))
                check_reserve = min(100, max(check_reserve, receipt["seconds"] * 1.5 + 2))
                response = f"exit {receipt['exit']}: " + receipt["output"]
            elif action == "finish" and active:
                if time.time() + 170 + check_reserve >= edit_deadline:
                    record(event="tool", id=editor_id, action=action, result="Review pending at finalization")
                    break
                current_receipt = receipt if matches(receipt, snapshot(writes)) else None
                review_item(active, evidence, current_receipt)
                if current_receipt:
                    touched.update(current_receipt["files"])
                active["sources"] = snapshot(touched)
                response = active["status"] + ": " + active["assessment"]
                if active["status"] != "pending":
                    evidence.clear()
                    touched.clear()
            elif action=="finish" and matches(receipt, snapshot(writes)) and receipt["exit"] is not None:
                record(event="tool", id=editor_id, action=action, result="finished")
                break
            else: raise ValueError("Write a nonempty submission, then run check before finish")
            if active and action in ("read", "write", "check"):
                if action in ("read", "write"):
                    touched.add(str(path.relative_to(APP)))
                observed = content if action == "write" else response
                evidence.append([action, obj.get("path"), observed[:3000]])
                evidence = evidence[-12:]
            record(event="tool", id=editor_id, action=action, path=obj.get("path"), offset=obj.get("offset", 0), result=response[:3500], chars=len(response),
                   **({"check": receipt} if action == "check" else {}))
            history.append([action,obj.get("path"),response])
        except (ValueError,OSError,TypeError,KeyError,AttributeError,subprocess.TimeoutExpired) as error:
            record(event="tool", id=editor_id, action=obj.get("action") if isinstance(obj, dict) else None, error=str(error))
            history.append(["error",str(error)])
    files = snapshot(writes)
    if files and (not matches(receipt, files) or receipt["exit"] is None):
        receipt = check_sources(writes, "finalization", min(100, max(1, deadline - time.time() - 85)))
        files = snapshot(writes)
    current = matches(receipt, files)
    obs={"checklist": checklist, "files":files,"check_exit":receipt["exit"] if current else None,"solver_calls":calls,
         "last_check": receipt, "check_matches_submission": current,
         "first_write_call":first_write,"first_check_call":first_check}
    (WORK/"observation.json").write_text(json.dumps(obs))
    return obs
