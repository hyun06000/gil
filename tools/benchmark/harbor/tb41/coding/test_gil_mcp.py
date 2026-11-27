"""The mcp arm against the real product server: a scripted model, the runner's stand-in user, and
the task folder before and after.

Needs a `gil` built with the product features; set GIL_TEST_BINARY to it. Skipped otherwise.
The scripted model shows that the path works, not that a model takes it.
"""
import json
import os
from pathlib import Path
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

with patch.dict(os.environ, TB_TASK="vf2-speedup-networkx"):
    import solver
import gil_mcp

TASK = "vf2-speedup-networkx"
NAME = "fast_networkx/__init__.py"
ORIGINAL = "def order(labels):\n    return sorted(labels)\n"
CHECK = [sys.executable, "-B", "-c", "from fast_networkx import order; assert order(['B','a']) == ['a','B'], 'case order'"]
BINARY = os.environ.get("GIL_TEST_BINARY")
CONTRACT = "objective: 과제 범위를 합의한다\nnext_action: 사용자에게 묻는다\ndone_when: 사용자의 답을 받았다\n"
PLAN = "plan/approved-plan.md"


@unittest.skipUnless(BINARY and Path(BINARY).is_file(), "GIL_TEST_BINARY is not a product-feature gil")
class McpArmTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        root = Path(temp.name).resolve()
        self.app, self.work, self.kept = root / "app", root / "project", root / "gil-record"
        (self.app / "fast_networkx").mkdir(parents=True)
        (self.app / "node_modules/pkg").mkdir(parents=True)
        (self.app / "node_modules/.bin").mkdir()
        (self.app / "node_modules/pkg/index.js").write_text("1\n")
        os.symlink("../pkg/index.js", self.app / "node_modules/.bin/pkg")
        self.work.mkdir()
        (root / "deadline").write_text(str(time.time() + 400))
        (self.app / NAME).write_text(ORIGINAL)
        (self.app / "README.md").write_text("the task's own file\n")
        home = root / "home"
        home.mkdir()
        self.env = patch.dict(os.environ, HOME=str(home))
        self.env.start()
        self.addCleanup(self.env.stop)
        self.events, self.requests = [], []

    def drive(self, script):
        """`script` yields the next answer from the last tool result, as the model would."""
        gil = gil_mcp.Gil(BINARY, self.app, record=self.kept)
        self.gil = gil
        steps = script(gil)
        last = {"result": None}

        def model(prompt, notes=False, tools=None, system=None):
            model.call_id = str(len(self.requests))
            self.requests.append({"prompt": prompt, "tools": tools, "system": system})
            results = [e for e in self.events if e.get("event") == "tool"]
            last["result"] = results[-1] if results else None
            return json.dumps(steps.send(last["result"]) if self.requests[1:] else next(steps))

        with patch.multiple(solver, APP=self.app, WORK=self.work, TASK=TASK, ask=model, RUN_SECONDS=5), \
                patch.dict(solver.CHECKS, {TASK: CHECK}), \
                patch.object(solver, "record", side_effect=lambda **e: self.events.append(e)):
            return solver.solve("Order labels ignoring case", gil=gil)

    def test_a_scripted_agent_walks_start_interview_and_one_experiment_and_the_folder_ends_clean(self):
        root = str(self.app)
        call = lambda tool, **args: {"action": "gil", "tool": tool, "arguments": {"project_root": root, **args}, "note": "n"}
        seen = {}

        def script(gil):
            said = yield call("gil_start", start_kind="tracked_scope")
            rules = "node_modules/\n__pycache__/\n"
            said = yield call("gil_start_prepare", rules=rules)
            seen["start"] = said.get("result") or self.fail(str(self.events[-3:]))
            basis = []
            for kind, report in (("question", "question: 무엇을 원하는가\nchoices: 없음\nresponse: 과제 설명대로\n"),
                                 ("interpretation", "interpretation: 대소문자를 무시해 정렬한다\nunresolved: 없음\n")):
                said = yield call("gil_open", kind=kind, contract=CONTRACT)
                basis.append(__import__("re").search(r"step:C\d+/S\d+", said["result"]).group(0))
                if kind == "question":
                    said = yield {"action": "say", "message": "어떤 정렬을 원하십니까?", "note": "n"}
                    seen["reply"] = said["result"]
                yield call("gil_close", report=report)
            yield call("gil_open", kind="synthesis", contract=CONTRACT)
            said = yield call("gil_plan_propose", draft={
                "scope": {"goal": "대소문자 무시 정렬", "in_scope": "order", "out_of_scope": "그 밖", "constraints": "없음", "success_criteria": "검사 통과"},
                "items": [{"id": "R1", "question": "order 가 대소문자를 무시하는가", "done_when": "검사가 통과한다"}],
                "synthesis": {"statement": "order 를 고친다", "basis_refs": "\n".join(basis), "exploration_checklist": "R1"}})
            seen["plan"] = said["result"]
            # The scripted model reads the server directly where a model would read the tool result.
            _, _, read = gil.server.call("gil_plan_read", {"project_root": root})
            said = yield call("gil_close", report=read["close_report"])
            synthesis = __import__("re").search(r"step:C\d+/S\d+", said["result"]).group(0)
            said = yield call("gil_open", kind="outcome", contract=CONTRACT)
            outcome = __import__("re").search(r"step:C\d+/S\d+", said["result"]).group(0)
            yield call("gil_close", report=f"verdict: success\nlesson: 요구를 기록했다\nsynthesis_ref: {synthesis}\nnext_direction.action: close_cycle\nnext_direction.reason: 탐색한다\n")
            yield call("gil_close", report=f"verdict: success\noutcome_ref: {outcome}\nhandoff_summary: 요구를 승인받았다\nnext_direction.action: open_child\nnext_direction.reason: 실험한다\n")
            _, _, read = gil.server.call("gil_plan_read", {"project_root": root})
            plan = read["active_plan"]
            yield call("gil_open", kind="experiment", contract=f"plan_revision: {plan['revision']}\nplan_digest: {plan['digest']}\nrequirement_id: R1\nrelevance: 필요하다\nreadiness: 준비됐다\nuncertainty: 하나를 줄인다\n")
            yield call("gil_open", kind="define", contract=CONTRACT)
            yield call("gil_close", report="problem: order 가 대소문자를 무시하는가?\nsuccess_condition: 검사가 통과한다. 그 밖은 성공이 아니다\n")
            yield call("gil_open", kind="hypothesis", contract=CONTRACT)
            yield call("gil_close", report="hypothesis: casefold 로 정렬하면 된다\nrationale: 키 함수다\nguardrail: 다른 파일은 고치지 않는다\n")
            yield call("gil_open", kind="verify", contract=CONTRACT)
            _, _, read = gil.server.call("gil_plan_read", {"project_root": root})
            yield {"action": "write", "path": NAME, "content": "def order(labels):\n    return sorted(labels, key=str.casefold)\n", "note": "n"}
            yield {"action": "run", "command": f"mkdir -p plan evidence && cat > {PLAN} <<'DOC'\n{read['document']}DOC\necho passed > evidence/C2.md", "note": "n"}
            yield {"action": "check", "note": "n"}
            said = yield call("gil_close", report=f"execution: 고치고 검사했다\nresult: 통과\nplan_document: {PLAN}\nevidence_files: evidence/C2.md\n")
            seen["verify"] = said["result"]
            yield {"action": "finish", "note": "n"}

        obs = self.drive(script)
        self.assertEqual((obs["stop"], obs["check_exit"]), ("finished", 0))
        # The model was given the server's instructions and tools, never the runner's.
        first = self.requests[0]
        names = [tool["name"] for tool in first["tools"]]
        self.assertIn("gil_open", names)
        self.assertIn("say", names)
        self.assertFalse([name for name in names if "unattended" in name or "monitor" in name])
        self.assertIn("This session is unattended", first["system"])
        self.assertIn("Let's do this as a GIL project", first["prompt"])
        # The stand-in user approved the start and the plan, and answered the question by policy.
        self.assertIn("start approved", seen["start"])
        self.assertIn("plan approved", seen["plan"])
        self.assertIn("go with your recommendation", seen["reply"])
        self.assertNotIn("REFUSED", seen["verify"])
        # What GIL added is out of the submission folder and kept apart; the task's own files remain.
        left = sorted(p.name for p in self.app.iterdir())
        self.assertEqual(left, ["README.md", "evidence", "fast_networkx", "node_modules", "plan"])
        self.assertEqual(sorted(obs["gil"]["removed_from_submission"]), [".gil", ".gilignore", "AGENTS.md", "CLAUDE.md"])
        self.assertTrue((self.kept / ".gil").is_dir() and (self.kept / ".gilignore").is_file())
        self.assertTrue((self.app / "node_modules/.bin/pkg").is_symlink())


class ControlArmTests(unittest.TestCase):
    def test_without_a_connected_server_the_request_names_no_gil_tool_and_no_user_line(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            app, work = root / "app", root / "project"
            (app / "fast_networkx").mkdir(parents=True)
            work.mkdir()
            (root / "deadline").write_text(str(time.time() + 400))
            (app / NAME).write_text(ORIGINAL)
            asked = []

            def model(prompt, notes=False, **more):
                model.call_id = "0"
                asked.append((prompt, more))
                return json.dumps({"action": "stop", "reason": "budget"})

            with patch.multiple(solver, APP=app, WORK=work, TASK=TASK, ask=model), \
                    patch.object(solver, "record", side_effect=lambda **e: None):
                obs = solver.solve("Order labels ignoring case")
            self.assertEqual(asked[0][1], {})
            self.assertNotIn("GIL", asked[0][0])
            self.assertNotIn("gil", obs)


if __name__ == "__main__":
    unittest.main()
