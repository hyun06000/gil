"""Build a searchable, offline viewer for one Harbor transcript directory."""
import html
import json
from pathlib import Path
import sys


def render(root):
    calls = {}
    transitions = []
    for line in (root / "transcript.jsonl").read_text().splitlines():
        event = json.loads(line)
        if event["event"] in ("verify_plan", "verify_stalled", "cycle_restore_started", "cycle_restore_completed"):
            transitions.append(event)
            continue
        calls.setdefault(event["id"], {})[event["event"]] = event

    rows = []
    for number, call in enumerate(calls.values(), 1):
        stage = call["request"]["stage"]
        answer = call.get("answer", {}).get("text", "응답 기록 없음")
        tool = call.get("tool", {})
        result = tool.get("error", tool.get("result", ""))
        fields = [number, stage, tool.get("action"), tool.get("path")]
        if tool.get("action") == "read":
            fields.append(f'offset {tool.get("offset", 0)}')
        if "error" in tool:
            fields.append("오류")
        title = " · ".join(str(field) for field in fields if field)
        rows.append(f"<details><summary>{html.escape(title)}</summary>"
                    f"<h3>모델 응답</h3><pre>{html.escape(answer)}</pre>")
        if messages := call["request"].get("messages"):
            rows.append("<h3>모델 요청</h3><pre>" +
                        html.escape(json.dumps(messages, ensure_ascii=False, indent=2)) + "</pre>")
        if tool.get("note"):
            rows.append(f"<h3>모델 메모</h3><pre>{html.escape(tool['note'])}</pre>")
        if tool:
            rows.append(f"<h3>도구 결과</h3><pre>{html.escape(result)}</pre>")
        rows.append("</details>")

    for event in transitions:
        rows.append("<details><summary>" + html.escape(event["event"]) + "</summary><pre>"
                    + html.escape(json.dumps(event, ensure_ascii=False, indent=2)) + "</pre></details>")

    head = """<!doctype html><html lang="ko"><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Harbor 에이전트 응답 기록</title>
<style>body{font:16px/1.5 system-ui,sans-serif;max-width:960px;margin:auto;padding:24px;color:#172d3a}
details{border-bottom:1px solid #ccd6d9;padding:9px}summary{cursor:pointer}
pre{white-space:pre-wrap;overflow-wrap:anywhere;background:#f1f5f6;padding:12px}
input{width:70%;padding:10px}h3{font-size:1rem}</style>
<h1>에이전트 응답 기록</h1><p>호출을 펼치면 모델 응답과 도구 결과를 볼 수 있습니다.
파일 읽기 결과는 앞 3,500자까지 표시합니다. <a href="transcript.jsonl">원본 기록</a></p>
<label>검색 <input type="search" id="query" placeholder="단계, 파일명, 응답 또는 오류"></label>
<p id="count"></p>"""
    script = """<script>
const rows = [...document.querySelectorAll('details')];
const query = document.getElementById('query');
function filter() {
  let visible = 0;
  const value = query.value.toLowerCase();
  for (const row of rows) {
    row.hidden = !row.textContent.toLowerCase().includes(value);
    if (!row.hidden) visible++;
  }
  document.getElementById('count').textContent = visible + ' / ' + rows.length + '개 호출';
}
query.addEventListener('input', filter);
filter();
</script></html>"""
    (root / "transcript.html").write_text(head + "".join(rows) + script)


if __name__ == "__main__":
    render(Path(sys.argv[1]))
