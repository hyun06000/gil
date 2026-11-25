"""Harbor owns the trial. GIL owns the Interview and Experiment graph."""
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
        result = await environment.exec(
            "chmod 644 /case/task.txt && "
            "setpriv --reuid=65534 --regid=65534 --clear-groups --no-new-privs "
            "sh -c 'mkdir /work/project && gil benchmark --in-container "
            "/case/task.txt /case/adapter.py /work/project /work/record > /work/result.json'",
            timeout_sec=850, user="root",
        )
        if result.return_code != 0:
            raise RuntimeError(f"GIL exited {result.return_code}: {result.stderr} {result.stdout}")
        await environment.download_file("/work/result.json", self.logs_dir / "gil-result.json")
