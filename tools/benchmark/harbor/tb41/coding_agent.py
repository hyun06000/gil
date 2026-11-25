import os
from tb_agent import HERE, SoundChangeAgent

TASKS={"ks-solver-cpp","react-lead-form","vf2-speedup-networkx"}

class CodeAgent(SoundChangeAgent):
    async def setup(self,environment):
        task=os.environ["TB_TASK"]
        assert task in TASKS
        cmd="mkdir -p /case /work/project /work/model && chown -R 65534:65534 /work && chown 65534:65534 /app"
        if task=="react-lead-form":
            cmd+=" && chown -R 65534:65534 /app/src && mkdir -p /app/node_modules/.tmp /app/output && chown 65534:65534 /app/node_modules/.tmp /app/output"
        status=await environment.exec(cmd,user="root")
        if status.return_code: raise RuntimeError(status.stdout or status.stderr)
        for name in ("solver.py","participant.py","mailbox_client.py"):
            await environment.upload_file(HERE/"coding"/name,"/case/"+name)
        await environment.upload_file(HERE/"worker.py","/case/worker.py")
        if os.environ["TB_ARM"]=="on":
            await environment.upload_file(os.environ["TB_GIL_BINARY"],"/case/gil")

    async def run_worker(self, environment):
        arm=os.environ["TB_ARM"]; task=os.environ["TB_TASK"]
        assert arm in ("on","off") and task in TASKS
        prep="chmod +x /case/participant.py /case/gil && " if arm=="on" else ""
        return await environment.exec(prep+"chmod 644 /case/task.txt && setpriv --reuid=65534 --regid=65534 --clear-groups --no-new-privs env HOME=/tmp TB_TASK="+task+" python3 /case/worker.py "+arm,user="root",timeout_sec=1790)
