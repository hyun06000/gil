# GIL Chain storage — M6-A

2026-10-31. 범위: 실제 Chain 생성·소유·Current·저장·재시작. 규범은
[기본 명세 §5](GIL%20Specification%20v0.1.md), [Node Model §6–7](GIL_Node_Model_v0.1.md),
[Cycle Model §5–10, §18–21](GIL_Cycle_Model_v0.1.md)이다.

## 활성화와 범위

`cargo build --locked -p gil --features chain-core --bin gil`로 개발 프로필을 만든다.
시각 Report/MCP 제품 경로 시험은 `--features chain-core,visual-authoring,product-safety`를 쓴다.
macOS와 Windows에 같은 도메인·MCP 규칙을 사용한다. 기존 설치본·업데이트 피드·배포 프로필은
이번 변경으로 갱신하지 않는다. 기본 빌드의 신규 생성 형식은 기존 4/5다.

`chain-core`의 새 Project는 `Project → Chain → Cycle → Step`을 실제로 소유한다.
Chain은 이름만 붙인 UI 그룹이나 Cycle의 `parent`가 아니다. M6-A는 첫 Open Chain 하나만
허용한다. Closing Interview, Chain Report, 종료 판정은 M6-B이며 다음 Chain 생성은 M6-C다.
`gil open chain` / `gil_open(kind: "chain")`은 Open Chain의 자식 생성을 거절한다.
이 버전에는 Chain 종료·취소·Chain revisit의 우회 API가 없다.

## 저장과 불변식

| 형식 | World 소유 | 시각 registry | Windows 저장 보호 |
|---|---|---|---|
| 4 | Project → Cycles | 없음 | 기존 계약 |
| 5 | Project → Cycles | 있음 | 기존 소유자 전용 계약 |
| 6 | Project → Chain → Cycles | 없음 | format 4와 동일 |
| 7 | Project → Chain → Cycles | 있음 | format 5와 동일 |

6/7은 `chain-core`만 읽고 쓴다. 구형 writer와 해당 기능이 없는 기본 빌드는 format gate에서
거절한다. 형식 번호만 낮추면 Chain 구조가 남으므로 역시 거절한다. 6에 visual registry를 넣거나
7에서 빼도 거절한다. 4/5는 읽을 때 Chain을 만들지 않고, 일반 commit도 원래 형식을 유지한다.

format 6/7의 World 부분은 다음과 같다. Existence/Will/Journey/Artifact registry는 기존 뿌리에
그대로 있고 한 `state.yaml`에 원자적으로 저장한다.

```yaml
chains:
  current: chain:C1
  next_id: 2
  nodes:
  - id: C1
    existence_ref: existence:X1
    state: open
    parent_chain_ref: null
    entry_snapshot_ref: snapshot:A1
    exit_snapshot_ref: null
    cycles:
      current: 1
      next_id: 2
      nodes: [...] # 기존 Cycle/Step 저장 계약
```

- 루트의 별도 `cycles`는 없다. 모든 Cycle은 정확히 한 Chain 안에 저장된다. 중복 graph,
  Chain 없는 Cycle, 거짓 Current, 추가 Chain/부모, 알 수 없는 소속 필드는 거절한다.
- 첫 Chain C1과 첫 Interview Cycle C1, X1/ES0/J0/A1은 한 번의 시작 저장으로 생성한다.
  Step·Will·Chain Open Report·인간 승인은 생성하지 않는다.
- Entry는 최초 관측 세계이자 첫 Interview Entry와 같아야 하며 Artifact registry에 실재한다.
  Open Chain의 Exit은 없다. Cycle이 닫혀도 Chain은 열린 상태다.
- Chain을 연 Existence는 내부 모든 Cycle의 실행 주체와 같고, Open Chain 중에 바뀌지 않는다.
- World Current 경로는 `chains.current → cycles.current → 현재 Step`이다. 세계 Snapshot은
  기존 가장 깊은 Cycle의 Entry/Verify/Exit 규칙에서 유도한다. Chain Entry로 매번 되돌리지 않는다.
- Cycle 생성·종료·revisit은 소유 Chain의 graph를 바꾼다. 기존 원자적 저장과 단일 활성 Cycle/
  Step 제약을 그대로 적용한다. 컨테이너 전이 자체로 Will이나 Journey revision을 만들지 않는다.
- `chain:C1`, `cycle:C1`, `step:C1/S1`은 서로 다른 주소다. CycleRef/StepRef를 다시 쓰지 않는다.
  M6-A에는 Chain이 하나이므로 내부 Cycle 발급기가 프로젝트 전체 high-water mark다.
  M6-C가 여러 Chain을 허용하기 전에 이 발급기를 프로젝트 전역으로 옮기는 저장 계약을 정해야 한다.

## 기존 기록의 명시적 이전 설계 — 아직 적용 명령 없음

4/5는 정상적인 legacy world로 계속 읽고 쓴다. 새 빌드를 쓰거나 `start`를 다시 부른다는 이유로
변환하지 않는다. 현재 visual migration은 **4→5만** 지원하며 Chain 이전에 재사용하지 않는다.

향후 Chain 이전은 사용자에게 구조·호환성 변경을 보여 준 후 명시적으로 실행한다. 필수 절차:

1. 읽기 전용 사전 검증, 기존 참조/Artifact/Journey/Will 목록과 state digest를 고정한다.
2. 사용자가 선택한 별도 경로에 모든 기록·객체를 보존하는 백업을 만들고 다시 읽어 검증한다.
3. 프로젝트 잠금 안에서 digest가 그대로인지 다시 검사한다. 기존 Cycle graph 전체를 한
   legacy-adopted Chain에 담는 후보와 이전 경계/출처를 별도 명세화한다. 과거의 생성 사건이나
   승인으로 위장하지 않는다. 과거 Synthesis·Report·Cycle/Step ID·Journey를 바꾸지 않는다.
4. 후보의 모든 불변식과 객체를 검증한 뒤 state만 원자적으로 교체한다. 실패하면 기존 state와
   백업을 보존한다. 작업 파일을 restore하거나 기존 객체를 지우지 않는다.
5. 구형 writer가 새 형식을 거절함을 확인한다. 역변환은 자동 제공하지 않는다. 백업 복원은
   이후 작업을 덮어쓸 수 있으므로 별도 명시적 동작으로 설계한다.

legacy-adopted 출처 schema와 실제 이전 API는 후속 작업이다. 현재 저장기에 임의 Chain을
끼워 넣는 함수나 사용자 기록 일괄 변환을 노출하지 않는다. M6-A의 완료 범위는 이 이전 설계와
legacy 보존/구형 writer 거절 검증까지다.

## 읽기와 UI

CLI start/status/context/monitor 및 MCP가 같은 Chain을 투영한다. Monitor v1에는 선택적
`chain` 객체와 `current.chain_ref`를 추가한다. `chain`에는 owner, state, parent, Entry/Exit,
current_cycle_ref, cycle_refs가 있다. legacy에는 두 필드를 생략한다. 기존 v1 소비자는 추가
필드를 무시할 수 있고, 갱신된 공용 UI는 현재 경로와 기록 요약에 실제 Chain을 표시한다.
조회는 상태 저장·가상 Chain/Report 생성·인간 승인 추론을 하지 않는다.

## 검증

- `tests/chains.rs`: 실제 포함 관계, Interview 완료 후 Experiment, 열린 Step/Will/Journey/세계
  재시작 복원, 동일 바이트 재저장, 16개 위조/고아/경계 거절, legacy 4/5 보존, CLI 프로세스 재시작.
- `tests/mcp.rs::chain_profile_stdio_start_monitor_and_restart_preserve_containment`:
  실제 stdio start→Monitor→프로세스 재시작→status/context와 기록 불변 확인.
- 기존 Cycle/Context/Will/Walk 시험 및 누적 실패의 반복 revisit 시험을 Chain 프로필에서도 실행한다.
- 기본 프로필 전체 Core·UI·배포 회귀시험은 별도로 수행한다. 기본 프로필 통과를 전체
  chain-core 프로필/Windows 실기/실제 fullscreen UI 인수 통과로 표시하지 않는다.

### 로컬 실행 결과 (macOS arm64, 2026-10-31)

- 기본 Core 전체: 1,432 통과, 2 ignored. `--all-targets` 빌드 통과.
- Chain 프로필: Chain/Context/Cycle/Walk/Will 141 통과, 누적 실패 반복 revisit 1 통과.
- Chain + visual-authoring + product-safety: 생성 장애 주입 9 통과/1 ignored,
  Chain·visual_start 통합 7 통과, native MCP 재시작 1 통과, `--all-targets` 빌드 통과.
- 공용 UI/Plugin 110 통과/4 skip, 배포·notice 검증 95 통과/1 skip.
- 초기 전체 실행의 sandbox IPC 거절은 IPC 허용 실행으로 재확인했다. 기존 시험의 루트 Cycle
  저장 경로·고정 format 5·Chain 부재 가정은 새 containment/profile에도 맞게 조정하고 다시 검증했다.
- `.github/workflows/chain-core.yml`은 macOS/Windows native CI에서 별도 실행한다.
  위 로컬 수치를 Windows 실기 또는 실제 Codex fullscreen 화면 인수로 해석하지 않는다.
