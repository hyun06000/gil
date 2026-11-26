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
                Luna(max_calls=None, budget=None, max_output=16000, reasoning="medium", timeout=240).ask([], [], editor=editor)
            body = json.loads(send.call_args.args[0].data)
            self.assertEqual((body["reasoning"], body["max_output_tokens"], send.call_args.kwargs["timeout"]),
                             ({"effort": effort}, 16000, 240))
        # The default client is unchanged.
        with patch.dict("os.environ", {"GIL_HARBOR_API_KEY": "test"}), patch("urllib.request.urlopen", return_value=io.BytesIO(json.dumps(result).encode())) as send:
            Luna().ask([], [])
        self.assertEqual(json.loads(send.call_args.args[0].data)["reasoning"], {"effort": "none"})
        self.assertEqual(send.call_args.kwargs["timeout"], 60)

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
