"""Verified submission checkpoint for a single isolated Verify step."""
import hashlib


class Checkpoint:
    def __init__(self):
        self.passed = None
        self.before = {}

    def before_write(self, path):
        if self.passed is not None and path not in self.before:
            self.before[path] = path.read_bytes() if path.exists() else None

    def accept(self, writes):
        self.passed = writes.copy()
        self.before.clear()

    def restore(self, writes, root, observe):
        if not self.before:
            return False
        changes = {}
        for path, original in self.before.items():
            name = path.relative_to(root).as_posix()
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            if digest != writes[name]:
                raise RuntimeError(f"Source changed outside editor: {name}")
            previous = hashlib.sha256(original).hexdigest() if original is not None else None
            if digest != previous:
                changes[name] = {"failed": digest, "restored": previous}
        if changes:
            observe("restore_started", changes)
            for path, original in self.before.items():
                if path.relative_to(root).as_posix() not in changes:
                    continue
                if original is None:
                    path.unlink()
                else:
                    temp = path.with_name(path.name + ".gil-restore-tmp")
                    with temp.open("xb") as handle:
                        handle.write(original)
                    temp.chmod(path.stat().st_mode)
                    temp.replace(path)
            observe("restore_completed", changes)
        self.before.clear()
        writes.clear()
        writes.update(self.passed)
        return bool(changes)


def seed_submission():
    """Capture pre-existing editor-writable files before Core takes the entry snapshot."""
    import os
    from pathlib import Path
    from solver import APP, WORK, source

    for directory, children, files in os.walk(APP):
        children[:] = [c for c in children if c not in (".git", "node_modules")]
        for name in files:
            path = Path(directory, name)
            if path.is_symlink() or not path.is_file():
                continue
            try:
                source(str(path), True)
            except ValueError:
                continue
            capture_file(WORK, APP, path)


def capture_file(work, app, path):
    mirror = work / "submission" / path.relative_to(app)
    mirror.parent.mkdir(parents=True, exist_ok=True)
    mirror.write_bytes(path.read_bytes())


def project_submission(request, work, source, record):
    """Apply and verify the Core-restored bytes for files touched by the failed trial."""
    import json
    import os
    import tempfile
    next_action = request["next"]
    failed = next(c for c in request["history"] if c["ref"] == next_action["failed"])
    verify = next(s for s in reversed(failed["steps"]) if s["kind"] == "verify")
    files = json.loads(verify["report"]["result"])["files"]
    changes = {}
    def digest(path):
        return hashlib.sha256(path.read_bytes()).hexdigest() if path.exists() else None

    for name, expected in files.items():
        actual = source(name, True)
        target = work / "submission" / name
        before, after = digest(actual), digest(target)
        if before != expected:
            raise RuntimeError(f"Submission differs from failed Verify: {name}")
        if before != after:
            changes[name] = {"failed": before, "restored": after}
    if not changes:
        raise ValueError("Revisit requires a changed submission")
    record(event="cycle_restore_started", changes=changes, **next_action)
    for name, hashes in changes.items():
        actual = source(name, True)
        target = work / "submission" / name
        if target.exists():
            with tempfile.NamedTemporaryFile(dir=actual.parent, delete=False) as tmp:
                tmp.write(target.read_bytes())
            os.chmod(tmp.name, actual.stat().st_mode)
            os.replace(tmp.name, actual)
        else:
            actual.unlink()
        if digest(actual) != hashes["restored"]:
            raise RuntimeError(f"Submission projection failed: {name}")
    record(event="cycle_restore_completed", changes=changes, **next_action)
    return {"action": "projected"}
