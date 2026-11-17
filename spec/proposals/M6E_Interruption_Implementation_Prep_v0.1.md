# M6-E 구현 준비 v0.1 — 현재 계약 안의 정직한 중단 경로와 남은 결정

> **2026-11-17 최종 합의에 따른 이전 후보 문서.** 아래 후보·권장안·구현 순서를 현재 작업 지시로
> 사용하지 않는다. 최신 설계는 [승인 계획 계약](../GIL_Approved_Plan_v0.1.md)의 후속 합의와
> 구현 이행 절, [Cycle Model](../GIL_Cycle_Model_v0.1.md) §7·복원·종료를 따른다.
> 새 Chain의 안정된 승인 요구 아래 여러 예/아니오 Define으로 탐색한다. 실패하면 변경 파일이
> 없어도 명시적 revisit을 거쳐 세계 전체를 복원하고 배움은 보존한다. 반성은 복원점과 다음 방향의
> 근거를 마련한다. 부분 세계 유지·실패 후 직접 진행·분할 관계 R·반복 명세 수정은 기본 경로로
> 채택하지 않았다. 이 문서의 기존 모델 시험은 그 후보만 검사하며 새 설계의 구현 증거가 아니다.


상태: **제안 + 구현 준비 기록(2026-11-16).** 저장 형식·Cycle Grammar·전이 의미·승인 계약을 바꾸지 않는다.
규범은 여전히 [Cycle Model §8 「완료할 수 없는 작업의 중단과 다음 방향」](../GIL_Cycle_Model_v0.1.md#완료할-수-없는-작업의-중단과-다음-방향),
[Approved Plan 개발 계약](../GIL_Approved_Plan_v0.1.md), [M4-R 승인 복구](../GIL_Revisit_Recovery_v0.1.md)다.
[기존 M6-E 설계안](M6E_Open_Step_Interruption_v0.1.md)의 `conclusion`·`revise_will`·`open_child` 확장·재시도 상한은
그대로 **미채택 후보**이며 이 문서가 결정하지 않는다. 이 문서는 (1) 오늘 계약으로 실제 가능한 중단 경로를 코드 근거와
함께 적고, (2) 이번 PR이 그 경로 안에서 바꾼 안내·시험을 기록하고, (3) 사용자 결정이 필요한 쟁점을 근거·대안과 함께 남긴다.

## 1. 오늘 계약에서 열린 Experiment를 끝낼 수 없을 때 실제로 가는 길

승인 계획 프로필(`approved-plan`)·제품 보호(`product-safety`) 빌드 기준이다. 각 단계는 코드 자리를 적었다.

| 단계 | 하는 일 | 근거 |
| --- | --- | --- |
| 1 | 중단·방향 전환 신호를 기록: `gil_plan_request_interview(reason)`. 아무것도 닫거나 열지 않는다 | `src/approved_plan.rs` `request_plan_interview`, `tests/guidance_wave2.rs` |
| 2 | 연결 끊김·사람 응답 대기는 종료가 아니다. 열린 Step을 그대로 두고 같은 기록에서 이어간다 | Cycle Model §8, §20.1 |
| 3 | 계약한 작업을 계속할 수 없으면 **중단 관측 문서**(수행 범위·막힌 이유·미검증 내용)를 저장해 Verify 증거로 닫는다. 부분 결과 파일은 그 Verify Snapshot에 보존된다 | `evidence_files` 신선도 규칙(`src/session.rs`), workflow 규칙 4 |
| 4 | Analysis·Outcome·Cycle Report에 미검증 범위를 이어 적는다. Outcome `verdict`는 현재 문법의 두 값만 허용되며(§8), 「평가하지 않았다」는 사실은 `lesson`·`handoff_summary`에 반증과 구분해 남긴다 | `spec/gil-spec.yaml` outcome/experiment `allowed_values` |
| 5 | Experiment `failure`의 기록 가능한 다음 방향은 `revisit`만이다 | `gil-spec.yaml` `allowed_values_when`, `src/cycles.rs::check_next_direction` |
| 6 | 실패 Cycle 아래에는 Interview도 Experiment도 열리지 않는다 | Cycle Model §6, `src/cycles.rs::why_not_open_child` |
| 7 | 제품 빌드의 revisit은 Monitor의 변경 목록 확인 → 「보존 후 되돌아가기」 승인이 필요하다. 승인 전에는 폴더·기록이 바뀌지 않는다 | `src/session/revisit_recovery.rs`, `tests/support_wave1/revisit_cases.rs` |
| 8 | 승인 복구는 before 세계 파일을 `.gil/revisit-preserved/<sha>/`에 보존한 뒤 폴더를 조상 Exit으로 복원한다. 부분 결과는 **폴더에서 빠지지만** Snapshot과 보존본에 남는다 | 같은 파일 `apply_recovery`, `files.json` |
| 9 | 되돌아온 자리에서 Interview 가지를 열어 새 판을 승인한다. 기존 항목은 원문 유지, 추가·순서·scope 변경만 허용. 실패 항목은 pending으로 돌아오고 재바인딩은 revisit 계보를 요구한다 | `src/approved_plan.rs` `retained_items`·`pending`·retry lineage 검사 |

따라서 **「현재 부분 결과를 폴더에 둔 채 재계획」하는 길은 오늘 계약에 없다.** 5·6단계가 그것을 막고, 그 변경은
Cycle Model §6·§11의 규범 변경(기존 설계안 D1)이다. 이번 PR은 그 길을 만들지 않았고, 안내도 「아직 없다」고만 말한다.

## 2. 이번 PR이 바꾼 것 (계약 불변)

- 안내 문구 3개(`src/i18n/*.json`, 5개 언어): `guide.interrupt_honest`(1~4단계의 정직한 중단 규칙), `guide.interrupt_partial_results`
  (7·8단계 전에 Verify `evidence_files`의 부분 결과를 먼저 보여 주고, 파일 유지 전이는 아직 없음을 알림), `plan.err.no_cancel`
  (항목 취소·대체·재정의는 계약에 없음, 추가·순서·scope 변경만).
- 합성 지점: `src/context.rs`(보존된 재인터뷰 요청 알림, failure·revisit으로 닫힌 Cycle의 다음 호출), `src/mcp/guidance.rs`
  (`gil_plan_request_interview` 영수증, Experiment failure·revisit 닫기 영수증), `src/approved_plan.rs`(항목 누락·정의 변경 거절문).
- 공통 지침 `src/mcp/workflow.md` 규칙 4·7에 각 한 단락. 복구 표는 PR #125와 겹치므로 건드리지 않았다. Skill 생성 영역 동기화.
- 회귀 시험: `src/approved_plan/tests.rs`(중단 흐름 전체: 요청 보존 → 중단 관측 증거 → failure·revisit → 폴더 파일 유지 →
  두 종류 자식 거절 → pending 복귀 → 안내 → 재저장 바이트 동일; 개발 프로필에서는 revisit 뒤 항목 삭제 거절문까지),
  `tests/support_wave1/revisit_cases.rs`(실제 stdio: 승인 전 거절·파일 불변, 승인 복구 뒤 폴더에서 빠지고 보존본 `files.json`에 남음),
  `src/mcp/guidance.rs`·`tests/guidance_wave2.rs`(영수증 문구).
- 바꾸지 않은 것: 저장 schema, format 번호, Grammar와 `allowed_values_when`, 승인 재사용 규칙, M4-R 차단·승인 경로, 기존 기록.
  이 PR을 「중단 기능 구현」으로 세지 않는다. 안내는 실제 모델 평가·Host 화면 인수를 거치지 않았다.

## 3. 사용자 결정이 필요한 쟁점

권장안은 권장일 뿐이며 결정은 사용자가 한다. 결정 전에는 §1의 경로가 유일한 길이다.

### D1. 결론 미도달 뒤 부분 결과를 폴더에 둔 채 재계획(Interview)을 열 수 있는가
- 문제: 제품 요구(§8 「현재 부분 결과에서 이어가기」)와 규범(§6 「failure → 자식 없음」)이 충돌한다. 오늘은 revisit이
  부분 결과를 폴더에서 치운다(보존은 된다).
- 대안 A — 기존 설계안: `conclusion: not_reached`일 때만 `failure → open_child` 허용(규범 1행 추가). 부분 세계가 다음 Entry가 된다.
  장점: product-safety에서도 파일 복원 없이 진행. 단점: Outcome/Cycle Report에 선택 필드가 늘고(format 번호는 불변, `deny_unknown_fields`
  writer는 거절), §6의 「실패 세계 미채택」 원칙에 예외가 생긴다.
- 대안 B — Interview만 허용: 결론 미도달 실패 아래에 **Interview**(Artifact 투명)만 열고 Experiment는 금지. 사람이 부분 파일의 처리를
  결정한 뒤 승인 판에 따라 Experiment를 연다. 대안 A보다 좁고, Interview Entry = 실패 Exit이므로 Snapshot 규칙은 그대로다.
- 대안 C — 현상 유지: revisit만. 부분 결과는 보존본에서 사람이 꺼낸다(「보존 파일을 작업 폴더에 다시 넣는 것은 복구 명령의 일부가 아니다」).
  추가 구현은 보존본 조회·표시만.
- 의존: D4(저장 구분)가 없으면 A·B는 「판정된 failure」와 구별할 수 없어 적용 범위를 정할 수 없다.

### D2. 결론 미도달 시도가 항목을 소비하는가, 재시도 횟수와 승인 재사용
- 오늘: 최신 실행이 `failure`면 항목은 pending으로 돌아오고(판정 failure와 미도달을 구분하지 않음), 재바인딩은 revisit 계보를 요구하며
  승인 판은 그대로 쓴다. 상한은 없다.
- 대안: (a) 현상 유지 (b) 기존 설계안의 2회 상한 + `attempt/retry_of` 선택 필드 (c) 모든 재시도에 새 Interview 요구.
- 주의: 「새 Chain에서 이전 승인 재사용 금지」는 이미 구현·시험돼 있고 이 쟁점과 무관하다.

### D3. Active Will 좁히기(`revise_will`)를 공개 동작으로 둘 것인가
- 오늘: Will은 Verify close와 함께 Done이 되며 `done_when`과 실제 행동의 어긋남은 Report 본문에서만 드러난다(Will Model §13 미결).
- 대안: 공개 보조 동작 추가 / 추가하지 않고 Report 정직성 규칙으로 처리(현상).

### D4. 「검증 미완료」를 저장 형식에서 어떻게 구분할 것인가
- 오늘: Outcome `verdict`는 `success|failure`뿐이다. 구분은 `lesson`·`handoff_summary` 본문에만 있다(이번 안내가 그 자리를 가리킨다).
- 대안: (a) 선택 필드 `conclusion`/`blocker`(기존 설계안; 부재 = 기존 의미, format 번호 불변) (b) 본문 규칙만 유지 (c) 제3 verdict(§8이 금지).
- 이 결정이 D1·D2의 전제다.

## 4. 결정 없이도 안전한 다음 작업

- M4-R 자동 승인 모드(2026-11-15 명세, 구현 대기): 설정 저장·출처 구분·동일 보호 검사. 중단 경로의 7·8단계를 사람 클릭 없이 통과시키되
  열린 단계 강제 종료·계획 변경 승인으로 확대하지 않는다.
- Monitor에서 failure·revisit으로 닫힌 Cycle의 `evidence_files`가 실제로 보이는지 화면 인수(이번 안내의 전제).
- 실제 모델 평가: 이번 안내가 중단을 success로 꾸미는 사례와 불필요한 조기 Cycle close를 줄이는지.
- 결정 뒤: D4 → D1/D2 순서로 설계안을 갱신하고 `design-tests/m6e` 모델을 결정에 맞춰 고친 다음 Core를 구현한다.

## 5. 의존성과 경계

- PR #125(안내 보완)와 `src/mcp/workflow.md`·`src/mcp/guidance.rs`·i18n·`tests/approved_plan_wire_wave1.rs`를 함께 만진다. 이 PR은
  규칙 4·7과 `plan()`·`closed()`의 다른 분기만 바꿨고 복구 표는 두었다. 병합 순서에 따라 Skill 생성 영역 재동기화가 필요할 수 있다.
- PR #128(.gilignore 설계, 미승인)은 수정하지 않는다. 보존·복원에서 제외 파일의 비간섭은 그 결정 뒤의 과제다.
- 기존 설계안 `M6E_Open_Step_Interruption_v0.1.md`와 `design-tests/m6e`는 수정하지 않았다(미채택 후보로 유지).

## 6. 로컬 검증 (macOS arm64, 2026-11-16)

| 명령 | 결과 |
| --- | --- |
| `cargo test --locked -p gil --features approved-plan,visual-authoring,product-safety --lib -- --test-threads=1` | 749 통과 · 0 실패 · 1 ignored(기존) |
| 같은 프로필 `--test context --test context_profiles --test guidance_wave1 --test guidance_wave2 --test approved_plan_wire_wave1` | 12 + 15 + 2 + 7 + 2 통과 · 0 실패 |
| `cargo build --locked -p gil --all-targets --features approved-plan,visual-authoring,product-safety` | 통과 |
| `cargo test --locked -p gil` (기본 프로필 전체) | 1,621 통과 · 0 실패 · 2 ignored(기존) |
| `node --test plugins/gil-companion-prototype/skill.test.mjs plugins/gil-companion-prototype/workflow.test.mjs design-tests/m6e/m6e.test.mjs` | 56 + 25 통과 |
| `git diff --check` | 공백 문제 없음 |

- 미실행: `npm ci --prefix plugins/gil-companion-prototype` 의존 Plugin 시험 나머지, `mcp-app` 빌드·UI 시험, 배포·notice 검증, Windows 실기, chain-core 전용 워크플로. 이 변경은 UI·배포 파일을 건드리지 않았다.
- 주의: 승인 프로필 시험 뒤 같은 임시 폴더(`gil-serve-*`, `gil-refresh-*`)에 format 6 기록이 남으면 기본 프로필의 Monitor 시험이
  `UnknownFormat { found: 6, known: 5 }`로 실패한다. 이 변경과 무관한 재사용 fixture 문제이며 폴더를 지우고 재실행해 통과를 확인했다.
- 합성 승인 입력 시험이며 실제 사람의 Monitor 클릭·실제 모델 평가·Host 화면 인수가 아니다.
