#!/usr/bin/env python3
"""A local model supplies semantic decisions. The adapter exposes bounded CSV operations."""
import csv
import json
import statistics
import sys
from functools import cache
from pathlib import Path

MODEL = "/case/model.gguf"
OPERATIONS = {"mean": statistics.mean, "median": statistics.median, "maximum": max, "minimum": min}
EVIDENCE = Path("observation.json")


@cache
def model():
    from llama_cpp import Llama
    return Llama(model_path=MODEL, n_threads=2, n_ctx=2048, verbose=False, seed=17)


def infer(task, prompt, choices=()):
    from llama_cpp import LlamaGrammar
    grammar = None
    if choices:
        grammar = LlamaGrammar.from_string("root ::= " + " | ".join(map(json.dumps, choices)), verbose=False)
    result = model().create_chat_completion(
        messages=[{"role":"system", "content":"You are a lab participant. Answer only the question in one concise English sentence. Follow the specified output format."},
                  {"role":"user","content":f"Task: {task}\n{prompt}"}],
        temperature=0, max_tokens=80, grammar=grammar,
    )
    answer = result["choices"][0]["message"]["content"].strip()
    if not answer or (choices and answer not in choices):
        raise ValueError(f"Invalid model response to {prompt!r}: {answer!r}")
    return answer


def close(**fields):
    def block(key, value):
        return key + ": |\n" + "".join("  " + line + "\n" for line in str(value).splitlines())
    return {"action":"close","report":"".join(block(k, v) for k, v in fields.items())}


def respond(req):
    n = req["next"]
    task = req["task"]
    interview = req["history"][0]["steps"]
    steps = req["history"][-1]["steps"]
    def step(items, kind):
        return next(s for s in reversed(items) if s["kind"] == kind and s["report"])
    def ask(prompt, choices=()):
        return infer(task, prompt, choices)
    if n["action"] == "open":
        name = n["recommended"]
        return {"action":"open", "kind":name, "contract":f"objective: {name} for the supplied task\nnext_action: Complete {name} using task and history\ndone_when: Record the {name} result"}
    kind = n["kind"]
    if kind == "question":
        question = ask("Ask one question about what the output must measure.")
        return close(question=question, choices="Use the aggregation and column requested in the task.", response=ask(question))
    if kind == "interpretation":
        response = step(steps, "question")["report"]["response"]
        return close(interpretation=ask(f"Participant answered: {response}. Interpret the requested calculation."), unresolved="none")
    if kind == "synthesis":
        q, i = step(steps, "question"), step(steps, "interpretation")
        spec = ask(f"Interpretation: {i['report']['interpretation']}. Write the experimental specification, naming input, exact statistic, and output.")
        decision = ask(f"Does this specification implement the task? {spec}\nAnswer accept or revise.", ["accept","revise"])
        return close(statement=spec, basis_refs=q["ref"]+"\n"+i["ref"], approved="benchmark_"+decision)
    spec = step(interview, "synthesis")["report"]["statement"]
    if kind == "define":
        return close(problem=spec, success_condition=ask(f"Give a measurable success criterion for: {spec}"))
    if kind == "hypothesis":
        return close(hypothesis=ask(f"Propose a testable hypothesis for: {spec}"), rationale=spec, guardrail="Read the supplied latency column only.")
    if kind == "verify":
        operation = ask(f"Select the CSV aggregation that implements this accepted specification: {spec}", list(OPERATIONS))
        with open("/case/latencies.csv") as f:
            values = [int(row["latency_ms"]) for row in csv.DictReader(f)]
        value = OPERATIONS[operation](values)
        EVIDENCE.write_text(json.dumps({"operation":operation,"value":value})+"\n")
        return close(execution=f"Computed {operation} across {len(values)} latency_ms values", result=value, evidence_files=EVIDENCE.name)
    if kind == "analysis":
        observed = EVIDENCE.read_text()
        interpretation = ask(f"Specification: {spec}. Observation: {observed}. Interpret the observation in milliseconds.")
        decision = ask(f"Specification: {spec}. Observation: {observed}. Did the experiment measure the requested statistic? Answer yes or no.", ["yes","no"])
        return close(hypothesis_fit=interpretation, problem_solved=decision, success_condition_met=decision, guardrail_triggered="no", interpretation=interpretation)
    if kind == "outcome":
        fields = {"next_direction.action":"close_cycle","next_direction.reason":"Record the measured result."}
        if n["cycle"] == "interview":
            return close(verdict="success", lesson=spec, synthesis_ref=step(steps,"synthesis")["ref"], **fields)
        analysis = step(steps,"analysis")["report"]
        return close(verdict="success" if analysis["success_condition_met"]=="yes" else "failure", lesson=analysis["interpretation"], **fields)
    if kind == "cycle":
        outcome = step(steps,"outcome")
        verdict = outcome["report"]["verdict"]
        fields = {"next_direction.action":"open_child" if verdict=="success" else "revisit",
                  "next_direction.reason":outcome["report"]["lesson"]}
        if verdict == "failure":
            fields["next_direction.target_cycle_ref"] = "cycle:C1"
        return close(verdict=verdict, outcome_ref=outcome["ref"], handoff_summary=outcome["report"]["lesson"], **fields)
    raise ValueError(n)


if __name__ == "__main__":
    print(json.dumps(respond(json.load(sys.stdin))))
