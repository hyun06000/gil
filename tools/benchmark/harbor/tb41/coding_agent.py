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
        arm = os.environ["TB_ARM"]
        if arm not in {"on", "off"}:
            raise ValueError("Unknown coding benchmark arm")
        modules = ["solver.py", "mailbox_client.py"]
        if arm == "on":
            modules.extend(["participant.py", "checkpoint.py"])
        for name in modules:
            await environment.upload_file(HERE/"coding"/name,"/case/"+name)
        if task == "vf2-speedup-networkx":
            await environment.upload_file(HERE/"coding"/"graph_smoke.py", "/case/graph_smoke.py")
        await environment.upload_file(HERE/"worker.py","/case/worker.py")
        if os.environ["TB_ARM"]=="on":
            await environment.upload_file(os.environ["TB_GIL_BINARY"],"/case/gil")
        else:
            status = await environment.exec(
                "! command -v gil && test ! -e /case/gil && test ! -e /case/participant.py "
                "&& test ! -e /case/checkpoint.py && test ! -e /work/record",
                user="65534",
            )
            if status.return_code:
                raise RuntimeError("GIL components found in the control environment")
            print("Control preflight passed: no gil on PATH or in /case, no participant, checkpoint or project record.", flush=True)

    async def run(self, instruction, environment, context):
        try:
            await super().run(instruction, environment, context)
        except Exception:
            if os.environ["TB_ARM"] == "on":
                # Preserve the Core graph, its world objects and the last adapter request.
                result = await environment.exec(
                    "cd /work && test -d record && tar -czf /tmp/gil-interrupted.tgz record project",
                    user="65534",
                )
                if result.return_code == 0:
                    await environment.download_file("/tmp/gil-interrupted.tgz", self.logs_dir / "gil-interrupted.tgz")
            raise
        finally:
            exists = await environment.exec("test -f /work/model/transcript.jsonl", user="root")
            if exists.return_code == 0:
                await environment.download_file("/work/model/transcript.jsonl", self.logs_dir / "transcript.jsonl")

    async def run_worker(self, environment):
        arm=os.environ["TB_ARM"]; task=os.environ["TB_TASK"]
        assert arm in ("on","off") and task in TASKS
        prep="chmod +x /case/participant.py /case/gil && " if arm=="on" else ""
        return await environment.exec(prep+"chmod 644 /case/task.txt && setpriv --reuid=65534 --regid=65534 --clear-groups --no-new-privs env HOME=/tmp TB_EXTRA_SECONDS=1800 TB_TASK="+task+" python3 /case/worker.py "+arm,user="root",timeout_sec=3590)
