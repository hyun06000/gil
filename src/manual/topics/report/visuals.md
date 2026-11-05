---
id: report/visuals
aliases:
  - visuals
title: 보고서에 시각 자료를 남긴다
summary: Verify 는 실제 결과를 눈으로 확인할 수 있는 PNG 를 첨부하거나, 왜 그림이 필요 없는지 구체적으로 선언한다. Analysis 와 Cycle Report 는 그 자료를 인용한다.
applies_when:
  step_kind: verify
  step_status: open
related:
  - step/verify/close
  - cycle/experiment/close
---

[언제 읽는가]
  Verify 를 닫기 전에 어떤 시각 자료를 남길지 정할 때.
  `gil close` 가 visual_citation_required · visual_declaration_conflict · WeakReason 으로 거절했을 때.
  Analysis 나 Cycle Report 를 닫는데 Verify 가 첨부한 자료를 인용하라고 거절당했을 때.

[시각 자료란]
  실제 세계의 결과를 눈으로 확인할 수 있게 만든 PNG 다. 모델이 그린 상상의 그림이 아니다.
  - 데이터 · 측정값         → 코드로 그린 플롯 (히스토그램 · 추세 · 비교 막대)
  - 구조 · 흐름 · 의존 관계  → 코드로 그린 다이어그램 (graphviz · mermaid 등)
  - UI · 웹 페이지 · 문서 렌더링 → 헤드리스 또는 일반 브라우저로 실제 결과를 띄운 스크린샷
  - 파일 · 설정 변화         → 이전/이후를 나란히 놓은 비교표 이미지 또는 diff 렌더링
  - 빌드 · 테스트 로그       → 결과 표를 그린 이미지 (로그 원문은 evidence_files 로 함께)
  AI 가 생성한 이미지, 손으로 그린 상상도, 결과를 보여 주지 않는 장식 이미지는 시각 자료가 아니다.
  source_kind 는 captured(실제 화면 캡처) · generated(코드가 데이터로 그림) · provided(사용자 제공)
  가운데 사실대로 고른다.

[지금 할 일]
  Verify 가 열린 동안 PNG 를 프로젝트 안에 저장하고, 파일 증거와 함께 typed close 로 닫는다.
  MCP 에서는 gil_help {project_root, topic: "report/visuals"}, CLI 에서는 gil help report/visuals
  (별칭: gil help visuals)로 이 문서를 읽는다. 별도 파일 생성·기록 변경은 없다.

  아래는 PNG 두 개를 새로 첨부하는 gil_close JSON 예다. project_root 와 expected_owner 는
  실제 연결·gil_status 의 열린 Verify 로 바꾼다. C3/S3 는 예시이며 현재 자리라는 뜻이 아니다.
  report 는 실제 관측으로 작성한다. 승인 계획이 있는 프로젝트는 Verify 에서 document 를 그대로
  저장하고 report 에 plan_document: <상대 경로>도 적는다. 요약본은 사용할 수 없다.

```json
{
  "project_root": "/selected/project",
  "report": "execution: 두 자료의 관측값으로 차트를 생성했다\nresult: 저장한 차트와 원자료를 비교했다\nevidence_files: |\n  report/earnings.png\n  report/targets.png\n",
  "expected_owner": {"kind": "step", "ref": "step:C3/S3"},
  "visuals": {
    "attachments": [
      {"key": "earnings", "path": "report/earnings.png", "source_kind": "generated", "source_description": "관측값으로 코드가 그린 첫 차트"},
      {"key": "targets", "path": "report/targets.png", "source_kind": "generated", "source_description": "관측값으로 코드가 그린 둘째 차트"}
    ],
    "uses": [
      {"source": {"kind": "new", "key": "earnings"}, "caption": "첫 관측 차트", "alt": "관측값의 비교", "purpose": "첫 결과 확인"},
      {"source": {"kind": "new", "key": "targets"}, "caption": "둘째 관측 차트", "alt": "비교 대상별 관측값", "purpose": "둘째 결과 확인"}
    ],
    "declaration": {"state": "attached"}
  }
}
```

  uses 는 declaration 안이 아니라 visuals 바로 아래에 둔다. uses 항목에는 key 를 따로 넣지 않는다.
  새 첨부는 source.kind: new 와 source.key 로 연결하며, 각 attachment key 를 정확히 한 번 사용한다.
  attached 선언에는 state 만 넣는다. reason·uses·attachments 를 선언 안에 넣지 않는다.

  이미 확정된 자료를 Analysis 나 Cycle Report 에서 인용할 때는 다음 source 구조를 쓴다.
  owner 는 원래 자료가 확정된 Verify 이며, expected_owner 는 지금 닫을 Analysis/Cycle 이므로 서로 다르다.
  asset_id 와 sha256 은 Verify close receipt 또는 gil_report_visuals 에서 받은 값을 그대로 복사한다.
  아래 1 과 0 반복 hash 는 구조 예시일 뿐 실제 자료 ID/hash 를 대신할 수 없다.

```json
{
  "attachments": [],
  "uses": [
    {"source": {"kind": "stored", "owner": {"kind": "step", "ref": "step:C3/S3"}, "asset_id": "1", "sha256": "0000000000000000000000000000000000000000000000000000000000000000"}, "caption": "앞선 Verify 결과", "alt": "저장된 관측 차트", "purpose": "이번 해석의 근거"}
  ]
}
```

  이것은 visuals 값이다. 비-Verify 인용에는 declaration 을 생략한다(null 도 쓰지 않는다).
  stored 는 파일 경로나 임시 key 를 받지 않는다. 처음 첨부하는 PNG 에 stored 를 쓰지 않는다.

  정말 그림이 필요 없는 검증(문자열 일치 · 종료 코드 · 단일 숫자 비교)이면 not_needed 에
  **무엇이 이미지를 대신하는지** 적는다. "없음" · "불필요" 같은 한마디는 거절된다.

      declaration: {state: "not_needed", reason: "두 파일의 sha256 일치만 비교했고 화면이 없다"}

  만들려 했지만 못 만들었으면 missing 에 이유와 follow_up 을 적는다. 없는 것을 불필요로 바꾸지 않는다.

[불변식]
  format 5 이상의 Verify 는 선언 없이 닫히지 않는다.
  evidence_files 에 이미지 파일이 있으면 not_needed 로 닫히지 않는다 — 첨부하거나 missing 으로 적는다.
  Verify 가 자료를 첨부했으면 그 Cycle 의 Analysis 와 Cycle Report 는 그중 하나 이상을 인용해야 닫힌다.
  인용은 receipt 또는 gil_report_visuals 가 돌려준 owner · asset_id · sha256 을 stored source 로 적고
  attachments 는 빈 배열, declaration 은 생략한다. 거절문이 인용 가능한 자료를 함께 알려 준다.
  닫힌 Report 에 그림을 나중에 더할 수 없다. 빠졌으면 다음 Cycle 의 Verify 에서 새로 만든다.
  첨부 성공은 저장 성공이다. 화면에 보였는지는 Monitor 에서 사람이 확인한 것만 말한다.

[흔한 실패]
  채팅에 결과 숫자를 적고 그림 없이 not_needed: "텍스트만" 으로 닫는 것 — WeakReason 으로 거절된다.
  플롯 PNG 를 evidence_files 에 적고도 not_needed 로 닫는 것 — visual_declaration_conflict 로 거절된다.
  Analysis 를 일반 텍스트 close 로 보내는 것 — Verify 가 첨부했다면 visual_citation_required 로 거절된다.

[관련 주제]
  step/verify/close
  cycle/experiment/close
