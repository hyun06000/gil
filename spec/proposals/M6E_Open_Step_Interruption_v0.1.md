# M6-E 제안 — 완료할 수 없는 열린 단계의 중단·보존·재시도 계약 (v0.1, 설계 제안)

상태: **설계 제안. 결정·Core 구현 미완료.** 기존 명세를 바꾸지 않으며 새 상태·명령·schema 를 Core 에 넣지 않는다.
검증: `node --test design-tests/m6e/m6e.test.mjs` — 순수 설계 모델의 전이표·fixture 검사(25건 통과). 이 모델은 GIL Core 가 아니다.

근거 자리: 기본 명세 §8 「진행이 멈춘 상태」·§24, Time Model §7·§18②, Will Model §13(중단된 Will), Cycle Model §9·§11·§21, Artifact Model §5·§7, Existence Model §7–9, Approved Plan 개발 계약.

## 1. 문제를 둘로 나눈다

| 구분 | 뜻 | 처리 |
|---|---|---|
| **운영상 기다림/재개** | 세션·앱·사람이 멈췄지만 기록은 온전하다 | 전이 없음. 같은 Existence 가 열린 Step·Active Will 을 읽고 이어간다 |
| **도메인 종료** | Node 를 사실대로 닫고 다음 시도로 간다 | 기존 close 문법 + 선택 필드 `conclusion` 하나 |

열린 Question/Synthesis/Outcome 이 사람 입력을 기다리는 것, 프로세스가 죽은 것, 앱이 끊긴 것은 모두 **기다림**이다. 자동 close·timeout·가짜 승인은 없다. 세계 관측 자체가 거절되는 환경(symlink·권한)도 기다림이며 환경을 고친 뒤 닫는다.

두 번째 축: **실패를 실제 근거로 완결할 수 있는 경우**(Verify 가 관측을 끝냈고 성공 조건이 평가됨 → 기존 `failure`)와 **필수 Report 를 만들 수 없는 경우**(행동이 끝나지 않아 `done_when` 을 못 채움). 후자가 M6-E 의 몫이다.

## 2. 접근 비교

| | A. 새 lifecycle 상태 (`abandoned`) | B. 세 번째 verdict (`inconclusive`) | **C. 좁혀서 닫기 + `conclusion` (추천)** |
|---|---|---|---|
| 기본 명세 §8 「Open/Closed 외 상태 금지」 | 위반 | 유지 | 유지 |
| Closed→Closed | 무르거나 abandoned 에서 revisit 허용 필요 | 유지 | 유지 |
| 기존 reader | 상태값 거절(안전하지만 전부 못 읽음) | verdict 검증 실패 | Report 추가 필드는 무시되어 읽힘 |
| Will | Done 아님·Active 아님의 제3 상태 필요 | 변화 없음 | Active Will 덮어쓰기(§6)로 범위를 좁힌 뒤 Done |
| Journey/Snapshot | 규칙 신설 | 변화 없음 | 기존 Verify close 규칙 그대로 |
| Chain 판정 전파 | 미정 | success/failure 이분법 깨짐 | 깨지지 않음 |

**C 를 추천한다.** 유일한 규범 변경은 Cycle Model §11 의 `Experiment failure → revisit` 표에 **`failure + not_reached → revisit | open_child`** 행 하나를 더하는 것이다(아래 §4·D1). 검사 `the only extension ...` 이 이 사실을 고정한다.

## 3. 계약

### 3.1 Active Will 좁히기 (`revise_will`)
Will Model §6 의 덮어쓰기를 공개 보조 동작으로 둔다. 같은 Will id·같은 target 에서 `objective/next_action/done_when` 만 바꾼다. Journey 사건이 아니며 Node 상태를 바꾸지 않는다. 중단 시 `done_when` 을 「중단 관측 문서를 저장한다」처럼 **달성 가능한 행동**으로 좁힌다. `done_when` 을 이미 충족된 것으로 사후 조작해 가짜 완료를 만드는 것은 금지(이것은 기계 검사 대상이 아니라 Report 의 정직성 규칙이다).

### 3.2 중단된 Verify 의 close
기존 Verify close 그대로다. `execution/result` 에 실제로 한 것과 막힌 것을 적고, `evidence_files` 에 **중단 관측 문서**(새 파일 → 신선도 조건 충족)를 둔다. 부분 파일은 그 Snapshot 에 보존된다. 빈 증거·변하지 않은 증거·채팅만으로는 닫히지 않는다(기존 규칙).

### 3.3 `conclusion` 필드 (Outcome · Cycle Report)
```text
conclusion: judged | not_reached      # 없으면 judged (기존 기록 호환)
blocker:    not_reached 일 때 필수, 비울 수 없음
```
- `not_reached` 는 `verdict: failure` 에서만 허용. `success + not_reached` 는 거절.
- Cycle Report 의 `conclusion` 은 Outcome 과 같아야 한다(verdict 와 같은 규칙).
- verdict 는 여전히 success|failure 둘이다. `not_reached` 는 「판정하지 않았다」는 선언이고 Chain 판정 전파를 바꾸지 않는다.

### 3.4 다음 방향
```text
Experiment  success/judged        → open_child
Experiment  failure/judged        → revisit                 (기존)
Experiment  failure/not_reached   → revisit | open_child    (M6-E 확장)
Interview   success               → open_child | close_chain
```
- `revisit`: 대상 Closed ancestor 분기점의 Exit 세계로 폴더를 되돌린다. 부분 파일은 폴더에서 사라지지만 실패 Cycle 의 Snapshot 에 남는다. product-safety 빌드에서는 실행이 거절된다(M4-R 과 동일).
- `open_child`: 부분 세계(중단 Verify 의 Snapshot = 그 Cycle 의 Exit)에서 다음 Cycle 을 연다. 폴더를 바꾸지 않으므로 **product-safety 아래에서도 진행 가능**하다. 다음 Cycle 이 Interview 면 사람이 부분 파일의 처리를 결정하고, Experiment 면 같은 항목의 재시도다.

### 3.5 승인 항목 재시도 (approved-plan)
- **pending 항목** = 활성 승인 판의 항목 중 *판정된(judged) 시도*나 *열린 시도*가 없는 것. not_reached 로 끝난 시도만 있는 항목은 pending 으로 남는다(현재 M6-D 의 「executed = 열렸다」와 다름 — D2).
- 다음 Experiment 는 첫 pending 항목을 묶는다. `attempt` = 같은 판에서 그 항목의 이전 시도 수 + 1, `retry_of` = 그 항목의 마지막 시도 Cycle. 기존 `Execution` 에 선택 필드 두 개가 늘어난다(없으면 attempt 1).
- **사람 동의의 재사용 범위**: 판+항목에 대한 승인은 not_reached 재시도에 재사용된다. judged failure 뒤에는 재사용되지 않으며 새 Interview 가 필요하다. 같은 판에서 not_reached 가 **2회**를 넘으면 세 번째는 거절(`RetryBudgetExhausted`) — 새 판이 승인되면 횟수는 0 에서 시작한다(D2).
- 재인터뷰 요청이 보존된 동안 새 Experiment 는 거절되고(기존 M6-D), 새 판 승인이 요청을 해소한다.

### 3.6 바뀌지 않는 것
Closed 기록 불변 · 단일 열린 Cycle/Step · Open Node 중 Existence 전환 금지 · Snapshot 확정 권한은 start 와 Verify close 둘 · Cycle close 는 Snapshot 을 만들지 않음 · 격리/quarantine 기능 없음(Snapshot 이 보존이다) · format 번호 불변 · 기존 기록 자동 이전 없음.

## 4. 기존 명세와의 충돌·접점 (숨기지 않음)
1. Cycle Model §11 「Experiment failure → revisit」에 not_reached 행 추가 → **규범 변경 1건**.
2. Approved Plan 「executed 는 열렸다는 뜻」 → pending 계산 의미 변경(not_reached 시도는 항목을 소비하지 않음).
3. Will Model §13 「중단·취소된 Will 의 처리」 → 좁혀서 Done 으로 해소; 별도 상태 없음.
4. Time Model §18② → `conclusion` + 기존 close 로 해소. Closed→Closed 유지.
5. 비-Verify Step 에서 폴더가 dirty 인 경우(Hypothesis 중 파일 편집) 는 여전히 close 거절이며 M6-E 가 풀지 않는다. product-safety 에서는 사용자가 수동으로 되돌려야 한다 — 결함으로 보고.

## 5. schema/tool 영향과 구현 단계 (제안)
| 단계 | 변경 | 소유 |
|---|---|---|
| 1 | `gil-spec.yaml` Outcome/Cycle Report 에 선택 `conclusion`·`blocker` 제약, `allowed_values_when` 확장(not_reached 행) | 통합 담당자 |
| 2 | `cycles.rs::is_branch_point` 는 그대로(open_child 선언이면 분기점) — 코드 변경 없음 확인 | 통합 |
| 3 | `approved_plan.rs`: `Execution{attempt, retry_of}` 선택 필드, `pending()` 을 judged 기준으로, 상한 검사 | 통합 |
| 4 | Will revise: `ProjectSession`/MCP 보조 도구(`gil_will_revise`) — target 고정, Journey 불변 | 통합 |
| 5 | guidance/workflow 문구: 「중단 시 Will 을 좁혀 사실대로 닫고 not_reached 를 선언한다」 | D 세션 소유 영역 |
Migration: 없음. 모든 필드는 선택이며 부재 = 기존 의미. format 6/7 의 `deny_unknown_fields` 때문에 구형 approved-plan writer 는 새 필드가 있는 기록을 거절한다(M6-D 와 같은 안전 거절).

## 6. 허용·금지 사례 (fixture 로 고정)
허용: 중단 문서로 닫힌 Verify(case1/2/5) · not_reached 뒤 open_child 재시도(case2a) · revisit 재시도(case2b) · 세션 교체 재개(case3) · 결론 없는 부분 파일(case4a) · 2회 뒤 Interview 재승인(case5) · 승인 대기 중 끊김과 재연결(case6).
금지: 빈/변하지 않은/없는/빈 파일 증거 · `success+not_reached` · blocker 없는 not_reached · Outcome 과 다른 Cycle verdict/conclusion · judged failure 에 open_child · Open Node/dirty/product-safety 에서 revisit · 두 번째 열린 Cycle · receipt 없는 approved:yes · 이전 판 승인 재사용 · 사람 응답 없는 Question close · 상한 초과 재시도 · 승인 없는 항목 실행.

## 7. 결정 쟁점 (최대 3)
- **D1.** `failure/not_reached → open_child` 를 허용할 것인가. 허용하면 product-safety 아래에서도 중단→재시도가 가능하고 부분 세계가 다음 출발점이 된다. 불허하면 M4-R 전까지 중단된 Experiment 는 기록만 남고 진행이 막힌다(case4b 와 같은 상태).
- **D2.** not_reached 시도가 항목을 소비하지 않는 pending 규칙과 **상한 2회**. 대안: 모든 재시도에 Interview 요구(상한 0) 또는 상한 없음.
- **D3.** Active Will 좁히기(`revise_will`)를 공개 동작으로 둘 것인가, 아니면 Agent 가 Report `execution/result` 에만 중단 사실을 적고 Will 은 그대로 Done 으로 둘 것인가(Will 의 `done_when` 과 실제 행동이 어긋남을 감수).
