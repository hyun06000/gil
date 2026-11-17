# 승인 항목 분할·대체 계약 결정안 v0.1

상태: **사용자 결정 대기 · 설계 검토와 독립 모델 시험만 완료** (2026-11-17).
검토 기준: `bcefbc4` (PR #142 통합 후). 아래 권장안·가칭 필드·동작은 제품 계약이 아니다.
기존 중단 결정안의 권장안을 승인된 사실로 사용하지 않는다. 이번 변경은 이 문서와
`design-tests/approved-item-split/`에 한정하며 규범·제품 전이·저장·도구·UI를 변경하지 않는다.

## 0. 사용자가 결정할 세 질문

| 질문 | 권장안 | 대안 | 사용자가 경험하는 차이 |
| --- | --- | --- | --- |
| **Q1. 기존 A의 실행 의무를 새 항목으로 넘길 수 있는가?** | R: A의 정의·승인·시도는 보존하고, 새 판의 명시적 `A → [A1,A2]` 관계로 앞으로의 실행 의무만 교체 | S: 현상 유지 / H: A를 부모 목표로 유지하고 자식 완료를 집계 | R에서는 A가 ‘성공’으로 바뀌지 않고 ‘대체됨, 과거 결과는 미실행/실패’로 보인다. A1·A2는 새 승인 뒤 각각 검증한다 |
| **Q2. 뒤의 B가 A의 결과를 기대할 때 얼마나 자동으로 이어갈 것인가?** | R의 좁은 직렬 규칙: 새 항목들은 A의 자리를 이어받고 B보다 먼저 수행. B의 입력 전제가 유지되는지 새 제안에서 확인 | 분할을 전부 보류 / 별도 의존성·충족 관계 설계까지 확장 | 순서를 자유롭게 바꿀 수 있는 범위가 줄어든다. B의 문제·완료 조건도 달라지면 B를 원문 수정하지 않고 별도 새 ID로 대체하는 승인이 필요하다 |
| **Q3. 부분 수행 A 뒤 재계획의 첫 제공 범위는?** | 기존에 합법적인 Interview 경계부터 R을 제공. 파일을 둔 채 실패 아래 Interview를 여는 경로는 M6-E 결정·검증 뒤 연결 | M6-E와 R을 함께 확정·구현 / 모두 현상 유지 | 초기에는 실패 뒤 보존·승인 복구를 거쳐 분할한다. 현재 폴더의 부분 파일을 그대로 둔 재계획은 아직 제공하지 않는다 |

세 권장안을 고르더라도 필드 이름·저장 호환 방식과 실제 구현은 후속 검토 대상이다.
별도 취소, 제3 verdict, 자동 완료, 병렬 실행, 일반 작업 관리 시스템은 제안하지 않는다.

## 1. 현재 계약과 코드에서 확인한 경계

아래 행 번호는 모두 `bcefbc4` 기준이다. 코드 관찰과 규범/안내를 구분한다.

| 허용/거절·관찰 | 근거 위치 |
| --- | --- |
| 새 제안은 열린 Interview Synthesis에서만 가능. 기존 항목 누락·question/done_when 변경은 거절. 항목 추가·순서·scope 변경은 새 승인으로 가능 | [approved_plan.rs](../../src/approved_plan.rs) `retained_items` 86행, `validate` 220행, `propose_plan` 484행 |
| 제안의 판·기준 판·Synthesis 주소·draft 전체가 digest에 묶임. 동일 최신 제안 재전송은 같은 제안. 편집하면 새 판/receipt 없음. 이미 받은 응답은 뒤집지 못함 | 같은 파일 `Proposal::hash` 116행, `propose_plan`, `decide_plan` 541행. `tests.rs::editing_an_approved_open_proposal_preserves_old_receipt_but_requires_new_consent` |
| 버튼 승인은 실행 활성화가 아님. 성공적으로 닫힌 Interview의 마지막 Outcome과 연결된 승인 Synthesis만 활성화 | `PlanState::activated` 155행, `check_plan_close` 567행. [계약](../GIL_Approved_Plan_v0.1.md) 「승인과 실행」 |
| pending은 최신 승인 items 중 미바인딩 또는 최신 시도가 닫힌 failure인 항목. 열린 시도는 pending에서 빠지지만 완료 증명이 아님 | `failed_execution` 111행, `pending` 188행, `next_plan_execution` 582행 |
| retry는 같은 ID·같은 정의를 새 Cycle에 바인딩. 직전 실패로 이어지는 revisit 계보 필요. 새 Interview가 중간에 있어도 계보를 지우지 않음 | `validate` 343–405행, [M4-R](../GIL_Revisit_Recovery_v0.1.md). 새 retry 횟수 제한 없음 |
| `current_item`은 현재 Cycle에 저장된 과거 판의 바인딩. 닫혀도 남음. `retry_pending`은 pending 중 이전 실행이 있는 항목 | `current_plan_item` 629행, [plans.rs](../../src/mcp/plans.rs) 88–95행. 열림은 `current_cycle.state`로 판독 |
| 작은 Define으로 A 일부만 수행해 전체 완료로 세면 안 됨. 현재 Core는 자연어의 범위 일치를 자동 증명하지 않음 | [Cycle Model §7](../GIL_Cycle_Model_v0.1.md#7-experiment-cycle의-define), [Approved Plan](../GIL_Approved_Plan_v0.1.md) 「실행 항목의 크기와 Define의 일치」. `pending`은 의미적 완료 검사기가 아님 |
| 새 실행의 Verify는 바인딩된 계획 문서 bytes와 별도 신선한 실제 작업 증거가 필요. 옛 증거만 재인용해서 통과하지 못함 | [evidence.rs](../../src/session/evidence.rs) `require_verify_evidence` 11–57행, `link_learning` 61행. [시험](../../src/approved_plan/tests.rs) `verify_requires_bound_document_and_separate_fresh_execution_evidence` |
| 재인터뷰 요청은 기록만 남김. 열린 Cycle을 자동 종료하거나 자식 Interview를 열지 않음. 미해소 요청은 다음 Experiment를 막음 | `request_plan_interview` 612행, `awaiting_interview` 205행, `next_plan_execution`; [cycles.rs](../../src/cycles.rs) `why_not_open_child` |
| 실패 뒤 방향은 revisit. 부분 결과를 폴더에 둔 failure→Interview는 아직 없음 | [구현 준비 §1](M6E_Interruption_Implementation_Prep_v0.1.md), [중단 결정안 §1](Chain_Interruption_Decision_Pack_v0.1.md). `gil-spec.yaml` experiment 조건표, `cycles.rs::check_next_direction` |
| Chain 종료는 현재 Closing Interview의 정확한 승인 Synthesis·판정·비어 있지 않은 handoff_summary로 검사. pending=0 검사나 항목 전체 success 검사는 없음 | [chain.rs](../../src/chain.rs) `check_report` 98–140행. 추가 Chain Report 필드는 거절 |
| 복원도 승인·정의·실행 순서·계보를 재검증 | [store.rs](../../src/store.rs) `StoredState::into_project` 373행 → [chain.rs](../../src/chain.rs) `validate_state` 143행 → `PlanState::validate` |

읽은 계약: [Roadmap §2·§11](../GIL_Roadmap.md), [Approved Plan](../GIL_Approved_Plan_v0.1.md),
[Cycle Model §7·§8·§16·§21](../GIL_Cycle_Model_v0.1.md), [Chain Storage](../GIL_Chain_Storage_v0.1.md),
두 M6-E 문서와 [기존 독립 모델](../../design-tests/m6e/README.md).
기존 M6-E 모델은 `conclusion`, `attempt/retry_of`, 재시도 상한을 포함한 **다른 미채택 후보**다.
그 시험 통과를 최신 중단 결정안이나 이번 R 후보의 전이 근거로 사용하지 않는다.

## 2. 세 대안과 권장 비용

| 후보 | 표현과 실행 | 장점 | 비용·미해결 |
| --- | --- | --- | --- |
| **S — 현상 유지** | A 원문 유지, 추가·재정렬만. retry는 같은 A | 저장/전이 변경 없음 | A1·A2를 추가해도 A의 의무가 남는다. 일부만 수행한 A 성공은 금지. 이미 너무 큰 항목의 분할 문제를 해결하지 못함 |
| **R — 명시적 의무 교체(권장)** | 새 판에 A와 새 ID들의 원문을 모두 보존하고 `A → [A1,A2]` 관계 추가. A는 이후 큐에서 제외, 새 항목은 각자 바인딩 | 실행·과거 정의·증거·승인을 재작성하지 않음. split과 1:1 replacement를 같은 한 종류 관계로 처리 | 승인 digest·큐·복원 검증·표시의 변경, 구형 writer 거절, 누적 항목 수 증가, 보수적 순서 제한. ‘A 완료’ 대신 ‘A 대체’라는 판독 필요 |
| **H — 부모 항목과 하위 실행 항목** | A를 현재 목표로 유지하고 A1·A2의 충족을 집계 | 원래 목표 진행률을 보여 주기 쉬움 | 부모/자식 역할, 일부 완료, 집계 성공 조건, 중첩·retry·부모 바인딩 계약이 필요. 자식 성공이 A 전체 성공이라는 오인을 막기 위한 추가 증명 필요 |

R은 ‘대체’와 ‘성공’을 분리하는 최소 후보다. split인지 대안 접근인지 별도 enum은 두지 않고
이유·범위 설명에서 구분한다. 새 항목은 **앞으로 수행하고 검증할 남은 문제**를 나타낸다.
과거 수행분은 증거와 함께 설명하되 A나 A1의 성공 기록으로 옮겨 쓰지 않는다.
부분 결과로 이미 만족한다고 생각되는 A1도 필요한 경우 새 검증 Cycle을 거친다.
단순 재포장·체크박스용 Cycle은 만들지 말고, 그 확인 자체가 의미 있는 최소 문제인지 검토한다.

## 3. R 후보: 최소 관계와 불변 기록

아래 이름은 모두 **가칭**이다. 제품 입력 schema나 저장 필드를 이번에 추가하지 않는다.

```text
Proposal r2 (base_revision=r1, 현재 Interview Synthesis)
  items: A(원문), A1(새 ID), A2(새 ID), B(원문)
  replacements: [{ source: A, targets: [A1,A2], reason: ... }]
  synthesis: 이전 수행/미검증 범위, 잔여 범위 대응, B의 입력 전제, 증거 참조
```

- 제안 전체의 기존 `items`는 이력 보존 목록도 겸한다. 새 판에서도 모든 과거 활성 항목의
  id/question/done_when을 유지한다. 누락을 대체로 추론하지 않는다.
  현재 `exploration_checklist == items의 ID 순서` 검사는 유지하는 후보다. 따라서 전체 목록에는
  대체 source도 남고, 실행 대상은 관계를 적용한 유효 목록에서만 얻는다. 승인 화면·export는
  전체 목록과 실제 실행 순서의 차이를 관계와 함께 보여 줘야 한다.
- 관계는 같은 Chain의 항목 ID를 참조한다. 기준 승인 판과 Chain은 기존 Proposal/컨테이너에서
  결정하므로 관계마다 별도 Chain ID·판·날짜·상태를 중복 저장하지 않는 후보다.
- 과거 승인된 관계는 이후 draft에도 원문 유지한다. 관계 도입 판은 처음 나타난 활성 판으로
  유도한다. 과거의 미승인/거절 draft는 활성 관계의 원천이 아니다.
- source는 기준 활성 판에서 아직 대체되지 않은 항목, 최신 결과가 success가 아닌 항목이어야 한다.
  열린 실행이 있으면 Interview 경계에 도달할 수 없으므로 적용 불가. 성공 A는 추가 작업으로 처리한다.
- targets는 1개 이상, 같은 제안의 새 ID들이다. 과거 활성 판에 있던 ID의 재활용, 자기 참조,
  다른 Chain/없는 ID, 중복 source, 중복/공유 target, 관계 순환을 거절한다.
  하나의 새 target을 같은 판에서 다시 source로 쓸 수 없다. 다음 활성 판에서 다시 나누는 것은 가능하다.
- 0개 target은 취소이므로 범위 밖. 여러 source를 하나의 target으로 합치는 병합도 범위 밖.
  서로 다른 source를 각자의 새 항목들로 교체하는 것은 한 판에서 검토 가능하다.
- old A의 CycleRef, Execution.revision/item_id, 고정 Define, Report, Snapshot, 시각 증거,
  Journey와 기존 승인 문서 bytes는 그대로다. 새 관계는 승인 이력에 추가할 뿐이다.
- ‘대체됨’은 활성 관계에서 계산하는 표시이지 Cycle verdict·저장된 item state가 아니다.
  A1/A2가 성공해도 A의 과거 미실행/failure는 그대로다. `Execution`에 부모·완료율·retry 번호를 추가하지 않는다.

## 4. 무엇을 새로 승인하는가

사용자는 새 scope 전체, 항목 정의·순서, 대체 source/targets와 이유, 제외·남은 범위,
부분 결과의 이용 계획, 후속 항목의 전제를 **같은 제안 판**으로 확인한다. 관계가 digest 밖에 있으면 안 된다.
scope가 달라졌다면 그 차이도 명시한다. 사람에게 보여 준 원문과 실행에 쓰는 원문을 일치시킨다.
이전 승인은 과거 실행과 당시 범위의 근거로 계속 유효하지만 새 ID·관계·새 범위의 실행을 허가하지 않는다.

승인 응답 → 일치하는 Synthesis close → Outcome → Interview success가 모두 끝난 뒤에만
새 판과 관계가 활성화된다. 버튼을 누른 직후에도 r1이 활성이고 Interview가 열려 있으므로 실행 불가다.
새 원자적 저장에서 Interview close와 활성 투영이 함께 성립해야 하며 별도 ‘대체 적용’ 명령은 권장하지 않는다.
기존 계획 제안/승인 경로를 확장하는 후보이고 별도 split 명령은 미제안이다.

거절은 현재 `request_changes` 의미를 유지한다. 과거 활성 판·실행·증거·pending은 그대로이며
거절 receipt만 남는다. 열린 Interview를 버리거나 옛 Experiment를 자동 재개하지 않는다.
같은 범위 유지가 답이라면 추가 Question/Synthesis로 이를 확인하고 유효한 승인으로 Interview를 닫는다.
제안 편집은 새 revision/digest와 새 응답을 요구하며 앞선 제안·거절·승인 receipt는 보존한다.

## 5. 부분 결과 재사용과 완료 판정

재사용은 **입력·증거의 재참조**다. 과거 Verify/파일/Snapshot 참조와 의미, 접근 가능 여부,
현재 세계에서 달라진 조건을 새 Define/Hypothesis/Verify 보고서에 설명한다. 기존 파일을
새 파일처럼 복제하거나 계획 문서만 제출하여 신선한 실행 증거로 꾸미지 않는다.
새 항목의 완료에는 그 항목과 일치하는 Define, 실제 검증, 새 관측 증거,
Analysis/Outcome/Cycle Report가 필요하다. 기존 `plan_document`와 증거 연결 규칙도 유지한다.

현재 파일이 그대로 사용 가능하더라도 새 완료는 자동 인정하지 않는다. 재검증 결과를 담은
새 관측 문서는 유효한 증거가 될 수 있지만 빈 문서나 과거 보고서 복사는 실질 검증이 아니다.
자연어 범위의 충분성은 Core가 자동 증명하지 않는다. 안내·실제 모델/사용자 평가가 별도로 필요하다.

파일 보존과 항목 관계는 다른 계약이다. 지금의 실패→승인 revisit은 Snapshot·보존본을 남기지만
작업 폴더를 조상 Exit으로 돌린다. 보존본을 작업 폴더에 다시 넣는 것은 복구 명령의 일부가 아니며,
이 문서는 추출/복원 명령을 추가하지 않는다. 부분 결과를 찾을 수 없으면 이용 가능하다고 기록하지 않는다.

## 6. 순서·pending·retry_pending·current_item

가칭 ‘유효 항목’ = 최신 활성 판의 items에서 활성 관계의 source를 제외한 목록이다.
pending은 이 목록에 **기존의 최신 실행 없음/닫힌 failure** 조건을 적용한다.
retry_pending은 그 pending 중 이전 실행이 있는 항목이다. 대체된 실패 A는 둘에서 빠지지만
실행 이력과 실패 보고서에서는 사라지지 않는다. A1의 첫 시도는 A의 retry가 아니다.
A1 실패 뒤 같은 A1 재시도에는 기존 revisit 계보 검사가 그대로 필요하다.

`current_item`은 현재 Cycle의 실제 바인딩과 그 판에서 읽는다. A가 닫혔어도 현재가 그 Cycle이면
A이며, Interview면 null, 새 Experiment가 열리면 A1이다. 최신 관계로 과거 Cycle의 바인딩을 바꾸지 않는다.
실행 요청과 loader는 요청 판이 그 Cycle 시점의 최신 활성 판인지, 그때의 첫 pending인지 재검증한다.
**과거 Execution의 순서를 최신 대체 관계로 다시 해석하면 안 된다.**

R의 Q2 권장은 보수적인 직렬 제한이다.

1. 관계를 도입하는 판은 기준 유효 순서에서 source 자리에 targets를 삽입한다.
   나머지 기존 유효 항목의 상대 순서는 유지한다. `[A,B] → [A1,A2,B]`다.
   일반 새 항목 추가는 가능하되 기존 항목과 targets의 이 상대 순서를 깨지 않는다.
2. 도입 시 A 뒤에 있던 B 등 모든 항목을 보수적으로 영향 대상으로 취급한다.
   이후 재정렬도 A의 대체 말단들이 B(또는 B의 대체 말단들)보다 앞서야 한다.
   이 제한은 관계의 도입 판·기준 판에서 유도하여 별도 dependency 필드를 저장하지 않는다.
3. 새 항목 실패 시 첫 pending으로 돌아오므로 뒤의 B를 건너뛰어 실행하지 않는다.
   A1을 다시 A11/A12로 나눠도 순서 제한을 말단까지 전파한다.
4. 제안 본문에서 B의 입력을 누가 어떤 결과로 제공하는지 확인한다. 순서는 데이터의 의미를
   보증하지 않는다. 전제가 미확인인 분할안은 승인 준비가 끝난 것으로 보이지 않는다.
   B 정의가 바뀐다면 B 원문을 유지하고 새 B1로 교체하는 관계를 같은 판에 포함할 수 있다.

이는 기존의 자유로운 재정렬을 **대체 관계가 있는 구간에서 좁히는 새 후보 규칙**이다.
독립적인 B까지 기다리는 비용이 있다. 선택적으로 제한을 해제하는 명령이나 의존성 DAG는 이번에 만들지 않는다.
이 비용을 수용하지 않는다면 Q2를 미결로 남기고 R 구현을 보류해야 한다.

## 7. A–G 사례별 판독

| 사례 | 현재 계약 | R 선택 후의 최소 흐름 |
| --- | --- | --- |
| A. 미실행 A | 삭제·재정의 불가. 추가만으로 A 의무를 없애지 못함 | 합법적 Interview → A 원문+A1/A2+관계 승인 → Interview 닫기 → A1/A2 실행. A는 미실행·대체됨 |
| B. 일부 수행 A | 열린 Cycle 먼저 사실대로 종료. failure 뒤 승인 복구·revisit 필요 | 실패 기록·증거 보존 → 기존 복구 → Interview에서 남은 범위·재사용 검토 → 분할. 폴더 유지 경로는 M6-E 별도 |
| C. 실패 A 재시도/대체 | 같은 A retry는 현재 가능. 다른 항목으로 대체는 불가 | 같은 정의/ID retry는 기존 승인·계보 유지. 다른 질문/완료 기준은 새 ID·새 승인·관계. 대체 target에 A의 실패를 옮기지 않음 |
| D. 성공 A 뒤 추가 | 원문 유지하고 D 추가·새 승인 가능 | 같은 경로 사용. 성공 A를 대체 source로 쓰지 않음. 성공 후 불충분한 기준 발견도 과거 success를 고치지 않고 새 검증 항목으로 명시 |
| E. 거절/편집 | 요청 변경 receipt·새 proposal 이력 보존 | 활성 관계·큐 무변경. 편집 후 새 승인 필요. 거절을 취소나 옛 실행 자동 재개로 처리하지 않음 |
| F. 승인 직후 끊김 | receipt와 실행 활성 판은 다름 | 읽기 후 정확한 단계 재개. Interview 미종료면 새 항목 실행 불가, 종료 저장 완료면 새 관계 큐로 재개 |
| G. B가 A 결과에 의존 | 의미 의존성은 본문·순서로 설명; 자동 증명 없음 | A1/A2를 먼저 검증하고 B 입력 충족을 확인. B 정의 변경은 별도 B→B1 승인. 불명확하면 열린 Interview에서 계속 질문 |

## 8. 종료와 Chain Report

R은 Chain 종료를 자동화하지 않는다. `pending=[]`이어도 열린 실행이 있을 수 있고,
대체 source가 실패/미실행일 수 있으므로 이를 ‘전부 성공’으로 표시하지 않는다.
현재 Closing Interview의 인간 판정, 정확한 승인 Synthesis 참조, clean 세계,
단일 열린 Cycle 제약과 Chain Report 필드 제한을 유지하는 후보다.

종료 제안은 현재 scope의 성공 기준, 대체된 원래 의무와 사유/승인 판, 새 항목들의 실제 결과,
검증하지 못한 범위·남은 유효 항목, 재사용한 부분 결과와 위치를 먼저 보여 준다.
남은 항목이 있으면 왜 종료 가능한지 사람이 판단한다. ‘모든 말단 success’를 새로운
기계적 Chain 성공 조건으로 추가하지 않는다. 실패 종료도 사용자 승인으로 가능하다.
scope 축소·변경을 원래 scope의 성공으로 표현하지 않는다.

추가 Chain Report 필드를 만들지 않고 `handoff_summary`에서 참조와 미완료 사실을 압축한다. 예:

> r2에서 A(실패, cycle:C2)를 A1/A2로 대체 승인했다. A의 원래 전체 완료는 확인하지 않았다.
> A1은 성공, A2는 미완료이며 B 입력은 확보하지 못했다. 부분 결과는 C2의 Verify Snapshot에 있다.
> r3 Closing Synthesis에서 현 범위의 탐색 종료를 failure로 승인했다. 다음 Chain은 새 승인이 필요하다.

관계 말단이 모두 성공한 경우에도 ‘대체 계획의 항목들이 성공’이라고 적고 원래 A의 성공 Report를 생성하지 않는다.

## 9. 재시작·중복·잘못된 참조

| 상황 | R의 요구사항 |
| --- | --- |
| 동일 최신 proposal 재전송 | 같은 제안을 반환. source를 한 번 더 교체하지 않음 |
| 동일 revision/digest/응답 재전송 | 열린 해당 Synthesis에서 같은 receipt. 다른 응답이나 낡은 판은 거절 |
| 기준 판이 낡음 | 최신 활성 판에서 다시 제안. 자동 rebase·관계 재매핑 금지 |
| 승인 receipt 저장 뒤 연결 단절 | 조회로 receipt 확인. capability 재발급은 기존 경로. 자동 재승인/자동 실행하지 않음 |
| Interview close 저장 전/후 단절 | 저장 전이면 옛 활성 판+열린 Interview, 후면 새 활성 판. 재조회 후 재개. 이미 닫힌 것을 재close하여 중복 적용하지 않음 |
| 새 Experiment open 응답 유실 | current Cycle·binding 조회. 중복 open은 단일 열린 Cycle gate로 거절 |
| 관계 중복·순환·잘못된 ID·타 Chain | 제안과 load에서 전체 거절. 자동 정리·일부 적용 없음 |
| 관계나 과거 실행을 저장 파일에서 변조 | 관계 도입 판, 당시 활성 판, 순서·완료 출처를 재검증. 최신 투영만 맞추는 복원 금지 |
| 파일 보존/관측/저장 실패 | 기존 열린 상태와 오류 유지. 관계 승인으로 증거 누락·dirty·실패 복구 조건을 우회하지 않음 |

저장은 기존 프로젝트 잠금과 state 단위 원자적 저장을 활용하는 후보다(`store.rs::save`,
시각 형식은 `visual_report::state_file::write`). 실제 디스크·전원 단절 내구성은 플랫폼별 별도 시험이다.
이번 모델의 JSON 재개는 그 보장을 증명하지 않는다.

## 10. format 4/5/6/7과 구형 reader/writer

**실제 확인한 코드와 앞으로 검증해야 할 주장을 분리한다.** 구형 실행 파일은 이번에 실행하지 않았다.

| 대상 | 현재 코드 근거 | 후보 영향·후속 입증 |
| --- | --- | --- |
| 4/5 | `store.rs::declared_format`, `into_project`: Chain 없는 구조. 자동 Chain 생성 없음 | R 적용 대상 아님. 읽기/재저장과 기존 digest 불변을 Rust golden fixture로 검증해야 함. 자동 이전 없음 |
| 관계 없는 6/7 | Chain/approved_plan 유무를 검증하며 opt-out 기록은 opt-out 유지 | R 구현 후에도 기존 바이트·계획 export·digest를 유지해야 함. 새 필드 기본값 직렬화가 이를 깨뜨릴 수 있음 |
| 관계 있는 6/7 후보 | `Draft`, `Proposal`, `PlanState`가 `#[serde(deny_unknown_fields)]` (`approved_plan.rs` 24·46·71행 부근) | 가칭 `Draft.replacements`를 넣으면 **bcefbc4 loader는 unknown-field로 거절할 것으로 코드상 판단**. 호환해서 실행 가능하다는 뜻이 아님. 후속 실제 구형 decoder/실행 파일 시험 필요 |
| chain-core 없는 빌드 | 6/7을 format gate에서 거절(`store.rs` 267행) | 유지. 번호를 낮추는 우회는 구조 검사에서도 거절 |
| approved-plan 없는 빌드 | `PlanState::validate`의 기능 gate | 장부 있는 기록은 거절. 관계 생략으로 다운그레이드하지 않음 |
| Report 본문에만 관계를 숨기는 대안 | Report는 키-값 map인 반면 장부는 엄격 typed 구조 | 구형 reader가 본문을 읽고도 A를 계속 실행할 수 있어 R의 기계 의미 저장에 부적합. 본문은 설명용이며 관계 진실원은 아님 |

권장 저장 후보는 6/7의 선택적 typed 확장으로 구형 writer가 **안전 거절**하게 하는 방식이다.
새 format 번호가 필요한지, 정확한 확장 위치·이름·제약은 아직 미확정이다. 호환성이란
‘옛 기록을 새 빌드가 보존하여 읽는다’와 ‘새 기록을 구형 빌드가 실행한다’를 구분해야 한다.

특히 기존 hash는 정규화된 Rust 직렬화 tuple 전체를 사용한다(`Proposal::hash`). 빈 관계를
새로 직렬화하면 옛 digest와 export bytes가 달라진다. 채택 시 필드 부재/빈 목록의 직렬화 생략과
hash 입력을 명시하고 옛 fixture의 digest·문서·재저장 bytes를 고정해야 한다. JS 모델 hash는
설계용 정렬 JSON이며 Rust hash·YAML 형식과 호환하지 않는다.
관계를 삭제해 구형 writer에 맞추는 변환은 의미 손실이므로 제공하지 않는다.

모든 과거 항목을 유지하면 현재 64 items/64KiB draft 제한(`Draft::validate`)에 빨리 닿을 수 있다.
이번에는 상한 확대·이력 압축·항목 ID 회수·legacy 이전을 설계하지 않는다. 제한에 닿으면 명시적으로
막힘을 알리고 종료/다음 Chain의 기존 승인 경로를 검토한다. 다음 Chain은 이전 승인을 승계하지 않는다.

## 11. M6-E 의존 관계와 미결 선택

- 미실행 A의 분할, 성공 A 이후 D 추가, 기존 승인 복구 뒤 실패 A 분할은 **새 M6-E 전이가 필요 없다**.
  단, 분할 자체는 R의 새 계약·구현 없이는 현재 불가능하다. ‘기존 경계에서 가능’과 ‘현재 제품 지원’을 구분한다.
- 열린 A는 현재 Report·증거 조건으로 사실대로 닫아야 한다. 닫을 수 없으면 기다림/막힘 유지다.
- 실패 A의 부분 파일을 현재 폴더에 둔 채 Interview로 이동하려면 기존 중단 결정안 Q1(결론 미도달
  구분)·Q2(Interview 자식 허용)가 먼저 결정·구현되어야 한다. 그 아래에서 **같은 ID retry**까지
  허용하려면 Q3의 계보 확장도 필요하다. 새 ID replacement라고 해서 그 전이 gate를 우회하지 않는다.
- 기존 중단 결정안만 채택해도 항목 대체는 해결되지 않는다. 그 문서는 `no_cancel`과 기존 정의 보존을 유지한다.
  반대로 R 채택은 `conclusion`, 새 failure 자식, `revise_will`, 재시도 상한을 승인하지 않는다.
- 기존 `design-tests/m6e`는 수정/연결하지 않는다. 이번 모델은 기존 성공 경계 또는 **이미 완료된 기존
  M4-R 복구**를 진입 전제로 쓰며 실패 직후 Interview를 거절한다. 다른 후보의 전이를 섞지 않는다.

남겨 둔 세부 선택: 저장 이름/확장 위치와 format gate 방식, 관계 없는 기록의 직렬화 규칙,
자유로운 의존성/순서 해제, 성공 항목의 취소·재정의, 0개 대상 취소, 여러 source 병합은 미확정 또는 범위 밖이다.
Q2의 보수적 순서 제한을 채택하지 않는다면 이번 모델이 더 자유로운 대안까지 검증한 것으로 해석하지 않는다.

## 12. 독립 설계 시험과 한계

[모델·실행 설명](../../design-tests/approved-item-split/README.md),
[모델](../../design-tests/approved-item-split/model.mjs), [시험](../../design-tests/approved-item-split/model.test.mjs).
R + Q2 직렬 제한 + 현재의 복구 경계 **한 조합만** 시험한다. S/H는 문서 비교만 한다.

검사: A–G, 과거 제안/receipt/바인딩/닫힌 결과 불변, 승인 전과 Interview 종료 전 실행 금지,
부분 수행/옛 증거만으로 새 성공 거절, 관계 중복/순환/잘못된 참조 거절, 거절 상태 보존,
재시작·중복·낡은 판, 재분할과 B 순서, 손상된 모델 기록, 제한된 사건 조합 8개.

모델의 `fullCoverage`는 사람이 검토한 의미적 충족 여부를 시험에 주입하는 **합성 oracle**이다.
Core가 자연어 충족을 판별한다는 증거가 아니다. `recovered`는 외부 M4-R가 완료됐다는 전제이며
승인·파일 복원 구현이 아니다. `activate`는 실제 세 단계 close를 추상화한다.
JSON journal/replay는 모델 무결성 비교용이며 제품 저장 장부/새 명령을 제안하지 않는다.
파일 bytes·Snapshot·Will·Journey·실제 Step grammar·잠금·Host UI·물리적 인간 승인은 시험하지 않는다.

검증 명령과 결과(2026-11-17):

```sh
node --test design-tests/approved-item-split/*.test.mjs
git diff --check
```

새 Node 시험 **27 통과, 0 실패**. `git diff --check`도 통과했다.
의존성은 Node 표준 라이브러리뿐이며 npm/Cargo 설치·빌드는 필요하지 않았다.
제품 코드 변경이 없어 Core/기존 JS/패키징/Tauri 시험은 실행하지 않았다. 기존 시험 통과나
제품 구현·호환성·실제 화면 인수를 이번 결과로 주장하지 않는다.

## 13. 승인 후 구현 순서와 PR 설명 초안

1. Q1–Q3 결정과 미결 저장 세부를 기록하고, 별도 PR에서 규범·Approved Plan·저장 계약을 맞춘다.
   M6-E의 미도달/폴더 유지 전이를 선택했다면 그 규범 결정과 경로 검증을 별도로 선행한다.
2. golden fixture로 옛 4/5/6/7·digest·export 보존과 구형 writer 거절부터 고정한다.
   그 뒤 관계 검증/활성화/시점별 큐 재생/실행 바인딩/loader를 같은 계약으로 구현한다.
3. 정확한 관계와 승인 범위 표시, pending/retry/current 투영, 미완료·대체 종료 안내를
   Core와 함께 연결한다. 제품 전이와 ‘아직 불가’ 안내가 불일치하지 않게 한다.
4. 실제 state commit 장애·재시작·낡은 요청·손상 참조·복구 보존을 Rust/stdio로 시험한다.
   기본 Core와 승인 프로필, 관련 JS·패키징 검사 후 실제 Host 버튼·판독·모델 행동을 별도 인수한다.
5. push/CI dispatch/병합/설치/배포는 별도 행동으로 남긴다. 이번 작업은 수행하지 않는다.

**PR 제목 초안:** `docs: propose approved item split/replacement contract and isolated design tests`

**PR 본문 초안:**

> 현재 승인 계획은 기존 항목의 누락·정의 변경을 거절하여, 이미 승인된 큰 항목을 작은 항목으로
> 재계획할 수 없다. 과거 승인·실행·부분 증거를 보존하는 현상 유지/명시적 대체/부모 집계의
> 세 후보를 비교하고, 새 승인 판에서 실행 의무만 교체하는 후보와 사용자 결정 질문 세 개를 제시한다.
>
> 독립 Node 모델은 권장 후보 하나의 기록 불변·승인 경계·부분 완료 오인 방지·관계 검증·재개·후속 순서를
> 검사한다. 기존 M6-E 권장안은 미승인으로 유지하고 새 제품 전이·필드·명령·규범을 구현하지 않는다.
> 변경은 새 결정안과 `design-tests/approved-item-split/`뿐이다.
>
> 검증: 새 Node 시험 27 통과. `git diff --check` 통과.
> Rust/제품 저장 호환·실제 파일 복구·Host UI는 미검증이며 이 PR의 완료 범위가 아니다.
