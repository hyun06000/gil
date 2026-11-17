# M6-D — 승인된 계획 개발 계약

상태: 개발 프로필 `approved-plan` (`chain-core` 포함). 기본 배포에 자동 활성화하지 않는다.
Cycle Model §12, §16–17, §21의 미정 저장 계약을 이 문서로 한정한다.

## 승인과 실행

**2026-11-15 진행 방식 후속 요구:** [Cycle Model §20](GIL_Cycle_Model_v0.1.md#20-진행-방식과-사람의-확인-지점)에
따라 주요 지점 확인을 기본으로 시작 Interview에서 방식을 선택하고 확인할 결과를 계획에 명시한다.
작업 중 변경은 기존 승인 판·재인터뷰 경계와 정합해야 한다. 자율 진행은 승인된 범위 안에서의
진행이며, 계획 변경·최종 종료·복구 자동 승인을 대신하지 않는다. 확인 지점과 방식 변경의 저장·
승인 계약은 아직 미결이며 아래 개발 구현에 이미 포함된 것으로 해석하지 않는다.

- 새 Chain의 `approved_plan`은 제안 이력과 실행 바인딩을 보존한다. 기존 format 4/5/6/7 프로젝트는 자동 변환하지 않는다. format 6/7의 선택적 확장 필드이며 이 확장을 모르는 기존 writer는 unknown-field 오류로 거절한다. 기능을 끈 새 writer도 거절한다.
- 열린 Interview Synthesis에서만 제안한다. 제안은 Synthesis 주소, 순번, 기준 승인 판, 범위(goal/in_scope/out_of_scope/constraints/success_criteria), 순서 있는 항목(id/question/done_when), approved를 제외한 전체 Synthesis Report를 묶는다. SHA-256은 이 정규 JSON 전체를 묶는다.
- 같은 내용 재전송은 같은 제안을 돌려준다. 열린 Synthesis의 편집은 새 제안이며 이전 판의 승인은 재사용하지 않는다. 이전 제안과 응답은 이력에 그대로 보존하고, 새 제안에 다시 응답하기 전에는 Synthesis를 닫을 수 없다. 수정 요청을 그대로 닫으면 다음 Question으로 간다.
- Monitor는 전체 제안과 판·digest를 표시하고 실제 버튼 클릭으로 승인/수정 요청을 전송한다. 승인 데이터는 정확한 scope·판·digest에 묶인다. 수정 요청은 approved:no로만 닫을 수 있다. 에이전트가 approved:yes만 제출해서 닫을 수 없다.
- App 전용 도구의 visibility는 인증이 아니다. 앱에만 전달되는 `_meta`의 256-bit 임시 capability를 추가로 검증한다. 토큰은 모델 응답/Context/저장소에 넣지 않는다. 서버 재시작 시 재발급한다. 이것은 정상 MCP Host에서의 입력 경로 구분이지 물리적 개인 신원 인증이나 악성 로컬 프로세스 방어가 아니다.
- 승인은 Step을 자동으로 닫지 않는다. 버튼 처리 성공 후 앱은 사용자 동작에 따른 계속 요청을 Host 채팅에 보내며, 전달 실패 시 저장된 승인과 별개로 수동 계속 안내를 보여 준다. 자동 재조회는 메시지를 보내지 않는다. 에이전트는 승인 결과와 동일한 Report로 Synthesis, Outcome, Interview Cycle을 닫는다. 성공적으로 닫힌 Interview의 마지막 승인 Synthesis만 실행 가능한 계획이 된다.
  - **2026-11-18 보완 — 계속 요청의 뜻을 응답과 제안에 맞춘다.** 계속 요청은 기록이 아니라 기록을 가리키는
    안내다. 어느 경우에나 `gil_plan_read`로 저장된 응답·정확한 제안·`next`를 먼저 읽게 하고, 기록과 메시지가
    다르면 기록을 따르게 한다. 저장된 응답과 그 제안의 `chain_verdict`에 따라 넷으로 나눈다.
    수정 요청은 승인이 아니다(`approved: no`로 닫고 다음 Question에서 묻는다. Experiment를 열거나 Chain Report를
    내지 않는다). 종료 제안(`chain_verdict`가 `success` 또는 `failure`)의 승인은 종료 절차로 이어진다(Synthesis →
    Outcome → `close_chain` Cycle Report → 승인된 판정을 `verdict`로 적은 Chain Report. Experiment나 새 Chain을
    열지 않고, Closing Interview 자신의 `success`를 Chain 판정으로 쓰지 않으며, Chain Report 뒤에는 사용자에게
    알리고 멈춘다). `chain_verdict`가 없는 제안의 승인은 실행 계획이다(Synthesis·Outcome·Interview Cycle을 닫고
    각 영수증의 다음 호출을 따라 Experiment로 간다). 화면이 가르지 못한 경우(알 수 없는 `chain_verdict` 값,
    읽을 수 없는 제안)는 다음 행동을 추측하지 않고 `next`를 따르게 한다. 첫 클릭·명시적 재전송·App을 다시 연
    뒤의 재전송은 같은 제안과 저장된 응답에서 같은 문장을 만든다. 저장 필드·승인 단계·도구 계약은 그대로다.
- Experiment를 열 때 현재 승인 판의 첫 미실행 항목을 자동 바인딩한다. 과거 항목 정의와 실행 Cycle을 고치지 않는다. 새 Interview는 항목 추가·순서 변경을 허용하지만 기존 항목의 누락·정의 변경은 거절한다(취소/대체 의미는 별도 계약).
- `[A,B,C]` 승인 후 A 실행, 다음 Interview에서 `[A,D,B,C]` 승인 시 pending은 `[D,B,C]`다. executed는 열렸다는 뜻이며 성공/실패는 연결된 Cycle Report가 진실원이다.
- 읽기 응답(`gil_plan_read`/`gil_plan_review`)의 `supported:true`에는 read-only `current_cycle {ref, state}`가 있다. `ref`는 실제 현재 Cycle의 typed ref, `state`는 실제 닫힘 여부에 따른 `open`/`closed`다. `current_item`은 Cycle이 닫혀도 남는 바인딩이므로 열림의 증거가 아니다. 이 필드가 없는 응답(옛 Core, `supported:false`)에서 Monitor는 열림을 추정하지 않고 미확인으로 표시한다. 이 필드는 저장·승인·입력 의미를 바꾸지 않는다.
- 열린 Experiment 중 `gil_plan_request_interview`로 범위 변경 이유를 보존한다. 열린 Step/Cycle은 그대로 두며, 현재 Cycle을 사실대로 닫고 다음 Interview에서 승인하기 전에는 추가 Experiment를 열지 않는다.
- 일반 destructive restore와 승인 없는 제품 revisit는 차단한다. 닫힌 실패의 재시도는 [M4-R 승인 복구](GIL_Revisit_Recovery_v0.1.md)를 따른다. 이전 실행 바인딩은 보존하고 직전 실패의 revisit 계보에서만 같은 항목을 다시 바인딩한다. 최신 실패 항목은 pending으로 돌아오며 읽기 응답의 retry_pending에 표시한다. 취소·abort는 여전히 M6-E 별도 계약이다.
- 2026-11-15 복구 계약에 수동/자동 승인 모드를 명세화했다(자동 모드 구현 대기). 사용자가 설정한
  자동 모드는 닫힌 실패의 복구 승인에만 적용한다. 이 문서의 계획 제안·수정·종료 승인과
  추적 범위 변경을 자동 승인하지 않으며, 자동 복구를 실제 사람의 버튼 클릭으로 기록하지 않는다.

## 심층 인터뷰와 간결한 승인 제안

**2026-11-17 사용자 결정, 후속 구현·평가 요구.** 인터뷰에서 사용자의 모호한 요구를 구체적인
상황·결과·범위·제약·완료 조건으로 확인한 뒤 제안한다. 근거와 질문의 운영은
[Cycle Model §5](GIL_Cycle_Model_v0.1.md#51-심층-인터뷰로-요구를-구체화한다)를 따른다.
제안은 긴 배경 문서 대신 목표와 결정할 명제, 순서 있는 작은 실행 항목을 먼저 제시한다.
상세 설명은 참조로 연결하되 scope의 다섯 요소와 필수 근거·승인 대상은 보존한다.
화면의 요약도 정확한 제안 판·전체 승인 범위를 오인하게 만들면 안 된다.

### 실행 항목의 크기와 Define의 일치

**2026-11-17 후속 사용자 결정 — 요구와 탐색 실험의 분리.** 이 결정은 아래의 기존 구현 제약을
대체할 설계 방향이며, 제품의 항목 바인딩·실행 순서·저장 형식은 아직 변경하지 않았다.

- Interview는 답할 질문, 원하는 결과, 포함·제외 범위, 권한·제약, 판단 기준을 확립한다.
  정답이나 전체 실험 절차를 미리 확정하지 않는다. 탐색적 데이터 분석(Exploratory Data Analysis,
  EDA)에서는 발견할 패턴·원인·결론을 정해 놓고 그것을 입증하도록 요구하지 않는다.
  탐색 목적과 자료 범위, 근거의 품질, 불확실성·한계의 보고 기준은 명확히 합의한다.
- 하나의 승인 요구에 여러 작은 Experiment가 기여할 수 있다. Define의 제목·문제·완료 범위를
  승인 항목과 동일하게 만들 필요는 없다. 각 Define은 어떤 승인 요구에 기여하는지와 지금
  확인할 가장 작은 문제·관측 가능한 완료 조건을 설명한다. 다음 문제는 앞선 관측에서 도출할 수 있다.
- Define은 반드시 예/아니오로 판정할 수 있는 하나의 명제를 질문으로 쓴다. ‘확인한다’·‘살펴본다’는
  활동 설명을 문제정의로 사용하지 않는다. 대상·범위·판정 근거를 명확히 하고 활동 수행의 완료를
  명제의 충족으로 바꾸지 않는다. 근거 부족으로 판정하지 못한 상태와 근거 있는 ‘아니오’는 구분한다.
- 범위와 제약 안의 문제 세분화·방법 변경은 요구 변경과 구분한다. 원하는 결과·대상 범위·완료 기준·
  권한을 바꾸거나 완화할 때는 재인터뷰와 새 명세 승인이 필요하다. 과거 승인·실험·증거는 보존한다.
- 고정된 Define이 너무 커서 완료 조건을 충족하지 못했다면 그 실험은 failure로 닫는다.
  Analysis에는 범위 과대와 미완료의 근거를, Outcome의 반성에는 문제 축소를 다음 대응으로 남긴다.
  기존 Define을 축소해 성공으로 고치지 않는다. 검증 미완료를 가설이 반증됐다는 뜻으로 쓰지 않는다.
- 개별 실험 성공은 승인 요구 전체의 완료가 아니다. Chain 종료에서는 요구별 답·근거·남은 한계를
  종합해 원래 판단 기준과 대조한다. 가능성 탐색의 근거 있는 부정 답변과 결과물 제공의 미달성은
  구분하며, 종료 판정은 정확한 종료 제안에 대한 인간 승인으로 확정한다.

**2026-11-17 두 번째 사용자 결정 — 관측에 따른 실험 선택(미구현).** 예상 작업 순서는
수정 가능한 탐색 계획이며 필수 선행 조건과 구분한다. 다음 Define은 승인 요구와의 관련성,
필요한 자료·선행 결과의 실제 확보 여부, 줄이려는 불확실성을 설명한다. 관측을 근거로 현재의
가설·해석과 다음 질문을 갱신하며, 단순히 승인 목록의 제목 순서대로 실행하도록 강제하지 않는다.
자료의 신뢰성 등 필요한 선행 조건을 건너뛰거나 미래 결과를 확보한 것으로 가정하지 않는다.

베이즈 이론처럼 증거에 따라 판단을 갱신하고 탐색 범위를 점진적으로 좁히는 설계 원칙이다.
정식 베이즈 추론이나 수치 확률·사전 분포 계산을 모든 실험에 의무화하는 결정은 아니다.
반증·예상 밖 관측은 보존하고 기존 해석을 수정한다. 새로운 증거가 요구하면 가능성을 다시
열 수 있으며, 원하는 결론에 맞춰 후보를 제거하거나 매번 범위가 줄었다고 꾸미지 않는다.
이는 한 번에 하나의 열린 Cycle이라는 경계를 유지하며, 병렬 실행이나 실패 후 새 전이를
승인하지 않는다. 구체적인 선택·연결 저장 계약과 요구 충족 판정은 후속 설계 대상이다.

**세 번째 논점의 사용자 결정 — 세계 복원:** 실패 후 되돌리기는 추적 대상 작업물과 산출물을
함께 대상 Snapshot으로 복원한다. 일부 결과의 유용성이나 ‘대부분 성공’이라는 판단을 근거로
현재 세계의 일부를 남기는 기본 경로를 만들지 않는다. 실패의 관측·증거·분석·반성은 보존하고,
그 배움을 근거로 복원된 세계에서 다음 작은 Define을 정한다. 부분 성공률은 고정된 완료 조건을
대체하지 않으며 확인되지 않은 실패 원인은 가설로 남긴다. 상세는 Cycle Model의
「세계 전체 복원과 배움의 보존」을 따른다. 요구 변경 시 재인터뷰 원칙과 기존 복구 승인 경계는 유지한다.

**질문 충족·인계 후속 합의:** 승인 질문별 현재 답·근거·반대 근거와 한계·남은 질문을 종합한다.
실험 성공 수로 충족을 계산하지 않는다. 이번 Chain에서 답이 부족하면 종료 인터뷰에서 그 한계와
판정을 승인받고, 다음 Chain의 새 인터뷰에서 부족함을 메울 실험·대체 자료를 합의한다.
이전 Chain의 후속 제안은 새 실행 승인이 아니며 과거 승인·관측·잠정 해석을 구분하여 인계한다.
종료는 요구 충족과 별개이고, 자동 종료·자동 다음 Chain 개설을 뜻하지 않는다.

**2026-11-18 사용자 결정 — 별도의 잠정 정리를 두지 않는다.** 위 합의의 ‘질문별 현재 답’은
종료 인터뷰에서 원래 요구의 답·근거·한계를 검토하는 내용이다. 실험이 진행되는 동안 요구별
‘현재 답’을 계속 작성·갱신하는 별도 기록, 실험 보고서와 따로 유지하는 잠정 판단·종합 해석,
그것을 다음 실험이나 요구 완료 판단의 기준으로 쓰는 방식은 **채택하지 않는다**(이전 제안 철회).
그런 정리는 논리적 근거에 기반한 명제인지 다시 검증해야 하고, 검증되지 않은 추측이 후속 실험의
전제로 굳어질 수 있기 때문이다. 대신 승인 요구에 연결된 실험과 그 원본 관측·분석·반성을 정확히
찾아 읽고, 다음 실험의 근거를 기존 Analysis·Outcome에서 마련한다. 읽기 도구는 새 결론·추측·요약
판단을 만들지 않는다.

이 결정이 바꾸지 **않는** 것: 각 실험의 Hypothesis·Analysis·Outcome(가설·해석·배움)이라는 원본
사고 기록, 관측과 해석의 구분, 종료 인터뷰에서 요구별 답·근거·한계를 검토하고 사용자가 종료와
판정을 승인하는 합의, 과거 기록의 보존, 실험 성공 횟수를 요구 완료율로 세지 않는 원칙. 이 문서의
‘잠정 해석’은 개별 실험의 Analysis·Outcome에 적힌 아직 확정되지 않은 해석을 가리키며, 따로 쌓는
요약 기록을 뜻하지 않는다. 조회 계약은 아래 「원본 근거 조회」가 정본이다.

**계속·종료 후속 합의:** 반성에서 다음 실험을 계속할 근거 또는 탐색을 멈출 근거를 마련한다.
다음 실험의 관련성·준비 상태·줄일 불확실성이나 추가 탐색의 한계를 설명한다. 종료 제안은
사용자 승인과 구분하며, 종료 인터뷰에서 결과·근거·미해결 사항을 보고 정확한 종료와 판정을
승인받는다. 횟수·성공률·무응답에 따른 자동 종료는 하지 않는다. 현재 전이·승인 경계는 유지한다.

**체인 명세 안정성과 적용 경계 후속 합의:** 새 요구–실험 설계는 새 Chain부터 적용하는 방향으로
정한다. 기존 Chain의 중간 전환과 과거 기록 이전은 후순위로 두고 과거 승인·완료 의미를 재해석하지
않는다. 새 Chain은 시작 인터뷰에서 합의한 요구·범위·제약·판단 기준을 기준으로 탐색한다.
Define의 세분화·가설 변경·관측에 따른 실험 순서 변경만을 이유로 같은 Chain의 명세를 반복 수정·
재승인하는 것을 기본 흐름으로 두지 않는다. 이전 Chain의 답과 미해결 사항은 다음 Chain의
시작 인터뷰에서 새 명세를 합의하는 근거가 된다.

사용자의 요구 자체가 달라지거나 범위·권한·판단 기준을 바꿔야 하는 경우를 자유로운 Define으로
위장하지 않는다. 그런 변경에는 사용자 합의가 필요하며, 기존 Chain의 정리와 새 Chain 인터뷰를
통한 처리 방향을 검토한다. 사용자 정정·취소 등 모든 예외적 재인터뷰를 금지하는 결정은 아니다.
세부 예외 전이와 저장 방식은 미결이며 현행 계획 revision 기능을 이번 문서 변경으로 제거하지 않는다.

**기존 구현과 후속 설계:** 현재 제품은 승인 항목과 실험을 직접 연결하며 같은 문제·완료 범위를
요구한다. 기존 항목의 누락·정의 변경 거절도 유지된다. 새 방향은 문구만 완화해서 적용할 수 없다.
요구–실험 연결, 요구별 미해결 상태, 실행 선택·순서, 종료 근거, 저장 호환·재개와 실패 후 전이를
설계·구현해야 한다. 그 전에는 현행 도구 검사를 우회하지 않는다. 작은 실험 성공의 자동 집계,
새 실패 전이, 병렬 실행, 기존 분할·대체 후보의 채택은 이번 결정에 포함되지 않는다.
(2026-11-18: 위 ‘요구별 미해결 상태’는 요구마다 저장하는 상태·판정 값이 아니다. 모든 요구는
종료 인터뷰의 승인 전까지 미해결이라는 사실만 읽기 응답이 말하며, 요구별 답을 축적하는 저장
구조는 두지 않는다.)

이번 문서 변경은 설계 방향을 기록한다. 기존 `scope/items/synthesis` 입력 모양,
제안 판·digest 결속, 저장 형식, 단일 항목 실행 바인딩의 실제 변경은 후속 구현에서 검토한다.
자연어의 문제 크기·기여·요구 충족을 Core가 자동 증명한다고 주장하지 않는다.
실행 절차를 설명하기 위한 새 필수 필드를 임의로 추가하지 않는다. 사용자 표시와 내용 작성의
간결화는 [Cycle Model §16](GIL_Cycle_Model_v0.1.md#간결하고-명징한-제안)을 따른다.

## MCP 입력 오류 복구

`gil_plan_read`/`gil_plan_review`의 `active_plan`은 실행 가능한 최신 승인 판의
`{revision, digest, scope, items}`를 원문 그대로 반환한다. 실행된 항목도 모두 포함하며,
승인 판이 아직 실행 가능하지 않으면 null이다. 열린 제안(`proposal`)·Experiment 바인딩 문서(`document`)와
구분한다. 이 읽기 투영은 저장 형식·승인·기록·Snapshot을 변경하지 않는다.

항목 추가 시 `active_plan.items` 전체를 복사해 새 ID를 넣는다. 기존 항목 누락과 question/done_when
변경은 각각 id·필드 경로로 거절한다. 불일치는 저장·받은 값을 escape한 미리보기(120자)로 보여 주고
최대 8개 차이와 나머지 개수를 표시한다. 전체 원문은 `active_plan`에서 읽는다. scope·순서 변경은
기존 계약대로 새 승인 대상이다. 거절문은 구체적 원인부터 보여 주고 실패한 제안을 저장하지 않는다.
새 append 모드나 기존 항목 자동 보정은 제공하지 않는다.

### Synthesis 근거 입력의 사전 안내와 일괄 진단

`synthesis` 스키마는 `statement`, `basis_refs`, `exploration_checklist`를 명시한다.
`basis_refs`의 유효성은 Cycle Model §16 그대로다: 현재 Interview에서 해당 Synthesis의
부모 사슬에 속한 닫힌 Question/Interpretation의 canonical StepRef를 최소 하나,
중복 없이 한 줄에 하나씩 적는다. JSON에서는 `"step:C5/S1\nstep:C5/S2"`와 같은 문자열이다.
예시 번호는 현재 프로젝트의 실제 닫힌 참조로 바꾼다. Cycle 참조·다른 Cycle·설명·쉼표 목록은
허용하지 않는다. 이전 Cycle/외부 배경은 statement에 설명하며 현재 Interview 근거를 대체하지 않는다.

열기 영수증과 Context에도 이 조건을 제공한다. 잘못된 참조는 모든 비어 있지 않은 줄을 검사해
필드 경로와 원래 1-based 줄 번호별 이유를 한 응답에 반환한다. 한 줄은 먼저 실패한 조건을
설명하고 전체 규칙도 함께 보여 준다. 거절된 제안은 저장하지 않으며 참조를 statement로
자동 이동하거나 Report·승인·기록·Snapshot을 수정하지 않는다. 저장 loader도 같은 검증을 유지한다.

## 문서와 증거

Interview에서는 프로젝트 파일을 쓰지 않는다. 승인 계획의 결정적 Markdown은 read/export로 제공한다. 해당 Experiment의 Verify에서 파일로 저장하고 `plan_document`에 안전한 상대 경로를 적는다. 확정될 Snapshot의 bytes가 바인딩된 승인 판의 export와 같아야 한다. 별도의 새/변경된 실제 작업 증거를 `evidence_files`에 기록해야 하며 계획 문서만으로 실행 증거를 대신하지 못한다. 계획·문서·증거·Analysis·Outcome의 기존 연결을 함께 보존한다.

`document`는 Experiment 열기와 실행 항목 바인딩이 성공한 순간부터 조회되는 결정적 문자열이다.
Interview 등 바인딩이 없는 현재 Cycle에서는 null이고, 조회나 실패한 close가 나중에 생성하지 않는다.
한 Experiment는 정확히 한 항목에 바인딩된다. 다른 항목의 작업을 수행했다는 서술로 실행 기록을 추가하지 않는다.
현재 Cycle이 닫혀도 바인딩과 그 판의 document는 보존한다. `next`는 현재 Graph의 다음 호출을 투영한다.
`close_report`는 제안의 Synthesis 원문이며 Synthesis가 닫힌 뒤 현재 단계의 Report로 재사용하지 않는다.

## 범위와 수용 검증

Core/저장 roundtrip, bare approval 거절, revision 교체, 중복 입력, 수정 요청, Interview 미종료 실행 거절, 큐 재정렬·과거 보존, 문서 bytes·실행 증거 검증, scope/token/digest 격리, UI 재연결을 자동 시험한다. 실제 Host 표시·물리적 버튼 클릭·Luna 실사용은 별도의 실제 환경 시험이다. 기본 배포 변경, 기존 기록 이전, M6-E 종료/취소 정책, M3 restore 안전성은 이 변경의 완료로 주장하지 않는다.

## 개발 검증 실행

```sh
cargo test --locked -p gil --features approved-plan,visual-authoring,product-safety --lib
cargo test --locked -p gil --features approved-plan,visual-authoring,product-safety --test context --test context_profiles --test guidance_wave1 --test guidance_wave2 --test approved_plan_wire_wave1
cargo build --locked -p gil --all-targets --features approved-plan,visual-authoring,product-safety
node --test mcp-app/plans.test.mjs
```

일반 `chain-core` 회귀와 기본 format 4/5 시험도 따로 실행한다. 이 프로필은 승인 없는 기존 합성 bootstrap을 의도적으로 거절하므로, 기존 전체 suite에 무조건 `--all-features`를 붙이는 것은 인수 기준이 아니다. 전용 suite의 사용자 응답은 명시적인 합성 fixture이며 실제 사람이 승인했다는 증거로 취급하지 않는다.

2026-11-02 문맥 인수 보완: 기존 context 투영 시나리오는 format 4 또는 승인 계획 확장 없는
format 6의 합성 기록을 실제 loader로 읽는다. 기본/Chain/승인 프로필 모두에서 옛 기록의
format·Chain 포함 관계·승인 ledger 부재를 보존하며, 새 승인 프로젝트의 맨 `approved: yes`
거절도 별도로 검사한다. 기존 시나리오를 새 승인 프로젝트로 오인해 부트스트랩하거나
프로필 시험을 건너뛰지 않는다. 실제 stdio 시험은 제안 대기·승인·Verify·Cycle 종료 후
Context의 read-only 성질, 종료 상태 안내, 서버 재시작 뒤 같은 문맥 복원을 검증한다.
합성 App 입력은 물리적 사용자 승인이나 Luna 실사용의 증거가 아니다.

## Monitor 조회 상태 (2026-11-02 보완)

승인 저장 결과와 화면의 최신 조회 상태를 구분한다. 최초 조회는 확인 중, 연속 1~2회 조회 실패는
중립적인 재확인 안내, 3회 이상은 지속적인 조회 실패와 원인·다시 읽기 버튼을 표시한다.
마지막으로 확인한 승인/수정 요청의 판과 기록은 조회 실패가 지우지 않는다. 다만 과거 확인 사실임을
명시하며 새 제안의 승인이나 현재 실행 상태로 해석하지 않는다. 조회 실패 시 capability와 승인 버튼은
비활성화하고 새로 읽은 제안에 대해서만 다시 표시한다.

성공한 조회만 30초 조용한 갱신 간격을 사용한다. 실패는 2·4·8·15초(상한)로 재시도하고 성공 후
초기화한다. 진행 중이라 생략한 조회를 성공으로 세지 않으며, 조회 도중 받은 변경 힌트를 잃지 않는다.
숨겨진 App의 polling 중단은 유지한다. 승인 저장 응답이 없거나 일치하는 receipt가 없으면 별도
‘저장 결과 불확실’을 유지하고 실제 기록을 다시 읽는다. 자동 승인 재전송이나 polling에 의한 채팅 전송은 하지 않는다.


## 후속 구현 이행 — 검토용, 제품 동작 변경 전

이 절은 2026-11-17 합의를 구현하는 순서다. 위 기존 도구·저장 계약은 실제 구현 전까지 유효하다.
새 Chain부터 적용하되 현재 바이너리의 새 Chain이 이미 새 의미를 지원한다고 안내하지 않는다.
기존 Chain의 자동 전환·과거 승인 재해석·일괄 기록 이전은 하지 않는다.

**2026-11-17 구현 상태.** 1번과 2·3·5번의 첫 실행 흐름을 개발 프로필 `exploration-flow`로
구현했다. 계약은 아래 「탐색 실행 계약」이 정본이다. 이 프로필은 어떤 배포 구성에도 켜지 않았고
기존 설치본·`approved-plan` 빌드의 동작은 그대로다. 4번(종료 인터뷰에서의 요구별 답·한계 투영과
Chain Report 인계 형식)과 Monitor의 요구별 화면, 실제 모델·Host 평가는 남아 있다.

**2026-11-18 갱신.** 표 2번의 ‘질문별 현재 답·근거·한계의 투영’ 가운데 실험 중에 유지하는 요구별
답의 투영은 위 사용자 결정으로 철회했다. 같은 자리를 **연결된 실험의 원본 기록 조회**가 대신하며
`exploration-flow` 프로필에 구현했다(아래 「원본 근거 조회」). 4번의 종료 인터뷰 검토는 그대로 남은
일이고, 그 검토의 재료도 따로 쌓은 요약이 아니라 이 원본 기록이다.

**2026-11-18 갱신 — 종료 승인 뒤의 안내와 인계 연결(4번의 안내 부분).** 종료 제안이 승인된 뒤에도 영수증이
실행 계획이 승인된 것처럼 「실행 작업으로 넘어간다」「Experiment는 그 뒤에 연다」고 말하던 결손을 고쳤다. Core의
검사(Closing Interview는 `close_chain`으로만 닫히고 Chain Report의 판정은 승인된 `chain_verdict`와 같아야 한다)는
이미 있었고 바뀌지 않았다. 바뀐 것은 안내뿐이다: 승인된 종료 Synthesis의 `chain_verdict`를 기록에서 읽어 그 뒤의
영수증·Context·`gil_plan_read`의 `next`가 Outcome → `close_chain` Cycle Report → 승인된 판정의 Chain Report를
가리키고, 실행된 실험 뒤의 Synthesis는 종료 제안에 담을 것(질문별 답·근거 참조·반대 근거와 한계, 미해결, 마치는
이유, 후속 후보)을, Chain Report 자리는 `handoff_summary`에 담을 것을 말한다. 닫힌 Chain 다음의 Opening Interview는
이전 인계에서 출발하되 새 승인이 필요함을 첫 줄에서 말한다. 종료 인터뷰에서 요구별 답을 구조적으로 보여 주는
화면·형식, Chain Report의 구조화된 인계 형식, 실제 모델·Host 평가는 그대로 남은 일이다.

| 순서 | 구현 범위 | 검증할 경계 |
| --- | --- | --- |
| 1 | 새 Chain 계약의 식별·저장 호환, 안정된 승인 요구와 여러 실험의 연결 | 기존 기록은 기존 의미로 읽고, 구형 writer가 새 의미를 훼손하지 않음. 요구 연결이 실행 승인 확대가 아님 |
| 2 | 관측에 근거한 다음 Define 선택과 연결된 실험의 원본 관측·분석·반성 조회 (11/17 표기 「질문별 현재 답·근거·한계의 투영」은 11/18 결정으로 대체) | 승인 항목 순서가 아닌 관련성·준비 상태·불확실성으로 선택. 개별 성공이 전체 요구 완료로 집계되지 않음. 읽기는 답을 종합·판정하지 않음 |
| 3 | 반성의 원인 역추적·복원점·계속/종료 근거를 기존 보고서와 연결 | 실패는 무변경 세계에서도 revisit 필수. 복원 대상·승인은 검증하며 지원되지 않는 Chain 간 복원은 명시적 거절 |
| 4 | 종료 인터뷰·Chain Report·다음 Opening Interview 인계 | 미해결 상태를 보존하고 사용자가 종료 판정 승인. 다음 Chain의 범위는 새 인터뷰에서 승인 |
| 5 | 문맥·Monitor·내장 설명서·다섯 언어 안내·평가 자료의 새 계약 대응 | 현재/새 계약을 혼동하지 않고 기존 실행 기록을 다시 해석하지 않음 |

새 Reflection Step을 임의로 추가하지 않는다. 반성은 Analysis의 근거와 Outcome의 lesson,
Cycle의 next_direction에 어떻게 연결할지부터 구현 설계에서 구체화한다. Define은 예/아니오로
판정할 단일 명제이며 관측 기준을 고정한다. 활동 수행이나 근거 있는 부정 답의 유용성을 이유로
명제 충족을 성공으로 바꾸지 않는다. 답할 근거가 없는 경우는 아니오와 구분해 보고하되 기존
success/failure 외의 새 verdict를 만들지 않는다. 실패 복원 후에도 관측·반성은 다음 실험의 근거다.

### 지침·예시 동시 갱신 대상

- `src/mcp/workflow.md`가 공통 프롬프트 정본이다. Plugin Skill은 sync-workflow로 생성하며 직접 고치지 않는다.
- `src/manual/topics/step/define/smallest.md`, `cycle/interview/clarify.md`, `cycle/revisit*.md`와
  `src/i18n/*.json`의 실제 안내를 함께 검토한다. 이번에는 지원 가능한 질문 형식·반성·복원 안내만
  보완하고 현재 항목 바인딩 제약을 남긴다. 향후 자유로운 실험 선택의 활성화는 코드와 함께 한다.
- `scenarios/chain-interview/`와 `design-tests/`의 기존 자료는 기존 계약/미채택 후보의 증거다.
  기존 시험 통과를 새 의미론의 검증으로 세지 않는다. 새 평가에서는 관측에 따라 문제를 바꿔도
  원래 요구를 유지하는 사례와 요구 자체 변경을 구분한다.
- 기존 preview 게시 장부·설치 버전은 역사적 사실로 유지한다. 소스 안내 수정은 설치본 갱신이 아니다.

### 필수 인수 사례

- 활동형 Define을 거절/재작성하도록 안내하고 질문의 대상·범위·예/아니오 판정 기준을 유지한다.
- 같은 요구에서 서로 다른 작은 질문을 순차 탐색하고, 반증으로 후보를 다시 열 수 있다.
- 질문의 긍정/부정을 결과를 본 뒤 뒤집지 않는다. 판정 미도달을 근거 있는 아니오로 바꾸지 않는다.
- 실패 후 변경 파일 있음/없음 모두 명시적 복원 경로를 남긴다. 작업물과 산출물 전체가 목표 세계와 일치한다.
- 잘못된 정보의 원인 위치와 복원점은 구분한다. 원인이 미확정이면 그렇게 기록한다.
- 종료 시 질문별 답·근거·한계를 제시하고 인간 승인 없이 닫지 않는다. 다음 인터뷰는 이전 기록을 활용하되 승인을 재사용하지 않는다.
- 기본/제품 구성의 저장·재시작·도구·UI 회귀와 실제 모델/Host 인수를 구분한다. 자동 시험이 자연어 의미 충족을 증명한다고 주장하지 않는다.

## 탐색 실행 계약 — 개발 프로필 `exploration-flow`

상태: 2026-11-17 구현. `exploration-flow`(`approved-plan` 포함)는 opt-in 개발 프로필이며 기본·제품
배포 구성에 켜지 않았다. 이 절은 그 프로필이 여는 **새 Chain** 에만 적용된다. 이 문서의 앞 절이
말하는 항목 바인딩·순서·재시도 규칙은 표식 없는 Chain(항목 큐 계약)에 그대로 유효하다.

### 계약의 식별과 저장

- Chain의 `approved_plan`에 `contract: exploration`이 있으면 탐색 계약, 없으면 항목 큐 계약이다.
  값은 `exploration` 하나뿐이며 항목 큐를 뜻하는 명시적 값은 없다. 표식 없는 원장은 이전과 같은
  바이트로 읽고 쓴다. 저장 형식 번호(6/7)는 바뀌지 않는다.
- 표식은 Chain을 열 때 정해지고 바뀌지 않는다. `exploration-flow` 빌드가 여는 Chain(새 프로젝트의
  첫 Chain과 닫힌 Chain 다음의 Chain)은 탐색 계약이다. 그 밖의 빌드는 표식을 쓰지 않는다.
- 닫힌 항목 큐 Chain 다음에 탐색 Chain이 올 수 있다. 앞 Chain의 원장·승인·실행 기록은 다시 쓰지
  않는다. 반대 방향(탐색 Chain 다음의 항목 큐 Chain)과 Chain 사이의 표식 이동은 load에서 거절한다.
  승인 원장이 없는 Chain(`chain-core`만의 기록) 다음에는 원장 없는 Chain이 오며 표식도 없다.
- 구형 writer와 `approved-plan`만의 빌드는 표식이 있는 기록을 읽지 못한다. 이 변경 전의 프로그램은
  `approved_plan`의 알 수 없는 필드로, 이후의 `approved-plan` 빌드는 명시적 사유로 거절하며 기록을
  바꾸지 않는다. 표식을 지워 항목 큐 규칙으로 읽히게 한 기록도 연결의 선택 근거 칸 때문에 거절된다.
- 기존 Chain의 중간 전환, 과거 기록의 일괄 이전, 과거 승인의 재해석은 없다.

### 승인 요구와 실험의 연결

- 제안의 입력 모양(`scope`/`items`/`synthesis`)과 판·digest 결속, Monitor 버튼 승인은 그대로다.
  탐색 계약에서 `items`는 실행 순서가 아니라 **승인된 요구**(답할 질문 `question`, 판단 기준 `done_when`)다.
- Experiment는 열 때 요구 하나에 명시적으로 연결된다. `gil open experiment`(MCP `gil_open`의
  `contract`)의 본문에 여섯 칸을 적는다. 첫 미실행 항목을 자동으로 고르지 않는다.

  ```text
  plan_revision: 1
  plan_digest: <gil_plan_read.active_plan.digest>
  requirement_id: <승인된 요구 id 하나>
  relevance: 이 질문이 그 요구의 답에 왜 필요한가
  readiness: 필요한 입력·선행 결과가 실제로 확보돼 있는가
  uncertainty: 이 실험이 줄이는 불확실성은 무엇인가
  ```

- Core가 검사하는 것: 실행 가능한 승인 판이 있는가(승인 전 거절), 보존된 재인터뷰 요청이 없는가,
  판과 digest가 **이 Chain의 현재 승인 판**과 정확히 같은가, 요구 id가 그 판에 있는가, 세 설명이
  비어 있지 않은가(각 4096바이트 이하), 모르는 칸이 없는가. 다른 Chain의 승인 digest, 같은 Chain의
  지난 판, 승인되지 않은 요구는 각각 사유를 밝혀 거절하고 기록을 바꾸지 않는다.
- Core가 검사하지 **않는** 것: Define이 그 요구에 실제로 기여하는지, 세 설명이 사실인지, Define이
  예/아니오 명제인지. 자연어 의미의 판정은 모델·실사용 평가의 몫이며 자동 시험으로 증명하지 않는다.
- 연결은 `executed_cycles`에 `{cycle_ref, revision, item_id, selection}`으로 남는다. 한 요구에 여러
  실험이 순차 연결될 수 있고 목록 순서의 제약이 없다. 성공은 요구를 소비하지 않으며 실패는 같은
  항목의 재바인딩을 만들지 않는다. 한 Experiment는 정확히 한 요구에 연결되고, 한 번에 열린 Cycle은 하나다.
- 세분화·방법·순서 변경은 새 제안을 요구하지 않는다. 요구·범위·권한·판단 기준 자체의 변경은
  기존대로 Interview의 새 승인 판이며(`gil_plan_request_interview`가 요청을 보존한다) 기존 요구의
  누락·정의 변경 거절도 그대로다. 요구 변경을 Define으로 위장했는지는 Core가 판정하지 못한다.

### 실패 뒤의 복원과 다음 실험

- 실패한 Experiment는 `revisit`만 선언할 수 있고(기존 문법), 제품 구성에서는 승인 복구를 거쳐야
  다음 Cycle을 열 수 있다. 복구의 대상 검증·승인·보존본·세계 전체 복원은 바꾸지 않았다.
  파일 차이가 없는 실패도 같은 경로를 지나며 변경 목록이 빈 승인 기록이 남는다.
- 복원 뒤의 Experiment도 연결을 들고 열린다. 같은 요구에 다른 작은 Define을 연결할 수 있고 실패한
  Define을 되풀이할 필요가 없다. 「직전 실패의 revisit 계보에서 같은 항목만 다시 바인딩」 규칙과
  `retry_pending`은 항목 큐 계약에만 남는다. 탐색 계약의 `retry_pending`은 언제나 비어 있다.
- 되돌아갈 곳이 다른 Chain의 Cycle이면 그 사실을 밝혀 거절한다. 다른 Chain의 기록을 근거로 읽는
  것과 그 세계로 복원하는 것은 다르며 후자는 구현하지 않았다.

### 읽기 투영

- `gil_plan_read`/`gil_plan_review`는 탐색 Chain에서만 다음을 더한다. 항목 큐 Chain의 응답은 그대로다.
  `contract: "exploration"`, `requirements[{id, question, done_when, experiments[{cycle_ref, state,
  verdict, revisit_from, plan_revision, selection}]}]`, `current_experiment`, `experiment_link`(위 본문의
  골격), `requirement_resolution: "user_approved_closing_interview_only"`.
- `pending`은 아직 실험이 연결되지 않은 요구, `current_item`은 현재 Cycle이 연결된 요구다.
  요구가 충족됐는지를 말하는 필드는 없다. 실험 성공 수를 합산하거나 완료율로 바꾸지 않는다.
- Context는 승인 요구와 연결된 실험의 기록된 판정, 현재 실험의 요구와 선택 근거, 모든 요구가
  미해결이라는 사실, 다음 호출을 적는다. 영수증과 Host 안내는 탐색 Chain의 자리에만 그 계약의
  문장을 덧붙인다. Monitor의 기존 승인 패널은 연결된 요구를 「실행함」(Cycle이 열렸다는 기록)으로,
  연결 없는 요구를 「남음」으로 보여 준다. 요구별 전용 화면은 후속이다.
- **기존 패널의 요구 원문 (2026-11-18 보정).** 열린 제안이 없을 때 패널은 요구의 질문·판단 기준을
  `current_item`에서만 읽었다. `current_item`은 현재 Cycle이 연결된 요구 하나이므로, 현재 실험이 다른
  요구로 옮기거나 현재 Cycle이 Interview이면 앞서 연결된 요구는 id·상태·Cycle 주소만 남고 원문이
  빠졌다. 이제 탐색 Chain(`contract: "exploration"`)의 응답에서는 그런 요구의 원문을 `requirements`의
  **같은 id**에서 글자 그대로 읽는다. 고치거나 줄이거나 다른 말로 옮기지 않는다.
  - 상태(「실행함」·「지금 열린 Cycle에 묶임」·「남음」), 개수, 행의 순서, 행에 적는 Cycle 주소(그 요구에
    마지막으로 연결된 Cycle)의 뜻은 그대로다. 한 요구에 실험이 여럿이어도 행은 하나다. 실패했거나 닫힌
    실험은 원문을 빼는 이유가 아니며, 원문이 보인다는 것은 그 요구가 충족됐다는 뜻이 아니다.
  - 제안이 열려 있으면 행은 그 제안(`proposal.draft.items`)의 원문만 쓴다. 승인된 `requirements`와
    섞지 않으며, 승인 버튼이 보내는 판·digest도 그 제안의 것이다.
  - 응답이 말하지 않은 것은 채우지 않는다. 표식이 없거나 다른 값이면(항목 큐 Chain 포함) 이 목록을
    읽지 않는다. 목록이 없거나, 그 id가 없거나 두 번 있거나, 값이 글이 아니면 그 칸을 비운다. 다른
    요구의 문구, 제안의 초안, 앞서 그린 화면의 내용으로 대신하지 않는다.
  - 서버 응답·저장 필드·상태·전이·승인 단계·도구 계약은 바뀌지 않았다. 화면 모듈
    `mcp-app/plans.mjs`의 읽기만 바뀌었다.

### 원본 근거 조회 (2026-11-18)

승인 요구에서 그 요구에 연결된 실험의 원본 기록과 저장 증거까지 가는 읽기 전용 경로다.
`exploration-flow` 빌드에만 등록되는 모델용 도구 둘이며, 그 빌드가 읽을 수 있는 모든 Chain(표식
유무 무관, 이전 Chain 포함)의 기록을 읽는다. 저장 형식·상태·전이·승인 방식은 바뀌지 않는다.

```text
gil_plan_read        requirements[].experiments[].cycle_ref   요구에 연결된 실험의 주소 (기존)
gil_report_read      owner {kind: cycle, ref}   그 실험의 위치·Step 주소 목록·Cycle Report 원문
                     owner {kind: step,  ref}   Step Report 하나의 원문 (+ Verify 가 적은 파일의 저장 상태)
                     owner {kind: chain, ref}   그 Chain 의 Cycle 주소·연결·Chain Report 원문
                     … + list, from             목록(cycles·steps·fields·files)의 한 쪽 — 이어 읽기
                     … + field, part?, offset?  칸 하나의 값(또는 이름)의 바이트 범위 — 이어 읽기
gil_evidence_read    owner {kind: step, ref}(Verify), path, offset?   그 Verify 의 Snapshot 에 저장된 바이트
```

- **출처.** 모든 응답은 `owner`, `chain {ref, state}`, `cycle {ref, kind, state, verdict, parent,
  revisit_from, entry_snapshot, exit_snapshot}`, `plan_link`를 싣는다. `plan_link`는 그 Experiment를
  열 때 기록된 연결이다: 탐색 Chain은 `{contract: "exploration", plan_revision, plan_digest,
  requirement_id}`, 표식 없는 Chain은 `{plan_revision, plan_digest, item_id}`, 연결이 없으면 `null`.
  `verdict`는 Cycle Report에 적힌 값 그대로이며 요구에 대한 해석이 아니다.
- **원문.** `report`는 `{state: "written", sha256, field_count, fields[{index, name, bytes, value}]}`
  또는 `{state: "not_written"}`이다. `fields`는 저장된 Report의 칸을 저장된 순서(이름순)로 싣고,
  `index`는 그 순서의 자리다(닫힌 Report는 바뀌지 않으므로 안정된 주소다). 값은 저장된 글자 그대로이며
  고르거나 다른 말로 옮기지 않는다. `sha256`은 `gil_report_visuals.report_sha256`과 같은 칸 digest다.
  Cycle 조회의 `steps`는 `{ref, kind, state, report}`(Verify는 `snapshot` 포함)의 주소 목록이고 Report
  본문을 싣지 않는다 — 어느 원본을 열지는 읽는 쪽이 고른다.
- **끝까지 읽기 (2026-11-18 검토 보완).** 한 응답에는 크기 한도(1 MiB)가 있고 저장된 Report에는
  한도가 없다. 따옴표·역슬래시가 많은 값은 JSON으로 적히며 두 배, 응답의 text 사본 안에서 다시 두 배가
  된다(약 200 KB의 값이 한 응답으로는 한도를 넘는다). 그래서 어떤 응답도 「전부 아니면 거절」이 아니다.
  응답은 실제 전송 크기로 센 분량(가변 부분 512 KiB) 안에서 조립하고, 싣지 않은 것은 **그것을 읽는
  입력과 함께** 밝힌다. `truncated: false`는 물은 것을 모두 실었다는 뜻이고 `truncated: true`는 일부를
  싣지 않았다는 뜻이다(응답에 대한 말이며 기록이나 요구의 완료를 뜻하지 않는다).
  - 값이 16 KiB를 넘거나 한 응답이 싣는 값의 합(64 KiB)을 넘긴 칸은 `value_omitted: true`와
    `read: {field: <index>}`로 나온다. 같은 호출에 `field`(와 이어 읽을 때 `offset`)를 더하면 그 값의
    바이트 범위를 받는다: 한 번에 최대 16 KiB, 글자 경계에서 자르며, `sha256`·`total_bytes`는 값 전체의
    것이고 `truncated`·`next_offset`·`remaining_bytes`가 나머지를 말한다.
  - 256바이트를 넘는 칸 이름은 `name_omitted: true`·`name_bytes`와 `read_name: {field, part: "name"}`으로
    나오고 같은 방식으로 범위를 읽는다.
  - 한 응답에 다 들지 않은 목록은 `more: [{list, from, total}]`로 나온다. 같은 호출에 `list`와 `from`을
    더하면 그 목록의 다음 쪽(`items`, `returned`, `next_from`)을 받는다. 목록은 Chain의 `cycles`,
    Cycle의 `steps`, 모든 Report의 `fields`, Verify의 `files`다. 쪽은 항목 사이에서만 나뉘고, 위치로
    물은 쪽은 언제나 한 항목 이상 나아간다.
  - 이어 읽기 입력은 한 호출에 하나다(`list`·`from` 또는 `field`·`part`·`offset`). 조각을 순서대로
    이으면 저장된 Report와 같고, `report.sha256`으로 확인할 수 있다. 생략은 기록이 없거나 읽을 수 없다는
    뜻이 아니다. 출처 정보에 되풀이되는 긴 요구 id(256바이트 초과)는 길이와 digest만 싣는다.
- **저장 증거.** 닫힌 Verify의 Step 조회는 `evidence {state: "indexed", snapshot, current_world,
  files[{path, declared_in, stored, sha256, in_current_world}]}`를 싣는다. 대상은 그 Verify Report가
  직접 적은 경로뿐이다: `evidence_files`의 각 줄과 `plan_document`(그 실험이 실행된 승인 판의 원문).
  `gil_evidence_read`는 그 가운데 하나를 **그 Verify가 확정한 Snapshot**에서 읽는다. 작업 폴더를
  읽지 않으며 복원하지 않는다. 그래서 되돌아가기로 폴더가 이전 세계로 돌아간 뒤에도 실패한 실험의
  증거가 읽히고, 같은 경로의 파일이 그 뒤 달라져도 지난 실험의 증거와 섞이지 않는다.
  `in_current_world`(`same`·`different`·`absent`·`unknown`)는 저장된 바이트와 **최신 확정 세계**의
  같은 경로를 견준 기록 대 기록 비교다. 작업 폴더의 미확정 변경은 관측하지 않는다(`gil_status`).
- **분량.** `gil_evidence_read`는 한 번에 최대 16 KiB를 싣는다. `sha256`·`total_bytes`는 저장 파일
  전체의 값이고, 잘린 응답은 `truncated: true`·`next_offset`·`remaining_bytes`와 안내문으로 나머지를
  읽는 호출을 말한다. 유효한 UTF-8 구간은 `encoding: "utf8"`의 `text`로(글자 중간에서 자르지 않는다),
  아니면 `encoding: "base64"`의 `data_base64`로 싣는다. Report의 값도 위 「끝까지 읽기」의 범위로 읽으며
  값 안을 말없이 자르지 않는다. `response_too_large`는 분량 계산이 틀렸을 때 큰 응답을 보내지 않기 위한
  마지막 방어일 뿐, 정상 경로에서 원본 조회를 막는 응답이 아니다.
- **만들지 않는 것.** 종합 답, 타당성·신뢰도 판정, 완료율, 요구 충족 여부를 말하는 칸이 없다. 실패한
  실험과 되돌리기 이전의 기록도 성공한 실험과 같은 방식으로 읽힌다. 서로 어긋나는 기록은 각자 적힌
  대로 돌려주며 어느 쪽도 채택하지 않는다. 기록에 있다는 사실은 그 내용이 참이거나 지금도 유효하다는
  판정이 아니다(응답의 `note`가 같은 말을 한다).
- **구분하는 거절.** 없는 주소는 `not_found`로 거절하고 가까운 번호나 다른 Cycle의 같은 번호로
  물러서지 않는다. 열린 Step·Cycle·Chain은 거절이 아니라 `report.state: "not_written"`이다. 증거 조회는
  `not_a_verify`(Verify가 아닌 Step), `report_not_written`(열린 Verify — Report도 Snapshot도 아직 없음),
  `not_declared`(그 Report가 적지 않은 경로; `declared`가 적힌 경로), `not_in_snapshot`(적었지만 그
  Snapshot에 없음), `evidence_unavailable`(저장 객체를 읽지 못함 — 접근 불가이지 증거 없음이 아님),
  `invalid_path`, `invalid_offset`, `record_unavailable`(프로젝트 기록을 읽지 못함)을 구분한다. 이어 읽기는
  `invalid_list`(그 기록에 없는 목록; `lists`가 가능한 목록), `invalid_from`(목록의 끝을 넘음),
  `invalid_field`(없는 칸 index), `invalid_offset`(끝을 넘었거나 글자의 중간), `report_not_written`(아직
  Report가 없는 Step의 칸)을 구분하며 빈 응답으로 답하지 않는다.
  모양이 틀린 입력(canonical이 아닌 주소 등)은 실행 전에 -32602로 거절된다.
- **읽기만.** 두 도구는 읽기 전용 세션으로 기록을 열며 commit·복원·복구·잔해 회수를 하지 않는다.
  조회 전후의 기록·저장 객체·작업 파일은 같다. 프로세스를 다시 시작해도 같은 주소로 같은 원본과
  출처를 얻는다.
- **안내.** `gil_plan_read`의 `next`(연결된 실험이 있을 때)와 Context의 요구 절이 이 경로를 가리킨다.
  항목 큐 Chain의 `gil_plan_read`·Context 문구와 `approved-plan`만의 빌드의 도구 표·설명은 바뀌지
  않았다. 공통 workflow는 도구가 연결돼 있을 때만 이 경로를 쓰도록 말한다.

**남은 한계.** 시각 자료의 픽셀은 여전히 App 전용 조회(`gil_monitor_report_image`)이며 모델은
`gil_report_visuals`의 메타데이터를 읽는다(이미지 파일을 `evidence_files`에도 적었다면 그 바이트는
base64로 읽힌다). `stored: "present"`는 Snapshot이 그 주소를 지닌다는 뜻이고 객체의 실재와 무결성은
읽을 때 확인된다. 이전 Chain의 승인 요구 **문장**은 `gil_plan_read`(현재 Chain만)가 아니라 그 Chain의
Verify가 저장한 `plan_document`로 읽는다. 복구 보존본(되돌아가기 직전 폴더의 사본)은 이 조회의 대상이
아니다. 도구는 원문을 줄 뿐이며, 모델이 필요한 원본을 고르고 근거에서 올바르게 추론하는지는 형식
검사로 입증하지 않는다.

### 검증과 한계

```sh
cargo test --locked -p gil --features exploration-flow,visual-authoring,product-safety --lib -- --test-threads=1
cargo test --locked -p gil --features exploration-flow,visual-authoring,product-safety --test exploration_flow
cargo test --locked -p gil --features exploration-flow,visual-authoring,product-safety --test exploration_tools
cargo test --locked -p gil --features exploration-flow,visual-authoring,product-safety --test exploration_evidence
cargo test --locked -p gil --features exploration-flow,visual-authoring,product-safety --test exploration_plan_panel
cargo test --locked -p gil --features exploration-flow,visual-authoring,product-safety --test approved_plan_wire_wave1 --test chain_recovery_preservation --test context --test context_profiles --test guidance_wave1 --test guidance_wave2 --test guidance_interview_chain --test resume_guidance --test manual
cargo build --locked -p gil --all-targets --features exploration-flow,visual-authoring,product-safety
```

`tests/exploration_flow.rs`는 실제 stdio MCP 프로세스·파일·승인 복구를 거치는 합성 검증이다:
한 요구 아래 서로 다른 Define 두 개(재승인 없음), 첫 성공이 요구를 끝내지 않음, 실패 → 승인된
revisit → 새 Define(파일 변경 있음/없음), 작업물·산출물 복원과 실패 증거 보존, 잘못된 연결·승인 전
실행·복원 우회·동시 Cycle 거절, 표식 없는 Chain의 항목 큐 의미 유지, 닫힌 항목 큐 Chain 다음의
탐색 Chain, 저장·재시작·읽기 불변. 항목 큐 계약의 기존 통합 시험은 이 빌드에서 **표식 없는 새
기록**(이전 빌드가 쓰는 바이트)으로 그대로 실행한다. `chain_monitor_fixtures`는 `approved-plan`
빌드의 Monitor 고정 자료라 이 프로필에서 실행하지 않는다(후속 Chain의 계약이 빌드에 따라 다르다).

`tests/exploration_tools.rs`(2026-11-17 보강)는 같은 방식으로 연 표식 있는 Chain에서 기존 도구·시각 자료
계약이 유지되는지를 본다: Verify의 시각 자료 선언과 증거 규칙([시각 증거 정책](GIL_Visual_Evidence_Policy_v0.1.md) §3,
승인 문서·`evidence_files`), Analysis·Experiment Cycle Report의 인용 의무와 다음 Define의 인용, 재시작 뒤 보고서·
Monitor view·저장 이미지의 동일성, 실패한 Experiment의 시각 증거가 승인 복구 뒤에도 읽히고 인용되는지, 잘못된 입력이
기록을 바꾸지 않고 이 구성에서 가능한 복구(직접 restore가 아닌 사람의 정리, Monitor 승인)와 실제로 열리는 설명서
주제를 가리키는지. 두 파일은 `tests/support_wave1/exploration.rs`의 시작·인터뷰·승인·연결 절차를 공유하며 승인
원장을 직접 고치거나 표식을 지우지 않는다. 이 탐색 구성 전용 시험들은 다른 구성에서는 컴파일되지 않으므로 자동
검사는 `tools/ci/expect-tests.mjs`로 최소 실행 건수를 요구한다.

`tests/exploration_evidence.rs`(2026-11-18)는 위 「원본 근거 조회」의 경계를 같은 방식의 실제 프로세스로 본다:
한 요구 아래 성공·실패 실험 각각의 Define·Verify·Analysis·Outcome·Cycle Report 원문과 출처, 승인 복구 뒤에도
읽히는 실패 실험의 보고서와 저장 증거, 같은 경로의 내용이 달라진 작업 폴더와의 구분, 재시작 뒤 같은 응답,
없는 주소·다른 범위의 주소·열린 Step·적지 않은 경로·읽을 수 없는 저장 객체의 구분, 긴 증거의 명시적 절단과
이어 읽기, 조회 전후 기록·작업 파일 불변, 표식 없는 Chain의 기존 응답 유지와 다음 Chain에서의 조회.
검토 보완으로 더한 2건: 따옴표·역슬래시 200 KB의 값을 가진 Report가 거절되지 않고 범위로 끝까지 읽히는지,
칸 6천 개·한 응답을 넘는 값·긴 이름의 Report가 쪽과 범위로 읽혀 저장한 내용과 같고 digest가 일치하는지,
모든 응답이 1 MiB 안인지, 잘못된 이어 읽기 입력의 구분, 조회 전후 불변과 재시작 뒤 같은 응답.

`tests/exploration_plan_panel.rs`(2026-11-18)는 위 「기존 패널의 요구 원문」을 실제 프로세스에 잇는다.
탐색 Chain 하나를 걸으며 App 경로 `gil_plan_review`의 응답을 자리마다 고치지 않고
`mcp-app/fixtures/exploration-plan-review.json`에 둔다: 승인 직후(연결 없음), R1 실험 열림·닫힘, 같은 요구의
두 번째 실험 실패, 승인된 되돌아가기 뒤 **R2 실험 열림**, 그 실험 닫힘, 이후 Interview(연결된 요구 없음),
새 제안 대기, 그 제안의 승인 기록, 새 판 승인 뒤 R3 실험 열림. 기본 실행은 새로 받은 응답을 그 파일과
JSON 값으로 대조하고 쓰지 않는다(`GIL_EXPLORATION_PLAN_WRITE=1`일 때만 다시 쓴다). 일회성 승인 capability와
개인 경로는 파일에 없다. `mcp-app/plans.test.mjs`가 그 응답을 실제 패널 모듈에 넣어 요구마다 한 행과 서버가
준 원문을 확인하고, 불완전한 응답·제안과 승인 요구의 분리·항목 큐 응답은 그 응답을 고친 합성 사례로 본다.
이 검사는 패널 모듈의 DOM 대역까지이며 실제 브라우저 표시와 읽는 사람의 이해를 보지 않는다.

합성 App 입력은 사람의 클릭이 아니다. 자연어 의미(Define의 명제 형식·요구 기여·요구 충족), 실제
모델의 준수, Host 화면 표시는 이 검증으로 입증하지 않는다. 위 명령의 로컬 실행은 macOS뿐이다. Windows에서의
이 프로필 실행은 `windows-2025` runner의 자동 검사 결과가 근거이며(#159 최종 검사부터 통과, 결과는
[로드맵](GIL_Roadmap.md)의 「2026-11-17 보강」), Windows PC 실기와 실제 Host 화면 인수는 수행하지 않았다.
