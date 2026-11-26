from pathlib import Path
import os
import sys
import time
from solver import WORK, solve

if os.isatty(0) or os.environ.get("DISPLAY") or os.environ.get("GIL_HARBOR_API_KEY"):
    raise RuntimeError("Expected a noninteractive trial with host-only credentials")
extra_seconds = int(os.environ.get("TB_EXTRA_SECONDS", "0"))
Path("/work/deadline").write_text(str(time.time() + 1735 + extra_seconds))
if sys.argv[1] == "off":
    solve(Path("/case/task.txt").read_text())
else:
    from checkpoint import seed_submission
    seed_submission()
    os.environ["GIL_TB41_CYCLE_RECOVERY"] = "1"
    os.environ["GIL_BENCHMARK_VERIFY_SECONDS"] = str(1750 + extra_seconds)
    with open("/work/result.json", "w") as out:
        os.dup2(out.fileno(), 1)
    os.execv(
        "/case/gil",
        [
            "gil",
            "benchmark",
            "--in-container",
            "/case/task.txt",
            "/case/participant.py",
            str(WORK),
            "/work/record",
        ],
    )
