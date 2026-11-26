"""Harbor owns the trial. GIL owns the Interview and Experiment graph."""
import asyncio
import json
from pathlib import Path
import tempfile
from harbor.agents.base import BaseAgent


class GilAgent(BaseAgent):
    @staticmethod
    def name():
        return "gil-qwen-local"

    def version(self):
        return "1"

    async def setup(self, environment):
        pass

    async def run(self, instruction, environment, context):
        with tempfile.TemporaryDirectory() as tmp:
            task = Path(tmp) / "task.txt"
            task.write_text(instruction)
            await environment.upload_file(task, "/case/task.txt")
        result = await self.execute(environment, context)
        if result.return_code != 0:
            raise RuntimeError(f"GIL exited {result.return_code}: {result.stderr} {result.stdout}")
        await environment.download_file("/work/result.json", self.logs_dir / "gil-result.json")
        await environment.download_file("/work/project/observation.json", self.logs_dir / "observation.json")

    async def execute(self, environment, context):
        return await self.run_worker(environment)

    async def run_worker(self, environment):
        return await environment.exec(
            "chmod 644 /case/task.txt && "
            "setpriv --reuid=65534 --regid=65534 --clear-groups --no-new-privs "
            "python /case/run-worker.py",
            timeout_sec=850, user="root",
        )


class LunaAgent(GilAgent):
    @staticmethod
    def name():
        return "gil-gpt-5.6-luna"

    def model_client(self):
        from luna import Luna
        return Luna()

    def live(self, request, answer, client):
        """Called on the host after every model reply; a trial may show or keep it as it happens."""

    async def execute(self, environment, context):
        client = self.model_client()
        worker = asyncio.create_task(self.run_worker(environment))
        try:
            with tempfile.TemporaryDirectory() as tmp:
                reply = Path(tmp) / "reply.json"
                while not worker.done():
                    # Receive a request once. The adapter waits for its reply before posting another.
                    packet = await environment.exec(
                        "cd /work/model && if mv request.json pending.json 2>/dev/null; then cat pending.json; fi",
                        user="65534")
                    if packet.return_code != 0:
                        raise RuntimeError(f"Harbor model mailbox failed: {packet.stderr}")
                    if packet.stdout and packet.stdout.strip():
                        req = json.loads(packet.stdout)
                        from luna import BudgetReached
                        try:
                            answer = await asyncio.to_thread(client.ask, req["messages"], req["choices"],
                                                             editor=req.get("editor") is True,
                                                             notes=req.get("notes") is True)
                            packet = {"answer": answer}
                        except BudgetReached:
                            if req.get("editor") is not True:
                                raise
                            # Editing ends in order: the worker closes its step and the files on disk are graded.
                            packet = {"answer": "", "stop": "budget"}
                            answer = json.dumps({"action": "stop", "reason": "budget"})
                        self.live(req, answer, client)
                        reply.write_text(json.dumps(packet))
                        await environment.upload_file(reply, "/work/model/reply.tmp")
                        published = await environment.exec("chmod 644 /work/model/reply.tmp && mv /work/model/reply.tmp /work/model/reply.json", user="root")
                        if published.return_code != 0:
                            raise RuntimeError("Could not publish model reply in the trial")
                    else:
                        await asyncio.wait({worker}, timeout=0.25)
            return await worker
        finally:
            if not worker.done():
                worker.cancel()
                await asyncio.gather(worker, return_exceptions=True)
            context.n_input_tokens = client.inputs
            context.n_output_tokens = client.outputs
            (self.logs_dir / "luna-usage.json").write_text(json.dumps(client.usage()))
