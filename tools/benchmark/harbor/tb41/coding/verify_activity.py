"""Current editor observations for a single GIL Verify step."""
import hashlib
import json

# Page text kept in every request (three full pages). A held page is not sent again.
PAGE_BUDGET = 27000
NOTES_KEPT = 16
NOTE_LIMIT = 400
# Consecutive calls that add no new page, no new file content and no new check result.
STALL_WARNING = 6
STALL_LIMIT = 12


def digest(data):
    return hashlib.sha256(data).hexdigest()


class VerifyActivity:
    def __init__(self, ref, baseline, failure):
        self.ref = ref
        self.files = {}
        self.held = {}
        self.seen = set()
        self.contents = {}
        self.notes = []
        self.stall = 0
        self.refused = 0
        for name, sha in baseline.items():
            self.version(name, sha)
        self.failure = failure
        self.repeats = 0

    def version(self, name, sha):
        info = self.files.get(name)
        if info is None or info["sha256"] != sha:
            info = self.files[name] = {"sha256": sha, "pages": {}, "written_here": False}
            # Pages of a replaced version are no longer what the file says.
            for key in [k for k in self.held if k[0] == name]:
                del self.held[key]
        if sha:
            self.contents.setdefault(name, set()).add(sha)
        return info

    def read(self, name, data, offset, end, eof):
        info = self.version(name, digest(data))
        pages = info["pages"]
        page = pages.setdefault(offset, {"end": end, "eof": eof, "reads": 0})
        page["reads"] += 1
        if page["reads"] > 1:
            self.repeats += 1
        return page["reads"]

    def page(self, name, data, offset, text, eof):
        """Counts a read and decides its reply: the page, or a refusal when the request already carries it."""
        count = self.read(name, data, offset, offset + len(text), eof)
        sha = self.files[name]["sha256"]
        held = (name, offset) in self.held
        new = (name, sha, offset) not in self.seen
        self.seen.add((name, sha, offset))
        self.held.pop((name, offset), None)
        self.held[(name, offset)] = {"path": name, "offset": offset, "end": offset + len(text), "eof": eof, "text": text}
        while len(self.held) > 1 and sum(len(p["text"]) for p in self.held.values()) > PAGE_BUDGET:
            del self.held[next(iter(self.held))]
        if held:
            self.refused += 1
            reply = (f"Not sent again: {name} [{offset},{offset + len(text)}) of version {sha[:12]} is unchanged "
                     "and is under 'Pages you hold'. Write a change, run the check, or read another page.")
        else:
            reply = text
        self.progress(new)
        return {"read_count": count, "sha256": sha, "held": held}, reply

    def write(self, name, sha, previous):
        known = self.contents.setdefault(name, set())
        if previous:
            known.add(previous)
        # Content this Verify already had for the file is a return, not a new attempt.
        self.progress(previous != sha and sha not in known)
        info = self.version(name, sha)
        if previous != sha:
            info.update(written_here=True, previous_sha256=previous)
        else:
            info["unchanged_writes"] = info.get("unchanged_writes", 0) + 1

    def progress(self, new):
        self.stall = 0 if new else self.stall + 1

    def stalled(self):
        return self.stall >= STALL_LIMIT

    def note(self, call, action, text):
        if isinstance(text, str) and text.strip():
            self.notes.append({"call": call, "action": action, "note": text.strip()[:NOTE_LIMIT]})

    def state(self, command, checked):
        return {"verify": self.ref, "files": self.files, "repeat_reads": self.repeats,
                "refused_reads": self.refused, "stalled_calls": self.stall,
                "local_check": {"command": command, "current_exit": checked,
                                "pending": checked is None, "last_failure": self.failure}}

    def prompt(self, command, checked):
        text = ('\nEvery tool call takes "note": what the material in this request settled and what you do next. '
                "Each request is built anew: your notes, the pages you hold and the receipts below are all that "
                "carries over, so put any conclusion you want to keep in the note.")
        if self.notes:
            text += "\nYour notes in this Verify, oldest first: " + json.dumps(self.notes[-NOTES_KEPT:], ensure_ascii=False)
        if self.held:
            text += ("\nPages you hold (current file versions; reading one of these again returns no text): "
                     + json.dumps(list(self.held.values()), ensure_ascii=False))
        if self.stall >= STALL_WARNING:
            text += (f"\n{self.stall} calls in a row brought no new page, no new file content and no new check "
                     f"result. After {STALL_LIMIT - self.stall} more, this Verify closes with what is recorded. "
                     "Write the change your notes point to, or run the check.")
        return (text + "\nGIL Verify activity. Page offsets are character ranges [start,end). "
                "Reuse a previously read version unless you need its text again. "
                "If you edited to address a failed check, run that check on the new version. "
                "Only the listed local command is available; report other criteria as unverified.\n"
                + json.dumps(self.state(command, checked), ensure_ascii=False))
