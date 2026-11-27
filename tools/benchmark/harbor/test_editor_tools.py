import io
import json
import unittest
from unittest.mock import patch

from luna import Luna, editor_answer


def tool_call(name, **args):
    return {"type": "function_call", "name": name, "arguments": json.dumps(args)}


class EditorTests(unittest.TestCase):
    def test_mixed_prose_and_tool(self):
        output = [{"type": "message", "content": [{"type": "output_text", "text": "We need inspect."}]},
                  tool_call("read", path="package.json", offset=0)]
        self.assertEqual(json.loads(editor_answer(output)), {"action": "read", "path": "package.json", "offset": 0})

    def test_invalid_calls(self):
        for output in ([], [tool_call("check"), tool_call("finish")], [tool_call("unknown")],
                       [tool_call("write", path="x")], [tool_call("read", path="x", offset=True)],
                       [tool_call("finish", action="write")]):
            with self.subTest(output=output), self.assertRaises(ValueError):
                editor_answer(output)

    def test_a_verify_request_takes_a_note_with_every_call_and_the_control_request_takes_none(self):
        noted = [tool_call("read", path="a.py", offset=0, note="look for the key")]
        plain = [tool_call("read", path="a.py", offset=0)]
        for notes, output, other in ((True, noted, plain), (False, plain, noted)):
            result = {"status": "completed", "output": output, "usage": {"input_tokens": 3, "output_tokens": 5}}
            with patch.dict("os.environ", {"GIL_HARBOR_API_KEY": "test"}), patch("urllib.request.urlopen", side_effect=lambda *a, **k: io.BytesIO(json.dumps(result).encode())) as send:
                client = Luna(max_calls=None, budget=None)
                answer = json.loads(client.ask([], [], editor=True, notes=notes))
                result["output"] = other
                with self.assertRaises(ValueError):
                    client.ask([], [], editor=True, notes=notes)
            self.assertEqual(answer.get("note"), "look for the key" if notes else None)
            for tool in json.loads(send.call_args.args[0].data)["tools"]:
                self.assertEqual("note" in tool["parameters"]["properties"], notes)
                self.assertEqual(tool["parameters"]["required"], list(tool["parameters"]["properties"]))

    def test_run_and_edit_calls_are_accepted_and_only_editor_requests_reason(self):
        self.assertEqual(json.loads(editor_answer([tool_call("run", command="make")])), {"action": "run", "command": "make"})
        self.assertEqual(json.loads(editor_answer([tool_call("edit", path="a.py", old="x", new="y")])),
                         {"action": "edit", "path": "a.py", "old": "x", "new": "y"})
        for output in ([tool_call("run")], [tool_call("run", command=3)], [tool_call("edit", path="a.py", old="x")]):
            with self.subTest(output=output), self.assertRaises(ValueError):
                editor_answer(output)
        text = [{"type": "message", "content": [{"type": "output_text", "text": "ok"}]}]
        for editor, output, effort in ((True, [tool_call("check")], "medium"), (False, text, "none")):
            result = {"status": "completed", "output": output, "usage": {"input_tokens": 3, "output_tokens": 5}}
            with patch.dict("os.environ", {"GIL_HARBOR_API_KEY": "test"}), patch("urllib.request.urlopen", return_value=io.BytesIO(json.dumps(result).encode())) as send:
                Luna(max_calls=None, budget=None, max_output=16000, reasoning="medium", timeout=240, stage_timeout=45).ask([], [], editor=editor)
            body = json.loads(send.call_args.args[0].data)
            # A stage without the editor shares the Core's 120 s turn, so its request waits 45 s at most.
            self.assertEqual((body["reasoning"], body["max_output_tokens"], send.call_args.kwargs["timeout"]),
                             ({"effort": effort}, 16000, 240 if editor else 45))
        # The default client is unchanged.
        with patch.dict("os.environ", {"GIL_HARBOR_API_KEY": "test"}), patch("urllib.request.urlopen", return_value=io.BytesIO(json.dumps(result).encode())) as send:
            Luna().ask([], [])
        self.assertEqual(json.loads(send.call_args.args[0].data)["reasoning"], {"effort": "none"})
        self.assertEqual(send.call_args.kwargs["timeout"], 60)

    def test_the_coding_agent_keeps_its_own_run_and_setup_and_its_model_has_a_cost_limit(self):
        import sys, types
        base = types.ModuleType("harbor.agents.base")
        base.BaseAgent = type("BaseAgent", (), {})
        with patch.dict(sys.modules, {"harbor": types.ModuleType("harbor"), "harbor.agents": types.ModuleType("harbor.agents"),
                                      "harbor.agents.base": base}), \
                patch.dict("os.environ", {"GIL_HARBOR_API_KEY": "test", "TB_ARM": "off", "TB_TASK": "react-lead-form"}):
            sys.path.insert(0, str(__import__("pathlib").Path(__file__).parent / "tb41"))
            try:
                from coding_agent import CodeAgent
                from tb_agent import SoundChangeAgent
            finally:
                sys.path.pop(0)
            for cls in (CodeAgent, SoundChangeAgent):
                for method in ("setup", "run", "run_worker", "model_client"):
                    owner = getattr(cls, method).__qualname__.split(".")[0]
                    self.assertIn(owner, ("CodeAgent", "SoundChangeAgent"), f"{cls.__name__}.{method} fell back to {owner}")
            client = CodeAgent().model_client()
            self.assertEqual((client.budget, client.max_calls, client.reasoning, client.max_output, client.overrun, client.timeout, client.stage_timeout),
                             (5_000_000, None, "medium", 16000, 0.2, 180, 100))
            with patch.dict("os.environ", {"TB_BUDGET_USD": "0.75"}):
                self.assertEqual(CodeAgent().model_client().budget, 750_000)
            for refused in ("0", "9", "-1"):
                with patch.dict("os.environ", {"TB_BUDGET_USD": refused}), self.assertRaises(ValueError):
                    CodeAgent().model_client()
            client.reserved = 5_000_000
            with patch("urllib.request.urlopen") as send, self.assertRaisesRegex(RuntimeError, "limit"):
                client.ask([{"role": "user", "content": "x"}], [], editor=True)
            send.assert_not_called()

    def test_only_a_ledger_request_can_run_a_fixed_probe_and_no_command_carries_a_criterion(self):
        from luna import EDITOR_TOOLS, LEDGER_TOOLS, NOTED_TOOLS
        names = lambda tools: {t["name"]: list(t["parameters"]["properties"]) for t in tools}
        self.assertEqual(names(LEDGER_TOOLS)["probe"], ["criterion", "note"])
        self.assertEqual({k: v for k, v in names(LEDGER_TOOLS).items() if k != "probe"}, names(NOTED_TOOLS))
        self.assertNotIn("probe", {**names(NOTED_TOOLS), **names(EDITOR_TOOLS)})
        # A command is the model's own look at things; it cannot be credited to a criterion.
        self.assertEqual(names(LEDGER_TOOLS)["run"], ["command", "note"])
        probe = [tool_call("probe", criterion="K3", note="see where it stands")]
        result = {"status": "completed", "output": probe, "usage": {"input_tokens": 3, "output_tokens": 5}}
        with patch.dict("os.environ", {"GIL_HARBOR_API_KEY": "test"}), patch("urllib.request.urlopen", side_effect=lambda *a, **k: io.BytesIO(json.dumps(result).encode())) as send:
            client = Luna(max_calls=None, budget=None)
            self.assertEqual(json.loads(client.ask([], [], editor=True, notes=True, ledger=True)),
                             {"action": "probe", "criterion": "K3", "note": "see where it stands"})
            self.assertEqual(json.loads(send.call_args.args[0].data)["tools"], LEDGER_TOOLS)
            with self.assertRaises(ValueError):
                client.ask([], [], editor=True, notes=True)

    def test_a_request_that_reasons_outside_the_editor_gets_the_effort_and_the_long_wait(self):
        text = [{"type": "message", "content": [{"type": "output_text", "text": "{}"}]}]
        result = {"status": "completed", "output": text, "usage": {"input_tokens": 3, "output_tokens": 5}}
        for reason, effort, wait in ((True, "medium", 180), (False, "none", 100)):
            with patch.dict("os.environ", {"GIL_HARBOR_API_KEY": "test"}), patch("urllib.request.urlopen", return_value=io.BytesIO(json.dumps(result).encode())) as send:
                Luna(max_calls=None, budget=None, reasoning="medium", timeout=180, stage_timeout=100).ask([], [], reason=reason)
            self.assertEqual((json.loads(send.call_args.args[0].data)["reasoning"], send.call_args.kwargs["timeout"]), ({"effort": effort}, wait))

    def test_a_request_with_a_schema_asks_the_provider_to_hold_the_answer_to_it(self):
        schema = {"type": "object", "properties": {"targets": {"type": "array", "items": {"type": "string"}}},
                  "required": ["targets"], "additionalProperties": False}
        text = [{"type": "message", "content": [{"type": "output_text", "text": '{"targets": ["K1"]}'}]}]
        result = {"status": "completed", "output": text, "usage": {"input_tokens": 3, "output_tokens": 5}}
        with patch.dict("os.environ", {"GIL_HARBOR_API_KEY": "test"}), patch("urllib.request.urlopen", return_value=io.BytesIO(json.dumps(result).encode())) as send:
            self.assertEqual(Luna(max_calls=None, budget=None).ask([], [], schema=schema), '{"targets": ["K1"]}')
        body = json.loads(send.call_args.args[0].data)
        self.assertEqual(body["text"], {"format": {"type": "json_schema", "name": "answer", "strict": True, "schema": schema}})
        self.assertNotIn("tools", body)

    def test_editor_request_and_ordinary_messages(self):
        scenarios = [(True, [], [tool_call("write", path="x.ts", content="export {}")], "write"),
                     (False, [], [], "hello"), (False, ["accept"], [], "accept")]
        for editor, choices, calls, expected in scenarios:
            with self.subTest(editor=editor, choices=choices):
                message = json.dumps({"answer": expected}) if choices else expected
                output = calls or [{"type": "message", "content": [{"type": "output_text", "text": message}]}]
                result = {"status": "completed", "output": output, "usage": {"input_tokens": 3, "output_tokens": 5}}
                with patch.dict("os.environ", {"GIL_HARBOR_API_KEY": "test"}), patch("urllib.request.urlopen", return_value=io.BytesIO(json.dumps(result).encode())) as send:
                    answer = Luna(max_calls=None, budget=None).ask([], choices, editor=editor)
                body = json.loads(send.call_args.args[0].data)
                if editor:
                    self.assertEqual(body["tool_choice"], "required")
                    self.assertIs(body["parallel_tool_calls"], False)
                    self.assertEqual(json.loads(answer)["action"], expected)
                else:
                    self.assertNotIn("tools", body)
                    self.assertEqual(answer, expected)
