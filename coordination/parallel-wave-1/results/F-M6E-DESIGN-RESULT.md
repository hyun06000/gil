# F — M6-E 설계 결과

최종 상태: **설계 제안 완료, 결정 및 Core 구현 미완료.** 생산 코드·schema·CLI/MCP 도구·기존 상태 파일·설치본은 그대로다.

## 산출물
- `spec/proposals/M6E_Open_Step_Interruption_v0.1.md` — 접근 3개 비교·추천안(C: 좁혀서 닫기 + `conclusion`)·전이 계약·충돌 목록·구현 단계·결정 쟁점 3개.
- `design-tests/m6e/transitions.json` — 전이 16행(전제·before/after·실패/재시작·증거·human input·금지), 허용 행렬, 거절 코드 54개.
- `design-tests/m6e/model.mjs` — 표를 읽어 판정하는 순수 모델(GIL Core 아님).
- `design-tests/m6e/fixtures/*.json` — 6개 사례 + 운영 경로 + 금지 12묶음.
- `design-tests/m6e/m6e.test.mjs` — Node 표준 라이브러리만 사용.

## 검증
```sh
node --test design-tests/m6e/m6e.test.mjs   # tests 25 · pass 25 · fail 0 (macOS, Node 26.5.0)
```
검사 내용: 표의 결정성·참조 무결성, 모든 전이 행과 도달 가능한 모든 거절 코드의 fixture 커버리지(모델에서 도달 불가한 4개 코드는 표에 이유를 명시), 각 사건 뒤 불변식(열린 Cycle/Step ≤1, Will ≤1, Journey revision = Done Will 수, Closed Cycle 의 Exit, pending 과 열린 Node 배타, 재시도 provenance), **닫힌 기록 불변**, **거절 시 상태 불변**, 모델 결정성, 「확장은 not_reached 행 하나뿐」.
실행하지 않은 것: Rust build/test(소유 파일이 `src/**` 를 건드리지 않아 불필요하며 cargo 큐를 쓰지 않았다), AGENTS.md 의 JS/패키징 검사(변경 없음), 실제 Host/Luna 시험.

## 변경 파일
신규만: 위 산출물. `src/**`, `spec/gil-spec.yaml`, 기존 명세, 로드맵, 다른 세션 파일은 수정하지 않았다.

## 다른 세션/통합 담당자에게 넘기는 인터페이스
- `gil-spec.yaml`: Outcome/Cycle Report 선택 필드 `conclusion ∈ {judged,not_reached}`(기본 judged), `blocker`(not_reached 시 필수), `allowed_values_when` 에 `failure/not_reached → revisit|open_child`.
- `approved_plan.rs`: `Execution{attempt: u32 (기본 1), retry_of: Option<String>}`, `pending()` 을 「judged 또는 열린 시도가 없는 항목」으로, 같은 판 not_reached 상한(제안 2).
- 보조 동작 `revise_will`(target 고정·Journey 불변) — Session/MCP 도구 추가는 통합 단계.
- D 세션(안내 문구): 중단 시 「Will 좁히기 → 중단 관측 문서 → 사실대로 close → not_reached」 순서.

## 발견한 결함·막힌 조건(범위 밖, 수정하지 않음)
- 비-Verify Step 에서 폴더가 dirty 가 되면 close 가 거절되고, product-safety 빌드에서는 `restore` 도 거절되어 사용자가 수동 복구해야 한다. M6-E 가 풀지 않는다.
- product-safety 아래에서 judged failure 의 `revisit` 은 기록만 되고 실행이 막혀 진행 불가(case4b). D1 을 불허하면 not_reached 도 같은 상태가 된다.

## 결정할 사항
D1 open_child 허용 여부 · D2 pending 규칙과 상한 2회 · D3 `revise_will` 공개 여부 (제안서 §7).
