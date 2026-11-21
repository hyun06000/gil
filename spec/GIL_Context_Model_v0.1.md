# GIL Context Model v0.1

> 같은 GIL Graph를 인간, 새 Agent 세션과 감사자가 서로 다른 해상도로 읽는 규칙을 정의한다.

---

## 1. 목적

GIL의 기록은 계속 누적된다. 매 Step을 열 때 전체 기록을 다시 LLM context에 넣으면 같은 내용이
반복되고, 프로젝트가 길어질수록 context 비용이 불필요하게 증가한다.

반대로 지나치게 압축하면 새 Agent나 새 세션이 현재 작업을 이어갈 수 없다.

이 문서는 다음 세 투영을 분리하여 이 문제를 다룬다.

```text
story    인간이 현재 상황을 이해한다.
context  새 Agent 세션이 현재 작업을 이어받는다.
history  전체 실행 경로를 감사하고 조사한다.
```

세 투영은 별도의 상태를 소유하지 않는다. Graph, Report와 Artifact reference가 유일한 진실
공급원이며, 각 투영은 같은 read model을 목적에 맞는 해상도로 읽는다.

### 1.1 적은 토큰으로 작업 문맥을 유지한다

**2026-11-15 목표 재확인.** 대화의 모든 내용을 저장·재전송하는 것을 목표로 삼지 않는다.
승인된 목표와 제약, 현재 위치·행동, 필요한 결과·실패의 교훈·미해결 사항과 근거 참조를 전달해
새 세션이 적은 입력으로 정확히 이어가게 한다. 원본 작업 기록의 보존과 모델에 보내는 분량은 구분한다.

- 현재 작업은 필요한 상세를 유지하고, 과거 작업은 계층별 짧은 보고서로 읽는다. 평상시에는
  바뀐 상태만 안내하고, 추가 근거는 필요할 때 해당 참조를 조회한다.
- 지식·과거 경험·앞으로 적용할 약속·참여자 합의도 현재 작업에 필요한 해상도로 제공한다.
  관측·해석·승인된 합의를 혼동하거나, 이후 반박된 결론을 현재의 확정 사실로 전달하지 않는다.
- 압축을 이유로 유효한 승인·필수 제약·다음 행동·명세가 요구하는 실패 기록을 임의로 빼지 않는다.
  원문을 되찾을 참조와 미검증 상태를 유지한다. 구체적인 선별 방식과 입력량 한도는 후속 설계다.
- 계층적 보고서 전달은 기반 구현이며, 긴 현재 실험이나 누적 실패가 많을 때 입력량이 일정하게
  유지된다는 보장은 아직 없다. 독립 지식·기억 객체를 만드는 것만으로 토큰 절감이 증명되지 않는다.

검증에서는 같은 작업·모델 조건에서 재개 입력 토큰, 추가 근거 조회, 전체 실행 토큰과 작업
정확성을 함께 측정한다. 초기 입력이 짧아도 재조회·재시도 때문에 전체 비용이 늘면 이를 그대로
기록한다. 현재 상태 오인·승인 위반·이미 반박된 시도의 반복·필수 근거 누락을 줄이면서 입력을
얼마나 절약했는지 평가하며, 글자 수 감소만으로 성공을 주장하지 않는다.

---

### 체인 간 질문·근거·미해결 사항 인계

**2026-11-17 사용자 결정.** 다음 Chain의 시작 인터뷰는 이전 Chain Report의 질문별 답·근거,
반대 근거·한계, 미해결 질문과 후속 실험·대체 자료 후보를 읽고 시작한다. 검증된 관측과 잠정
해석을 구분하며 과거 결론을 무조건 사실로 전달하지 않는다. 사용자와 새 탐색 범위를 합의하고,
이전 Chain이 제안한 다음 작업을 실행 승인으로 취급하지 않는다. 새 증거로 기존 해석을 수정하면
영향받는 질문을 재검토하되 과거 기록은 보존한다. 전체 대화 대신 현재 답과 필요한 원본 참조를
전달한다. 구체적인 요구–실험 연결과 조회 구현은 후속 설계 대상이다.

**2026-11-18 사용자 결정 — 적용 범위의 정정.** 위 문단의 ‘현재 답’은 종료 인터뷰와 Chain Report가
원래 요구에 대해 검토·기록하는 답·근거·한계를 가리킨다. 실험이 진행되는 동안 요구별 ‘현재 답’이나
잠정 정리를 따로 작성·갱신해 두고, 그것을 다음 실험의 전제나 요구 완료 판단의 기준으로 쓰는 방식은
채택하지 않는다(이전 제안 철회 — 그런 정리는 근거에 기반한 명제인지 다시 검증해야 하고, 추측이 후속
실험의 전제로 굳어질 수 있다). ‘잠정 해석’은 각 실험의 Analysis·Outcome에 적힌 아직 확정되지 않은
해석이며 별도의 저장 기록이 아니다. 새 세션과 다음 실험은 연결된 실험의 **원본** 관측·분석·반성을
필요한 만큼 골라 읽는다. 읽기 투영은 새 결론·요약 판단을 만들지 않는다. 관측과 해석의 구분, 과거
기록의 보존, 종료 인터뷰의 요구별 검토와 사용자 승인은 그대로다.

**2026-11-17 구현(개발 프로필 `exploration-flow`의 새 Chain).** 탐색 계약의 Chain에서 Context는
현재 승인 판의 요구마다 연결된 실험과 그 Cycle Report의 판정을 기록 그대로 적고, 현재 실험이
기여하는 요구와 열 때 적은 관련성·준비 상태·불확실성을 보이며, 모든 요구가 미해결이고 판정은
Closing Interview에서 사용자가 승인한다는 사실과 다음 호출을 적는다. 요구의 충족 여부나 완료율은
투영하지 않는다. 항목 큐 계약의 Chain과 승인 원장 없는 기록의 Context는 바뀌지 않았다.
이전 Chain의 질문별 답·한계를 Chain Report에서 구조화해 인계하는 부분은 여전히 후속이다.

**2026-11-18 구현(같은 개발 프로필).** Context의 요구 절은 연결된 실험이 있을 때 원본을 여는 경로를
한 문단으로 덧붙인다: 연결된 실험의 주소 → `gil_report_read`(그 실험의 Step 주소와 Cycle Report,
이어서 Report 하나의 원문) → `gil_evidence_read`(그 Verify의 Snapshot에 저장된 증거). Context 자체는
지나온 Cycle의 Step을 여전히 펼치지 않으며 요구별 답을 종합하지 않는다. 계약과 응답 모양, 거절의
구분, 남은 한계는 [승인 계획 계약의 「원본 근거 조회」](GIL_Approved_Plan_v0.1.md#원본-근거-조회-2026-11-18)가
정본이다. 이 조회는 §1.1의 원칙(평상시에는 짧은 연결 정보, 추가 근거는 필요할 때 해당 참조를 조회)을
따른다. 응답이 짧다는 사실만으로 토큰 절감이나 정확성 개선을 주장하지 않는다 — 실제 모델의 추가 조회
횟수·전체 토큰·작업 정확성은 측정하지 않았다.

**2026-11-18 안내 연결(투영·저장 변경 없음).** 닫힌 Chain 다음에 명시적으로 연 Chain의 Opening Interview에
아직 Step이 없을 때, Context의 다음 호출 줄은 이전 Chain의 판정·인계 요약을 읽고(결정에 필요하면 그 원본
Report도) 답을 얻은 것·미해결로 남은 것·남겨 둔 후보에서 출발해 새 범위를 사용자와 합의하라고 안내한다.
인계의 후보는 승인이 아니고 이전 Chain의 승인·계획은 재사용되지 않는다는 점도 같은 줄에서 말한다. 이전
Chain의 투영(판정·승인 근거·인계 요약)은 그대로이며, Chain Report에서 질문별 답을 구조화해 인계하는 형식은
여전히 후속이다.

### 1.2 현재 유효한 합의를 이어받는다

**2026-11-15 방향 확정, 상세 저장·조회 계약과 구현 대기.** 새 세션에는 현재 작업에 적용되는
유효한 합의의 내용·적용 범위·근거 참조를 짧게 전달한다. 합의의 변경 이력과 원문은 필요할 때
조회하며, 전체 대화를 반복 전달하지 않는다.

| 구분 | 예 | 적용 원칙 |
| --- | --- | --- |
| 설명·협업 선호 | 약어 풀이, 간결한 보고 | 표현과 협업 방식에 적용하며 실행 권한으로 해석하지 않는다 |
| 작업 결정·제약 | 검증 보류, 기능 우선순위 | 지정된 프로젝트·작업 범위에 적용한다 |
| 실행 승인 | 특정 계획의 특정 판과 범위 승인 | 기존 승인 계약의 대상·판·조건에 결속하며 요약이 승인 증거를 대신하지 않는다 |

- 사용자의 명시적 표현과 실제 승인 기록을 근거로 삼는다. 에이전트의 해석·제안은 확정 합의와
  구분하며, 응답이 없다는 이유로 승인 또는 동의로 기록하지 않는다.
- 합의가 바뀌면 무엇이 무엇을 대체·철회했는지와 근거를 남긴다. 과거 기록은 보존하고,
  기본 문맥에는 현재 유효한 내용을 전달한다. 일부 범위만 변경하면 나머지 합의를 지우지 않는다.
- 새 표현이 기존 합의를 대체하는지 또는 적용 범위가 불명확하면 이를 미해결로 표시한다.
  해당 판단이 다음 행동에 필요할 때 확인하고, 임의로 더 넓은 권한을 선택하지 않는다.
- 프로젝트의 합의를 다른 프로젝트나 사용자 전체의 영구 선호로 자동 확대하지 않는다.
  참여자 관계 설명만으로 역할·권한·대표성을 추론하지 않는다.
- 압축하면서 승인 대상·판·필수 조건과 보류·철회 여부를 잃지 않는다. 승인 유효성은 각 기능의
  정본 기록으로 판단하며, 합의 요약 자체는 새 승인 경로나 실행 명령을 만들지 않는다.

후속 검증에서는 새 대화가 선호·작업 결정·실행 승인을 구분하는지, 변경된 범위만 갱신하는지,
철회된 승인이나 대체된 합의를 재사용하지 않는지 확인한다. 독립 합의 객체의 저장 형식·조회
도구와 기존 기록 연결 방식은 아직 확정하지 않는다.

### 1.3 응답 대기에서 돌아온 세션

[Cycle Model §20.1](GIL_Cycle_Model_v0.1.md#201-응답-대기와-재개)에 따라 현재 기록을 다시 읽고
마지막 결과·응답 대상·필요한 결정·가능한 다음 행동을 짧게 전달한다. 대기 중 변경된 파일·계획과
승인 저장 여부를 구분하며, 관측하지 못한 상태를 최신으로 전달하지 않는다. 이전 대화 전체를
반복 제공하거나 이미 저장된 같은 승인을 다시 받는 방식으로 재개를 대신하지 않는다.
이 절은 2026-11-15 후속 요구이며 상세 조회·변경 감지와 안내 구현은 남아 있다.

## 2. 핵심 원칙 — 거리에 따른 해상도

현재 행동에서 멀리 있는 기록일수록 상위 Report로 압축하고, 가까운 기록일수록 자세하게 읽는다.

```text
이전 Chain들               → Chain Report
현재 Chain의 이전 Cycle들 → Cycle Report
현재 Cycle                 → Step Report
현재 Node                  → 상태, 요구 schema, 허용된 다음 행동
```

이를 Context Resolution Rule이라 부른다.

```text
Project
├─ Previous Chains          coarse: Chain Report
└─ Current Chain
   ├─ Previous Cycles       medium: Cycle Report
   └─ Current Cycle
      ├─ Previous Steps     fine: Step Report
      └─ Current Node       exact: current state and rules
```

상위 Report가 존재하는 닫힌 범위의 내부 Graph를 기본 context에 다시 펼치지 않는다.

---

## 3. 압축은 삭제가 아니다

context에서 과거 Step을 생략해도 Step Node와 Report는 Graph에 그대로 존재한다.

- Knowledge를 삭제하지 않는다.
- 실패 가지를 삭제하지 않는다.
- Artifact snapshot을 삭제하지 않는다.
- 필요하면 history 또는 명시적 조회로 세부 근거를 읽을 수 있다.

압축은 무엇이 존재하는지를 바꾸지 않고, 현재 목적에 어떤 해상도로 투영하는지만 바꾼다.

`existence_ref`와 `journey_ref`는 기본 context에서 재귀적으로 펼치지 않는다. 복원은 동일한
원본 Existence와 Journey를 선택한다는 뜻이며 전체 역사와 모든 Knowledge·Memory를 프롬프트에
복제한다는 뜻이 아니다.

```text
Node 하나의 provenance              O(1) reference
Journey revision                    고정된 구성 요소 head reference
기본 Action Context                 선택된 현재 투영
명시적 history 또는 ref 조회        요청한 범위만 펼침
```

renderer는 historical provenance reference를 만났다는 이유만으로 그 reference가 다시 가리키는
과거 reference를 자동 확장하지 않는다.

---

## 4. `gil story`

`gil story`는 인간을 위한 현재 Journey의 설명이다.

주요 사용자는 CLI나 GUI를 직접 사용하지 않고 AI와 채팅하는 사람이다. 대화 세션에는 이미 최근
맥락이 있으므로 전체 실행 기록을 매번 반복할 필요가 없다.

기본 story는 다음 질문에 답한다.

- 지금 어떤 큰 문제를 다루고 있는가?
- 현재 어떤 실험을 하고 있는가?
- 어디까지 진행됐는가?
- 이 위치로 오게 만든 중요한 성공·실패와 전환은 무엇인가?
- 다음에 무엇을 하려는가?

기본 story는 다음을 하지 않는다.

- 모든 이전 Chain의 내부 Cycle을 펼치지 않는다.
- 모든 이전 Cycle의 Step을 펼치지 않는다.
- LLM onboarding에 필요한 schema 전체를 보여주지 않는다.
- 전체 감사 로그를 대신하지 않는다.

story의 문장은 `GIL Specification v0.1`의 Report 작성 불변식을 따라 객관적이고 자립적으로
읽혀야 한다.

---

## 5. `gil context`

`gil context`는 새로운 Agent 세션이 현재 위치에서 작업을 이어가기 위한 계층적 onboarding
입력이다.

명시적으로 다음과 같은 때 호출한다.

- 새로운 AI 세션이 시작될 때
- 다른 Agent가 프로젝트를 이어받을 때
- 대화 context가 압축되어 과거 작업 설명이 사라졌을 때
- 장시간 중단한 작업을 재개할 때
- 인간 또는 Agent가 현재 실행 context의 재구성을 요청할 때

새 Step이나 Node를 열 때마다 전체 `gil context`를 자동으로 출력하지 않는다. 같은 세션에서는
이미 전달된 대화 context를 사용하고, 새 Node의 역할·필수 Report·다음 행동에 관한 delta만
안내한다.

### Context의 최소 구성

```text
[GIL 규칙]
  현재 Node에 적용되는 문법과 Report 원칙

[프로젝트에서 이어받은 것]
  이전 Chain들의 Chain Report

[현재 Chain]
  승인된 방향
  이전 Cycle들의 Cycle Report
  현재 Journey에서 반드시 알아야 하는 실패 Cycle Report

[현재 Cycle]
  현재 Cycle의 Define
  현재 Cycle 안에서 지금까지 생성된 Step Report
  현재 Artifact 상태

[현재 위치]
  Current Node와 상태
  clean / dirty
  허용된 구조적 행동
```

아직 구현되지 않은 계층이나 상태는 출력하지 않는다. 존재하지 않는 Chain, Artifact snapshot 또는
dirty 상태를 있는 것처럼 표현하지 않는다.

Context는 현재 무엇이 참인지 기술한다. 현재 Agent가 실제로 무엇을 수행하려 하는지는 Journey의
Current Will이 담당하고, GIL Grammar는 현재 또는 작업 완료 뒤 허용되는 구조적 행동을 담당한다.
새 Agent onboarding에서는 세 절을 구분해 함께 읽는다.

```text
Action Context
= Lineage(World Current)
+ Journey(Existence Current)
+ Applicable Grammar
```

Will의 생명주기와 불변식은 `GIL Will Model v0.1`을 따른다. Context는 Current Node를 보고
구체적인 Will을 추측하거나 생성하지 않는다.

새 세션은 `GIL Existence Model v0.1`에 따라 World Current와 Existence Current를 독립적으로
복원한다. 모델이나 세션이 달라져도 같은 Existence를 선택하면 같은 Journey와 Current Will을
이어간다.

최초 Interview가 진행 중이면 context는 정상 Experiment 상태를 지어내지 않고, Current
Existence의 Relation·목표·Synthesis 중 아직 형성되지 않은 것을 보여준다. 승인된 Synthesis
이후에는 Current Existence와 사용자와의 Relation을 onboarding에 필요한 해상도로 투영한다.

실행형 Open Node에서는 Active Will을 별도 절로 보여준다. Applicable Grammar는 실제 작업을
마친 뒤 Report와 함께 `gil close`를 실행하면 Will Done과 Node Close가 함께 확정된다고
표시한다.

### 실패 지식

해상도 압축은 실패 지식을 누락할 이유가 아니다.

- 닫힌 과거 Cycle의 실패는 Cycle Report 해상도로 전달한다.
- 현재 열린 Cycle 안의 실패한 Step 접근은 Step Report 해상도로 전달한다.
- 현재 방향과 무관한 세부 Step은 기본 context에서 생략할 수 있다.
- 동일한 실패를 피하는 데 필요한 Report는 반드시 포함한다.

정확한 relevance 선택 알고리즘은 이후에 결정한다. v0에서는 구조적 lineage와 명세가 항상
전달하도록 요구한 실패 Report를 기준으로 한다.

### 반복 실패의 누적 투영 — v0 구현 경계 (2026-10-31)

Cycle Model §10의 실패 전달은 직전 `revisit_from` 하나로 끝나지 않는다. 현재 Cycle과
`pending_cycle_revisit`의 출처를 시작점으로, 각 Cycle의 `parent`와 `revisit_from`을 따라
확정된 실패 경험을 수집한다. 구조적 lineage 계산은 여전히 parent만 따른다. Context 수집에
provenance를 사용한다고 해서 실패 Cycle이 성공한 현재 가지의 조상이 되는 것은 아니다.

- 같은 조상으로 C3 실패→C4 실패→C5를 시작하면 C3·C4의 요약을 모두 전달한다.
- 새 가지가 성공하고 일반 자식 Cycle을 열어도 parent의 provenance를 통해 앞선 실패를 유지한다.
- 더 먼 조상으로 돌아갈 때는 실패 출처가 딛고 있던 경로도 따라가 그 경로의 경험을 보존한다.
- 앞선 의존 경험부터 결정적으로 나열하고 Cycle별 한 번만 투영한다. ID 크기나 저장 배열 순서를
  경험의 의미로 사용하지 않는다. 현재 Cycle의 실패는 현재 절에 이미 있으므로 과거 절에 중복하지 않는다.
- 과거 실패도 §7.1의 선택적 투영(Define의 목적/성공 기준, 판정 Outcome의 교훈, Cycle Report의
  판정/인수인계/다음 방향)과 원본 CycleRef만 싣는다. 내부 Step·임의 추가 필드를 전부 펼치지 않는다.
- 단순히 저장소에 존재하는 모든 실패를 나열하지 않는다. 현재 경로와 pending의 검증된 참조로
  연결된 경험이 대상이다. 다른 Existence의 Journey를 자동 병합하거나 relevance 점수를 만들지 않는다.

수집은 read-only이며 원본 Report·Journey·Artifact를 수정하지 않는다. 별도 실패 목록을 저장하거나
이름을 재발급하지 않으므로 기존 format 4/5도 같은 참조에서 재구성한다. 이 투영 개선은 제품의
restore/revisit 안전 차단을 해제하지 않는다.

---

## 6. `gil history`

`gil history`는 전체 실행 경로를 감사하고 조사하기 위한 투영이다.

다음을 펼칠 수 있어야 한다.

- 모든 Chain과 Chain Report
- 모든 Cycle과 Cycle Report
- 모든 Step과 Step Report
- parent와 revisit_from
- 실패 가지와 전환점
- Artifact snapshot reference

history는 기록의 완전성을 우선하고 context 크기를 줄이는 것을 목표로 하지 않는다.

v0에서 명령이 아직 구현되지 않았더라도 전체 Graph는 저장되어야 한다. story 또는 context에서
생략했다는 이유로 원본을 잃어서는 안 된다.

---

## 7. 구현 단계별 Context

### 7.1 M2에서의 최소 Context

M2에는 Chain과 Artifact snapshot이 아직 없다(Artifact는 M3에서 들어온다 —
`GIL Artifact Model v0.1`). 따라서 첫 `gil context`는 다음으로 제한한다.

```text
[이전 Cycle]
  현재 Cycle의 ancestor를 Cycle 해상도로 선택적 투영
  Define에서 실험 목적과 성공 기준을 읽음
  outcome_ref가 가리키는 Outcome에서 판정의 교훈을 읽음
  Cycle Report에서 verdict, handoff, next_direction을 읽음
  ancestor Cycle의 Step Graph는 펼치지 않음

[현재 Cycle]
  현재 Cycle의 Step Report

[현재 위치]
  Current Cycle과 Step
  허용된 다음 행동
```

검사 가능한 핵심 불변식:

```text
previous_cycle_step_graph_expansions_in_context == 0
previous_cycle_projections_in_context == ancestor_count
current_cycle_step_occurrences_in_context == current_cycle_step_count
```

현재 Cycle이 첫 Cycle이라면 이전 Cycle 절은 없다.

여기서 "Cycle Report 해상도"는 Cycle Report에 물리적으로 저장된 필드만 출력한다는 뜻이
아니다. Cycle을 대표하는 정보를 원본 Graph와 구조적 참조에서 읽되, 내부 Step의 시간순
전개를 다시 보여주지 않는다는 뜻이다.

이전 Cycle의 선택적 투영은 다음 원본만 읽는다.

```text
실험 목적과 성공 기준 → 그 Cycle의 유일하고 immutable한 Define
판정의 교훈           → Cycle Report.outcome_ref가 가리키는 Outcome
판정과 인수인계       → Cycle Report.verdict, handoff_summary, next_direction
```

이 투영은 Define이나 Outcome의 내용을 Cycle Report에 복제하여 저장하지 않는다. Step ID,
Hypothesis, Verify, Analysis와 내부 전환 과정을 펼치지 않으므로 Step Graph 확장이 아니다.
`handoff_summary`는 다음 Cycle이 반드시 받아야 할 지식에 집중하며, 실험 목적·성공 기준·판정
이유를 모두 다시 서술할 책임을 지지 않는다.

### 7.2 M3 Artifact Context

M3의 `gil context`는 전체 Snapshot registry나 파일 목록을 반복하지 않는다. 새 Agent가 지금
행동을 이어 가는 데 필요한 세계 정보만 현재 위치 해상도로 투영한다.

```text
[현재 세계]
  현재 위치에서 유도한 공개 SnapshotRef
  작업 폴더가 그 세계와 같은지(clean / dirty)
  현재 열린 Node가 변경을 확정할 수 있는 Verify인지
  dirty인 비-Verify 자리라면 의도 확인·필요한 변경 보존과 제품이 허용하는 복구 절차 안내
```

내부 manifest·blob 주소, Snapshot registry 전체, 모든 파일의 경로와 변경 목록, 이전 Cycle마다
반복되는 Artifact 세부는 출력하지 않는다.

`gil context`는 명시적으로 요청될 때 현재 폴더를 한 번 관측할 수 있다. 그러나 모든 `open`과
`close`가 전체 Context를 되풀이하지는 않는다. 일상 명령은 현재 행동에 필요한 짧은 nudge를
주고, 전체 onboarding은 새 세션이나 사용자가 요구한 때에만 `gil context`가 맡는다.

---

## 8. Story와 Context의 차이

두 투영은 비슷한 정보를 읽을 수 있지만 독자와 책임이 다르다.

| | story | context |
|---|---|---|
| 독자 | 인간 | 새 Agent 세션 |
| 목적 | 현재 상황 이해 | 작업 재개 |
| 표현 | 자연어 중심 | 구조적이고 빠짐없는 onboarding |
| 문법 정보 | 필요한 만큼 | 현재 행동에 필요한 규칙 포함 |
| 과거 세부 | 중요한 전환만 | 계층별 Report 해상도 |
| 전체 감사 | 하지 않음 | 하지 않음 |

두 투영 모두 과거 범위의 내부 Step 전체를 기본으로 반복하지 않는다.

---

## 9. Node Open의 출력

Node Open은 전체 context 재전송 사건이 아니다.

최소 delta만 보여준다.

```text
- 새 Node의 Kind와 상태
- 이 Node가 답해야 하는 핵심 질문
- Close에 필요한 Report 필드
- Current Will의 delta 또는 Active Will이 없다는 사실
- 작업 완료 뒤 허용되는 구조적 행동
```

새 세션 여부를 GIL이 추측하여 자동으로 전체 context를 출력하지 않는다. Agent 또는 host가
세션 경계에서 `gil context`를 명시적으로 호출한다.

---

## 10. Renderer

story, context와 history는 read model이며 renderer와 분리한다.

```text
Read Model
├─ plain text
├─ Markdown
├─ HTML
└─ structured data
```

renderer가 바뀌어도 포함되는 지식의 해상도와 의미가 바뀌어서는 안 된다. Monitor는 같은 read
model을 사용하되 사람이 필요할 때 상위 Report에서 내부 Graph로 점진적으로 펼칠 수 있게 한다.

여기서 같은 read model은 story, context와 Monitor가 같은 거대한 출력 객체나 같은 문자열을
공유한다는 뜻이 아니다. 검증된 원본 사실의 한 Snapshot에서 독자별 projection을 만든다는
뜻이다. Monitor의 Snapshot 경계, 읽기 일관성, 부재 표현과 renderer 안전 규칙은
`GIL Monitor Model v0.1`을 따른다.

---

## 11. 현재 결정하지 않는 것

- `gil context`의 Markdown·JSON 출력 schema
- context token budget과 자동 절단 정책
- relevance scoring
- 오래된 Chain Report를 Project 수준으로 다시 압축하는 규칙
- 분산 Agent 사이의 context 전송 형식
- `gil history`의 필터와 검색 문법
- Monitor의 접기·펼치기 UI
- 시각 자료를 context에 포함하는 조건
- Journey 구성 요소별 context 투영과 token budget
- 특정 provenance reference를 명시적으로 펼치는 조회 문법

---

## 12. 핵심 문장

> **GIL은 기억을 무조건 많이 전달하지 않는다. 현재 위치와 계층적 거리에 따라 필요한
> 해상도로 지식을 전달한다.**

> **story는 인간이 현재를 이해하기 위한 것이고, context는 새 Agent가 현재에서 다시 시작하기
> 위한 것이며, history는 전체 경로를 감사하기 위한 것이다.**

> **압축은 삭제가 아니다. 상위 Report 뒤에 있는 내부 Graph는 필요할 때 다시 읽을 수 있다.**

## M6-A 현재 Chain 투영 (2026-10-31)

[실제 Chain 저장](GIL_Chain_Storage_v0.1.md)을 쓰는 개발 프로젝트에서만 현재 Chain의
참조·Open 상태·Existence·Entry·현재 Cycle을 먼저 표시한다. 아래 Cycle/누적 실패/Step 해상도는
그대로 유지한다. 4/5 기록에는 Chain을 지어내지 않는다.

M6-C (2026-11-01): 다음 Chain을 명시적으로 열면 이전 Chain들의 ID·판정·closing_synthesis_ref·
handoff_summary를 생성 순서로 읽는다. 이전 Chain의 전체 Cycle/Step 본문을 기본 Context에
펼치거나 Report를 새로 요약해 저장하지 않는다. 세부 이력은 `gil story`, Monitor 타임라인,
Node detail과 증거 이미지 조회로 읽는다. 새 Opening Interview의 사용자 승인은 별도로 필요하다.
과거 승인 문자열을 새 주제에 대한 승인으로 간주하지 않는다.
