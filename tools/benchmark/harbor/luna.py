"""Host-only, budgeted Responses API client for one Harbor trial."""
import json
import os
import urllib.error
import urllib.request

MODEL = "gpt-5.6-luna"
MAX_CALLS = 16
MAX_OUTPUT = 128
# Rates in millionths of a USD per token, standard short-context pricing.
INPUT = 0.20
OUTPUT = 1.20
BUDGET = 5000  # $0.005 per trial


class Luna:
    def __init__(self):
        self.key = os.environ["GIL_HARBOR_API_KEY"]
        self.calls = 0
        self.inputs = 0
        self.outputs = 0
        self.reserved = 0

    def ask(self, messages, choices):
        body = {"model": MODEL, "input": messages, "reasoning": {"effort": "none"},
                "max_output_tokens": MAX_OUTPUT, "store": False, "service_tier": "default"}
        if choices:
            body["text"] = {"format": {"type":"json_schema", "name":"selection", "strict":True,
                "schema":{"type":"object", "properties":{"answer":{"type":"string", "enum":choices}},
                          "required":["answer"], "additionalProperties":False}}}
        data = json.dumps(body).encode()
        # Use bytes as a conservative token estimate, with margin for message formatting.
        reservation = (len(data) + 256) * INPUT + MAX_OUTPUT * OUTPUT
        if self.calls >= MAX_CALLS or self.reserved + reservation > BUDGET:
            raise RuntimeError("Luna per-trial cost or call limit reached before sending request")
        self.calls += 1
        self.reserved += reservation
        request = urllib.request.Request("https://api.openai.com/v1/responses", data=data, headers={
            "Authorization": "Bearer " + self.key, "Content-Type": "application/json"})
        try:
            with urllib.request.urlopen(request, timeout=60) as resp:
                result = json.load(resp)
        except urllib.error.HTTPError as error:
            raise RuntimeError(f"Luna API HTTP {error.code} on call {self.calls}") from None
        usage = result["usage"]
        self.inputs += usage["input_tokens"]
        self.outputs += usage["output_tokens"]
        self.reserved += usage["input_tokens"] * INPUT + usage["output_tokens"] * OUTPUT - reservation
        if result["status"] != "completed":
            raise RuntimeError(f"Luna response status {result['status']}")
        answer = "".join(part["text"] for item in result["output"] if item["type"] == "message"
                         for part in item["content"] if part["type"] == "output_text").strip()
        if choices:
            answer = json.loads(answer)["answer"]
        if not answer or (choices and answer not in choices):
            raise ValueError("Luna did not return a permitted answer")
        return answer

    def usage(self):
        return {"model":MODEL, "calls":self.calls, "input_tokens":self.inputs, "output_tokens":self.outputs,
                "estimated_usd":(self.inputs * INPUT + self.outputs * OUTPUT) / 1_000_000}
