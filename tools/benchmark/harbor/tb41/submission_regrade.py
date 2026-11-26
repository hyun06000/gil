"""Grade frozen submission bytes. This adapter has no model client or editor."""
import base64
import gzip
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shlex
import tempfile

from harbor.agents.base import BaseAgent


class FrozenSubmission(BaseAgent):
    @staticmethod
    def name():
        return "graded-transcript-submission"

    def version(self):
        return "1"

    async def setup(self, environment):
        pass

    async def run(self, instruction, environment, context):
        # On this grading-only branch the optional string dispatch field carries the
        # compressed bundle. No transcript, private tests or reference files are uploaded.
        event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text())
        payload = json.loads(gzip.decompress(base64.b64decode(event["inputs"]["unsigned_preview_version"])))
        if payload["source_run"] != 45871560079:
            raise ValueError("Unexpected source measurement")
        task = os.environ["TB_TASK"]
        records = payload["submissions"][task]
        if not records:
            raise ValueError("No submitted files")
        hashes = {}
        with tempfile.TemporaryDirectory() as temp:
            local = Path(temp, "source")
            for name, item in records.items():
                path = PurePosixPath(name)
                if (path.is_absolute() or path.as_posix() != name or ".." in path.parts
                        or name == "." or any(part.startswith(".") for part in path.parts)):
                    raise ValueError("Invalid submission path")
                allowed = {
                    "ks-solver-cpp": name == "solution.cpp" or (name.endswith(".hpp") and name != "oracle.hpp"),
                    "vf2-speedup-networkx": name == "fast_networkx.py" or name.startswith("fast_networkx/"),
                    "react-lead-form": name.startswith("src/") and not name.startswith(("src/spec/", "src/data/"))
                                       and name.endswith((".ts", ".tsx", ".css")),
                }
                data = item["source"].encode()
                if not allowed[task] or hashlib.sha256(data).hexdigest() != item["sha256"]:
                    raise ValueError("Invalid submission bytes")
                local.write_bytes(data)
                dest = "/app/" + name
                result = await environment.exec("mkdir -p -- " + shlex.quote(str(PurePosixPath(dest).parent)), user="root")
                if result.return_code:
                    raise RuntimeError(result.stderr)
                await environment.upload_file(local, dest)
                hashes[name] = item["sha256"]
        (self.logs_dir / "observation.json").write_text(json.dumps({"files": hashes, "source_run": payload["source_run"]}))
