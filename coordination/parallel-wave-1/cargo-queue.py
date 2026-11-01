#!/usr/bin/env python3
"""Local macOS coordination: hold one lock through a build AND its test subprocesses.

Usage: python3 coordination/parallel-wave-1/cargo-queue.py -- cargo test ...
Also wrap a runner that consumes the shared target/debug/gil executable.
Not part of the shipped GIL runtime; no network or dependency installation.
"""
import fcntl
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile


def main():
    def interrupted(_signum, _frame):
        raise KeyboardInterrupt

    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGHUP, interrupted)
    command = sys.argv[1:]
    if command[:1] == ["--"]:
        command = command[1:]
    if not command:
        raise SystemExit("usage: cargo-queue.py -- COMMAND [ARG ...]")
    common = Path(subprocess.check_output(
        ["git", "rev-parse", "--path-format=absolute", "--git-common-dir"], text=True
    ).strip())
    cache = common.parent / "target" / "claude-wave-1"
    cache.mkdir(parents=True, exist_ok=True)
    with (cache / ".queue.lock").open("a+") as lock:
        print("Waiting for the shared build/test slot...", file=sys.stderr, flush=True)
        fcntl.flock(lock, fcntl.LOCK_EX)
        env = os.environ.copy()
        env["CARGO_TARGET_DIR"] = str(cache)
        env["CARGO_BUILD_JOBS"] = "2"
        env["GIL_PARALLEL_TARGET_DIR"] = str(cache)
        # Use the native temporary root; each command gets independent test fixture names.
        with tempfile.TemporaryDirectory(prefix="gil-wave1-") as temp:
            env["TMPDIR"] = temp
            print(f"Build/test slot acquired (target: {cache})", file=sys.stderr, flush=True)
            child = subprocess.Popen(command, env=env, start_new_session=True)
            try:
                code = child.wait()
            except KeyboardInterrupt:
                for sig in (signal.SIGINT, signal.SIGTERM, signal.SIGKILL):
                    try:
                        os.killpg(child.pid, sig)
                    except ProcessLookupError:
                        break
                    try:
                        child.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        continue
                    break
                code = 130
            return code if code >= 0 else 128 - code


if __name__ == "__main__":
    raise SystemExit(main())
