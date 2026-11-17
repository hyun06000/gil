# 승인 항목 분할·대체 독립 설계 시험

[결정안](../../spec/proposals/Approved_Item_Split_Replacement_Decision_v0.1.md)의
**미승인 후보 R + 보수적 직렬 순서 + 현재의 복구 경계**만 검증한다. 제품 구현·계약 승인이 아니다.
현상 유지 S와 부모 집계 H, `design-tests/m6e`의 다른 미채택 전이를 가져오지 않는다.

```sh
node --test design-tests/approved-item-split/*.test.mjs
```

Node 표준 라이브러리만 사용한다. GIL 실행·설치·프로젝트 생성이나 파일 복원은 하지 않는다.

- `model.mjs`: 순수 복사 전이, 제안/응답/활성 판, 불변 실행 바인딩, 새 관계와 유효 큐,
  기존 실패 복구 경계, 후속 순서, JSON 재생 비교와 보고용 투영.
- `model.test.mjs`: A–G와 부정 사례. 성공 전이마다 이전 제안 원문·확정 receipt·바인딩·닫힌
  결과의 prefix 불변을 검사하고, 거절 시 입력 상태 전체가 그대로인지 검사한다.
  모든 성공 경계에서 JSON 재개를 검사한다. 8개 제한된 사건 조합도 검사한다(전체 상태공간 증명 아님).

모델 이름/필드는 제품 저장 schema나 명령 이름이 아니다. `initial()`은 이미 승인·활성화된
합성 r1 `[A,B]`에서 시작한다. `interview`는 열린 Synthesis까지의 유효한 Interview를 추상화하고,
`activate`는 일치하는 Synthesis/Outcome/성공 Interview의 close 완료를 추상화한다.
`recovered`는 기존 M4-R 승인·보존·revisit이 밖에서 완료됐다는 전제이며 실제 복구를 구현하지 않는다.
실패 직후 `interview`는 거절한다. 같은 항목 retry의 계보는 이 제한된 직렬 경로의 표식으로만 검사한다.

`fullCoverage`는 사람이 검토한 전체 범위 충족 여부를 주입한 합성 oracle이다. 모델이 자연어를
판정하지 않는다. `freshObservation`/`reuse`는 관측과 출처의 추상화이며 파일·SHA 검증이 아니다.
계획 scope/Synthesis도 문자열로 축약했고 실제 `basis_refs`, grammar, snapshot, Will/Journey,
capability·사용자 버튼·프로젝트 잠금·파일 내구성·YAML decoder는 모델링하지 않는다.

`journal`과 `restart`는 사건 재생 결과와 저장된 모델 상태를 비교하는 시험 장치다. 제품에 event log를
추가하는 제안이나 악의적 로컬 변조 방어가 아니다. 사건과 상태를 함께 다시 쓰는 공격을 방지하지 않는다.
모델의 SHA-256은 정렬 JSON을 묶을 뿐 Rust의 기존 직렬화/digest 호환 증거가 아니다.
종료 summary는 원래 verdict와 대체 관계를 분리하는 표시만 검사하며 Chain close를 구현하지 않는다.

2026-11-17: 27 tests 통과, 0 실패. 제품 Core/호환성/파일 보존/실제 인간 승인 시험은 미실행이다.
