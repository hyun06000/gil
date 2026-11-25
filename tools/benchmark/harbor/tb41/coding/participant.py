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
        question = model(
            instruction + "\nAsk about a decision that changes implementation or verification. "
            "On revision target a remaining gap. Previous interview findings: " + json.dumps(previous[-2:])
        )
        q = basis + "\n" + question
        return close(
            question=q, choices="Resolve from supplied evidence or record as open.",
            response=model("Answer from the supplied evidence. Cite a source for confirmed facts. "
                           "Distinguish assumptions and missing evidence. Question: " + q),
        )
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
        spec = model(
            "Build an ordered roadmap from the interview. For each material acceptance criterion give "
            "its source, observable result, and evidence available locally or deferred to the verifier. "
            "Keep open assumptions explicit and name the first action that can resolve each. Interview: " + findings
        )
        decision = model(
            "Review the roadmap against the interview, cited evidence, and actual tools. Revise if it "
            "contradicts a source, drops an uncertainty, or promises unavailable local verification. "
            "Explicitly deferred verification is acceptable. Interview: " + findings + "\nRoadmap: " + spec,
            ["accept", "revise"]
        )
        return close(
            statement=spec,
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
