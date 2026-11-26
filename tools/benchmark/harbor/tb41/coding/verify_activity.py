"""Current editor observations for a single GIL Verify step."""
import hashlib
import json



def digest(data):
    return hashlib.sha256(data).hexdigest()


class VerifyActivity:
    def __init__(self, ref, baseline, failure):
        self.ref = ref
        self.files = {}
        self.refused = 0
        for name, sha in baseline.items():
            self.version(name, sha)
        self.failure = failure
        self.repeats = 0

    def version(self, name, sha):
        info = self.files.get(name)
        if info is None or info["sha256"] != sha:
            info = self.files[name] = {"sha256": sha, "pages": {}, "written_here": False}
        return info

    def read(self, name, data, offset, end, eof, shown=False):
        info = self.version(name, digest(data))
        pages = info["pages"]
        page = pages.setdefault(offset, {"end": end, "eof": eof, "reads": 0})
        page["reads"] += 1
        if page["reads"] > 1:
            self.repeats += 1
        if shown:
            self.refused += 1
        return page["reads"]

    def write(self, name, sha, previous):
        info = self.version(name, sha)
        if previous != sha:
            info.update(written_here=True, previous_sha256=previous)
        else:
            info["unchanged_writes"] = info.get("unchanged_writes", 0) + 1

    def state(self, command, checked, stalled=0):
        return {"verify": self.ref, "files": self.files, "repeat_reads": self.repeats,
                "refused_reads": self.refused, "stalled_calls": stalled,
                "local_check": {"command": command, "current_exit": checked,
                                "pending": checked is None, "last_failure": self.failure}}

    def prompt(self, command, checked, stalled=0):
        return ("\nGIL Verify activity. Page offsets are character ranges [start,end). "
                "If you edited to address a failed check, run that check on the new version. "
                "Only the listed local command is fixed; what you run yourself is evidence too, and criteria "
                "nothing ran remain unverified.\n"
                + json.dumps(self.state(command, checked, stalled), ensure_ascii=False))
