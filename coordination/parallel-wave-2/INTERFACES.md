# Wave 2 고정 경계

## B가 추가하는 조회 필드

`src/mcp/plans.rs::view`의 supported:true 응답에만 다음 read-only 필드를 추가한다.

```json
{"current_cycle":{"ref":"cycle:C2","state":"closed"}}
```

- ref는 실제 current Cycle의 typed ref. state는 실제 is_closed에 따라 open 또는 closed다.
- 저장·승인 grant·도구 입력·scope/token/digest 의미는 바꾸지 않는다. 기존 current_item은 바인딩으로 남긴다.
- supported:false나 옛 Core에서 필드가 없으면 UI는 열림을 추정하지 않는다. 상태 미확인으로 표시한다.
- B가 서버 projection + UI consumer + 두 상태/부재 fallback 시험을 함께 소유한다.
- C/D/E는 이번 작업에서 이 필드에 의존하지 않는다. 필요한 결합 회귀는 합류 뒤에 더한다.

## C와 E의 시험 파일 소유권

- C: src/context.rs, src/mcp/guidance.rs, src/mcp/workflow.md, tests/guidance_wave1.rs,
  plugins/gil-companion-prototype/workflow.test.mjs, 생성 Skill, 해당 Manual 설명.
- E: tests/common/mod.rs, src/mcp.rs의 cfg(test) 내부, src/visual_migration.rs의 cfg(test) 내부,
  plugins/gil-companion-prototype/skill.test.mjs, 아래 허용 CI.
- E가 common에 helper를 더해도 C/D는 아직 없는 helper를 가정해 import하지 않는다. baseline API로
  독립 시험을 완성한다. common의 bootstrap를 몰래 승인 프로필로/legacy로 바꾸어 기존 검사 의미를
  전역 변경하지 않는다. 명시적 fixture 선택 또는 별도 helper로 나눈다.
- E의 CI가 C/D 수정 전 테스트를 실행해 실패하면 그 정확한 범위를 기록하고 합류 후 재검증한다.

## D의 평가기·Windows 시험

- tools/scenarios/**, scenarios/wave1의 평가 계약/음성 fixture, tests/approved_plan_wire_wave1.rs,
  tests/support_wave1/**는 D 단독 소유다. 기존 runs/ 과거 실행 결과는 덮어쓰지 않는다.
- state 위치를 테스트가 임의 추측하지 않고 기존 제품 레이아웃/환경 계약에서 찾는다. 실제 개인
  LocalAppData에 기록하지 않도록 시험 루트 아래로 격리한다. 승인 capability는 결과 파일에 남기지 않는다.
- 과거 승인, 현재 Chain 승인, 현재 활성 판 승인, item 바인딩은 별개다. 관측이 부족하면 unknown이며
  과거 어딘가의 approve가 있음을 pass 근거로 삼지 않는다.

## 통합 담당자 소유

Project/Chain/Session/approved_plan 저장 의미, schema, capture/restore apply, F 설계 결정,
공통 큐/프롬프트, 고지/배포/릴리즈, 로드맵. 이번 수정으로 A/B 제품 연결 또는 M6-E가 완료된 것은 아니다.
