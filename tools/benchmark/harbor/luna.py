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


def editor_tool(name, description, properties):
    return {
        "type": "function", "name": name, "description": description, "strict": True,
        "parameters": {"type": "object", "properties": properties,
                       "required": list(properties), "additionalProperties": False},
    }


EDITOR_TOOLS = [
    editor_tool("read", "Read a source file, starting at a character offset (zero for the beginning).",
                {"path": {"type": "string"}, "offset": {"type": "integer"}}),
    editor_tool("write", "Replace a writable source file with its complete content.",
                {"path": {"type": "string"}, "content": {"type": "string"}}),
    editor_tool("edit", "Replace one passage of a writable source file. The old text must occur exactly once.",
                {"path": {"type": "string"}, "old": {"type": "string"}, "new": {"type": "string"}}),
    editor_tool("run", "Run a shell command in the task directory, without network, and return its exit status and output.",
                {"command": {"type": "string"}}),
    editor_tool("check", "Run the task's local build or smoke check.", {}),
    editor_tool("finish", "Submit the edited and checked files.", {}),
]

# GIL Verify asks for a note with every call: each request is built anew, so the note is what carries over.
NOTE = {"note": {"type": "string", "description":
                 "What the material in this request settled and what you do next (one or two sentences)."}}
NOTED_TOOLS = [editor_tool(t["name"], t["description"], {**t["parameters"]["properties"], **NOTE}) for t in EDITOR_TOOLS]


def editor_answer(output, tools=EDITOR_TOOLS):
    calls = [item for item in output if item["type"] == "function_call"]
    if len(calls) != 1:
        raise ValueError("Expected exactly one editor call")
    call = calls[0]
    args = json.loads(call["arguments"])
    for tool in tools:
        if tool["name"] == call["name"]:
            properties = tool["parameters"]["properties"]
            if not isinstance(args, dict) or args.keys() != properties.keys():
                break
            for key, schema in properties.items():
                expected = int if schema["type"] == "integer" else str
                if type(args[key]) is not expected:
                    raise ValueError("Invalid editor argument type")
            return json.dumps({"action": call["name"], **args}, ensure_ascii=False)
    raise ValueError("Unknown editor tool or invalid arguments")


class BudgetReached(RuntimeError):
    """The trial's call or cost limit: no request was sent."""


class Luna:
    def __init__(self, *, max_calls=MAX_CALLS, budget=BUDGET, max_output=MAX_OUTPUT, reasoning="none", timeout=60, overrun=0.0):
        self.overrun = overrun
        self.reasoning = reasoning
        self.timeout = timeout
        self.max_calls = max_calls
        self.budget = budget
        self.max_output = max_output
        self.key = os.environ["GIL_HARBOR_API_KEY"]
        self.calls = 0
        self.inputs = 0
        self.outputs = 0
        self.reserved = 0

    def ask(self, messages, choices, *, editor=False, notes=False):
        # Reasoning applies to editor requests only: the other stages share the Core's 120 s turn limit.
        body = {"model": MODEL, "input": messages, "reasoning": {"effort": self.reasoning if editor else "none"},
                "max_output_tokens": self.max_output, "store": False, "service_tier": "default"}
        if editor:
            body.update(tools=NOTED_TOOLS if notes else EDITOR_TOOLS, tool_choice="required", parallel_tool_calls=False)
        elif choices:
            body["text"] = {"format": {"type":"json_schema", "name":"selection", "strict":True,
                "schema":{"type":"object", "properties":{"answer":{"type":"string", "enum":choices}},
                          "required":["answer"], "additionalProperties":False}}}
        data = json.dumps(body).encode()
        # Use bytes as a conservative token estimate, with margin for message formatting.
        reservation = (len(data) + 256) * INPUT + self.max_output * OUTPUT
        # Editor requests stop at the budget. A client may let the few short requests of the other stages
        # run past it by `overrun`, so a trial whose editing was stopped can still record its analysis.
        limit = self.budget if editor or self.budget is None else self.budget * (1 + self.overrun)
        if ((self.max_calls is not None and self.calls >= self.max_calls) or
                (limit is not None and self.reserved + reservation > limit)):
            raise BudgetReached("Luna per-trial cost or call limit reached before sending request")
        self.calls += 1
        self.reserved += reservation
        request = urllib.request.Request("https://api.openai.com/v1/responses", data=data, headers={
            "Authorization": "Bearer " + self.key, "Content-Type": "application/json"})
        try:
            with urllib.request.urlopen(request, timeout=self.timeout) as resp:
                result = json.load(resp)
        except urllib.error.HTTPError as error:
            try:
                detail = json.loads(error.read()).get("error", {})
                diagnostic = str(detail.get("message", "")).replace(self.key, "[redacted]")[:350]
            except (ValueError, AttributeError):
                diagnostic = "no structured error message"
            raise RuntimeError(f"Luna API HTTP {error.code} on call {self.calls}: {diagnostic}") from None
        usage = result["usage"]
        self.inputs += usage["input_tokens"]
        self.outputs += usage["output_tokens"]
        self.reserved += usage["input_tokens"] * INPUT + usage["output_tokens"] * OUTPUT - reservation
        if result["status"] != "completed":
            raise RuntimeError(f"Luna response status {result['status']}")
        if editor:
            return editor_answer(result["output"], NOTED_TOOLS if notes else EDITOR_TOOLS)
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
