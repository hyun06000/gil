"""Read the two files Harbor collected, plus pass/fail names from its separate verifier."""
import hashlib
import json


def receipt(folder):
    artifacts = folder / "artifacts" / "app"
    rules_file = artifacts / "rules.json"
    ordering_file = artifacts / "ordering.txt"
    checks_file = folder / "verifier" / "ctrf.json"
    result = {}
    if rules_file.is_file() and ordering_file.is_file():
        raw = rules_file.read_bytes()
        order = ordering_file.read_bytes()
        rules = json.loads(raw)
        names = [s.strip() for s in order.decode().splitlines() if s.strip() and not s.startswith("#")]
        result.update(rule_count=len(rules), ordering_count=len(names),
                      ordering_matches=names == [r["name"] for r in rules],
                      rules_sha256=hashlib.sha256(raw).hexdigest(), ordering_sha256=hashlib.sha256(order).hexdigest())
    if checks_file.is_file():
        result["checks"] = {r["name"]:r["status"] for r in json.loads(checks_file.read_text())["results"]["tests"]}
    return result
