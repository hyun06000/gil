"""Shared source-editing tools for the three pinned CPU tasks."""
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import time

from mailbox_client import ask, record

WORK = Path("/work/project")
APP = Path("/app")
TASK = os.environ["TB_TASK"]
RUN_SECONDS = 100
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


def snapshot():
    """Hashes of the files a submission may contain, as they are on disk now."""
    found = {}
    for directory, children, files in os.walk(APP):
        children[:] = [c for c in children if c not in (".git", "node_modules", "__pycache__")]
        for name in files:
            path = Path(directory, name)
            if path.is_symlink() or not path.is_file():
                continue
            try:
                source(str(path), True)
            except ValueError:
                continue
            found[path.relative_to(APP).as_posix()] = hashlib.sha256(path.read_bytes()).hexdigest()
    return found


def solve(task,plan="", *, cycle=None, baseline=None, verify_ref=None, failed_check=None):
    public=[]
    for directory,children,files in os.walk(APP):
        children[:]=[v for v in children if v not in ("node_modules",".git","__pycache__","build","dist")]
        public.extend(str((Path(directory)/f).relative_to(APP)) for f in files)
        if len(public)>600: break
    history=[]; writes={}; checked=None; calls=0
    ran = set()
    checkpoint = None
    if plan and cycle is None:
        from checkpoint import Checkpoint
        checkpoint = Checkpoint()
    activity = None
    if cycle:
        from verify_activity import VerifyActivity, digest
        activity = VerifyActivity(verify_ref or cycle, (baseline or {}).get("files", {}), failed_check)
        from checkpoint import capture_file
        record(event="verify_plan", cycle=cycle, plan=plan)
        if baseline:
            writes.update(baseline["files"])
    check_output = ""
    original_hashes = {}
    regression = False
    stop = "deadline"
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
{{"action":"edit","path":"relative/file","old":"text that occurs exactly once in the file","new":"its replacement"}}
{{"action":"run","command":"shell command"}} (sh in /app, no network, {RUN_SECONDS} s limit: build, execute or time anything, including the task's own commands and test scripts you write under /tmp; submission files it changes are recorded)
{{"action":"check"}} (run task compiler/build/import smoke)
{{"action":"finish"}} after editing the solution and checking it. An incomplete solution can be graded.
For vf2 use fast_networkx.py or a fast_networkx package. For ks use solution.cpp. For React work in src/.
If a local check fails, use its output to fix the source. No remote tools or private test data."""
        if checkpoint and last_failure:
            prompt += "\nLast failed experiment and recovery: " + last_failure
        if activity:
            prompt += activity.prompt(CHECKS[TASK], checked)
        answer=ask(prompt, notes=True) if activity else ask(prompt); calls+=1
        obj = {}
        try:
            obj=json.loads(answer.strip().removeprefix("```json").removeprefix("```").removesuffix("```"))
            action=obj["action"]
            activity_event = {}
            if activity:
                activity.note(calls, action, obj.get("note"))
            if action=="read":
                path=source(obj["path"])
                start=max(0,int(obj.get("offset",0)))
                if activity:
                    data = path.read_bytes()
                    # Match Path.read_text() universal newline semantics.
                    content = data.decode().replace("\r\n", "\n").replace("\r", "\n")
                    page = content[start:start+9000]
                    name = path.relative_to(APP).as_posix()
                    activity_event, response = activity.page(name, data, start, page, start + len(page) >= len(content))
                else:
                    response=path.read_text()[start:start+9000]
            elif action in ("write", "edit"):
                path=source(obj["path"],True)
                if action == "edit":
                    text = path.read_text()
                    found = text.count(obj["old"]) if obj["old"] else 0
                    if found != 1:
                        raise ValueError(f"old text occurs {found} times in {obj['path']}; it must occur exactly once")
                    content = text.replace(obj["old"], obj["new"])
                else:
                    content=obj["content"]
                if not content.strip(): raise ValueError("empty source file")
                previous = None
                if activity:
                    previous = digest(path.read_bytes()) if path.exists() else None
                    original_hashes.setdefault(path, previous)
                if checkpoint:
                    checkpoint.before_write(path)
                unchanged = bool(activity and previous == digest(content.encode()))
                if not unchanged:
                    path.parent.mkdir(parents=True,exist_ok=True); path.write_text(content)
                writes[str(path.relative_to(APP))]=hashlib.sha256(path.read_bytes()).hexdigest()
                if cycle:
                    capture_file(WORK, APP, path)
                    activity.write(path.relative_to(APP).as_posix(), writes[str(path.relative_to(APP))], previous)
                if first_write is None:
                    first_write = calls
                if not unchanged:
                    checked = None
                    check_output = ""
                response = "unchanged" if unchanged else "written"
            elif action=="run":
                command = obj["command"]
                if not command.strip(): raise ValueError("empty command")
                before = snapshot()
                if checkpoint:
                    for name in before:
                        checkpoint.before_write(APP / name)
                proc = subprocess.Popen(["sh", "-c", command], cwd=APP, stdout=subprocess.PIPE,
                                        stderr=subprocess.STDOUT, start_new_session=True)
                try:
                    output, _ = proc.communicate(timeout=RUN_SECONDS)
                    code = proc.returncode
                except subprocess.TimeoutExpired:
                    os.killpg(proc.pid, signal.SIGKILL)
                    output, _ = proc.communicate()
                    code = f"none, stopped after {RUN_SECONDS} s"
                else:
                    # Nothing the command left in the background outlives it.
                    try: os.killpg(proc.pid, signal.SIGKILL)
                    except OSError: pass
                after = snapshot()
                changed = sorted(n for n in before.keys() | after.keys() if before.get(n) != after.get(n))
                for name in changed:
                    path = APP / name
                    if name in after:
                        writes[name] = after[name]
                        if checkpoint and checkpoint.passed is not None and name not in before:
                            checkpoint.before.setdefault(path, None)
                        if cycle:
                            capture_file(WORK, APP, path)
                            activity.write(name, after[name], before.get(name))
                    else:
                        writes.pop(name, None)
                        if cycle:
                            (WORK / "submission" / name).unlink(missing_ok=True)
                state = (command, tuple(sorted(after.items())))
                if activity and not changed:
                    activity.progress(state not in ran)
                ran.add(state)
                if changed:
                    checked = None
                    check_output = ""
                    if first_write is None:
                        first_write = calls
                response = f"exit {code}: " + output.decode(errors="replace")[-3500:]
                if changed:
                    response += f"\nSubmission files changed by this command: {changed}"
                activity_event["command"] = command
            elif action=="check" and activity and checked is not None:
                # No file changed since this result: running it again observes nothing new.
                activity.progress(False)
                response = ("Not run again: no file changed since this check. Change a file if a requirement is still "
                            "open, otherwise finish. " + check_output)
            elif action=="check":
                if activity:
                    activity.progress(True)
                if first_check is None:
                    first_check = calls
                proc=subprocess.run(CHECKS[TASK],cwd=APP,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=100)
                checked=proc.returncode
                response=f"exit {checked}: "+proc.stdout.decode(errors="replace")[-3500:]
                if cycle:
                    for name, expected_hash in writes.items():
                        if hashlib.sha256(source(name, True).read_bytes()).hexdigest() != expected_hash:
                            raise RuntimeError(f'Submission changed during local check: {name}')
                    check_output = response
                    if checked:
                        activity.failure = {"verify": activity.ref, "exit": checked, "output": response}
                    else:
                        activity.failure = None
                    regression = bool(checked and baseline and any(
                        writes.get(str(path.relative_to(APP))) != digest for path, digest in original_hashes.items()))
                    if (not baseline and checked == 0 and writes) or regression:
                        record(event="tool", id=ask.call_id, action="check", cycle=cycle, result=response)
                        stop = "regression" if regression else "local_baseline"
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
                stop = "finished"
                break
            else: raise ValueError("Write a nonempty submission, then run check before finish")
            if activity:
                activity_event["note"] = obj.get("note")
            record(event="tool", id=ask.call_id, action=action, path=obj.get("path"), offset=obj.get("offset", 0), result=response[:3500], chars=len(response), **activity_event)
            if activity and action == "read" and not activity_event["held"]:
                # The page itself travels under "Pages you hold"; the sliding results keep its receipt.
                history.append([action, obj.get("path"), f"page [{start},{start + len(response)}) received"])
            else:
                history.append([action,obj.get("path"),response])
        except (ValueError,OSError,TypeError,KeyError,AttributeError,subprocess.TimeoutExpired) as error:
            record(event="tool", id=ask.call_id, action=obj.get("action") if isinstance(obj, dict) else None, error=str(error))
            history.append(["error",str(error)])
            if activity:
                activity.progress(False)
        if activity and activity.stalled():
            record(event="verify_stalled", id=ask.call_id, cycle=cycle, calls=activity.stall)
            stop = "stalled"
            break
    obs={"files":writes,"check_exit":checked,"solver_calls":calls,
         "first_write_call":first_write,"first_check_call":first_check}
    if cycle:
        (WORK/"verify-activity.json").write_text(json.dumps(activity.state(CHECKS[TASK], checked)))
        obs.update(repeat_reads=activity.repeats, cycle=cycle, check_command=CHECKS[TASK], check_output=check_output,
                   regression=regression, local_baseline=not baseline and checked == 0 and bool(writes),
                   stop=stop, refused_reads=activity.refused, notes=len(activity.notes))
    if checkpoint:
        obs["verified_checkpoint_restores"] = recoveries
    (WORK/"observation.json").write_text(json.dumps(obs))
    return obs
