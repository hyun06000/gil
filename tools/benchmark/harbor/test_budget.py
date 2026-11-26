"""A trial that reaches its cost limit ends in order: editing stops, the record is written, nothing is lost."""
import asyncio
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import time
import types
import unittest
from unittest.mock import patch

from luna import BudgetReached, Luna

CODING = Path(__file__).parent / "tb41" / "coding"


def completed(output):
    return lambda *a, **k: io.BytesIO(json.dumps({"status": "completed", "output": output,
                                                  "usage": {"input_tokens": 10, "output_tokens": 10}}).encode())


class BudgetTests(unittest.TestCase):
    def test_editor_requests_stop_at_the_budget_and_other_stages_may_run_a_fifth_past_it(self):
        text = [{"type": "message", "content": [{"type": "output_text", "text": "ok"}]}]
        with patch.dict(os.environ, {"GIL_HARBOR_API_KEY": "test"}), patch("urllib.request.urlopen", side_effect=completed(text)) as send:
            client = Luna(max_calls=None, budget=1_000_000, max_output=100, overrun=0.2)
            client.reserved = 1_000_000
            with self.assertRaises(BudgetReached):
                client.ask([{"role": "user", "content": "edit"}], [], editor=True)
            self.assertEqual(send.call_count, 0)
            self.assertEqual(client.ask([{"role": "user", "content": "analyse"}], []), "ok")
            client.reserved = 1_200_000
            with self.assertRaises(BudgetReached):
                client.ask([{"role": "user", "content": "analyse"}], [])
            self.assertEqual(send.call_count, 1)

    def test_the_host_answers_an_editor_request_over_budget_with_a_stop_and_the_worker_ends_its_editing(self):
        base = types.ModuleType("harbor.agents.base")
        base.BaseAgent = type("BaseAgent", (), {})
        fake = {"harbor": types.ModuleType("harbor"), "harbor.agents": types.ModuleType("harbor.agents"), "harbor.agents.base": base}
        with patch.dict(sys.modules, fake), tempfile.TemporaryDirectory() as temp:
            import gil_harbor
            logs = Path(temp)
            requests = [json.dumps({"messages": [], "choices": [], "editor": True, "notes": False, "stage": "solver"})]
            uploaded, shown = [], []

            class Client:
                calls = inputs = outputs = 0
                def ask(self, messages, choices, **options):
                    raise BudgetReached("limit")
                def usage(self):
                    return {"estimated_usd": 0.5}

            class Environment:
                async def exec(self, command, **options):
                    if "pending.json" in command:
                        return types.SimpleNamespace(return_code=0, stdout=requests.pop() if requests else "", stderr="")
                    return types.SimpleNamespace(return_code=0, stdout="", stderr="")
                async def upload_file(self, source, target):
                    uploaded.append(json.loads(Path(source).read_text()))

            class Agent(gil_harbor.LunaAgent):
                logs_dir = logs
                def model_client(self):
                    return Client()
                async def run_worker(self, environment):
                    while not uploaded:
                        await asyncio.sleep(0.01)
                    return types.SimpleNamespace(return_code=0)
                def live(self, request, answer, client):
                    shown.append(answer)

            context = types.SimpleNamespace()
            result = asyncio.run(Agent().execute(Environment(), context))
            self.assertEqual(result.return_code, 0)
            # Harbor's own result carries the estimated cost, marked as an estimate.
            self.assertEqual((context.cost_usd, context.metadata["cost_is_estimate"]), (0.5, True))
            self.assertEqual(uploaded, [{"answer": "", "stop": "budget"}])
            self.assertEqual(json.loads(shown[0]), {"action": "stop", "reason": "budget"})
            # A stage that cannot end in order still fails the trial, as before.
            requests.append(json.dumps({"messages": [], "choices": [], "editor": False, "stage": "analysis"}))
            uploaded.clear()

            class Waiting(Agent):
                async def run_worker(self, environment):
                    await asyncio.sleep(30)

            with self.assertRaises(BudgetReached):
                asyncio.run(Waiting().execute(Environment(), types.SimpleNamespace()))

    def test_the_solver_closes_with_the_files_as_they_are_and_says_why(self):
        sys.path.insert(0, str(CODING))
        try:
            with patch.dict(os.environ, TB_TASK="vf2-speedup-networkx"):
                import mailbox_client
                import solver
        finally:
            sys.path.pop(0)
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            app, work, mail = root / "app", root / "project", root / "model"
            for folder in (app, work, mail):
                folder.mkdir()
            (root / "deadline").write_text(str(time.time() + 300))
            (app / "fast_networkx.py").write_text("def order(l): return sorted(l)\n")
            replies = iter([{"answer": json.dumps({"action": "write", "path": "fast_networkx.py", "content": "def order(l): return sorted(l, key=str.casefold)\n"})},
                            {"answer": "", "stop": "budget"}])
            real_exists = Path.exists

            def exists(path):
                if path == mail / "reply.json" and not real_exists(path):
                    path.write_text(json.dumps(next(replies)))
                return real_exists(path)

            with patch.multiple(solver, APP=app, WORK=work, TASK="vf2-speedup-networkx"), \
                    patch.object(mailbox_client, "MAIL", mail), patch.object(Path, "exists", exists):
                obs = solver.solve("Order labels ignoring case")
                with self.assertRaisesRegex(RuntimeError, "Model host stopped: budget"):
                    next_reply = {"answer": "", "stop": "budget"}
                    (mail / "reply.json").write_text(json.dumps(next_reply))
                    mailbox_client.ask("Assess", stage="analysis")
            self.assertEqual((obs["stop"], obs["solver_calls"], obs["check_exit"]), ("budget", 1, None))
            self.assertEqual(list(obs["files"]), ["fast_networkx.py"])
            self.assertEqual(json.loads((work / "observation.json").read_text())["stop"], "budget")
            events = [json.loads(l) for l in (mail / "transcript.jsonl").read_text().splitlines()]
            self.assertEqual(events[-3]["event"], "stopped")
            self.assertEqual((app / "fast_networkx.py").read_text(), "def order(l): return sorted(l, key=str.casefold)\n")


if __name__ == "__main__":
    unittest.main()
