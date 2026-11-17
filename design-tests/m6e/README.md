# design-tests/m6e

> **2026-11-17 적용 범위:** 이 자료의 기존 사례·검사 결과는 당시 제품 계약 또는 미채택 설계
> 후보의 기록이다. 최신 합의는 [승인 계획 계약](../../spec/GIL_Approved_Plan_v0.1.md)의 구현 이행 절을
> 따른다. 안정된 요구 아래 여러 예/아니오 Define, 관측 기반 선택, 실패 시 무변경 세계에서도
> 명시적 전체 복원, 질문별 답·한계 인계를 새로 검증해야 한다. 현재 시험 통과를 새 계약의 구현
> 또는 의미 평가 완료로 주장하지 않는다. 과거 기대값을 새 설계에 맞았던 것처럼 소급 수정하지 않는다.


M6-E 제안([spec/proposals/M6E_Open_Step_Interruption_v0.1.md](../../spec/proposals/M6E_Open_Step_Interruption_v0.1.md))의
전이표가 서로 모순되지 않는지 확인하는 **순수 설계 모델**이다. GIL Core 구현이 아니며 파일·YAML·잠금·Chain 컨테이너·Monitor 를 다루지 않는다.
여기서 통과했다는 것은 표가 자기모순이 없다는 뜻이고, 어떤 Rust 구현이 존재하거나 이렇게 동작한다는 뜻이 아니다.

```sh
node --test design-tests/m6e/m6e.test.mjs
```

- `transitions.json` — 전이표·허용 행렬·거절 코드. 모델이 이 표를 읽어 판정한다(단일 진실원).
- `model.mjs` — 사건을 상태에 적용하는 순수 함수와 불변식 검사.
- `fixtures/case*.json` — 설계 사례 1~6, `operational_paths.json`, `forbidden.json`(막혀야 하는 사례).
- `m6e.test.mjs` — Node 표준 라이브러리만 사용. 모든 전이 행·도달 가능한 거절 코드의 커버리지, 닫힌 기록 불변, 거절 시 상태 불변을 함께 검사한다.

fixture 의 사람 응답·승인은 합성 입력이며 실제 사람이 승인했다는 증거가 아니다.
