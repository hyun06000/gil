"""Identical sound-change tools and Luna solver for both experimental conditions."""

import csv
import difflib
import json
import time
from collections import Counter
from pathlib import Path

MAIL = Path("/work/model")
WORK = Path("/work/project")
DEADLINE = Path("/work/deadline")


def ask(prompt, choices=()):
    messages = [
        {
            "role": "system",
            "content": "You are solving a historical linguistics benchmark. Work only with the provided task and training evidence. Follow the current instruction.",
        },
        {"role": "user", "content": prompt},
    ]
    draft = MAIL / "request.tmp"
    draft.write_text(json.dumps({"messages": messages, "choices": choices}))
    draft.replace(MAIL / "request.json")
    reply = MAIL / "reply.json"
    until = time.monotonic() + 85
    while not reply.exists():
        if time.monotonic() > until:
            raise TimeoutError("Model relay timed out")
        time.sleep(0.1)
    answer = json.loads(reply.read_text())["answer"]
    reply.unlink()
    return answer


def validated(rules):
    if not isinstance(rules, list):
        raise ValueError("rules must be an array")
    keys = {"name", "src", "tgt", "left", "right"}
    for r in rules:
        if (
            not isinstance(r, dict)
            or set(r) != keys
            or not all(isinstance(v, str) for v in r.values())
        ):
            raise ValueError("Supply exactly five string fields per rule")
        if not r["src"] or not r["name"] or "\n" in r["name"]:
            raise ValueError("Each rule needs a nonempty source and one-line name")
    if len({r["name"] for r in rules}) != len(rules):
        raise ValueError("Rule names must be unique")
    return rules


def evaluate(pairs, rules):
    from apply import apply_cascade

    errors = []
    edits = Counter()
    for proto, target in pairs:
        actual = apply_cascade(proto, rules)
        if actual != target:
            errors.append((proto, actual, target))
            for tag, i, j, k, l in difflib.SequenceMatcher(
                None, actual, target, autojunk=False
            ).get_opcodes():
                if tag != "equal":
                    edits[(actual[i:j], target[k:l])] += 1
    return len(pairs) - len(errors), errors, edits


def submit(rules):
    for folder in (WORK, Path("/app")):
        (folder / "rules.json").write_text(json.dumps(rules, ensure_ascii=False))
        (folder / "ordering.txt").write_text("".join(r["name"] + "\n" for r in rules))


def solve(task, plan=""):
    import sys

    sys.path.insert(0, "/app/engine")
    pairs = list(csv.reader(open("/app/data/train.tsv"), delimiter="\t"))
    if pairs and pairs[0][0].lower() in ("proto", "proto_form"):
        pairs = pairs[1:]
    best = []
    score, errors, edits = evaluate(pairs, best)
    feedback = ""
    tries = 0
    evaluated = 0
    rejected_finishes = 0
    deadline = float(DEADLINE.read_text())
    while time.time() < deadline - 95:
        offset = (tries * 13) % max(1, len(errors))
        sample = (errors[offset:] + errors[:offset])[:22]
        prompt = f"""{task}
{plan}
You can evaluate candidate ordered rule lists on the training data and submit the best list.
Current best list: {json.dumps(best, ensure_ascii=False)}
Exact matches: {score}/{len(pairs)}. Sample errors (proto, current prediction, target): {sample}
Frequent outstanding edits (source, target), count: {edits.most_common(20)}
Previous attempt: {feedback}
Return one JSON object. To test a revised complete list use {{"action":"evaluate","rules":[...]}}.
First evaluate at least one nonempty rule list. To submit the best evaluated list use {{"action":"finish"}}. Improve where evidence supports a rule.
Finish when further changes have no plausible basis. Output only JSON."""
        answer = ask(prompt)
        tries += 1
        try:
            raw = (
                answer.strip()
                .removeprefix("```json")
                .removeprefix("```")
                .removesuffix("```")
            )
            choice = json.loads(raw)
            if choice.get("action") == "finish":
                if not best:
                    rejected_finishes += 1
                    raise ValueError(
                        "No nonempty candidate has been evaluated. Propose a rule from the training errors."
                    )
                break
            if choice.get("action") != "evaluate":
                raise ValueError("Choose evaluate or finish")
            rules = validated(choice["rules"])
            if not rules:
                raise ValueError("An empty ordering is not a valid submission")
            candidate, misses, remaining = evaluate(pairs, rules)
            evaluated += 1
            feedback = f"{candidate} correct with proposed list {json.dumps(rules, ensure_ascii=False)}"
            if not best or candidate > score:
                score, errors, edits, best = candidate, misses, remaining, rules
                submit(best)
            if score == len(pairs):
                break
        except (KeyError, ValueError, TypeError, AttributeError) as exc:
            feedback = str(exc)
    if not best:
        raise RuntimeError("No valid candidate before the trial deadline")
    observation = {
        "evaluated": evaluated,
        "rejected_empty_finishes": rejected_finishes,
        "training_exact": score,
        "training_total": len(pairs),
        "solver_calls": tries,
        "rules": len(best),
    }
    (WORK / "observation.json").write_text(json.dumps(observation))
    return observation
