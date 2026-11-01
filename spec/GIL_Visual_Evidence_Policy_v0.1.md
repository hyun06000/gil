# GIL 시각 증거 정책 v0.1 — 보고서가 결과를 보여 주게 만드는 계약

작성: 2026-11-02. 상태: **Core 계약·지침·Monitor·평가 구현, 합성 시험 통과, 실제 모델 측정과 Host 화면 확인 대기.**
기준: [제품 원칙](GIL_Visual_Report_Proposal_v0.1.md), [작성 경로 계약](GIL_Visual_Report_Authoring_Contract_v0.1.md),
[표시 계약](GIL_Visual_Report_Display_Contract_v0.1.md), [저장 계약](GIL_Visual_Report_Storage_Contract_v0.1.md).

## 1. 문제

시각 보고서의 저장·전송·표시 경로는 T1.2 G1–G5로 완성됐지만, 실사용에서 **사용자가 요구하지 않는 한
Agent 가 시각 자료를 만들지 않았다.** 지침은 "도움이 되면 쓰라"였고, 계약은 Verify 의 선언 하나를 요구할 뿐
`not_needed: "없음"` 같은 한마디로 통과됐다. Analysis·Outcome·Cycle Report 는 Verify 의 그림을 인용하지
않아도 닫혔고, Monitor 는 시각 자료를 Report 본문 아래에 뒀다. 결과적으로 "시각 자료가 첨부된 노드 보고서"는
가능했지만 기본 동작이 아니었다.

## 2. 시각 증거의 정의

시각 증거는 **실제 세계의 결과를 눈으로 확인할 수 있게 만든 정적 PNG** 다.

| 검증이 관측한 것 | 시각 증거 | `source_kind` |
| --- | --- | --- |
| 숫자·측정값·데이터셋 | 실제 데이터로 코드가 그린 플롯 | `generated` |
| 구조·흐름·의존 관계 | 실제 구조에서 생성한 다이어그램 | `generated` |
| 웹 페이지·UI·렌더링된 문서 | 헤드리스 또는 일반 브라우저로 실제 결과를 띄운 스크린샷 | `captured` |
| 파일·설정의 이전/이후 | 비교표 또는 diff 렌더링 이미지 (원본 파일은 `evidence_files`) | `generated` |
| 빌드·테스트 실행 | 결과 표 이미지 (로그 원문은 `evidence_files`) | `generated` |

**AI 가 생성한 그림, 상상도, 장식 이미지는 시각 증거가 아니다.** `source_kind` 는 사실대로 적는다. 이 원칙은
기계적으로 검사할 수 없으므로 계약 문서·도구 설명·schema 설명·workflow·Skill 에 같은 문장으로 적는다.

## 3. 강제 수준과 근거

지침만으로 보장되지 않는 것은 Core 가 거절한다. 거절은 새 쓰기에만 적용되며 과거 기록은 다시 재지 않는다.

| 규칙 | 적용 범위 | 강제 | 거절 코드 | 근거 |
| --- | --- | --- | --- | --- |
| Verify 는 선언(`attached` / `not_needed` + reason / `missing` + reason + follow_up) 없이 닫히지 않는다 | format 5·7 | 기존 | `visual_declaration_required` | G5 계약 |
| `not_needed`/`missing` 의 reason 은 두 단어 이상·여섯 글자 이상이며 상투어(없음·불필요·not needed·n/a…) 전체 일치가 아니다 | format 5·7 새 쓰기 | **신규** | `visual_reason_weak` | 한마디 이유는 "무엇이 이미지를 대신하는지" 말하지 않는다. 내용의 진실성은 판정하지 않고 형식만 잰다 |
| `evidence_files` 에 이미지 파일(png·jpg·gif·webp·bmp·svg·tif)이 있으면 `not_needed` 로 닫히지 않는다 | format 5·7 새 Verify | **신규** | `visual_declaration_conflict` | 이미 만든 그림을 첨부하지 않는 것은 누락이지 불필요가 아니다 |
| Verify 가 자료를 첨부했으면(`attached`, 또는 uses 가 있는 `missing`) 같은 Cycle 의 **Analysis** 와 **Experiment Cycle Report** 는 그중 하나 이상을 `stored` source 로 인용해야 닫힌다 | format 5·7 새 쓰기 | **신규** | `visual_citation_required` | 해석과 인계 문서가 증거를 가리켜야 Monitor 의 그 노드에서 그림이 보이고, 다음 협력자가 Verify 노드를 따로 뒤지지 않는다. 거절문이 인용 가능한 asset_id·sha256 을 함께 준다 |
| Outcome 의 인용 | — | 권장 | — | 판정문은 짧고 인계는 Cycle Report 가 맡는다. 과도한 마찰을 피한다 |
| 어떤 Verify 에 시각 자료가 "필요한가" | — | 지침·평가 | — | 작업 종류의 판단은 기계적으로 할 수 없다. workflow·gil_help·평가 시나리오가 맡는다 |

일반 텍스트 close `{project_root, report}` 와 typed close 가 같은 검사를 지난다. `prepare_visual_close` 가 두
경로의 합류점이다. 거절은 논리 상태·열린 노드·Will·Journey 를 바꾸지 않는다.

## 4. 호환성

- 저장 형식(4·5·6·7)과 객체 저장소는 바뀌지 않는다. 자동 이관은 없다.
- 저장된 기록의 읽기 검증(`registry::codec`)은 그대로다. 이전 판이 받아들인 `not_needed: "필요 없음"` 은
  계속 읽힌다. reason 실질 검사는 Session 의 쓰기 경계(`close_verify_with_images_and_commit`)에서만 한다.
- 공개 도구 schema 는 필드를 추가하지 않았다. `source_kind` 와 `not_needed.reason` 에 설명문만 더했다.
- Monitor View v1 에 선택 필드 `visuals: {state, count}` 를 Step 과 Cycle Report 에 더했다(additive, 없으면 생략).
  이전 UI 는 모르는 필드를 무시하고, 새 UI 는 없으면 배지를 그리지 않는다.
- 이전 빌드에서 Verify 가 자료를 첨부한 채 열린 Cycle 의 Analysis 를 새 빌드가 닫을 때는 인용이 요구된다.
  거절문이 인용 값을 알려 주므로 같은 close 를 한 번 다시 보내면 된다.
- format 4 프로젝트와 기본 빌드는 영향이 없다.

## 5. Agent 가 스스로 만들게 하는 안내

- `src/mcp/workflow.md` 규칙 4 에 "요구 없이도 결과를 보이게 한다" 절과 위 표를 넣었다. 규칙 5 에 인용 의무,
  규칙 6 에 `report/visuals` Topic 을 적었다. Plugin Skill 은 같은 원문을 동기화한다. Host 별 사본은 없다.
- `gil_help topic: report/visuals` (새 bundled Topic, Verify 가 열렸을 때 상태 기반 목록에 포함). 시각 계약
  거절은 Router 가 이 Topic 하나로 안내한다.
- `gil_open`(verify·analysis) 과 `gil_close`(verify·outcome) 의 receipt 안내문이 다음 호출에서 시각 자료를
  만들거나 인용하라고 말한다. `gil_close` 도구 설명과 authoring schema 설명이 같은 원칙을 적는다.

## 6. Monitor

- 노드 상세: 시각 자료 패널이 Report 본문보다 **먼저** 선다. 머리글에 개수(`시각 자료 2개`), `not_needed` 는
  점선 상자에 `시각 자료 없음 — 사유: …`, `missing` 은 강조 테두리에 사유·후속 계획을 드러낸다.
  썸네일은 가로로 흐르고 누르면 기존 원본 크기 보기로 바뀐다(`image-zoom.mjs` 유지).
- Graph 노드: 기록된 선언이 있는 Step 에 배지(첨부 개수, 없음 선언은 `–`). aria-label 에도 같은 말을 넣는다.
- 요약 카드와 진행 패널: `시각 자료 N개 (이 Cycle M개) · 없음 선언 K건 · 미완 J건`.
- 줌·미니맵(`ui/viewport.mjs`)과 업데이트 안내(`src/mcp/update.rs`)는 손대지 않았다. 배지는 `.node.step::after`
  로 그려 좌표·미니맵 계산에 끼어들지 않는다.

## 7. 평가

- `mcp-app/guidance-eval.mjs`: 그림을 요구받지 않은 데이터 분석·웹 검사 Verify 에서 플롯/스크린샷을 만드는 것이
  다음 행동인지(`perform_current_action`), 해시 비교처럼 보여 줄 것이 없으면 구체적 이유의 `not_needed` 로 닫는지,
  Verify 가 첨부한 뒤 Analysis 를 일반 텍스트로 닫으려 할 때 `gil_close_with_visuals` 를 고르는지 5개 케이스를
  더했다.
- `mcp-app/interview-workflow-eval.mjs`: 코드로 PNG 막대 그래프를 그리는 fixture 도구 `save_synthetic_chart`
  (Node zlib 만 사용)를 더하고 `visualAttachedUnprompted`·`noWeakNotNeeded` 판정을 추가했다. 프롬프트는 그림을
  요구하지 않는다.
- `tools/scenarios` 채점기: 절차 항목 `visual_evidence_attached`. data-science·frontend rubric 은
  `visual_evidence_expected: true` 라 모든 Verify 가 `not_needed`/`missing` 이면 fail 이다.
- 모델 평가는 유료이며 이 변경에서 실행하지 않았다. 실행 전 대상 모델·비용을 확인한다.

## 8. 하지 않는 것·한계

- AI 생성 이미지를 기계적으로 가려내지 않는다. `source_kind` 는 작성자 기재다.
- "이 Verify 에 그림이 필요했는가" 를 Core 가 판정하지 않는다. 지침·평가로 측정한다.
- 저장 성공은 화면 표시가 아니다. Codex·Claude Desktop 의 실제 화면은 사용자 확인 항목이다.
- 설치본 공개·자동 병합은 이 변경에 포함하지 않는다.
