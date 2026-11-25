"""Model-free controls; reference files are mounted in the positive control only."""
import os
from pathlib import Path
from harbor.agents.base import BaseAgent


class Replay(BaseAgent):
    @staticmethod
    def name():
        return "submission-contract-control"

    def version(self):
        return "1"

    async def setup(self, environment):
        pass

    async def run(self, instruction, environment, context):
        await environment.upload_file(Path(__file__).with_name("solver.py"), "/tmp/solver.py")
        rules = "[]"
        if os.environ["TB_ARM"] == "reference":
            files = Path(os.environ["TB_AUDIT_FIXTURES"])
            for name in ("ground_truth_rules.json", "ground_truth_ordering.txt"):
                await environment.upload_file(files/name,"/tmp/"+name)
            rules = "load_rules('/tmp/ground_truth_rules.json','/tmp/ground_truth_ordering.txt')"
        command = ("mkdir -p /work/project && PYTHONPATH=/app/engine:/tmp python -c "
                   + '"from apply import load_rules; from solver import submit; submit(' + rules + ')"')
        result = await environment.exec(command,user="root")
        if result.return_code:
            raise RuntimeError(result.stderr or result.stdout)
