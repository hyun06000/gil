import json
import time
import uuid
from pathlib import Path
MAIL = Path("/work/model")
WORK = Path("/work/project")
DEADLINE = Path("/work/deadline")


def record(**event):
    if event.get("event") == "tool":
        # The next request carries what the last call did, so the host can show it while the trial runs.
        ask.last = {k: (v[:300] if isinstance(v, str) else v) for k, v in event.items()
                    if k in ("action", "path", "offset", "command", "error", "result", "note")}
    with (MAIL / "transcript.jsonl").open("a") as log:
        print(json.dumps({"time": time.time(), **event}, ensure_ascii=False), file=log)


def ask(prompt, choices=(), stage="solver", notes=False, ledger=False, schema=None):
    ask.call_id = uuid.uuid4().hex
    messages = [
        {
            "role": "system",
            "content": "You are solving a software implementation benchmark using the supplied task files. Follow the current instruction.",
        },
        {"role": "user", "content": prompt},
    ]
    record(event="request", id=ask.call_id, stage=stage,
           **({"messages": messages} if stage == "hypothesis" else {}))
    draft = MAIL / "request.tmp"
    draft.write_text(json.dumps({"messages": messages, "choices": choices, "editor": stage == "solver", "notes": notes, "ledger": ledger,
                                 "stage": stage, "schema": schema, "last": getattr(ask, "last", None)}))
    draft.replace(MAIL / "request.json")
    reply = MAIL / "reply.json"
    # Longer than the host can take: three tries of 180 s with 25 s of waits for the editor, two of 45 s otherwise.
    until = time.monotonic() + (600 if stage == "solver" else 230)
    while not reply.exists():
        if time.monotonic() > until:
            raise TimeoutError("Model relay timed out")
        time.sleep(0.1)
    packet = json.loads(reply.read_text())
    if packet.get("stop"):
        # The host will not ask the model again. Editing ends in order; any other stage cannot go on.
        reply.unlink()
        record(event="answer", id=ask.call_id, stage=stage, text="", stop=packet["stop"])
        if stage != "solver":
            raise RuntimeError("Model host stopped: " + packet["stop"])
        return json.dumps({"action": "stop", "reason": packet["stop"]})
    answer = packet["answer"]
    record(event="answer", id=ask.call_id, stage=stage, text=answer)
    reply.unlink()
    return answer


