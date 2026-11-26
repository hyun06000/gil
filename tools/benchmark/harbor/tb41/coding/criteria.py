"""The criteria ledger of a GIL benchmark Chain.

Define splits the public requirements into items. Verify runs a command for an item, and the item
holds only while that command exits 0 on the files as they are. Analysis reads the ledger instead
of asking for an opinion, and the next Experiment takes the items still open.
"""
import hashlib
import json

KINDS = ("failure", "invariant", "behaviour")


def digest(files):
    """One name for the submission as a whole: its files and their hashes."""
    return hashlib.sha256(json.dumps(sorted(files.items())).encode()).hexdigest()


def parse(text):
    """Items from a model's JSON array; ValueError says what to repair."""
    body = text.strip().removeprefix("```json").removeprefix("```").removesuffix("```").strip()
    try:
        raw = json.loads(body)
    except ValueError as error:
        raise ValueError(f"not a JSON array: {error}") from None
    if not isinstance(raw, list) or not raw:
        raise ValueError("expected a non-empty JSON array of criteria")
    items = []
    for entry in raw:
        if not isinstance(entry, dict):
            raise ValueError("every criterion is an object")
        requirement = entry.get("requirement")
        if not isinstance(requirement, str) or not requirement.strip():
            raise ValueError("every criterion needs a requirement")
        kind = entry.get("kind") if entry.get("kind") in KINDS else "behaviour"
        probe = entry.get("probe")
        items.append({
            "id": f"K{len(items) + 1}", "kind": kind, "requirement": requirement.strip(),
            "source": str(entry.get("source") or "").strip(),
            "plan": probe.strip() if isinstance(probe, str) and probe.strip() else None,
            "status": "unrun", "command": None, "exit": None, "output": "", "digest": None, "failed_on": [],
        })
    return items


class Ledger:
    def __init__(self, items):
        self.items = {item["id"]: item for item in items}

    def json(self):
        return list(self.items.values())

    def status(self, key, current):
        """What is known about an item for the submission named by `current`."""
        item = self.items[key]
        if item["plan"] is None and item["command"] is None:
            return "unverifiable"
        if item.get("set_aside"):
            return "set_aside"
        if item["status"] in ("holds", "fails") and item["digest"] != current:
            return "stale"
        return item["status"]

    def record(self, key, command, code, output, current):
        item = self.items[key]
        item.update(status="holds" if code == 0 else "fails", command=command, exit=code,
                    output=output, digest=current)
        if code == 0:
            # A probe that passes settles a criterion that had been set aside.
            item.pop("set_aside", None)
        elif current not in item["failed_on"]:
            item["failed_on"].append(current)
        return item["status"]

    def set_aside(self, key, reason):
        """A judgement made in Define: this criterion or its probe does not describe the task."""
        self.items[key]["set_aside"] = str(reason).strip() or "set aside"

    def having(self, current, *statuses):
        return [key for key in self.items if self.status(key, current) in statuses]

    def open(self, current):
        return self.having(current, "unrun", "fails", "stale")

    def counts(self, current):
        found = {}
        for key in self.items:
            status = self.status(key, current)
            found[status] = found.get(status, 0) + 1
        return found

    def shown(self, current, targets=()):
        rows = []
        for key, item in self.items.items():
            status = self.status(key, current)
            row = {"id": key, "kind": item["kind"], "requirement": item["requirement"], "status": status}
            if key in targets:
                row["target"] = True
            if item["source"]:
                row["source"] = item["source"]
            if item["command"]:
                row["probe"] = item["command"]
                if status == "fails":
                    row["output"] = item["output"]
            elif item["plan"]:
                row["probe_plan"] = item["plan"]
            if item["failed_on"]:
                # History for whoever judges next; no rule follows from the number.
                row["failed_on_submissions"] = len(item["failed_on"])
            if item.get("set_aside"):
                row["set_aside"] = item["set_aside"]
            rows.append(row)
        return rows
