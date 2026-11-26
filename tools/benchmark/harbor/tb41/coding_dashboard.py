"""Build a standalone comparison from the six original Harbor receipts."""
import html
import json
import sys
from pathlib import Path

src, out = map(Path, sys.argv[1:])
out.mkdir(parents=True, exist_ok=True)
records = {}
for file in src.rglob("*.json"):
    row = json.loads(file.read_text())
    if "task" in row and row.get("arm") in ("on", "off"):
        key = (row["task"], row["arm"])
        if key in records:
            raise ValueError(f"duplicate result for {key}")
        records[key] = row

esc = html.escape
names = {"react-lead-form":"React 폼 수정", "vf2-speedup-networkx":"그래프 동형성", "ks-solver-cpp":"C++ 수치해석"}
page = ['''<!doctype html><html lang="ko"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Terminal-Bench · CPU 3과제</title><style>
body{margin:0;background:#f4f5f0;color:#203229;font:16px/1.55 system-ui,sans-serif}main{max-width:960px;margin:48px auto;padding:0 18px}section{margin:24px 0;background:white;padding:20px;border-top:3px solid #24734b}h1{font-size:clamp(32px,6vw,52px);margin:8px 0}h2{margin:0}p,small{color:#536258}.scroll{overflow:auto}table{width:100%;border-collapse:collapse;font-variant-numeric:tabular-nums}td,th{padding:9px;text-align:right;white-space:nowrap;border-bottom:1px solid #ddd}th:first-child{text-align:left}details{margin-top:12px}summary{cursor:pointer}code{overflow-wrap:anywhere}a{color:#175936}</style><main><small>Terminal-Bench 4.1.0 · GPT-5.6 Luna</small><h1>CPU 3과제 비교</h1><p>각 조건 1회 · 동일한 소스 편집 도구 · 조건별 30분 · 호출 횟수 상한 없음</p>''']
for task, name in names.items():
    page.append(f'<section><h2>{name}</h2><small>{task}</small><div class=scroll><table><tr><th>조건</th><th>공식 점수</th><th>확인한 파일</th><th>모델 호출</th><th>실행 시간</th><th>추정 비용</th></tr>')
    details=[]
    for arm, label in (("off","GIL 끔"),("on","GIL 켬")):
        r=records.get((task,arm),{})
        rewards=r.get("rewards") or {}
        score=rewards.get("reward")
        receipt=r.get("receipts",{})
        u=r.get("usage") or {}
        cost=u.get("estimated_usd")
        elapsed=r.get("seconds")
        page.append(f'<tr><th>{label}</th><td>{score if score is not None else "—"}</td><td>{sum(receipt.values())}/{len(receipt)}</td><td>{u.get("calls","—")}</td><td>{f"{elapsed:.1f}초" if elapsed is not None else "—"}</td><td>{f"${cost:.5f}" if cost is not None else "—"}</td></tr>')
        raw=json.dumps({"공식 지표":rewards,"채점 항목":r.get("checks"),"제출 파일 해시 대조":receipt,"실행 오류":r.get("error")},indent=2,ensure_ascii=False)
        details.append(f'<details><summary>{label} 채점·제출 내역</summary><pre style="white-space:pre-wrap">{esc(raw)}</pre></details>')
    page.append("</table></div>"+"".join(details)+"</section>")
page.append('<p>확인한 파일은 모델이 편집한 파일 중 Harbor가 수집한 사본과 SHA-256이 일치한 개수입니다. 점수가 없는 실행은 —로 표시합니다.</p><a href="comparison.json">원본 측정값</a></main></html>')
(out/"index.html").write_text("".join(page))
(out/"comparison.json").write_text(json.dumps(list(records.values()),indent=2,ensure_ascii=False))
