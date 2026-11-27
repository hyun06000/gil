#!/usr/bin/env bash
set -euo pipefail
case "$TB_TASK" in
  react-lead-form|ks-solver-cpp|vf2-speedup-networkx) ;;
  *) exit 2 ;;
esac
agent=coding_agent:CodeAgent
base=$(cd "$(dirname "$0")" && pwd)
root=${RUNNER_TEMP:-/tmp}
work=$(mktemp -d "$root/tb41.XXXXXXXX")
upstream=4070d6cef56a7d20cb5dc98d5e10ada88a6b9440
git -C "$work" init -q
git -C "$work" sparse-checkout set tasks/$TB_TASK
git -C "$work" remote add origin https://github.com/harbor-framework/terminal-bench.git
git -C "$work" fetch -q --depth=1 --filter=blob:none origin "$upstream"
git -C "$work" checkout --quiet --detach FETCH_HEAD
test "$(git -C "$work" rev-parse HEAD)" = "$upstream"
task="$work/tasks/$TB_TASK"
# Both agent environments deny external egress. Use the release's independent verifier unchanged.
# The codex arm is Harbor's own Codex CLI agent: it calls the model from inside the task container,
# so that container keeps the task's own network setting.
if [[ $TB_ARM != codex && $TB_ARM != codex-gil ]]; then
  sed -i '/^\[environment\]$/a network_mode = "no-network"' "$task/task.toml"
fi
output="$root/tb41-$TB_ARM"
mkdir "$output"
export PYTHONPATH="$base:$base/.."
if [[ $TB_TASK == react-lead-form || $TB_ARM == codex-gil ]]; then
  echo 'RUN command -v python3 || (apt-get update -qq && apt-get install -y -qq python3)' >> "$task/environment/Dockerfile"
fi
rc=0
if [[ $TB_ARM == codex ]]; then
  # No GIL, no stand-in agent of ours: the model's own harness, to see what a continuous
  # conversation does on the same task under the same time allowance. There is no USD cap here;
  # the agent timeout is what bounds the trial.
  OPENAI_API_KEY="$GIL_HARBOR_API_KEY" harbor run --path "$task" --agent codex --model "openai/${TB_CODEX_MODEL:-gpt-5.6-luna}" \
    --env docker --agent-timeout-multiplier 0.125 --jobs-dir "$output" --job-name pair || rc=$?
  # Our report reads files only our own agent writes; what it can still read (result, checks) is kept.
  TB_UPSTREAM="$upstream" python3 "$base/coding_report.py" "$output" "$TB_ARM" "$TB_TASK" || echo "coding_report could not summarise the codex trial"
  exit "$rc"
fi
if [[ $TB_ARM == codex-gil ]]; then
  # The same Codex agent with the product's GIL MCP server connected through coding/gil_proxy.py,
  # which keeps the runner tools, answers for the user and takes GIL's files out of /app at the end.
  kit="$work/gil-kit"
  mkdir "$kit"
  cp "$TB_GIL_BINARY" "$kit/gil"
  cp "$base/coding/gil_proxy.py" "$base/coding/gil_mcp.py" "$kit/"
  chmod -R a+rX "$kit"
  printf '{"mcpServers":{"gil":{"command":"python3","args":["/gil/gil_proxy.py","/gil/gil","/app","/logs/agent"]}}}\n' > "$work/gil.mcp.json"
  OPENAI_API_KEY="$GIL_HARBOR_API_KEY" harbor run --path "$task" --agent codex --model "openai/${TB_CODEX_MODEL:-gpt-5.6-luna}" \
    --mcp-config "$work/gil.mcp.json" --mounts "[{\"type\":\"bind\",\"source\":\"$kit\",\"target\":\"/gil\",\"read_only\":true}]" \
    --ak "prompt_template_path=$base/coding/codex_gil_prompt.md.j2" \
    --env docker --agent-timeout-multiplier 0.125 --jobs-dir "$output" --job-name pair || rc=$?
  TB_UPSTREAM="$upstream" python3 "$base/coding_report.py" "$output" "$TB_ARM" "$TB_TASK" || echo "coding_report could not summarise the codex-gil trial"
  exit "$rc"
fi
harbor run --path "$task" --agent "$agent" --env docker --agent-timeout-multiplier 0.125 --jobs-dir "$output" --job-name pair || rc=$?
TB_UPSTREAM="$upstream" python3 "$base/coding_report.py" "$output" "$TB_ARM" "$TB_TASK"
exit "$rc"
