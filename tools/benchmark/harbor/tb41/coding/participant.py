#!/usr/bin/env python3
"""GIL adapter for the public sound-change task. Solving takes place during Verify."""

import json
import sys
from solver import WORK, ask, solve


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
        return ask(task + "\nCurrent instruction: " + prompt, choices, stage=n["kind"])

    if n["action"] == "open":
        k = n["recommended"]
        return {
            "action": "open",
            "kind": k,
            "contract": f"objective: Complete {k} for the task\nnext_action: Carry out {k} using collected evidence\ndone_when: Record the {k} report",
        }
    k = n["kind"]
    if k == "question":
        q = model(
            "Ask one short question clarifying the deliverable or success criterion."
        )
        return close(
            question=q,
            choices="Derive the answer from the task brief.",
            response=model("Answer from the brief: " + q),
        )
    if k == "interpretation":
        return close(
            interpretation=model(
                "Interpret this participant answer in one sentence: "
                + step("question")["report"]["response"]
            ),
            unresolved="none",
        )
    if k == "synthesis":
        q, i = step("question"), step("interpretation")
        spec = model(
            "Write an actionable experiment specification using only the available source read, write and check tools: "
            + i["report"]["interpretation"]
        )
        decision = model(
            "Does this specification implement the task? " + spec, ["accept", "revise"]
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
    if k == "hypothesis":
        return close(
            hypothesis=model(
                "Suggest an experimentally testable implementation strategy for: "
                + spec
            ),
            rationale=spec,
            guardrail="Use the public task files and local tools.",
        )
    if k == "verify":
        hypothesis = step("hypothesis")["report"]["hypothesis"]
        obs = solve(
            task, "Accepted specification: " + spec + "\nHypothesis: " + hypothesis
        )
        return close(
            execution="Edited the submission files in the task workspace and recorded the local check.",
            result=json.dumps(obs),
            evidence_files="observation.json",
        )
    if k == "analysis":
        obs = (WORK / "observation.json").read_text()
        analysis = model(
            "Interpret this observed training result. The independent verifier has not run: "
            + obs
        )
        achieved = bool(json.loads(obs)["files"]) and json.loads(obs)["check_exit"] == 0
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
