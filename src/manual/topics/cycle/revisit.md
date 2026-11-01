---
id: cycle/revisit
title: 실패한 Cycle 에서 되돌아가 새 갈래를 연다
summary: 닫힌 실패 Cycle 이 적어 둔 조상으로 돌아가 그 세계를 복원하고, 그 아래에 새 Cycle 을 연다. 두 걸음이다.
applies_when:
  cycle_status: closed
related:
  - cycle/revisit/target
  - cycle/experiment/close
  - current
---

[언제 읽는가]
  Experiment 를 `failure` 와 `revisit` 으로 닫은 뒤, 다음에 무엇을 할지 물을 때.
  `gil revisit` 이 거절했을 때.

[지금 할 일]
  제품판이 Monitor 승인을 요구하면 아래 명령을 먼저 실행하지 않는다.
  Monitor에서 변경 목록과 보존 위치를 확인하고 **보존 후 되돌아가기**를 누른다.
  복원 완료를 확인한 다음 `gil open <interview|experiment>`로 새 Cycle을 연다.
  채팅 동의는 버튼 승인이 아니다. 중단된 승인 복원만 receipt 안내에 따라 `gil revisit`으로 재개한다.
  보호 대상 변경이나 미지원 플랫폼으로 거절되면 기존 기록과 파일을 보존하고 중단한다.

  되돌아가는 것과 새 갈래를 여는 것은 **두 걸음**이다.

      gil revisit
      gil open <interview|experiment>

  첫 걸음은 인수를 받지 않는다. 갈 곳은 Cycle 을 닫을 때 이미 확정됐다.

[불변식]
  되돌아가는 곳은 **조상**이다 — 형제가 아니다. 형제가 되는 것은 그 뒤에 여는 새 Cycle 이고,
  대상이 실패 Cycle 의 직접 부모일 때만 그렇다. 더 먼 조상으로 가면 새 Cycle 은 그 조상의
  자식이지 실패 Cycle 의 형제가 아니다.

  `gil revisit` 은 **새 Cycle 을 만들지 않는다.** 서 있는 자리를 대상으로 옮기고, 그 대상이
  확정했던 세계로 작업 폴더를 되돌린다. Kind 를 고르는 것은 다음 걸음이다.

  Artifact 만 되돌아간다. Journey 는 전진한다 — 실패 Cycle 과 그 Report, 거기서 얻은
  Knowledge, 완료된 Will 은 하나도 사라지지 않는다. 실패가 확정했던 Snapshot 객체도 남는다.
  사라지는 것은 작업 폴더에 비친 모습뿐이다.

  새 Cycle 은 되돌아간 대상을 부모로 삼고, 그 대상의 Exit 세계에서 출발하며, 버린 실패
  Cycle 을 갈래의 출처로 지닌다. 그 출처는 **계보의 변이 아니다** — 계보는 부모만 따라간다.

  같은 실패 Cycle 에서 두 번 되돌아갈 수는 없다. 그러나 새로 연 Cycle 도 실패하면 그 Cycle 의
  Report 가 같은 조상을 다시 대상으로 삼을 수 있다. 시도 횟수에 상한은 없다.

  Cycle 을 여는 것은 컨테이너를 여는 것이라 행동 계약을 적지 않는다.

[올바른 예]
      gil revisit
      → 출처: cycle:C3 · experiment · failure
        대상: cycle:C2 · closed
        Artifact 세계: snapshot:A2 로 복원했다

      gil open experiment
      → 열었다: cycle:C4 · experiment · 부모 cycle:C2
        갈래 출처: cycle:C3

[흔한 실패]
  되돌아갈 곳을 인수로 적는 것.

      gil revisit cycle:C2

  거절된다. 갈 곳은 Cycle 을 닫는 순간 Report 에 확정됐다.

  작업 폴더가 dirty 인 채로 되돌아가려는 것. 되돌아감은 기준 세계와 같을 때만 실행한다 —
  현재 변경을 보존하고 이유를 확인한다. 제품판에서는 `gil restore`를 자동 실행하거나 기록을 고치지 않는다.

  실패한 Cycle 을 직접 다시 열려는 것. 닫힌 Cycle 은 바뀌지 않는다. 새 시도는 조상 아래의
  **새 Cycle** 이다.

[관련 주제]
  cycle/revisit/target
