import json
import time
import uuid
from pathlib import Path
MAIL = Path("/work/model")
WORK = Path("/work/project")
DEADLINE = Path("/work/deadline")


def record(**event):
    with (MAIL / "transcript.jsonl").open("a") as log:
        print(json.dumps({"time": time.time(), **event}, ensure_ascii=False), file=log)


def ask(prompt, choices=(), stage="solver"):
    ask.call_id = uuid.uuid4().hex
    record(event="request", id=ask.call_id, stage=stage)
    messages = [
        {
            "role": "system",
            "content": "You are solving a software implementation benchmark using the supplied task files. Follow the current instruction.",
        },
        {"role": "user", "content": prompt},
    ]
    draft = MAIL / "request.tmp"
    draft.write_text(json.dumps({"messages": messages, "choices": choices, "editor": stage == "solver"}))
    draft.replace(MAIL / "request.json")
    reply = MAIL / "reply.json"
    until = time.monotonic() + 85
    while not reply.exists():
        if time.monotonic() > until:
            raise TimeoutError("Model relay timed out")
        time.sleep(0.1)
    answer = json.loads(reply.read_text())["answer"]
    record(event="answer", id=ask.call_id, stage=stage, text=answer)
    reply.unlink()
    return answer


