# GIL Host UI Model v0.1

> 상태: Draft  
> 범위: GIL Core의 Monitor 사실을 Agent Host와 지속형 Desktop Companion의 공용 UI에 전달하는 계약  
> 비범위: 특정 Host SDK, Tauri component 구현, SVG 좌표, domain write action

설치 완료 조건, persistent Host surface 판정과 Native Companion 배포·설치 fallback은
`GIL Distribution Model v0.1`이 소유한다.

---

## 1. 목적

GIL의 인간 사용자는 Codex·Claude Desktop 같은 Agent Host 안에서 Agent와 협업한다. 그러나
Monitor는 선택 기능이 아니며 대화가 길어져도 밀려나지 않고 계속 보여야 한다. Host가 실제 수명
계약을 만족하는 지속형 fullscreen·panel·PiP surface를 제공하면 그 안에 싣는다. 제공하지 않거나
요청 뒤 `inline`에 머물면 지원 Host 표면 또는 같은 UI의 Tauri Companion을 안내하며, Companion은
사용자가 선택할 때만 연다.
어느 경우에도 GIL의 내부 Rust 타입이나 특정 Host API에 결합하지 않아야 한다.

이 문서는 다음 두 경계를 고정한다.

1. GIL Core가 Host UI에 넘기는 **버전 있는 읽기 모델**
2. 사람이 화면을 탐색할 때 발생하는 **표현 의도**

Graph·Journey·Will·Artifact를 바꾸는 의미 동작은 이 문서의 표현 의도가 아니다.

---

## 2. 소유권과 구조

```text
GIL domain state
  → owned MonitorSnapshot                 내부 read model
  → pure projection
  → MonitorViewV1                         Host 중립 wire model
  → shared UI bundle                      layout·interaction·accessibility
  → Host adapter                          수명·전달·현재 Project 연결
```

- `MonitorSnapshot`은 GIL Core의 내부 타입이다. 직접 직렬화하지 않는다.
- `MonitorViewV1`은 좌표가 없는 사실 계약이다.
- 공용 UI bundle이 DAG layout과 interaction을 한 번만 구현한다.
- Host adapter는 같은 UI를 싣는 얇은 수명·전달 경계다. Host마다 Graph layout을 다시 만들지 않는다.
- 특정 Host가 지속형 embedded UI를 제공하지 못하면 같은 `MonitorViewV1`을 Tauri Companion에
  싣는다. loopback browser는 개발·진단용 최후 fallback이다. 어느 표면도 다른 사실 계약을 만들지
  않는다.

---

## 3. `MonitorViewV1`

논리 구조는 다음과 같다.

```text
MonitorViewV1
├─ schema_version
├─ captured_at_unix_ms
├─ current
│  ├─ existence_ref
│  ├─ journey_ref
│  ├─ cycle_ref
│  └─ step_ref
├─ timeline[]
│  └─ TimelineCycleV1
├─ current_will
├─ world
└─ next_actions[]
```

### 3.1 최상위 칸

| 칸 | 형 | 규칙 |
|---|---|---|
| `schema_version` | integer | v1에서는 정확히 `1` |
| `captured_at_unix_ms` | integer | UTC Unix epoch 이후 밀리초, `0..2^53-1` |
| `current` | object | 현재 Existence·Journey·Cycle·Step |
| `timeline` | array | 이 Journey가 발급한 모든 Cycle, 발급 순서 |
| `current_will` | object 또는 `null` | 현재 행동 단위 |
| `world` | object | 기준 Snapshot과 clean·dirty·unknown |
| `next_actions` | array | 지금 실제로 성공할 수 있는 동작만 |

`current.step_ref`는 Cycle 경계에 서 있으면 `null`이다. 나머지 최상위 칸은 생략하지 않는다.

### 3.2 `TimelineCycleV1`

```text
TimelineCycleV1
├─ cycle_ref
├─ kind
├─ state
├─ relation_to_current
├─ parent_cycle_ref
├─ revisit_from_cycle_ref
├─ experiment_definition
├─ report
└─ steps[]
```

- `relation_to_current`는 `active_path | revisit_source | abandoned | other` 중 하나다.
- 네 관계는 상호 배타적이다. 여러 boolean으로 표현하지 않는다.
- `parent_cycle_ref`와 `revisit_from_cycle_ref`는 서로 다른 관계다. revisit을 parent edge로
  바꾸지 않는다.
- `experiment_definition`은 Experiment의 유일한 Define에서 읽은 `problem`과
  `success_condition`이다. 없으면 `null`이다.
- `report`는 닫힌 Cycle의 선택적 투영이다. 열려 있으면 `null`이다.
- `steps`는 그 Cycle이 만든 순서 그대로이며 비어 있어도 `[]`다.

`report`가 있을 때의 구조는 다음과 같다.

```text
CycleReportV1
├─ verdict
├─ handoff_summary
├─ outcome_lesson
└─ next_direction
   ├─ action
   ├─ reason
   └─ target_cycle_ref
```

`outcome_lesson`과 `next_direction`은 없으면 `null`이다. `next_direction`이 있을 때도 `reason`과
`target_cycle_ref`는 각각 없을 수 있다. 이 구조는 내부 `CycleReportFacts`의 선택적 투영보다
새 사실을 만들지 않는다.

Host wire에는 내부 `MonitorSnapshot.active_lineage`와 `inactive_cycles`를 싣지 않는다. 같은 Cycle을
여러 목록으로 보내면 UI가 어느 쪽을 진실로 삼을지 다시 결정해야 한다. `timeline` 하나가 Step DAG를
그리는 유일한 Cycle 목록이다.

### 3.3 `StepV1`

각 Step은 다음 네 사실만 초기 View에 싣는다.

```text
step_ref · kind · state · summary
```

`summary`는 Monitor read model이 정한 Report 원문의 대표 칸이며, 없으면 `null`이다. wire model은
이를 자르거나 다시 요약하지 않는다. 줄임표와 줄바꿈은 UI의 표현 책임이다.

`StepKindV1`은 `question | interpretation | synthesis | define | hypothesis | verify | analysis |
outcome`만 가진다. `cycle_entry`와 `cycle_exit`는 Step kind가 아니며 Step 목록이나 상세에 나타나지
않는다. Cycle 경계에 서 있다는 사실은 `current.step_ref: null`과 Cycle의 상태·가능한 다음 행동으로
표현한다. 내부 Snapshot이 경계를 Step처럼 싣고 있으면 투영은 이를 정상 Step으로 표시하지 않고
명시적으로 거절한다.

전체 Step Report는 초기 View에 넣지 않는다. 선택한 Step의 상세는 §7의 `NodeDetailV1`으로 요청한다.

### 3.4 `current_will`, `world`, `next_actions`

`current_will`은 `will_ref`, `objective`, `next_action`, `done_when`, `target_step_ref`,
`existence_ref`, `journey_ref`를 가진다.

`world`는 `baseline_snapshot_ref`, `state`, `reason`, `verify_can_confirm`을 가진다.

- `state`는 `clean | dirty | unknown`이다.
- `reason`은 `unknown`의 원인이 있을 때 그 원문이며, 그 밖에는 `null`이다.
- 파일 목록·manifest 주소·blob 주소·digest는 내보내지 않는다.

각 `next_actions` 항목은 `kind`, `step_kind`, `cycle_kind`, `command`, `reason`, `help_ref`를
가진다. 명령과 도움말이 없으면 각각 `null`이다. 아직 구현되지 않은 전이를 가능성처럼 싣지
않는다.

- `kind`는 `close_step | open_step | close_cycle | open_cycle | open_branch | revisit | restore`다.
- Step 동작은 `step_kind`에 대상 Step kind를 보존하고 `cycle_kind`는 `null`이다.
- Cycle 동작은 `cycle_kind`에 대상 Cycle kind를 보존하고 `step_kind`는 `null`이다.
- `revisit`과 `restore`는 두 대상 kind가 모두 `null`이다.
- `command`는 사람이 실행할 글자이지 동작의 구조를 판정하는 값이 아니다.

따라서 값을 지닌 내부 `ActionKind`를 합성 문자열로 폭발시키거나 인자를 버리지 않는다.

---

## 4. Canonical JSON v1

Host 경계의 canonical encoding은 UTF-8 JSON이다. Host가 같은 모양의 native object를 직접
전달해도 의미는 이 JSON 계약과 같아야 한다. 이는 MCP나 특정 SDK를 요구하지 않는다.

구현은 typed `MonitorViewV1` 투영과 JSON encoder를 독립된 조각으로 나눌 수 있다. typed 투영만
있는 과도기에는 이 절의 의미를 바꾸지 않으며, JSON 호환 완료를 주장하지 않는다. transport를
나중에 고르더라도 외부 byte encoding이 필요해지는 순간에는 이 canonical JSON을 사용한다.

- Graph·Journey·Artifact의 typed reference는 명세의 canonical 문자열로 쓴다. 예:
  `cycle:C2`, `step:C2/S3`. Manual의 `TopicId`는 이 불변식의 typed reference가 아니라 주소이며,
  View에서는 표시와 조회에 필요한 canonical 문자열만 보존한다.
- typed View 안의 닫힌 낱말은 **View 전용 enum**으로 표현한다. 내부 domain enum을 그대로
  재사용하지 않고 사용자 문자열로도 대신하지 않는다.
- JSON에서 enum은 이 문서가 정한 ASCII lowercase snake_case 문자열로 쓴다.
- optional object·scalar key는 생략하지 않고 `null`로 쓴다.
- collection key는 생략하지 않고 빈 경우 `[]`로 쓴다.
- 문자열은 원문 전체를 보존한다. HTML escape·Markdown 변환·요약·truncation을 하지 않는다.
- 배열의 순서는 의미다. renderer가 ID나 제목으로 다시 정렬하지 않는다.
- 객체 key 순서는 의미가 아니다.
- 숫자로 시간 순서를 추정하지 않는다. `timeline`과 `steps`의 배열 순서를 따른다.
- 같은 `schema_version`의 소비자는 모르는 object key를 무시할 수 있다. 그러나 모르는 enum 값이나
  지원하지 않는 `schema_version`은 추측해 그리지 않고 명시적으로 거절한다.

View 전용 enum을 `&'static str`로 대신하지 않는다. 그 형식은 출력에는 편하지만 외부 JSON을
typed View로 되읽는 계약을 표현하지 못한다. enum의 JSON 문자열 표현은 encoder 경계의 일이고,
typed View 자체는 닫힌 값의 집합을 보존한다.

### 4.1 Grammar 지원 범위

`MonitorViewV1`의 닫힌 enum은 함께 실린 **GIL Grammar v0.1**의 어휘를 지원한다. 현재 CLI는 이
Grammar를 사용한다. 라이브러리 안에서 별도 `RuleSet`을 주입할 수 있다는 사실만으로 그 확장값이
Host UI v1의 공개 어휘가 되지는 않는다.

주입된 Grammar가 v1에 없는 verdict나 direction action을 실제 Snapshot에 만들면 투영은 Cycle
Report를 `null`로 숨기거나 `other`로 바꾸지 않고 **View 전체를 명시적으로 거절**한다. 오류에는
해당 `cycle_ref`와 읽지 못한 원문을 보존한다. 새 어휘를 Host UI가 지원하기로 결정할 때는 View의
호환성 규칙에 따라 enum과 schema version을 함께 검토한다.

`MonitorSnapshot → MonitorViewV1` 투영은 순수 함수다. 파일·Project·clock을 다시 읽거나 잠금을
추가로 잡지 않는다. `captured_at_unix_ms`도 Snapshot이 이미 가진 관측 시각에서 만든다.

---

## 5. 좌표가 없는 계약

다음은 `MonitorViewV1`에 들어가지 않는다.

- x·y 좌표, lane 번호, 폭·높이
- SVG path, HTML, CSS class
- 접힘 여부, 선택된 Step, viewport와 zoom
- 색·아이콘·문자열 길이 제한
- Host panel ID나 browser URL

이 값은 Project의 사실이 아니라 공용 UI의 파생 표현이다. 같은 View가 넓은 panel, 좁은 inline
surface, 접근성용 목록에서 서로 다르게 보여도 가리키는 Step·Cycle·관계는 같아야 한다.

---

## 6. Presentation Intent v1

초기 interaction은 다음 표현 의도만 가진다.

| intent | 필요한 값 | 효과 |
|---|---|---|
| `select_step` | `step_ref` | Step을 선택하고 상세 요청의 대상을 정한다 |
| `clear_selection` | 없음 | 선택을 지운다 |
| `focus_current` | 없음 | 현재 Step 또는 Cycle 경계로 viewport를 옮긴다 |
| `collapse_cycle` | `cycle_ref` | Cycle 안의 Step을 접는다 |
| `expand_cycle` | `cycle_ref` | 접힌 Cycle을 펼친다 |
| `set_filters` | `FilterStateV1` | active path·failed branch·Interview의 표시를 바꾼다 |
| `reset_view` | 없음 | 임시 선택·접기·filter·viewport를 기본값으로 돌린다 |

이 의도는 공용 UI 안에서 처리하는 수명이 짧은 presentation state다.

- `.gil`에 저장하지 않는다.
- Report·Journey·Memory·Will로 승격하지 않는다.
- GIL action receipt처럼 가장하지 않는다.
- 다른 Agent Host가 반드시 이어받아야 할 상태가 아니다.
- 없는 Step이나 Cycle을 가리키면 UI는 의도를 버리고 최신 View를 유지한다.

pan과 zoom은 renderer 내부의 연속적인 viewport 상태라 wire intent 종류로 고정하지 않는다.

`FilterStateV1`은 `show_active_path`, `show_failed_branches`, `show_interviews` 세 boolean을 모두
가진다. key를 생략해 이전 값을 암묵적으로 유지하지 않는다. filter는 대상을 Graph에서 삭제하지
않고 화면에서만 감춘다. 현재 선택이 감춰지면 선택도 함께 해제한다.

### 6.1 Step DAG의 기준 표현

2026-09-11 인간 판독 시제품 검토에서 다음 **세로형** 표현을 v0 Companion의 기준으로 확정했다. 이 규칙은
`MonitorViewV1`의 사실을 바꾸지 않는 공용 UI의 presentation 계약이다.

1. Graph의 기본 단위는 **Step**이다. 모든 Step은 작은 원형 node로 표시한다. 선택한 Step도 node
   자체를 카드 크기로 키우지 않고 선택 강조만 더한다.
2. Step은 발급 시간 순서에 따른 서로 다른 행에 하나씩 놓인다. 같은 Cycle의 Step과 순차 자식
   Cycle은 같은 열을 유지한다. 실패 뒤 형제 Cycle처럼 실제 형제가 생길 때만 생성 순서에 따라
   오른쪽의 새 열을 쓴다.
3. 형제 분기 edge는 **항상 오른쪽**으로 갈라진다. 출발점에서 오른쪽으로 짧게 분기한 뒤 새 열을
   따라 진행하며, 공간이 부족하다는 이유로 왼쪽 열을 선택하거나 왼쪽으로 우회하지 않는다. 필요한
   오른쪽 여백은 layout이 먼저 확보한다. 필요 이상으로 길게 휘어 Graph를 가로지르지 않는다.
4. revisit은 생성 edge와 구분되는 점선이며 **항상 왼쪽**으로 되돌아간다. 완료된 전환에서 새 Cycle의
   `revisit_from_cycle_ref`는 **되돌아간 목표가 아니라 출발하게 만든 실패 Cycle**이고,
   `parent_cycle_ref`가 **실제로 되돌아간 목표 Cycle**이다. 따라서 점선은 `revisit_from` 실패
   Cycle의 마지막 Step 왼쪽에서 출발해 parent Cycle의 마지막 Step, 즉 화면에서 표현 가능한 Exit
   경계로 왼쪽 corridor를 따라 향한다. 같은 열의 revisit도 오른쪽으로 물러서지 않는다. 왼쪽 공간이
   부족하면 방향을 바꾸지 않고 Graph의 왼쪽 gutter를 늘린다. 화살촉은 parent 쪽 도착점을 명시한다.
   새 Cycle의 첫 Step에서 실패 Cycle로 향하는 선을 그리면 관계를 반대로 표현한 것이므로 금지한다.
5. Step kind는 색으로 보조 구분하되 색만으로 뜻을 전달하지 않는다. Cycle은 Step 묶음을 감싸는
   점선 경계와 이름으로 표시한다.
6. 정확히 하나의 Step을 선택한다. 선택한 node 옆에는 짧은 요약 카드를 띄우고, node 중심에서
   카드까지 굵고 반투명한 화살표를 직접 연결한다. 카드는 pointer event를 가로채 node 선택을
   막지 않는다.
7. 요약 카드는 Graph 안의 위치 관계를 설명한다. 전체 Report는 별도의 detail inspector에 계속
   표시하며, 요약 카드가 detail을 대신하지 않는다.
8. 펼친 Cycle의 우측 상단에는 접기 control을 둔다. 접으면 Cycle 안의 Step을 숨기고 약간 큰 원형
   Cycle node 하나로 바꾼다. 그 원형 node를 누르면 원래 Step 묶음으로 다시 펼친다.
9. Cycle을 접으면 빈 행을 그대로 남기지 않는다. 이후 Cycle과 Step을 위로 재배치하고 생성 edge,
   revisit edge, 선택 요약 카드와 연결 화살표도 새 좌표에서 다시 계산한다. 따라서 Cycle 수와 Step
   수가 커져도 접힌 Graph의 정보 밀도가 실제로 높아진다.
10. 접기·펼치기, 선택과 재배치는 presentation state다. `.gil`, Report, Journey, Memory, Will 또는
    `MonitorViewV1`에 기록하지 않는다.

Cycle을 접어도 Cycle의 존재, 순서, 부모·형제·revisit 관계를 삭제하지 않는다. 접힌 원은 같은
Cycle의 축약 표현이며, 펼치기는 원래 View 사실을 다시 보여 주는 동작이다.

#### 6.1.1 MCP Monitor의 가로형 기본 투영 (2026-09-27)

MCP Monitor는 **시간이 왼쪽에서 오른쪽으로 흐르는 가로형**을 기본으로 삼는다. inline에서도
동일하며, 방향 기본값을 실제 fullscreen 전환의 증거로 삼지 않는다. 사용자는 `가로로 보기` /
`세로로 보기`로 방향을 명시적으로 바꿀 수 있고, 그 선택은 같은 App instance의 mode 전환 동안
유지된다. native Companion의 기본값은 기존 세로형이다.

- 같은 공용 `place()`의 시간 순서와 관계를 투영한다. Step 하나가 배타적인 x 자리를 사용하며
  같은 Cycle과 첫 순차 자식은 같은 y lane을 잇는다. 실제 후속 형제만 생성 순서대로 아래의
  새 lane을 사용한다. 화면 폭에 맞춰 시간을 임의로 줄바꿈하지 않는다.
- 생성 edge는 오른쪽으로 진행하고 형제 lane으로 짧게 갈라진다. revisit은 여전히 실패 Cycle의
  마지막 표시 node에서 parent의 마지막 표시 node로 간다. 2026-09-28 디자인 결정에 따라
  **출발 node 상단 → 위쪽 전용 gutter → 왼쪽 과거 → 목표 node 상단**의 세 구간, 두 꺾임으로
  그린다. 화살촉은 아래를 향해 목표의 상단에 닿는다. 양 끝의 수평 돌출 구간을 없애 생성 edge와
  같은 선을 타지 않도록 한다. 접힌 Cycle 및 선택 강조 node의 실제 반지름을 반영한다.
  위쪽 통로는 Graph에 가깝게 둔다. 첫 lane 중심과 가장 가까운 통로는 28px(기존 52px),
  Cycle 경계와는 12px를 띄우고, 겹치는 되돌아감만 10px씩 바깥 통로를 추가한다.
  source/target의 의미를 회전하거나 뒤집지 않는다. 접기 control과 이름은 node 아래에 둔다.
- Cycle 접기는 숨겨진 시간 자리만큼 뒤의 node를 **왼쪽으로** 압축한다. 선택한 Step이 접혀도
  그 Cycle 원을 강조하고 요약·전체 Report의 동일성을 보존한다.
- 요약 카드는 Graph 안에서 선택 node 아래의 비어 있는 자리에 놓고 반투명 파란 화살표로 잇는다.
  연결선 몸통은 화살촉의 밑변에서 끝나며 끝점 밖으로 cap을 내밀지 않는다. 촉은 몸통 방향에
  맞춰 카드 쪽으로 돌출하고, 반투명은 몸통·촉 전체에 한 번만 적용해 이음부가 진해지지 않는다.
  다른 node·Cycle 경계·접기 control을 덮지 않는다. 공간이 모자라면 canvas를 확장하고 스크롤로
  닿게 한다. 전체 Report는 기존 detail inspector에 그대로 남는다.
- 방향 선택은 presentation state일 뿐이다. View/detail wire, `.gil`, scope, Journey 및 domain
  action을 변경하지 않는다. 대형 Graph의 virtualization·minimap은 여전히 별도 후속 범위다.

---

## 7. `NodeDetailV1`

초기 View는 Graph를 읽는 데 필요한 낮은 해상도만 싣는다. 사용자가 Step을 선택했을 때 Host
adapter는 같은 Project scope에서 canonical `StepRef` 하나로 상세를 요청한다.

```text
NodeDetailV1
├─ schema_version: 1
├─ step_ref
├─ cycle_ref
├─ kind
├─ state
└─ report
   └─ fields[]
      ├─ name
      └─ value
```

- `report`는 저장된 Report의 이름과 값을 **이름순으로** 보존한 목록이며, 열려 있으면 `null`이다.
  현재 `Report`의 canonical 순서가 이름순이고 삽입·저장 순서는 기록되지 않는다.
- 닫힌 Step의 `report.fields`는 `{ "name": string, "value": string }` 객체의 배열이다. JSON
  object의 key 순서에 기대지 않고 배열 순서로 이름순 계약을 보존한다. 빈 Report를 닫힌 Report로
  가장하지 않으며, domain이 허용한 실제 Report만 투영한다.
- field 이름과 값은 원문이다. UI가 아는 이름만 고르는 whitelist나 renderer용 label을 적용하지
  않는다.
- Cycle Entry와 Exit는 Step이 아니므로 `NodeDetailV1`의 조회 대상이 아니다. 그런 `StepRef`는
  다른 대상을 추측하지 않고 `not_found`다.
- UI는 임의의 `.gil/state.yaml` 경로를 받아 직접 읽지 않는다.
- 요청한 Step이 현재 Project에 없으면 v1은 `not_found`로 답하고 다른 Step을 추측하지 않는다.
- Project가 바뀌었는지는 Host adapter가 scope identity로 먼저 가른다. 서로 다른 Project의 요청을
  현재 Project에 보내지 않는다.
- 상세 조회도 read-only이며 domain state와 Artifact 세계를 바꾸지 않는다.
- 같은 Step의 상세 재조회가 실패하면 마지막으로 확인한 보고서를 보존하고 최신 내용을 확인하지
  못했음을 알린다. 다른 Step을 고르면 이전 보고서를 즉시 걷으며, 같은 Step의 중복 조회나
  Project를 떠났다가 돌아오는 경우에도 최신 화면의 최신 요청만 Report와 오류 안내를 바꾼다.
  재조회 성공은 상세 오류만 걷고 다른 동작의 오류 안내는 지우지 않는다.

Report media의 resource 계약은 아직 없으므로 v1 상세가 임의 HTML·script·외부 resource를
실행하지 않는다.

---

## 8. 갱신과 stale 상태

Host는 최초에 완전한 `MonitorViewV1` 하나를 전달한다. 이후 watcher나 Host channel은 다음 의미만
가진 hint를 보낼 수 있다.

> 이 Project의 사실이 바뀌었을 수 있으니 완전한 View를 다시 요청한다.

- hint는 Node·edge·Report 사실을 담지 않는다.
- UI는 partial event를 기존 View에 합쳐 Graph 사실을 만들지 않는다.
- 새 View가 오기 전까지 기존 View를 마지막으로 확인된 상태로 표시할 수 있다.
- Project scope가 바뀌면 이전 View와 detail을 즉시 폐기한다.
- refresh 실패를 `dirty`나 빈 Graph로 바꾸지 않는다. 마지막 View와 조회 실패를 구분한다.

---

## 9. Host Adapter의 책임

Host adapter는 다음만 책임진다.

1. 현재 대화가 명시적으로 가리키는 Project scope 연결
2. View와 detail의 전달 및 수명 관리
3. refresh hint를 받으면 완전한 View 재조회
4. 공용 UI bundle을 Host의 persistent surface 또는 inline preview에 탑재
5. 지원하지 않는 schema를 사람에게 명시적으로 표시
6. 요청 성공이 아니라 실제 display mode와 수명 계약으로 persistent surface 지원 여부 판정

Host adapter는 다음을 하지 않는다.

- `.gil` 직접 파싱·수정
- Cycle 관계·lane·요약 재계산
- domain gate 복제
- 최근 폴더나 cwd를 추측해 Project 선택
- Host별로 다른 Graph 의미 구현

초기 embedded Monitor는 read-only다. 승인·open·close·revisit·restore 같은 버튼은 별도의 Human
Checkpoint와 typed action 계약이 확정될 때까지 제공하지 않는다.

### 9.1 Desktop Companion adapter

Host가 지속형 surface를 제공하지 않는 동안 v0의 기준 adapter는 **하나의 Tauri Companion**이다.

- Companion process와 창은 Project나 Agent session마다 새로 만들지 않는다.
- 하나의 창에서 여러 GIL Project를 명시적으로 등록하고 전환한다.
- 각 Project는 canonical root와 stable scope identity로 구분한다.
- Project를 바꾸면 이전 View·detail·presentation state를 현재 Project에 섞지 않고 폐기하거나
  Project별 임시 상태로 격리한다.
- 현재 선택한 Project만 foreground 해상도로 관찰한다. 등록되었다는 이유만으로 모든 Project를
  계속 비싸게 재관측하지 않는다.
- 최근 Project 목록, 마지막 선택과 창 위치는 Companion 설정이다. `.gil`의 Graph·Journey·Memory가
  아니며 Project Artifact에도 포함하지 않는다.
- Companion은 `.gil/state.yaml`을 직접 해석하지 않고 GIL의 검증된 read API 또는 canonical
  `MonitorViewV1` adapter만 사용한다.
- Companion이 떠 있지 않아도 GIL의 의미 동작과 저장은 정상 동작한다.
- 사용자에게 보이는 Companion 표식은 소문자 `gil` 워드마크이며 단독 대문자 `G`를 쓰지 않는다.

Tauri는 v0 packaging 결정이지 wire 계약이 아니다. 장차 Agent Host가 지속형 panel을 제공하면 같은
UI bundle과 View/detail 계약을 그 adapter에 싣고, Tauri를 필수 설치에서 다시 내릴 수 있다.
단, inline 카드만 제공하거나 PiP 요청 뒤 실제 mode가 `inline`이면 지속형 surface로 세지 않는다.

#### 9.1.1 실제 GIL read adapter의 첫 조각

fixture 다음의 첫 조각은 사용자가 **명시적으로 선택한 하나의 GIL Project**를 읽는 adapter다.
이 조각의 갱신 방식은 수동 refresh 하나뿐이며 watcher, 최근 Project 저장과 창 위치 저장은 포함하지
않는다.

1. 사용자는 OS folder picker에서 Project root를 선택한다. adapter는 cwd, 최근 폴더 또는 열린
   대화에서 Project를 추측하지 않는다.
2. 선택한 root는 GIL의 검증된 project open 경로를 통과해야 한다. Companion은
   `.gil/state.yaml`을 직접 읽거나 부분적으로 해석하지 않는다.
3. 성공한 open은 stable scope identity와 완전한 `MonitorViewV1`을 돌려준다. UI는 fixture와 같은
   wire 계약으로 이를 받으며 별도의 Tauri 전용 Graph 사실을 만들지 않는다.
4. Step 선택은 현재 scope와 canonical `StepRef`를 함께 전달해 `NodeDetailV1`을 요청한다. scope가
   바뀌었거나 Step이 없으면 다른 Project나 가까운 Step으로 물러서지 않는다.
5. 수동 refresh는 현재 선택 Project의 완전한 `MonitorViewV1`을 다시 요청한다. 기존 View에 파일
   변화나 부분 event를 합쳐 새 사실을 만들지 않는다.
6. refresh가 실패하면 마지막으로 검증된 View를 그대로 보존하고, 그것이 최신이라고 가장하지 않은 채
   실패 이유를 별도로 표시한다. 실패를 `dirty`, 빈 Graph 또는 Project 제거로 바꾸지 않는다.
7. Project 없음, GIL Project가 아님, 지원하지 않는 저장 판, 잠금 경쟁, 손상과 관측 실패는 서로 다른
   typed refusal로 보존한다. UI는 오류 문자열을 파싱해 의미를 추측하지 않는다.
8. folder picker 취소는 상태 변화 없는 취소다. 현재 Project와 마지막 View를 폐기하지 않는다.

이 adapter의 모든 동작은 read-only다. project open, View 조회, detail 조회, refresh와 거절은
`.gil`의 Graph·Report·Journey·Memory·Will, Artifact 파일, Snapshot 창고와 작업 파일을 쓰지 않는다.
관측이 객체를 확정하거나 `state.yaml`을 다시 저장해서도 안 된다. 실제 GIL Project를 선택해 View와
detail을 읽고 refresh한 뒤에도 이 바이트와 객체 수가 전부 같다는 시험이 첫 조각의 합격 조건이다.

fixture는 제거하지 않는다. fixture는 공용 UI bundle의 결정적 표현 회귀 시험이며, 실제 adapter는
같은 `MonitorViewV1`·`NodeDetailV1` 경계에 새로운 사실 공급원으로만 붙는다.

#### 9.1.2 Companion-local settings

Companion이 재시작 뒤에도 Project 목록과 창의 자리를 복원하려면 Project 밖의 **앱 전용 설정**을
사용한다. 이 설정은 Monitor 사실도 Journey의 일부도 아니며 다른 Host가 따라야 하는 wire 계약도
아니다.

v0 설정이 기억하는 것은 다음뿐이다.

```text
CompanionSettings v1
├─ 등록한 Project[]
│  ├─ canonical root                 native adapter 안에서만 쓰는 비공개 위치
│  ├─ stable scope identity
│  ├─ display name
│  └─ last used order
├─ 마지막으로 선택한 scope identity | null
└─ 창 geometry
   ├─ position | null
   ├─ size
   └─ maximized
```

- 설정은 OS가 이 앱에 배정한 app-config directory 한 곳에 저장한다. `.gil`, Project root, cwd와
  사용자의 임의 문서 폴더에 두지 않는다.
- canonical root는 재실행 뒤 native registry를 복원하는 데만 사용한다. JS·DOM·tooltip·View·detail
  JSON·오류 receipt로 보내지 않는다. UI 명령은 계속 scope identity만 사용한다.
- 설정의 scope identity를 믿어 Project를 합치지 않는다. root를 canonicalize해 identity를 다시
  계산하고, 저장된 값과 다르면 그 항목을 unavailable로 표시한다.
- 앱 시작은 등록됐다는 이유로 모든 Project의 GIL Graph나 Artifact 세계를 읽지 않는다. 설정만
  복원한 뒤 마지막으로 선택했던 Project 하나만 foreground read adapter로 연다.
- 마지막 Project가 없거나 열리지 않으면 다른 Project를 임의로 선택하지 않는다. 목록은 유지하고
  실패한 항목을 unavailable로 표시하며 사용자가 선택하게 한다.
- Project를 목록에서 제거하는 것은 Companion 설정의 항목만 지운다. Project directory, `.gil`,
  Artifact, Snapshot과 Report는 삭제하거나 수정하지 않는다.
- display name이 같은 항목이 둘 이상이면 UI에서만 stable scope identity의 짧은 suffix를 덧붙여
  구분한다. suffix는 표시일 뿐 identity가 아니며 Host 요청에는 전체 scope를 사용한다.
- 마지막 선택은 View를 성공적으로 읽은 뒤에만 바꾼다. 열지 못한 Project나 취소된 picker가 마지막
  정상 선택을 덮어쓰지 않는다.
- 창 position과 size는 logical pixel로 저장한다. 복원 시 현재 연결된 display의 보이는 영역과 최소
  크기에 맞춰 제한하며, 사라진 monitor의 좌표 때문에 창이 화면 밖에 놓이지 않게 한다.
- Step 선택, Cycle 접기, scroll과 열린 detail은 이 판에서 저장하지 않는다. Project별 presentation
  state는 창이 살아 있는 동안의 임시 상태다.

설정 저장은 앱 설정 파일 하나를 대상으로 하는 atomic replace다. 완전한 새 내용을 임시 파일에 쓰고
동기화한 뒤 기존 파일을 교체한다. 중간 실패 시 마지막 정상 설정을 보존하며 Project 파일에는 손대지
않는다. 여러 UI event가 연달아 발생하면 debounce할 수 있지만, 정상 종료 전에 마지막 값을 flush한다.

지원하지 않는 settings schema나 읽을 수 없는 설정은 빈 설정으로 가장하거나 자동으로 덮어쓰지 않는다.
원본 파일을 그대로 보존하고, 이번 실행은 설정이 저장되지 않는 임시 상태로 열며 사람에게 typed
settings refusal을 표시한다. 명시적 reset UX는 이 조각의 범위 밖이다.

Companion settings를 쓰는 동안에도 GIL read adapter의 read-only 불변식은 그대로다. 시험은 설정
파일만 바뀌고 등록한 모든 Project의 파일 수·바이트·Artifact 객체 수는 전혀 바뀌지 않음을 확인한다.

### 9.2a 2026-09-23 MCP fullscreen 기준점

정식 `ui://` V3 진단판의 앱 선언은 `inline/fullscreen/pip`이었다. macOS 사용자 화면 실측:

| Host 표면 | Host 광고 | 요청 응답·event | 실측 범위 |
| --- | --- | --- | --- |
| Codex | inline, fullscreen | fullscreen | 오른쪽 패널과 대화 병행, 사용자 유지 확인 |
| Claude Desktop Code | inline | inline | 지속형 fullscreen 표면 아님 |
| Claude Desktop Cowork | inline, fullscreen | fullscreen | 확대 화면과 입력창 병행 |

제품 기본은 **Codex / Claude Desktop Cowork의 MCP fullscreen**으로 옮긴다. PiP 성공이라고
부르지 않는다. Companion은 삭제·축소하지 않고 Windows 검수와 Host 수명 미달에 대비해 유지한다.
진단판 성공은 실제 GIL Project, live refresh, 작업 전환 및 재시작 검수와 별개다.

첫 실제 연결은 기존 `ui/companion.js`·`layout.js`·CSS를 번들하여 같은 UI를 제공한다.
`mcp-app`는 Host 표시·stdio 도구 연결만 소유한다. 별도 채팅 UI는 만들지 않고 Host composer를 쓴다.

- 준비 도구는 명시적인 Project root만 받는다. Core의 exact-root read-only 검증 후 opaque scope를
  돌려준다. 렌더 도구와 App에는 root가 아니라 scope만 보내며, UI tool-input에도 경로를 싣지 않는다.
- `show_gil_monitor`만 resource에 연결한다. 조회·상세·hint 도구에 resource를 붙여 매번 새 카드를
  만들지 않는다. HTML의 내용 hash가 resource URI를 정해 오래된 cache와 혼동하지 않는다.
- 앱은 `inline/fullscreen`을 선언한다. Host 연결과 실제 Monitor 준비 뒤 fullscreen 지원 광고가
  확인되면 인스턴스당 한 번 자동 요청한다. 이미 fullscreen이면 요청하지 않는다. 거절·무응답·inline
  반환 때 자동 반복하지 않고 `모니터 펼치기`를 유지한다. 사용자가 inline으로 돌아오거나 다른 표면을
  선택하면 다시 강제로 펼치지 않는다. Host 광고·반환 mode·context event를 별도로 남기며, 반환값이
  없거나 inline이면 성공으로 가장하지 않는다. Companion은 사용자가 요청할 때만 연다.
- 최초 App 하나는 준비한 scope 하나에 묶인다. 다른 Project는 새로 명시하여 연다. native picker와
  최근 Project 설정은 그대로 남으며, MCP 화면이 native 선택을 몰래 바꾸지 않는다.
- 감시는 Core의 `watch_hints`를 재사용한다. 2초 heartbeat는 counter만 읽으며 Project를 훑지 않는다.
  변화 hint 때 완전 View를 다시 읽고, 무변화 reconciliation은 5분, 실패는 2/4/8/16/30/60초 backoff다.
  UI별 마지막 성공 cursor를 보유해 한 화면이 다른 화면의 hint를 소비하지 않는다.
- 사라진 App은 heartbeat를 중단한다. 90초 lease와 15초 회수 주기로 watcher를 해제한다.
  숨김 중에는 heartbeat timer도 예약하지 않는다. 재개 시 같은 scope로 완전 조회하며, 진행 중 조회가
  있으면 끝난 뒤 복귀 요청들을 하나로 합친다. 보관된 page의 숨김·복원은 실제 teardown과 구분하여
  다시 갱신할 수 있어야 한다. 실제 teardown 뒤에는 뒤늦은 응답이 timer를 되살리지 않는다.
  복귀 자체로 새 App이나 fullscreen 요청을 만들지 않는다. transport 재시작만을 이유로 사용자에게
  매번 재열기를 요구하지 않는다. 이 계약은 Host가 제공하지 않는 iframe 보존을 보장한다는 뜻이 아니다.
- read·detail은 `ReadOnlySession` 경계다. 실패 중 마지막 View와 보고서를 보존하며 `.gil` 복구나
  write action을 UI에 열지 않는다. 표면 준비 완료를 `opened_persistent_host`로 보고하지 않는다.
- fullscreen에는 Host 채팅 입력창이 겹칠 수 있으므로 문서 끝에 스크롤 가능한 하단 여백을 둔다.
  초기 여백은 viewport 높이에 따라 200–320 CSS px와 기기 safe-area 하단값이다. 보고서를 자르거나
  별도 채팅창을 만들지 않는다. Host 바깥 DOM을 측정했다고 주장하지 않으며, inline과 native에는
  이 여백을 적용하지 않는다. 더 큰 입력창·다른 Host의 실제 가림 여부는 별도로 검수한다.

macOS 두 실제 Host의 GIL 화면 검수, Cowork Plugin 설치 경로 및 Windows 검수는 별도 인수 항목이다.

#### 2026-09-28 MCP App 우선 인수

새 Rust 설치본 `0.2.0+codex.20260927134559`의 가로 DAG·fullscreen·노드 상세 확인 요청에
사용자가 동작 성공을 보고했다. 이는 macOS Codex의 실제 GIL 화면 증거이며 Cowork Plugin·Windows·
Host 완전 종료 후 표현 상태 복원까지 대신 증명하지 않는다. 이어 다른 대화에 갔다 돌아오는 시험에서
사용자가 선택·접힘·보고서가 “그대로 유지돼”라고 확인했다. 수동 inline·세로 선택의 유지나 새 App
복원을 확인한 것으로 확대하지 않는다.

다음 개발은 MCP App에 집중한다. Host composer와 함께 Graph·요약 카드·전체 Report·실시간 갱신을
사용하는 경로를 유지하고, 별도 채팅이나 Tauri 전용 UI를 새로 구현하지 않는다. Tauri 전용 검증은
현재 MCP App 인수의 선행 조건에서 제외하되, 공용 renderer와 read-only 경계의 시험은 유지한다.
Companion의 보존·검증 재개 조건은 Distribution Model §3.1을 따른다.

#### 2026-09-25 재시작 후 자동 연결 복원

사용자 실측에서 계산기 제작 → 공학용 계산기로 확장하는 동안 Codex Monitor의 실시간 Graph
갱신은 성공했다. 그러나 완전 종료 뒤 예전 화면의 scope만 남고 메모리 등록부가 비어 상세 조회가
거절됐다. 화면 복원과 서버 연결 복원을 구분하며, 수동 재연결을 정상 UX로 삼지 않는다.

- 명시적인 `gil_monitor_prepare` 성공 시 **MCP Monitor 전용 사용자 로컬 설정**에 root와 파일시스템
  identity를 저장한다. Project·`.gil`·Companion 설정·Plugin cache·HTML·localStorage에는 쓰지 않는다.
- macOS 저장 위치는 `~/Library/Application Support/GIL/monitor-bindings-v1/`이다. 디렉터리 0700,
  파일 0600, schema version이 있는 scope별 immutable JSON을 원자적으로 공개한다. 동시 Codex/Cowork
  연결의 서로 다른 항목이 덮어써지지 않으며 손상·미지원 설정은 보존하고 typed refusal로 답한다.
- scope는 canonical root와 `.gil` 디렉터리의 OS identity를 함께 식별한다. Unix의 device·inode와
  사용 가능한 생성 시각을 쓴다. 매 commit마다 교체되는 `state.yaml` inode는 identity로 쓰지 않는다.
  같은 경로에 새 Project를 명시적으로 열어도 **옛 화면의 scope는 새 Project를 가리키지 않는다.**
- 재시작한 서버는 요청받은 scope의 설정 **하나만** 지연 복원한다. 전체 최근 목록을 훑거나 cwd·
  Companion 선택·비슷한 이름으로 추측하지 않는다. canonical 위치와 OS identity를 다시 검사하고
  View/Report는 `ReadOnlySession`으로 새로 읽는다. root는 계속 Rust 안에만 남는다.
- hint revision은 opaque **서버 세대 + counter**다. PID·경로 원문을 싣지 않는다. 재시작 후 counter가
  같아도 완전 View를 곧바로 다시 읽으며, 조용한 프로젝트의 5분 reconciliation은 유지한다.
- 폴더 없음·교체·설정 손상은 마지막 Graph를 보존한 채 정확한 이유를 알린다. 연결 실패 재시도는
  2/4/8/16/30/60초로 제한한다. `unknown_scope` 때문에 App을 영구 정지시키지 않는다.
- 저장 기능 이전에 연 화면에는 복원 기록이 없으므로 업데이트 후 **한 번만** 새 Monitor를 열어
  명시적 선택을 저장해야 한다. 과거 대화나 파일 시스템을 수색해 선택을 추정하지 않는다.
- 설정 제거/Plugin 제거 연동, 표현 상태의 Host 간 복원, Windows OS identity·설치 검수는 별도다.
  Unix가 아닌 환경에서 약한 identity로 자동 복원을 가장하지 않으며 Companion을 유지한다.

### 9.2 2026-09-08 Host surface 예비 진단 (이전 관찰)

Codex Desktop의 inline Plugin UI에서 fixture 기반 GIL Companion을 열고
`requestDisplayMode({ mode: "pip" })`를 버튼과 초기 자동 요청 두 경로로 실행했으나 화면은
`inline`에 머물렀다. 그러나 이 진단은 실제 `ui://` MCP App의 `ui/initialize`에서
`appCapabilities.availableDisplayModes`를 선언하고 Host가 광고한 capability와 요청 반환값을 함께
기록한 시험이 아니었다. 따라서 이 결과는 해당 진단 카드가 PiP로 전환되지 않았다는 증거일 뿐,
Codex Host의 PiP 미지원이나 실제 적용 mode를 확정하는 증거가 아니다.

정식 판정은 장차 실제 `ui://` probe에서 다음을 함께 기록한 뒤 내린다.

1. 앱이 `ui/initialize`에서 선언한 `availableDisplayModes`
2. Host context가 광고한 `availableDisplayModes`
3. 사용자 동작에 묶인 `requestDisplayMode({ mode: "pip" })`의 원문 반환값
4. `openai:set_globals`로 관측한 최신 실제 mode와 surface 수명

그때까지 다음 제품 결정을 유지한다.

1. Codex inline Plugin UI는 공용 UI bundle과 interaction의 시제품·회귀 표면으로 유지한다.
2. 검증되지 않은 PiP를 지속형 Monitor의 전제로 삼지 않는다.
3. 같은 세션마다 loopback server를 새로 띄우는 browser 경로를 기본 UX로 삼지 않는다.
4. 지속 관찰의 v0 기준 구현은 Tauri Companion이다.
5. 장차 PiP가 생겨도 대화에 밀리지 않음·명시적 종료 전 유지·Project scope 복구·완전한 View 갱신을
   실측한 뒤에만 persistent surface로 판정한다.

이 판정은 현재 또는 미래의 Codex가 PiP를 지원하지 않는다는 주장이 아니다. 정식 probe를 수행할
때 adapter 시험을 다시 수행하며, GIL Core와 `MonitorViewV1`은 바꾸지 않는다.

---

## 10. 보안과 격리

- View의 모든 문자열은 data다. HTML·Markdown·명령으로 실행하지 않는다.
- UI는 Project 밖의 경로를 요청하거나 표시를 근거로 파일을 열지 않는다.
- 한 Project의 View·detail·presentation state를 다른 Project와 섞지 않는다.
- browser fallback의 capability URL과 loopback 검증은 Monitor Model의 기존 계약을 따른다.
- Host가 가진 더 넓은 권한은 GIL UI의 권한이 아니다.

---

## 11. 호환성

- `schema_version`은 wire 의미의 판 번호다. 저장 format 번호와 다르다.
- key 추가처럼 기존 의미를 바꾸지 않는 확장은 v1 안에서 가능하다.
- key 제거·형 변경·enum 의미 변경·배열 순서 의미 변경은 새 schema version을 요구한다.
- 내부 Rust 타입과 파일 배치는 공개 계약이 아니므로 wire 의미를 보존하면 바꿀 수 있다.
- fallback과 모든 Host adapter는 같은 fixture에 대해 의미상 같은 View를 받아야 한다.

---

## 12. 시험 가능한 불변식

1. 같은 `MonitorSnapshot`은 Host 종류와 무관하게 같은 `MonitorViewV1`을 만든다.
2. 투영은 추가 관측·저장·잠금을 하지 않는다.
3. wire에는 `timeline`만 있고 `active_lineage`·`inactive_cycles` 중복 목록은 없다.
4. Cycle과 Step의 배열 순서는 내부 read model의 발급 순서와 같다.
5. Graph·Journey·Artifact의 모든 typed reference는 canonical 문자열로 왕복한다.
6. optional key와 collection key의 모양은 상태에 따라 사라지지 않는다.
7. 긴 summary와 Report 값은 wire에서 잘리지 않는다.
8. 좌표·SVG·HTML·presentation state가 wire에 없다.
9. presentation intent 전후 `.gil`, Journey, Will, Artifact 세계가 같다.
10. 선택한 Step만 `NodeDetailV1`으로 읽고 초기 View에는 전체 Report가 없다.
11. 현재 Project에 없는 detail 요청은 `not_found`이며 다른 Node로 물러서지 않는다.
12. update hint만으로 화면의 Graph 사실이 바뀌지 않는다.
13. 모르는 schema·enum은 조용히 추측해 그리지 않는다.
14. 공용 fixture를 Host surface와 fallback에서 읽었을 때 Step·Cycle·관계가 같다.
15. 같은 Cycle의 Step은 같은 lane(세로형의 열 / 가로형의 행)을 유지하고, 실제 형제 Cycle만 새 lane을 사용한다.
16. Cycle 접기 뒤 숨겨진 시간 자리만큼 이후 node가 재배치되며 edge와 선택 카드가 새 좌표를 따른다.
17. 선택 요약 카드의 연결 화살표는 선택한 Step 또는 접힌 Cycle node에서 시작한다.
18. 완료된 revisit 점선은 `revisit_from` Cycle의 마지막 Step에서 parent Cycle의 마지막 Step으로
    향하며, 새 Cycle에서 `revisit_from` Cycle로 향하지 않는다.
19. 세로형의 형제 분기는 오른쪽, revisit 출발·corridor는 왼쪽이다. 가로형은 §6.1.1대로 시간은
    오른쪽, 형제 lane은 아래쪽, revisit은 상단에서 출발해 위쪽 corridor를 왼쪽으로 따라간 뒤
    목표 상단에 아래 화살촉으로 닿는 두 꺾임이다. viewport나 lane 위치
    때문에 한 방향 안에서 규칙을 뒤집지 않는다.
20. 방향을 바꿔도 scope·선택·접힘·전체 Report와 node/edge의 참조는 같고, 카드·접기 control이 node를 덮지 않는다.

---

## 13. 아직 정하지 않는 것

- Codex·Claude Desktop이 장차 제공할 지속형 UI SDK와 packaging 방법 및 출시 시점
- 공용 UI bundle의 구체 framework와 배포 단위
- Tauri Companion과 GIL Core 사이 View/detail transport의 구체 선택
- GIL Grammar v0.1 밖의 사용자 정의 Grammar를 Host UI에서 지원하는 방식
- semantic zoom, minimap, 검색과 대형 Graph virtualization
- presentation state를 Host 재시작 뒤 복원할지 여부
- 같은 Project 안에서 View가 갱신된 뒤의 detail 요청까지 식별하는 `view_token` 또는 revision.
  v1 첫 구현은 이를 추측하지 않고 없는 Step에 `not_found`만 답한다
- Report media의 안전한 resource reference
- Human Checkpoint와 domain write action UI
- 등록 Project가 매우 많을 때 background 관찰·알림 정책

이 항목은 `MonitorViewV1`의 사실 의미를 바꾸지 않는 범위에서 후속 명세가 정한다.

---

## 14. 핵심 문장

> **GIL Host UI 계약은 Graph를 그린 결과가 아니라 그릴 수 있는 사실을 전달한다. 좌표와 선택은
> UI의 임시 상태이고, Project의 사실은 오직 GIL Core가 만든 버전 있는 View에서 온다.**

> **지속형 surface가 없는 Host에 Monitor의 수명을 억지로 맡기지 않는다. v0 Companion은 한 번 뜬
> 창에서 여러 Project를 명시적으로 전환하며, Host가 그 수명을 제공하게 되면 같은 계약을 옮긴다.**

> **inline UI는 공용 bundle의 미리보기일 수 있지만 설치 완료를 이루는 지속형 Monitor는 아니다.**
