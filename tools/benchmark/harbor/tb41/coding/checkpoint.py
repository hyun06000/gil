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
