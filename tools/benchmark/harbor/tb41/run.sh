#!/usr/bin/env bash
set -euo pipefail
: "${GIL_HARBOR_API_KEY:?missing model credential}" "${TB_ARM:?choose on or off}"
base=$(cd "$(dirname "$0")" && pwd)
root=${RUNNER_TEMP:-/tmp}
work=$(mktemp -d "$root/tb41.XXXXXXXX")
upstream=4070d6cef56a7d20cb5dc98d5e10ada88a6b9440
git -C "$work" init -q
git -C "$work" sparse-checkout set tasks/sound-change-cascade
git -C "$work" remote add origin https://github.com/harbor-framework/terminal-bench.git
git -C "$work" fetch -q --depth=1 --filter=blob:none origin "$upstream"
git -C "$work" checkout --quiet --detach FETCH_HEAD
test "$(git -C "$work" rev-parse HEAD)" = "$upstream"
task="$work/tasks/sound-change-cascade"
# Both agent environments deny external egress. Use the release's independent verifier unchanged.
sed -i '/^\[environment\]$/a network_mode = "no-network"' "$task/task.toml"
output="$root/tb41-$TB_ARM"
mkdir "$output"
export PYTHONPATH="$base:$base/.."
rc=0
harbor run --path "$task" --agent-import-path tb_agent:SoundChangeAgent --env docker --agent-timeout-multiplier 0.0625 --jobs-dir "$output" --job-name pair || rc=$?
python3 "$base/report.py" "$output" "$TB_ARM"
exit "$rc"
