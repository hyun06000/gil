# PR 초안

제목: `Add synthetic evaluation for result-first Chain closing and handoff`

## 목적과 범위 / Purpose and scope

기존 Chain Core 시험과 W18은 종료 승인과 다음 Chain의 권한 경계를 검사하지만,
사용자에게 제시한 결과·한계가 종료·인계 기록과 일치하는지는 평가하지 않는다.
문서·분석·화면·코드·파일 없는 탐색·부분 실패·접근 실패·검증 후 변경/삭제·승인 전후 연결 끊김·
새 Chain 인계의 합성 10사례와 좋은/실패 예시를 추가한다.

공개 메시지 인용, 사건 순서, 승인 판/digest/scope, Verify 파일 bytes, 재개·새 인터뷰 상태를
검사하는 오프라인 Node 도구를 추가한다. 이해도·한계·인계 충실성은 별도 의미 평가이며,
합성 App 승인·도구 성공을 인간 승인·화면 표시·열람 성공으로 세지 않는다.
기존 이벤트 형식과 라이브러리를 읽기 전용으로 재사용하고 not_run/unknown 예시·실제 반복 절차를 제공한다.

## 검증 / Verification

- 새 JS 자체시험 49 passed; 기존 Interview/scenarios JS 64 passed.
- 사례 검사·미실행/unknown 결과 검사 정상, 잘못된 성공 주장은 예상 종료 1로 거절.
- 입력 오류·근거 누락·판정 위조·출력 덮어쓰기 경계를 자체시험에서 실제 실행 확인.
- `git diff --check` 통과.
- Core 시험 추가 없음. Cargo/native stdio·실제 모델·Host·사용자 필드 테스트는 not_run.

기준: `bcefbc4` (PR #142 통합 후). 변경은 `scenarios/chain-closing/`와
`tools/chain-closing-eval/`에 한정한다. 제품·UI·workflow·규범·로드맵·CI 변경이 없다.
이 검증의 성공은 평가 도구의 일관성을 뜻하며 제품 사용자 경험의 합격을 뜻하지 않는다.

## 남은 점 / Limitations — 기존 시험의 제품 프로필 제한

- `tests/chain_closing.rs`와 `tests/chain_handoff.rs`는 `chain-core`로 활성화되며,
  `tests/common/mod.rs`의 `bootstrap_from`·`closing_interview`·`closing_ready`와
  맨 `approved: yes` 합성 fixture를 재사용한다. 이 fixture는 승인 계획이 없는
  기본/Chain 계약용이다. 새 `approved-plan` 프로젝트에서는 `bootstrap_from`이
  이를 명시적으로 거절한다. 따라서 `chain-core` 검사 결과를
  `approved-plan,visual-authoring,product-safety` 제품 프로필의 통과로 확대하지 않는다.
- 제품 프로필의 실제 승인 입력은 propose → App decide → close_report 흐름으로
  별도 검증해야 한다. 기존 `tests/approved_plan_wire_wave1.rs`, `tests/support_wave1`,
  `tools/scenarios/wire-runner.mjs`(W18 포함)를 재사용할 수 있으나 이 PR 작업에서는
  native Core/stdio를 실행하지 않았다. `cargo build --all-targets` 성공만으로 해당
  프로필의 모든 통합 시험이 실행·통과했다고 판단해서도 안 된다.
- 새 오프라인 도구는 CI에 연결하지 않았다. 위 JS 결과는 로컬 실행 증거이며,
  PR CI·실제 모델·Host·물리적 인간 승인은 별도다. scope와 fixture를 자동으로
  바꾸거나 기존 시험을 skip하는 수정은 포함하지 않는다.

## 확인 / Checklist

- [x] CONTRIBUTING과 CODE_OF_CONDUCT를 읽었습니다.
- [x] 관련 없는 변경과 전체 재포맷을 섞지 않았습니다.
- [x] 평가 도구의 시험·문서를 추가했고 제품 프로필의 미실행 범위를 설명했습니다.
- [x] 실제 사용자 기록·자격 증명·생성 바이너리·cache를 포함하지 않았습니다.
- [x] 새 자료는 합성 fixture와 자체 작성 문서/코드이며 추가 제3자 자료·고지 대상이 없습니다.
- [x] 사용자 파일·설치·업데이트·restore 동작은 변경하지 않습니다.
