"""Harbor host bridge; one shared model configuration and task toolset for both arms."""

import os
from pathlib import Path
import tempfile
from gil_harbor import LunaAgent
from luna import Luna

HERE = Path(__file__).resolve().parent


class LoggedLuna(Luna):
    def ask(self, messages, choices, *, editor=False, notes=False):
        result = super().ask(messages, choices, editor=editor, notes=notes)
        print(
            f"[{os.environ['TB_ARM']}] Luna call {self.calls} | input {self.inputs} | output {self.outputs} | estimated USD {self.usage()['estimated_usd']:.5f}",
            flush=True,
        )
        return result


class SoundChangeAgent(LunaAgent):
    @staticmethod
    def name():
        return "gil-comparison-" + os.environ["TB_ARM"]

    def model_client(self):
        return LoggedLuna(max_calls=None, budget=None, max_output=4096)

    async def setup(self, environment):
        result = await environment.exec(
            "mkdir -p /case /work/project /work/model && chown -R 65534:65534 /work && touch /app/rules.json /app/ordering.txt && chown 65534:65534 /app/rules.json /app/ordering.txt",
            user="root",
        )
        if result.return_code:
            raise RuntimeError(result.stdout or result.stderr)
        for name in ("participant.py", "solver.py", "worker.py"):
            await environment.upload_file(HERE / name, "/case/" + name)
        if os.environ["TB_ARM"] == "on":
            await environment.upload_file(os.environ["TB_GIL_BINARY"], "/case/gil")

    async def run_worker(self, environment):
        arm = os.environ["TB_ARM"]
        assert arm in ("off", "on")
        setup = "chmod +x /case/gil /case/participant.py && " if arm == "on" else ""
        return await environment.exec(
            setup
            + "chmod 644 /case/task.txt && setpriv --reuid=65534 --regid=65534 --clear-groups --no-new-privs python /case/worker.py "
            + arm,
            user="root",
            timeout_sec=1790,
        )

    async def run(self, instruction, environment, context):
        with tempfile.TemporaryDirectory() as tmp:
            task = Path(tmp) / "task.txt"
            task.write_text(instruction)
            await environment.upload_file(task, "/case/task.txt")
        result = await self.execute(environment, context)
        # GIL exit 2 records a completed failed Experiment; Harbor still grades the submitted files.
        allowed = (0, 2) if os.environ["TB_ARM"] == "on" else (0,)
        if result.return_code not in allowed:
            raise RuntimeError(
                f"worker exit {result.return_code}: {result.stdout} {result.stderr}"
            )
        if os.environ["TB_ARM"] == "on":
            await environment.download_file(
                "/work/result.json", self.logs_dir / "gil-result.json"
            )
        await environment.download_file(
            "/work/project/observation.json", self.logs_dir / "observation.json"
        )
