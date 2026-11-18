# GIL Cycle Model v0.1

> Chain의 Cycle Graph, Cycle의 공통 수명, Interview Cycle의 최소 문법을 정의한다.

## 1. 목적

이 문서는 `GIL Specification v0.1`과 `GIL Node Model v0.1`이 열어 둔 다음 영역을
구체화한다.

- Chain 안에서 Cycle이 형성하는 Graph
- Cycle의 Open / Close와 Report
- Cycle 수준의 success / failure와 revisit
- Interview Cycle과 Experiment Cycle의 역할
- Interview Cycle의 최소 Step Grammar
- Chain의 시작과 종료에서 인간이 맡는 역할

이 문서는 구현 형식, 저장 엔진, 다중 부모 Merge를 정의하지 않는다. Artifact Snapshot의 계약은
`GIL Artifact Model v0.1`이 갖는다.

---

## 2. 재귀적 계층

GIL의 Reasoning 구조는 재귀적이다.

```text
Project
└── Chain Graph
    └── Chain
        └── Cycle Graph
            └── Cycle
                └── Step Graph
                    └── Step
```

각 계층에서 같은 원칙이 반복된다.

- 성공한 Node는 다음 자식 Node의 기반이 될 수 있다.
- 실패한 Node는 자식을 만들지 않는다.
- 실패는 삭제되지 않고 Report와 Knowledge로 Journey에 남는다.
- 실패 뒤에는 과거의 유효한 분기점으로 revisit하여 그 아래에 새 가지를 만든다. 직접 부모로
  돌아간 경우에만 실패 가지와 새 가지가 형제다.
- revisit은 과거 Node를 다시 열거나 수정하지 않는다.

failure는 막다른 오류가 아니다.

> **failure는 현재 가지의 가설이 충족되지 않았음을 나타내며, 지식을 유지한 채 다른 가지를
> 시도하기 위한 논리적 발판이다.**

---

## 3. Cycle Graph

Chain은 Cycle Graph를 포함한다.

Cycle Graph는 DAG를 형성할 수 있으며 부모·자식과 형제 관계를 가진다.

```text
Chain
└─ Cycle A
   ├─ Cycle B
   └─ Cycle C
```

Cycle B와 Cycle C는 Cycle A를 부모로 하는 형제다.

Cycle ID는 프로젝트 전체에서 유일하며 `cycle:C2`처럼 참조한다. Step은 Cycle 안에서 번호를
부여받고 `step:C2/S3`처럼 소속 Cycle을 포함해 참조한다. 현재 Cycle이 자명한 story나 status의
`#3`은 인간용 축약일 뿐 저장 reference가 아니다. 상세 규칙은 `GIL Node Model v0.1` §2.1을
따른다.

### 실패한 Cycle은 자식을 만들지 않는다

```text
Cycle A
└─ Cycle B (failure)
   └─ Cycle C              허용하지 않음
```

Cycle B가 실패한 뒤 새로운 시도를 하려면 유효한 Closed ancestor로 revisit한다.

```text
Cycle A
├─ Cycle B (failure)
└─ Cycle C
```

Graph에서 B와 C는 형제지만 Journey에는 B의 실패 경험이 남는다.

```text
A → B의 실패 경험 → A revisit → C
```

---

## 4. Cycle Kind

Cycle에는 최소 두 Kind가 있다.

```text
cycle.kind:
- interview
- experiment
```

두 Kind는 같은 Cycle 수명을 공유한다.

- Entry / Exit Boundary
- Open / Closed 상태
- 내부 Step Graph
- Cycle Report
- Artifact Exit 상태 (`GIL Artifact Model v0.1` §7)
- Kind에 따른 verdict
- revisit

차이는 내부 Step Grammar와 역할이다.

### Interview Cycle

사용자의 모호한 의도를 작은 문장으로 만들고, AI의 이해가 맞는지 인간에게 확인한다.

### Experiment Cycle

고정된 Define과 성공 조건 아래에서 하나의 실험 가설을 검증한다.

> **하나의 Experiment Cycle은 하나의 실험에 대응한다.**

---

## 5. Chain의 첫 Cycle

Chain의 첫 Cycle은 반드시 Interview Cycle이다.

프로젝트 최초 `gil start`도 같은 규칙을 따른다. 최초 Existence와 빈 Journey를 만든 뒤 그
Existence가 소유한 Interview Cycle을 첫 Cycle로 연다. 사용자 Relation과 목표를 CLI 설정값으로
미리 받지 않고 이 Interview 안에서 처음 형성한다.

```text
Chain Entry
    ↓
Interview Cycle
    ↓
Experiment Cycle
```

Chain은 사용자의 자연어 요청을 곧바로 실험하지 않는다.

먼저 심층 인터뷰를 통해 다음을 수행한다.

- 사용자의 큰 질문을 명확한 문장으로 만든다.
- 사용자의 의도와 제약을 확인한다.
- 큰 문제를 실험 가능한 작은 명제로 분할한다.
- 사용자가 그 이해와 탐색 범위를 승인하게 한다.

### 5.1 심층 인터뷰로 요구를 구체화한다

**2026-11-17 사용자 결정.** 심층 인터뷰의 목적은 질문 수나 문서 분량을 늘리는 것이 아니라,
모호한 바람을 사용자가 판단하고 에이전트가 실행·검증할 수 있는 요구로 바꾸는 것이다.

**후속 합의:** 심층 인터뷰는 요구의 명확성과 탐색의 자유를 함께 보장한다. 탐색적 데이터 분석
(Exploratory Data Analysis, EDA)의 결론·패턴·원인을 사전에 정답으로 상정하지 않는다.
무엇을 알아보려는지, 어떤 자료·권한으로 어디까지 탐색할지, 근거와 한계를 어떻게 판단할지는
합의하되 관측 결과에 따라 작은 실험 질문을 발전시킨다. 아래 실행 후보는 출발점이지 확정된
정답이나 모든 실험의 목록이 아니다. 새 사실이 요구를 바꿀 때 재인터뷰하고, 같은 요구 안의
실험 세분화 자체는 명세 변경과 구분한다(제품 연결은 §7의 후속 구현 대상).

- 추상적인 표현에는 실제 사용 상황·입력·원하는 결과의 예와 원하지 않는 반례를 묻는다.
  “편리하게”, “잘 분석”, “전부 자동화”를 그대로 완료 기준으로 옮기지 않는다.
- 대상과 문제, 기대 결과, 포함·제외 범위, 제약, 우선순위, 판단 가능한 완료 조건을 구체화한다.
  사용자가 이미 답한 내용은 재사용하고, 다음 결정에 영향을 주는 미해결 쟁점부터 하나씩 묻는다.
- 사용자 답변으로 확인된 사실, 에이전트의 해석·권장안, 아직 모르는 것을 구분한다.
  사용자가 방법을 모르면 구체적인 선택지와 차이를 제시하며, 기술 절차 전체를 사용자에게 설계하게 하지 않는다.
- 큰 목표를 각각 결과와 완료 조건을 가진 작은 실행 후보로 나누고 선행 조건과 순서를 설명한다.
  첫 후보는 현재 자료·권한·승인 범위에서 바로 착수 가능해야 한다. 전제가 불확실하면 그 전제를
  확인하는 작은 조사·실험을 먼저 제안하고, 뒤의 작업을 이미 실행 가능한 것처럼 확정하지 않는다.
- 다음 항목을 실행하거나 승인하기 위해 중요한 모호성이 남으면 Question으로 돌아간다.
  해결과 무관한 질문, 고정 질문 수, 이미 확인한 요구의 반복은 심층 인터뷰의 합격 근거가 아니다.

합격 기준은 사용자가 “무엇이 달라져야 하는지”를 구체적인 예와 완료 조건으로 확인할 수 있고,
에이전트가 첫 실행 항목의 입력·범위·결과·검증 방법을 설명할 수 있다는 것이다.
미래의 모든 세부 절차를 확정할 필요는 없으며 새 사실은 재인터뷰의 근거가 된다.
제안의 간결한 표현은 §16, 실행 문제의 크기는 §7을 따른다.

프로젝트 bootstrap Interview에는 다음 최소 범위가 반드시 포함된다.

- 사용자가 원하는 이름·별칭·호칭을 물어 프로젝트 로컬 Human Participant를 식별한다.
- 최초 Existence의 Journey에 Human Participant와 어떤 관계인지 자립적인 자연어로 기록한다.
  소유자·도구 또는 명령자·하위 실행자 관계를 추론하지 않는다.
- 사용자가 무엇을 하고 싶은지 큰 질문을 수집한다.
- AI가 이해한 의도를 작은 명제로 제안한다.
- 사용자가 yes/no로 명제를 승인하며 no이면 이유를 다시 묻는다.
- 승인된 Synthesis가 최초 탐색 체크리스트를 만든다.
- 작업 목적에 맞는 파일 추적 제외 범위를 제안하고 사용자 승인을 받는다. GIL의 일반 추천과
  에이전트의 프로젝트별 추천을 `.gilignore`로 관리하는 의미는
  [Artifact Model §3.6](GIL_Artifact_Model_v0.1.md#36-인터뷰에서-정하는-명시적-추적-제외)을 따른다.
  제외 파일도 읽기·수정·삭제할 수 있으며 추적 제외는 불변성이나 접근 제한이 아니다.

2026-11-15 추적 범위 결정은 구현 대기다. 현행 시작은 첫 Snapshot과 Interview 개설을 함께
확정하므로, 최초 보관 전에 범위를 승인하는 절차의 연결이 필요하다(Artifact Model §3.6·§4).
재인터뷰에서의 제외 변경도 기존 단일 열린 Cycle과 파일 변경 경계를 우회하지 않는다.

승인된 Synthesis 전에는 Experiment Cycle을 열 수 없다.

Chain Open Report는 두지 않는다. Chain 생성 시에는 구조적 Open 상태만 존재하고, 첫 Interview
Cycle의 승인된 Synthesis가 Chain의 최초 탐색 범위를 만든다.

이 Synthesis는 큰 질문을 실험 가능한 작은 항목으로 원자화한 탐색 체크리스트를 소유한다.
체크리스트의 각 항목은 아직 Cycle Node가 아니라 앞으로 열 수 있는 Cycle 후보다.

v0에서는 체크리스트 후보를 독립적인 형제 가지로 미리 펼치거나 다중 부모로 합류시키지 않는다.
정상 진행에서는 체크리스트와 Interview 결과를 다음 Cycle로 직렬 전파하고, 현재 항목을 수행한
Cycle이 다음 항목의 Cycle로 이어진다.

```text
Opening Interview
        ↓ checklist: [A, B, C]
Experiment A
        ↓
Experiment B
        ↓
Experiment C
```

다만 Experiment가 실패하면 v0부터 Closed ancestor로 revisit하고, 그 ancestor 아래에 새 형제
Cycle을 열 수 있다. 이 실패 분기는 단일 Artifact 상태를 선택하여 되돌아가는 동작이며, 성공한
여러 Artifact를 결합하는 Merge가 아니다.

독립 체크리스트 후보들의 병렬 형제 실행, 성공 가지의 합류와 Artifact Merge는 v0에서 다루지
않는다.

---

## 6. Cycle Kind 사이의 연결

Chain의 첫 Cycle이 Interview여야 한다는 시작 규칙과, 성공한 Cycle의 자식 규칙은 구분한다.

```text
Interview(success)  → Interview | Experiment
Experiment(success) → Interview | Experiment
Experiment(failure) → 자식 없음
```

Interview Cycle은 `failure`로 닫히지 않는다. 인간이 승인한 Synthesis를 얻으면 `success`로
닫히고, 아직 얻지 못했다면 Open 상태로 남는다.

성공한 Experiment Cycle 뒤에 Interview Cycle을 열 수 있어야 한다.

```text
Interview A
└─ Experiment B (success, Artifact v2)
   └─ Interview C (Artifact v2 유지)
      └─ Experiment D
```

이는 실험 결과를 보존한 채 사용자에게 새로운 의문이나 Chain의 판정을 물을 수 있게 한다.

### Interview Cycle의 Artifact

Interview Cycle은 일반적으로 Artifact를 변경하지 않는다.

그러나 Artifact가 없다는 뜻은 아니다. Entry Snapshot 계승 규칙(`GIL Artifact Model v0.1`
§7.1·§7.5)에 따라 Interview Cycle은 저를 연 전이가 준 Artifact 상태를
그대로 상속하고, 변경하지 않았다면 같은 상태를 자신의 Exit로 전달한다.

```text
Interview Entry: Artifact v2
Interview Exit:  Artifact v2
```

Interview Cycle은 Artifact 관점에서 투명할 수 있다.

### 실패한 Experiment 뒤의 Interview

실패한 Cycle은 자식을 만들 수 없으므로 그 아래에 Interview를 열지 않는다. 과거의 유효한
Closed ancestor로 revisit한 뒤 그 아래에 새 Interview 가지를 만든다. 대상이 실패 Cycle의
직접 부모일 때 그 Interview가 실패 Cycle의 형제가 된다.

이때 실패한 Experiment의 Artifact 상태는 현재 상태로 채택되지 않지만, Report와 Knowledge는
Journey에 남아 새 Interview에 전달된다.

---

## 7. Experiment Cycle의 Define

하나의 Experiment Cycle에는 정확히 하나의 Define만 존재한다.

### Define의 명제 형식 — 2026-11-17 사용자 명확화

Define.problem은 반드시 예/아니오로 답할 수 있는 하나의 명제를 물음으로 제시한다.
‘확인한다’, ‘살펴본다’, ‘분석한다’ 같은 활동의 수행을 문제정의로 삼지 않는다. 질문의 대상·범위와
예/아니오를 구분할 관측 기준을 명확히 하며 ‘조사를 수행하고 보고서를 남겼다’를 명제의 충족으로
바꾸지 않는다. 활동 목록과 검증 절차는 명제를 판정하기 위한 방법이다.

예: ‘이상 날짜를 살펴본다’ 대신 ‘지정한 자료에서 정의된 허용 날짜 형식을 벗어나는 값이 존재하는가?’를
묻는다. ‘날짜를 통합할 수 있는지 확인한다’ 대신 ‘대상 파일의 모든 날짜를 합의한 규칙에 따라
정보 손실 없이 공통 형식으로 변환할 수 있는가?’를 묻는다. 문장 끝만 물음표로 바꾸는 것이 아니라
명제 자체의 판정 범위·기준이 있어야 한다. 결과를 본 뒤 기준이나 질문의 긍정·부정을 뒤집지 않는다.

근거 있는 ‘아니오’도 다음 탐색에 유용하지만 조사 수행의 완료를 이유로 명제가 충족됐다고
표시하지 않는다. 자료 부족·미검증으로 답할 수 없는 상태는 ‘아니오’의 근거가 아니며 판정 미도달로
구분해 보고한다. 이 명확화가 새 저장 판정값을 추가하거나 기존 실패·중단 계약을 변경하지는 않는다.
탐색은 정답을 미리 가정하는 것이 아니라 이러한 작은 질문들을 관측에 따라 구성하는 과정이다.

### 7.1 지금 실행 가능한 가장 작은 문제

**2026-11-17 사용자 결정.** Define은 Chain의 큰 목표나 제안서 전체를 복사하지 않는다.
심층 인터뷰에서 확인한 요구와 승인된 현재 실행 항목, 이미 확보한 결과·제약을 근거로
**현재 수행하고 결과를 판정할 수 있는 가장 작은 단위의 문제 하나**를 추론하여 정의한다.

“가장 작다”는 문장이 짧거나 파일·명령이 하나라는 뜻이 아니다. 다음 조건을 함께 만족하는
의미 있는 최소 실험 단위다.

- 하나의 구체적인 질문 또는 해결할 문제를 갖는다. 별도로 성공·실패를 판정할 수 있는 다른 목표는 섞지 않는다.
- 필요한 입력과 선행 결과·권한이 현재 확보되어 있거나, 바로 그것의 확인이 이번 문제다.
- 이번에 얻을 관측 가능한 결과와 완료 판단이 명확하다. 후속 항목까지 끝내야만 성공을 판단할 수 있다면 다시 나눈다.
- 범위와 멈출 지점이 분명하며, 다음 문제는 별도 후보로 남긴다.
- 더 나누었을 때 독립적인 질문·검증 결과가 생기지 않는 단순 준비 동작까지 별도 Cycle로 만들지는 않는다.

예: Chain 목표가 “매출 자료로 개선안을 만든다”라면, 한 Define에 자료 점검·원인 분석·차트·개선안 작성까지
묶지 않는다. 첫 문제는 “제공된 표의 날짜·금액 열이 월별 집계에 사용할 수 있는가”로 정할 수 있다.
그 판정 기준은 필요한 열이 존재하고 날짜·금액의 형식과 누락 상태가 합의한 월별 집계 요건을 충족하는지다.
요건 충족 여부의 실제 관측을 근거로 답하며, 확인 작업이나 보고서 작성 자체를 성공 조건으로 삼지 않는다.
원인 분석과 개선안은 이 결과를 받은 뒤의 문제다. 실행 시간·파일 수·단어 수만으로 크기를 판정하지 않는다.

Problem은 해결할 질문과 범위를, Success Condition은 같은 문제의 관측 가능한 판정 조건을 담는다.
작업 방법의 가설은 Hypothesis에서 세우고 구체적인 실행은 Verify에서 수행한다. 제안 단계의 실행 순서는
출발점이며, 방법을 Define의 고정 계약에 불필요하게 묶지 않는다.

**2026-11-17 후속 사용자 결정 — 탐색적 Define(설계 확정 방향; 첫 실행 흐름은 개발 프로필
`exploration-flow`의 새 Chain에 구현 — [탐색 실행 계약](GIL_Approved_Plan_v0.1.md#탐색-실행-계약--개발-프로필-exploration-flow)).** Interview의 승인 요구와
각 Experiment의 문제는 같은 크기일 필요가 없다. 하나의 요구에 여러 작은 실험이 기여할 수 있고,
Define의 제목·문제·완료 조건을 승인 항목과 동일하게 강제하지 않는 방향으로 계약을 재설계한다.
각 Define은 승인 요구와의 관련성, 이번에 확인할 최소 문제와 관측 가능한 완료 조건을 설명한다.
앞선 실험 결과로 다음 문제를 정하되 하나의 Cycle에는 하나의 고정된 Define만 둔다.
**두 번째 사용자 결정:** 다음 실험은 승인 요구와의 관련성, 자료·선행 결과의 준비 상태,
줄일 불확실성을 근거로 선택한다. 예상 순서는 관측에 따라 바꿀 수 있으나 필요한 선행 조건은
유지한다. 베이즈 이론처럼 증거로 가설·해석을 갱신하며 탐색 범위를 좁힌다. 수치 확률 계산의
의무화나 반증을 무시한 일방적 축소를 뜻하지 않으며, 새 증거에 따라 후보를 다시 열 수 있다.
제품의 순서·연결 계약은 후속 구현 대상이다.

작은 실험의 성공을 전체 요구 완료로 세지 않는다. Chain 종료에서 요구별 답·증거·미충족 부분을
종합하여 승인된 기준으로 판단한다. 요구·범위·제약 변경은 재인터뷰로 승인받는다.

이미 고정한 Define이 너무 커서 충족하지 못했다면 현재 실험은 failure로 닫고, Analysis에는
범위 과대의 근거를, Outcome의 반성에는 다음 문제를 축소할 필요를 남긴다. 기존 Define을
바꾸거나 과거 실패를 성공으로 고치지 않는다. 실패 후 새 실험을 여는 전이는 탐색 실행 계약의 Chain에서만 구현됐다:
명시적 revisit과 복원 뒤, 같은 승인 요구에 다른 작은 Define을 연결해 연다. 그 밖의 Chain에서는 미결이다.
[승인 계획 계약](GIL_Approved_Plan_v0.1.md#실행-항목의-크기와-define의-일치)의 기존 제품 제약은
후속 구현 전까지 유지한다. 이번 합의는 저장·바인딩·큐·복구 규칙을 즉시 해제하지 않는다.

### 7.2 기존 수명과 불변 조건

Cycle도 공통 Node이므로 Cycle을 Open한 Existence와 Close한 Existence는 같아야 한다. Cycle이
Open인 동안 다른 Existence로 전환할 수 없다. v0에서 하나의 실험을 여러 Existence가 나누어
소유하거나 진행하지 않는다.

Cycle은 내부 Step Graph를 담는 컨테이너 Node이므로 Cycle 자체가 장기 Active Will을 점유하지
않는다. Current Will은 Cycle 안에서 현재 실제 행동을 수행하는 가장 깊은 Step에 대응한다.
Cycle Close는 별도의 Cycle Will Done이 아니라 모든 내부 Step의 Close와 Cycle Report 조건을
검사한다.

Define이 닫히면 다음 두 항목은 바꿀 수 없다.

- Problem
- Success Condition

> **새로운 Define은 새로운 실험이며, 따라서 반드시 새로운 Experiment Cycle을 시작한다.**

같은 Cycle에서 바꿀 수 있는 것은 가설과 접근 방법이다.

```text
Define
├─ Hypothesis A
└─ Hypothesis B
```

Define이 잘못됐거나 성공 조건이 부족하다는 사실을 발견했다면 현재 Cycle은 `failure`로
닫는다. 실패 이유는 Report에 기록하고, 새로운 Define은 새 Cycle에서 작성한다.

---

## 8. Cycle Verdict

Cycle의 verdict는 정확히 두 값뿐이다.

```text
success
failure
```

별도의 `invalidated`, `inconclusive`, `aborted` verdict를 추가하지 않는다. 구체적인 이유는
Report에 기록한다.

### Experiment Cycle의 판정

Experiment Cycle의 verdict는 실험을 통해 가설과 성공 조건이 충족되었는지만 말한다.

```text
가설과 성공 조건 충족     → success
가설 또는 성공 조건 불충족 → failure
검증 불가능               → failure
실험 설계가 잘못됨         → failure
```

실험이 명제의 참·거짓을 알아내는 데 유용한 정보를 주었다는 이유만으로 success가 되지는 않는다.
가설이 충족되지 않았다면 failure다.

### Interview Cycle의 판정

Interview Cycle은 `success`로만 닫힐 수 있다. 인간이 Synthesis를 명시적으로 승인해야만
그 success가 성립한다.

따라서 Success의 근거는 Cycle Kind에 따라 분리된다.

```text
Experiment success → 해당 Cycle의 고정된 Define과 Success Condition
Interview success  → 인간이 yes로 승인한 Synthesis
```

Define에 근거해야 한다는 기존 Success 불변식은 Experiment에 적용된다. Define이 없는
Interview에 그 규칙을 확장하지 않는다.

Interview의 `no` 응답은 곧바로 Cycle failure가 아니다. AI의 현재 이해가 맞지 않다는 관측이며,
질문을 더 이어가야 한다.

사용자가 현재 방향의 탐색을 중단하거나 Chain의 대주제 자체가 잘못됐다고 확인한 경우에도,
AI가 그 의도를 정확한 Synthesis로 제시하고 사용자가 승인했다면 Interview는 성공한 것이다.
그 Synthesis가 Chain 종료를 제안한다면 `chain_verdict`가 `failure`일 수 있다.

필요한 인간 응답이나 승인을 아직 얻지 못했다면 논리적 failure를 만들지 않는다. Question 또는
Synthesis가 Open인 채로 남고, 따라서 Interview Cycle도 Open 상태로 남는다. 실행 오류나 연결
중단 역시 이 문서가 정의하는 Interview verdict가 아니다.

---

### 완료할 수 없는 작업의 중단과 다음 방향

**2026-11-15 제품 방향 확정, 상세 전이 계약·구현 대기.** 검증을 끝내지 못한 작업도
중단 이유와 부분 결과를 사실대로 정리하고, 기록과 결과물을 보존한 채 다음 행동을
선택할 수 있어야 한다. 이 요구만으로 현재 열린 단계를 강제 종료하거나 새 명령을 허용하지 않는다.

- 앱·대화의 연결만 끊긴 경우는 작업 종료가 아니다. 기존 기록을 읽고 같은 작업을 이어간다.
- 검증을 마쳐 실패를 확인한 경우는 기존 실패 종료·복구 계약을 따른다.
- 검증을 완료하지 못한 경우는 수행한 범위, 막힌 이유, 미검증 내용을 구분한다.
  중단을 성공이나 가설의 반증으로 꾸미지 않는다. **2026-11-19 결정:** 검증 미완료는
  원본 보고서 본문에 명시하며 별도 판정값·구분 필드·잠정 정리 저장소를 추가하지 않는다.
  수행한 범위, 실제 관측과 근거, 검증하지 못한 부분, 막힌 이유를 짧고 명확하게 구분한다.
  반증 근거가 없다면 「이번 실험은 검증을 완료하지 못했으며 원래 질문의 답은 확인하지 못했다」고
  밝힌다. 실행을 시도하지 못한 경우도 그대로 적고 증거를 지어내지 않는다.
  검증 단계의 실제 관측을 분석·결과·실험 보고서에서 참조하며, 후속 요약·인계에서도
  미검증이라는 한계를 생략해 반증으로 바꾸지 않는다. 정해진 새 양식이나 중복 장문을 요구하지 않는다.
  이 결정은 열린 단계의 강제 종료·행동 완료 선언을 허용하지 않는다. 실패로 종료한 뒤의
  전체 복원 원칙은 같으며, 안전하게 보존·종료하지 못하는 경우는 아래 현재 상태 유지 원칙을 따른다.
- **2026-11-19 후속 결정 — 실행 약속 미달성의 실패 종료:** 원래 실행 약속과 완료 조건은
  보존한다. 수행하지 못한 범위와 이유·관측 근거를 원본 보고서에 남긴 뒤 시도를 실패로 종료하는
  방향을 채택한다. 사후에 약속을 「중단 사유 작성」으로 축소하거나 미수행 행동을 달성했다고
  표시하지 않는다. 시도의 종료와 행동 약속의 달성은 다르며, 이 실패가 원래 질문의 반증을 뜻하지 않는다.
  실험 단계의 보고·분석·반성과 실험 실패 종료를 연결하고, 반성에서 정한 근거 있는 복원점으로
  세계 전체를 명시적으로 복원한다. 단계마다 새 failure 판정값을 넣거나 중간 단계를 건너뛰라는 뜻이 아니다.
  실행 약속의 미달성 종료와 현재 Will 완료 처리의 정합은
  [Will Model의 미달성 종료 계약](GIL_Will_Model_v0.1.md#실행-약속을-달성하지-못한-시도의-종료)을 따른다.
  제품 방향은 확정했으며, 상세 저장·원자적 종료·재시작 검증은 추가 구현 범위다.
  보존이나 안전한 종료 자체가 불가능하면 현재 상태를 유지한다. 연결 끊김·응답 대기를 자동 실패로 바꾸지 않는다.
- 부분 결과가 있으면 다음 행동을 안내하기 전에 실제 내용이나 접근 가능한 링크를 보여 준다.
  없는 결과를 만들거나, 보존에 실패한 파일을 보존 완료로 보고하지 않는다.
- 실패로 닫은 실험은 **명시적 revisit → 복원된 세계에서 다음 실험 또는 허용된 인터뷰**로
  이어간다. 부분 결과는 증거로 보존하되 현재 세계에 선별 유지하지 않는다. 변경 파일이 없어도
  그래프상의 복원을 생략하지 않는다. 아직 닫히지 않은 대기·연결 중단은 실패로 꾸미지 않는다.
  상태·승인·파일 보호 조건에 따라 가능한 경로와 막힌 이유를 안내하고 복원 전 증거 보존을 확인한다.
- 기존 승인은 새 작업 범위로 확대하지 않는다. 이전 상태로의 복구는 승인 복구 계약의
  적용 범위를 따르며, 자동 승인 모드를 열린 단계 강제 종료나 계획 변경 승인으로 확대하지 않는다.
- 과거 기록 삭제, 가짜 완료, 필수 증거 생략, 여러 작업 주기의 동시 개설로 막힘을 우회하지 않는다.
  환경 문제로 안전한 보존·종료 자체가 불가능하면 현재 상태와 막힘을 유지한다.

원본 본문에 미검증 사실을 기록하고 원래 실행 약속을 보존한 채 실패로 종료하는 방향은 위 결정으로 확정했다.
[과거 중단 설계안](proposals/M6E_Open_Step_Interruption_v0.1.md)의 약속 축소·부분 세계 유지 후보를 채택한 것은 아니다.
**2026-11-19 후속 결정 — 복원 뒤 다음 실험:** 실험 재시도에 고정 횟수 제한을 두지 않는다.
반성에서 이전 관측·실패 원인·달라진 조건 또는 방법·다음에 줄일 불확실성을 근거로 제시하고,
이미 승인된 범위·비용 조건·권한 안이면 새 인터뷰나 반복 승인 없이 다음 작은 실험을 진행한다.
모든 항목에 새 변화가 있어야 한다는 기계적 조건은 아니지만, 동일 시도를 반복할 때도 왜 추가
관측이 필요한지 설명해야 한다. 횟수 제한이 없다는 것은 근거 없는 반복이나 무제한 지출의 허용이 아니다.
사용자가 정한 예산·자원·권한 한도와 중단 요청은 유지한다.

범위·비용 조건·권한이 기존 승인에서 달라지는 경우 변경 내용과 이유를 먼저 제시하고 사용자에게
재확인받은 뒤 실행한다. 승인된 예산 안에서 이미 허용한 사용을 반복 승인 대상으로 바꾸지 않는다.
요구 자체의 변경에는 해당 인터뷰·명세 승인 경계를 적용하며, 열린 실험 중에는 변경 요청을
보존하고 기존 단일 열린 주기 규칙을 지킨다. 승인 대기를 임의 실행으로 우회하지 않는다.
계속할 근거가 없으면 반성에서 멈춤·방향 변경의 근거를 마련하고 사용자에게 제시한다.
체인 종료는 종료 인터뷰의 사용자 승인을 거치며, 다음 체인은 새 인터뷰·새 승인을 받는다.

복원 자체의 승인·보존·보호 검사는 별개로 유지한다. 이 결정은 자동 복구를 활성화하거나
도구 통신·다운로드 등의 기술적 재시도 제한을 제거하지 않는다. 과거 2회 제한 후보는 채택하지 않는다.
미달성 종료의 상세 계약과 구현을 마치기 전에는 기존 검사·저장 규칙을 우회하지 않는다.

## 9. Cycle-level Revisit

### Artifact snapshot 경계 — 규범은 다른 문서가 갖는다

Cycle-level revisit은 **과거 Cycle Exit의 Artifact 상태로 돌아가는 동작**이므로 Artifact
Timeline 위에 선다. 그 계약의 규범 단일 진실 공급원은 `GIL Artifact Model v0.1`이다.

| 무엇 | 어디 |
|---|---|
| **Artifact 정체성** (경로 + 바이트) · 경로 정규화 · 관리 대상 종류 | Artifact Model §3 |
| 최초 기준 세계와 `gil start` | Artifact Model §4 |
| Snapshot 확정 권한 (start와 Verify close 둘뿐) | Artifact Model §5 |
| 비-Verify Step·Cycle close의 변경 거절 · manifest 비교 · 안정된 관측 | Artifact Model §6 |
| **Cycle의 `entry_snapshot_ref`·`exit_snapshot_ref` 저장 계약** | Artifact Model §7.1·§7.2 |
| Verify close 트랜잭션과 `snapshot_ref` 유도 | Artifact Model §7.3·§7.4 |
| World Current Snapshot 유도 | Artifact Model §7.6 |
| **Cycle Exit Snapshot 선택 규칙** | Artifact Model §7.5 |
| `gil restore` — 목표 유도 · 원자성 · 멱등성 · 확인 계약 | Artifact Model §8 |
| 저장과 원자성 경계 | Artifact Model §10 |

여기서는 **Cycle Graph가 지는 몫**만 적는다 — 어느 전이가 어느 세계에서 출발하는가.

### 전이가 Entry Snapshot을 정한다

모든 Cycle은 생성과 동시에 실재하는 `entry_snapshot_ref`를 지닌다. 그 값은 **그 Cycle을 연
전이가 출발한 세계**다.

| Cycle을 연 전이 | Entry Snapshot |
|---|---|
| 뿌리 (`gil start`) | 최초 기준 Snapshot |
| `open_child` | 부모 Cycle의 Exit |
| `revisit` | **revisit 대상 Cycle의 Exit** |
| merge (미래) | 별도 merge 규칙 — 아직 없다 |

> **구조적 `parent`와 갈래의 출처인 `revisit_from`을 같은 것으로 취급하지 않는다.**

`open_child`에는 `revisit_from`이 없다. revisit으로 열린 Cycle의 구조적 부모와 Artifact
세계의 출발점은 모두 revisit 대상이고, `revisit_from`만 버려진 실패 Cycle을 가리킨다. 이
셋을 각자 제자리에 두어야 "누구의 사고와 세계를 이어받았는가"와 "어느 실패가 이 새 갈래를
낳았는가"를 함께 답할 수 있다. 미래의 merge에서는 구조적 부모와 Entry 세계의 관계를 별도
규칙으로 다시 정한다.

### Cycle Exit

```text
Verify Closed   → 세계를 확정한다 (Artifact Model §5 — 바꿀 수 있는 유일한 경계).
                  바뀐 세계면 새 SnapshotRef, 그대로면 기존 것을 다시 가리킨다
Cycle Exit      → 새 Snapshot을 만들지 않는다.
                  마지막 Outcome의 구조적 lineage에서 가장 최근에 확정된 Verify
                  Snapshot을 기록하고, 없으면 그 Cycle의 Entry Snapshot을 계승한다.
```

버려진 가지의 Snapshot이나 이름이 가장 큰 Snapshot을 고르지 않는다. 이는 `basis_refs`(§16)와
`synthesis_ref`(§17)가 따르는 것과 **같은 계보 규칙**이다.

Entry와 Exit이 같은 `SnapshotRef`일 수 있다 — 그 Cycle이 세계를 바꾸지 않았다는 뜻이며,
Artifact를 건드리지 않는 Interview Cycle이 늘 그렇다.

### Staging은 GIL 상태가 아니다

GIL은 사용자에게 staging 상태나 `add` 동작을 노출하지 않는다. Verify Close가 현재 Artifact
tree 전체를 하나의 immutable snapshot으로 원자적으로 확정한다. 내부 저장 기법이 무엇이든
GIL Graph나 사용자 작업 흐름에 staging Node 또는 staging lifecycle을 추가하지 않는다.

### Revisit의 시간 규칙

Cycle revisit은 Step revisit과 같은 시간 규칙을 따른다.

- 대상은 현재 Cycle Lineage 위의 Closed ancestor여야 한다.
- 대상은 새 자식 Cycle을 가질 수 있는 유효한 분기점이어야 한다.
- 대상 Cycle 자체를 다시 열거나 수정하지 않는다.
- 대상 Cycle의 Exit에서 확정된 Artifact 상태로 돌아간다.
- Knowledge와 Journey는 되돌아가지 않는다.
- 이미 생성된 Cycle의 존재와 실패 기록은 사라지지 않는다.

```text
되돌아가는 것
- Current
- 대상 Cycle Exit의 Artifact 상태

되돌아가지 않는 것
- Knowledge
- Journey
- 실패 경험
- 이미 생성된 Cycle의 존재
```

revisit 뒤에 여는 새 Cycle의 구조적 부모는 **revisit 대상 Cycle**이다. 대상이 실패 Cycle의
직접 부모라면 새 Cycle과 실패 Cycle은 형제 관계가 된다. 더 먼 조상을 대상으로 삼으면 새
Cycle은 실패 Cycle의 형제가 아닐 수 있으므로, "형제"는 흔한 모양의 이름이지 일반 불변식이
아니다.

새 Cycle의 `entry_snapshot_ref`는 **revisit 대상 Cycle의 Exit Snapshot**이다. 실패 Cycle의
Exit을 계승하지 않는다. 실패 Cycle이 확정한 Snapshot 객체와 Report·Knowledge·Done Will은
그대로 남지만, 그 Artifact 모습은 새 시도의 현재 세계로 채택하지 않는다. 실패가 남긴 일부
Artifact를 선택적으로 가져오는 일은 revisit이 아니라 미래의 명시적 이월 또는 merge의 몫이다.

### 2026-11-17 사용자 결정: 세계 전체 복원과 배움의 보존

되돌리기는 관리 대상 세계 전체를 대상 Snapshot으로 복원한다. 세계의 작업물(코드 등)과
산출물(코드를 실행하여 만든 결과 등)을 함께 되돌린다. 산출물만 현재 폴더에 남기거나
‘대부분 성공했다’는 판단으로 일부 파일을 예외적으로 이월하는 것을 기본 경로로 삼지 않는다.
여기서 전체는 GIL이 추적·복원하는 세계의 범위이며 추적 제외 대상까지 복원한다는 뜻이 아니다.

자아의 경험·반성·Journey와 실패 증거는 되돌리지 않는다. 실패 Snapshot·보고서의 보존은
그 파일 상태를 다음 실험의 현재 세계로 채택하는 것과 다르다. 다음 시도는 복원된 세계와
축적된 배움에서 출발한다. 실패 결과물의 선택적 이월은 이번 결정에 포함하지 않는다.

예를 들어 Define이 ‘대상 파일들의 날짜를 통합할 수 있는가’이고 고정된 완료 조건에 미달하면,
일부 파일에 적용됐다는 비율로 성공을 정당화하지 않는다. Analysis에는 적용·실패 사례와
관측 범위를 정확히 남기고, 결측값·이상 날짜를 충분히 파악하지 못했다는 해석은 실제 증거와
연결한다. 확인되지 않은 원인은 다음 가설로 구분한다. Outcome의 반성은 날짜 분포·결측·이상치를
더 탐색할 필요를 담을 수 있다. 복원 뒤 다음 작은 Define에서 그 문제를 조사하여 더 충분한
변환 가설을 찾는다. 배운 가설은 이어받되 실패한 세계 상태는 자동 계승하지 않는다.

‘대부분’, 임의의 성공 비율, 산출물의 주관적 유용성은 기존 완료 조건을 대체하지 않는다.
이 결정은 세계 복원 원칙이며 자동 복구 승인이나 새로운 실패 자식 전이를 허용하지 않는다.

### 2026-11-17 사용자 결정: 반성에서 복원 근거와 다음 사이클을 결정

복원 기준점을 직전 상태로 자동 고정하지 않는다. 반성 단계의 주요 역할은 지금까지의 Step과
Cycle을 역으로 탐색하며, 어떤 Chain의 어떤 Cycle·Step에서 잘못된 정보·가정·해석이 생겼고
현재 판단에 어떻게 전파됐는지 근거로 설명하는 것이다. 시간상 앞섰다는 이유만으로 원인으로
단정하지 않는다. 관측 사실·해석·미검증 가설을 구분하고 관련 보고서·증거를 참조한다.

실패 후 반성에서는 원인 위치와 복원 후보를 구분하여 제시한다. 잘못된 정보가 발생한
Cycle의 종료 상태가 안전한 복원점이라고 자동 간주하지 않는다. 어떤 전제가 여전히 유효하고,
선택한 기준 세계가 잘못된 영향을 제거하며 다음 탐색에 적합한지 설명한다. 근거가 부족하면
원인을 미확정으로 남기고 추가 확인할 작은 문제를 제시한다. 모든 과거 기록을 매번 복사하지 않고
관련 참조를 따라 필요한 범위만 역추적한다.

**최종 합의:** 실패한 실험은 항상 명시적인 revisit을 거친다. 반성은 복원 여부의 예외를 만드는
대신 복원 기준점과 근거, 다음 최소 문제·입력·선행 조건을 마련한다. 파일 변경이 없어 복원 전후
세계가 같더라도 그래프상의 복원 경로와 새 시도는 기록한다. 실패의 작업물·산출물을 선별 이월하지
않는다. 성공 후 복원이 필요 없는 정상 진행에서는 반성이 다음 Cycle 또는 종료 인터뷰를 준비한다.
실패 판정이 복구의 자동 승인은 아니며 기존 수동/자동 복구 승인 설정과 대상 검증을 유지한다.

반성에서의 원인 판단과 복원 실행은 분리한다. 복원은 기존 승인·대상 검증·세계 전체 복원 경계를
따른다. 다른 Chain까지 원인 근거를 추적할 수 있어야 하지만, 다른 Chain의 상태로 실제 복원하는
기능은 별도 설계 대상이다. 현재 지원하지 않는 대상을 발견하면 이를 명시하고 가까운 다른 지점으로
몰래 대체하거나 과거 승인·기록을 수정하지 않는다.

여기서 반성은 논리적 역할을 뜻한다. 현재 Analysis/Outcome의 근거·lesson과 Cycle의 다음 방향에
연결할 구체적 계약은 후속 설계이며, 이 문서 변경으로 새 Reflection Step 종류나 저장 필드를
추가하지 않는다. 기존 ‘직전으로 기본 복원’ 설명 대신 근거 기반 기준점 선택을 따른다.

### Revisit의 상태 경계

revisit은 두 시간선을 구분한다.

```text
Artifact timeline
  대상 Cycle Exit에 새겨진 Snapshot을 현재 폴더에 다시 투영한다.

Journey timeline
  계속 전진한다. revisit 이전 상태로 복원하지 않는다.
```

복원은 개별 파일별 판단이 아니라 Snapshot 단위로 수행한다. 파일의 생성·수정·삭제는
snapshot 사이의 차이일 뿐, Cycle Graph가 파일별 규칙을 소유하지 않는다.

다음과 같은 GIL의 전역 상태는 Journey 시간선을 따르며 revisit의 영향을 받지 않는다.

- Cycle과 Step Graph
- Report와 Knowledge
- 실패 경험과 revisit 사건
- Existence와 현재까지 누적된 상태
- 실행된 Cycle 기록과 승인된 Synthesis

프로젝트 루트의 `.gil` 이 Artifact tree에서 제외된다는 것, 복원이 그 안을 건드릴 수 없다는
것, 그리고 더 깊은 곳의 `.gil` 이 중첩 GIL 경계로 거절된다는 것은
`GIL Artifact Model v0.1` §3.4가 정한다.

### Clean 세계 불변식

revisit은 현재 Artifact tree가 현재 경계의 `snapshot_ref`와 정확히 일치할 때만 실행할 수 있다.
기록되지 않은 생성·수정·삭제가 하나라도 있으면 dirty 상태이며 revisit을 거부한다.

```text
current Artifact tree == current snapshot_ref
  → revisit 허용

current Artifact tree != current snapshot_ref
  → revisit 거부
```

v0에는 dirty 상태를 덮어쓰는 강제 revisit을 두지 않는다. 현재 접근을 버리더라도 허용된 Step을
통해 변경 상태를 snapshot으로 확정하고, Analysis·Outcome과 Cycle Report를 작성하여 현재
실패 경로를 먼저 닫아야 한다. 실패의 Artifact snapshot과 Report는 Graph에 남는다.

Cycle revisit의 정당한 순서는 다음과 같다.

```text
1. 현재 Verify snapshot 확정
2. Analysis와 Outcome 작성
3. Cycle Report 작성, 실패 Cycle 종료
4. `gil revisit`으로 대상 Closed ancestor를 Current로 삼고 그 Exit Snapshot 복원
5. `pending_cycle_revisit`에 방금 닫은 실패 Cycle을 기록
6. 별도의 `gil open <kind>`로 대상 ancestor 아래에 새 Cycle 시작
```

Cycle-level revisit은 Step-level revisit과 같은 두 단계 구조를 따른다. `gil revisit` 자체는 새
Cycle의 Kind를 고르거나 새 Cycle ID를 발급하지 않는다. 다음 `gil open <kind>`가 새 Cycle을
만들며, 그때 `parent = revisit 대상`, `revisit_from = 실패 Cycle`,
`entry_snapshot_ref = revisit 대상의 exit_snapshot_ref`를 기록하고 pending 상태를 비운다.

같은 실패 Cycle에서 `gil revisit`을 두 번 실행할 수는 없다. 첫 실행 뒤 Current가 대상
ancestor로 옮겨졌기 때문이다. 그러나 그 뒤 열린 새 Cycle도 실패하면, 그 **새 실패 Cycle**의
Report가 같은 ancestor를 다시 대상으로 삼을 수 있다. 별도의 사용 횟수나 소진 플래그를 두지
않는다.

Step-level revisit도 같은 재귀 원리를 따른다. 현재 Step을 Report로 닫고 과거의 유효한 Step
경계의 세계로 복원한 뒤 새 Hypothesis 형제 가지를 연다.

내부 저장 엔진이 어떤 분기 기법을 쓰더라도 그것은 사용자가 직접 관리하는 별도 개념이 아니다.
새 Hypothesis 또는 새 Cycle 가지가 열릴 때 GIL Graph의 분기에 귀속되어 자동으로
생성되는 구현 세부이며, 그 어휘를 공개 표면에 노출하지 않는다
(`GIL Artifact Model v0.1` §14).

### 실행 경계

Cycle Report의 `revisit`은 **기록할 수 있고 실행할 수 있는** 방향이다(`GIL Artifact Model
v0.1` §11의 1번 상태).

`gil revisit`은 선언된 Closed ancestor로 Current와 Artifact 세계를 옮기고
`pending_cycle_revisit`을 남긴다. 이 명령만으로 새 Cycle을 만들지는 않는다. 이어지는
`gil open <interview|experiment>`가 새 Cycle을 열고 pending을 `revisit_from`으로 옮긴다.

둘 중 어느 단계에서도 대상 Cycle이나 실패 Cycle을 다시 열거나 수정하지 않는다.

---

## 10. 새 Cycle이 받는 것

새 Cycle은 부모의 세계에서 시작하지만 현재 Journey가 이미 배운 것을 가지고 시작한다.

최소 입력은 다음과 같다.

1. 현재 Chain의 승인된 방향
2. 구조적 부모 Cycle의 Cycle Report
3. 부모 이후 현재 Journey에서 닫힌 실패 Cycle Report들
4. 그 Cycle을 연 전이가 출발한 Artifact 세계 — 새 Cycle의 `entry_snapshot_ref`가 된다.
   `open_child`면 구조적 부모의 Exit이고, `revisit`이면 revisit 대상의 Exit이다
   (`GIL Artifact Model v0.1` §7.1)

실패 Cycle Report는 항상 전달한다. 같은 실패를 반복할 수 있기 때문이다.

실패 Cycle 내부의 모든 Step Report를 기본 Context에 넣지는 않는다. 압축된 Cycle Report를
전달하고, 세부 근거가 필요할 때 해당 Cycle 내부 Graph를 조회한다.

---

## 11. Cycle Close와 Cycle Report

내부 Outcome이 닫혀도 Cycle은 아직 Open이다.

```text
내부 Outcome 닫힘
        ↓
Cycle Report 작성
        ↓
Cycle Closed
        ↓
Cycle Exit
```

Cycle Report는 Cycle Close의 필수 조건이다. Cycle Exit은 Boundary이므로 자신의 Report를
갖지 않는다.

### Cycle Report의 최소 schema

```text
verdict
outcome_ref
handoff_summary
next_direction.action
next_direction.reason
next_direction.target_cycle_ref  (revisit일 때만, 예: cycle:C2)
```

#### verdict

Cycle Node 자신의 판정이다. 정확히 `success | failure` 중 하나다.

Cycle Report가 가리키는 Outcome도 Step Graph의 판정을 소유하지만, 두 verdict는 서로 다른
계층에 귀속된다. Cycle을 닫을 때 두 값이 같은지 검사한다.

```text
cycle_report.verdict == outcome_ref.verdict
```

두 값이 어긋나면 Cycle을 닫을 수 없다.

#### outcome_ref

내부 Step Graph의 마지막 Outcome Report를 가리킨다. verdict, lesson과 Step 수준의 방향을
근거로 연결한다. Outcome의 lesson과 Step 수준의 방향을 Cycle Report에 복제하지 않는다.
값은 `step:C2/S5` 형태의 StepRef다. `#5` 같은 화면 축약은 저장하지 않는다.

#### handoff_summary

다음 Cycle이 받아야 하는 현재 Cycle 내부 기록의 압축이다. 전역 Agent-side state인 Journey
전체를 요약하는 필드가 아니다.

다음을 중심으로 작성한다.

- 중요했던 실패
- 반증된 접근
- 중요한 전환점
- 반복하면 안 되는 시도
- 다음 Cycle이 반드시 알아야 할 미해결 위험

내부 Step Report 전체를 자동 투영하지 않는다.

[공통 보고서 작성 원칙](GIL%20Specification%20v0.1.md#짧고-간결한-report)에 따라 새 정보 중심으로
짧게 쓴다. 판정·근거가 다른 필드나 참조에 있으면 인수인계 문장에서 같은 내용을 반복하지 않는다.
실패 이유나 다음 작업에 필요한 위험은 생략하지 않는다.

### Cycle story의 선택적 투영

Cycle Report의 필드만 나열해서는 해당 Cycle에 참여하지 않은 협업자가 전체 실험을 이해하지
못할 수 있다. 그렇다고 Define과 Outcome의 내용을 `handoff_summary`에 복제하지 않는다.

`gil story`는 Cycle의 구조적 참조를 따라 인수인계에 필요한 최소 맥락을 선택적으로 투영한다.
Experiment Cycle의 닫힌 story 절에는 최소한 다음을 보여준다.

```text
실험한 것      ← 유일한 Define.problem
성공 기준      ← 유일한 Define.success_condition
결과           ← Cycle Report.verdict
성공·실패 이유 ← outcome_ref가 가리키는 Outcome.lesson
다음에 넘길 것 ← Cycle Report.handoff_summary
다음 방향      ← Cycle Report.next_direction
```

이 투영은 Step Report 전체를 펼치는 것이 아니다. Cycle을 식별하고 판정을 이해하는 데 필요한
시작점과 끝점만 읽는다.

```text
이 Cycle이 남긴 것  [experiment · 닫힘]
  실험한 것: ...
  성공 기준: ...
  결과: ...
  실패한 이유: ...
  다음 Cycle에 넘길 것: ...
  다음 방향: ...
```

Cycle Report schema에는 `problem`, `success_condition`, `lesson`을 다시 추가하지 않는다. 원본은
각각 immutable Define과 마지막 Outcome이며, story는 그 원본에서 읽는다.

### Story의 저장과 표현 분리

Report의 구조화된 원본과 Graph의 참조가 진실 공급원이다. plain text, Markdown과 HTML은 같은
read model을 서로 다른 방식으로 표현하는 renderer이며 별도의 판정을 소유하지 않는다.

```text
Graph + Reports + Artifact references
               ↓
           Read Model
          ┌────┼─────┐
          ↓    ↓     ↓
        text markdown html
```

v0 CLI의 기본 renderer는 plain text다. plain text도 제목, 빈 줄과 일관된 indentation으로 다음
블록을 시각적으로 구분해야 한다.

- 실험 정의
- 판정과 근거
- handoff
- 다음 방향

향후 Monitor는 같은 read model을 Markdown 또는 안전하게 제한된 HTML로 렌더링할 수 있다.
HTML 문자열을 Report의 진실 공급원으로 저장하거나, renderer마다 별도의 의미를 만들지 않는다.

### 시각 자료

모든 Node Report는 필요할 때 표, 차트, 이미지, 화면 캡처와 같은 시각 자료를 증거로 연결할 수
있어야 한다. 큰 바이너리나 임의의 HTML을 Report 본문에 직접 복제하지 않고 안정된 참조로
연결한다.

시각 자료 참조는 향후 최소한 다음 의미를 표현할 수 있어야 한다.

```text
kind
artifact_ref
caption
alt_text
provenance
```

- `artifact_ref`는 해당 자료가 속한 Artifact snapshot 또는 안정된 리소스를 가리킨다.
- `caption`은 이 자료가 어떤 주장이나 관측을 뒷받침하는지 설명한다.
- `alt_text`는 자료를 직접 볼 수 없는 독자에게 핵심 정보를 전달한다.
- `provenance`는 생성 또는 관측 출처를 추적한다.

정확한 schema, 저장 방식과 허용 media type은 시각 자료를 구현하기 전에 별도로 정한다. 첫
Monitor 구현은 이 결정을 기다리지 않고 구조적 사실과 신뢰하지 않는 텍스트만 표시한다. 그 전에도
Report의 추가 필드로 증거 참조를 기록할 수 있지만, renderer가 이해하지 못하는 참조를 임의로
실행하거나 삽입해서는 안 된다. HTML renderer의 기본 안전 경계는 `GIL Monitor Model v0.1`을
따른다.

#### next_direction

Cycle Graph에서 다음에 어디로 갈지 구조적으로 기록한다.

```text
next_direction.action:
  open_child | revisit | close_chain

next_direction.reason:
  비워 둘 수 없음

next_direction.target_cycle_ref:
  action이 revisit일 때 필수인 CycleRef
```

`next_direction.target_cycle_ref`는 `action`에 따른 **조건부 필드**다. 따라서 모든 Cycle
Report의 무조건적인 `close_requires`에는 넣지 않는다.

```text
action == revisit  → target_cycle_ref 필수
action != revisit  → target_cycle_ref 금지
```

값은 반드시 `cycle:C2` 형태의 typed `CycleRef`여야 한다. bare ID·화면 축약·StepRef·SnapshotRef는
받지 않는다. 대상을 기록할 때와 저장 상태를 복원할 때 다음을 모두 검사한다.

- 같은 Project의 Cycle Graph에 실재한다.
- Closed다.
- 현재 실패 Cycle 자신의 구조적 Lineage 위에 있으며 자기 자신이 아니다.
- 새 자식 Cycle을 가질 수 있는 유효한 분기점이다.
- Lineage는 구조적 `parent`만 따르며 `revisit_from`을 따라가지 않는다.

`target_cycle_ref`는 실패 Cycle을 닫는 순간에 방향의 근거와 함께 확정된다. 이후
`gil revisit`은 Cycle ID나 Snapshot ID를 인수로 받지 않고 이 참조를 읽는다.

Cycle Kind, verdict와 action은 다음처럼 맞물린다.

```text
Experiment success → open_child
Experiment failure → revisit
Interview success  → open_child | close_chain
```

`open_child`는 Cycle Graph 계층의 방향이다. 현재 Cycle이 이미 닫힌 뒤 그 Cycle을 부모로 다음
Cycle을 연다는 뜻이다. Step Outcome의 `close_cycle`과 반대되는 값이 아니다. 정상적인 성공
경로는 두 방향을 순서대로 사용한다.

```text
Outcome.next_direction.action = close_cycle
  → 현재 Cycle의 Step Graph를 끝냄

CycleReport.next_direction.action = open_child
  → 닫힌 Cycle을 부모로 다음 Cycle을 엶
```

`close_chain`은 성공한 Interview Cycle에서만 선택할 수 있다. 이때 해당 Interview가 참조하는
승인된 Synthesis에 `chain_verdict`가 있어야 한다. Experiment Cycle은 Chain을 직접 닫을 수 없다.

`open_child`는 다음 Cycle을 즉시 생성한다는 뜻이 아니다. 현재 성공 Cycle을 부모로 새 Cycle을
열 방향을 확정한다는 뜻이다.

---

## 12. Interview Cycle의 목적

인간은 자신이 원하는 것을 처음부터 정확한 문장으로 설명하지 못할 수 있다.

Interview Cycle의 기능은 모호한 생각을 잘게 나누어 문장으로 만들고, AI가 이해한 내용을
사용자에게 다시 제안하여 확인하는 것이다.

> **AI는 “이렇게 이해했다”고 제안하고, 사용자는 “그것이 내가 원한 것이다”라고 확정한다.**

Interview Cycle은 실험 계획을 고정하지 않는다.

- 하나의 Interview Cycle은 여러 Experiment 후보를 만들 수 있다.
- v0에서 후보들은 하나의 활성 경로를 따라 순차적으로 Cycle이 된다.
- 실패 후 revisit이 발생하면 대안 Cycle이 기존 실패 Cycle의 형제가 될 수 있다.
- 독립 후보들의 형제 실행과 성공 가지의 합류는 이후 버전에서 다룬다.
- 승인된 문제 분할은 변경 가능한 탐색 계획이다.
- 실제 Cycle DAG는 실험 결과에 따라 점진적으로 형성된다.
- 승인된 사용자 의도와 제약을 벗어나는 질문은 새로운 Interview Cycle에서 확인한다.

> **Interview는 고정된 DAG를 승인하는 것이 아니라, DAG가 자랄 수 있는 사용자 의도의 경계를
> 승인한다.**

---

## 13. Interview Cycle의 최소 Step Grammar

Interview Cycle v0의 내부 Step Kind는 네 가지다.

```text
question
interpretation
synthesis
outcome
```

최소 Grammar는 다음과 같다.

```text
Interview Entry
      ↓
Question
      ↓
Interpretation
   ┌──────┴──────┐
   ↓             ↓
Question      Synthesis
           │
      ┌────┴────┐
      ↓         ↓
 approved:no  approved:yes
      ↓         ↓
  Question   Outcome(success)
```

### 전이 의미

- `Entry → Question`: Interview는 사용자 질문으로 시작한다.
- `Question → Interpretation`: 응답을 받은 뒤에만 해석한다.
- `Interpretation → Question`: 중요한 모호성이 남았다.
- `Interpretation → Synthesis`: 사용자의 의도를 문장으로 제안할 준비가 됐다.
- `Synthesis(no) → Question`: 왜 아닌지 다시 묻는다.
- `Synthesis(yes) → Outcome(success)`: 인간 승인을 근거로 성공한다.

---

## 14. Question

Question은 질문 문장 하나가 아니라 질문과 인간 응답으로 완성되는 원자적 상호작용이다.

```text
Question OPEN
- AI가 질문과 보기를 제시
- 인간 응답 대기

Question CLOSED
- 질문과 응답이 함께 확정
```

질문만 있는 Question과 대답만 있는 Response는 존재하지 않는다. 별도의 Response Step Kind를
두지 않는다.

### Required Report

```text
question
choices
response
```

#### question

사용자에게 실제로 물은 문장이다. `prompt`라는 용어는 AI 실행 지시와 혼동될 수 있으므로
사용하지 않는다.

#### choices

가능한 한 사용자가 쉽게 답할 수 있는 객관식 보기를 제공한다.

모든 Question에는 다음 경로를 제공한다.

- `아직 잘 모르겠음 — 더 작은 질문 필요`
- `직접 입력`

보기는 사용자의 생각을 제한하는 정답 목록이 아니라 생각을 시작하기 위한 발판이다.
복수 선택 가능 여부를 명시한다. 적절한 보기를 만들 수 없다면 억지 객관식을 만들지 않는다.

#### response

선택 번호만 저장하지 않고 사용자의 원문 응답을 보존한다.

Question은 인간 응답이 오기 전에는 닫을 수 없다. AI의 해석은 Question Report에 섞지 않고
다음 Interpretation에 기록한다.

---

## 15. Interpretation

Interpretation은 직전 Question의 응답이 무엇을 의미하는지 해석한다.

### Required Report

```text
interpretation
unresolved
```

#### interpretation

사용자의 응답으로부터 정당하게 이해한 범위를 기록한다.

#### unresolved

사용자가 말하지 않아 아직 확정할 수 없는 중요한 범위를 기록한다. 중요한 unresolved가 남으면
다음 Question으로 묻는다. 충분히 해소되면 Synthesis로 갈 수 있다.

### Interpretation 불변식

> **Interpretation은 사용자의 응답을 더 큰 의도로 확장하지 않는다.**

응답을 정리하거나 좁힐 수는 있다. 그러나 응답 범위를 넘어선 의도를 사실로 추가하지 않는다.
새로운 의도가 필요하다면 반드시 다음 Question에서 확인한다.

---

## 16. Synthesis

Synthesis는 여러 Interpretation을 결합해 사용자의 의도를 명시적인 문장으로 제안하고,
같은 Node 수명 안에서 인간의 yes/no 승인을 받는다.

별도의 Approval Step Kind를 두지 않는다.

```text
Synthesis OPEN
- AI가 이해한 내용을 제시
- “이것이 맞는가?”라고 질문
- 인간의 yes/no 대기

Synthesis CLOSED
- statement와 approved가 함께 확정
```

### 간결하고 명징한 제안

**2026-11-17 사용자 결정.** 심층적으로 질문하되 승인 제안은 장황한 문서로 만들지 않는다.
사용자가 맞다/아니다 또는 수정할 부분을 빠르게 판단할 수 있는 짧은 명제로 표현한다.

- 첫 부분은 합의할 목표·이번 범위·중요 제약·확인이 필요한 선택을 짧게 보여 준다.
- 실행 후보는 “하나의 문제 → 기대 결과 → 완료 판단”이 보이는 짧은 목록이나 표로 제시한다.
  필요한 입력·선행 조건·첫 행동·검증 방법은 해당 항목 옆에 간결하게 설명한다.
- 한 명제에 독립적으로 반대할 수 있는 여러 결정을 숨기지 않는다. 중요한 미결 질문은 승인 문구 속에
  묻어 두지 말고 먼저 질문하며, 확인된 범위와 후속 후보를 구분한다.
- 인터뷰 대화 전문, 같은 배경 설명, 보고서와 계획의 중복 서술은 본문에 반복하지 않는다.
  상세 근거는 원본 참조로 연결하되 승인 범위·제약·완료 조건·위험·불확실성을 생략하지 않는다.
- 근거 없는 요구를 짧은 문장으로 바꾸었다고 합의된 요구가 되는 것은 아니다. 명제는 basis_refs로
  실제 인터뷰 근거에 연결한다. 간결화는 정확한 제안 판에 대한 승인과 원문 보존을 약화하지 않는다.

이는 제안 내용의 작성 원칙이다. 화면에서 긴 원문을 접는 것만으로 충족하지 않는다.
문자 수·항목 수 제한, 새 저장 필드나 명제별 별도 승인 도구를 여기서 도입하지 않는다.

### Required Report

```text
statement
basis_refs
approved
chain_verdict   (Chain 종료를 제안하는 Synthesis일 때만)
exploration_checklist   (Opening Interview의 Synthesis일 때)
```

#### statement

AI가 사용자에게 확인받으려는 명시적인 이해 또는 문제 분할이다.

사용자가 단순한 yes/no로 답하기 어렵다면 statement가 충분히 작고 명확하지 않은 것이다.

#### basis_refs

statement의 근거가 된 Interview Node들을 가리킨다. Synthesis는 근거 없는 새로운 목표나 제약을
추가할 수 없다. 먼저 Question으로 확인해야 한다.
각 값은 `step:C1/S1` 형태의 StepRef다. Report 문법에서는 block scalar 안에 한 줄에 하나씩
적는다.

```yaml
basis_refs: |
  step:C1/S1
  step:C1/S2
```

다음 불변식을 모두 만족해야 한다.

- 빈 줄을 제외한 각 줄은 canonical StepRef다.
- 참조 대상은 현재 Synthesis와 같은 Interview Cycle에 있다.
- 참조 대상은 Closed된 Question 또는 Interpretation이며 현재 Synthesis의 구조적 Lineage에
  속한다. ID 크기, 발급 순서나 닫힌 시각으로 선후를 추정하지 않는다.
- 같은 StepRef를 중복해서 적지 않는다.
- bare ID, `#1`과 쉼표로 이어 붙인 목록은 허용하지 않는다.
- 필드가 없거나 block scalar가 비어 있으면 모두 유효한 근거가 없는 같은 실패로 거절한다.

#### approved

정확히 다음 두 값 중 하나다.

```text
yes
no
```

`yes`만 명시적 승인이다.

- `yes` → Outcome(success)
- `no` → 왜 아닌지 묻는 Question

`no`는 Interview Cycle failure가 아니다.

#### chain_verdict

Synthesis가 Chain 종료를 제안할 때, 사용자가 승인하는 Chain 수준의 판정을 명시한다.

```text
success
failure
```

Closing Interview Cycle 자신의 verdict와 Chain verdict는 서로 다른 판정이다. 예를 들어
사용자가 Chain을 failure로 닫는 Synthesis를 승인했다면 Closing Interview Cycle은 인간 승인에
성공했으므로 `success`이고, Synthesis의 `chain_verdict`는 `failure`다.

#### exploration_checklist

Opening Interview의 Synthesis가 승인받으려는 원자화된 탐색 항목들의 순서 있는 목록이다.
각 항목은 아직 Cycle이 아니라 Cycle 후보다. v0에서는 이 순서를 따라 Interview 결과와 함께
다음 Cycle로 직렬 전파한다.

체크리스트는 고정 계약이 아니다. 실험 중 새 탐색 범위를 발견하면 이후 Interview에서 사용자에게
확인하여 남은 탐색 순서에 반영할 수 있다.

새 Interview Synthesis는 이전 Synthesis나 완료된 Cycle을 대체하지 않는다. 다음 두 상태를
구분한다.

```text
executed_cycles
  이미 실행된 Cycle과 그 Report. 불변이며 삭제하거나 덮어쓰지 않는다.

pending_exploration
  아직 Cycle로 열리지 않은 승인된 후보들의 현재 직렬 순서.
```

새 Synthesis는 승인된 변경을 `pending_exploration`에 누적한다. 항목을 추가하거나 순서를
조정하더라도 이미 실행된 항목과 그 결과는 `executed_cycles`에 그대로 남는다. 현재 남은
탐색 순서는 지금까지 승인된 Synthesis들을 순서대로 적용한 투영이다.

```text
Opening Synthesis: [A, B, C]
Experiment A:      completed
New Synthesis:     D를 다음 탐색으로 승인

executed_cycles:    [A]
pending_exploration:[D, B, C]
```

---

## 17. Interview Outcome

Interview Outcome은 기존 Outcome schema를 재사용한다.

```text
verdict
lesson
next_direction.action
next_direction.reason
next_direction.target_node_ref   (revisit일 때만인 StepRef)
```

Interview Outcome에는 다음 필드가 추가로 필요하다.

```text
synthesis_ref
```

규칙은 다음과 같다.

- Interview Outcome의 verdict는 항상 `success`다.
- `synthesis_ref`가 필수다.
- `synthesis_ref`는 같은 Interview Cycle에서 Closed되었고 현재 Outcome의 구조적 Lineage에
  속하는 Synthesis를 가리키는 StepRef다.
- 참조한 Synthesis의 `approved`는 `yes`여야 한다.
- 승인되지 않은 Synthesis를 근거로 success를 닫을 수 없다.

```text
success → close_cycle
```

응답이나 승인을 얻지 못한 Interview에는 Outcome을 만들지 않는다. 마지막 Question 또는
Synthesis와 Interview Cycle을 Open 상태로 둔다.

---

## 18. Chain의 성공과 실패

Chain의 큰 질문은 자연어 수준의 사용자 의도다. Experiment 결과만으로 Chain verdict를 자동
판정하지 않는다.

실험 Journey를 바탕으로 Interview Cycle을 열어 사용자에게 Chain의 큰 질문을 다시 제시한다.

Chain은 성공한 Closing Interview Cycle을 통해서만 닫을 수 있다. Closing Interview의
Synthesis가 승인되었다는 점에서 Interview verdict는 항상 `success`이며, 그 Synthesis가 승인한
Chain verdict는 `success | failure` 중 하나다.

```text
Opening Interview
        ↓
Experiment Cycles
        ↓
Closing Interview
        ↓
인간 승인
```

### 종료 인터뷰는 결과물을 먼저 보여 준다

**2026-11-15 결정, 에이전트 안내·실사용 검증 반영 대기.** 작업 결과물이 있는 경우 에이전트는
종료 인터뷰의 첫 사용자 메시지에서 결과물을 먼저 제시하고, 그 결과를 기준으로 종료 여부를
묻는다. 종료 질문만 띄워 사용자가 “무엇을 만들었는지 보여 달라”고 다시 요청하게 하지 않는다.
이는 성공 종료와 실패 종료 모두에 적용하며, 실패했더라도 실제 남은 부분 결과물이 있으면 보여 준다.

기본 순서는 **만든 것과 용도 → 실제 결과물·확인 경로 → 검증 결과와 남은 한계 → 종료 질문**이다.

| 결과물 | 먼저 제시할 내용 |
| --- | --- |
| 문서·보고서 | 실제 파일을 여는 링크와 주요 내용 또는 문서 미리보기 |
| 표·분석 결과 | 주요 수치·표·차트와 원본 결과 파일 링크 |
| 화면·디자인 | 실제 결과 화면이나 저장된 캡처, 필요하면 변경 전후 비교, 확인 가능한 실행·파일 링크 |
| 코드·프로그램 | 무엇이 달라졌는지, 주요 변경 내용이나 동작 예시, 관련 파일과 실제 시험 결과 링크 |
| 이미지·음성·영상 등 | Host가 지원하는 표시·재생 방식으로 실제 결과물을 제시하고 원본 확인 경로를 제공 |

- “완료했다”는 설명이나 파일 이름 나열만으로 제시를 대신하지 않는다. 사용자가 바로 판단할 수
  있는 대표 결과를 채팅 또는 접근 가능한 표시 화면에 보여 주고, 전체 결과도 열어볼 수 있게 한다.
  Monitor의 과거 노드를 사용자가 직접 찾아야만 결과를 볼 수 있는 흐름에 의존하지 않는다.
- 어느 승인 항목의 어떤 결과인지, 검증한 상태와 현재 파일이 같은지 확인 가능한 범위에서 설명한다.
  오래된 캡처·과거 실험 결과·부분 결과를 최신 완성본으로 제시하지 않는다. 현재 결과가 사라졌거나
  이후 바뀌었다면 저장된 증거와 구분하고 그 한계를 알린다.
- 결과 제시와 종료 질문은 한 흐름으로 묶는다. “보여 줄까요?”를 먼저 묻거나, 결과를 이미 제시한
  뒤 별도의 형식적인 “봤나요?” 승인을 추가하지 않는다. 실제 사용자의 종료 판단을 기다린다.
- 표시가 불가능하면 가능한 파일 링크·내용 발췌·저장된 증거를 제공하고 무엇을 확인할 수 없는지
  명시한다. 도구 호출 성공을 사용자 화면 표시·열람 성공으로 간주하지 않는다. 사용자가 확인할
  수 없다고 하면 접근 문제를 다루며, 무응답이나 표시 시도만으로 종료 승인을 만들어내지 않는다.
- 결과물이 없는 탐색은 없다고 설명하고 조사·판단 결과와 근거를 보여 준다. 부분 결과·실패·미검증
  상태를 구분하며 종료 화면을 채우려고 결과물이나 증거를 꾸며내지 않는다.

이 단계에서는 기존 결과와 증거를 읽고 제시한다. 결과 파일, 캡처, 시연 기록 등 새 파일 생성이나
재검증이 필요하면 앞선 허용된 실행·검증 단계에서 준비한다. 종료 인터뷰 중 파일을 수정하거나
실험을 실행하는 우회 경로를 만들지 않으며 단일 열린 Cycle 규칙을 지킨다.

공통 에이전트 지침과 종료 직전·인터뷰 진입 안내는 위 순서를 구체적인 다음 행동으로 유도해야 한다.
기존 결과와 증거를 먼저 찾아 제시하도록 안내하고, 결과 표시 자체를 Core가 자동 판정했다고
주장하지 않는다. 새 저장 필드·자동 종료·추가 승인 단계를 도입하지 않으며, 아래의 인간 승인과
Chain Report 계약은 그대로 적용한다. 복구 자동 승인 모드도 종료 승인을 대신하지 않는다.

### 2026-11-17 사용자 결정: 질문의 충족과 다음 Chain의 인터뷰

Chain 종료와 요구 충족은 구분한다. 모든 질문에 충분히 답하지 못해도 종료 인터뷰에서 한계와
판정을 정확히 승인받아 이번 탐색을 닫을 수 있다. 미충족 요구를 완료로 바꾸거나 실험 성공 수·
비율로 전체 성공을 계산하지 않는다. 기존 success/failure 판정과 인간 승인 경계를 유지한다.

각 승인 질문에 대한 현재 답, 근거 참조, 반대 근거·한계, 남은 질문을 간결하게 종합한다.
실패한 실험도 후보를 좁히는 근거가 될 수 있다. 새 증거 또는 반성에서 기존 근거가 반박되면
그 근거에 의존한 현재 해석도 재검토한다. 과거 실험·승인 기록 자체는 다시 쓰지 않는다.

**2026-11-18 사용자 결정 — 종합의 자리.** 위 종합은 종료 인터뷰에서 원래 요구의 답·근거·한계를
검토할 때 하는 일이다. 실험이 이어지는 동안 요구별 ‘현재 답’이나 잠정 정리를 별도 기록으로
작성·갱신하고 그것을 다음 실험이나 요구 완료 판단의 기준으로 쓰는 방식은 채택하지 않는다(이전
제안 철회). 그런 정리는 논리적 근거에 기반한 명제인지 다시 검증해야 하고, 추측이 후속 실험의
전제로 굳어질 위험이 있다. 다음 실험의 근거는 연결된 실험의 원본 Analysis·Outcome(해석·반성)과
그 관측·증거에서 마련한다. 각 실험의 Hypothesis·Analysis·Outcome이라는 원본 사고 기록과 관측·해석의
구분은 그대로이며, 종료와 판정은 사용자가 승인한다. 원본을 찾아 읽는 경로는
[승인 계획 계약의 「원본 근거 조회」](GIL_Approved_Plan_v0.1.md#원본-근거-조회-2026-11-18)를 따른다.

답이 부족한 채 닫을 때는 확인한 답·근거, 미해결 질문, 이번에 더 진행하기 어려운 이유,
부족함을 메울 실험·대체 자료 후보를 남긴다. 다음 Chain의 Opening Interview는 이 기록을
읽고 시작하여 추가 자료 확보·다른 방법·교차 검증 등 대안을 사용자와 합의한다. 이전 인터뷰를
빈 상태에서 반복하지 않으며, 이전 Chain의 제안이 새 Chain의 실행 승인을 대신하지 않는다.
새 Chain의 범위와 실행 방향은 새 인터뷰에서 확립한다. 새 Chain을 자동 생성·승인하지 않는다.

검증된 관측, 잠정 해석, 미해결 질문을 구분해 인계한다. 이전 결론을 무조건 확정 사실로
계승하지 않으며 새 증거로 수정할 수 있다. 인계는 현재의 판단과 필요한 근거 참조를 압축하고
전체 대화·보고서를 복제하지 않는다. 이 결정은 기존 Chain Report와 다음 인터뷰의 내용 요구이며,
새 저장 필드·자동 충족 판정·새 종료 상태를 추가하는 구현은 아니다.

### 체인 명세의 안정성과 새 설계 적용 경계

2026-11-17 후속 합의: 새 설계는 새 Chain부터 적용하며 기존 Chain의 중간 전환과 과거 기록
이전은 후순위다. 시작 인터뷰의 명세는 탐색의 기준으로 유지한다. 승인된 요구 안에서 Define을
작게 나누고 가설·실험 순서를 갱신하기 위해 같은 Chain의 명세를 반복 수정·재승인할 필요는 없다.
부족한 답과 새로운 탐색 방향은 종료·인계를 거쳐 다음 Chain의 시작 인터뷰에서 합의할 수 있다.
실제 요구·범위·권한·판단 기준의 변경은 단순 실험 조정과 구분하고 사용자 합의를 유지한다.
사용자 정정·취소 같은 예외 처리의 전이는 후속 설계이며, 현행 재인터뷰 도구를 즉시 삭제하거나
모든 재인터뷰를 금지하지 않는다. 구현·저장 호환은 아직 변경하지 않았다.

### 2026-11-17 사용자 결정: 반성의 계속·중단 근거와 종료 승인

반성은 과거 원인과 복원 기준점뿐 아니라 현재 탐색을 계속할 근거 또는 멈출 근거를 마련한다.
계속한다면 승인 범위·자료·권한 안에서 다음 작은 실험이 어떤 미해결 질문과 불확실성을 줄일지,
필요한 입력·선행 결과가 확보됐는지 설명한다. 멈춤을 제안한다면 충분한 답을 확보했는지,
자료·권한의 부족 또는 진전 없는 반복 등 무엇이 추가 탐색을 제한하는지 실제 근거로 설명한다.
실험 횟수·성공률만으로 자동 종료하지 않으며, 기다림과 종료도 구분한다. 새로운 관측 없이
같은 시도를 반복하거나 Chain 이름만 바꾸는 것을 새 탐색 근거로 삼지 않는다.

반성의 판단은 종료 제안의 근거이지 종료 승인이 아니다. 종료 인터뷰에서 실제 결과물,
질문별 답·근거·남은 한계와 멈추려는 이유를 제시하고, 사용자가 정확한 종료 제안과 Chain 판정을
승인해야 닫는다. 사용자 응답 대기나 무응답을 승인·실패·자동 종료로 처리하지 않는다.
계속·재인터뷰·후속 Chain 대안도 필요에 따라 제시하며 새 범위의 실행은 별도로 합의한다.

이 결정은 반성의 역할과 인간 승인 경계를 확정한다. 새 Reflection Step 종류·저장 필드,
실패 후 새 전이·자동 종료를 구현하거나 기존 전이 검사를 우회하는 승인은 아니다.
기존 Analysis/Outcome·Cycle 방향과 연결하는 구체적 표현은 후속 구현 설계 대상이다.

### Chain success

Closing Interview가 실험 결과를 큰 질문에 연결한 Synthesis를 제시하고, 사용자가 Chain을
success로 닫는 문장에 `yes`로 승인해야 한다. 이 Synthesis의 `chain_verdict`는 `success`다.

### Chain failure

현재 탐색 경로의 Experiment들이 계속 실패해 더 할 일이 없어 보이더라도 자동으로 Chain을
닫지 않는다.

Closing Interview는 다음을 사용자에게 설명한다.

- 어떤 Cycle 경로를 탐색했는가
- 어떤 가설들이 실패했는가
- 어디서 방향을 전환했는가
- 현재 남아 있는 새 아이디어가 있는가

그 뒤 사용자에게 다음 방향을 묻는다.

```text
1. 현재 Chain에서 새 아이디어를 더 실험한다
2. 문제를 다시 인터뷰하고 현재 Chain을 계속한다
3. 별도의 새 Chain을 Opening Interview로 시작한다
4. 현재 결론으로 이 Chain을 닫는다
5. 아직 잘 모르겠음
6. 직접 입력
```

선택은 Question에서 얻는다. AI는 선택을 작은 Synthesis로 명확히 만들고 최종 yes/no 승인을
받는다. 사용자가 현재 Chain을 failure로 닫는 문장에 `yes`로 승인해야 Chain failure가 된다.
이 Synthesis의 `chain_verdict`는 `failure`다.

---

## 19. Chain Report

Chain을 닫으려면 Chain Report가 필요하다.

최소 schema는 다음과 같다.

```text
verdict
closing_synthesis_ref
handoff_summary
```

### verdict

Chain Node 자신의 판정이다. 정확히 `success | failure` 중 하나다.

Closing Interview Cycle의 verdict를 그대로 사용하지 않는다. Closing Interview Cycle의
`success`는 인간 승인을 얻는 인터뷰가 성공했다는 뜻이며, 사용자가 승인한 Chain verdict는
`success`일 수도 `failure`일 수도 있다.

Chain을 닫을 때 다음 값이 같은지 검사한다.

```text
chain_report.verdict
==
closing_synthesis_ref가 가리키는 synthesis.chain_verdict
```

두 값이 어긋나면 Chain을 닫을 수 없다.

### closing_synthesis_ref

Chain success 또는 failure를 인간이 승인한 정확한 Synthesis를 직접 가리킨다. 중간의 Interview
Cycle이나 Outcome을 Chain Report에서 다시 참조하지 않는다.
값은 해당 Synthesis를 가리키는 StepRef다.

참조 대상은 다음 조건을 모두 만족해야 한다.

- 현재 Chain의 성공한 Closing Interview Cycle 안에 존재한다.
- `approved`가 `yes`다.
- `chain_verdict`가 존재한다.
- `chain_verdict`가 Chain Report의 `verdict`와 같다.

### handoff_summary

다음 Chain이 받아야 하는 중요한 Cycle 경로, 실패, 전환점과 남은 의문을 압축한다.
질문별 현재 답·근거 참조·미해결 한계와 이번 탐색을 마치는 이유, 후속 실험·대체 자료 후보를
포함한다. 관측과 잠정 해석을 구분하고 다음 Opening Interview에서 결정할 사항을 명시한다.
후속 후보를 이미 승인된 실행 계획처럼 표현하지 않는다.
(2026-11-18: 이 칸은 Chain을 닫을 때 원본 기록을 근거로 한 번 적는 인계 요약이다. 실험 도중에
유지하는 요구별 ‘현재 답’ 기록이 아니며, 근거는 원본 Report의 참조로 남긴다.)

전역 Agent-side state인 Journey 전체를 요약하는 필드가 아니다. 현재 Chain 내부에서 다음
Chain으로 넘겨야 할 내용만 선별한다.

전체 Cycle Report를 자동 투영하지 않는다. 세부 기록은 Graph에 남아 있으며 필요할 때 조회한다.

**2026-11-18 안내 연결(계약 변경 없음).** 승인된 종료 Synthesis를 닫은 뒤의 영수증과 다음 호출 안내는 그
Synthesis의 `chain_verdict`를 기록에서 읽어 Outcome → `close_chain` Cycle Report → 승인된 판정의 Chain Report
순서를 가리킨다. Closing Interview 자신의 `success`가 Chain 판정이 아니라는 점과 Experiment를 열지 않는다는
점을 같은 자리에서 말하고, Chain Report 자리의 안내는 위 `handoff_summary`에 담을 것을 적는다. 수정 요청을
받은 종료 제안은 승인이 아니며 Chain은 열린 채로 남는다고 안내한다. 새 필드·상태·전이·검사는 없고 위의
판정 일치 검사가 그대로 진실원이다. 종료 뒤 다음 Chain을 자동으로 열지 않는다.

---

## 20. 진행 방식과 사람의 확인 지점

**2026-11-15 제품 방향 확정, 상세 계약·구현 대기.** 기본 진행 방식은 **주요 지점 확인**이다.
시작 Interview에서 작업에 맞는 방식을 제안하고, 어느 결과에서 멈춰 확인받을지 승인 계획에
명시한다. 진행 방식의 기본값 자체는 계획 승인이나 작업 실행 승인이 아니다.

| 방식 | 사람의 참여 | 예 |
| --- | --- | --- |
| 단계별 확인 (`stepwise`) | 계획에 정한 작은 결과마다 확인 | 화면 수정 결과를 하나씩 검토 |
| 주요 지점 확인 (`milestone`, 기본) | 중요한 중간 결과·방향 전환에서 확인 | 분석의 중간 결론 검토 |
| 승인 범위 내 자율 진행 (`autonomous`) | 정해진 범위에서 진행하고 결과 확인 | 승인된 조건 안에서 반복 시험 |

- 확인 지점에는 확인할 결과·완료 조건·사람의 결정 이후 가능한 다음 행동을 적는다.
  “단계별”을 모든 도구 호출마다 승인받는 것으로 해석하지 않는다.
- 확인 지점에서는 실제 결과와 남은 한계를 먼저 보여 준다. 필요한 응답을 기다리는 동안
  다음 확인 대상 작업을 임의로 진행하거나, 무응답을 승인으로 처리하지 않는다.
- 작업 중 사용자 요청으로 방식을 변경할 수 있다. 변경 제안·확정과 적용 시점을 기록하고,
  이미 수행한 작업의 승인 이력을 바꾸거나 새 방식으로 과거 누락 승인을 대신하지 않는다.
  열린 Cycle이 있을 때는 기존 재인터뷰 요청·종료 경계를 지키며 강제 종료·동시 개설하지 않는다.
- 자율 진행에서도 승인된 목표·범위를 벗어나야 하면 계획 변경을 확인받는다. 에이전트가
  편의를 위해 확인 빈도를 낮추거나 기존 승인 범위를 확대하지 않는다.
- 실패 실험 복구의 수동/자동 승인 설정은 별도다. 자율 진행 선택만으로 자동 복구가 켜지지
  않으며, 복구 자동 승인이 작업 계획·진행 방식 변경의 승인을 대신하지 않는다.
- 어떤 방식에서도 Chain의 최종 success / failure는 인간이 종료 Interview에서 승인한다.

확인 지점의 저장 형식·승인 증거·전환 명령·재시작 후 대기 복원은 후속 설계다. 현재 Cycle
Grammar에 새 전이·승인 값을 추가하지 않으며, 기존 프로젝트에 모드를 임의로 부여하지 않는다.

---

### 20.1 응답 대기와 재개

**2026-11-15 방향 확정, 후속 안내·상세 재개 계약과 구현 대기.** 사람의 응답을 기다리는
상태는 검증 불가능으로 중단한 작업과 구분한다. 시간이 지났거나 앱·대화 연결이 끊겼다는
이유만으로 승인·실패·종료를 만들거나 기록을 정리하지 않는다.

- 대기 중에는 응답 대상인 질문·제안, 관련 결과와 근거, 현재 위치를 보존한다. 저장된
  승인과 아직 응답이 없는 상태, 승인 저장 결과가 불확실한 상태를 혼동하지 않는다.
- 복귀 시 현재 기록을 다시 읽고 마지막 결과·기다리는 결정·가능한 다음 행동을 짧게 보여 준다.
  결과물이 있으면 확인 가능한 내용이나 링크를 제공하며, 전체 대화 재독이나 같은 제안의
  반복 승인을 기본으로 요구하지 않는다.
- 파일·계획·승인 판이 바뀌었는지 확인하고 이전 제안이 현재에도 유효한지 판단한다.
  조회·관측 실패는 “변경 없음”이 아니다. 불확실성을 표시하고 종전 화면으로 실행하지 않는다.
- 유효한 미응답 제안은 그대로 이어받는다. 변경으로 제안이 유효하지 않으면 바뀐 점과
  새 제안을 보여 주고 해당 승인 계약을 따른다. 이미 저장된 승인은 시간 경과만으로 지우지
  않으며, 대상·판·조건 변경에 따른 효력은 각 기능의 승인 계약으로 판단한다.
- 사용자는 계속 진행·계획 변경·중단을 선택할 수 있다. 실제 실행은 현재 상태의 허용 경로를
  따르며, 중단 요청은 이 문서의 기록·부분 결과 보존 요구로 연결한다. 열린 작업의 강제 종료,
  새 작업 동시 개설, 아직 구현되지 않은 중단 명령으로 우회하지 않는다.
- 재촉 알림보다 정확하고 간결한 재개를 우선한다. 이 결정은 반복 알림·예약 실행·자동 취소
  기능을 도입하지 않으며, 무응답을 자동 승인 설정으로 해석하지 않는다.

후속 검증은 장기 미응답, 연결 끊김, 응답 저장 직후 연결 끊김, 대기 중 파일·계획 변경,
조회 실패와 재시작을 포함한다. 표시·재조회·변경 감지의 상세 계약은 별도 설계하며,
이번 문서 변경이 이미 실행 가능한 재개·중단 기능을 추가한 것으로 해석하지 않는다.

## 21. 현재 결정하지 않는 것

이 문서는 다음을 의도적으로 결정하지 않는다.

- Interview 질문 선택지를 생성하는 알고리즘
- 사용자의 자연어 응답을 구조화하는 구현 방식
- 실험 후보 계획의 저장 schema
- 탐색 체크리스트 항목의 식별자·상태·의존 관계 schema
- Chain Report 안의 Project 수준 `next_direction` 필드 (추가하지 않음; 직렬 다음 Chain은 아래 명시적 명령으로 처리)
- Chain-level revisit의 구체적인 명령과 상태 머신
- 다중 부모 Cycle과 Artifact Merge
- 독립 체크리스트 항목의 형제 실행과 합류 규칙
- 동시에 진행되는 Cycle들의 Merge
- Artifact Snapshot 저장 엔진
- `.gilignore`의 형식, 기본 제외 목록과 기존 ignore 파일 호환 정책
- §20 진행 방식의 전환 명령·저장 형식·확인 지점 승인 증거와 적용 시점의 상세 계약
- §20.1 응답 대기 보존·재개 표시·변경 감지의 상세 구현과 장기간 열린 작업의 명시적 정리 절차

다중 부모 Merge는 서로 다른 성공 Artifact 가지를 실제로 결합해야 하는 사례가 나타날 때
별도로 설계한다. 성공한 Experiment의 Artifact를 유지한 채 Interview를 여는 문제는 단일
부모-자식 연결로 해결하며 Merge를 요구하지 않는다.

2026-11-01 M6-C 보완: 직렬 다음 Chain은 Closed Chain에서 명시적 `gil open chain` /
`gil_open(kind: "chain")`으로 연결한다. Chain Report를 바꾸거나 자동 실행하지 않는다. 새 Chain의
Opening Interview는 부모 Exit에서 시작하고, 이전 Chain Report를 Context로 받는다. Cycle ID는
Project 전체에서 유일하며 기존 Step 주소·세계·Journey는 보존한다. 세부 불변식과 호환 조건은
[Chain storage의 M6-C 계약](GIL_Chain_Storage_v0.1.md#직렬-다음-chain과-인수인계--m6-c)을 따른다.
Chain revisit·병렬·다중 부모는 이 보완에 포함하지 않는다.

### 단계별 구현 범위

구조 전체를 한 번에 구현하지 않는다.

```text
v0
  한 번에 하나의 활성 Cycle 경로
  실행된 Cycle과 Journey 보존
  승인된 남은 탐색 순서의 직렬 전파
  실패 Cycle에서 Closed ancestor로 revisit
  형제 Cycle을 통한 분기 표현
  대상 ancestor Exit의 Artifact 상태 복원

later
  독립적으로 성공한 형제 Cycle들의 합류
  다중 부모 Report 전달과 Artifact Merge
```

v0의 형제 Cycle은 실패 후 revisit으로 생기는 대안 가지다. 동시에 여러 성공 가지를 활성화하거나
합치는 기능은 아니다. 이 단계 구분은 이후 구조의 가능성을 부정하지 않으면서, Artifact Merge를
요구하지 않는 분기부터 먼저 구현하기 위한 순서다.

---

## 22. 핵심 문장

> **Chain은 Interview로 시작하고 Interview로 판정된다.**

> **Interview Cycle은 성공으로만 닫힌다. Chain의 failure도 성공한 Closing Interview에서
> 인간이 승인한다.**

> **Interview는 실험 계획을 구속하지 않고, 실험 DAG가 자랄 수 있는 사용자 의도의 경계를
> 승인한다.**

> **하나의 Experiment Cycle에는 하나의 고정된 Define이 있고, 새로운 Define은 새로운
> Cycle을 시작한다.**

> **새 Cycle은 부모의 세계에서 시작하지만, 현재 Journey가 이미 배운 실패를 모두 가지고
> 시작한다.**

> **Question은 질문과 응답의 쌍이고, Synthesis는 명제와 인간 승인의 쌍이다.**

> **Interpretation은 사용자의 응답보다 더 큰 의도를 확정하지 않는다.**

> **failure는 끝이 아니라, 재귀적 Graph에서 다른 가지를 만들기 위한 논리적 발판이다.**

## M6-D 개발 계약 연결 (2026-11-01)

§16·§21의 승인 후보 목록, revision 및 실행 연결은 선택적 `approved-plan` 프로필에서
[승인 계획 계약](GIL_Approved_Plan_v0.1.md)으로 구체화한다. 기존 기본 전이를 자동 변경하지 않는다.
Monitor 버튼은 정확한 제안 판에 대한 응답을 기록하며 Synthesis/Outcome/Interview를 대신 닫지 않는다.
열린 Experiment의 재인터뷰 요청은 보존하되 현재 Cycle을 사실대로 닫기 전 새 Cycle을 만들지 않는다.
이 개발 프로필의 Cycle 재시도 바인딩과 취소는 후속 계약이며, 기존 Core의 반복 revisit 구현과 구분한다.
