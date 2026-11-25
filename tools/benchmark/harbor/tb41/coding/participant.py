#!/usr/bin/env python3
"""GIL Interview and Experiment adapter for isolated coding tasks."""

import json
import re
import sys
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
            context["cited_source_excerpts"] = cited_sources(task)
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
    if k == "question":
        previous = [s["report"] for s in steps if s["kind"] in ("interpretation", "synthesis") and s["report"]]
        q = model(
            "Ask a question whose answer changes implementation or how success can be established. "
            "Compare brief, cited sources and actual tools. On revision target a remaining gap. "
            "Previous interview findings: " + json.dumps(previous[-2:])
        )
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
