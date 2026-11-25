"""Render a self-contained Korean comparison report from paired Harbor results."""

import html
import json
import sys
from pathlib import Path

src, out = map(Path, sys.argv[1:])
out.mkdir(parents=True, exist_ok=True)


def arm(name):
    paths = list(src.rglob(name + ".json"))
    return (
        json.loads(paths[0].read_text())
        if len(paths) == 1
        else {"arm": name, "error": "결과 파일 없음"}
    )


a, b = arm("off"), arm("on")


def metric(data, key):
    if key == "rules" and "rule_count" in data.get("submission", {}):
        return data["submission"]["rule_count"]
    if key == "score":
        rewards = data.get("rewards", {})
        return rewards.get("reward", rewards.get("reward_score"))
    if key == "training":
        obs = data.get("training", {})
        return (
            obs["training_exact"] / obs["training_total"]
            if obs.get("training_total")
            else None
        )
    if key in ("rules", "solver_calls"):
        return data.get("training", {}).get(key)
    if key == "time":
        return data.get("elapsed_seconds")
    return data.get("usage", {}).get(key)


def fmt(key, v, delta=False):
    if v is None:
        return "—"
    sign = "+" if delta and v > 0 else ""
    if key == "training":
        return f"{sign}{v * 100:.2f}" + (" %p" if delta else "%")
    if key == "score":
        return f"{sign}{v:.4f}"
    if key == "time":
        return f"{sign}{v:.1f}초"
    if key == "estimated_usd":
        return f"{sign}${v:.5f}"
    return f"{sign}{v:,}"


rows = []
for k, label in [
    ("score", "공식 검증 점수"),
    ("training", "훈련 예시 정확도"),
    ("rules", "제출 규칙 수"),
    ("solver_calls", "풀이 단계 모델 호출"),
    ("time", "에이전트 실행 시간"),
    ("calls", "모델 호출"),
    ("input_tokens", "입력 토큰"),
    ("output_tokens", "출력 토큰"),
    ("estimated_usd", "추정 API 비용"),
]:
    x, y = metric(a, k), metric(b, k)
    rows.append(
        f"<tr><th>{label}</th><td>{fmt(k, x)}</td><td>{fmt(k, y)}</td><td>{fmt(k, y - x, True) if x is not None and y is not None else '—'}</td></tr>"
    )


def esc(v):
    return html.escape(str(v))


def card(data, title):
    err = data.get("error")
    rewards = data.get("rewards", {})
    score = metric(data, "score")
    raw = esc(json.dumps(rewards, ensure_ascii=False))
    return f"<article><h2>{title}</h2><strong>{fmt('score', score)}</strong><p>{esc(err) if err else '공식 채점 완료' if rewards else '채점 기록 없음'}</p><details><summary>검증기 지표</summary><code>{raw}</code></details></article>"


steps = " → ".join(b.get("steps", []))
records = json.dumps({"off": a, "on": b}, indent=2, ensure_ascii=False)
(out / "comparison.json").write_text(records)
page = """<!doctype html><html lang="ko"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Terminal-Bench 4.1 · GIL 비교</title><style>
*{box-sizing:border-box}body{margin:0;background:#f4f5f0;color:#202c26;font:16px/1.55 system-ui,sans-serif}main{max-width:900px;margin:auto;padding:4rem 1.25rem}h1{font-size:clamp(2rem,6vw,3.6rem);letter-spacing:-.05em;margin:.4rem 0}h2{margin:0;font-size:1rem}p{margin:.5rem 0 1rem}header small{font-weight:700;color:#475f51}section{display:grid;grid-template-columns:1fr 1fr;gap:1rem;margin:2rem 0}article{border-top:4px solid #707c79;background:white;padding:1.2rem}article+article{border-color:#16714e}strong{display:block;font:600 2.6rem/1.3 ui-monospace,monospace}.scroll{overflow:auto}table{width:100%;border-collapse:collapse;background:white;font-variant-numeric:tabular-nums}td,th{padding:.8rem;text-align:right;border-bottom:1px solid #dae1da;white-space:nowrap}th:first-child{text-align:left}thead{background:#e3ebe2}details{margin:.8rem 0}summary{cursor:pointer}footer{margin-top:1.5rem;color:#425648}a{color:#125736}code{overflow-wrap:anywhere} @media(max-width:550px){main{padding-top:2rem}section{gap:.5rem}article{padding:.7rem}td,th{padding:.55rem}}
</style><main><header><small>Terminal-Bench 4.1.0 · GPT-5.6 Luna</small><h1>GIL 실행 비교</h1><p>sound-change-cascade · 훈련 예시에서 순서가 있는 음운 변화 규칙 추론</p></header>"""
page += (
    "<section>"
    + card(a, "GIL 끔")
    + card(b, "GIL 켬")
    + "</section><div class=scroll><table><thead><tr><th>지표</th><th>GIL 끔</th><th>GIL 켬</th><th>차이 (켬 − 끔)</th></tr></thead><tbody>"
    + "".join(rows)
    + "</tbody></table></div>"
)
page += "<footer><p>CPU 과제 1개 · 조건별 1회 · 동일한 모델과 훈련 도구 · 30분 시간 예산 · 모델 호출 횟수 상한 없음. 공식 검증 점수는 분리된 채점기에서 계산하며 훈련 정확도는 에이전트가 실행 중 관측한 값입니다.</p>"
if any(r.get("training", {}).get("rules") == 0 for r in (a, b)):
    page += "<p><b>빈 제출이 포함된 실행입니다.</b> 공식 채점기는 규칙 적용 순서가 비어 있으면 0점을 줍니다. 이 결과만으로 모델의 규칙 추론 능력을 비교하기 어렵습니다.</p>"
for data, label in ((a, "GIL 끔"), (b, "GIL 켬")):
    receipt = data.get("submission", {})
    if receipt.get("checks"):
        checks = "".join(
            f"<li>{esc(name)} · {esc(status)}</li>"
            for name, status in receipt["checks"].items()
        )
        page += f"<details><summary>{label} 제출 파일과 채점 항목</summary><p>규칙 {esc(receipt.get('rule_count'))}개 · 순서 {esc(receipt.get('ordering_count'))}개</p><ul>{checks}</ul></details>"
if steps:
    page += (
        "<details><summary>GIL 진행 단계</summary><p>" + esc(steps) + "</p></details>"
    )
page += '<a href="comparison.json">측정값 JSON</a>'
run = a.get("run_id") or b.get("run_id")
if run and str(run).isdigit():
    page += f' · <a href="https://github.com/hyun06000/gil/actions/runs/{run}">실행 로그</a>'
(out / "index.html").write_text(page + "</footer></main></html>")
