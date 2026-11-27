#!/usr/bin/env python3
"""GIL Interview and Experiment adapter for isolated coding tasks."""

import json
import os
import re
import sys
import time
from pathlib import Path
from criteria import CRITERIA, HYPOTHESIS, NEXT, PROBES, TARGETS, Ledger, digest, loads, parse
from mailbox_client import MAIL, record
from solver import APP, CHECKS, PAGE, TASK, VIEW_BUDGET, WORK, ask, solve, source

# No step of the Chain is counted. How long to look, how many questions to screen and how often to
# rewrite a draft are the model's judgement; what a request can carry is the harness's page and
# view size, shared with the editor. The one outside limit is the time the Core gives a step.
STARTED = time.monotonic()


def step_time_allows():
    """Whether this step has used less of its window than it has left. Without a window, always."""
    window = os.environ.get("GIL_BENCHMARK_STEP_SECONDS")
    spent = time.monotonic() - STARTED
    return not window or spent < float(window) - spent


def cited_sources(task):
    """Read the public files explicitly named in the brief, as far as one request can carry them."""
    excerpts = {}
    remaining = VIEW_BUDGET
    for match in re.finditer(r"`([^`\n]+)`|(?<![\w/])(/app/[\w./-]+)", task):
        name = (match[1] or match[2]).rstrip(".,;:")
        try:
            path = source(name)
            key = str(path.relative_to(APP))
            if key in excerpts or not path.is_file() or not remaining:
                continue
            size = min(PAGE, remaining)
            with path.open("rb") as handle:
                raw = handle.read(size + 1)
            if b"\x00" in raw:
                continue
            snippet = raw[:size].decode("utf-8", errors="replace")
            excerpts[key] = {"text": snippet, "truncated": len(raw) > size}
            remaining -= len(raw[:size])
        except (ValueError, OSError, RuntimeError):
            continue
    return excerpts


def interview_reads():
    log = MAIL / "transcript.jsonl"
    reads = []
    if log.exists():
        with log.open() as events:
            for line in events:
                event = json.loads(line)
                if event.get("interview_read"):
                    page = {key: event[key] for key in ("path", "offset", "result", "more")}
                    page["readable"] = event.get("readable", True)
                    reads.append(page)
    return reads


def explore(task, stage, focus, cited, reads):
    candidates = []
    for directory, children, files in os.walk(APP):
        children[:] = sorted(v for v in children if not v.startswith(".") and
                             v not in ("node_modules", "__pycache__", "build", "dist"))
        for name in sorted(files):
            if name.startswith("."):
                continue
            path = Path(directory, name)
            if not path.is_symlink() and path.is_file():
                candidates.append(str(path.relative_to(APP)))

    # Page through a selected file; a completed file need not be offered again.
    offsets = {r["path"]: r["offset"] + PAGE if r["more"] else None for r in reads}
    # The model reads until it judges the evidence sufficient; only the step's own time ends it sooner.
    while step_time_allows():
        available = [name for name in candidates if offsets.get(name, 0) is not None]
        if not available:
            return
        options = ["read " + name for name in available]
        stop = "continue interview"
        can_stop = stage != "question" or any(page["readable"] for page in reads)
        selection = ask(json.dumps({
            "task": task, "stage": stage, "focus": focus, "provided_excerpts": list(cited),
            "observed_pages": reads, "available_files": available,
            "instruction": "Inspect public files that can change the question or roadmap. "
                           "Choose a file to read its next page, or continue when evidence is sufficient. "
                           "A filename alone does not establish its contents.",
        }, ensure_ascii=False), options + ([stop] if can_stop else []), stage=stage + "_read")
        if can_stop and selection == stop:
            return
        name = selection.removeprefix("read ")
        if selection not in options:
            raise ValueError("Invalid interview file selection")
        start = offsets.get(name, 0)
        readable = True
        try:
            with source(name).open("rb") as handle:
                handle.seek(start)
                raw = handle.read(PAGE + 1)
            more = len(raw) > PAGE
            snippet = raw[:PAGE].decode("utf-8", errors="replace")
            if b"\x00" in raw:
                snippet, more, readable = "[binary file]", False, False
        except (OSError, ValueError, RuntimeError) as error:
            snippet, more, readable = str(error), False, False
        page = {"path": name, "offset": start, "result": snippet, "more": more, "readable": readable}
        record(event="tool", id=ask.call_id, action="read", interview_read=True, **page)
        reads.append(page)
        offsets[name] = start + PAGE if more else None


def reviewed_promotions(model, findings, new_pages, prior_texts):
    """Accept only new, attributable text that directly resolves the stated uncertainty."""
    if not new_pages:
        return []
    proposal = model(
        "Compare the unresolved interview decisions with these newly inspected pages. "
        "Return a JSON array only. For each decision directly settled by new evidence return "
        '{"claim":"precise resolved decision","path":"exact path","quote":"exact decisive substring"}. '
        "Use [] when the text only describes related fields or the evidence was already available. "
        "Implementation preference, model agreement and proposed tests are not evidence. Interview: "
        + findings + "\nNew pages: " + json.dumps(new_pages))
    try:
        items = json.loads(proposal)
    except ValueError:
        return []
    approved = []
    for item in items if isinstance(items, list) else []:
        if not isinstance(item, dict):
            continue
        claim, path, quote = (item.get(k) for k in ("claim", "path", "quote"))
        if not all(isinstance(v, str) and v.strip() for v in (claim, path, quote)):
            continue
        page = next((p for p in new_pages if p["path"] == path and quote in p["result"]), None)
        if page is None or any(quote in text for text in prior_texts):
            continue
        verdict = model("Does the quoted passage in its context establish the entire claim and "
                        "distinguish it from the competing interpretation? Mere compatibility is insufficient. "
                        + json.dumps({"claim": claim, "quote": quote, "context": page["result"]}),
                        ["supported", "insufficient"])
        if verdict == "supported":
            approved.append({"claim": claim, "path": path, "offset": page["offset"], "quote": quote})
    return approved


def experiment_handoff(model, findings, evidence_rule):
    instructions = (
        "Write a prospective experiment plan. Implementation and execution happen later in Verify. "
        "State the deliverable, binding constraints, reversible working assumption, first available "
        "action, what observation could disprove the assumption, and what is deferred to grading. "
        "A spec can authorize a trial before the hypothesis is verified. Do not claim it succeeded. "
        + evidence_rule + " Interview: " + findings)
    spec = model(instructions)
    while True:
        review = model(
            "Review readiness to START this isolated experiment. Give a short concrete decision basis. "
            "If work can start, identify the reversible choice, first action and how to evaluate it. "
            "Hidden grading details and unresolved method choices can remain provisional. If work "
            "cannot start, name the conflicting requirements or missing input and one specific "
            "interview question that could resolve it. A draft that asks to implement now, asserts "
            "unsupported facts or contains no plan needs rewriting here. "
            + evidence_rule + " Interview: " + findings + "\nDraft: " + spec)
        outcome = model(
            "Classify readiness. start = feasible first action consistent with known constraints, "
            "with unresolved choices explicitly provisional. clarify = missing input or conflicting "
            "requirement prevents any valid reversible first action AND another interview question "
            "can resolve it. rewrite = a defective draft that can be fixed with existing information. "
            "Do not require completed code or hidden verifier results to authorize a first trial. "
            + evidence_rule + " Interview: " + findings + "\nDraft: " + spec + "\nReview: " + review,
            ["start", "clarify", "rewrite"])
        # A defective draft is rewritten for as long as the review asks for it and the step has time.
        if outcome != "rewrite" or not step_time_allows():
            break
        spec = model(instructions + "\nRepair this draft using the review. Draft: " + spec
                     + "\nReview: " + review)
    # Preserve the contract: a still-defective draft has no approval.
    return spec + "\nHandoff review (" + outcome + "): " + review, (
        "accept" if outcome == "start" else "revise")


# Nothing in the Chain is counted or thresholded. Where to go back to after a failure is judged in
# the reflection that closes the attempt, and whether there is time for another experiment is the
# model's call too: it is told the minutes left. The Chain ends on its own only when a Verify
# reports that the trial's time or model budget ran out.
SPENT = ("budget", "deadline")
# What each step does, and nothing else.
ACTS = {
    "question": "Ask the one open question the public files raise.",
    "interpretation": "Say what the exchange settled and what stays uncertain.",
    "synthesis": "Write the plan the experiments start from.",
    "define": "Write which problem this experiment solves and the exact state that counts as solved.",
    "hypothesis": "Write the method that would bring that state about.",
    "analysis": "Say whether, with the hypothesis carried out, the problem was solved.",
    "outcome": "Reflect: if it was not solved, why and where to go back; if it was, the candidates for the next experiment.",
}
# The steps whose requests carry the public sources read so far.
SOURCED = ("question", "interpretation", "synthesis", "define", "hypothesis")


def verify_result(cycle):
    """The observation a closed Verify left in its report, or None."""
    step = next((s for s in reversed(cycle.get("steps") or []) if s["kind"] == "verify" and s["report"]), None)
    try:
        return json.loads(step["report"]["result"]) if step else None
    except (ValueError, KeyError):
        return None


def ledger_plan(cycle):
    """What a ledger Experiment committed to in its Define: targets, what held at entry, the items."""
    step = next((s for s in reversed(cycle.get("steps") or []) if s["kind"] == "define" and s["report"]), None)
    try:
        plan = json.loads(step["report"]["success_condition"].split("\nLedger: ", 1)[1]) if step else None
    except (ValueError, KeyError, IndexError):
        return None
    return plan if isinstance(plan, dict) and plan.get("criteria") else None


def summary(result):
    """What a Verify observed, without the ledger's full text."""
    return {key: result.get(key) for key in ("stop", "check_exit", "regression", "targets", "entry", "ledger", "swept",
                                             "outside_hypothesis") if key in result}


def merged(items, lost):
    """Items as they stand in the world the run is in, with what an abandoned attempt left:
    the submissions its problems failed on, and what it defined."""
    if items is None:
        return lost
    for item in items:
        other = next((o for o in lost or [] if o["id"] == item["id"]), None)
        if not other:
            continue
        item["failed_on"] = sorted(set(item["failed_on"]) | set(other["failed_on"]))
        if not item["probe"] and other["probe"]:
            # A definition made in the abandoned world still says what solved would mean; its result is stale here.
            item.update({key: other[key] for key in ("state", "probe", "exit", "output", "digest", "status")})
    return items


def lineage(cycles, node):
    """A Cycle and the Cycles above it, nearest first. A record whose parents do not lead upward ends there."""
    seen = []
    while node and not any(node is other for other in seen):
        seen.append(node)
        yield node
        node = cycles.get(node.get("parent"))


def exit_result(cycles, node):
    """What the files were when a Cycle closed: its own last Verify, or, for a Cycle that changed
    nothing (an Interview), the nearest one before it."""
    for above in lineage(cycles, node):
        result = verify_result(above)
        if result:
            return result
    return None


def latest_interview(cycles, node, root):
    """The Interview an Experiment works under: the nearest one above it."""
    for above in lineage(cycles, node):
        if above.get("kind") == "interview":
            return above
    return root


def interview_list(cycle):
    """The numbered list an Interview wrote in its Synthesis; None when it wrote none."""
    step = next((s for s in reversed(cycle.get("steps") or []) if s["kind"] == "synthesis" and s["report"]), None)
    try:
        items = json.loads(step["report"]["statement"].split("\nCriteria: ", 1)[1].splitlines()[0]) if step else None
    except (ValueError, KeyError, IndexError):
        return None
    return items if isinstance(items, list) and items else None


def crossed_off(cycle):
    """The numbers the Analysis of an Experiment crossed off."""
    step = next((s for s in reversed(cycle.get("steps") or []) if s["kind"] == "analysis" and s["report"]), None)
    try:
        found = json.loads(step["report"]["interpretation"].split("\nCrossed off: ", 1)[1].splitlines()[0]) if step else []
    except (ValueError, KeyError, IndexError):
        return []
    return found if isinstance(found, list) else []


def entry_ledger(cycles, parent, failed):
    """The list an Experiment starts from. Directly under an Interview it is the list that Interview
    wrote. Otherwise it is the parent's list as it stood when the parent closed, with what the
    parent's Analysis crossed off and what an Experiment abandoned from here left."""
    for node in lineage(cycles, parent):
        if node.get("kind") == "interview":
            return interview_list(node)
        result = verify_result(node)
        if result and result.get("criteria"):
            ledger = Ledger(result["criteria"])
            ledger.cross_off([key for key in crossed_off(node) if key in ledger.items], node["ref"])
            return merged(ledger.json(), (verify_result(failed) or {}).get("criteria") if failed else None)
    return None


def chain_record(history):
    """What the Experiments so far set out to solve and how each ended, for an Interview that writes the list again."""
    record = []
    for cycle in history:
        plan = ledger_plan(cycle) if cycle.get("kind") == "experiment" else None
        if not plan:
            continue
        outcome = next((s["report"] for s in reversed(cycle["steps"]) if s["kind"] == "outcome" and s["report"]), {})
        record.append({"experiment": cycle["ref"], "problems": plan["targets"], "verdict": (cycle.get("report") or {}).get("verdict"),
                       "crossed_off": crossed_off(cycle), "reflection": outcome.get("lesson")})
    return record


def noted(cycle, label):
    """What the reflection that closed an Experiment wrote down under a label, as JSON; {} when nothing."""
    step = next((s for s in reversed((cycle or {}).get("steps") or []) if s["kind"] == "outcome" and s["report"]), None)
    try:
        found = json.loads(step["report"]["lesson"].split("\n" + label + ": ", 1)[1].splitlines()[0]) if step else {}
    except (ValueError, KeyError, IndexError):
        return {}
    return found if isinstance(found, dict) else {}


def scope_of(hypothesis):
    """The files a hypothesis said its method changes; None when it did not say."""
    try:
        names = json.loads(hypothesis.split("\nFiles: ", 1)[1].splitlines()[0])
    except (ValueError, IndexError):
        return None
    found = set()
    for name in names if isinstance(names, list) else []:
        try:
            found.add(source(str(name), True).relative_to(APP).as_posix())
        except (ValueError, OSError):
            continue    # a file no submission may contain is not something a method can change
    return found


def choose(model, ledger, entry, lesson, pool):
    """Which of the candidate criteria this Experiment takes as its problem: the model's judgement."""
    answer = model(
        "Choose the problem of this experiment: the candidate criteria one coherent method can bring about. "
        "Every criterion you do not name stays open for a later experiment, so name only what this one will "
        f"solve. About {minutes_left():.0f} minutes of the trial are left. Candidates: {pool}. "
        'Answer as {"targets": ["K1", "K4"]}. Ledger: '
        + json.dumps(ledger.shown(entry), ensure_ascii=False) + ("\n" + lesson if lesson else ""), schema=TARGETS)
    try:
        targets = [key for key in loads(answer).get("targets") or [] if key in pool]
    except (ValueError, AttributeError, TypeError):
        targets = []
    # An answer that names nothing usable leaves every candidate to this experiment.
    return list(dict.fromkeys(targets)) or list(pool)


def write_list(model, spec, previous, record):
    """The Interview numbers the problems to be solved. When a list exists already, the Interview
    writes it again and says which problems it keeps."""
    request = (
        "Write the numbered list of problems this task poses: what an independent grader could check, using "
        'only the task text and the public files read in this interview, as {"criteria": [...]}. Each has a '
        "requirement (one checkable statement naming an observable result), a kind (failure, invariant or "
        "behaviour), a source (file: short quote) and same_as. List first every failure case the sources "
        "describe (what must happen when a write, an input or a dependency fails) as kind failure, then "
        "everything that must stay unchanged as kind invariant, then ordinary behaviour. Roadmap: " + spec)
    if previous:
        request += (
            "\nA list exists already, and the experiments below worked from it. Write the list again in the light "
            "of what they found. For a problem you keep as it is, set same_as to its number in the existing list: "
            "what was defined and found about it is kept, including that it was solved. Set same_as to an empty "
            "string for a problem that is new or that you state differently; it starts unsolved. Leave out what "
            "the task does not require. Existing list: " + json.dumps(Ledger(previous).shown(None), ensure_ascii=False)
            + "\nExperiments: " + json.dumps(record, ensure_ascii=False))
    else:
        request += '\nThis is the first list: set same_as to an empty string everywhere.'
    try:
        return parse(model(request, schema=CRITERIA), previous or ())
    except ValueError:
        return None


def define_states(model, ledger, targets, lesson):
    """Define says exactly what solved means: for each target, the state that must hold and the probe
    that shows it. Targets the answer gives no probe for are not part of the problem."""
    answer = model(
        "This is the problem definition. For each target criterion write (1) state: the exact state that must "
        "hold for it to count as solved, including the starting state it is observed from; (2) probe: a shell "
        "script, run with bash in the task directory, that exits 0 exactly when that state holds and non-zero "
        "otherwise. The probe is fixed here, before any method is chosen, and it judges whatever "
        "implementation follows. So it rests only on the public interface the task names (its commands, files "
        "and formats) and never on how a solution is built. It prepares its own starting state the way an "
        "outside party would: it writes the input and any pre-existing files directly, in the form the sources "
        "give, and does not call the solution to produce them. Where the requirement is that something must not "
        "change, the starting state contains that something (existing records, a stale file) and the probe "
        "compares bytes. Where it is about a failure, the probe creates the failing condition. It restores "
        "what it touched in the task directory, and prints what differed when it fails. It is run exactly as "
        "written and nobody can repair it afterwards, so whatever helper it writes must run with the tooling and "
        "module system the project's own commands already use. "
        'Answer as {"probes": [{"id": "K1", "state": "...", "probe": "..."}]}. Targets: '
        + json.dumps([{key: ledger.items[t][key] for key in ("id", "kind", "requirement", "source")} for t in targets],
                     ensure_ascii=False)
        + ("\n" + lesson if lesson else ""), schema=PROBES)
    try:
        written = loads(answer).get("probes") or []
    except (ValueError, AttributeError, TypeError):
        written = []
    defined = []
    for entry in written:
        if (isinstance(entry, dict) and entry.get("id") in targets and entry["id"] not in defined
                and str(entry.get("probe") or "").strip() and str(entry.get("state") or "").strip()):
            ledger.define(entry["id"], entry["state"], entry["probe"])
            defined.append(entry["id"])
    return defined


def minutes_left():
    return (float((WORK.parent / "deadline").read_text()) - time.time()) / 60


def close(**fields):
    def field(k, v):
        return k + ": |\n" + "".join("  " + line + "\n" for line in str(v).splitlines())

    return {
        "action": "close",
        "report": "".join(field(k, v) for k, v in fields.items()),
    }


def reply(r):
    n = r["next"]
    if n["action"] == "project":
        from checkpoint import project_submission
        return project_submission(r, WORK, source, record)
    task = r["task"]
    current = r["history"][-1]
    cycles = {c.get("ref"): c for c in r["history"]}
    # A record that names no parent has none: a Cycle is not its own parent.
    parent = (cycles.get(current.get("parent")) if current.get("parent") else None) or {}
    active = bool(os.environ.get("GIL_TB41_CYCLE_RECOVERY")) and n.get("cycle") == "experiment"
    # The first Experiment of the Chain stands directly under the root Interview: nothing is submitted yet.
    first = active and parent.get("kind") == "interview" and not parent.get("parent")
    failed = cycles.get(current["revisit_from"]) if current.get("revisit_from") else None
    lesson = ""
    if failed:
        evidence = {s["ref"]: s["report"][field] for s in failed["steps"]
                    for kind, field in (("hypothesis", "hypothesis"), ("verify", "result"),
                                        ("analysis", "interpretation"), ("outcome", "lesson")) if s["kind"] == kind and s["report"]}
        for ref, text in evidence.items():
            if ":" in ref and text.startswith("{"):
                with_ledger = verify_result({"steps": [{"kind": "verify", "report": {"result": text}}]})
                if with_ledger and "criteria" in with_ledger:
                    evidence[ref] = json.dumps(summary(with_ledger))
        lesson = ("Closed failed experiment " + failed["ref"] + " Journey " + failed["journey"] + "; restored " + parent["ref"]
                  + " at " + parent["exit_world"] + ". Use the failed prediction and observed check "
                  "to propose a different hypothesis and change the implementation. Evidence: "
                  + json.dumps(evidence, ensure_ascii=False))
    steps = r["history"][-1]["steps"]
    interview = latest_interview(cycles, current, r["history"][0])["steps"]
    # Hypotheses already tried under this Define. Each was abandoned by going back to the Define:
    # the world returned to what it was there, and what the attempt found stays in the record.
    abandoned = [s for s in steps if s["kind"] == "outcome" and s["report"] and s["report"].get("verdict") == "failure"]
    if abandoned:
        tried = {s["ref"]: s["report"][field] for s in steps
                 for kind, field in (("hypothesis", "hypothesis"), ("analysis", "interpretation"), ("outcome", "lesson"))
                 if s["kind"] == kind and s["report"]}
        lesson += ("\nUnder this problem definition these hypotheses were tried and abandoned; the files are back as "
                   "they were at the Define. Propose a different change. Record: " + json.dumps(tried, ensure_ascii=False))

    def step(kind, history=steps):
        return next(s for s in reversed(history) if s["kind"] == kind and s["report"])

    def model(prompt, choices=(), schema=None):
        # Each step does one thing, and only Verify has tools. A step that writes is told so: with the
        # tools listed here it announced inspections and wrote tool calls into its text.
        context = {"task": task, "step": k, "act_of_this_step": ACTS.get(k, "Write this step's text."),
                   "tools_in_this_step": "none: what you write is recorded as this step's text; reading, changing and "
                                         "running happen only in Verify",
                   "local_check_run_in_verify": CHECKS[TASK]}
        if k in SOURCED:
            context["cited_source_excerpts"] = cited
            context["interview_reads"] = reads
        if n.get("cycle") == "interview" and reopened:
            # An Interview opened in the middle of a Chain: why, and what the experiments found.
            context["why_this_interview_was_opened"] = reopened
            context["experiments_so_far"] = chain_record(r["history"])
        shaped = {"schema": schema} if schema else {}
        return ask(json.dumps(context, ensure_ascii=False) + "\nCurrent instruction: " + prompt,
                   choices, stage=k, **shaped)

    # Why a later Interview was opened: the reflection that asked for it wrote the reason down.
    asked_by = failed or parent
    reopened = (noted(asked_by, "Next").get("new_interview") or noted(asked_by, "Way back").get("reason")
                if n.get("cycle") == "interview" and asked_by else None)
    if n["action"] == "open":
        k = n["recommended"]
        return {
            "action": "open",
            "kind": k,
            "contract": f"objective: Complete {k} for the task\nnext_action: Carry out {k} using collected evidence\ndone_when: Record the {k} report",
        }
    k = n["kind"]
    if k in SOURCED:
        cited = cited_sources(task)
        reads = interview_reads()
        prior_read_count = len(reads)
        # Looking at the public files is the interview's act. A problem definition and a hypothesis
        # only write, from what the record already holds.
        if k in ("question", "synthesis"):
            focus = [s["report"] for s in steps if s["report"]]
            explore(task, k, focus, cited, reads)
    if k == "question":
        previous = [s["report"] for s in steps if s["kind"] in ("interpretation", "synthesis") and s["report"]]
        anchors = {f"{page['path']} (byte {page['offset']})": page["result"]
                   for page in reads if page["readable"]}
        if not anchors:
            anchors = {f"{path} (byte 0)": excerpt["text"] for path, excerpt in cited.items()}
        if anchors:
            anchor = next(iter(anchors)) if len(anchors) == 1 else model(
                "Select the observed page with a concrete fact, conflict or gap that most changes "
                "the decision. Prior findings: " + json.dumps(previous), list(anchors))
            basis = "Observed file: " + anchor
            instruction = ("State a specific observation from this page, then ask the question it raises. "
                           "Distinguish current behavior from the requested behavior. Page: " + anchors[anchor])
        else:
            basis = "Task brief (no readable source observed)"
            instruction = "Start from an explicit fact in the brief. Identify what cannot yet be observed."
        findings = []
        prompt = (instruction + "\nPropose one unresolved question and two plausible answers with "
                  "different concrete implementation or verification consequences. Do not ask whether "
                  "to follow an explicit requirement. On revision target a remaining gap. Prior findings: "
                  + json.dumps(previous))
        while True:
            question = model(prompt)
            disposition = model(
                "Check the proposed question against the brief and all supplied source contents. "
                "Choose answered if evidence settles the decision, even if ignoring it would change code. "
                "Choose immaterial if the alternatives require the same action or have unsupported premises. "
                "Choose open only for a grounded unresolved decision whose alternatives change a specific "
                "implementation or verification action. Candidate: " + question,
                ["answered", "immaterial", "open"],
            )
            if disposition == "open":
                response = model("For this open decision, cite what is known, identify missing evidence "
                                 "and the next discriminating observation. Label any provisional choice "
                                 "as an assumption. Question: " + question)
                return close(question=basis + "\n" + question,
                             choices="Compare the candidate alternatives against evidence.",
                             response="\n".join(findings + [response]))
            finding = model("Explain the classification " + disposition + " in one sentence. "
                            "For a settled question cite the source and its answer; for an immaterial "
                            "question identify the unchanged decision or unsupported premise. Candidate: "
                            + question)
            findings.append(disposition + ": " + finding)
            # Whether another question is worth asking is judged from what was screened so far.
            if not step_time_allows() or model(
                    "The candidate was screened out. Judge from the brief, the source contents and the screened "
                    "topics whether another decision remains that is open and would change a specific "
                    "implementation or verification action. Screened: " + json.dumps(findings),
                    ["another question", "no open question remains"]) != "another question":
                break
            prompt = (instruction + "\nPropose a different unresolved question with two plausible answers "
                      "and a specific different action for each. Avoid these screened topics: "
                      + json.dumps(findings) + "\nPrevious interview findings: " + json.dumps(previous))
        return close(question=basis + "\nNo decision-changing open question identified in this review.",
                     choices="Carry the evidence review into synthesis.",
                     response="\n".join(findings))
    if k == "interpretation":
        exchange = json.dumps(step("question")["report"])
        interpretation = model("Summarize confirmed requirements and the decisions supported by this exchange: " + exchange)
        gaps = model("Identify remaining uncertainties and how each can be resolved, including evidence "
                     "the available check cannot provide. Say none only if justified. Exchange: "
                     + exchange + "\nInterpretation: " + interpretation)
        return close(interpretation=interpretation, unresolved=gaps)
    if k == "synthesis":
        q, i = step("question"), step("interpretation")
        findings = json.dumps([s["report"] for s in steps if s["kind"] in ("question", "interpretation") and s["report"]])
        new_pages = [p for p in reads[prior_read_count:] if p["readable"]]
        prior_texts = [p["result"] for p in reads[:prior_read_count]] + [p["text"] for p in cited.values()] + [task]
        promotions = reviewed_promotions(model, findings, new_pages, prior_texts)
        evidence_rule = ("Every prior open decision or provisional choice stays provisional except "
                         "these individually verified promotions: " + json.dumps(promotions)
                         + ". A working choice may be used with a named future check. ")
        spec, decision = experiment_handoff(model, findings, evidence_rule)
        # The Interview numbers the problems. A later Interview writes the list again from the one
        # the Chain was working from.
        before = (exit_result(cycles, asked_by) or {}).get("criteria") if asked_by else None
        if before and not failed:
            crossed = Ledger(before)
            crossed.cross_off([key for key in crossed_off(asked_by) if key in crossed.items], asked_by["ref"])
            before = crossed.json()
        listed = write_list(model, spec, before, chain_record(r["history"]))
        return close(
            statement=spec + "\nEvidence accepted for changes of status: " + json.dumps(promotions)
            + ("\nCriteria: " + json.dumps(listed, ensure_ascii=False) if listed else ""),
            basis_refs=q["ref"] + "\n" + i["ref"],
            approved="benchmark_" + decision,
        )
    spec = step("synthesis", interview)["report"]["statement"].split("\nCriteria: ")[0]
    if first:
        spec += "\nFirst milestone: produce a nonempty candidate that passes the available local check. Full task criteria remain for the next experiment and independent grading."
    elif active:
        spec += "\nContinue from the locally checked candidate. Inspect its files and address the remaining public requirements."
    if lesson:
        spec += "\n" + lesson
    if k == "define" and active and not first:
        items = entry_ledger(cycles, parent, failed)
        entry = digest((exit_result(cycles, parent) or {}).get("files") or {})
        ledger = Ledger(items) if items else None
        if ledger:
            # The reflection of a solved parent named the candidates for this Experiment.
            named = noted(parent, "Next").get("candidates") or []
            pool = [key for key in named if key in ledger.open(entry)] or ledger.open(entry)
            targets = define_states(model, ledger, choose(model, ledger, entry, lesson, pool), lesson) if pool else []
            if targets:
                plan = {"targets": targets, "held_at_entry": ledger.having(entry, "solved"), "criteria": ledger.json()}
                return close(
                    problem=spec + "\nThis experiment solves: "
                    + json.dumps([{key: ledger.items[t][key] for key in ("id", "requirement", "state")} for t in targets],
                                 ensure_ascii=False),
                    success_condition="Solved when, on the final files, every target's probe exits 0, the local check "
                    "passes, and no problem already crossed off fails. The probes are fixed in this definition."
                    "\nLedger: " + json.dumps(plan, ensure_ascii=False),
                )
    if k == "define":
        return close(
            problem=spec,
            success_condition="Edited submission passes " + json.dumps(CHECKS[TASK]) if first else model(
                "Give an observable success condition for: " + spec
            ),
        )
    plan = ledger_plan(current) if active and not first else None
    criteria = step("define")["report"]["success_condition"] if n["cycle"] == "experiment" else ""
    if k == "hypothesis" and plan:
        chosen = [i for i in plan["criteria"] if i["id"] in plan["targets"]]
        try:
            said = loads(model(
                "State the method of this experiment: the change that will bring about the state defined for each "
                "target, what it changes in which files, and why that produces each state. The Verify that follows "
                "carries out exactly this method and nothing more, and may change only the files you name; the "
                "probes below then decide whether the problem is solved. This step writes no file. "
                'Answer as {"method": "...", "files": ["relative/path"]}. Targets: '
                + json.dumps([{key: i[key] for key in ("id", "kind", "requirement", "state", "probe")} for i in chosen],
                             ensure_ascii=False)
                + ("\n" + lesson if lesson else ""), schema=HYPOTHESIS))
            method, files = str(said["method"]), [str(name) for name in said["files"]]
        except (ValueError, KeyError, TypeError):
            method, files = "No method was stated.", []
        return close(
            hypothesis=method + "\nFiles: " + json.dumps(files),
            rationale=spec,
            guardrail="Verify does no more than this method and changes only these files. No criterion that held at entry may fail.",
        )
    if k == "hypothesis":
        return close(
            hypothesis=model(
                "Use the working assumption and first action in the accepted specification to make one testable prediction. Identify the observation that would contradict it and whether that observation is available locally or only during grading. Accepted specification: "
                + spec + "\nSuccess criteria: " + criteria
            ),
            rationale=spec,
            guardrail="Use the public task files and local tools.",
        )
    if k == "verify":
        hypothesis = step("hypothesis")["report"]["hypothesis"]
        options = {}
        if active:
            options["cycle"] = current["ref"]
            options["verify_ref"] = n["target"]
            if failed:
                old = next(s for s in reversed(failed["steps"]) if s["kind"] == "verify")
                result = json.loads(old["report"]["result"])
                options["failed_check"] = {"verify": old["ref"], "exit": result["check_exit"],
                                           "output": result["check_output"]}
            if not first:
                options["baseline"] = exit_result(cycles, parent)
            if plan:
                # The Define stands; an attempt abandoned under it leaves its probes and failures.
                earlier = [verify_result({"steps": [s]}) for s in steps if s["kind"] == "verify" and s["report"]]
                items = plan["criteria"]
                for result in earlier:
                    items = merged(items, (result or {}).get("criteria"))
                options.update(criteria=items, targets=plan["targets"], scope=scope_of(hypothesis))
        obs = solve(
            task, ("Problem definition: " + step("define")["report"]["problem"] + "\nHypothesis to carry out, exactly and no further: "
                   + hypothesis) if plan else
            "Accepted specification: " + spec + "\nSuccess criteria: " + criteria + "\nHypothesis: " + hypothesis, **options
        )
        return close(
            execution="Recorded file receipts and the local check status in observation.json.",
            result=json.dumps(obs),
            evidence_files="observation.json\nverify-activity.json" if active else "observation.json",
        )
    if k == "analysis":
        obs = (WORK / "observation.json").read_text()
        analysis = model(
            "Assess each accepted criterion against concrete observations. Identify the check by its "
            "actual command. Distinguish demonstrated, contradicted and unverified criteria. "
            "File hashes demonstrate edits only. The independent verifier has not run. Roadmap: "
            + spec + "\nCriteria: " + criteria + "\nObserved: " + obs
        )
        observed = json.loads(obs)
        if plan and observed.get("criteria"):
            # With the hypothesis carried out, is the problem solved? The ledger says; the model explains.
            ledger, now = Ledger(observed["criteria"]), observed["submission_digest"]
            unsolved = {t: ledger.status(t, now) for t in plan["targets"] if ledger.status(t, now) not in ("holds", "solved")}
            broken = ledger.having(now, "broken")
            outside = observed.get("outside_hypothesis") or []
            achieved = bool(observed["files"]) and observed["check_exit"] == 0 and not unsolved and not broken and not outside
            # Analysis crosses off what this Experiment solved; a problem half solved is not crossed off.
            crossed = [t for t in plan["targets"] if not ledger.items[t]["solved_in"]] if achieved else []
            verdict = {"solved": achieved, "crossed_off": crossed, "targets_holding": [t for t in plan["targets"] if t not in unsolved],
                       "targets_not_holding": unsolved, "targets_at_entry": observed.get("entry"),
                       "crossed_off_before_and_now_failing": broken, "changed_outside_the_hypothesis": outside,
                       "local_check_exit": observed["check_exit"], "ledger": ledger.counts(now), "open": ledger.open(now)}
            analysis = "Ledger verdict (computed from probe results on the final files): " + json.dumps(verdict) + "\n" + model(
                "The verdict below was computed from the probes of the problem definition, run on the final files "
                "after the hypothesis was carried out; it is not yours to re-judge. Say what was done as "
                "hypothesized, and for each target that does not hold what its probe's output shows. Do not "
                "propose what to do next: that is the reflection's. Verdict: " + json.dumps(verdict)
                + "\nHypothesis: " + step("hypothesis")["report"]["hypothesis"]
                + "\nLedger: " + json.dumps(ledger.shown(now, plan["targets"]), ensure_ascii=False))
            analysis += "\nCrossed off: " + json.dumps(crossed)
            return close(
                hypothesis_fit=analysis,
                problem_solved="yes" if achieved else "no",
                success_condition_met="yes" if achieved else "no",
                guardrail_triggered="yes" if broken or outside else "no",
                interpretation=analysis,
            )
        if first:
            achieved = observed["local_baseline"]
            analysis += "\nOnly the local check is assessed in this milestone. Full task behavior remains unverified."
        else:
            achieved = bool(observed["files"]) and observed["check_exit"] == 0 and model(
                "Choose demonstrated only if every material criterion has direct evidence. A missing, "
                "deferred or contradicted criterion requires incomplete. Criteria: " + criteria
                + "\nRoadmap: " + spec + "\nObserved: " + obs + "\nAssessment: " + analysis,
                ["demonstrated", "incomplete"],
            ) == "demonstrated"
        return close(
            hypothesis_fit=analysis,
            problem_solved="yes" if achieved else "partial",
            success_condition_met="yes" if achieved else "no",
            guardrail_triggered="no",
            interpretation=analysis,
        )
    if k == "outcome":
        fields = {
            "next_direction.action": "close_cycle",
            "next_direction.reason": "Record the completed trial.",
        }
        if n["cycle"] == "interview":
            return close(
                verdict="success",
                lesson=spec,
                synthesis_ref=step("synthesis")["ref"],
                **fields,
            )
        analysis = step("analysis")["report"]
        if plan and analysis["success_condition_met"] != "yes":
            # Reflection decides which layer was wrong. Either way the world goes back and the record stays.
            obs = json.loads(step("verify")["report"]["result"])
            reflection = model(
                "The problem was not solved. Reflect: why not, and where to go from here. There are three places, "
                "and they differ in what gets written again. (1) This experiment's Define: its problem definition "
                "(targets, states, probes) stands and only a new hypothesis is written. (2) The parent experiment: "
                "this experiment's problem definition is written again, with its targets, states and probes. "
                "(3) A new interview: the numbered list of problems itself is written again. The task is the same "
                "in all three. So: if the states and probes match their sources and a different method could "
                "bring the states about, the hypothesis was wrong. If a state or its probe does not match its "
                "source, the targets were badly chosen, or they depend on work not yet done, the problem "
                "definition was wrong. A probe that fails for a reason of its own belongs to the problem "
                "definition too: if it stops before it reaches what it is meant to observe, or fails the same "
                "way whatever the method was, no new hypothesis can mend it, because a probe is written only in a "
                "problem definition. If the list is what is wrong (it misreads the task, lacks something the "
                "task requires, or a problem crossed off earlier now fails and you can say what differs between "
                "then and now), the list has to be written again. "
                f"State which it is and what the evidence is. About {minutes_left():.0f} minutes "
                "of the trial are left. Analysis: "
                + analysis["interpretation"] + ("\n" + lesson if lesson else ""))
            way = None if obs.get("stop") in SPENT else model(
                "Choose where this experiment goes from here, as your reflection concluded. new_hypothesis: back to "
                "this experiment's Define; its targets, states and probes stand, the files return to what they "
                "were there, and only a different hypothesis is written. new_problem_definition: back to the "
                "parent experiment; its files are restored, and this experiment's targets, states and probes are "
                "written again for the same task. A defect in a probe is mended only this way. new_interview: "
                "back to the parent experiment, its files restored, and from there a new interview writes the "
                "numbered list of problems again. Reflection: " + reflection,
                ["new_hypothesis", "new_problem_definition", "new_interview"])
            if way == "new_hypothesis":
                return close(
                    verdict="failure",
                    lesson=reflection,
                    **{"next_direction.action": "revisit",
                       "next_direction.target_node_ref": step("define")["ref"],
                       "next_direction.reason": "The hypothesis is doubted; the problem definition stands."},
                )
            # The Cycle closes as failed. Where the run goes after going back is written down for it.
            return close(verdict="failure", lesson=reflection + "\nWay back: "
                         + json.dumps({"to": way, "reason": reflection if way == "new_interview" else ""}, ensure_ascii=False), **fields)
        lesson_learned = analysis["interpretation"]
        if plan:
            # Solved. The reflection judges what comes next: candidates for the next Experiment, a new
            # Interview when the list itself has to be written again, or nothing more.
            obs = json.loads(step("verify")["report"]["result"])
            ledger, now = Ledger(obs["criteria"]), obs["submission_digest"]
            ledger.cross_off([key for key in crossed_off(current) if key in ledger.items], current["ref"])
            try:
                said = loads(model(
                    "The problem of this experiment is solved and its numbers are crossed off. Decide what comes "
                    "next. Usually: candidates, the open problems worth taking in the next experiment. If instead "
                    "the list itself has to be written again (it misreads the task or lacks something the task "
                    "requires), give the reason in new_interview and a new interview will write it; otherwise "
                    "leave new_interview empty. Leave both empty when nothing worth doing remains or the time left "
                    f"does not allow another experiment. About {minutes_left():.0f} minutes of the trial are left. "
                    'Answer as {"candidates": ["K5"], "new_interview": ""}. '
                    "Ledger: " + json.dumps(ledger.shown(now), ensure_ascii=False), schema=NEXT))
                picked = {"candidates": [key for key in said.get("candidates") or [] if key in ledger.open(now)],
                          "new_interview": str(said.get("new_interview") or "").strip()}
            except (ValueError, AttributeError, TypeError):
                picked = {"candidates": [], "new_interview": ""}
            lesson_learned += "\nNext: " + json.dumps(picked, ensure_ascii=False)
        return close(
            verdict="success"
            if analysis["success_condition_met"] == "yes"
            else "failure",
            lesson=lesson_learned,
            **fields,
        )
    outcome = step("outcome")
    verdict = outcome["report"]["verdict"]
    direction = {
        "next_direction.action": "open_child" if verdict == "success" else "revisit",
        "next_direction.reason": "Preserve the measured outcome.",
    }
    if verdict == "failure":
        direction["next_direction.target_cycle_ref"] = current.get("parent") or "cycle:C1"
    if active:
        obs = json.loads(step("verify")["report"]["result"])
        room = obs.get("stop") not in SPENT
        if first and obs["local_baseline"]:
            direction["benchmark_transition"] = "verified_local_baseline"
        elif obs["regression"] and not plan:
            direction["benchmark_transition"] = "observed_regression"
        elif plan and obs.get("criteria") and room:
            said, back = noted(current, "Next"), noted(current, "Way back")
            if verdict == "success" and said.get("new_interview"):
                # The list has to be written again: a new Interview opens on the world as it is.
                direction["benchmark_transition"] = "interview_open"
            elif verdict == "success" and said.get("candidates"):
                # The reflection named candidates: the next Experiment continues from this world.
                direction["benchmark_transition"] = "criteria_open"
            elif verdict == "failure" and back.get("to") == "new_interview":
                # Back to the parent's world, and from there a new Interview.
                direction["benchmark_transition"] = "interview_after_failure"
            elif verdict == "failure" and back.get("to") == "new_problem_definition":
                # Back to the parent's world, and a new Experiment defines the problem again.
                direction["benchmark_transition"] = "criteria_failed"
    return close(
        verdict=verdict,
        outcome_ref=outcome["ref"],
        handoff_summary=outcome["report"]["lesson"],
        **direction,
    )


if __name__ == "__main__":
    print(json.dumps(reply(json.load(sys.stdin))))
