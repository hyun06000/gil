#!/usr/bin/env python3
"""Compatibility entry point: Wave 1 shared artifacts were unsafe across worktrees."""
from pathlib import Path
import runpy

if __name__ == "__main__":
    runpy.run_path(str(Path(__file__).resolve().parents[1] / "parallel-wave-2" / "cargo-queue.py"), run_name="__main__")
