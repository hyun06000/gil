#!/usr/bin/env bash
# Build a test image from the Linux binary and the synthetic participant only.
set -euo pipefail
context=$(mktemp -d)
trap 'rm -rf "$context"' EXIT
cp "${1:?path to Linux gil executable}" "$context/gil"
cp tools/benchmark/smoke-adapter.py "$context/adapter"
cp tools/benchmark/probe-display.py "$context/probe.py"
cat >"$context/Dockerfile" <<'DOCKERFILE'
FROM python:3.13-slim-trixie
COPY gil /usr/local/bin/gil
COPY adapter /case/adapter
COPY probe.py /case/probe.py
COPY *.json /case/
DOCKERFILE
python3 - "$context" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
base = dict(goal="Sum the integers and compare to 15", numbers=[3, 5, 7], expected=15)
for name, change in [('pass', {}), ('fail', {'expected': 4}), ('revision', {'revise':True}), ('human', {'decision': 'yes'}), ('empty', {'evidence':False}), ('early', {'early_write':True})]:
    (root / (name+'.json')).write_text(json.dumps(dict(base, **change)))
PY
img=$(docker build -q "$context")
docker run --rm --read-only --network none --user 65534:65534 --cap-drop ALL --security-opt no-new-privileges --tmpfs /tmp:rw,mode=1777 "$img" python /case/probe.py
for case in pass fail revision human empty early; do
  code=0
  "$1" benchmark "$img" "/case/$case.json" /case/adapter >"$context/result" 2>"$context/error" || code=$?
  case "$case" in
    pass|fail|revision)
      expected=0
      if [[ $case == fail ]]; then expected=2; fi
      if [[ $code != "$expected" ]]; then cat "$context/error"; exit 1; fi
      python3 - "$context/result" "$case" <<'PY'
import json, sys
r=json.load(open(sys.argv[1]))
assert r['authority']=='benchmark_adapter'
assert r['verdict']==('success' if sys.argv[2]!='fail' else 'failure')
interview, experiment = r['history']
decisions = [s['report']['approved'] for s in interview['steps'] if s['kind']=='synthesis']
assert decisions == (['benchmark_revise','benchmark_accept'] if sys.argv[2]=='revision' else ['benchmark_accept'])
verify = next(s for s in experiment['steps'] if s['kind']=='verify')['report']
assert verify['result']=='15' and verify['evidence_files']=='observation.txt'
assert experiment['report']['verdict']==r['verdict']
PY
      ;;
    early)
      test "$code" = 1
      grep -q 'workspace changed outside an open Verify' "$context/error"
      ;;
    human|empty)
      test "$code" = 1
      if ! grep -q 'turn limit reached' "$context/error"; then cat "$context/error"; exit 1; fi
      if [[ $case == human ]]; then grep -q approved "$context/error"; else grep -q verify_evidence_required "$context/error"; fi
      ;;
  esac
  echo "Passed benchmark case: $case"
done
