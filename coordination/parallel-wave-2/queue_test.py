"""No Cargo/network: prove worktree cache separation and shared serialization."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

QUEUE = Path(__file__).with_name("cargo-queue.py").resolve()

class QueueTest(unittest.TestCase):
    def test_worktrees_are_isolated_but_commands_serialize(self):
        with tempfile.TemporaryDirectory(prefix="gil-queue-test-") as temp:
            root = Path(temp)
            repo = root / "repo"
            repo.mkdir()
            def git(*args):
                subprocess.run(["git", "-C", str(repo), *args], check=True,
                               stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
            git("init")
            git("-c", "user.name=Queue Test", "-c", "user.email=queue@example.invalid",
                "-c", "commit.gpgsign=false", "commit", "--allow-empty", "-m", "fixture")
            second = root / "second"
            git("worktree", "add", "-b", "second", str(second))
            sentinel = root / "exclusive"
            program = (
                "import json,os,pathlib,time; "
                f"p=pathlib.Path({str(sentinel)!r}); "
                "p.open('x').close(); time.sleep(.2); p.unlink(); "
                "print(json.dumps({k:os.environ[k] for k in "
                "['CARGO_TARGET_DIR','GIL_PARALLEL_TARGET_DIR','TMPDIR','CARGO_INCREMENTAL',"
                "'CARGO_PROFILE_DEV_DEBUG','CARGO_PROFILE_TEST_DEBUG','CARGO_BUILD_JOBS']}))"
            )
            def start(cwd, code=program):
                return subprocess.Popen([sys.executable, str(QUEUE), "--", sys.executable, "-c", code],
                    cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            def finish(p, expected=0):
                out, err = p.communicate(timeout=30)
                self.assertEqual(p.returncode, expected, err)
                return json.loads(out)
            p1, p2 = start(repo), start(second)
            a, b = finish(p1), finish(p2)
            self.assertNotEqual(a["CARGO_TARGET_DIR"], b["CARGO_TARGET_DIR"])
            self.assertEqual(a["CARGO_TARGET_DIR"], finish(start(repo))["CARGO_TARGET_DIR"])
            for result in (a, b):
                self.assertEqual(result["CARGO_TARGET_DIR"], result["GIL_PARALLEL_TARGET_DIR"])
                self.assertFalse(Path(result["TMPDIR"]).exists())
                self.assertEqual(result["CARGO_INCREMENTAL"], "0")
                self.assertEqual(result["CARGO_BUILD_JOBS"], "2")
                self.assertEqual(result["CARGO_PROFILE_DEV_DEBUG"], "0")
                self.assertEqual(result["CARGO_PROFILE_TEST_DEBUG"], "0")
            failed = finish(start(repo, "import os,json,sys; print(json.dumps({'temp':os.environ['TMPDIR']})); sys.exit(7)"), 7)
            self.assertFalse(Path(failed["temp"]).exists())

if __name__ == "__main__":
    unittest.main()
