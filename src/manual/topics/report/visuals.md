---
id: report/visuals
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

      gil_close {
        project_root, report (evidence_files 포함),
        expected_owner: {kind: "step", ref: "<열린 Verify>"},
        visuals: {
          attachments: [{key: "plot", path: "evidence/plot.png",
                         source_kind: "generated", source_description: "matplotlib 로 그린 그룹 평균"}],
          uses: [{source: {kind: "new", key: "plot"},
                  caption: "그룹별 평균 키", alt: "B 가 가장 높은 세 막대", purpose: "가설 판정 근거"}],
          declaration: {state: "attached"}
        }
      }

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
