"""The criteria ledger of a GIL benchmark Chain.

The ledger lists what the task requires. Each step of an Experiment has one part in it:

- Define takes some criteria as its problem and writes, for each, the exact state that must hold
  and a probe: a script that exits 0 exactly when that state holds. The probe is fixed there.
- Hypothesis names the method that would bring those states about.
- Verify carries the method out and runs the probes. It writes no probe and changes none.
- Analysis reads the ledger: with the method carried out, do the states hold or not.
- Reflection says why not and where to go back, or names the candidates for the next Experiment.
"""
import hashlib
import json

KINDS = ("failure", "invariant", "behaviour")


def digest(files):
    """One name for the submission as a whole: its files and their hashes."""
    return hashlib.sha256(json.dumps(sorted(files.items())).encode()).hexdigest()


def obj(**properties):
    """A strict JSON schema object: every property required, nothing else allowed."""
    return {"type": "object", "properties": properties, "required": list(properties), "additionalProperties": False}


TEXT = {"type": "string"}
LIST = {"type": "array", "items": TEXT}
CRITERION = obj(requirement=TEXT, kind={"type": "string", "enum": list(KINDS)}, source=TEXT)
# The shapes the participant asks the model for; the provider holds the answer to them.
CRITERIA = obj(criteria={"type": "array", "items": CRITERION})
TARGETS = obj(targets=LIST)
PROBES = obj(probes={"type": "array", "items": obj(id=TEXT, state=TEXT, probe=TEXT)})
HYPOTHESIS = obj(method=TEXT, files=LIST)
CANDIDATES = obj(candidates=LIST, new={"type": "array", "items": CRITERION})
SET_ASIDE = obj(set_aside={"type": "array", "items": obj(id=TEXT, reason=TEXT)})


def loads(text):
    return json.loads(text.strip().removeprefix("```json").removeprefix("```").removesuffix("```").strip())


def item(number, entry):
    requirement = entry.get("requirement") if isinstance(entry, dict) else None
    if not isinstance(requirement, str) or not requirement.strip():
        raise ValueError("every criterion is an object with a requirement")
    return {
        "id": f"K{number}", "kind": entry.get("kind") if entry.get("kind") in KINDS else "behaviour",
        "requirement": requirement.strip(), "source": str(entry.get("source") or "").strip(),
        # Written by the Define that takes the criterion as its problem.
        "state": None, "probe": None,
        "status": "unrun", "exit": None, "output": "", "digest": None, "failed_on": [],
    }


def parse(text):
    """Items from a model's answer: the criteria object, or a bare array of them."""
    try:
        raw = loads(text)
    except ValueError as error:
        raise ValueError(f"not JSON: {error}") from None
    if isinstance(raw, dict):
        raw = raw.get("criteria")
    if not isinstance(raw, list) or not raw:
        raise ValueError("expected a non-empty list of criteria")
    return [item(number, entry) for number, entry in enumerate(raw, 1)]


class Ledger:
    def __init__(self, items):
        self.items = {entry["id"]: entry for entry in items}

    def json(self):
        return list(self.items.values())

    def add(self, entries):
        """Requirements a reflection found the ledger lacked."""
        added = []
        for entry in entries:
            try:
                new = item(len(self.items) + 1, entry)
            except ValueError:
                continue
            self.items[new["id"]] = new
            added.append(new["id"])
        return added

    def define(self, key, state, probe):
        """Define fixes what solved means for a criterion. An earlier result said nothing about this probe."""
        self.items[key].update(state=state.strip(), probe=probe, status="unrun", exit=None, output="", digest=None)

    def status(self, key, current):
        """What is known about a criterion for the submission named by `current`."""
        entry = self.items[key]
        if entry.get("set_aside"):
            return "set_aside"
        if not entry["probe"]:
            return "undefined"     # no Define has said yet what state would count
        if entry["status"] in ("holds", "fails") and entry["digest"] != current:
            return "stale"
        return entry["status"]

    def record(self, key, code, output, current):
        entry = self.items[key]
        entry.update(status="holds" if code == 0 else "fails", exit=code, output=output, digest=current)
        if code != 0 and current not in entry["failed_on"]:
            entry["failed_on"].append(current)
        return entry["status"]

    def set_aside(self, key, reason):
        """A judgement of a reflection: this criterion does not describe the task."""
        self.items[key]["set_aside"] = str(reason).strip() or "set aside"

    def having(self, current, *statuses):
        return [key for key in self.items if self.status(key, current) in statuses]

    def open(self, current):
        return self.having(current, "undefined", "unrun", "fails", "stale")

    def counts(self, current):
        found = {}
        for key in self.items:
            status = self.status(key, current)
            found[status] = found.get(status, 0) + 1
        return found

    def shown(self, current, targets=()):
        rows = []
        for key, entry in self.items.items():
            status = self.status(key, current)
            row = {"id": key, "kind": entry["kind"], "requirement": entry["requirement"], "status": status}
            if entry["source"]:
                row["source"] = entry["source"]
            if key in targets:
                # The problem of this Experiment: the state that must hold and the probe that shows it.
                row.update(target=True, state=entry["state"], probe=entry["probe"])
            if status == "fails":
                row["output"] = entry["output"]
            if entry["failed_on"]:
                # History for whoever judges next; no rule follows from the number.
                row["failed_on_submissions"] = len(entry["failed_on"])
            if entry.get("set_aside"):
                row["set_aside"] = entry["set_aside"]
            rows.append(row)
        return rows
