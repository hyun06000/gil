# GIL Will Model v0.1

> **2026-11-19 후속 합의:** 실행 약속의 ‘진행 중’, ‘달성 완료’, ‘미달성 종료’를 구분한다.
> 미달성 종료는 아직 미구현이며, 분리 목록과 단일 종료 이력 중 저장 방식은 결정 대기다.
> 아래 `active_will`·`done_wills`의 두 자리 설명과 저장 예시는 현재 구현이다.
> 새 상태를 기존 Done으로 간주하거나 저장 방식을 이미 확정한 것으로 읽지 않는다.

## 1. 목적

Will은 현재 선택된 Existence가 **지금부터 무엇을 하려 하는지** 기록하는 Journey-side 상태다.

`gil context`가 현재 세계와 상태를 정확히 전달해도, 열린 Node에서 실제로 무엇을 먼저 해야
하는지 전달하지 않으면 새 Agent는 다음 GIL 명령을 즉시 실행할 수 있다. 실제 M2 인수인계
dogfood에서 열린 Verify를 받은 Agent가 검증을 수행하기 전에 `gil close`를 실행한 사례가 이를
드러냈다.

Will은 이 빈자리를 맡는다. GIL이 문제 해결 계획을 대신 생성하는 기능이 아니다. 런타임 모델이나
세션이 아니라 `GIL Existence Model v0.1`의 지속적 Existence에 속한다.

`Will`이라는 이름은 이 문서가 정의하는 **현재 행동 단위에만** 쓴다. 미래 조건에서 기억해야 할
지속적 규약은 Prospective Memory이며 `GIL Specification v0.1` §3.2가 정의한다.

---

## 2. 세 책임의 분리

```text
Context
  지금 어떤 세계와 상태에 있는가

Will
  이 상태에서 Current Existence가 무엇을 하려 하는가

Grammar
  현재 또는 작업 완료 뒤 어떤 구조적 전이가 허용되는가
```

이 셋을 행동 시점에 함께 읽은 것이 Action Context다.

```text
Action Context = Context + Current Will + Applicable Grammar
```

`next_direction`은 닫힌 Outcome 또는 Cycle 뒤 Graph에서 향할 구조적 방향이고, Will이 아니다.
`next_moves`는 GIL 문법상 가능한 명령이고, Will이 아니다.

**Prospective Memory도 Will이 아니다.** 미래 조건에 걸어 두는 지속적 규약은 Memory에 속하며
`done`으로 소비되지 않는다(`GIL Specification v0.1` §3.2). 그 조건이 충족되면 새로운 Current
Will을 만드는 근거가 될 수 있지만, 근거와 행동 단위는 서로 다른 것이다. 조건 판정과 Will
생성의 trigger는 아직 정하지 않았다.

```text
Prospective Memory   릴리즈 직전에는 전체 테스트를 실행한다.   지속 · done 없음
Current Will         전체 테스트를 실행해 릴리즈 가능 여부를    하나 · done 으로 확정
                     검증한다.
```

---

## 3. Will의 소유권

Will은 특정 Node의 고정된 구조 상태나 런타임 Agent가 아니라 선택된 Existence의 Journey에
속한다.

- Artifact 복원으로 되돌리지 않는다(`GIL Artifact Model v0.1` §9).
- Step 또는 Cycle revisit으로 과거 Will을 다시 활성화하지 않는다.
- Current Node는 Will이 생긴 구조적 맥락을 참조할 수 있지만 Will을 소유하지 않는다.
- 과거의 세계로 돌아가는 Existence도 자신의 최신 Current Will을 사용한다.
- 모델 또는 세션이 바뀌어도 같은 Existence를 선택하면 같은 Current Will을 복원한다.
- Existence를 명시적으로 전환하면 새 Existence 자신의 Current Will을 사용한다.

---

## 4. 최소 형태

v0의 Will은 다음 의미를 가진다.

```text
Will
  id
  existence_ref
  target_node_ref
  objective
  next_action
  done_when
```

### objective

현재 행동이 무엇을 이루려는지 자립적으로 기술한다.

### next_action

Agent가 지금 실제 세계에서 가장 먼저 수행할 행동을 기술한다. GIL 명령만 적어서는 안 된다.

### done_when

Will을 완료했다고 확정할 관측 가능한 조건을 기술한다.

### identity와 target

`existence_ref`는 Will을 가진 지속적 Existence를, `target_node_ref`는 이 Will과 함께 열린
실행형 Node를 가리킨다. 원자적 Open으로 둘의 관계가 고정되므로 별도의
`originating_node_ref`를 두지 않는다.

두 필드는 `GIL Node Model v0.1`의 프로젝트 로컬 typed reference를 사용한다. 예를 들어
`existence:X1`과 `step:C2/S3`이다. 현재 화면에서 보이는 `#3` 같은 축약값은
`target_node_ref`로 저장하지 않는다. Will ID도 같은 규칙에 따라 `will:W4`로 참조한다.

현재 구현은 Will의 status를 필드로 중복 저장하지 않는다. `active_will` register에 있으면 Active,
`done_wills` append-only 목록에 있으면 Done이다. 후속 합의의 미달성 종료를 어디에 표현할지는
아래 저장 대안 비교에서 결정하며, 현재의 두 자리 제약을 새 계약에 그대로 강제하지 않는다.

### 저장 위치와 ID 발급

Active Will은 해당 Existence의 Journey 안에 **inline 객체**로 저장한다. Done이 되기 전에는
별도의 object store나 reference만 남기는 간접 계층을 만들지 않는다.

Done 시 같은 객체를 `active_will`에서 꺼내 그 Journey의 `done_wills` append-only 목록 끝으로
옮긴다. Done 목록은 ref 목록이 아니라 immutable Will 객체 목록이다. Journey revision의
`will_head_ref`만 마지막 Done 객체를 `will:W4`처럼 가리킨다.

Will ID는 Existence별이 아니라 프로젝트 전체에서 유일하다. Project root의
`next_will_id` high-water mark가 발급하며, Existence가 달라도 같은 ID를 재사용하지 않는다.

```yaml
next_will_id: 2

existences:
  X1:
    journey:
      active_will:
        id: W1
        existence_ref: existence:X1
        target_node_ref: step:C1/S1
        objective: 사용자의 프로젝트 목표를 확인한다
        next_action: 목표의 종류를 선택지와 함께 질문한다
        done_when: 사용자의 원문 응답을 얻는다
      done_wills: []
```

객체 자신의 `id`는 bare `W1`, 다른 객체가 가리키는 값은 typed `will:W1`이다.

---

## 5. 하나의 Current Will

각 Existence의 Journey에는 Current Will이 최대 하나만 존재한다.

```text
active_will_count <= 1
```

여러 Will을 동시에 현재 행동으로 두지 않는다. 무엇을 먼저 해야 하는지 다시 모호해지기 때문이다.

### 가장 깊은 실행형 Node

Existence ownership은 Open Chain·Cycle·Step 모두에 적용되지만 Current Will은 각 계층마다
하나씩 생기지 않는다. 현재 실제 행동을 수행하는 **가장 깊은 실행형 Open Node** 하나에만
대응한다.

```text
Chain Open                 ownership: X1
└─ Cycle Open              ownership: X1
   └─ Verify Step Open     ownership: X1 · Current Will target
```

Chain과 Cycle처럼 내부 Graph를 담는 컨테이너 Node는 열린 동안 장기 Active Will을 점유하지
않는다. 내부 Graph가 없는 Step, 또는 미래에 실제 행동을 직접 수행하는 다른 leaf Node가
Current Will의 target이 된다.

```text
current_will.target_node_ref = deepest_open_action_node_ref
```

현재 실행형 Open Node가 없다면 Active Will이 없을 수 있다. GIL은 컨테이너가 열려 있다는
이유만으로 가상의 Will을 만들지 않는다.

---

## 6. 덮어쓰기와 확정

Active Will을 수행 가능하게 구체화하는 동안에는 같은 Current Will을 덮어쓴다.

- 표현 보정과 행동 구체화는 새 Journey 사건을 만들지 않는다.
- 덮어쓰기 전 초안을 시간선에 누적하지 않는다.
- 덮어쓴 Will은 계속 `active_will` register에 있다.

행동이 `done_when`을 충족하면 Will을 `done`으로 확정한다. 미달성 상태에서 시도를 실패로
끝내는 후속 계약은 아래 「실행 약속을 달성하지 못한 시도의 종료」를 따른다. 미달성을 달성으로 간주하지 않는다.

- Done Will은 immutable하다.
- Done Will은 Journey Timeline에 시간순으로 남는다.
- 다음 행동은 새로운 Active Will로 시작한다.
- Done이 아닌 Will을 완료된 것처럼 시간선에 남기지 않는다.

```text
W1 active
→ W1 active 내용 보정
→ W1 done
→ W2 active
→ W2 done
→ W3 active
```

Will의 `done`은 행동을 수행했다는 뜻이다. 가설이나 Cycle이 성공했다는 뜻이 아니다.

```text
Will done
  비HTTP 입력 테스트를 실행했다.

Verify observation
  테스트 두 개가 실패했다.

Hypothesis verdict
  failure
```

Will에는 success/failure verdict를 두지 않는다.

---

## 7. Node와의 생명주기

실행형 Node Open은 그 Node에서 수행할 Will을 확정하는 자연스러운 경계다. 사용자에게 보이는
정상 lifecycle 문법은 `gil open`과 `gil close` 둘이다.

```text
Action Node Open
→ 같은 transaction에서 Active Will 확정
→ 실제 작업 수행
→ done_when 충족
→ Report와 함께 Node Close 요청
→ 같은 transaction에서 Will Done + Journey revision + Node Close
```

v0 구현에서 Node Kind만 보고 구체적인 Will을 임의 생성해서는 안 된다. Agent가 명시적으로
작성하거나, 승인된 기존 Report에서 결정적으로 투영할 수 있어야 한다.

실행형 Node의 `gil close`는 그 Node를 target으로 한 Active Will을 함께 Done으로 확정한다.
Chain과 Cycle 같은 컨테이너 Node의 Close는 Will Done을 만들지 않는다. 대신 내부 Open Node가
없고 해당 계층의 Report와 다른 Close 조건을 충족해야 한다.

Node Open 시 고정된 `existence_ref`의 Will만 이 lifecycle에 참여한다. Open Node가 있는 동안
Existence 전환을 금지하므로 다른 Existence의 Will로 같은 Node의 Close gate를 통과할 수 없다.

### 원자적 Open

실행형 Node와 그 Node를 target으로 한 Active Will은 하나의 format 3 save로 함께 생성한다.

Agent는 같은 `gil open`의 stdin으로 Action Contract를 명시한다.

```bash
gil open <<'EOF'
objective: 사용자의 프로젝트 목표를 확인한다
next_action: 목표의 종류를 선택지와 함께 질문한다
done_when: 사용자의 원문 응답을 얻는다
EOF
```

현재 자리에서 Kind 선택이 필요할 때만 `gil open question`처럼 Kind를 함께 적는다. 세 필드는
모두 비울 수 없다. stdin이 없거나 필드가 빠지거나 저장이 실패하면 Node, Will과
`next_will_id` 중 어느 것도 바뀌지 않는다.

```text
Action Node
  status: open
  existence_ref: existence:X1

X1 Journey
  active Will
    target_node_ref: step:C2/S3
    objective
    next_action
    done_when
```

Node만 있거나 Will만 있는 중간 상태를 허용하지 않는다. Will 입력이나 저장이 실패하면 Node도
열리지 않는다.

### Active 단계

Active Will이 있는 동안 실제 작업을 수행한다. Active Will의 표현 보정과 행동 구체화는
domain상 같은 register의 덮어쓰기지만, v0의 기본 traversal CLI에 별도 Will 수정 명령을 두지
않는다. 실제 필요가 확인되면 host/UI 또는 보조 명령으로 추가한다.

`done_when`은 기대한 성공이 아니라 행동 완료를 판정할 수 있게 작성한다. 관측 결과가 실패여도
의도한 실행과 관측을 마쳤다면 Close할 수 있고, success/failure 해석은 Report와 Outcome에
남긴다.

### 실행 약속을 달성하지 못한 시도의 종료

**2026-11-19 사용자 결정: 방향 확정, 상세 저장·종료 구현 대기.** 원래 실행 약속과
`done_when`은 보존하고, 실제 수행 범위·관측 근거·미수행 이유를 원본 보고서에 기록한 뒤
시도를 실패로 종료할 수 있어야 한다. 완료 조건을 뒤늦게 낮추거나 약속을 중단 보고 작성으로
바꾸어 달성을 선언하지 않는다. 보고서를 작성했다는 사실은 원래 행동을 달성했다는 증거가 아니다.

실험의 실패 종료와 명시적 전체 복원은 [Cycle Model](GIL_Cycle_Model_v0.1.md#완료할-수-없는-작업의-중단과-다음-방향)을 따른다.
행동 미달성과 질문의 반증을 혼동하지 않으며, 새 판정값·검증 미완료 구분 필드·잠정 정리 저장소는 만들지 않는다.
안전한 기록 보존·종료가 불가능하면 열린 단계와 현재 상태를 유지한다. 단순 연결 중단·응답 대기는 실패 종료 대상이 아니다.

**후속 결정(2026-11-19): 실행 약속에 ‘미달성 종료’를 별도로 표현한다.**

| 실행 약속의 상태 | 의미 |
| --- | --- |
| 진행 중 | 약속한 행동을 수행 중이며 아직 종료하지 않음 |
| 달성 완료 | 약속한 행동과 완료 조건을 충족함. 실험 가설의 성공을 뜻하지 않음 |
| 미달성 종료 | 약속한 행동을 마치지 못한 채 시도를 끝냄. 이유·근거는 원본 보고서에 기록 |

기존 Done의 달성 의미는 유지한다. 미달성 종료는 가설의 새 판정값이나 검증 미완료 사유 분류가
아니라 실행 약속의 생명주기 구분이다. 별도 사유 필드·잠정 정리 저장소는 만들지 않는다.
끝난 약속의 원문·완료 조건·대상 단계 연결은 두 종료 경우 모두 보존하며 사후에 고치지 않는다.
미달성 종료 기록은 달성 완료로 표시하지 않고, 실패 실험의 분석·반성·복원으로 연결한다.

현재 통합 Close는 Active Will을 무조건 Done 목록에 넣으므로 위 계약은 **구현 대기**다.
보조 약속 수정 명령으로 이 결정을 대신하지 않는다. 새 저장 필드명·상태 코드·형식 번호·이전 명령은
아직 결정하지 않았다. 구현 시 원래 약속·보고서 연결, 원자적 종료, 재시작 후 조회,
중복 종료·저장 실패 시 상태 보존을 검증한다.

#### 저장 대안 — 미승인, 구현 전 결정

| 대안 | 장점 | 비용·주의점 |
| --- | --- | --- |
| 기존 달성 완료 목록과 미달성 종료 목록을 분리 | 기존 Done 목록의 의미·내용을 유지하며, 소속 목록으로 두 결과를 구별할 수 있음 | 전체 종료 순서·최신 종료·과거 판 참조를 두 목록에 걸쳐 검증해야 함. 현재 Done만 찾는 조회·참조 검사를 확장해야 하므로 단순 목록 추가로 끝나지 않음 |
| 하나의 종료 이력에 달성/미달성 구분을 함께 저장 | 두 종류의 시도를 같은 시간선에서 조회·역추적하고 원자적 종료 규칙을 한 경로로 관리하기 쉬움 | 기존 Done 목록에서 범용 종료 이력으로의 읽기 호환·변환 계약과 명시적 결과 구분이 필요함. 달성만 필요한 조회는 결과를 걸러야 하며 기존 기록을 검증 없이 재해석하면 안 됨 |

두 방식 모두 진행 중 약속은 하나만 유지한다. 종료 시 진행 중 자리 비우기·종료 기록·보고서·
이력 참조 갱신을 함께 확정해야 한다. 분리 방식을 택해도 전체 시간선은 화면에서 합쳐 보여 줄 수
있으며, 단일 이력을 택해도 달성 완료만 조회할 수 있다. 화면 모양이 저장 방식을 결정하지 않는다.

기존 기록은 원문·식별자·기존 완료 의미를 보존한다. 과거 Done을 본문 추측으로 미달성으로
재분류하지 않는다. 현재 구형 reader는 알 수 없는 저장 필드를 거절하므로, 어느 방식도 구형
실행 파일이 새 기록을 그대로 읽고 쓸 수 있다고 가정하지 않는다. 저장 방식 결정 후 읽기 호환·
쓰기 차단·형식 경계를 정하며, 이번 합의는 기존 프로젝트 자동 이전을 승인하지 않는다.

### 통합 Close transaction

다음은 현재 구현의 일반 완료 경로다. 위 미달성 종료를 이미 지원한다는 뜻이 아니다.
실행형 Node의 `gil close`는 Report 입력을 받으며 내부적으로 다음을 한 번에 수행한다.

```text
1. Current Existence == Node.existence_ref 확인
2. active_will.existence_ref == Node.existence_ref 확인
3. active_will.target_node_ref == Node.ref 확인
4. Report schema와 의미 제약 검증
5. Active Will을 immutable Done 목록에 append
6. active_will register를 비움
7. 새 Journey revision 생성과 current_journey_ref 이동
8. Node.journey_ref를 그 새 revision으로 기록
9. Report + Node.status = closed 확정
10. 완성된 format 3 state를 원자적으로 교체
```

하나라도 실패하면 Active Will과 Open Node를 포함한 기존 상태 전체를 유지한다. Will Done만
남거나 Report 없는 Closed Node가 생기는 중간 상태를 허용하지 않는다.

Close 성공 뒤 저장은 다음 모양이 된다.

```yaml
next_will_id: 2

existences:
  X1:
    current_journey_ref: journey:X1@J1
    journey:
      active_will: null
      done_wills:
        - id: W1
          existence_ref: existence:X1
          target_node_ref: step:C1/S1
          objective: 사용자의 프로젝트 목표를 확인한다
          next_action: 목표의 종류를 선택지와 함께 질문한다
          done_when: 사용자의 원문 응답을 얻는다
      revisions:
        J1:
          will_head_ref: will:W1
```

Will Done은 `done_when`의 행동을 수행했다는 선언이며 가설의 success/failure 판정이 아니다.

---

## 8. Verify 예시

```text
[Context]
Cycle 2의 Verify가 열려 있다.
현재 가설은 HTTP(S) scheme과 network location을 검사하는 것이다.
아직 검증 결과는 없다.

[Current Will]
objective
  HTTP(S)가 아닌 후보를 건너뛰는 가설을 검증한다.

next_action
  현재 구현에서 비HTTP 입력 테스트를 실행해 실패를 관측한다.

done_when
  구현 전후 결과를 확보해 Verify Report를 작성할 수 있다.

[Applicable Grammar]
Will이 done이고 Verify Report가 준비되면 `gil close`로 Verify를 닫는다.
```

---

## 9. Journey Timeline

Journey Timeline의 v0 최소 투영은 완료된 Will을 완료 시간순으로 나열하고 마지막에 Current
Active Will을 보여준다.

```text
Journey Timeline
  W1 done
  W2 done
  W3 active
```

Graph는 논리적 근거와 분기를 설명하고, Will Timeline은 Agent가 실제 시간순으로 무엇을 하려
했고 완료했는지 설명한다. Graph Current가 revisit으로 과거를 가리켜도 Will Timeline은
되돌아가지 않는다.

---

## 10. Context와 onboarding

`gil context`의 read model은 서술적 책임을 유지한다. 새 Agent onboarding에서는 Context와
Current Will과 Applicable Grammar를 서로 구분된 절로 함께 전달할 수 있다.

Current Will이 없다면 GIL은 실제 다음 작업을 추측하지 않고 없음을 명시한다.

```text
[Current Will]
  active Will이 없다.
```

---

## 11. v0 공개 문법

Will lifecycle을 별도 traversal 명령으로 노출하지 않는다.

```text
gil open   = Action Node + Active Will 시작
gil close  = Will Done + Journey revision + Report + Node Close
```

Current Will 조회는 `gil status`와 `gil context`의 read model이 담당한다.

실행형 Node Open은 Will 세 필드를 같은 stdin 입력에서 받는다. 기존 Report parser와 같은
field 문법을 사용하되 이것은 Node Close Report가 아니라 Action Contract다. 별도 대화형 입력을
암묵적으로 기다리지 않는다.

---

## 12. 검사 가능한 핵심 불변식

```text
active_will_count <= 1
will_id_is_unique_in_project
active_will_is_inline
done_wills_are_append_only_objects
active_will_targets_deepest_open_action_node
action_node_and_active_will_open_atomically
done_will_is_immutable
revisit_does_not_restore_historical_will
will_done_does_not_mean_hypothesis_success
action_node_close_completes_its_active_will
close_transaction_is_all_or_nothing
closed_node_journey_ref_contains_its_done_will
container_node_close_does_not_require_own_will
context_does_not_invent_missing_will
```

---

## 13. 아직 결정하지 않는 것

- Active Will 덮어쓰기의 crash-safe transaction
- 시간 값의 clock과 정렬 규칙
- `done_when` 미달성 시 원래 약속 보존·미수행 이유 기록·실패 종료 방향은 2026-11-19 확정했다.
  남은 것은 위 후속 계약의 저장 표현·원자적 종료·재시작 처리이며, 일반 취소 기능은 별도 미결이다.
- 여러 Agent가 동시에 Will을 갱신하는 방식
- Node Kind별 Will template
- Will과 Report provenance의 참조 단위
- Prospective Memory의 조건 충족 판정과 그로부터 Will을 만드는 trigger·평가 알고리즘

---

## 14. 핵심 문장

> **Context는 지금 무엇이 참인지 말하고, Will은 지금 무엇을 하려는지 말한다.**

> **미래 조건에서 기억해야 할 지속적 규약은 Prospective Memory다. Will은 Current Existence가
> 지금 수행하려는 하나의 행동 단위다. Prospective Memory의 조건이 충족되면 새로운 Current
> Will의 근거가 될 수 있다.**

> **현재 Will은 하나다. Active Will은 구체화하며 덮어쓰고, Done Will만 Journey Timeline에
> 확정한다.**

> **Revisit은 세계의 위치를 바꾸지만 Will Timeline을 되돌리지 않는다.**
