#!/usr/bin/env python3
"""GIL Interview and Experiment adapter for isolated coding tasks."""

import json
import os
import re
import sys
from pathlib import Path
from mailbox_client import MAIL, record
from solver import APP, CHECKS, TASK, WORK, ask, solve, source


def cited_sources(task):
    """Read bounded excerpts of public files explicitly named in the brief."""
    excerpts = {}
    remaining = 24000
    for match in re.finditer(r"`([^`\n]+)`|(?<![\w/])(/app/[\w./-]+)", task):
        name = (match[1] or match[2]).rstrip(".,;:")
        try:
            path = source(name)
            key = str(path.relative_to(APP))
            if key in excerpts or not path.is_file() or not remaining:
                continue
            size = min(9000, remaining)
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
            if len(candidates) == 120:
                break
        if len(candidates) == 120:
            break

    # Page through a selected file; a completed file need not be offered again.
    offsets = {r["path"]: r["offset"] + 6000 if r["more"] else None for r in reads}
    # Keep discovery within the existing time and context window for one GIL step.
    for _ in range(3):
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
                raw = handle.read(6001)
            more = len(raw) > 6000
            snippet = raw[:6000].decode("utf-8", errors="replace")
            if b"\x00" in raw:
                snippet, more, readable = "[binary file]", False, False
        except (OSError, ValueError, RuntimeError) as error:
            snippet, more, readable = str(error), False, False
        page = {"path": name, "offset": start, "result": snippet, "more": more, "readable": readable}
        record(event="tool", id=ask.call_id, action="read", interview_read=True, **page)
        reads.append(page)
        offsets[name] = start + 6000 if more else None


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
    for attempt in range(2):
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
        if outcome != "rewrite":
            break
        if attempt == 0:
            spec = model(instructions + "\nRepair this draft using the review. Draft: " + spec
                         + "\nReview: " + review)
    # Preserve the contract: a still-defective draft has no approval.
    return spec + "\nHandoff review (" + outcome + "): " + review, (
        "accept" if outcome == "start" else "revise")


def close(**fields):
    def field(k, v):
        return k + ": |\n" + "".join("  " + line + "\n" for line in str(v).splitlines())

    return {
        "action": "close",
        "report": "".join(field(k, v) for k, v in fields.items()),
    }


def reply(r):
    n = r["next"]
    task = r["task"]
    steps = r["history"][-1]["steps"]
    interview = r["history"][0]["steps"]

    def step(kind, history=steps):
        return next(s for s in reversed(history) if s["kind"] == kind and s["report"])

    def model(prompt, choices=()):
        context = {"task": task, "available_tools": {
            "read": "Read public task source", "write": "Replace permitted solution files",
            "check": CHECKS[TASK], "finish": "Hand submission to the independent verifier"}}
        if k in ("question", "interpretation", "synthesis"):
            context["cited_source_excerpts"] = cited
            context["interview_reads"] = reads
        return ask(json.dumps(context, ensure_ascii=False) + "\nCurrent instruction: " + prompt,
                   choices, stage=k)

    if n["action"] == "open":
        k = n["recommended"]
        return {
            "action": "open",
            "kind": k,
            "contract": f"objective: Complete {k} for the task\nnext_action: Carry out {k} using collected evidence\ndone_when: Record the {k} report",
        }
    k = n["kind"]
    if k in ("question", "interpretation", "synthesis"):
        cited = cited_sources(task)
        reads = interview_reads()
        prior_read_count = len(reads)
        if k in ("question", "synthesis"):
            focus = [s["report"] for s in steps if s["report"]][-2:]
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
                "the decision. Prior findings: " + json.dumps(previous[-2:]), list(anchors))
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
                  + json.dumps(previous[-2:]))
        for attempt in range(2):
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
            prompt = (instruction + "\nPropose a different unresolved question with two plausible answers "
                      "and a specific different action for each. Avoid these screened topics: "
                      + json.dumps(findings) + "\nPrevious interview findings: " + json.dumps(previous[-2:]))
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
        return close(
            statement=spec + "\nEvidence accepted for changes of status: " + json.dumps(promotions),
            basis_refs=q["ref"] + "\n" + i["ref"],
            approved="benchmark_" + decision,
        )
    spec = step("synthesis", interview)["report"]["statement"]
    if k == "define":
        return close(
            problem=spec,
            success_condition=model(
                "Give an observable success condition for: " + spec
            ),
        )
    criteria = step("define")["report"]["success_condition"] if n["cycle"] == "experiment" else ""
    if k == "hypothesis":
        return close(
            hypothesis=model(
                "Suggest an experimentally testable implementation strategy for: "
                + spec + "\nSuccess criteria: " + criteria
            ),
            rationale=spec,
            guardrail="Use the public task files and local tools.",
        )
    if k == "verify":
        hypothesis = step("hypothesis")["report"]["hypothesis"]
        obs = solve(
            task, "Accepted specification: " + spec + "\nSuccess criteria: " + criteria + "\nHypothesis: " + hypothesis
        )
        return close(
            execution="Recorded file receipts and the local check status in observation.json.",
            result=json.dumps(obs),
            evidence_files="observation.json",
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
        return close(
            verdict="success"
            if analysis["success_condition_met"] == "yes"
            else "failure",
            lesson=analysis["interpretation"],
            **fields,
        )
    outcome = step("outcome")
    verdict = outcome["report"]["verdict"]
    direction = {
        "next_direction.action": "open_child" if verdict == "success" else "revisit",
        "next_direction.reason": "Preserve the measured outcome.",
    }
    if verdict == "failure":
        direction["next_direction.target_cycle_ref"] = "cycle:C1"
    return close(
        verdict=verdict,
        outcome_ref=outcome["ref"],
        handoff_summary=outcome["report"]["lesson"],
        **direction,
    )


if __name__ == "__main__":
    print(json.dumps(reply(json.load(sys.stdin))))
