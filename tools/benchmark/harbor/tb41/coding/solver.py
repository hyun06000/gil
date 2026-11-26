"""Shared source-editing tools for the three pinned CPU tasks."""
import difflib
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
SWEEP_SECONDS = 240     # for running every stored probe again when a Verify ends
# What one request can show. The model sees every file it has read, whole and current, instead of
# the last few pages: a task that needs its specification and its own code side by side fits.
PAGE = 60000            # characters of one file per read
VIEW_BUDGET = 240000    # characters of file text in a request; the least recently touched file leaves first
RESULTS_KEPT = 30       # recent tool results in a request, within RESULTS_BUDGET characters
RESULTS_BUDGET = 60000
OUTPUT_LIMIT = 12000    # characters kept from the end of a command's or check's output
# Calls in a row that bring no new file text, no new file content and no new command result.
STALL_WARNING = 2
# What the model wrote to itself: the only memory of intent between requests, in either arm.
NOTES_KEPT = 16
NOTE_LIMIT = 400
STALL_LIMIT = 12
# What a submission may contain, in the words a refusal uses.
WRITABLE = {
    "ks-solver-cpp": "solution.cpp and .hpp files other than oracle.hpp",
    "react-lead-form": ".ts, .tsx and .css files under src/ (outside src/spec/ and src/data/), and package.json, "
                       "tsconfig.json, vite.config.ts and index.html",
    "vf2-speedup-networkx": "fast_networkx.py and anything under fast_networkx/",
}
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
          # The task collects these project files with src/ as the submission, and its verifier builds with them.
          "react-lead-form": rel in ("package.json", "tsconfig.json", "vite.config.ts", "index.html") or (
              rel.startswith("src/") and not rel.startswith(("src/spec/","src/data/")) and rel.endswith((".ts",".tsx",".css"))),
          "vf2-speedup-networkx": rel=="fast_networkx.py" or rel.startswith("fast_networkx/"),
        }
        if not allowed[TASK]:
            raise ValueError(f"{rel} is not part of the submission and cannot be changed by any tool. "
                             f"Only these are graded and writable: {WRITABLE[TASK]}. Choose a writable source path "
                             "or solve it inside those files.")
    return path


def snapshot_of(files):
    """The hashes of the named submission files as they are on disk now (absent files left out)."""
    return {name: hashlib.sha256((APP / name).read_bytes()).hexdigest() for name in files if (APP / name).is_file()}


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


class View:
    """The file text a request carries: one entry per file page, always as the file is on disk now."""

    def __init__(self):
        self.pages = {}

    def has(self, name, offset, sha):
        page = self.pages.get((name, offset))
        return bool(page) and page["sha256"] == sha

    def put(self, name, offset, content, sha):
        text = content[offset:offset + PAGE]
        self.pages.pop((name, offset), None)
        self.pages[(name, offset)] = {"path": name, "offset": offset, "end": offset + len(text),
                                      "chars": len(content), "sha256": sha, "text": text}
        while len(self.pages) > 1 and sum(len(p["text"]) for p in self.pages.values()) > VIEW_BUDGET:
            del self.pages[next(iter(self.pages))]
        return self.pages[(name, offset)]

    def refresh(self):
        """A file changed by a write, an edit or a command is shown as it is now; a removed one leaves."""
        for (name, offset), page in list(self.pages.items()):
            path = APP / name
            if not path.is_file():
                del self.pages[(name, offset)]
                continue
            data = path.read_bytes()
            sha = hashlib.sha256(data).hexdigest()
            if sha != page["sha256"]:
                try:
                    content = text_of(data)
                except ValueError:
                    del self.pages[(name, offset)]
                    continue
                order = list(self.pages)
                self.put(name, offset, content, sha)
                # Keep its place: a refresh is not a new look at the file.
                self.pages = {key: self.pages[key] for key in order if key in self.pages}

    def shown(self):
        return [{k: v for k, v in page.items() if k != "sha256"} for page in self.pages.values()]


class Stall:
    """Counts calls in a row that observed nothing new, in either arm."""

    def __init__(self, baseline):
        self.count = 0
        self.pages = set()
        self.contents = {name: {sha} for name, sha in baseline.items()}
        self.runs = set()

    def step(self, new):
        self.count = 0 if new else self.count + 1

    def read(self, name, sha, offset, shown):
        # Text the request already carries is nothing new, whichever call put it there.
        new = not shown and (name, sha, offset) not in self.pages
        self.pages.add((name, sha, offset))
        self.step(new)

    def write(self, name, sha, previous):
        known = self.contents.setdefault(name, set())
        if previous:
            known.add(previous)
        # Content the session already had for the file is a return, not a new attempt.
        self.step(previous != sha and sha not in known)
        known.add(sha)

    def run(self, command, files):
        state = (command, tuple(sorted(files.items())))
        self.step(state not in self.runs)
        self.runs.add(state)


def edit_hint(text, old, found):
    """Where a passage that did not match once went wrong, so the next attempt is not the same guess."""
    if found > 1:
        return "Include a neighbouring line so the passage is unique."
    if not old:
        return "Give the passage to replace."
    if " ".join(old.split()) in " ".join(text.split()):
        return "It matches except for spaces or line breaks: copy the passage exactly as the file in view has it."
    lines = text.splitlines()
    first = next((line.strip() for line in old.splitlines() if line.strip()), "")
    at = next((number for number, line in enumerate(lines, 1) if first and first in line), None)
    if at:
        return (f"Its first line is at line {at} of the file, but what follows there differs from your passage. "
                "Use a shorter passage or write the whole file.")
    close = difflib.get_close_matches(first, [line.strip() for line in lines], n=1, cutoff=0.6)
    return ((f"No line contains its first line; the closest line in the file is: {close[0][:160]} " if close
             else "No line of the file resembles its first line. ")
            + "The file may have changed since you composed the passage: use what is in view now.")


def text_of(data):
    # Match Path.read_text() universal newline semantics.
    return data.decode().replace("\r\n", "\n").replace("\r", "\n")


def recent(history):
    """The latest results that fit, whole: an entry is kept or left out, never cut in the middle."""
    kept, size = [], 0
    for entry in reversed(history[-RESULTS_KEPT:]):
        size += len(json.dumps(entry, ensure_ascii=False))
        if kept and size > RESULTS_BUDGET:
            break
        kept.append(entry)
    return kept[::-1]


def run_command(command, limit):
    """Runs a shell command in the task directory; nothing it starts outlives it."""
    proc = subprocess.Popen(["sh", "-c", command], cwd=APP, stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, start_new_session=True)
    try:
        output, _ = proc.communicate(timeout=limit)
        code = proc.returncode
    except subprocess.TimeoutExpired:
        os.killpg(proc.pid, signal.SIGKILL)
        output, _ = proc.communicate()
        code = f"none, stopped after {limit} s"
    else:
        try: os.killpg(proc.pid, signal.SIGKILL)
        except OSError: pass
    return code, output.decode(errors="replace")


LEDGER_RULES = (
    "\nCriteria ledger of this Chain. A criterion holds only when a command run with its id exits 0 on the files "
    "as they are now. Write each probe so that it exits non-zero when the requirement is violated: assert the "
    "observable result; for something that must not change, compare its bytes before and after; for a failure "
    "case, first create the failing condition (for example a path that cannot be written) and assert what must "
    "remain. Keep probe scripts under /tmp. Changing a submission file makes earlier results stale, and every "
    "stored probe is run again on the final files when this step ends. This experiment succeeds when its targets "
    "hold and nothing that held before is broken; the other open criteria are for later experiments.")


def solve(task,plan="", *, cycle=None, baseline=None, verify_ref=None, failed_check=None, criteria=None, targets=()):
    public=[]
    for directory,children,files in os.walk(APP):
        children[:]=[v for v in children if v not in ("node_modules",".git","__pycache__","build","dist")]
        public.extend(str((Path(directory)/f).relative_to(APP)) for f in files)
        if len(public)>600: break
    history=[]; writes={}; checked=None; calls=0
    view = View()
    refusal = ["", 0]
    notes = []
    stall = Stall((baseline or {}).get("files", {}))
    checkpoint = None
    if plan and cycle is None:
        from checkpoint import Checkpoint
        checkpoint = Checkpoint()
    activity = None
    if cycle:
        from verify_activity import VerifyActivity
        activity = VerifyActivity(verify_ref or cycle, (baseline or {}).get("files", {}), failed_check)
        from checkpoint import capture_file
        record(event="verify_plan", cycle=cycle, plan=plan)
        if baseline:
            writes.update(baseline["files"])
    ledger = None
    if cycle and criteria:
        from criteria import Ledger, digest as submission
        ledger = Ledger(criteria)
    closing = False
    check_output = ""
    original_hashes = {}
    regression = False
    stop = "deadline"
    recoveries = 0
    last_failure = ""
    first_write = first_check = None
    deadline=float((WORK.parent / "deadline").read_text())
    while time.time()<deadline-100:
        view.refresh()
        prompt=f"""{task}
{plan}
Public source files: {sorted(public)[:120]}
Files you have edited: {list(writes)}
Files in view, as they are on disk now (a file you write, edit or change by a command is updated here; reading one of these again brings nothing new): {json.dumps(view.shown(),ensure_ascii=False)}
Recent tool results: {json.dumps(recent(history),ensure_ascii=False)}
Work toward the full official task. Call exactly one of the supplied editor tools. Available actions:
{{"action":"read","path":"relative/file","offset":0}} (puts up to {PAGE} characters of the file in view)
{{"action":"write","path":"relative/file","content":"complete source file"}}
{{"action":"edit","path":"relative/file","old":"text that occurs exactly once in the file","new":"its replacement"}}
{{"action":"run","command":"shell command"}} (sh in /app, no network, {RUN_SECONDS} s limit: build, execute or time anything, including the task's own commands and test scripts you write under /tmp; submission files it changes are recorded)
{{"action":"check"}} (run task compiler/build/import smoke)
{{"action":"finish"}} after editing the solution and checking it. An incomplete solution can be graded.
For vf2 use fast_networkx.py or a fast_networkx package. For ks use solution.cpp. For React work in src/.
If a local check fails, use its output to fix the source. No remote tools or private test data."""
        if checkpoint and last_failure:
            prompt += "\nLast failed experiment and recovery: " + last_failure
        # The task text may name a longer allowance; this is the one that applies here.
        prompt += f"\nTime left in this session: {max(0, int((deadline - 100 - time.time()) // 60))} minutes."
        if stall.count >= STALL_WARNING:
            prompt += (f"\n{stall.count} calls in a row brought no new file text, no new file content and no new "
                       f"command result. After {STALL_LIMIT - stall.count} more, editing ends with the files as they are. "
                       "The full current text of every file you have read or written is above under 'Files in view': "
                       "reading one again returns nothing. Change a file, run something you have not run, or finish.")
        prompt += ('\nEvery tool call takes "note": what the material in this request settled and what you do next. '
                   "Each request is built anew: your notes, the files in view and the recent results are all that "
                   "carries over, so put any conclusion you want to keep in the note.")
        if notes:
            prompt += "\nYour notes so far, oldest first: " + json.dumps(notes[-NOTES_KEPT:], ensure_ascii=False)
        if ledger:
            prompt += (LEDGER_RULES + f"\nTargets of this experiment: {list(targets)}\n"
                       + json.dumps(ledger.shown(submission(writes), targets), ensure_ascii=False))
        if activity:
            prompt += activity.prompt(CHECKS[TASK], checked, stall.count)
        answer=ask(prompt, notes=True, ledger=True) if ledger else ask(prompt, notes=True); calls+=1
        obj = {}
        try:
            obj=json.loads(answer.strip().removeprefix("```json").removeprefix("```").removesuffix("```"))
            action=obj["action"]
            activity_event = {}
            if action == "stop":
                # The host stopped asking the model (the trial's cost limit): what is on disk is what gets graded.
                record(event="stopped", id=ask.call_id, cycle=cycle, reason=obj.get("reason"))
                stop = str(obj.get("reason") or "stopped")
                calls -= 1
                break
            note = obj.get("note")
            if isinstance(note, str) and note.strip():
                notes.append({"call": calls, "action": action, "note": note.strip()[:NOTE_LIMIT]})
            if action=="read":
                path=source(obj["path"])
                start=max(0,int(obj.get("offset",0)))
                data = path.read_bytes()
                content = text_of(data)
                name = path.relative_to(APP).as_posix()
                sha = hashlib.sha256(data).hexdigest()
                shown = view.has(name, start, sha)
                page = view.put(name, start, content, sha)
                stall.read(name, sha, start, shown)
                span = f"{name} [{start},{page['end']}) of {len(content)} characters"
                response = (f"Already in view and unchanged: {span}. Change a file, run a command, or read another file."
                            if shown else f"Now in view: {span}")
                activity_event = {"sha256": sha, "shown": shown}
                if activity:
                    activity_event["read_count"] = activity.read(name, data, start, page["end"], page["end"] >= len(content), shown)
                # The transcript keeps the start of what the model was shown.
                logged = response if shown else page["text"]
            elif action in ("write", "edit"):
                path=source(obj["path"],True)
                if action == "edit":
                    text = path.read_text()
                    found = text.count(obj["old"]) if obj["old"] else 0
                    if found != 1:
                        raise ValueError(f"old text occurs {found} times in {obj['path']}; it must occur exactly once. "
                                         + edit_hint(text, obj["old"], found))
                    content = text.replace(obj["old"], obj["new"])
                else:
                    content=obj["content"]
                if not content.strip(): raise ValueError("empty source file")
                previous = hashlib.sha256(path.read_bytes()).hexdigest() if path.exists() else None
                if activity:
                    original_hashes.setdefault(path, previous)
                if checkpoint:
                    checkpoint.before_write(path)
                unchanged = previous == hashlib.sha256(content.encode()).hexdigest()
                if not unchanged:
                    path.parent.mkdir(parents=True,exist_ok=True); path.write_text(content)
                writes[str(path.relative_to(APP))]=hashlib.sha256(path.read_bytes()).hexdigest()
                name = path.relative_to(APP).as_posix()
                stall.write(name, writes[name], previous)
                # What was written is in view at once: no read is needed to see it.
                view.put(name, 0, text_of(path.read_bytes()), writes[name])
                if cycle:
                    capture_file(WORK, APP, path)
                    activity.write(path.relative_to(APP).as_posix(), writes[str(path.relative_to(APP))], previous)
                if first_write is None:
                    first_write = calls
                if not unchanged:
                    checked = None
                    check_output = ""
                response = ("unchanged: the file already had this content" if unchanged
                            else "written; the file is in view as it is now")
            elif action=="run":
                command = obj["command"]
                if not command.strip(): raise ValueError("empty command")
                before = snapshot()
                if checkpoint:
                    for name in before:
                        checkpoint.before_write(APP / name)
                criterion = (obj.get("criterion") or "").strip() if ledger else ""
                if criterion and criterion not in ledger.items:
                    raise ValueError(f"{criterion} is not a criterion of the ledger; use one of {list(ledger.items)} or leave it empty")
                code, output = run_command(command, RUN_SECONDS)
                after = snapshot()
                changed = sorted(n for n in before.keys() | after.keys() if before.get(n) != after.get(n))
                for name in changed:
                    path = APP / name
                    if name in after:
                        writes[name] = after[name]
                        if checkpoint and checkpoint.passed is not None and name not in before:
                            checkpoint.before.setdefault(path, None)
                        stall.write(name, after[name], before.get(name))
                        if cycle:
                            capture_file(WORK, APP, path)
                            activity.write(name, after[name], before.get(name))
                    else:
                        writes.pop(name, None)
                        if cycle:
                            (WORK / "submission" / name).unlink(missing_ok=True)
                if not changed:
                    stall.run(command, after)
                else:
                    stall.runs.add((command, tuple(sorted(after.items()))))
                if changed:
                    checked = None
                    check_output = ""
                    if first_write is None:
                        first_write = calls
                response = f"exit {code}: " + output[-OUTPUT_LIMIT:]
                if criterion:
                    held = ledger.record(criterion, command, code, output, submission(writes))
                    response = f"criterion {criterion} {held} on the current files. " + response
                    activity_event["criterion"] = criterion
                    record(event="probe", id=ask.call_id, cycle=cycle, criterion=criterion, status=held, exit=code)
                if changed:
                    response += f"\nSubmission files changed by this command: {changed}"
                activity_event["command"] = command
            elif action=="check" and checked is not None and check_output:
                # No file changed since this result: running it again observes nothing new.
                stall.step(False)
                response = ("Not run again: no file changed since this check. Change a file if a requirement is still "
                            "open, otherwise finish. " + check_output)
            elif action=="check":
                stall.step(True)
                if first_check is None:
                    first_check = calls
                proc=subprocess.run(CHECKS[TASK],cwd=APP,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=100)
                checked=proc.returncode
                response=f"exit {checked}: "+proc.stdout.decode(errors="replace")[-OUTPUT_LIMIT:]
                check_output = response
                if cycle:
                    for name, expected_hash in writes.items():
                        if hashlib.sha256(source(name, True).read_bytes()).hexdigest() != expected_hash:
                            raise RuntimeError(f'Submission changed during local check: {name}')
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
                waiting = [key for key in targets if ledger and ledger.status(key, submission(writes)) != "holds"]
                if waiting and not closing:
                    closing = True
                    raise ValueError(f"Targets not holding on the current files: {waiting}. Run their probes or fix what "
                                     "they show. Calling finish again closes this experiment with them open.")
                record(event="tool", id=ask.call_id, action=action, result="finished")
                stop = "finished"
                break
            else: raise ValueError("Write a nonempty submission, then run check before finish")
            activity_event["note"] = obj.get("note")
            if action != "read":
                logged = response
            record(event="tool", id=ask.call_id, action=action, path=obj.get("path"), offset=obj.get("offset", 0), result=logged[:3500], chars=len(logged), **activity_event)
            history.append([action, obj.get("path") or obj.get("command"), response])
            refusal = ["", 0]
        except (ValueError,OSError,TypeError,KeyError,AttributeError,subprocess.TimeoutExpired) as error:
            message = str(error)
            refusal = [message, refusal[1] + 1] if message == refusal[0] else [message, 1]
            if refusal[1] > 1:
                message += f" (refused {refusal[1]} times in a row with the same answer: repeating it will not change the result)"
            record(event="tool", id=ask.call_id, action=obj.get("action") if isinstance(obj, dict) else None, error=message)
            history.append(["error", message])
            stall.step(False)
        if stall.count >= STALL_LIMIT:
            record(event="verify_stalled" if cycle else "stalled", id=ask.call_id, cycle=cycle, calls=stall.count)
            stop = "stalled"
            break
    if ledger:
        # The final files are what counts: the local check and every stored probe are run on them
        # again, without the model.
        if checked is None and writes:
            proc = subprocess.run(CHECKS[TASK], cwd=APP, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=100)
            checked = proc.returncode
            check_output = f"exit {checked}: " + proc.stdout.decode(errors="replace")[-OUTPUT_LIMIT:]
        current, started, swept = submission(writes), time.time(), []
        for key, item in ledger.items.items():
            if not item["command"] or (item["digest"] == current and item["status"] in ("holds", "fails")):
                continue
            if time.time() - started > SWEEP_SECONDS:
                break
            before = ledger.status(key, current)
            code, output = run_command(item["command"], 60)
            swept.append({"criterion": key, "before": before, "after": ledger.record(key, item["command"], code, output, current)})
        if submission(snapshot_of(writes)) != current:
            raise RuntimeError("A probe changed the submission during the final sweep")
        record(event="probe_sweep", cycle=cycle, results=swept)
    obs={"files":writes,"check_exit":checked,"solver_calls":calls,
         "first_write_call":first_write,"first_check_call":first_check,"stop":stop,"notes":len(notes)}
    if cycle:
        (WORK/"verify-activity.json").write_text(json.dumps(activity.state(CHECKS[TASK], checked, stall.count)))
        obs.update(repeat_reads=activity.repeats, cycle=cycle, check_command=CHECKS[TASK], check_output=check_output,
                   regression=regression, local_baseline=not baseline and checked == 0 and bool(writes),
                   refused_reads=activity.refused)
    if ledger:
        current = submission(writes)
        obs.update(criteria=ledger.json(), targets=list(targets), submission_digest=current,
                   ledger=ledger.counts(current), swept=swept)
    if checkpoint:
        obs["verified_checkpoint_restores"] = recoveries
    (WORK/"observation.json").write_text(json.dumps(obs))
    return obs
