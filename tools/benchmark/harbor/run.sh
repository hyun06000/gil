#!/usr/bin/env bash
set -euo pipefail
source_dir=$(cd "$(dirname "$0")" && pwd)
binary=$(realpath "${1:?pass a Linux GIL executable}")
output=$(realpath -m "${2:?pass a new results directory}")
mkdir "$output"
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
cp -R "$source_dir/latency" "$scratch/"
context="$scratch/latency/environment"
cp "$binary" "$context/gil"
revision=91cad51170dc346986eccefdc2dd33a9da36ead9
weights_hash=6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e
curl --fail --location --silent --show-error --retry 3 "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/$revision/qwen2.5-1.5b-instruct-q4_k_m.gguf" -o "$context/model.gguf"
echo "$weights_hash  $context/model.gguf" | sha256sum -c -
PYTHONPATH="$source_dir" harbor run --path "$scratch/latency" --agent-import-path gil_harbor:GilAgent --env docker --jobs-dir "$output" --job-name benchmark
python3 - "$output" <<'PY'
import json, sys
from pathlib import Path
root = Path(sys.argv[1]) / "benchmark"
trial_file, = root.glob("*/result.json")
trial = json.loads(trial_file.read_text())
if trial.get("exception_info") or (trial.get("verifier_result") or {}).get("rewards", {}).get("reward") != 1:
    raise SystemExit(f"Harbor trial failed: {trial.get('exception_info')}, verifier={trial.get('verifier_result')}")
summary = json.loads((trial_file.parent / "agent/gil-result.json").read_text())
print("Harbor reward 1. GIL final verdict:", summary["verdict"])
PY
