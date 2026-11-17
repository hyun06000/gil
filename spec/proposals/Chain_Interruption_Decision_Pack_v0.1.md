# Chain 중단·보존·재계획 결정안 v0.1 (Chain Interruption Decision Pack)

> **2026-11-17 최종 합의에 따른 이전 후보 문서.** 아래 후보·권장안·구현 순서를 현재 작업 지시로
> 사용하지 않는다. 최신 설계는 [승인 계획 계약](../GIL_Approved_Plan_v0.1.md)의 후속 합의와
> 구현 이행 절, [Cycle Model](../GIL_Cycle_Model_v0.1.md) §7·복원·종료를 따른다.
> 새 Chain의 안정된 승인 요구 아래 여러 예/아니오 Define으로 탐색한다. 실패하면 변경 파일이
> 없어도 명시적 revisit을 거쳐 세계 전체를 복원하고 배움은 보존한다. 반성은 복원점과 다음 방향의
> 근거를 마련한다. 부분 세계 유지·실패 후 직접 진행·분할 관계 R·반복 명세 수정은 기본 경로로
> 채택하지 않았다. 이 문서의 기존 모델 시험은 그 후보만 검사하며 새 설계의 구현 증거가 아니다.


상태: **결정안(2026-11-17). 사용자 결정 대기. 코드·규범 명세·공통 로드맵·기존 제안서는 바꾸지 않았다.**
이 문서는 사용자가 아래 질문에 답하면 바로 구현에 들어갈 수 있도록 **현재 코드의 동작, 결정 질문, 권장안을 택했을 때의
전이·보존·재개·호환 규칙, 구현 순서와 검증 사례**를 한 곳에 모은다. 권장안은 권장일 뿐이며 사용자 결정이 아니다.
새 상태·도구·저장 칸·재시도 횟수는 **모두 미확정**이다. 이름이 필요한 자리는 「가칭」으로 적는다.

정본과 선행 문서:
[Cycle Model §8 「완료할 수 없는 작업의 중단과 다음 방향」](../GIL_Cycle_Model_v0.1.md#완료할-수-없는-작업의-중단과-다음-방향),
[Approved Plan 개발 계약](../GIL_Approved_Plan_v0.1.md), [M4-R 승인 복구 계약](../GIL_Revisit_Recovery_v0.1.md),
[Chain 저장 계약](../GIL_Chain_Storage_v0.1.md), [Roadmap §11 M6-E](../GIL_Roadmap.md),
[M6-E 설계안(미채택 후보)](M6E_Open_Step_Interruption_v0.1.md), [M6-E 구현 준비(2026-11-16)](M6E_Interruption_Implementation_Prep_v0.1.md).
구현 준비 문서의 §1(오늘 계약 안의 경로 9단계)과 §3(D1~D4)은 그대로 유효하며 여기서 되풀이하지 않는다. 이 문서는 그 질문을
**결정 가능한 꼴로 좁히고**, 결정 뒤의 작업을 적는다.

약어 풀이: Verify = 실제 작업·관측 단계. Outcome = 판정 단계. Cycle Report = 실험 하나를 닫는 보고서. Interview = 사용자와
합의하는 주기. Experiment = 하나의 실험 주기. revisit = 과거 성공 지점으로 되돌아가기. Snapshot = 그 시점의 파일 전체 모습.
approved-plan = 승인 계획 프로필(사람이 Monitor 버튼으로 계획을 승인하는 빌드). product-safety = 제품 보호 빌드(파일 되돌리기를
승인 없이 실행하지 않는 빌드). MCP = Host(Codex·Claude Desktop)와 GIL Core가 대화하는 프로토콜.

---

## 후속 결정: 반성에서 복원 기준점 선정

반성은 관련 Step·Cycle을 역추적하여 잘못된 정보의 발생·전파 근거를 찾고 복원 기준점을 판단한다.
직전 상태를 자동 기본값으로 채택하지 않는다. 원인 위치와 안전한 복원 상태는 구분하며 근거가
부족하면 원인을 확정하지 않는다. 실패하면 명시적 revisit의 근거와 다음 작은 Cycle을 준비한다. 성공 후 정상 진행에서는 다음 Cycle을 준비한다. 원인 추적은
다른 Chain의 기록도 포함할 수 있지만 실제 Chain 간 복원 전이는 아직 승인·구현되지 않았다.
복원 판단은 자동 실행 승인이 아니다. 구체적 반성 기록과 기존 전이·승인 연결은 후속 설계한다.

## 후속 결정: 세계 전체를 되돌리고 배움을 유지

2026-11-17 사용자 결정으로, 실패 후 되돌리기는 추적 대상 작업물과 산출물 모두를 대상
Snapshot으로 복원한다. 현재 부분 세계를 유용성·성공 비율에 따라 유지하는 방식을 기본안으로
삼지 않는다. 아래의 부분 파일 유지·재계획 권장안은 이 실패 후 재탐색 경로에 채택되지 않은
이전 후보로 읽는다. 실패 증거·관측·분석·반성은 보존하며, 복원된 세계에서 다음 작은 문제를
탐색한다. 미검증 원인을 사실로 단정하거나 ‘대부분 성공’을 완료 조건 대신 쓰지 않는다.
정본은 [Cycle Model](../GIL_Cycle_Model_v0.1.md)의 「세계 전체 복원과 배움의 보존」이다.
자동 복구 승인·새 전이·저장 필드는 이번 결정으로 추가하지 않는다.

## 0. 이전 후보의 결정 질문 (현재 결정 요청 아님)

| 순위 | 질문 | 권장안 | 다른 질문과의 관계 |
| --- | --- | --- | --- |
| **Q1** | 「검증을 끝내지 못한 실패」를 저장 형식에서 어떻게 구분하는가 | Experiment의 Outcome과 Cycle Report에 **선택 구분 칸 하나(가칭 `conclusion`)**를 두고, Outcome에만 막힘 이유 칸(가칭 `blocker`)을 함께 요구한다. 없으면 「판정된 실패」로 읽는다. 검증은 `gil-spec.yaml`이 아니라 코드(`target_cycle_ref`와 같은 방식)에서 한다 | Q2·Q3의 전제. 이것이 없으면 다른 결정을 적용할 범위를 정할 수 없다 |
| **Q2** | 부분 결과를 작업 폴더에 둔 채 재계획(Interview)으로 갈 수 있는가 | **허용하되 Interview만.** 결론 미도달 실패 아래에 Interview 한 종류만 열 수 있고 Experiment는 금지. 이는 Cycle Model §3·§6 「실패한 Cycle은 자식을 만들지 않는다」의 **규범 변경**이다 | Q1 필요. Q3의 계보 규칙에 영향 |
| **Q3** | 중단된 승인 항목의 상태, 재시도, 이전 승인 재사용 범위 | **항목 소비 규칙·상한은 현상 유지**(실패하면 항목은 다시 미실행으로 돌아오고, 횟수 제한 없음, 저장 칸 추가 없음). Q2를 택하면 재시도 계보 검사에 「부모 사슬로 직전 미도달 시도에 닿는 경우」를 추가한다. 시도 횟수는 저장하지 않고 기존 기록에서 계산해 **표시만** 한다 | Q1·Q2에 종속 |
| **Q4** | 끝낼 수 없는 Will(행동 계약)을 좁히는 공개 동작을 둘 것인가 | **두지 않는다.** 중단 관측 문서와 Report 본문이 진실을 말하고 Will은 지금처럼 Verify close와 함께 Done이 된다 | 독립. 구현 준비 문서 D3 그대로 |

Q1·Q2만 답하면 구현의 대부분이 정해진다. Q3는 「현상 유지」가 기본값이며, Q4는 「두지 않음」이 기본값이다.

---

## 1. 현재 코드가 세 가지 중단을 다루는 방식

승인 계획 프로필 + 제품 보호 빌드(현재 시험판 preview.31 / Windows v0.3.x) 기준이다. 각 행에 코드·시험 자리를 적었다.

### 1.1 연결 끊김·응답 대기 — 종료가 아니다

| 동작 | 근거 |
| --- | --- |
| 열린 Step·Active Will·열린 Cycle은 그대로 남고 새 세션이 `gil_context`로 이어받는다. 시간 경과·연결 끊김으로 실패·승인·종료를 만들지 않는다 | Cycle Model §20.1, 기본 명세 §8 「진행이 멈춘 상태」, `src/context.rs::next_call` |
| 두 번째 프로세스는 프로젝트 잠금에서 거절된다. 남은 잠금 파일만으로는 막지 않는다 | `src/lock.rs`(`a_second_hold_in_this_process_is_refused_not_granted`, `a_leftover_lock_file_does_not_block_anyone`) |
| 프로세스가 복원 도중 죽으면 다음 열기가 먼저 미완 복원을 되돌리고 임시 객체를 치운 뒤 기록을 읽는다 | `src/session.rs::open_at`(잠금 → 복구 → 회수 → 읽기), `src/restore/mod.rs`(`active`·`PLAN`·rollback) |
| 승인 대기 중 끊김: 저장된 승인은 지워지지 않고, 응답 유실은 `gil_plan_read`·`gil_status`로 다시 읽는다 | Approved Plan 「MCP 입력 오류 복구」, `src/mcp/workflow.md` 규칙 8 복구 표 |
| 관측 자체가 불가능한 환경(링크·권한)은 dirty가 아니라 「모름」으로 거절하고 아무것도 바꾸지 않는다 | `src/session.rs::require_clean`의 `WorldState::Unknown` 분기 |

### 1.2 검증된 실패 — 기존 실패 종료·복구 계약

| 동작 | 근거 |
| --- | --- |
| Outcome `verdict: failure` → Cycle Report `failure`, 다음 방향은 **`revisit`만** 기록 가능 | `spec/gil-spec.yaml` experiment `allowed_values_when`, `src/validate.rs::check_report` |
| 실패 Cycle 아래에는 Interview도 Experiment도 열리지 않는다(`open_child`를 적은 Cycle만 분기점) | `src/cycles.rs::is_branch_point`, `why_not_open_child`, 복원 시 `check_restored`의 `ParentDidNotOpenAChild` |
| 제품 빌드의 되돌아가기는 Monitor 변경 목록 확인 → 「보존 후 되돌아가기」 버튼. 모델의 `gil_revisit`은 저장된 승인의 재개만 한다 | `src/session/revisit_recovery.rs::resume_revisit`·`approve_revisit`, `tests/support_wave1/revisit_cases.rs` |
| 승인 복구는 before 세계 전체를 `.gil/revisit-preserved/<sha>/`에 `files.json`·`approval.json`·`source.json`과 함께 보존한 뒤 조상 Exit 세계로 복원한다 | 같은 파일 `apply_recovery`, `tests/chain_recovery_preservation.rs` |
| 실패 항목은 다시 미실행(pending)으로 돌아오고, 같은 항목의 재바인딩은 직전 실패를 `revisit_from` 사슬로 이어야 한다 | `src/approved_plan.rs::pending`·`validate`(retry lineage), `plan.err.retry_lineage` |
| 재인터뷰 요청이 남아 있으면 새 Experiment를 열 수 없고, 새 판 승인이 요청을 해소한다 | `next_plan_execution`(`plan.err.reinterview_pending`), `awaiting_interview` |

### 1.3 검증을 끝내지 못한 중단 — 오늘은 「판정된 실패」와 같은 길

| 동작 | 근거 |
| --- | --- |
| 중단 관측 문서(수행 범위·막힌 이유·미검증 내용)를 새 파일로 저장해 Verify 증거로 닫는다. 부분 결과 파일은 그 Verify Snapshot에 보존된다 | `src/session/evidence.rs::require_verify_evidence`(빈 파일·변하지 않은 증거 거절), 안내 `guide.interrupt_honest` |
| Outcome은 현재 문법의 두 값만 허용하므로 `failure`로 닫고, 「평가하지 않았다」는 사실은 `lesson`·`handoff_summary` 본문에만 남는다 | `src/approved_plan/tests.rs::an_item_that_cannot_finish_closes_truthfully_keeps_partial_files_and_gets_no_shortcut` |
| 그 뒤는 1.2와 완전히 같다: 방향은 `revisit`뿐, 자식 없음, Monitor 승인 복구가 부분 결과를 폴더에서 치운다(보존은 됨) | `tests/support_wave1/revisit_cases.rs::an_interrupted_item_keeps_its_partial_files_until_the_approved_recovery_and_shows_them_first` |
| 안내는 「부분 결과를 둔 채 이어가는 전이는 아직 계약에 없다」고 말한다 | `guide.interrupt_partial_results`, `plan.err.no_cancel`, `src/mcp/workflow.md` 규칙 7 |

### 1.4 오늘 막혀 있는 자리 (이 문서가 풀려는 것)

1. **저장 구분 없음.** 판정된 실패와 결론 미도달이 기록에서 같다. Monitor·Context·승인 장부가 둘을 가를 수 없다.
2. **failure → 자식 없음.** 부분 결과를 폴더에 둔 채 사람에게 묻는 길이 없다. 유일한 길인 되돌아가기는 폴더를 치운다.
3. **재시도 계보는 `revisit_from`만 인정한다.** 되돌아가지 않고 이어가는 길이 생기면 계보 검사가 그 재시도를 거절한다.
4. **안내가 「없다」고 말한다.** 전이가 생기면 두 안내 문자열(`guide.interrupt_partial_results`, `plan.err.no_cancel`)과 공통 지침 규칙 7을 고쳐야 한다.

---

## 2. 결정 질문 — 선택지·권장안·장단점

### Q1. 결론 미도달의 저장 구분 (전제)

제품 요구: 「현재 `failure` 판정과 결론 미도달을 저장 형식에서 어떻게 구분할지는 별도 설계한다」(Cycle Model §8).
`verdict`는 두 값만 허용한다(§8, 기본 명세). 제3 verdict는 선택지가 아니다.

| 선택지 | 내용 | 장점 | 단점 |
| --- | --- | --- | --- |
| **(a) 선택 구분 칸 — 권장** | Experiment **Outcome**에 구분 칸(가칭 `conclusion`: 판정됨 / 결론 미도달)과, 미도달일 때만 필수인 막힘 이유 칸(가칭 `blocker`). **Cycle Report**에는 구분 칸만 두고 Outcome과 같아야 한다(`verdict` 일치 검사와 같은 규칙). 칸이 없으면 「판정됨」 | Report는 키-값 맵으로 저장되며 `validate.rs::check_report`는 필수 칸 누락과 제약만 검사하므로 **추가 칸은 기존 reader가 거절하지 않는다**(`link_learning`이 이미 추가 칸을 넣는 선례). 형식 번호 불변. Monitor·Context·승인 장부가 같은 칸을 읽는다 | 선택 칸의 값 제약은 `gil-spec.yaml`에 적을 수 없다(`rules.rs::check_constraints_point_at_real_fields`가 `close_requires` 밖의 제약을 거절). 따라서 `next_direction.target_cycle_ref`처럼 **코드가 조건부 칸을 검사**해야 한다 |
| (b) 본문 규칙만 | 오늘 그대로. `lesson`·`handoff_summary` 문장으로만 구분 | 변경 없음 | 기계가 구분하지 못하므로 Q2·Q3를 적용할 범위를 정할 수 없다. Monitor가 둘을 같은 실패로 보여 준다 |
| (c) 제3 verdict | `inconclusive` 등 | 한 칸으로 끝 | Cycle Model §8·기본 명세 위반. Chain 판정 전파·승인 장부의 `failure` 판정 코드가 전부 바뀐다 |

(a)의 세부(결정 뒤 구현자가 정하되 지금 고정하지 않는 것): 칸 이름, 두 값의 표기, Outcome의 막힘 이유 칸을 별도로 둘지
`lesson`에 합칠지. 권장은 **별도 칸**이다. 제품 요구가 「수행 범위·막힌 이유·미검증 내용」을 구분하라고 하고, Monitor가 그
한 줄을 바로 보여 줄 수 있기 때문이다.

불변 규칙(어느 선택지든): `success`와 「결론 미도달」의 조합은 거절한다. 미도달은 **판정하지 않았다는 선언**이지 성공도
반증도 아니며, Chain 판정·승인 항목의 「실패」 처리를 바꾸지 않는다.

### Q2. 부분 결과를 폴더에 둔 채 재계획으로 갈 수 있는가 (규범 변경 명시)

제품 요구: 다음 경로는 「현재 부분 결과에서 이어가기 / 이전 상태로 돌아가 재시도 / 인터뷰로 계획 변경」(Cycle Model §8).
오늘은 두 번째만 있고, 첫 번째·세 번째는 되돌아간 **뒤에만** 가능하다(부분 결과는 폴더에서 빠진다).

| 선택지 | 내용 | 장점 | 단점 |
| --- | --- | --- | --- |
| **(B) Interview만 허용 — 권장** | Cycle Report가 `failure` + 「결론 미도달」일 때 다음 방향으로 `revisit` 또는 `open_child`를 적을 수 있고, `open_child`로 여는 자식은 **Interview 한 종류만**. Experiment는 거절 | Interview는 Artifact에 투명하다(Entry = 실패 Cycle의 Exit = 부분 결과가 든 Verify Snapshot, Exit = 같은 값). 파일을 만지지 않으므로 **제품 보호 빌드에서도 승인 없이 진행**할 수 있다. 부분 결과의 처리는 사람이 Interview에서 결정한다. 그 Interview가 성공하면 다음 Experiment는 Interview의 자식으로 열리고 **부분 세계에서 출발**한다(= 「현재 결과에서 이어가기」). 되돌아가기는 그대로 남는다(= 「이전 상태로 돌아가 재시도」) | **Cycle Model §3·§6의 규범 변경 1건**(「실패한 Cycle은 자식을 만들지 않는다」에 「결론 미도달 실패 아래의 Interview」 예외). `gil-spec.yaml`의 `allowed_values_when`에 행이 하나 늘고 복원 재검증(`cycle.rs::validate_cycle_close` 호출)도 같은 표를 쓴다 |
| (A) Interview·Experiment 모두 허용(기존 설계안 D1) | 위에 더해 같은 항목의 Experiment를 바로 다시 연다 | 사람 개입 없이 재시도 가능 | 부분 세계를 사람이 확인하지 않은 채 다음 실험의 출발점으로 삼는다. 제품 요구 「파일을 되돌리기 전에 부분 결과의 보존을 확인」·「기존 승인은 새 작업 범위로 확대하지 않는다」와 긴장. (B)보다 넓어 결정이 늘어난다(재시도 상한 등) |
| (C) 현상 유지 | 되돌아가기만. 부분 결과는 보존본에서 사람이 꺼낸다 | 규범 불변 | 「현재 부분 결과에서 이어가기」가 없다. 보존본을 폴더에 넣는 것은 복구 명령의 일부가 아니므로 사용자 수작업이다. 제품 요구 미충족 |

(B)의 부속 사항(결정 시 함께 확인):

- **방향 선택 시점의 확인.** 방향은 에이전트가 Cycle Report에 적는다. `open_child`를 적으면 그 뒤의 Interview는 실패로 닫히지
  않으므로 **되돌아가기 길이 없어진다**. 반대로 `revisit`을 적으면 Monitor 버튼이 필요하다(오늘과 같음). 따라서 안내 규칙으로
  「Cycle Report를 닫기 전에 부분 결과를 보여 주고 사용자에게 두 경로 중 하나를 묻는다」를 둔다. 방향 자체를 버튼으로
  승인받는 보장은 새 도구가 필요하므로 이번 결정안에서는 제안하지 않는다. 기록된 사람의 승인은 그 다음 단계(Interview의
  계획 승인 버튼, 복구 버튼)가 맡는다.
- **되돌아가기 대상이 될 수 있는가.** `open_child`를 적은 미도달 Cycle은 구조상 분기점이므로(`is_branch_point`), 그 뒤의 실패
  Experiment가 그 미도달 Cycle을 `target_cycle_ref`로 되돌아갈 수 있다. 되돌아가면 **부분 세계**가 복원된다. 권장은 **허용**
  (그 세계는 사람이 뒤따른 Interview에서 확인한 세계다). 금지하려면 `check_next_direction` ⑤에 조건 하나를 더한다.
- **적용 프로필.** 문법은 하나(`gil-spec.yaml`)이므로 모든 프로필에 같은 행이 생긴다. 승인 계획 프로필에만 한정하려면 코드
  분기가 필요하다. 권장은 **공통 적용**(규범은 하나이고, 제품 빌드는 어차피 승인 계획 프로필이다).
- **바뀌지 않는 것.** 열린 Cycle을 둔 채 다른 Cycle을 열 수 없다(`why_not_open_child::CurrentStillOpen`). 판정된 실패 아래의
  자식은 여전히 없다. Interview 아래 Experiment의 Entry는 Interview Exit이다(Artifact Model §7.5). Snapshot 확정 권한은 start와
  Verify close 둘뿐이다.

### Q3. 중단된 항목의 상태·재시도·이전 승인 재사용

오늘: 최신 실행이 `failure`로 닫힌 항목은 다시 pending(판정·미도달 구분 없음). 재바인딩은 `revisit_from` 계보 필요. 승인 판은
그대로 재사용. 상한 없음(`old_failure_requires_exact_app_consent_preserves_evidence_and_retries_twice`가 두 번 재시도를 검사).
「새 Chain에서 이전 승인 재사용 금지」는 이미 구현·시험돼 있고 이 질문과 무관하다.

| 선택지 | 내용 | 장점 | 단점 |
| --- | --- | --- | --- |
| **(a) 현상 유지 + 계보 확장 — 권장** | 항목 소비 규칙·승인 재사용·상한은 그대로. 저장 칸 추가 없음. Q2(B)를 택하면 계보 검사에 「새 Cycle의 **부모 사슬**에 직전 시도가 있고 그 시도가 결론 미도달인 경우」를 인정한다(판정된 실패는 오늘처럼 `revisit_from`만). 시도 횟수는 `executed_cycles`와 Cycle Report에서 계산해 Context·`gil_plan_read`에 **표시만** 한다 | 승인 장부(`PlanState`·`Execution`)가 바뀌지 않아 `deny_unknown_fields` writer 호환 문제가 없다. 재시도 상한을 지금 정하지 않는다(사용자 결정 보류 그대로) | 같은 항목이 무한히 미도달을 반복할 수 있다. 다만 (B) 경로의 재시도는 매번 사람이 Interview에서 새 판을 승인하므로 사람이 멈출 수 있다 |
| (b) 기존 설계안: 2회 상한 + `attempt`·`retry_of` 칸 | 미도달 시도는 항목을 소비하지 않고, 같은 판에서 3회째 거절 | 반복을 기계가 막는다 | `Execution`에 칸이 늘어 구형 approved-plan writer가 기록을 거절(안전 거절이지만 영향 범위가 늘어남). 상한 수치는 미확정 결정 사항 |
| (c) 모든 재시도에 새 Interview 요구 | 되돌아간 뒤에도 Experiment를 바로 열 수 없음 | 사람이 매번 본다 | 오늘 통과하는 시험(`retries_twice`, `failed_item_can_be_reinterviewed_and_rebound…`)의 의미가 바뀌고 M4-R 복구 뒤 흐름이 길어진다 |

이전 승인 재사용의 경계(어느 선택지든): 되돌아가기 경로는 오늘처럼 **같은 승인 판**으로 같은 항목을 다시 바인딩한다.
(B) 경로는 **새 판**이 필요하다(Interview에서 기존 항목은 원문 유지, 추가·순서·scope 변경만). 어느 경로도 승인 범위를
넓히지 않는다. 항목의 취소·대체·재정의는 여전히 없다(`plan.err.no_cancel`).

### Q4. 끝낼 수 없는 Will을 좁히는 공개 동작

구현 준비 문서 D3 그대로다. 권장은 **두지 않음**. Will은 Verify close와 함께 Done이 되고(`project.rs::close_action_step`),
`done_when`과 실제 행동의 어긋남은 중단 관측 문서와 Report 본문이 말한다. Will Model §13 「중단·취소된 Will의 처리」는
미결로 남는다. 이 질문은 다른 셋과 독립이며 나중에 바꿔도 저장 형식에 영향이 없다.

---

## 3. 권장안(Q1 a · Q2 B · Q3 a · Q4 없음)을 택했을 때의 상태 전이와 승인 경계

### 3.1 전이표 (Experiment Cycle Report)

| verdict | 구분(가칭 `conclusion`) | 기록할 수 있는 `next_direction.action` | 다음에 열 수 있는 Cycle |
| --- | --- | --- | --- |
| success | 판정됨 | `open_child` | Interview · Experiment (오늘과 같음) |
| failure | 판정됨(칸 없음 포함) | `revisit` | 되돌아간 조상 아래 Interview · Experiment (오늘과 같음) |
| failure | **결론 미도달** | `revisit` **또는** `open_child` | `revisit`: 오늘과 같음. `open_child`: **Interview만** |
| success | 결론 미도달 | 거절 | — |

Outcome(Step 계층)의 `next_direction.action`은 바뀌지 않는다(`failure → revisit | close_cycle`). Interview는 여전히 success로만
닫히고 구분 칸을 갖지 않는다. Chain Report는 바뀌지 않는다(추가 칸 거절 유지).

### 3.2 세 경로와 사람의 승인 경계

| 제품 요구의 경로 | 전이 | 폴더 | 사람의 기록된 승인 |
| --- | --- | --- | --- |
| 현재 부분 결과에서 이어가기 | 미도달 failure → `open_child` → Interview(부분 세계) → 새 판 승인 → Experiment(Entry = 부분 세계) | 바뀌지 않음 | Interview의 계획 승인 버튼(M6-D) |
| 이전 상태로 돌아가 재시도 | failure(판정·미도달 모두) → `revisit` → Monitor 「보존 후 되돌아가기」 → `gil_open` | 보존 후 조상 Exit으로 복원 | 복구 버튼(M4-R, 자동 모드는 별도 결정) |
| 인터뷰로 계획 변경 | 위 두 경로 모두 Interview를 거친다. 재인터뷰 요청(`gil_plan_request_interview`)이 있으면 Experiment는 새 판 승인 전까지 거절 | 경로에 따름 | 계획 승인 버튼 |

승인 경계 불변: 채팅의 동의는 승인이 아니다. 복구 자동 승인 모드는 「이미 실패로 닫힌 실험의 복구」에만 적용하며 이 문서가
그 범위를 넓히지 않는다. 열린 Step·Cycle의 강제 종료는 없다. 중단은 언제나 **Verify를 중단 관측 문서로 닫는 것**에서 시작한다.

### 3.3 코드 자리 (권장안을 택했을 때 바뀌는 판정)

| 판정 | 오늘 | 권장안 |
| --- | --- | --- |
| Cycle Report 문법 | `failure → revisit` | 구분 칸이 미도달이면 `revisit | open_child`. 코드 검사: 구분 칸 값, Outcome과의 일치, 미도달일 때 막힘 이유 칸 비어 있지 않음 |
| `open_child`로 열 Kind | 부모가 분기점이면 두 Kind | 부모가 미도달 failure면 Interview만(`cycles.rs::open_child`에 Kind 검사 추가, 복원 `check_restored`도 같은 검사) |
| 재시도 계보(`approved_plan.rs::validate`) | `revisit_from` 사슬로 직전 실패에 닿아야 함 | 추가: 부모 사슬로 직전 시도에 닿고 그 시도가 미도달이면 인정 |
| Context·영수증 | 「이어가는 전이는 아직 없다」 | 미도달 failure 뒤: 부분 결과 목록 → 두 경로 안내(Interview로 이어가기 / 복구 버튼). 판정된 failure 뒤: 오늘과 같음 |

---

## 4. 저장 형식 영향과 실패 시 보존 규칙

### 4.1 형식

- format 번호(4/5/6/7)는 바꾸지 않는다. 기존 기록을 자동 이전하지 않는다. `PlanState`·`Execution`·`Chain`의 구조 칸은 그대로다.
- 새 칸은 Outcome·Cycle Report의 **키-값**이다(`store.rs`의 `report: Option<BTreeMap<String,String>>`). 부재 = 기존 의미.
- `gil-spec.yaml`에는 Cycle Report `allowed_values_when`의 행만 늘어난다. 선택 칸의 값 제약은 코드에 둔다(§2 Q1).
- 되돌아가기 보존본(`revisit-preserved/`)·승인 출처(`source.json`)·복구 설정 사이드카는 손대지 않는다.

### 4.2 무엇이 어디에 남는가

| 대상 | 보존 자리 | 근거 |
| --- | --- | --- |
| 부분 결과 파일 | 중단 Verify의 Snapshot(객체 저장소). (B) 경로에서는 작업 폴더에도 그대로. 되돌아가기 경로에서는 폴더에서 빠지되 Snapshot + `revisit-preserved/<sha>/`에 남음 | `evidence.rs`, `apply_recovery`, `chain_recovery_preservation.rs` |
| 중단 관측 문서 | Verify `evidence_files`의 한 줄로 Snapshot에 고정 | `require_verify_evidence`(새 파일이어야 신선도 통과) |
| 막힌 이유·미검증 범위 | Outcome 막힘 이유 칸 + `lesson`, Cycle Report `handoff_summary` | Q1 |
| 승인 이력·실행 바인딩·재인터뷰 요청 | `approved_plan` 장부(append만, 과거 판 불변) | `approved_plan.rs::validate` |
| 실패 실험 기록·Journey·Done Will | 닫힌 기록 불변. Interview·Experiment가 아래에 열려도 바뀌지 않음 | Time Model §7, `check_restored` |
| 이전 시각 증거(PNG) | 해당 Verify에 그대로. 되돌아가기·새 Chain 뒤에도 조회 가능 | `tests/chain_handoff.rs` 실제 PNG 시험 |

### 4.3 실패 시 규칙 (보존에 실패하면 보존 완료로 보고하지 않는다)

| 상황 | 결과 |
| --- | --- |
| 중단 관측 문서를 쓸 수 없거나 Snapshot 확정이 실패 | Verify close 거절. Verify는 열린 채로 남고 「기다림」이다. 어떤 기록도 닫히지 않는다 |
| 관측 자체가 거절(링크·권한·경로 길이) | `WorldState::Unknown` 거절. 환경을 고친 뒤 같은 close를 다시 보낸다 |
| Cycle Report 저장 실패 | 단일 `state.yaml` 저장이므로 전부 아니면 전무. Cycle은 열린 채 |
| 되돌아가기 보존본 불완전(`files.json`·blob 불일치) | 「보존본이 불완전하다」 거절, 기존 파일 무변경(`apply_recovery`) |
| (B) 경로에서 Interview를 연 뒤 부분 파일을 사용자가 손으로 바꿈 | Interview는 비-Verify이므로 dirty 상태에서는 Step·Cycle close 거절(기존 dirty gate). 제품 빌드의 복구 안내는 restore를 권하지 않는다(`session.dirty_recovery_without_restore`). **새 규칙 없음**, 다만 안내에 「부분 세계를 둔 Interview 중에는 파일을 바꾸지 않는다」를 넣는다 |

---

## 5. 중단·재계획 도중 프로세스 종료·중복 요청의 재개

(B) 경로의 새 전이는 모두 **파일을 만지지 않는 단일 `state.yaml` 저장**이므로 기존 규칙으로 충분하다. 새 저널·새 재개 명령은 없다.

| 끊긴 시점 | 재개 | 근거 |
| --- | --- | --- |
| Verify close 도중 | 객체가 먼저 내구성을 얻고 state는 나중이다. 다음 열기가 임시 객체를 회수한다. Verify는 열린 채이므로 같은 close를 다시 보낸다 | `session.rs::open_at`(`reap_tmp`), Artifact Model §10 |
| Cycle Report 저장 도중 | 닫혔거나 안 닫혔거나 둘 중 하나. `gil_status`로 확인. 닫힌 Cycle에 두 번째 close는 거절 | `cycles.rs::close_current`, workflow 규칙 8 「Close response lost」 |
| `open_child` Interview 저장 도중 | 열렸거나 안 열렸거나. 열린 Cycle이 있으면 두 번째 open은 `CurrentStillOpen` 거절 → 세 번째 Cycle은 생기지 않는다 | `why_not_open_child` |
| Interview 승인 대기 중 끊김 | 제안·판·digest는 저장돼 있다. `gil_plan_read`로 수신 상태를 읽는다. 승인 저장 결과 불확실은 재조회 | Approved Plan 「MCP 입력 오류 복구」 |
| `gil_plan_request_interview` 중복 | 같은 요청은 한 번만 저장된다(`contains` 검사) | `request_plan_interview` |
| 되돌아가기 경로의 모든 지점 | 기존 M4-R 재개 계약: 승인 저장 뒤 종료 → `gil_revisit`이 같은 승인으로 재개, 완료 뒤는 no-op, 완료 뒤 새 파일은 거절(삭제 없음) | `approved_recovery_survives_state_save_then_process_death_during_restore`, `after_completion_a_restart_is_a_no_op_and_a_later_file_is_refused_not_swept` |
| 두 세션이 동시에 요청 | 프로젝트 잠금이 두 번째를 거절한다. 외부 편집기는 잠그지 못한다는 한계는 그대로 | `lock.rs` |

새로 검증할 것: 「미도달 failure close → Interview open」 사이에 죽었을 때 `gil_context`가 **두 경로 안내**를 다시 내는지,
그리고 복원된 기록이 바이트 동일하게 재저장되는지.

---

## 6. 기존 기록 읽기·쓰기와 구형 실행 파일에 대한 호환 영향

| 기록 | 새 빌드 | 구형 빌드(preview.31 / v0.3.2 이하) |
| --- | --- | --- |
| 새 전이를 한 번도 쓰지 않은 기존 기록(4/5/6/7) | 그대로 읽고 바이트 동일 재저장(시험으로 고정) | 영향 없음 |
| 구분 칸만 쓴 기록(미도달 + `revisit`) | 구분을 읽고 Monitor·Context에 표시 | **읽힌다.** `check_report`는 추가 칸을 거절하지 않는다. 다만 구분을 모르므로 일반 실패로 표시하고 제약을 검사하지 않는다 |
| `failure` + `open_child` 행을 쓴 기록 | 정상 | **전체 거절.** 복원이 `validate_cycle_close`(`cycle.rs` 약 871행)로 Cycle Report를 다시 재고, 구형 표에는 그 행이 없다. 형식 번호와 무관한 **안전 거절**이며 기록은 바뀌지 않는다. 사용자에게는 「열 수 없음」으로 보이므로 설치본 갱신 안내가 필요하다 |
| 승인 장부 | Q3(a)면 변화 없음 | 변화 없음. Q3(b)를 택하면 `deny_unknown_fields`로 전체 거절 |
| format 4/5(legacy, Chain 없음) | 문법 행은 공통이므로 미도달 Interview 전이가 가능하다(승인 장부는 없음) | 위와 같음 |

새 빌드가 구형 기록에 새 전이를 적용하는 것은 사용자의 일반 작업이며 이전(migration)이 아니다. 기록을 일괄 변환하는 명령은
만들지 않는다. 같은 프로젝트를 구형·신형 설치본이 번갈아 열 수 있다는 점은 안내 문구에 적는다.

---

## 7. 결정 후 구현: 파일·작업 순서·검증 사례

### 7.1 작업 순서

| 순서 | 작업 | 파일 | 완료 판단 |
| --- | --- | --- | --- |
| 1 | 규범 반영(사용자 결정을 문장으로) | `spec/GIL_Cycle_Model_v0.1.md` §3·§6·§8·§11, `spec/GIL_Approved_Plan_v0.1.md` 재시도 계보 문장, `spec/GIL_Roadmap.md` §11 M6-E 체크, `spec/gil-spec.yaml` 행 추가 | 규범 변경 1건이 명시되고 설계안 D1~D4의 상태가 「결정됨」으로 바뀜 |
| 2 | Core 판정 | `src/cycles.rs`(구분 칸 읽기, 미도달 Kind 제한, 복원 재검증), `src/cycle.rs`(Outcome↔Cycle Report 일치), `src/validate.rs`(필요 시 조건부 칸 공통 함수), `src/approved_plan.rs`(계보 확장, 시도 횟수 계산) | §7.3 거절 사례가 모두 거절되고 허용 사례가 통과 |
| 3 | 안내 | `src/context.rs`, `src/mcp/guidance.rs`, `src/i18n/*.json`(5개 언어, 새 키 위주), `src/mcp/workflow.md` 규칙 4·7 + `sync-workflow.mjs --write`, `src/manual` Topic(해당 시) | 안내 계약 회귀 시험(`guidance_wave1/2`, `skill.test.mjs`) |
| 4 | Monitor 표시 | `mcp-app`(노드 배지·상세에 구분·막힘 이유), `gil_plan_read`에 시도 횟수 | `node --test mcp-app/*.test.mjs`. 실제 Host 화면 인수는 별도 |
| 5 | 설계 모델 정리 | `design-tests/m6e`(결정에 맞춰 `attempt`·`retry_of`·상한 제거 또는 유지) | 모델 시험 통과, 더는 「미채택 후보」가 아님 |
| 6 | 전체 회귀 | §7.4 명령 | 두 OS CI 통과. 설치본 게시는 별도 승인 |

2와 3은 같은 PR에 둔다(안내가 「없다」고 말하는 상태로 전이만 먼저 들어가면 안 된다). 1은 결정 직후 먼저 올린다.

### 7.2 손대지 않는 파일

`src/session/revisit_recovery.rs`(승인 출처·보존본), `src/recovery_mode.rs`, `src/restore/*`, `src/artifact/*`(추적 정책·preview),
`src/mcp/recovery.rs`·`plans.rs`의 도구 schema, Monitor의 복구·계획 승인 버튼, `distribution/*`.

### 7.3 검증 사례

허용(통과해야 함):

1. 중단 관측 문서 + 부분 파일로 Verify close → Outcome `failure` + 미도달 + 막힘 이유 → Cycle Report 미도달 + `open_child` → Interview가 열리고 Entry = 실패 Cycle Exit = 부분 파일이 든 Snapshot. 작업 폴더 무변경.
2. 1 뒤 Interview에서 항목 원문 유지 + 추가/순서 변경 판 승인(합성 버튼) → Interview success → Experiment가 같은 항목에 바인딩되고 Entry = Interview Exit(부분 세계). 재시도 계보 검사 통과.
3. 미도달 + `revisit` → 오늘과 같은 Monitor 승인 복구 → 보존본 생성 → `gil_open` → 같은 항목 재바인딩(기존 시험 유지).
4. 판정된 failure(구분 칸 없음·「판정됨」)는 오늘과 완전히 같다(기존 시험 전부 통과).
5. 미도달 Cycle을 뒤의 실패가 되돌아가기 대상으로 삼으면 부분 세계로 복원된다(Q2 부속 결정에 따라 허용/거절 중 하나로 고정).
6. 1 직후 프로세스 종료 → 새 프로세스의 `gil_context`가 두 경로를 안내하고 기록 바이트 동일 재저장.
7. 기존 fixture(format 4·6·7, 승인 판 있는 기록)의 바이트 동일 재저장과 Monitor 투영 불변.
8. 구형 표(현재 `gil-spec.yaml`)로 사례 1의 기록을 읽으면 Cycle Report 재검증에서 거절, 기록 무변경(호환 §6 고정).

거절(막혀야 함):

9. `success` + 미도달.
10. 미도달인데 막힘 이유 칸이 비어 있음. Outcome과 Cycle Report의 구분이 다름.
11. 미도달 failure 아래 Experiment `open_child`.
12. 판정된 failure 아래 Interview·Experiment(오늘과 같음).
13. 미도달 뒤 Interview가 열려 있는데 두 번째 Cycle을 염. 열린 Experiment를 둔 채 Interview를 염.
14. (B) 경로의 Experiment가 직전 시도와 계보 없이(다른 가지에서) 같은 항목을 바인딩함.
15. 재인터뷰 요청이 남은 채 Experiment를 염. 새 판에서 미도달 항목을 빼거나 정의를 바꿈(`plan.err.no_cancel` 유지).
16. 빈 증거·변하지 않은 증거·채팅만으로 Verify close(오늘과 같음).

### 7.4 실행 명령

```sh
cargo test --locked -p gil --features approved-plan,visual-authoring,product-safety --lib -- --test-threads=1
cargo test --locked -p gil --features approved-plan,visual-authoring,product-safety --test context --test context_profiles --test guidance_wave1 --test guidance_wave2 --test approved_plan_wire_wave1 --test chain_recovery_preservation
cargo test --locked -p gil
cargo build --locked -p gil --all-targets --features approved-plan,visual-authoring,product-safety
node --test plugins/gil-companion-prototype/skill.test.mjs plugins/gil-companion-prototype/workflow.test.mjs design-tests/m6e/m6e.test.mjs mcp-app/*.test.mjs
```

같은 `target/`에서 Cargo를 동시에 돌리지 않는다. 승인 프로필 시험 뒤 임시 폴더에 format 6 기록이 남으면 기본 프로필 Monitor
시험이 `UnknownFormat`으로 실패하므로 폴더를 지우고 재실행한다(구현 준비 문서 §6의 알려진 fixture 문제).

---

## 8. 다른 세션과 무관하게 유지하는 계약 경계

같은 시기에 진행 중인 **복구 자동 승인 입력 경로**(M4-R, `recovery_mode.rs`·Monitor 버튼)와 **인터뷰 기반 추적 제외**(`.gilignore`,
추적 정책·보존 범위) 작업과 이 결정안은 다음 경계로 갈라진다. 세션 이름이 달라도 같은 파일을 만지는지로 판단한다.

| 이 결정안이 의존하지 않는 것 | 이유 |
| --- | --- |
| 복구 승인 모드(수동/자동)와 설정 입력 경로 | (B) 경로는 파일을 만지지 않아 복구 승인이 필요 없다. 되돌아가기 경로는 어느 모드든 「이미 실패로 닫힌 실험의 복구」라는 기존 계약을 그대로 쓴다 |
| `.gilignore`·추적 정책·보존 범위 | 부분 결과의 보존은 Verify Snapshot(기존 whole-tree 계약)에 의존한다. 제외 파일이 Snapshot·보존본에서 빠지게 되면 중단 관측 문서가 「보존된 것」을 적을 때 그 범위를 따르면 되고 전이는 바뀌지 않는다 |
| 안내 문구 PR의 복구 표·결과 우선 안내 | 이 결정안은 새 키와 규칙 4·7의 해당 단락만 바꾼다. 기존 두 키(`guide.interrupt_partial_results`, `plan.err.no_cancel`)의 「아직 없다」 문장은 Core 전이가 들어가는 같은 PR에서만 고친다 |

| 이 결정안이 다른 세션에 요구하지 않는 것 | 이유 |
| --- | --- |
| 자동 승인을 열린 단계 종료·계획 변경·미도달 판정으로 확대 | 복구 계약 「선택과 적용 범위」가 금지한다 |
| 보존본을 작업 폴더로 되돌리는 명령 | (B) 경로가 그 필요를 없앤다. 복구 명령의 일부가 아니라는 계약 유지 |
| 저장 형식 번호·Chain 이전·동시 실험·Chain revisit | 범위 밖(§9) |

공유 파일 충돌 가능 지점은 `src/i18n/*.json`, `src/mcp/workflow.md`(+Skill 생성 영역), `src/context.rs`뿐이다. 병합 순서에 따라
Skill 생성 영역 재동기화가 필요할 수 있다.

---

## 9. 이 문서가 결정하지 않는 것 (범위 밖)

- 자동 복구 활성화·설정 입력 경로(M4-R 제안서), Chain 자체 되돌아가기, 동시 실험, 기존 기록의 Chain 이전, 일반 restore 해제.
- 진행 방식(§20)·응답 대기 재개(§20.1)의 저장·전환 명령.
- 새 칸의 이름·표기, 재시도 상한 수치, 새 도구·새 상태. 모두 미확정이며 이 문서는 그것들을 확정된 것으로 적지 않았다.
- 실제 모델 평가·Host 화면 인수·설치본 게시·필드 테스트(사용자가 preview.31로 직접 진행).
