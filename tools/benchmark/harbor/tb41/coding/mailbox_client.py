import json
import time
from pathlib import Path
MAIL = Path("/work/model")
WORK = Path("/work/project")
DEADLINE = Path("/work/deadline")


def ask(prompt, choices=()):
    messages = [
        {
            "role": "system",
            "content": "You are solving a software implementation benchmark using the supplied task files. Follow the current instruction.",
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


